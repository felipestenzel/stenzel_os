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

use crate::arch::x86_64_arch::{apic, nmi, pic};
use nmi::NmiReason;

/// Vetores PIC remapeados.
pub const PIC_OFFSET1: u8 = 32;
pub const PIC_OFFSET2: u8 = 40;

/// IRQ0 -> 32
pub const IRQ_TIMER: u8 = PIC_OFFSET1 + 0;
/// IRQ1 -> 33
pub const IRQ_KEYBOARD: u8 = PIC_OFFSET1 + 1;
/// IRQ12 -> 44 (mouse)
pub const IRQ_MOUSE: u8 = PIC_OFFSET2 + 4;

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
.global stenzel_isr44
.global stenzel_isr240
.global stenzel_isr241
.global stenzel_isr242
.global stenzel_isr243
.global stenzel_isr244

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

// IRQ12 mouse
stenzel_isr44:
    pushq $0
    pushq $44
    jmp stenzel_isr_common

// IPI: Reschedule
stenzel_isr240:
    pushq $0
    pushq $240
    jmp stenzel_isr_common

// IPI: TLB Shootdown
stenzel_isr241:
    pushq $0
    pushq $241
    jmp stenzel_isr_common

// IPI: Call Function
stenzel_isr242:
    pushq $0
    pushq $242
    jmp stenzel_isr_common

// IPI: Stop CPU
stenzel_isr243:
    pushq $0
    pushq $243
    jmp stenzel_isr_common

// IPI: Panic
stenzel_isr244:
    pushq $0
    pushq $244
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

    // Check if returning to user mode (CS RPL == 3)
    // CS is at offset 8 from current RSP (after error/vector skip)
    testq $3, 8(%rsp)
    jz 1f
    // Returning to user mode: set DS and ES to user data selector (0x1b)
    push %rax
    mov $0x1b, %ax
    mov %ax, %ds
    mov %ax, %es
    pop %rax
1:
    iretq
"#, options(att_syntax));

extern "C" {
    fn stenzel_isr32();
    fn stenzel_isr33();
    fn stenzel_isr44();
    fn stenzel_isr240();
    fn stenzel_isr241();
    fn stenzel_isr242();
    fn stenzel_isr243();
    fn stenzel_isr244();
}

/// Envia EOI para o controlador de interrupções apropriado
#[inline]
fn send_eoi(irq: u8) {
    if apic::is_enabled() {
        apic::eoi();
    } else {
        pic::eoi(irq);
    }
}

