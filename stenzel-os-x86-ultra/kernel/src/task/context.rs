    use core::arch::naked_asm;

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct Context {
        pub rsp: u64,
    }

    /// Prepara stack para uma thread nova.
    ///
    /// Layout esperado pela `context_switch`:
    ///   [rsp + 0]  = rbx
    ///   [rsp + 8]  = rbp
    ///   [rsp + 16] = r12
    ///   [rsp + 24] = r13
    ///   [rsp + 32] = r14
    ///   [rsp + 40] = r15
    ///   [rsp + 48] = return RIP (thread_trampoline)
    ///
    /// Retorna o `rsp` inicial.
    pub fn prepare_stack(stack_top_aligned_16: u64, entry_rip: u64) -> u64 {
        let mut sp = stack_top_aligned_16;

        unsafe fn push(sp: &mut u64, v: u64) {
            *sp -= 8;
            let p = *sp as *mut u64;
            p.write(v);
        }

        // push ret addr primeiro, depois registradores em ordem reversa
        unsafe {
            push(&mut sp, entry_rip); // ret
            push(&mut sp, 0); // r15
            push(&mut sp, 0); // r14
            push(&mut sp, 0); // r13
            push(&mut sp, 0); // r12
            push(&mut sp, 0); // rbp
            push(&mut sp, 0); // rbx
        }

        sp
    }

    /// Troca de contexto entre duas threads (cooperativa).
    ///
    /// # Safety
    /// Deve ser chamada com IRQs desabilitadas e garantindo que `old` e `new` apontam para
    /// Contexts válidos e vivos.
    #[unsafe(naked)]
    pub unsafe extern "C" fn context_switch(_old: *mut Context, _new: *const Context) {
        naked_asm!(
            "push r15",
            "push r14",
            "push r13",
            "push r12",
            "push rbp",
            "push rbx",
            "mov [rdi], rsp",     // old.rsp = rsp
            "mov rsp, [rsi]",     // rsp = new.rsp
            "pop rbx",
            "pop rbp",
            "pop r12",
            "pop r13",
            "pop r14",
            "pop r15",
            "ret",
        );
    }
