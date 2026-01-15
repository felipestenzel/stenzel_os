//! Scheduler preemptivo (round-robin) + tasks (kernel e user).
//!
//! Ponto importante: o caminho de preempção entra pelo IRQ0 (timer), que
//! chama `on_timer_tick(tf)` passando uma `TrapFrame` criada em assembly.
//!
//! O scheduler pode decidir trocar de task retornando um ponteiro para outra
//! TrapFrame (salva na stack do próximo task). O stub em assembly fará
//! `iretq` a partir desse frame.

#![allow(dead_code)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use core::cell::UnsafeCell;
use core::ptr;
use core::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

use spin::Once;

use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{
    Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};

use crate::arch::x86_64_arch::{gdt, syscall};
use crate::arch::x86_64_arch::interrupts::TrapFrame;
use crate::mm;
use crate::security::Cred;
use crate::signal::{SignalState, SignalHandlers};
use crate::sync::IrqSafeMutex;
use crate::util::{KError, KResult};

mod userprog;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskState {
    Ready,
    Running,
    Blocked,  // Aguardando evento (wait, etc)
    Zombie,   // Terminado, aguardando wait do pai
    Exited,   // Terminado e pode ser removido
}

struct KernelStack {
    mem: Box<[u8]>,
}

impl KernelStack {
    fn new(bytes: usize) -> Self {
        let mut v = Vec::with_capacity(bytes);
        v.resize(bytes, 0);
        Self { mem: v.into_boxed_slice() }
    }

    fn top(&self) -> u64 {
        let base = self.mem.as_ptr() as u64;
        (base + self.mem.len() as u64) & !0xF
    }
}

pub struct Task {
    id: u64,
    parent_id: u64,
    /// Thread group ID (processo principal)
    tgid: u64,
    name: String,
    state: UnsafeCell<TaskState>,
    exit_status: UnsafeCell<i32>,
    is_user: bool,
    /// Se é uma thread (compartilha address space com o líder)
    is_thread: bool,
    // Kernel stack próprio para traps/syscalls.
    kstack: KernelStack,
    // Root do page table (CR3) se user task.
    cr3: PhysFrame<Size4KiB>,
    // TrapFrame salva quando o task é preemptado.
    saved_tf: UnsafeCell<*mut TrapFrame>,
    // Credenciais mínimas
    cred: UnsafeCell<Cred>,
    // Estado de signals (pendentes e blocked)
    signals: SignalState,
    // Handlers de signals
    signal_handlers: SignalHandlers,
    /// Endereço para limpar quando a thread terminar (set_tid_address)
    clear_child_tid: UnsafeCell<u64>,
    /// GS base do usuário (TLS) - salvo durante context switch
    user_gs_base: UnsafeCell<u64>,
}

unsafe impl Send for Task {}
unsafe impl Sync for Task {}

impl Task {
    fn state(&self) -> TaskState {
        unsafe { *self.state.get() }
    }
    fn set_state(&self, st: TaskState) {
        unsafe { *self.state.get() = st; }
    }
    fn saved_tf(&self) -> *mut TrapFrame {
        unsafe { *self.saved_tf.get() }
    }
    fn set_saved_tf(&self, tf: *mut TrapFrame) {
        unsafe { *self.saved_tf.get() = tf; }
    }
    fn cred(&self) -> Cred {
        unsafe { *self.cred.get() }
    }
    fn exit_status(&self) -> i32 {
        unsafe { *self.exit_status.get() }
    }
    fn set_exit_status(&self, status: i32) {
        unsafe { *self.exit_status.get() = status; }
    }
    pub fn signals(&self) -> &SignalState {
        &self.signals
    }
    pub fn signal_handlers(&self) -> &SignalHandlers {
        &self.signal_handlers
    }
    fn user_gs_base(&self) -> u64 {
        unsafe { *self.user_gs_base.get() }
    }
    fn set_user_gs_base(&self, base: u64) {
        unsafe { *self.user_gs_base.get() = base; }
    }
}

struct Scheduler {
    next_id: u64,
    current: Arc<Task>,
    runq: VecDeque<Arc<Task>>,
    // Quantum simples em ticks
    quantum: u32,
    remaining: u32,
}

static SCHED: Once<IrqSafeMutex<Scheduler>> = Once::new();
static CURRENT_PTR: AtomicPtr<Task> = AtomicPtr::new(ptr::null_mut());
static NEXT_ID: AtomicU64 = AtomicU64::new(2); // 0=idle, 1=init, next=2

/// Inicializa scheduler e cria processos user (ring3).
///
/// Deve ser chamado *antes* de habilitar interrupções.
pub fn init_and_launch_userspace() {
    init_userspace();
}

fn init_userspace() {
    // Task "idle" usa o contexto atual (stack do boot). Ele só executa `hlt`.
    let (current_cr3, _) = Cr3::read();
    let idle = Arc::new(Task {
        id: 0,
        parent_id: 0,
        tgid: 0,
        name: "idle".into(),
        state: UnsafeCell::new(TaskState::Running),
        exit_status: UnsafeCell::new(0),
        is_user: false,
        is_thread: false,
        kstack: KernelStack::new(0), // não usado (usa a stack atual)
        cr3: current_cr3,
        saved_tf: UnsafeCell::new(ptr::null_mut()),
        cred: UnsafeCell::new(crate::security::Cred::root()),
        signals: SignalState::new(),
        signal_handlers: SignalHandlers::new(),
        clear_child_tid: UnsafeCell::new(0),
        user_gs_base: UnsafeCell::new(0),
    });

    // Cria 2 user tasks para demonstrar preempção.
    let t1 = spawn_user_task("user-a", userprog::prog_a_bytes());
    let t2 = spawn_user_task("user-b", userprog::prog_b_bytes());

    let mut runq = VecDeque::new();
    runq.push_back(t1);
    runq.push_back(t2);

    let sched = Scheduler {
        next_id: 1,
        current: idle.clone(),
        runq,
        quantum: 8,
        remaining: 8,
    };

    SCHED.call_once(|| IrqSafeMutex::new(sched));
    CURRENT_PTR.store(Arc::as_ptr(&idle) as *mut Task, Ordering::Release);

    // Como estamos em idle/kernel agora, configure pelo menos um stack seguro
    // para syscalls (não deve acontecer aqui, mas evita surpresa).
    unsafe { syscall::set_kernel_stack_top(read_rsp()); }

    crate::kprintln!("sched: userspace pronto (2 tasks)");
}

