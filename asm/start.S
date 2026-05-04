/* RustOS boot assembly - adapted from Pintos start.S for multiboot */

/* Multiboot constants */
.set MB_MAGIC,    0x1BADB002
.set MB_ALIGN,    1 << 0           /* Align loaded modules on page boundaries */
.set MB_MEMINFO,  1 << 1           /* Provide memory map */
.set MB_FLAGS,    MB_ALIGN | MB_MEMINFO
.set MB_CHECKSUM, -(MB_MAGIC + MB_FLAGS)

.set KERNEL_OFFSET, 0xC0000000

/* PDE flags for 4MB pages */
.set PDE_PRESENT,  0x01
.set PDE_WRITABLE, 0x02
.set PDE_4MB,      0x80
.set PDE_FLAGS,    PDE_PRESENT | PDE_WRITABLE | PDE_4MB

/* CR0/CR4 bits */
.set CR0_PG,  0x80000000          /* Paging */
.set CR0_WP,  0x00010000          /* Write-Protect */
.set CR0_EM,  0x00000004          /* FP Emulation */
.set CR4_PSE, 0x00000010          /* Page Size Extension (4MB pages) */

/*
 * Multiboot header - must be within first 8KB of the kernel image.
 * Placed in its own section so the linker script can put it first.
 */
.section .multiboot, "a"
.align 4
    .long MB_MAGIC
    .long MB_FLAGS
    .long MB_CHECKSUM

/*
 * Boot page directory - used for initial paging setup.
 * Identity maps first 64MB AND maps them at 0xC0000000 (higher half).
 * Uses 4MB pages (PSE) for simplicity.
 */
.section .boot.bss, "aw", @nobits
.align 4096
boot_page_directory:
    .skip 4096

/*
 * Boot code - runs at physical addresses before paging is enabled.
 * Multiboot drops us here in 32-bit protected mode, paging off.
 *   EAX = multiboot magic number (0x2BADB002)
 *   EBX = physical address of multiboot info structure
 */
.section .boot, "ax"
.code32
.global start
.type start, @function
start:
    cli

    /* Save multiboot info pointer for later */
    movl %ebx, (multiboot_info_phys - KERNEL_OFFSET)

    /* Clear the page directory */
    leal boot_page_directory, %edi
    xorl %eax, %eax
    movl $1024, %ecx
    rep stosl

    /*
     * Identity map first 64MB: PDE[0..15]
     * Each 4MB page: phys_addr | PDE_FLAGS (Present + Writable + 4MB)
     */
    leal boot_page_directory, %edi
    movl $PDE_FLAGS, %eax          /* Start at physical 0 */
    movl $16, %ecx
1:
    stosl
    addl $0x00400000, %eax         /* Next 4MB block */
    loop 1b

    /*
     * Higher-half map at 0xC0000000: PDE[768..783]
     * Same physical pages, but accessed via virtual addresses.
     */
    leal (boot_page_directory + 768 * 4), %edi
    movl $PDE_FLAGS, %eax
    movl $16, %ecx
1:
    stosl
    addl $0x00400000, %eax
    loop 1b

    /* Enable PSE (Page Size Extension) in CR4 for 4MB pages */
    movl %cr4, %eax
    orl $CR4_PSE, %eax
    movl %eax, %cr4

    /* Load page directory base register (CR3) */
    leal boot_page_directory, %eax
    movl %eax, %cr3

    /* Enable paging + write-protect + FP emulation in CR0 */
    movl %cr0, %eax
    orl $(CR0_PG | CR0_WP | CR0_EM), %eax
    movl %eax, %cr0

    /*
     * Paging is now active. Both identity and higher-half mappings exist.
     * Jump to higher-half code. We can't use a relative jump here because
     * the .text section is linked at virtual addresses.
     */
    leal higher_half_entry, %eax
    jmp *%eax

/*
 * Higher-half entry point - runs at virtual addresses (0xC0100000+).
 * Now we can use the kernel stack and call into Rust.
 */
.section .text
.type higher_half_entry, @function
higher_half_entry:
    /* Set up the kernel stack */
    movl $stack_top, %esp
    xorl %ebp, %ebp               /* Null-terminate backtraces */

    /* Remove identity mapping: zero PDE[0..15] */
    movl $boot_page_directory, %edi
    addl $KERNEL_OFFSET, %edi      /* Convert to virtual address */
    xorl %eax, %eax
    movl $16, %ecx
    rep stosl

    /* Flush TLB by reloading CR3 */
    movl %cr3, %eax
    movl %eax, %cr3

    /* Call Rust kernel main */
    call main

    /* main() should never return. If it does, halt. */
1:
    cli
    hlt
    jmp 1b

/*
 * Kernel stack - 4KB (one page) in BSS (virtual address space).
 * Must be page-aligned because running_thread() uses ESP & ~0xFFF
 * to find the Thread struct at the page base.
 */
.section .bss
.align 4096
stack_bottom:
    .skip 4096
.global stack_top
stack_top:

/*
 * Multiboot info pointer - saved by boot code, read by Rust kernel.
 * Stored as a physical address; Rust code must add KERNEL_OFFSET.
 */
.section .data
.global multiboot_info_phys
multiboot_info_phys:
    .long 0

/*
 * Physical memory size in 4KB pages (for Pintos compatibility).
 * Will be populated during memory detection.
 */
.global init_ram_pages
init_ram_pages:
    .long 0