/// IPI vectors
pub const IPI_RESCHEDULE: u8 = 240;
pub const IPI_TLB_SHOOTDOWN: u8 = 241;
pub const IPI_CALL_FUNCTION: u8 = 242;
pub const IPI_STOP: u8 = 243;
pub const IPI_PANIC: u8 = 244;

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
            // EOI antes do schedule para manter o controlador destravado.
            send_eoi(0);
            crate::sched::on_timer_tick(tf)
        }
        IRQ_KEYBOARD => {
            // Leitura do scancode pelo i8042 (0x60)
            let sc = unsafe { x86_64::instructions::port::Port::<u8>::new(0x60).read() };
            // Processa no driver de teclado
            crate::drivers::keyboard::process_scancode(sc);
            send_eoi(1);
            tf as *mut TrapFrame
        }
        IRQ_MOUSE => {
            // Leitura do byte do mouse pelo i8042 (0x60)
            let byte = unsafe { x86_64::instructions::port::Port::<u8>::new(0x60).read() };
            // Processa no driver de mouse
            crate::drivers::mouse::process_byte(byte);
            send_eoi(12);
            tf as *mut TrapFrame
        }
        // IPI handlers - these handle EOI internally
        IPI_RESCHEDULE | IPI_TLB_SHOOTDOWN | IPI_CALL_FUNCTION | IPI_STOP | IPI_PANIC => {
            super::ipi::handle_ipi(vector);
            tf as *mut TrapFrame
        }
        _ => {
            // IRQ inesperada
            send_eoi(0);
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
            // NMI handler on its own stack to handle even during stack overflow
            idt.non_maskable_interrupt
                .set_handler_fn(nmi_handler)
                .set_stack_index(crate::arch::x86_64_arch::gdt::NMI_IST_INDEX);

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
            idt[IRQ_MOUSE].set_handler_addr(VirtAddr::new(stenzel_isr44 as *const () as u64));

            // IPI handlers (vectors 240-244)
            idt[IPI_RESCHEDULE].set_handler_addr(VirtAddr::new(stenzel_isr240 as *const () as u64));
            idt[IPI_TLB_SHOOTDOWN].set_handler_addr(VirtAddr::new(stenzel_isr241 as *const () as u64));
            idt[IPI_CALL_FUNCTION].set_handler_addr(VirtAddr::new(stenzel_isr242 as *const () as u64));
            idt[IPI_STOP].set_handler_addr(VirtAddr::new(stenzel_isr243 as *const () as u64));
            idt[IPI_PANIC].set_handler_addr(VirtAddr::new(stenzel_isr244 as *const () as u64));
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

/// NMI (Non-Maskable Interrupt) handler
/// NMIs are used for critical hardware events:
/// - Hardware memory errors (ECC/parity)
/// - Watchdog timeouts
/// - System board errors
/// - Debug/profiling (performance counters overflow)
/// - Panic IPI from other CPUs
extern "x86-interrupt" fn nmi_handler(stack_frame: InterruptStackFrame) {
    // NMI handling is critical - need to determine the source

    // Check for panic IPI (cross-CPU panic notification)
    if nmi::is_panic_nmi() {
        nmi::handle_panic_nmi(stack_frame);
    }

    // Check system status ports for NMI source
    let reason = nmi::get_nmi_reason();

    match reason {
        NmiReason::MemoryParity => {
            crate::kprintln!("NMI: Memory parity error detected!");
            nmi::handle_memory_error(&stack_frame);
        }
        NmiReason::IoCheck => {
            crate::kprintln!("NMI: I/O channel check error!");
            nmi::handle_io_error(&stack_frame);
        }
        NmiReason::Watchdog => {
            crate::kprintln!("NMI: Watchdog timeout!");
            nmi::handle_watchdog(&stack_frame);
        }
        NmiReason::PerformanceCounter => {
            // Performance monitoring overflow - used for profiling
            nmi::handle_perf_counter();
        }
        NmiReason::Unknown => {
            crate::kprintln!("NMI: Unknown source\n{:#?}", stack_frame);
            // Re-enable NMI after handling
            nmi::reenable_nmi();
        }
    }
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

    let addr = match Cr2::read() {
        Ok(a) => a,
        Err(_) => {
            crate::kprintln!("PAGE FAULT: Invalid CR2 value");
            crate::arch::halt_loop();
        }
    };

    // PageFaultErrorCode flags:
    // - PROTECTION_VIOLATION: page was present (protection violation) vs not present (missing page)
    // - CAUSED_BY_WRITE: caused by write vs read
    // - USER_MODE: caused by user-mode code vs kernel-mode
    // - INSTRUCTION_FETCH: caused by instruction fetch (NX violation)

    let is_user_mode = error_code.contains(PageFaultErrorCode::USER_MODE);
    let is_write = error_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE);
    let is_protection_violation = error_code.contains(PageFaultErrorCode::PROTECTION_VIOLATION);

    // If page is not present and it's a user-mode fault, try demand paging
    let is_not_present = !is_protection_violation;

    // Case 1: Page not present - try demand paging
    if is_user_mode && is_not_present {
        if let Ok(()) = crate::mm::vma::handle_page_fault(addr.as_u64()) {
            return;
        }
    }

    // Case 2: Write to present but read-only page - check for CoW
    if is_user_mode && is_write && is_protection_violation {
        // This might be a CoW page - get the physical address using current CR3
        if let Some(phys_addr_with_offset) = crate::mm::virt_to_phys_current(addr) {
            // Page-align the physical address (CoW manager uses page-aligned addresses)
            let phys_addr = x86_64::PhysAddr::new(phys_addr_with_offset.as_u64() & !0xFFF);
            let ref_count = crate::mm::cow::ref_count(phys_addr);

            // Check if this is a CoW-managed frame (ref_count >= 1)
            // is_shared() returns true if ref_count > 1, but we also need to handle ref_count == 1
            // because that means this process is the sole owner and just needs WRITABLE flag added
            if ref_count >= 1 {
                // Handle CoW fault - this handles both shared (copy) and sole-owner (just make writable)
                match crate::mm::cow::handle_cow_fault(addr.as_u64(), phys_addr) {
                    Ok(new_frame) => {
                        // Update the page table USING CURRENT CR3 (not kernel mapper!)
                        // This is critical - we need to modify the current process's page table
                        if update_page_mapping_current_cr3(addr, new_frame) {
                            return;
                        }
                    }
                    Err(_) => {
                        // Not a CoW page after all, fall through to SIGSEGV
                    }
                }
            } else {
                // Frame exists but not in CoW manager - might be a genuine read-only page
                // that the process has write permission for (e.g., stack guard)
                // Check VMA permissions via handle_cow_page_fault
                if let Ok(()) = crate::mm::vma::handle_cow_page_fault(addr.as_u64(), phys_addr) {
                    return;
                }
            }
        }
    }

    // If we get here, it's either:
    // 1. Kernel mode fault (bug)
    // 2. Protection violation (SIGSEGV)
    // 3. Invalid address (SIGSEGV)

    if is_user_mode {
        use crate::signal::{sig, segv_code};

        // Determine SIGSEGV code based on fault type
        let si_code = if is_not_present {
            segv_code::SEGV_MAPERR // Address not mapped
        } else {
            segv_code::SEGV_ACCERR // Invalid permissions
        };

        // Check if there's a SIGSEGV handler
        let has_handler = crate::sched::has_signal_handler(sig::SIGSEGV);

        if has_handler {
            // Mark SIGSEGV as pending - will be delivered on next scheduler tick
            // Note: This may cause re-faulting until signal is delivered.
            // TODO: Implement custom page fault stub for proper synchronous signal delivery
            crate::sched::send_signal_to_current(sig::SIGSEGV);
            crate::kprintln!(
                "SIGSEGV: queued for process at {:?} (code={}, has_handler=true)",
                addr, si_code
            );
            // Don't return - fall through to terminate for now
            // Proper implementation would modify the return frame
        }

        // No handler (default action) or can't deliver properly - terminate with SIGSEGV
        crate::kprintln!(
            "SIGSEGV: terminating process at {:?} (code={})",
            addr, si_code
        );
        crate::syscall::sys_exit(128 + sig::SIGSEGV as u64); // 128 + 11 = 139
    } else {
        // Kernel mode fault - this is a bug
        crate::kprintln!("KERNEL PAGE FAULT @ {:?} err={:?}\n{:#?}", addr, error_code, stack_frame);
        crate::arch::halt_loop();
    }
}