#[inline]
fn read_rsp() -> u64 {
    let rsp: u64;
    unsafe {
        core::arch::asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags));
    }
    rsp
}

pub fn current_task() -> &'static Task {
    let p = CURRENT_PTR.load(Ordering::Acquire);
    assert!(!p.is_null(), "current task não inicializado");
    unsafe { &*p }
}

pub fn current_cred() -> Cred {
    current_task().cred()
}

/// Chamado no IRQ0. Pode devolver uma TrapFrame diferente para retomar outro task.
pub fn on_timer_tick(tf: &mut TrapFrame) -> *mut TrapFrame {
    // Se o scheduler não foi inicializado, apenas retorna o TrapFrame atual
    let Some(sched_lock) = SCHED.get() else {
        return tf as *mut TrapFrame;
    };
    let mut sched = sched_lock.lock();

    // Salva TrapFrame do task atual
    let cur = sched.current.clone();
    let _cur_id = cur.id;
    cur.set_saved_tf(tf as *mut TrapFrame);

    // Se o task atual estava em user mode, salva seu GS_BASE
    // (CS & 3 == 3 significa ring 3 / user mode)
    if cur.is_user && (tf.cs & 3) == 3 {
        unsafe {
            const IA32_GS_BASE: u32 = 0xC0000101;
            let gs_base = x86_64::registers::model_specific::Msr::new(IA32_GS_BASE).read();
            cur.set_user_gs_base(gs_base);
        }
    }

    // Conta quantum e decide se troca
    if sched.remaining > 0 {
        sched.remaining -= 1;
    }
    let do_switch = sched.remaining == 0;

    if !do_switch {
        return tf as *mut TrapFrame;
    }

    sched.remaining = sched.quantum;

    // Coloca o current de volta na fila (se ainda rodável)
    if cur.state() == TaskState::Running {
        cur.set_state(TaskState::Ready);
        sched.runq.push_back(cur);
    }

    // Escolhe próximo
    let mut next = None;
    let mut checked = 0usize;
    while let Some(t) = sched.runq.pop_front() {
        checked += 1;
        match t.state() {
            TaskState::Ready => {
                crate::kprintln!("[pick] task {} is Ready, picking (checked {} tasks)", t.id, checked);
                next = Some(t);
                break;
            }
            TaskState::Exited => {
                // drop
            }
            TaskState::Blocked | TaskState::Zombie => {
                // Mantém na fila, mas não executa
                sched.runq.push_back(t);
            }
            TaskState::Running => {
                // shouldn't
                sched.runq.push_back(t);
            }
        }
    }

    let Some(next) = next else {
        // nada para rodar: continua no current
        return tf as *mut TrapFrame;
    };

    // Prepara troca
    next.set_state(TaskState::Running);
    let next_tf = next.saved_tf();
    if next_tf.is_null() {
        crate::kprintln!("sched: BUG - next tf nulo para task {}", next.id);
        return tf as *mut TrapFrame;
    }

    // Atualiza current
    sched.current = next.clone();
    CURRENT_PTR.store(Arc::as_ptr(&next) as *mut Task, Ordering::Release);

    // Troca address space e stacks (TSS + syscall)
    unsafe {
        if next.is_user {
            crate::kprintln!(
                "sched: switching to task {} - kstack.top={:#x}",
                next.id,
                next.kstack.top()
            );
            let (_, cur_flags) = Cr3::read();
            Cr3::write(next.cr3, cur_flags);
            gdt::set_kernel_stack_top(next.kstack.top());
            syscall::set_kernel_stack_top(next.kstack.top());

            // Verifica se o próximo task vai retornar para user mode
            let next_tf_ref = &*next_tf;
            let returning_to_user = (next_tf_ref.cs & 3) == 3;

            if returning_to_user {
                // Restaura o GS_BASE do usuário para este task
                const IA32_GS_BASE: u32 = 0xC0000101;
                let gs_val = next.user_gs_base();
                crate::kprintln!("[gs_restore] task {} gs_base={:#x}", next.id, gs_val);
                x86_64::registers::model_specific::Msr::new(IA32_GS_BASE)
                    .write(gs_val);
            }

            // CRITICAL: Restore KERNEL_GS_BASE to CPU_LOCAL before iretq to userspace.
            // This is necessary because if we're inside a syscall (swapgs was done),
            // KERNEL_GS_BASE contains user GS base. When we iretq to a different user
            // task, the new task's syscall will do swapgs and get the wrong GS base.
            // Fix: Always ensure KERNEL_GS_BASE = CPU_LOCAL before returning to userspace.
            const IA32_KERNEL_GS_BASE: u32 = 0xC0000102;
            x86_64::registers::model_specific::Msr::new(IA32_KERNEL_GS_BASE)
                .write(syscall::cpu_local_ptr() as u64);
        }
    }

    next_tf
}

