/* Linker script para programas userspace do Stenzel OS */

ENTRY(_start)

/* Endereço base para programas de usuário */
/* Abaixo do limite canonical (0x0000_7FFF_FFFF_FFFF) */
USER_BASE = 0x400000;

/* Stack do usuário (cresce para baixo) */
USER_STACK_TOP = 0x7FFFFFFFE000;
USER_STACK_SIZE = 0x100000; /* 1MB */

SECTIONS
{
    . = USER_BASE;

    /* Código executável */
    .text : ALIGN(4K)
    {
        *(.text.start)  /* Entry point primeiro */
        *(.text .text.*)
    }

    /* Dados somente leitura */
    .rodata : ALIGN(4K)
    {
        *(.rodata .rodata.*)
    }

    /* Dados inicializados */
    .data : ALIGN(4K)
    {
        *(.data .data.*)
    }

    /* Dados não inicializados (zerados) */
    .bss : ALIGN(4K)
    {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }

    /* Seções de debug (não carregadas na memória) */
    /DISCARD/ :
    {
        *(.comment)
        *(.note.*)
        *(.eh_frame*)
    }
}
