//! Thread/Task management module.

#![allow(dead_code)]

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};

    use crate::security::{self, Cred};
    use crate::sync::{IrqSafeGuard, IrqSafeMutex};
    use spin::Once;

    pub mod context;
    pub mod shell;

    type ThreadFn = fn(usize);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ThreadState {
        Ready,
        Running,
        Blocked,
        Exited,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Tid(pub u64);

    pub struct KernelStack {
        buf: alloc::boxed::Box<[u8]>,
        top: u64,
    }

    impl KernelStack {
        pub fn new(size: usize) -> Self {
            // Aloca uma região de stack para a thread.
            let mut v = alloc::vec![0u8; size];
            let top = (v.as_mut_ptr() as u64) + (size as u64);
            Self {
                buf: v.into_boxed_slice(),
                top,
            }
        }

        pub fn top_aligned_16(&self) -> u64 {
            self.top & !0xF
        }
    }

    pub struct Thread {
        pub id: Tid,
        pub name: String,
        pub state: UnsafeCell<ThreadState>,
        pub context: UnsafeCell<context::Context>,
        pub kstack: KernelStack,
        pub entry: ThreadFn,
        pub arg: usize,
        pub cred: UnsafeCell<Cred>,
    }

    unsafe impl Send for Thread {}
    unsafe impl Sync for Thread {}

    impl Thread {
        fn new(id: Tid, name: &str, entry: ThreadFn, arg: usize, cred: Cred) -> Arc<Self> {
            let kstack = KernelStack::new(128 * 1024); // 128KiB por thread (ajustável)
            let t = Thread {
                id,
                name: name.into(),
                state: UnsafeCell::new(ThreadState::Ready),
                context: UnsafeCell::new(context::Context { rsp: 0 }),
                kstack,
                entry,
                arg,
                cred: UnsafeCell::new(cred),
            };

            // Prepara stack e contexto para iniciar em thread_trampoline
            let rsp = context::prepare_stack(t.kstack.top_aligned_16(), thread_trampoline as *const () as u64);
            unsafe { (*t.context.get()).rsp = rsp };

            Arc::new(t)
        }

        #[inline]
        pub fn state(&self) -> ThreadState {
            unsafe { *self.state.get() }
        }

        #[inline]
        pub fn set_state(&self, s: ThreadState) {
            unsafe { *self.state.get() = s };
        }

        pub fn cred(&self) -> Cred {
            unsafe { (*self.cred.get()).clone() }
        }

        pub fn set_cred(&self, c: Cred) {
            unsafe { *self.cred.get() = c };
        }
    }

    struct Scheduler {
        next_tid: u64,
        current: Option<Arc<Thread>>,
        runq: VecDeque<Arc<Thread>>,
    }

    impl Scheduler {
        fn new() -> Self {
            Self {
                next_tid: 1,
                current: None,
                runq: VecDeque::new(),
            }
        }

        fn alloc_tid(&mut self) -> Tid {
            let id = self.next_tid;
            self.next_tid += 1;
            Tid(id)
        }

        fn push_ready(&mut self, t: Arc<Thread>) {
            t.set_state(ThreadState::Ready);
            self.runq.push_back(t);
        }

        fn pop_next(&mut self) -> Option<Arc<Thread>> {
            self.runq.pop_front()
        }
    }

    static SCHED: Once<IrqSafeMutex<Scheduler>> = Once::new();
    static CURRENT: AtomicPtr<Thread> = AtomicPtr::new(core::ptr::null_mut());

    fn sched() -> &'static IrqSafeMutex<Scheduler> {
        SCHED.call_once(|| IrqSafeMutex::new(Scheduler::new()))
    }

    fn sched_lock() -> IrqSafeGuard<'static, Scheduler> {
        sched().lock()
    }


    static NEED_RESCHED: AtomicBool = AtomicBool::new(false);
    static LAST_TICK: AtomicU64 = AtomicU64::new(0);

    pub fn init() {
        // Cria "boot thread" representando o contexto atual.
        let root_cred = security::user_db().login("root").expect("root user");

        let mut g = sched_lock();
        let tid = g.alloc_tid();

        // O boot thread usa a stack atual (do bootloader), então não vamos preparar stack.
        let boot = Arc::new(Thread {
            id: tid,
            name: "boot".into(),
            state: UnsafeCell::new(ThreadState::Running),
            context: UnsafeCell::new(context::Context { rsp: 0 }),
            kstack: KernelStack::new(16 * 1024), // não usado, mas mantemos
            entry: |_| {},
            arg: 0,
            cred: UnsafeCell::new(root_cred),
        });

        CURRENT.store(Arc::as_ptr(&boot) as *mut Thread, Ordering::SeqCst);
        g.current = Some(boot);
    }

    pub fn spawn_kernel(name: &str, entry: ThreadFn, arg: usize) -> Tid {
        let cred = current_cred();
        let mut g = sched_lock();
        let tid = g.alloc_tid();
        let t = Thread::new(tid, name, entry, arg, cred);
        g.push_ready(t);
        tid
    }

    pub fn current() -> Option<&'static Thread> {
        let p = CURRENT.load(Ordering::SeqCst);
        if p.is_null() {
            None
        } else {
            Some(unsafe { &*p })
        }
    }

    pub fn current_cred() -> Cred {
        current()
            .map(|t| t.cred())
            .unwrap_or_else(|| security::Cred::root())
    }

    pub fn set_current_cred(c: Cred) {
        if let Some(t) = current() {
            t.set_cred(c);
        }
    }

    pub fn on_timer_tick() {
        // Marca necessidade de reschedule (coop por enquanto).
        NEED_RESCHED.store(true, Ordering::SeqCst);
        LAST_TICK.store(crate::arch::x86_64_arch::interrupts::ticks(), Ordering::Relaxed);
    }

    pub fn on_keyboard_scancode(_scancode: u8) {
        // TODO: implementar input buffer (para shell via teclado).
        // Hoje o shell usa serial; teclado fica como infraestrutura.
    }

    pub fn yield_now() {
        // Se o scheduler não foi inicializado, apenas espera uma interrupção
        if SCHED.get().is_none() {
            x86_64::instructions::hlt();
            return;
        }

        // Se não há próximo, não faz nada.
        let (old_ctx, new_ctx) = {
            let mut g = sched_lock();

            let Some(old) = g.current.take() else {
                // Sem current, apenas hlt
                drop(g);
                x86_64::instructions::hlt();
                return;
            };
            if old.state() == ThreadState::Running {
                old.set_state(ThreadState::Ready);
                g.runq.push_back(old.clone());
            }

            let Some(new) = g.pop_next() else {
                // ninguém para rodar: volta o old como current
                let old = g.runq.pop_back().expect("runq inconsistente");
                old.set_state(ThreadState::Running);
                CURRENT.store(Arc::as_ptr(&old) as *mut Thread, Ordering::SeqCst);
                g.current = Some(old);
                return;
            };

            new.set_state(ThreadState::Running);
            CURRENT.store(Arc::as_ptr(&new) as *mut Thread, Ordering::SeqCst);
            g.current = Some(new.clone());

            let old_ctx = unsafe { &mut *old.context.get() as *mut context::Context };
            let new_ctx = unsafe { &*new.context.get() as *const context::Context };
            (old_ctx, new_ctx)
        };

        unsafe { context::context_switch(old_ctx, new_ctx) };
    }

    pub fn run() -> ! {
        // Começa rodando qualquer thread pronta.
        yield_now();

        // Loop idle do "boot thread"
        loop {
            if NEED_RESCHED.swap(false, Ordering::SeqCst) {
                yield_now();
            }
            // Se não há nada, dorme.
            x86_64::instructions::hlt();
        }
    }

    extern "C" fn thread_trampoline() -> ! {
        let t = current().expect("thread_trampoline: no current thread");
        let entry = t.entry;
        let arg = t.arg;

        // Executa função da thread.
        entry(arg);

        // Se retornar, encerra.
        exit_current();
    }

    pub fn exit_current() -> ! {
        {
            let mut g = sched_lock();
            let Some(cur) = g.current.take() else {
                crate::kprintln!("task: exit_current sem current");
                crate::arch::halt_loop();
            };
            cur.set_state(ThreadState::Exited);
            // não re-enfileira
            // Próxima thread:
            if let Some(next) = g.pop_next() {
                next.set_state(ThreadState::Running);
                CURRENT.store(Arc::as_ptr(&next) as *mut Thread, Ordering::SeqCst);
                g.current = Some(next.clone());
                let old_ctx = unsafe { &mut *cur.context.get() as *mut context::Context };
                let new_ctx = unsafe { &*next.context.get() as *const context::Context };
                drop(g);
                unsafe { context::context_switch(old_ctx, new_ctx) };
                unreachable!();
            }
        }
        crate::arch::halt_loop();
    }