/// Syscall exit: marca task atual como Zombie (ou Exited) e troca imediatamente.
///
/// Este caminho **não retorna**.
pub fn exit_current_and_switch(status: u64) -> ! {
    // Escolhe o próximo task e prepara os registradores/CR3.
    // IMPORTANTE: precisamos soltar o lock do scheduler antes de fazer o salto
    // (senão a trava nunca é liberada e o sistema congela no próximo IRQ).

    let (next_tf, is_user, next_cr3, next_kstack_top, next_gs_base) = {
        let sched_lock = SCHED.get().expect("sched não inicializado");
        let mut sched = sched_lock.lock();

        let cur = sched.current.clone();
        cur.set_exit_status(status as i32);

        // Se tem parent, vira Zombie; senão, pode ser descartado
        if cur.parent_id != 0 {
            cur.set_state(TaskState::Zombie);
            // Envia SIGCHLD ao pai e acorda se estiver bloqueado
            for t in sched.runq.iter() {
                if t.id == cur.parent_id {
                    // Envia SIGCHLD
                    t.signals.send(crate::signal::sig::SIGCHLD);
                    // Acorda se estiver bloqueado
                    if t.state() == TaskState::Blocked {
                        t.set_state(TaskState::Ready);
                    }
                    break;
                }
            }
        } else {
            cur.set_state(TaskState::Exited);
        }

        // Escolhe próximo pronto
        let mut next = None;
        while let Some(t) = sched.runq.pop_front() {
            if t.state() == TaskState::Ready {
                next = Some(t);
                break;
            }
        }

        let Some(next) = next else {
            crate::kprintln!("sched: nenhum task restante; halt");
            crate::arch::halt_loop();
        };

        next.set_state(TaskState::Running);
        let next_tf = next.saved_tf();
        assert!(!next_tf.is_null());

        sched.current = next.clone();
        CURRENT_PTR.store(Arc::as_ptr(&next) as *mut Task, Ordering::Release);

        (next_tf, next.is_user, next.cr3, next.kstack.top(), next.user_gs_base())
    };

    unsafe {
        if is_user {
            Cr3::write(next_cr3, Cr3::read().1);
            gdt::set_kernel_stack_top(next_kstack_top);
            syscall::set_kernel_stack_top(next_kstack_top);

            // Verifica se o próximo task vai retornar para user mode
            let next_tf_ref = &*next_tf;
            let returning_to_user = (next_tf_ref.cs & 3) == 3;

            if returning_to_user {
                // Restaura o GS_BASE do usuário para este task
                const IA32_GS_BASE: u32 = 0xC0000101;
                x86_64::registers::model_specific::Msr::new(IA32_GS_BASE)
                    .write(next_gs_base);
            }

            // CRITICAL: Restore KERNEL_GS_BASE to CPU_LOCAL before iretq to userspace.
            const IA32_KERNEL_GS_BASE: u32 = 0xC0000102;
            x86_64::registers::model_specific::Msr::new(IA32_KERNEL_GS_BASE)
                .write(syscall::cpu_local_ptr() as u64);
        }
    }

    unsafe { crate::arch::x86_64_arch::switch::stenzel_switch_to(next_tf) }
}

/// Retorna o PID do task atual.
pub fn current_pid() -> u64 {
    current_task().id
}

/// Aguarda um filho terminar (waitpid).
///
/// - pid == -1: aguarda qualquer filho
/// - pid > 0: aguarda filho específico
/// - status_ptr: ponteiro para onde escrever o status (ou null)
///
/// Retorna o PID do filho que terminou, ou erro.
pub fn wait_for_child(pid: i64, status_ptr: u64) -> Result<u64, KError> {
    let parent = current_task();
    let parent_id = parent.id;

    loop {
        let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
        let mut sched = sched_lock.lock();

        // Procura filhos zombie
        let mut zombie_idx = None;
        let mut has_children = false;

        for (idx, task) in sched.runq.iter().enumerate() {
            if task.parent_id == parent_id {
                has_children = true;

                // Se pid == -1, aceita qualquer filho; senão, só o específico
                let matches = pid == -1 || task.id == pid as u64;

                if matches && task.state() == TaskState::Zombie {
                    zombie_idx = Some(idx);
                    break;
                }
            }
        }

        if let Some(idx) = zombie_idx {
            // Remove o zombie da fila
            let zombie = sched.runq.remove(idx).unwrap();
            let child_pid = zombie.id;
            let exit_status = zombie.exit_status();

            // Marca como Exited para limpeza
            zombie.set_state(TaskState::Exited);

            drop(sched); // Libera o lock antes de acessar memória user

            // Escreve o status se o ponteiro for válido
            if status_ptr != 0 {
                unsafe {
                    let ptr = status_ptr as *mut i32;
                    // Encode como Linux: exit_status << 8
                    *ptr = (exit_status as i32) << 8;
                }
            }

            return Ok(child_pid);
        }

        if !has_children {
            // Sem filhos: ECHILD
            return Err(KError::NoChild);
        }

        // Tem filhos mas nenhum zombie: bloqueia e espera
        // Primeiro, marca o parent como Blocked
        let cur = sched.current.clone();
        cur.set_state(TaskState::Blocked);
        cur.set_saved_tf(core::ptr::null_mut()); // Será salvo no tick

        // Libera o lock
        drop(sched);

        // Yield para outro task
        // Como estamos bloqueados, o scheduler vai pular nós até sermos acordados
        x86_64::instructions::interrupts::enable_and_hlt();

        // Quando acordarmos, voltamos ao loop para verificar novamente
    }
}

/// Envia um signal para um processo pelo PID.
///
/// - pid > 0: envia para o processo específico
/// - pid == 0: envia para todos os processos no mesmo grupo (não implementado)
/// - pid == -1: envia para todos os processos (não implementado)
/// - pid < -1: envia para o grupo -pid (não implementado)
pub fn send_signal(pid: i64, signum: u32) -> Result<(), KError> {
    use crate::signal::sig;

    if signum == 0 || signum >= sig::NSIG {
        return Err(KError::Invalid);
    }

    let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
    let sched = sched_lock.lock();

    // Encontra o processo alvo
    let target = if pid > 0 {
        // Procura na fila
        let target_id = pid as u64;
        let mut found = None;

        // Verifica se é o processo atual
        if sched.current.id == target_id {
            found = Some(sched.current.clone());
        } else {
            // Procura na runqueue
            for task in sched.runq.iter() {
                if task.id == target_id {
                    found = Some(task.clone());
                    break;
                }
            }
        }

        found.ok_or(KError::NotFound)?
    } else {
        // Por enquanto, só suportamos pid > 0
        return Err(KError::NotSupported);
    };

    drop(sched); // Libera o lock antes de processar

    // Envia o signal
    target.signals.send(signum);

    // Se o signal é SIGKILL ou SIGTERM e o processo está bloqueado, acorda-o
    if signum == sig::SIGKILL || signum == sig::SIGTERM || signum == sig::SIGINT {
        if target.state() == TaskState::Blocked {
            target.set_state(TaskState::Ready);
        }
    }

    Ok(())
}

/// Processa signals pendentes para o processo atual.
///
/// Retorna true se o processo deve terminar.
pub fn handle_pending_signals() -> bool {
    use crate::signal::{default_action, DefaultAction};

    let task = current_task();

    while let Some(signum) = task.signals.dequeue() {
        // Verifica se há um handler customizado
        let handler = task.signal_handlers.get(signum);

        match handler {
            Some(action) if action.is_ignored() => {
                // Signal ignorado explicitamente
                continue;
            }
            Some(action) if action.has_handler() => {
                // Tem um handler customizado - será entregue via deliver_signal
                // Por enquanto, apenas registramos que o signal está pendente
                // A entrega real acontece no retorno para userspace
                task.signals.send(signum); // Re-enfileira para entrega
                return false;
            }
            _ => {
                // Usa ação default
                let default = default_action(signum);

                match default {
                    DefaultAction::Terminate | DefaultAction::CoreDump => {
                        // Termina o processo
                        crate::kprintln!("signal: processo {} terminado por signal {}", task.id, signum);
                        return true;
                    }
                    DefaultAction::Stop => {
                        task.set_state(TaskState::Blocked);
                    }
                    DefaultAction::Continue => {
                        if task.state() == TaskState::Blocked {
                            task.set_state(TaskState::Ready);
                        }
                    }
                    DefaultAction::Ignore => {
                        // Ignora
                    }
                }
            }
        }
    }

    false
}

