//! Rotinas de troca de contexto baseadas em TrapFrame.
//!
//! `stenzel_switch_to(tf_ptr)`:
//! - Assume que `tf_ptr` aponta para uma TrapFrame no layout definido em
//!   `arch::x86_64_arch::interrupts::TrapFrame`.
//! - Restaura registradores e faz `iretq`.
//!
//! Isso é usado principalmente em syscalls que "não retornam", por exemplo
//! `exit()` que mata o processo atual e troca direto para o próximo.

use core::arch::global_asm;

use crate::arch::x86_64_arch::interrupts::TrapFrame;

global_asm!(r#"
.section .text
.global stenzel_switch_to

stenzel_switch_to:
    // rdi = TrapFrame*
    mov %rdi, %rsp

    pop %rax
    pop %rbx
    pop %rcx
    pop %rdx
    pop %rsi
    pop %rdi
    pop %rbp
    pop %r8
    pop %r9
    pop %r10
    pop %r11
    pop %r12
    pop %r13
    pop %r14
    pop %r15

    add $16, %rsp
    iretq
"#, options(att_syntax));

extern "C" {
    #[link_name = "stenzel_switch_to"]
    fn stenzel_switch_to_asm(tf: *mut TrapFrame) -> !;
}

/// Troca para o TrapFrame informado (não retorna).
pub unsafe fn stenzel_switch_to(tf: *mut TrapFrame) -> ! {
    stenzel_switch_to_asm(tf)
}
