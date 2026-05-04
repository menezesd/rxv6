OUTPUT_FORMAT("elf32-i386")
OUTPUT_ARCH(i386)
ENTRY(_start)

SECTIONS
{
    . = 0x08048000 + SIZEOF_HEADERS;
    .text : { *(.text .text.*) }
    .rodata : { *(.rodata .rodata.*) }
    . = ALIGN(0x1000);
    .data : { *(.data .data.*) }
    .bss : { *(.bss .bss.*) *(COMMON) }
    /DISCARD/ : { *(.eh_frame) *(.comment) }
}