/// Entrega um signal com handler customizado para o processo.
///
/// Configura a stack do usuário com o SignalFrame e retorna
/// (novo_rsp, handler_addr) para executar o handler.
pub fn deliver_signal(tf: &mut TrapFrame) -> Option<(u64, u64)> {
    use crate::signal::{sa_flags, setup_signal_frame};

    let task = current_task();

    // Procura um signal pendente com handler
    while let Some(signum) = task.signals.dequeue() {
        let handler = match task.signal_handlers.get(signum) {
            Some(h) => h,
            None => {
                // Sem handler, usa default (já tratado em handle_pending_signals)
                task.signals.send(signum);
                continue;
            }
        };

        if handler.is_ignored() {
            continue;
        }

        if !handler.has_handler() {
            // SIG_DFL - re-enfileira para tratamento default
            task.signals.send(signum);
            continue;
        }

        // Tem um handler customizado!
        // Prepara o frame na stack do usuário

        // Coleta registradores do TrapFrame
        let regs = [
            tf.r15, tf.r14, tf.r13, tf.r12, tf.r11,
            tf.r10, tf.r9, tf.r8, tf.rbp, tf.rdi,
            tf.rsi, tf.rdx, tf.rcx, tf.rbx, tf.rax, 0,
        ];

        let old_mask = task.signals.blocked_mask();

        // Configura o frame
        if let Some((new_sp, handler_addr)) = setup_signal_frame(
            tf.rsp,
            signum,
            &handler,
            tf.rip,
            tf.rsp,
            tf.rflags,
            &regs,
            old_mask,
        ) {
            // Bloqueia signals durante o handler (se SA_NODEFER não estiver set)
            if (handler.sa_flags & sa_flags::SA_NODEFER) == 0 {
                let new_mask = old_mask | (1u64 << signum) | handler.sa_mask;
                task.signals.set_blocked_mask(new_mask);
            }

            // Se SA_RESETHAND, reseta o handler para default
            if (handler.sa_flags & sa_flags::SA_RESETHAND) != 0 {
                task.signal_handlers.reset(signum);
            }

            // Configura o TrapFrame para executar o handler
            // RDI = signum (primeiro argumento)
            // RSI = ponteiro para siginfo (segundo argumento) - ajustado no frame
            // RDX = ponteiro para ucontext (terceiro argumento) - ajustado no frame
            tf.rdi = signum as u64;
            tf.rsi = new_sp + 8; // &info
            tf.rdx = new_sp + 8 + core::mem::size_of::<crate::signal::Siginfo>() as u64; // &uc
            tf.rsp = new_sp;
            tf.rip = handler_addr;

            crate::kprintln!(
                "signal: entregando signal {} ao processo {}, handler={:#x}",
                signum, task.id, handler_addr
            );

            return Some((new_sp, handler_addr));
        } else {
            // Erro ao configurar frame - termina o processo
            crate::kprintln!("signal: erro ao configurar frame para signal {}", signum);
            return None;
        }
    }

    None
}

/// Substitui o address space do processo atual e salta para o novo código.
///
/// Usado por execve para substituir a imagem do processo.
/// Esta função não retorna.
pub fn exec_replace(new_cr3: PhysFrame<Size4KiB>, entry: u64, stack_pointer: u64) -> ! {
    let task = current_task();

    // Prepara um TrapFrame para a nova execução
    let tf = unsafe { init_user_trapframe(task.kstack.top(), entry, stack_pointer) };

    // Atualiza o TrapFrame salvo do task
    task.set_saved_tf(tf);

    // Reset user_gs_base para o novo processo (fresh start, sem TLS)
    task.set_user_gs_base(0);

    // Troca para o novo address space
    unsafe {
        Cr3::write(new_cr3, Cr3::read().1);
        gdt::set_kernel_stack_top(task.kstack.top());
        syscall::set_kernel_stack_top(task.kstack.top());

        // CRITICAL: Set GS_BASE = 0 for fresh process (no TLS yet)
        // We're inside a syscall so current GS_BASE = CPU_LOCAL (kernel's)
        // After iretq to userspace, GS_BASE needs to be valid user value
        const IA32_GS_BASE: u32 = 0xC0000101;
        x86_64::registers::model_specific::Msr::new(IA32_GS_BASE).write(0);

        // CRITICAL: Restore KERNEL_GS_BASE to CPU_LOCAL before iretq to userspace.
        const IA32_KERNEL_GS_BASE: u32 = 0xC0000102;
        x86_64::registers::model_specific::Msr::new(IA32_KERNEL_GS_BASE)
            .write(syscall::cpu_local_ptr() as u64);
    }

    crate::kprintln!("exec_replace: saltando para entry={:#x}, sp={:#x}", entry, stack_pointer);

    // Salta para o novo código
    unsafe { crate::arch::x86_64_arch::switch::stenzel_switch_to(tf) }
}

// ---------------- fork ----------------