// ------------------ CoW page table update helper ------------------

/// Updates a page mapping in the CURRENT process's page table (using CR3).
/// This is used by CoW handling to update the process's page table, not the kernel's.
fn update_page_mapping_current_cr3(
    virt_addr: VirtAddr,
    new_frame: x86_64::structures::paging::PhysFrame<x86_64::structures::paging::Size4KiB>,
) -> bool {
    use x86_64::registers::control::Cr3;
    use x86_64::structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, Size4KiB,
    };

    let phys_offset = crate::mm::physical_memory_offset();
    let (level_4_table_frame, _cr3_flags) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = phys_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    // Create a mapper for the current process's page table
    let mut mapper = unsafe { OffsetPageTable::new(&mut *page_table_ptr, phys_offset) };

    let page: Page<Size4KiB> = Page::containing_address(virt_addr);

    // Unmap the old entry
    if let Ok((_old_frame, flush)) = mapper.unmap(page) {
        flush.flush();
    } else {
        // Page wasn't mapped? This shouldn't happen for CoW
        return false;
    }

    // Map the new frame with writable flag
    let flags = PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | PageTableFlags::USER_ACCESSIBLE;

    // We need a frame allocator for potential page table allocations
    // Use a dummy allocator since we're just remapping an existing entry
    struct DummyAllocator;
    unsafe impl FrameAllocator<Size4KiB> for DummyAllocator {
        fn allocate_frame(
            &mut self,
        ) -> Option<x86_64::structures::paging::PhysFrame<Size4KiB>> {
            None
        }
    }
    let mut dummy = DummyAllocator;

    match unsafe { mapper.map_to(page, new_frame, flags, &mut dummy) } {
        Ok(flush) => {
            flush.flush();
            true
        }
        Err(_) => false,
    }
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
