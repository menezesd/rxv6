/* Interrupt entry stubs for RustOS.
 * Ported from Pintos intr-stubs.S.
 *
 * Generates 256 interrupt stub functions (intr00_stub .. intrff_stub)
 * and the intr_stubs table of function pointers.
 *
 * Each stub pushes the frame pointer, a dummy error code (or uses the
 * real one the CPU pushed), and the vector number, then jumps to intr_entry.
 */

/*
 * STUB macro: generates one interrupt stub.
 *
 * For vectors WITHOUT a real error code, the stub pushes EBP (frame
 * pointer for backtraces) then a fake error code of 0.
 *
 * For vectors WITH a real error code, the CPU has already pushed the
 * error code onto the stack. The stub swaps it with EBP (so the
 * frame pointer is below the error code) to keep the same stack layout.
 */

/* Stubs where the CPU does NOT push an error code. */
.macro stub_no_error vec
    .align 4
    intr\vec\()_stub:
        pushl %ebp                 /* frame_pointer */
        pushl $0                   /* error_code = 0 (fake) */
        pushl $0x\vec              /* vec_no */
        jmp intr_entry
.endm

/* Stubs where the CPU DOES push an error code. */
.macro stub_error vec
    .align 4
    intr\vec\()_stub:
        /* Stack: ... | EIP | CS | EFLAGS | error_code(real) */
        /* We want: ... | EIP | CS | EFLAGS | frame_pointer | error_code | vec_no */
        /* Swap error_code with EBP: */
        xchgl %ebp, (%esp)        /* EBP <-> error_code; now EBP=error_code, (%esp)=old EBP */
        /* Stack: ... | EIP | CS | EFLAGS | old_EBP(=frame_pointer) */
        pushl %ebp                 /* push error_code (real, was in EBP) */
        movl 4(%esp), %ebp        /* restore EBP from stack (the old_EBP slot) */
        pushl $0x\vec              /* vec_no */
        jmp intr_entry
.endm

/* ---- Generate all 256 stubs ---- */

.text

/* 0x00 - 0x07: no error code */
stub_no_error 00
stub_no_error 01
stub_no_error 02
stub_no_error 03
stub_no_error 04
stub_no_error 05
stub_no_error 06
stub_no_error 07

/* 0x08: double fault - HAS error code */
stub_error 08

/* 0x09: no error code */
stub_no_error 09

/* 0x0a - 0x0e: have error codes */
stub_error 0a
stub_error 0b
stub_error 0c
stub_error 0d
stub_error 0e

/* 0x0f - 0x10: no error code */
stub_no_error 0f
stub_no_error 10

/* 0x11: alignment check - HAS error code */
stub_error 11

/* 0x12 - 0x14: no error code */
stub_no_error 12
stub_no_error 13
stub_no_error 14

/* 0x15: control protection exception - HAS error code */
stub_error 15

/* 0x16 - 0x1c: no error code */
stub_no_error 16
stub_no_error 17
stub_no_error 18
stub_no_error 19
stub_no_error 1a
stub_no_error 1b
stub_no_error 1c

/* 0x1d - 0x1e: have error codes */
stub_error 1d
stub_error 1e

/* 0x1f - 0xff: no error code (software/IRQ vectors) */
stub_no_error 1f

/* Macro to generate 16 stubs in a row */
.macro stubs_16 prefix
    stub_no_error \prefix\()0
    stub_no_error \prefix\()1
    stub_no_error \prefix\()2
    stub_no_error \prefix\()3
    stub_no_error \prefix\()4
    stub_no_error \prefix\()5
    stub_no_error \prefix\()6
    stub_no_error \prefix\()7
    stub_no_error \prefix\()8
    stub_no_error \prefix\()9
    stub_no_error \prefix\()a
    stub_no_error \prefix\()b
    stub_no_error \prefix\()c
    stub_no_error \prefix\()d
    stub_no_error \prefix\()e
    stub_no_error \prefix\()f
.endm

stubs_16 2
stubs_16 3
stubs_16 4
stubs_16 5
stubs_16 6
stubs_16 7
stubs_16 8
stubs_16 9
stubs_16 a
stubs_16 b
stubs_16 c
stubs_16 d
stubs_16 e
stubs_16 f

/* ---- Common interrupt entry point ---- */

.align 16
.globl intr_entry
intr_entry:
    /* Stack at this point (from high to low):
     *   [SS, ESP]          -- only if privilege change
     *   EFLAGS
     *   CS
     *   EIP
     *   frame_pointer      -- pushed by stub
     *   error_code          -- pushed by stub (or CPU)
     *   vec_no             -- pushed by stub
     *   <- ESP
     */

    /* Save segment registers */
    pushl %ds
    pushl %es
    pushl %fs
    pushl %gs

    /* Save general-purpose registers */
    pushal

    /* Load kernel data segment into DS, ES */
    movl $0x10, %eax
    movl %eax, %ds
    movl %eax, %es

    /* Push pointer to IntrFrame as argument to intr_handler */
    pushl %esp
    call intr_handler
    addl $4, %esp

    /* Fall through to intr_exit */

.globl intr_exit
intr_exit:
    /* Restore general-purpose registers */
    popal

    /* Restore segment registers */
    popl %gs
    popl %fs
    popl %es
    popl %ds

    /* Skip vec_no, error_code, frame_pointer */
    addl $12, %esp

    iret

/* ---- Stub pointer table ---- */

.section .rodata
.align 4
.globl intr_stubs
intr_stubs:

.macro stub_entry vec
    .long intr\vec\()_stub
.endm

stub_entry 00
stub_entry 01
stub_entry 02
stub_entry 03
stub_entry 04
stub_entry 05
stub_entry 06
stub_entry 07
stub_entry 08
stub_entry 09
stub_entry 0a
stub_entry 0b
stub_entry 0c
stub_entry 0d
stub_entry 0e
stub_entry 0f

stub_entry 10
stub_entry 11
stub_entry 12
stub_entry 13
stub_entry 14
stub_entry 15
stub_entry 16
stub_entry 17
stub_entry 18
stub_entry 19
stub_entry 1a
stub_entry 1b
stub_entry 1c
stub_entry 1d
stub_entry 1e
stub_entry 1f

/* Macro for 16 table entries */
.macro entries_16 prefix
    stub_entry \prefix\()0
    stub_entry \prefix\()1
    stub_entry \prefix\()2
    stub_entry \prefix\()3
    stub_entry \prefix\()4
    stub_entry \prefix\()5
    stub_entry \prefix\()6
    stub_entry \prefix\()7
    stub_entry \prefix\()8
    stub_entry \prefix\()9
    stub_entry \prefix\()a
    stub_entry \prefix\()b
    stub_entry \prefix\()c
    stub_entry \prefix\()d
    stub_entry \prefix\()e
    stub_entry \prefix\()f
.endm

entries_16 2
entries_16 3
entries_16 4
entries_16 5
entries_16 6
entries_16 7
entries_16 8
entries_16 9
entries_16 a
entries_16 b
entries_16 c
entries_16 d
entries_16 e
entries_16 f