/// Fork a partir de um SyscallFrame (chamado via syscall).
///
/// Converte o SyscallFrame para TrapFrame e chama fork_current.
pub fn fork_current_from_syscall(sf: &crate::arch::x86_64_arch::syscall::SyscallFrame) -> Result<u64, KError> {
    let parent = current_task();

    if !parent.is_user {
        return Err(KError::NotSupported);
    }

    let child_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

    // 1. Cria nova kernel stack para o filho
    let child_kstack = KernelStack::new(64 * 1024);

    // 2. Clona o address space do pai
    let child_cr3 = clone_address_space(parent.cr3)?;

    // 3. Cria TrapFrame para o filho a partir do SyscallFrame
    // O filho será escalonado via iretq, então precisamos de um TrapFrame
    let child_tf = unsafe {
        let mut sp = child_kstack.top() & !0xFu64;
        sp -= core::mem::size_of::<TrapFrame>() as u64;
        let tf = sp as *mut TrapFrame;

        // Zera o TrapFrame
        core::ptr::write_bytes(tf as *mut u8, 0, core::mem::size_of::<TrapFrame>());

        // Copia registradores do SyscallFrame
        (*tf).rax = 0; // Filho retorna 0
        (*tf).rbx = sf.rbx;
        (*tf).rcx = sf.rcx; // RIP de retorno no syscall
        (*tf).rdx = sf.rdx;
        (*tf).rsi = sf.rsi;
        (*tf).rdi = sf.rdi;
        (*tf).rbp = sf.rbp;
        (*tf).r8 = sf.r8;
        (*tf).r9 = sf.r9;
        (*tf).r10 = sf.r10;
        (*tf).r11 = sf.r11; // RFLAGS
        (*tf).r12 = sf.r12;
        (*tf).r13 = sf.r13;
        (*tf).r14 = sf.r14;
        (*tf).r15 = sf.r15;

        // Configura para iretq
        (*tf).rip = sf.rcx; // RIP de retorno (syscall salva em RCX)
        (*tf).cs = (gdt::user_code_selector().0 as u64) | 3;
        (*tf).rflags = sf.r11 | 0x200; // Garante IF=1
        // RSP do usuário está em CPU_LOCAL.user_rsp_tmp
        (*tf).rsp = get_user_rsp();
        (*tf).ss = (gdt::user_data_selector().0 as u64) | 3;

        tf
    };

    // 4. Cria o novo Task
    let child = Arc::new(Task {
        id: child_id,
        parent_id: parent.id,
        tgid: child_id, // Fork cria novo processo, então tgid = pid
        name: format!("{}-child", parent.name),
        state: UnsafeCell::new(TaskState::Ready),
        exit_status: UnsafeCell::new(0),
        is_user: true,
        is_thread: false,
        kstack: child_kstack,
        cr3: child_cr3,
        saved_tf: UnsafeCell::new(child_tf),
        cred: UnsafeCell::new(parent.cred()),
        signals: parent.signals.clone(),
        signal_handlers: parent.signal_handlers.clone(),
        clear_child_tid: UnsafeCell::new(0),
        user_gs_base: UnsafeCell::new(parent.user_gs_base()),
    });

    // 5. Adiciona o filho à fila do scheduler
    {
        let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
        let mut sched = sched_lock.lock();
        sched.runq.push_back(child);
    }

    crate::kprintln!("fork: criado filho {} (pai={})", child_id, parent.id);

    // Pai retorna o PID do filho
    Ok(child_id)
}

/// Obtém o RSP do usuário salvo durante a entrada do syscall.
fn get_user_rsp() -> u64 {
    unsafe {
        (*syscall::cpu_local_ptr()).user_rsp_tmp
    }
}

/// Fork: cria uma cópia do processo atual.
///
/// Retorna o PID do filho para o pai, e 0 para o filho.
/// Retorna erro se não for possível criar o processo.
pub fn fork_current(parent_tf: &TrapFrame) -> Result<u64, KError> {
    

    let parent = current_task();

    if !parent.is_user {
        // Só podemos fazer fork de processos user
        return Err(KError::NotSupported);
    }

    let child_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

    // 1. Cria nova kernel stack para o filho
    let child_kstack = KernelStack::new(64 * 1024);

    // 2. Clona o address space do pai
    let child_cr3 = clone_address_space(parent.cr3)?;

    // 3. Copia o TrapFrame do pai para a kernel stack do filho
    // O filho deve retornar 0 do fork
    let child_tf = unsafe {
        let mut sp = child_kstack.top() & !0xFu64;
        sp -= core::mem::size_of::<TrapFrame>() as u64;
        let tf = sp as *mut TrapFrame;

        // Copia o TrapFrame do pai
        core::ptr::copy_nonoverlapping(parent_tf as *const TrapFrame, tf, 1);

        // Filho retorna 0
        (*tf).rax = 0;

        tf
    };

    // 4. Cria o novo Task
    let child = Arc::new(Task {
        id: child_id,
        parent_id: parent.id,
        tgid: child_id, // Fork cria novo processo, então tgid = pid
        name: format!("{}-child", parent.name),
        state: UnsafeCell::new(TaskState::Ready),
        exit_status: UnsafeCell::new(0),
        is_user: true,
        is_thread: false,
        kstack: child_kstack,
        cr3: child_cr3,
        saved_tf: UnsafeCell::new(child_tf),
        cred: UnsafeCell::new(parent.cred()),
        signals: parent.signals.clone(),
        signal_handlers: parent.signal_handlers.clone(),
        clear_child_tid: UnsafeCell::new(0),
        user_gs_base: UnsafeCell::new(parent.user_gs_base()),
    });

    // 5. Adiciona o filho à fila do scheduler
    {
        let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
        let mut sched = sched_lock.lock();
        sched.runq.push_back(child);
    }

    crate::kprintln!("fork: criado filho {} (pai={})", child_id, parent.id);

    // Pai retorna o PID do filho
    Ok(child_id)
}

/// Clona o address space (page tables) de um processo.
///
/// Copia todas as páginas mapeadas no user space (entradas 0-255 do P4).
/// Usa cópia profunda (não copy-on-write por simplicidade).
fn clone_address_space(parent_cr3: PhysFrame<Size4KiB>) -> Result<PhysFrame<Size4KiB>, KError> {
    use x86_64::structures::paging::PageTableFlags;

    let mut fa = mm::frame_allocator_lock();
    let _phys_off = mm::physical_memory_offset();

    // Aloca novo P4 para o filho
    let child_p4_frame = fa.allocate().ok_or(KError::NoMemory)?;
    let child_p4_virt = mm::phys_to_virt(child_p4_frame.start_address());
    let child_p4_ptr = child_p4_virt.as_mut_ptr::<PageTable>();

    // Zera o P4 do filho
    unsafe { (*child_p4_ptr).zero(); }

    // Acessa o P4 do pai
    let parent_p4_virt = mm::phys_to_virt(parent_cr3.start_address());
    let parent_p4_ptr = parent_p4_virt.as_ptr::<PageTable>();

    unsafe {
        let child_p4 = &mut *child_p4_ptr;
        let parent_p4 = &*parent_p4_ptr;

        // Copia entradas do kernel (256-511) diretamente
        for i in 256..512 {
            child_p4[i] = parent_p4[i].clone();
        }

        // Para o user space (0-255), precisamos fazer cópia profunda
        for i in 0..256 {
            if !parent_p4[i].flags().contains(PageTableFlags::PRESENT) {
                continue;
            }

            // Clona o P3
            let parent_p3_phys = parent_p4[i].addr();
            let child_p3_frame = fa.allocate().ok_or(KError::NoMemory)?;

            child_p4[i].set_addr(
                child_p3_frame.start_address(),
                parent_p4[i].flags(),
            );

            clone_page_table_level(
                parent_p3_phys,
                child_p3_frame.start_address(),
                3,
                &mut *fa,
            )?;
        }
    }

    Ok(child_p4_frame)
}

