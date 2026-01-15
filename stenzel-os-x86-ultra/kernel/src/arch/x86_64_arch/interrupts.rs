//! Interrupções (IDT) + stubs em assembly.
//!
//! Neste estágio, nós **não** usamos apenas `extern "x86-interrupt"` para tudo,
//! porque queremos um caminho de *preempção real* onde o handler pode decidir
//! retornar para outra task (trocando a `TrapFrame`).
//!
//! Estratégia:
//! - IRQ0 (timer) e IRQ1 (keyboard) usam stubs em assembly que:
//!   - empilham registradores em um layout `TrapFrame`
//!   - chamam um dispatcher Rust que pode escolher uma outra `TrapFrame` para retomar
//! - Exceções (page fault, general protection, etc.) continuam em Rust via
//!   `extern "x86-interrupt"` por simplicidade e para termos mensagens melhores.

#![allow(dead_code)]

use core::arch::global_asm;
use core::sync::atomic::{AtomicU64, Ordering};

use spin::Once;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use x86_64::VirtAddr;

use crate::arch::x86_64_arch::pic;

/// Vetores PIC remapeados.
pub const PIC_OFFSET1: u8 = 32;
pub const PIC_OFFSET2: u8 = 40;

/// IRQ0 -> 32
pub const IRQ_TIMER: u8 = PIC_OFFSET1 + 0;
/// IRQ1 -> 33
pub const IRQ_KEYBOARD: u8 = PIC_OFFSET1 + 1;

static IDT: Once<InterruptDescriptorTable> = Once::new();
static TICKS: AtomicU64 = AtomicU64::new(0);

/// Layout de TrapFrame exatamente como o stub em assembly empilha.
///
/// Ordem (do menor endereço para o maior) quando `&TrapFrame` aponta para o topo:
/// rax, rbx, rcx, rdx, rsi, rdi, rbp, r8..r15, vector, error, CPU frame (rip,cs,rflags,rsp,ss)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapFrame {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub vector: u64,
    pub error: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

global_asm!(r#"
.section .text
.global stenzel_isr32
.global stenzel_isr33

// IRQ0 timer
stenzel_isr32:
    pushq $0
    pushq $32
    jmp stenzel_isr_common

// IRQ1 keyboard
stenzel_isr33:
    pushq $0
    pushq $33
    jmp stenzel_isr_common

stenzel_isr_common:
    // Salva GPRs
    push %r15
    push %r14
    push %r13
    push %r12
    push %r11
    push %r10
    push %r9
    push %r8
    push %rbp
    push %rdi
    push %rsi
    push %rdx
    push %rcx
    push %rbx
    push %rax

    // rdi = &TrapFrame
    mov %rsp, %rdi

    // Alinha stack para chamada SysV (16 bytes antes do call)
    mov %rsp, %rax
    and $-16, %rsp

    call stenzel_interrupt_dispatch

    // rax = TrapFrame* para retomar
    mov %rax, %rsp

    // Restaura GPRs do frame escolhido
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

    add $16, %rsp // vector + error
    iretq
"#, options(att_syntax));

extern "C" {
    fn stenzel_isr32();
    fn stenzel_isr33();
}

/// Dispatcher chamado do stub em assembly.
/// Retorna ponteiro para a `TrapFrame` que deve ser retomada.
#[no_mangle]
pub extern "C" fn stenzel_interrupt_dispatch(tf: &mut TrapFrame) -> *mut TrapFrame {
    let vector = tf.vector as u8;

    match vector {
        IRQ_TIMER => {
            TICKS.fetch_add(1, Ordering::Relaxed);
            // Atualiza o contador de tempo do sistema
            crate::time::tick();
            // EOI antes do schedule para manter o PIC destravado.
            pic::eoi(0);
            let ret_tf = crate::sched::on_timer_tick(tf);
            // Debug: mostra switch de contexto
            if ret_tf != tf as *mut TrapFrame {
                let new_tf = unsafe { &*ret_tf };
                crate::kprintln!(
                    "[sched] switch to rip={:#x} cs={:#x} rsp={:#x}",
                    new_tf.rip, new_tf.cs, new_tf.rsp
                );
            }
            ret_tf
        }
        IRQ_KEYBOARD => {
            // Leitura do scancode pelo i8042 (0x60)
            let sc = unsafe { x86_64::instructions::port::Port::<u8>::new(0x60).read() };
            // Processa no driver de teclado
            crate::drivers::keyboard::process_scancode(sc);
            pic::eoi(1);
            tf as *mut TrapFrame
        }
        _ => {
            // IRQ inesperada
            pic::eoi(0);
            tf as *mut TrapFrame
        }
    }
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

pub fn init_idt() {
    IDT.call_once(|| {
        let mut idt = InterruptDescriptorTable::new();

        // Exceções principais
        idt.breakpoint.set_handler_fn(breakpoint_handler);

        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(crate::arch::x86_64_arch::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt.general_protection_fault
            .set_handler_fn(general_protection_fault_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);

        // IRQs com stubs custom
        unsafe {
            idt[IRQ_TIMER].set_handler_addr(VirtAddr::new(stenzel_isr32 as *const () as u64));
            idt[IRQ_KEYBOARD].set_handler_addr(VirtAddr::new(stenzel_isr33 as *const () as u64));
        }

        idt
    });

    let idt = IDT.get().expect("IDT não inicializado");
    idt.load();
}

// ------------------ handlers de exceção ------------------

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    crate::kprintln!("EXC: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    crate::kprintln!("EXC: DOUBLE FAULT\n{:#?}", stack_frame);
    crate::arch::halt_loop();
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    crate::kprintln!("EXC: GPF code={}\n{:#?}", error_code, stack_frame);
    crate::arch::halt_loop();
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;
    let addr = Cr2::read();
    crate::kprintln!("EXC: PAGE FAULT @ {:?} err={:?}\n{:#?}", addr, error_code, stack_frame);
    crate::arch::halt_loop();
}

// ------------------ helpers (enable/disable) ------------------

/// Inicializa o subsistema de interrupções (IDT).
pub fn init() {
    init_idt();
}

/// Desabilita interrupções e retorna se estavam habilitadas.
#[inline]
pub fn disable() -> bool {
    let was = x86_64::instructions::interrupts::are_enabled();
    x86_64::instructions::interrupts::disable();
    was
}

/// Restaura o estado de interrupções.
#[inline]
pub fn restore(was_enabled: bool) {
    if was_enabled {
        x86_64::instructions::interrupts::enable();
    }
}

#[inline]
pub fn enable() {
    x86_64::instructions::interrupts::enable();
}

#[inline]
pub fn are_enabled() -> bool {
    x86_64::instructions::interrupts::are_enabled()
}
