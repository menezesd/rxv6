OUTPUT_FORMAT("elf32-i386")
OUTPUT_ARCH("i386")
ENTRY(start)

KERNEL_OFFSET = 0xC0000000;

SECTIONS
{
    /* Boot code and data at physical 1MB - runs before paging */
    . = 1M;

    .boot : {
        KEEP(*(.multiboot))
        *(.boot .boot.*)
    }

    .boot.bss (NOLOAD) : ALIGN(4K) {
        *(.boot.bss)
    }

    /* Kernel at virtual 0xC0100000+, loaded at physical 1MB+ */
    . += KERNEL_OFFSET;

    .text ALIGN(4K) : AT(ADDR(.text) - KERNEL_OFFSET) {
        _start_text = .;
        *(.text .text.*)
    }

    .rodata ALIGN(4K) : AT(ADDR(.rodata) - KERNEL_OFFSET) {
        *(.rodata .rodata.*)
        . = ALIGN(4K);
        _end_kernel_text = .;
    }

    .data ALIGN(4K) : AT(ADDR(.data) - KERNEL_OFFSET) {
        *(.data .data.*)
    }

    .bss ALIGN(4K) : AT(ADDR(.bss) - KERNEL_OFFSET) {
        _start_bss = .;
        *(.bss .bss.*)
        *(COMMON)
        _end_bss = .;
    }

    _end = .;

    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
    }
}