/// Clona recursivamente um nível de page table.
///
/// IMPORTANTE: Apenas páginas com USER_ACCESSIBLE são clonadas (deep copy).
/// Páginas do kernel (sem USER_ACCESSIBLE) são compartilhadas (shallow copy).
fn clone_page_table_level(
    parent_phys: PhysAddr,
    child_phys: PhysAddr,
    level: u8,
    fa: &mut crate::mm::BitmapFrameAllocator,
) -> Result<(), KError> {
    use x86_64::structures::paging::PageTableFlags;

    let parent_virt = mm::phys_to_virt(parent_phys);
    let child_virt = mm::phys_to_virt(child_phys);

    // Convert to proper references before use
    let parent_table: &PageTable = unsafe { &*parent_virt.as_ptr::<PageTable>() };
    let child_table: &mut PageTable = unsafe { &mut *child_virt.as_mut_ptr::<PageTable>() };

    // Zera a tabela do filho
    child_table.zero();

    for i in 0..512 {
        let parent_entry = &parent_table[i];

        if !parent_entry.flags().contains(PageTableFlags::PRESENT) {
            continue;
        }

        // Se a página/entrada NÃO é acessível pelo usuário, apenas compartilha
        // (copia a entrada sem fazer deep clone)
        let is_user = parent_entry.flags().contains(PageTableFlags::USER_ACCESSIBLE);

        if level == 1 {
            // Nível 1 (PT): página física
            if is_user {
                // Página do usuário: faz deep copy (aloca novo frame e copia conteúdo)
                let parent_frame_phys = parent_entry.addr();
                let child_frame = fa.allocate().ok_or(KError::NoMemory)?;

                // Copia o conteúdo da página
                let parent_page = mm::phys_to_virt(parent_frame_phys).as_ptr::<u8>();
                let child_page = mm::phys_to_virt(child_frame.start_address()).as_mut_ptr::<u8>();
                unsafe {
                    core::ptr::copy_nonoverlapping(parent_page, child_page, 4096);
                }

                child_table[i].set_addr(
                    child_frame.start_address(),
                    parent_entry.flags(),
                );
            } else {
                // Página do kernel: compartilha (shallow copy)
                child_table[i] = parent_entry.clone();
            }
        } else if parent_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            // Huge page: compartilha (não suportado deep clone)
            child_table[i] = parent_entry.clone();
        } else {
            // Nível intermediário: clona recursivamente
            let parent_next_phys = parent_entry.addr();
            let child_next_frame = fa.allocate().ok_or(KError::NoMemory)?;

            child_table[i].set_addr(
                child_next_frame.start_address(),
                parent_entry.flags(),
            );

            clone_page_table_level(
                parent_next_phys,
                child_next_frame.start_address(),
                level - 1,
                fa,
            )?;
        }
    }

    Ok(())
}

// ---------------- user task creation ----------------

const USER_BASE: u64 = 0x0000_0000_0040_0000;
const USER_STACK_TOP: u64 = 0x0000_0000_0080_0000;

fn spawn_user_task(name: &str, prog: &'static [u8]) -> Arc<Task> {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

    // Kernel stack próprio
    let kstack = KernelStack::new(64 * 1024);

    // Address space (novo P4)
    let cr3 = create_user_address_space(prog).expect("falha ao criar address space");

    // Frame inicial na stack do kernel -> iretq para ring3
    let tf_ptr = unsafe { init_user_trapframe(kstack.top(), USER_BASE, USER_STACK_TOP) };

    Arc::new(Task {
        id,
        parent_id: 0, // spawned pelo kernel
        tgid: id, // Novo processo, tgid = pid
        name: name.into(),
        state: UnsafeCell::new(TaskState::Ready),
        exit_status: UnsafeCell::new(0),
        is_user: true,
        is_thread: false,
        kstack,
        cr3,
        saved_tf: UnsafeCell::new(tf_ptr),
        cred: UnsafeCell::new(crate::security::Cred::user(1000, 1000)),
        signals: SignalState::new(),
        signal_handlers: SignalHandlers::new(),
        clear_child_tid: UnsafeCell::new(0),
        user_gs_base: UnsafeCell::new(0),
    })
}

unsafe fn init_user_trapframe(kstack_top: u64, entry: u64, user_stack_top: u64) -> *mut TrapFrame {
    // Alinha para 16
    let mut sp = kstack_top & !0xFu64;
    sp -= core::mem::size_of::<TrapFrame>() as u64;
    let tf = sp as *mut TrapFrame;

    // zera
    core::ptr::write_bytes(tf as *mut u8, 0, core::mem::size_of::<TrapFrame>());

    (*tf).vector = 0;
    (*tf).error = 0;
    (*tf).rip = entry;
    (*tf).cs = (gdt::user_code_selector().0 as u64) | 3;
    (*tf).rflags = 0x202; // IF=1
    (*tf).rsp = user_stack_top;
    (*tf).ss = (gdt::user_data_selector().0 as u64) | 3;

    tf
}

