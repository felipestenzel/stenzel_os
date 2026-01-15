//! Programas user-mode embutidos (ring3) para validação.
//!
//! A ideia é demonstrar:

#![allow(dead_code)]
//! - Entrada em ring3 via `iretq`
//! - Syscall via instrução `syscall`
//! - Preempção via timer IRQ alternando entre dois processos
//!
//! Em um sistema "de verdade", estes binários virão de um loader ELF e/ou
//! initramfs. Aqui nós mantemos um blob mínimo para garantir um bring-up sólido.

use core::arch::global_asm;
use core::slice;

global_asm!(r#"
.section .rodata
.global stenzel_userprog_a_start
.global stenzel_userprog_a_end
.global stenzel_userprog_b_start
.global stenzel_userprog_b_end

// User program A
stenzel_userprog_a_start:
    // write(1, msg, len)
    mov $1, %rax
    mov $1, %rdi
    lea msg_a(%rip), %rsi
    mov $(msg_a_end - msg_a), %rdx
    syscall
    // delay loop
    mov $5000000, %rcx
1:
    loop 1b
    jmp stenzel_userprog_a_start

msg_a:
    .ascii "[user-a] ola do ring3 via syscall!\n"
msg_a_end:

stenzel_userprog_a_end:

// User program B
stenzel_userprog_b_start:
    mov $1, %rax
    mov $1, %rdi
    lea msg_b(%rip), %rsi
    mov $(msg_b_end - msg_b), %rdx
    syscall
    mov $5000000, %rcx
2:
    loop 2b
    jmp stenzel_userprog_b_start

msg_b:
    .ascii "[user-b] alternando por timer IRQ (preempt)!\n"
msg_b_end:

stenzel_userprog_b_end:
"#, options(att_syntax));

extern "C" {
    static stenzel_userprog_a_start: u8;
    static stenzel_userprog_a_end: u8;
    static stenzel_userprog_b_start: u8;
    static stenzel_userprog_b_end: u8;
}

pub fn prog_a_bytes() -> &'static [u8] {
    unsafe {
        let start = &stenzel_userprog_a_start as *const u8;
        let end = &stenzel_userprog_a_end as *const u8;
        slice::from_raw_parts(start, end as usize - start as usize)
    }
}

pub fn prog_b_bytes() -> &'static [u8] {
    unsafe {
        let start = &stenzel_userprog_b_start as *const u8;
        let end = &stenzel_userprog_b_end as *const u8;
        slice::from_raw_parts(start, end as usize - start as usize)
    }
}