fn create_user_address_space(prog: &'static [u8]) -> Result<PhysFrame<Size4KiB>, KError> {
    use x86_64::structures::paging::{Mapper, OffsetPageTable};

    // Aloca um frame para o P4
    let mut fa = mm::frame_allocator_lock();
    let p4_frame = fa.allocate().ok_or(KError::NoMemory)?;

    // Ponteiro virtual do P4 via offset
    let p4_virt = mm::phys_to_virt(p4_frame.start_address());
    let p4_ptr = p4_virt.as_mut_ptr::<PageTable>();

    unsafe { (*p4_ptr).zero(); }

    // Copy ALL present P4 entries from kernel (bootloader uses low-half, not high-half)
    let (active_p4_frame, _) = Cr3::read();
    let active_p4_virt = mm::phys_to_virt(active_p4_frame.start_address());
    let active_p4_ptr = active_p4_virt.as_ptr::<PageTable>();

    unsafe {
        let p4_ref = &mut *p4_ptr;
        let active_p4_ref = &*active_p4_ptr;
        // Copy only KERNEL entries (PRESENT but NOT USER_ACCESSIBLE).
        // This ensures user-space mappings are not inherited.
        for i in 0..512 {
            let flags = active_p4_ref[i].flags();
            if flags.contains(PageTableFlags::PRESENT) && !flags.contains(PageTableFlags::USER_ACCESSIBLE) {
                p4_ref[i] = active_p4_ref[i].clone();
            }
        }
    }

    // Mapper para o novo P4
    let phys_off = mm::physical_memory_offset();
    let mut mapper = unsafe { OffsetPageTable::new(&mut *p4_ptr, phys_off) };

    // Mapeia código user
    map_and_copy_user(&mut mapper, &mut *fa, VirtAddr::new(USER_BASE), prog, false)?;

    // Mapeia stack user (32 KiB)
    let stack_pages = 8;
    let stack_base = USER_STACK_TOP - (stack_pages as u64 * 4096);
    for i in 0..stack_pages {
        let va = VirtAddr::new(stack_base + (i as u64) * 4096);
        let page: Page<Size4KiB> = Page::containing_address(va);
        let frame = fa.allocate().ok_or(KError::NoMemory)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
        unsafe { mapper.map_to(page, frame, flags, &mut *fa).map_err(|_| KError::NoMemory)?.flush(); }
    }

    Ok(p4_frame)
}

fn map_and_copy_user(
    mapper: &mut x86_64::structures::paging::OffsetPageTable<'static>,
    fa: &mut crate::mm::BitmapFrameAllocator,
    base: VirtAddr,
    data: &[u8],
    writable: bool,
) -> Result<(), KError> {
    use x86_64::structures::paging::Mapper;

    let pages = (data.len() + 4095) / 4096;
    for i in 0..pages {
        let va = base + (i as u64) * 4096;
        let page: Page<Size4KiB> = Page::containing_address(va);
        let frame = fa.allocate().ok_or(KError::NoMemory)?;
        let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
        if writable {
            flags |= PageTableFlags::WRITABLE;
        }
        unsafe { mapper.map_to(page, frame, flags, fa).map_err(|_| KError::NoMemory)?.flush(); }

        // Copia bytes para o frame via phys_offset
        let phys = frame.start_address();
        let dst = mm::phys_to_virt(phys).as_mut_ptr::<u8>();
        let off = i * 4096;
        let len = core::cmp::min(4096, data.len() - off);
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr().add(off), dst, len);
            if len < 4096 {
                core::ptr::write_bytes(dst.add(len), 0, 4096 - len);
            }
        }
    }
    Ok(())
}

// ======================== Thread support ========================

/// Clone: cria uma nova thread (compartilhando address space)
pub fn clone_thread(
    sf: &syscall::SyscallFrame,
    flags: u64,
    child_stack: u64,
    parent_tidptr: u64,
    child_tidptr: u64,
    tls: u64,
) -> KResult<u64> {
    use crate::syscall::clone_flags::*;

    let parent = current_task();

    if !parent.is_user {
        return Err(KError::NotSupported);
    }

    let thread_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

    // Thread usa o mesmo CR3 (compartilha address space)
    let thread_cr3 = parent.cr3;

    // Nova kernel stack para a thread
    let thread_kstack = KernelStack::new(64 * 1024);

    // Cria TrapFrame para a thread
    let thread_tf = unsafe {
        let mut sp = thread_kstack.top() & !0xFu64;
        sp -= core::mem::size_of::<TrapFrame>() as u64;
        let tf = sp as *mut TrapFrame;

        // Copia o frame do syscall
        (*tf).rax = 0; // Thread retorna 0
        (*tf).rbx = sf.rbx;
        (*tf).rcx = sf.rcx;
        (*tf).rdx = sf.rdx;
        (*tf).rsi = sf.rsi;
        (*tf).rdi = sf.rdi;
        (*tf).rbp = sf.rbp;
        (*tf).r8 = sf.r8;
        (*tf).r9 = sf.r9;
        (*tf).r10 = sf.r10;
        (*tf).r11 = sf.r11;
        (*tf).r12 = sf.r12;
        (*tf).r13 = sf.r13;
        (*tf).r14 = sf.r14;
        (*tf).r15 = sf.r15;

        (*tf).vector = 0;
        (*tf).error = 0;

        // RIP de retorno
        (*tf).rip = sf.rcx;
        (*tf).cs = (gdt::user_code_selector().0 as u64) | 3;
        (*tf).rflags = sf.r11 | 0x200;

        // Stack da thread (fornecido pelo caller)
        (*tf).rsp = if child_stack != 0 { child_stack } else { get_user_rsp() };
        (*tf).ss = (gdt::user_data_selector().0 as u64) | 3;

        tf
    };

    // Thread group ID = tgid do parent (todas as threads do mesmo processo)
    let thread_tgid = parent.tgid;

    // Se CLONE_SETTLS, usa o TLS fornecido; senão herda do parent
    let thread_gs_base = if (flags & CLONE_SETTLS) != 0 && tls != 0 {
        tls
    } else {
        parent.user_gs_base()
    };

    // Cria a nova thread
    let thread = Arc::new(Task {
        id: thread_id,
        parent_id: parent.id,
        tgid: thread_tgid,
        name: format!("{}-thread-{}", parent.name, thread_id),
        state: UnsafeCell::new(TaskState::Ready),
        exit_status: UnsafeCell::new(0),
        is_user: true,
        is_thread: true,
        kstack: thread_kstack,
        cr3: thread_cr3,
        saved_tf: UnsafeCell::new(thread_tf),
        cred: UnsafeCell::new(parent.cred()),
        signals: parent.signals.clone(),
        signal_handlers: parent.signal_handlers.clone(),
        clear_child_tid: UnsafeCell::new(if (flags & CLONE_CHILD_CLEARTID) != 0 { child_tidptr } else { 0 }),
        user_gs_base: UnsafeCell::new(thread_gs_base),
    });

    // Se CLONE_PARENT_SETTID, escreve TID no parent
    if (flags & CLONE_PARENT_SETTID) != 0 && parent_tidptr != 0 {
        unsafe {
            *(parent_tidptr as *mut i32) = thread_id as i32;
        }
    }

    // Se CLONE_CHILD_SETTID, escreve TID no child
    if (flags & CLONE_CHILD_SETTID) != 0 && child_tidptr != 0 {
        unsafe {
            *(child_tidptr as *mut i32) = thread_id as i32;
        }
    }

    // Adiciona à fila de execução
    {
        let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
        let mut sched = sched_lock.lock();
        sched.runq.push_back(thread);
    }

    crate::kprintln!("clone: criada thread {} (tgid={}, pai={})", thread_id, thread_tgid, parent.id);

    Ok(thread_id)
}

/// Define o endereço para limpar quando a thread terminar
pub fn set_clear_child_tid(tidptr: u64) {
    let task = current_task();
    unsafe {
        *task.clear_child_tid.get() = tidptr;
    }
}

/// Retorna o TID (thread ID) da task atual
pub fn current_tid() -> u64 {
    current_task().id
}

/// Retorna o PID do processo pai
pub fn current_ppid() -> u64 {
    current_task().parent_id
}

/// Retorna o TGID (thread group ID) da task atual
pub fn current_tgid() -> u64 {
    current_task().tgid
}

/// Termina todas as threads do grupo de processos
pub fn exit_thread_group(status: u64) -> ! {
    let current = current_task();
    let tgid = current.tgid;

    // Marca todas as threads do grupo como exited
    {
        let sched_lock = SCHED.get().expect("scheduler não inicializado");
        let sched = sched_lock.lock();

        for task in sched.runq.iter() {
            if task.tgid == tgid {
                task.set_state(TaskState::Exited);
                task.set_exit_status(status as i32);
            }
        }
    }

    // Sai da thread atual
    exit_current_and_switch(status);
}

/// Envia sinal para uma thread específica
pub fn send_signal_to_thread(tgid: u64, tid: u64, sig: u32) -> KResult<()> {
    let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
    let sched = sched_lock.lock();

    // Procura a thread
    for task in sched.runq.iter() {
        if task.id == tid && (tgid == u64::MAX || task.tgid == tgid) {
            task.signals.send(sig);
            return Ok(());
        }
    }

    // Verifica também o task atual
    if sched.current.id == tid && (tgid == u64::MAX || sched.current.tgid == tgid) {
        sched.current.signals.send(sig);
        return Ok(());
    }

    Err(KError::NotFound)
}

// ---------------- Init process spawning ----------------

/// Spawn /bin/init como PID 1.
///
/// Esta função deve ser chamada depois que o scheduler foi inicializado
/// e os binários userland foram instalados no filesystem.
pub fn spawn_init() -> KResult<()> {
    use crate::process;

    // Debug direto na serial sem usar lock - para identificar se chegamos aqui
    unsafe {
        use x86_64::instructions::port::Port;
        let mut port: Port<u8> = Port::new(0x3F8);
        for b in b"[DBG:spawn_init_ENTERED]\n" {
            port.write(*b);
        }
    }

    crate::kprintln!("sched: carregando /bin/init...");

    // Carrega o ELF de /bin/init
    let argv = ["/bin/init"];
    let envp = ["PATH=/bin", "HOME=/root"];

    let loaded = match process::load_elf_from_path("/bin/init", &argv, &envp) {
        Ok(l) => l,
        Err(e) => {
            crate::kprintln!("sched: falha ao carregar /bin/init: {:?}", e);
            return Err(e);
        }
    };

    crate::kprintln!(
        "sched: init loaded - entry={:#x}, sp={:#x}",
        loaded.entry,
        loaded.stack_pointer
    );

    // Cria a kernel stack para o init
    let kstack = KernelStack::new(64 * 1024);

    // Cria o TrapFrame para saltar para userspace
    let tf_ptr = unsafe {
        init_user_trapframe(kstack.top(), loaded.entry, loaded.stack_pointer)
    };

    // Cria o task para init (PID 1)
    let init_task = Arc::new(Task {
        id: 1,  // PID 1 é sempre o init
        parent_id: 0,  // Kernel é o pai
        tgid: 1,
        name: "init".into(),
        state: UnsafeCell::new(TaskState::Ready),
        exit_status: UnsafeCell::new(0),
        is_user: true,
        is_thread: false,
        kstack,
        cr3: loaded.cr3,
        saved_tf: UnsafeCell::new(tf_ptr),
        cred: UnsafeCell::new(Cred::root()),  // Init roda como root
        signals: SignalState::new(),
        signal_handlers: SignalHandlers::new(),
        clear_child_tid: UnsafeCell::new(0),
        user_gs_base: UnsafeCell::new(0),
    });

    // Adiciona init à fila de execução
    {
        let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
        let mut sched = sched_lock.lock();
        sched.runq.push_back(init_task);
    }

    crate::kprintln!("sched: /bin/init pronto para execução (PID 1)");

    Ok(())
}

/// Inicializa o scheduler com apenas o idle task e sem user tasks.
///
/// Após chamar esta função, você pode usar spawn_init() para iniciar /bin/init.
pub fn init_scheduler_only() {
    let (current_cr3, _) = Cr3::read();

    // Task "idle" usa o contexto atual
    let idle = Arc::new(Task {
        id: 0,
        parent_id: 0,
        tgid: 0,
        name: "idle".into(),
        state: UnsafeCell::new(TaskState::Running),
        exit_status: UnsafeCell::new(0),
        is_user: false,
        is_thread: false,
        kstack: KernelStack::new(0),
        cr3: current_cr3,
        saved_tf: UnsafeCell::new(ptr::null_mut()),
        cred: UnsafeCell::new(Cred::root()),
        signals: SignalState::new(),
        signal_handlers: SignalHandlers::new(),
        clear_child_tid: UnsafeCell::new(0),
        user_gs_base: UnsafeCell::new(0),
    });

    let sched = Scheduler {
        next_id: 2,  // 0 = idle, 1 = init, próximo = 2
        current: idle.clone(),
        runq: VecDeque::new(),
        quantum: 8,
        remaining: 8,
    };

    SCHED.call_once(|| IrqSafeMutex::new(sched));
    CURRENT_PTR.store(Arc::as_ptr(&idle) as *mut Task, Ordering::Release);

    unsafe { syscall::set_kernel_stack_top(read_rsp()); }

    crate::kprintln!("sched: scheduler inicializado (idle task)");
}
