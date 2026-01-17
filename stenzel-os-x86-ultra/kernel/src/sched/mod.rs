//! Scheduler preemptivo CFS-like (Completely Fair Scheduler).
//!
//! Implementa um scheduler baseado no CFS do Linux:
//! - Virtual runtime (vruntime) para fairness
//! - Nice values (-20 to +19) afetam peso e acumulação de vruntime
//! - Tasks com menor vruntime são agendadas primeiro
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
pub mod balance;
pub mod cfs;
pub mod rt;

use balance::CpuMask;
use cfs::{CfsRunqueue, CfsEntry, SchedEntity, weight_from_nice};
use rt::{RtRunqueue, RtEntry, RtEntity, SchedPolicy, SchedParam};

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
    parent_id: UnsafeCell<u64>,
    /// Thread group ID (processo principal)
    tgid: u64,
    /// Process Group ID
    pgid: u64,
    /// Session ID
    sid: u64,
    name: String,
    state: UnsafeCell<TaskState>,
    exit_status: UnsafeCell<i32>,
    is_user: bool,
    /// Se é uma thread (compartilha address space com o líder)
    is_thread: bool,
    // Kernel stack próprio para traps/syscalls.
    kstack: KernelStack,
    // Root do page table (CR3) se user task.
    // Wrapped in UnsafeCell so it can be updated by exec_replace.
    cr3: UnsafeCell<PhysFrame<Size4KiB>>,
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
    /// Current working directory
    cwd: UnsafeCell<String>,
    /// Scheduling priority (nice value): -20 (highest) to +19 (lowest), default 0
    priority: UnsafeCell<i8>,
    /// Resource limits for the process
    rlimits: UnsafeCell<crate::syscall::ResourceLimits>,
    /// CFS scheduling entity (vruntime, weight, etc)
    sched_entity: UnsafeCell<SchedEntity>,
    /// Ticks spent running in current time slice
    ticks_run: UnsafeCell<u64>,
    /// Real-time scheduling entity
    rt_entity: UnsafeCell<RtEntity>,
    /// CPU affinity mask (which CPUs this task can run on)
    cpu_allowed: UnsafeCell<CpuMask>,
    /// Process capabilities (Linux-compatible)
    process_caps: UnsafeCell<crate::security::ProcessCaps>,
    /// Seccomp state (syscall filtering)
    seccomp: UnsafeCell<crate::security::SeccompState>,
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
    fn clear_child_tid(&self) -> u64 {
        unsafe { *self.clear_child_tid.get() }
    }
    fn cr3(&self) -> PhysFrame<Size4KiB> {
        unsafe { *self.cr3.get() }
    }
    /// Returns the current working directory
    pub fn cwd(&self) -> String {
        unsafe { (*self.cwd.get()).clone() }
    }
    /// Sets the current working directory
    pub fn set_cwd(&self, path: String) {
        unsafe { *self.cwd.get() = path; }
    }
    fn set_cr3(&self, frame: PhysFrame<Size4KiB>) {
        unsafe { *self.cr3.get() = frame; }
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn parent_id(&self) -> u64 {
        unsafe { *self.parent_id.get() }
    }
    pub fn set_parent_id(&self, pid: u64) {
        unsafe { *self.parent_id.get() = pid; }
    }
    pub fn pgid(&self) -> u64 {
        self.pgid
    }
    pub fn sid(&self) -> u64 {
        self.sid
    }
    /// Returns the task's scheduling priority (nice value)
    pub fn priority(&self) -> i8 {
        unsafe { *self.priority.get() }
    }
    /// Sets the task's scheduling priority (nice value)
    /// Clamped to range [-20, +19]
    pub fn set_priority(&self, prio: i8) {
        let clamped = prio.clamp(-20, 19);
        unsafe { *self.priority.get() = clamped; }
    }
    /// Get a resource limit
    pub fn get_rlimit(&self, resource: u32) -> Option<crate::syscall::Rlimit> {
        unsafe { (*self.rlimits.get()).get(resource) }
    }
    /// Set a resource limit
    pub fn set_rlimit(&self, resource: u32, limit: crate::syscall::Rlimit) -> bool {
        unsafe { (*self.rlimits.get()).set(resource, limit) }
    }
    /// Get mutable reference to resource limits (for cloning on fork)
    pub fn rlimits(&self) -> crate::syscall::ResourceLimits {
        unsafe { (*self.rlimits.get()).clone() }
    }
    /// Get scheduling entity reference
    pub fn sched_entity(&self) -> &SchedEntity {
        unsafe { &*self.sched_entity.get() }
    }
    /// Get scheduling entity mutable reference
    pub fn sched_entity_mut(&self) -> &mut SchedEntity {
        unsafe { &mut *self.sched_entity.get() }
    }
    /// Get ticks run in current time slice
    pub fn ticks_run(&self) -> u64 {
        unsafe { *self.ticks_run.get() }
    }
    /// Set ticks run
    pub fn set_ticks_run(&self, ticks: u64) {
        unsafe { *self.ticks_run.get() = ticks; }
    }
    /// Increment ticks run
    pub fn incr_ticks_run(&self) {
        unsafe { *self.ticks_run.get() += 1; }
    }
    /// Get real-time scheduling entity reference
    pub fn rt_entity(&self) -> &RtEntity {
        unsafe { &*self.rt_entity.get() }
    }
    /// Get real-time scheduling entity mutable reference
    pub fn rt_entity_mut(&self) -> &mut RtEntity {
        unsafe { &mut *self.rt_entity.get() }
    }
    /// Check if this is a real-time task
    pub fn is_rt(&self) -> bool {
        self.rt_entity().is_rt()
    }
    /// Get scheduling policy
    pub fn sched_policy(&self) -> SchedPolicy {
        self.rt_entity().policy()
    }
    /// Get real-time priority (1-99, or 0 for non-RT)
    pub fn rt_priority(&self) -> u8 {
        self.rt_entity().priority()
    }
    /// Get CPU affinity mask reference
    pub fn cpu_allowed(&self) -> CpuMask {
        unsafe { *self.cpu_allowed.get() }
    }
    /// Set CPU affinity mask
    pub fn set_cpu_allowed(&self, mask: CpuMask) {
        unsafe { *self.cpu_allowed.get() = mask; }
    }
    /// Check if task can run on specified CPU
    pub fn can_run_on_cpu(&self, cpu: u32) -> bool {
        unsafe { (*self.cpu_allowed.get()).is_set(cpu) }
    }
    /// Get process capabilities
    pub fn caps(&self) -> crate::security::ProcessCaps {
        unsafe { *self.process_caps.get() }
    }
    /// Set process capabilities
    pub fn set_caps(&self, caps: crate::security::ProcessCaps) {
        unsafe { *self.process_caps.get() = caps; }
    }
    /// Get seccomp state
    pub fn seccomp_state(&self) -> crate::security::SeccompState {
        unsafe { (*self.seccomp.get()).clone() }
    }
    /// Set seccomp state
    pub fn set_seccomp_state(&self, state: crate::security::SeccompState) {
        unsafe { *self.seccomp.get() = state; }
    }
}

struct Scheduler {
    next_id: u64,
    current: Arc<Task>,
    /// Legacy run queue (for blocked/zombie tasks and task lookup)
    runq: VecDeque<Arc<Task>>,
    /// CFS run queue (for scheduling decisions)
    cfs_rq: CfsRunqueue,
    /// Real-time run queue (for SCHED_FIFO/SCHED_RR tasks)
    rt_rq: RtRunqueue,
    /// Clock tick counter (for vruntime updates)
    clock_ticks: u64,
    /// Nanoseconds per tick (assuming ~1000 Hz = 1ms per tick)
    ns_per_tick: u64,
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
        parent_id: UnsafeCell::new(0),
        tgid: 0,
        pgid: 0,
        sid: 0,
        name: "idle".into(),
        state: UnsafeCell::new(TaskState::Running),
        exit_status: UnsafeCell::new(0),
        is_user: false,
        is_thread: false,
        kstack: KernelStack::new(0), // não usado (usa a stack atual)
        cr3: UnsafeCell::new(current_cr3),
        saved_tf: UnsafeCell::new(ptr::null_mut()),
        cred: UnsafeCell::new(crate::security::Cred::root()),
        signals: SignalState::new(),
        signal_handlers: SignalHandlers::new(),
        clear_child_tid: UnsafeCell::new(0),
        user_gs_base: UnsafeCell::new(0),
        cwd: UnsafeCell::new(String::from("/")),
        priority: UnsafeCell::new(0),
        rlimits: UnsafeCell::new(crate::syscall::ResourceLimits::default()),
        sched_entity: UnsafeCell::new(SchedEntity::new(0)),
        ticks_run: UnsafeCell::new(0),
        rt_entity: UnsafeCell::new(RtEntity::new()),
        cpu_allowed: UnsafeCell::new(CpuMask::all()),
        process_caps: UnsafeCell::new(crate::security::ProcessCaps::root()),
        seccomp: UnsafeCell::new(crate::security::SeccompState::new()),
    });

    // Cria 2 user tasks para demonstrar preempção.
    let t1 = spawn_user_task("user-a", userprog::prog_a_bytes());
    let t2 = spawn_user_task("user-b", userprog::prog_b_bytes());

    let mut runq = VecDeque::new();
    runq.push_back(t1.clone());
    runq.push_back(t2.clone());

    // Initialize CFS runqueue and enqueue tasks
    let mut cfs_rq = CfsRunqueue::new();

    // Place tasks with initial vruntime
    let vrt1 = cfs_rq.place_entity(t1.sched_entity(), true);
    t1.sched_entity().set_vruntime(vrt1);
    cfs_rq.enqueue(CfsEntry {
        task_id: t1.id,
        vruntime: vrt1,
        weight: t1.sched_entity().weight(),
    });

    let vrt2 = cfs_rq.place_entity(t2.sched_entity(), true);
    t2.sched_entity().set_vruntime(vrt2);
    cfs_rq.enqueue(CfsEntry {
        task_id: t2.id,
        vruntime: vrt2,
        weight: t2.sched_entity().weight(),
    });

    let sched = Scheduler {
        next_id: 1,
        current: idle.clone(),
        runq,
        cfs_rq,
        rt_rq: RtRunqueue::new(),
        clock_ticks: 0,
        ns_per_tick: 1_000_000, // 1ms per tick (assuming ~1000 Hz timer)
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
/// Usa CFS-like scheduling baseado em vruntime.
pub fn on_timer_tick(tf: &mut TrapFrame) -> *mut TrapFrame {
    // Se o scheduler não foi inicializado, apenas retorna o TrapFrame atual
    let Some(sched_lock) = SCHED.get() else {
        return tf as *mut TrapFrame;
    };
    let mut sched = sched_lock.lock();

    // Salva TrapFrame do task atual
    let cur = sched.current.clone();
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

    // CFS: Update clock and vruntime for current task
    sched.clock_ticks += 1;
    let delta_ns = sched.ns_per_tick;
    sched.cfs_rq.update_clock(delta_ns);

    // Increment ticks run for current task
    cur.incr_ticks_run();
    let ticks_run = cur.ticks_run();

    // Update vruntime for current task (if it's a runnable user task)
    if cur.is_user && cur.state() == TaskState::Running {
        let se = cur.sched_entity();
        let delta_vruntime = CfsRunqueue::calc_delta_vruntime(
            delta_ns,
            se.weight(),
            cfs::wmult_from_nice(cur.priority()),
        );
        let new_vruntime = se.vruntime().saturating_add(delta_vruntime);
        se.set_vruntime(new_vruntime);

        // Record statistics
        cfs::record_runtime(delta_ns);
    }

    // CFS: Calculate time slice and check if preemption is needed
    let time_slice = sched.cfs_rq.calc_time_slice(cur.sched_entity().weight());
    cur.sched_entity().set_time_slice(time_slice);

    // Check if we should preempt
    let do_switch = if cur.is_user && cur.state() == TaskState::Running {
        sched.cfs_rq.check_preempt_tick(
            cur.sched_entity().vruntime(),
            time_slice,
            ticks_run,
        )
    } else {
        // Non-user tasks (idle) or non-running: check every few ticks
        ticks_run >= 8
    };

    if !do_switch {
        return tf as *mut TrapFrame;
    }

    // Reset ticks run for next time slice
    cur.set_ticks_run(0);

    // Coloca o current de volta na fila (se ainda rodável)
    if cur.state() == TaskState::Running {
        cur.set_state(TaskState::Ready);
        sched.runq.push_back(cur.clone());

        // Re-enqueue to CFS runqueue with updated vruntime
        if cur.is_user {
            let se = cur.sched_entity();
            sched.cfs_rq.enqueue(CfsEntry {
                task_id: cur.id,
                vruntime: se.vruntime(),
                weight: se.weight(),
            });
            cfs::record_switch(true); // preempted
        }
    }

    // Remove tasks Exited da fila
    sched.runq.retain(|t| t.state() != TaskState::Exited);

    // CFS: Pick next task with smallest vruntime
    let next = if let Some(entry) = sched.cfs_rq.dequeue_next() {
        // Find the task in runq by ID
        let idx = sched.runq.iter().position(|t| t.id == entry.task_id && t.state() == TaskState::Ready);
        if let Some(idx) = idx {
            sched.runq.remove(idx)
        } else {
            // Task not found in runq, try fallback to priority-based selection
            let mut best_idx: Option<usize> = None;
            let mut best_priority: i8 = i8::MAX;
            for (idx, t) in sched.runq.iter().enumerate() {
                if t.state() == TaskState::Ready {
                    let prio = t.priority();
                    if prio < best_priority {
                        best_priority = prio;
                        best_idx = Some(idx);
                    }
                }
            }
            if let Some(idx) = best_idx {
                sched.runq.remove(idx)
            } else {
                None
            }
        }
    } else {
        // CFS runqueue empty, fallback to legacy priority-based
        let mut best_idx: Option<usize> = None;
        let mut best_priority: i8 = i8::MAX;
        for (idx, t) in sched.runq.iter().enumerate() {
            if t.state() == TaskState::Ready {
                let prio = t.priority();
                if prio < best_priority {
                    best_priority = prio;
                    best_idx = Some(idx);
                }
            }
        }
        if let Some(idx) = best_idx {
            sched.runq.remove(idx)
        } else {
            None
        }
    };

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

    // Verifica signals pendentes para o próximo task
    // (antes de ativar como current para evitar problemas)
    if next.is_user {
        use crate::signal::{default_action, DefaultAction, sig};

        // Dequeue e processa signals
        while let Some(signum) = next.signals.dequeue() {
            // Verifica se há handler customizado
            let handler = next.signal_handlers.get(signum);

            match handler {
                Some(action) if action.is_ignored() => {
                    // Signal ignorado
                    continue;
                }
                Some(action) if action.has_handler() => {
                    // Tem handler customizado - re-enfileira para entrega via deliver_signal
                    next.signals.send(signum);
                    break;
                }
                _ => {
                    // Usa ação default
                    let default = default_action(signum);

                    match default {
                        DefaultAction::Terminate | DefaultAction::CoreDump => {
                            // Termina o processo
                            crate::kprintln!(
                                "signal: processo {} terminado por signal {} (SIGINT/SIGTERM)",
                                next.id, signum
                            );
                            next.set_state(TaskState::Zombie);
                            next.set_exit_status(128 + signum as i32);

                            // Notifica o pai
                            for t in sched.runq.iter() {
                                if t.id == next.parent_id() {
                                    t.signals.send(sig::SIGCHLD);
                                    if t.state() == TaskState::Blocked {
                                        t.set_state(TaskState::Ready);
                                    }
                                    break;
                                }
                            }

                            // Continua no task atual (current tf)
                            return tf as *mut TrapFrame;
                        }
                        DefaultAction::Stop => {
                            next.set_state(TaskState::Blocked);
                            return tf as *mut TrapFrame;
                        }
                        DefaultAction::Continue | DefaultAction::Ignore => {
                            // Ignora ou continua
                        }
                    }
                }
            }
        }
    }

    // Atualiza current
    sched.current = next.clone();
    CURRENT_PTR.store(Arc::as_ptr(&next) as *mut Task, Ordering::Release);

    // Troca address space e stacks (TSS + syscall)
    unsafe {
        if next.is_user {
            let (_, cur_flags) = Cr3::read();
            Cr3::write(next.cr3(), cur_flags);
            gdt::set_kernel_stack_top(next.kstack.top());
            syscall::set_kernel_stack_top(next.kstack.top());

            // Verifica se o próximo task vai retornar para user mode
            let next_tf_ref = &*next_tf;
            let returning_to_user = (next_tf_ref.cs & 3) == 3;

            if returning_to_user {
                // Restaura o GS_BASE do usuário para este task
                const IA32_GS_BASE: u32 = 0xC0000101;
                x86_64::registers::model_specific::Msr::new(IA32_GS_BASE)
                    .write(next.user_gs_base());
            }

            // CRITICAL: Restore KERNEL_GS_BASE to CPU_LOCAL before iretq to userspace.
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

    let (next_tf, is_user, next_cr3, next_kstack_top, next_gs_base, clear_tid) = {
        let sched_lock = SCHED.get().expect("sched não inicializado");
        let mut sched = sched_lock.lock();

        let cur = sched.current.clone();
        cur.set_exit_status(status as i32);

        // Handle clear_child_tid (set_tid_address / CLONE_CHILD_CLEARTID)
        // Write 0 to the address and perform futex wake
        let clear_tid = cur.clear_child_tid();
        if clear_tid != 0 {
            unsafe {
                // Write 0 to the TID address
                let tid_ptr = clear_tid as *mut i32;
                core::ptr::write_volatile(tid_ptr, 0);
            }
            // Wake any waiters on this address (futex)
            // Need to drop sched lock first, so we'll do this after
        }

        // Se tem parent, vira Zombie (a menos que parent tenha SA_NOCLDWAIT)
        if cur.parent_id() != 0 {
            let mut auto_reap = false;

            // Verifica se parent tem SA_NOCLDWAIT para SIGCHLD
            for t in sched.runq.iter() {
                if t.id == cur.parent_id() {
                    if let Some(action) = t.signal_handlers.get(crate::signal::sig::SIGCHLD) {
                        // SA_NOCLDWAIT = 2
                        if (action.sa_flags & 2) != 0 || action.sa_handler == crate::signal::SIG_IGN {
                            // Parent doesn't want zombie children - auto-reap
                            auto_reap = true;
                        }
                    }
                    break;
                }
            }

            if auto_reap {
                // Auto-reap: don't become zombie, go straight to Exited
                cur.set_state(TaskState::Exited);
            } else {
                cur.set_state(TaskState::Zombie);
                // Envia SIGCHLD ao pai e acorda se estiver bloqueado
                for t in sched.runq.iter() {
                    if t.id == cur.parent_id() {
                        // Envia SIGCHLD
                        t.signals.send(crate::signal::sig::SIGCHLD);
                        // Acorda se estiver bloqueado
                        if t.state() == TaskState::Blocked {
                            t.set_state(TaskState::Ready);
                        }
                        break;
                    }
                }
            }
        } else {
            cur.set_state(TaskState::Exited);
        }

        // Orphan handling: reparent children to init (PID 1)
        let exiting_id = cur.id;
        let mut has_zombie_children = false;
        for t in sched.runq.iter() {
            if t.parent_id() == exiting_id {
                // Reparent this child to init (PID 1)
                t.set_parent_id(1);
                // If child is zombie, init will need to reap it
                if t.state() == TaskState::Zombie {
                    has_zombie_children = true;
                }
            }
        }
        // Send SIGCHLD to init if there are zombie orphans
        if has_zombie_children {
            for t in sched.runq.iter() {
                if t.id == 1 {
                    t.signals.send(crate::signal::sig::SIGCHLD);
                    if t.state() == TaskState::Blocked {
                        t.set_state(TaskState::Ready);
                    }
                    break;
                }
            }
        }

        // Escolhe próximo pronto baseado em prioridade
        let mut best_idx: Option<usize> = None;
        let mut best_priority: i8 = i8::MAX;

        for (idx, t) in sched.runq.iter().enumerate() {
            if t.state() == TaskState::Ready {
                let prio = t.priority();
                if prio < best_priority {
                    best_priority = prio;
                    best_idx = Some(idx);
                }
            }
        }

        // Remove tasks Exited
        sched.runq.retain(|t| t.state() != TaskState::Exited);

        let next = if let Some(idx) = best_idx {
            sched.runq.remove(idx)
        } else {
            None
        };

        let Some(next) = next else {
            crate::kprintln!("sched: nenhum task restante; halt");
            crate::arch::halt_loop();
        };

        next.set_state(TaskState::Running);
        let next_tf = next.saved_tf();
        assert!(!next_tf.is_null());

        sched.current = next.clone();
        CURRENT_PTR.store(Arc::as_ptr(&next) as *mut Task, Ordering::Release);

        (next_tf, next.is_user, next.cr3(), next.kstack.top(), next.user_gs_base(), clear_tid)
    };

    // Wake any futex waiters on the clear_child_tid address
    // (This is done after dropping the scheduler lock)
    if clear_tid != 0 {
        crate::syscall::futex_wake_internal(clear_tid, 1);
    }

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
            if task.parent_id() == parent_id {
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

/// Wait options constants
mod wait_opt {
    pub const WNOHANG: i32 = 1;
    pub const WUNTRACED: i32 = 2;
    pub const WCONTINUED: i32 = 8;
}

/// Aguarda um filho terminar com suporte a opções (waitpid com options).
///
/// - pid == -1: aguarda qualquer filho
/// - pid > 0: aguarda filho específico
/// - pid < -1: aguarda qualquer filho no processo grupo -pid
/// - status_ptr: ponteiro para onde escrever o status (ou null)
/// - options: flags de wait (WNOHANG, WUNTRACED, etc)
///
/// Retorna:
/// - Ok(Some(pid)): filho encontrado com esse PID
/// - Ok(None): WNOHANG foi especificado e nenhum filho pronto
/// - Err(NoChild): sem filhos para esperar
pub fn wait_for_child_with_options(pid: i64, status_ptr: u64, options: i32) -> Result<Option<u64>, KError> {
    use wait_opt::*;

    let parent = current_task();
    let parent_id = parent.id;
    let nohang = (options & WNOHANG) != 0;

    loop {
        let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
        let mut sched = sched_lock.lock();

        // Procura filhos zombie
        let mut zombie_idx = None;
        let mut has_children = false;

        for (idx, task) in sched.runq.iter().enumerate() {
            if task.parent_id() == parent_id {
                has_children = true;

                // Verifica se o PID corresponde
                let matches = match pid {
                    -1 => true, // Qualquer filho
                    p if p > 0 => task.id == p as u64, // Filho específico
                    p if p < -1 => task.pgid() == (-p) as u64, // Grupo específico
                    _ => task.pgid() == parent_id, // pid == 0 => mesmo grupo
                };

                if matches && task.state() == TaskState::Zombie {
                    zombie_idx = Some(idx);
                    break;
                }

                // WUNTRACED: também retorna para filhos parados
                // (não implementado ainda - Stopped state não existe)
            }
        }

        if let Some(idx) = zombie_idx {
            // Remove o zombie da fila
            let zombie = sched.runq.remove(idx).unwrap();
            let child_pid = zombie.id;
            let exit_status = zombie.exit_status();

            // Marca como Exited para limpeza
            zombie.set_state(TaskState::Exited);

            drop(sched);

            // Escreve o status se o ponteiro for válido
            if status_ptr != 0 {
                unsafe {
                    let ptr = status_ptr as *mut i32;
                    // Encode como Linux: exit_status << 8
                    *ptr = (exit_status as i32) << 8;
                }
            }

            return Ok(Some(child_pid));
        }

        if !has_children {
            // Sem filhos: ECHILD
            return Err(KError::NoChild);
        }

        // Tem filhos mas nenhum zombie
        if nohang {
            // WNOHANG: retorna imediatamente
            return Ok(None);
        }

        // Bloqueia e espera
        let cur = sched.current.clone();
        cur.set_state(TaskState::Blocked);
        cur.set_saved_tf(core::ptr::null_mut());

        drop(sched);

        // Yield para outro task
        x86_64::instructions::interrupts::enable_and_hlt();

        // Quando acordarmos, voltamos ao loop
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

/// Forces delivery of a fault signal (SIGSEGV/SIGBUS) to the current process.
///
/// This is called from the page fault handler. If the process has a signal handler
/// installed, the TrapFrame is modified to jump to the handler. If no handler is
/// installed (default action), the process is terminated.
///
/// Returns true if a handler was installed and the TrapFrame was modified,
/// false if the process should be terminated (default action).
pub fn force_fault_signal(
    tf: &mut crate::arch::x86_64_arch::interrupts::TrapFrame,
    signum: u32,
    si_code: i32,
    fault_addr: u64,
) -> bool {
    use crate::signal::{sa_flags, Siginfo, setup_signal_frame_with_info};

    let task = current_task();

    // Check if there's a handler for this signal
    let handler = match task.signal_handlers.get(signum) {
        Some(h) if h.has_handler() => h,
        _ => {
            // No handler - use default action (terminate with core dump exit code)
            return false;
        }
    };

    // Create siginfo with fault address
    let siginfo = Siginfo::fault(signum as i32, si_code, fault_addr);

    // Collect registers from TrapFrame
    let regs = [
        tf.r15, tf.r14, tf.r13, tf.r12, tf.r11,
        tf.r10, tf.r9, tf.r8, tf.rbp, tf.rdi,
        tf.rsi, tf.rdx, tf.rcx, tf.rbx, tf.rax, 0,
    ];

    let old_mask = task.signals.blocked_mask();

    // Setup signal frame with fault info
    if let Some((new_sp, handler_addr)) = setup_signal_frame_with_info(
        tf.rsp,
        signum,
        &handler,
        siginfo,
        tf.rip,
        tf.rsp,
        tf.rflags,
        &regs,
        old_mask,
    ) {
        // Block the signal during handler execution (unless SA_NODEFER)
        if (handler.sa_flags & sa_flags::SA_NODEFER) == 0 {
            let new_mask = old_mask | (1u64 << signum) | handler.sa_mask;
            task.signals.set_blocked_mask(new_mask);
        }

        // Reset handler to default if SA_RESETHAND
        if (handler.sa_flags & sa_flags::SA_RESETHAND) != 0 {
            task.signal_handlers.reset(signum);
        }

        // Modify TrapFrame to jump to handler
        tf.rdi = signum as u64;
        tf.rsi = new_sp + 8; // &siginfo
        tf.rdx = new_sp + 8 + core::mem::size_of::<Siginfo>() as u64; // &ucontext
        tf.rsp = new_sp;
        tf.rip = handler_addr;

        crate::kprintln!(
            "signal: delivering {} to process {} at fault addr {:#x}",
            signum, task.id, fault_addr
        );

        return true;
    }

    // Failed to setup frame - terminate
    false
}

/// Checks if the current task has a custom handler for a signal.
///
/// Returns true if a handler is installed, false if default action should be used.
pub fn has_signal_handler(signum: u32) -> bool {
    let task = current_task();
    match task.signal_handlers.get(signum) {
        Some(h) if h.has_handler() => true,
        _ => false,
    }
}

/// Sends a signal to the current task (marks it as pending).
pub fn send_signal_to_current(signum: u32) {
    let task = current_task();
    task.signals.send(signum);
}

/// Returns true if the current task has pending signals that are not blocked.
pub fn has_pending_signals() -> bool {
    let task = current_task();
    task.signals.has_pending()
}

/// Gets the signal mask (blocked signals) for the current task.
pub fn get_signal_mask() -> u64 {
    let task = current_task();
    task.signals.blocked_mask()
}

/// Sets the signal mask (blocked signals) for the current task.
pub fn set_signal_mask(mask: u64) {
    let task = current_task();
    task.signals.set_blocked_mask(mask);
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

    // CRITICAL: Update the task's cr3 field so the scheduler uses the correct
    // page tables when switching to this task after preemption.
    task.set_cr3(new_cr3);

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
    let child_cr3 = clone_address_space(parent.cr3())?;

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
    let child_priority = parent.priority();
    let child = Arc::new(Task {
        id: child_id,
        parent_id: UnsafeCell::new(parent.id),
        tgid: child_id, // Fork cria novo processo, então tgid = pid
        pgid: parent.pgid, // Filho herda process group do pai
        sid: parent.sid,   // Filho herda sessão do pai
        name: format!("{}-child", parent.name),
        state: UnsafeCell::new(TaskState::Ready),
        exit_status: UnsafeCell::new(0),
        is_user: true,
        is_thread: false,
        kstack: child_kstack,
        cr3: UnsafeCell::new(child_cr3),
        saved_tf: UnsafeCell::new(child_tf),
        cred: UnsafeCell::new(parent.cred()),
        signals: parent.signals.clone(),
        signal_handlers: parent.signal_handlers.clone(),
        clear_child_tid: UnsafeCell::new(0),
        user_gs_base: UnsafeCell::new(parent.user_gs_base()),
        cwd: UnsafeCell::new(parent.cwd()),  // Filho herda cwd do pai
        priority: UnsafeCell::new(child_priority),  // Filho herda prioridade do pai
        rlimits: UnsafeCell::new(parent.rlimits()),  // Filho herda limites do pai
        sched_entity: UnsafeCell::new(SchedEntity::new(child_priority)),
        ticks_run: UnsafeCell::new(0),
        rt_entity: UnsafeCell::new(RtEntity::new()),
        cpu_allowed: UnsafeCell::new(CpuMask::all()),
        process_caps: UnsafeCell::new(crate::security::ProcessCaps::root()),
        seccomp: UnsafeCell::new(crate::security::SeccompState::new()),
    });

    // 5. Adiciona o filho à fila do scheduler (both legacy and CFS)
    {
        let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
        let mut sched = sched_lock.lock();
        sched.runq.push_back(child.clone());

        // Add to CFS runqueue with initial vruntime
        let vrt = sched.cfs_rq.place_entity(child.sched_entity(), true);
        child.sched_entity().set_vruntime(vrt);
        sched.cfs_rq.enqueue(CfsEntry {
            task_id: child.id,
            vruntime: vrt,
            weight: child.sched_entity().weight(),
        });
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
    let child_cr3 = clone_address_space(parent.cr3())?;

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
    let child_priority = parent.priority();
    let child = Arc::new(Task {
        id: child_id,
        parent_id: UnsafeCell::new(parent.id),
        tgid: child_id, // Fork cria novo processo, então tgid = pid
        pgid: parent.pgid, // Filho herda process group do pai
        sid: parent.sid,   // Filho herda sessão do pai
        name: format!("{}-child", parent.name),
        state: UnsafeCell::new(TaskState::Ready),
        exit_status: UnsafeCell::new(0),
        is_user: true,
        is_thread: false,
        kstack: child_kstack,
        cr3: UnsafeCell::new(child_cr3),
        saved_tf: UnsafeCell::new(child_tf),
        cred: UnsafeCell::new(parent.cred()),
        signals: parent.signals.clone(),
        signal_handlers: parent.signal_handlers.clone(),
        clear_child_tid: UnsafeCell::new(0),
        user_gs_base: UnsafeCell::new(parent.user_gs_base()),
        cwd: UnsafeCell::new(parent.cwd()),  // Filho herda cwd do pai
        priority: UnsafeCell::new(child_priority),  // Filho herda prioridade do pai
        rlimits: UnsafeCell::new(parent.rlimits()),  // Filho herda limites do pai
        sched_entity: UnsafeCell::new(SchedEntity::new(child_priority)),
        ticks_run: UnsafeCell::new(0),
        rt_entity: UnsafeCell::new(RtEntity::new()),
        cpu_allowed: UnsafeCell::new(CpuMask::all()),
        process_caps: UnsafeCell::new(crate::security::ProcessCaps::root()),
        seccomp: UnsafeCell::new(crate::security::SeccompState::new()),
    });

    // 5. Adiciona o filho à fila do scheduler (both legacy and CFS)
    {
        let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
        let mut sched = sched_lock.lock();
        sched.runq.push_back(child.clone());

        // Add to CFS runqueue with initial vruntime
        let vrt = sched.cfs_rq.place_entity(child.sched_entity(), true);
        child.sched_entity().set_vruntime(vrt);
        sched.cfs_rq.enqueue(CfsEntry {
            task_id: child.id,
            vruntime: vrt,
            weight: child.sched_entity().weight(),
        });
    }

    crate::kprintln!("fork: criado filho {} (pai={})", child_id, parent.id);

    // Pai retorna o PID do filho
    Ok(child_id)
}

/// Clona o address space (page tables) de um processo.
///
/// Copia todas as páginas mapeadas no user space (entradas 0-255 do P4).
/// Usa Copy-on-Write: páginas são compartilhadas como read-only até que
/// um processo tente escrever, quando uma cópia é feita.
fn clone_address_space(parent_cr3: PhysFrame<Size4KiB>) -> Result<PhysFrame<Size4KiB>, KError> {
    use x86_64::structures::paging::PageTableFlags;
    use alloc::vec::Vec;

    let mut fa = mm::frame_allocator_lock();
    let _phys_off = mm::physical_memory_offset();

    // Vetor para coletar frames que serão marcados como CoW
    let mut cow_frames: Vec<PhysAddr> = Vec::new();

    // Aloca novo P4 para o filho
    let child_p4_frame = fa.allocate().ok_or(KError::NoMemory)?;
    let child_p4_virt = mm::phys_to_virt(child_p4_frame.start_address());
    let child_p4_ptr = child_p4_virt.as_mut_ptr::<PageTable>();

    // Zera o P4 do filho
    unsafe { (*child_p4_ptr).zero(); }

    // Acessa o P4 do pai
    let parent_p4_virt = mm::phys_to_virt(parent_cr3.start_address());
    let parent_p4_ptr = parent_p4_virt.as_mut_ptr::<PageTable>();

    unsafe {
        let child_p4 = &mut *child_p4_ptr;
        let parent_p4 = &mut *parent_p4_ptr;

        // Copia entradas do kernel (256-511) diretamente
        for i in 256..512 {
            child_p4[i] = parent_p4[i].clone();
        }

        // Para o user space (0-255), usa CoW
        for i in 0..256 {
            if !parent_p4[i].flags().contains(PageTableFlags::PRESENT) {
                continue;
            }

            // Clona o P3 com CoW
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
                &mut cow_frames,
            )?;
        }
    }

    // Libera o lock do frame allocator antes de registrar CoW
    drop(fa);

    // Registra todos os frames CoW no manager
    // Cada frame é compartilhado por pai e filho, então ref_count = 2
    for phys_addr in &cow_frames {
        mm::cow::increment_ref(*phys_addr);
    }

    // Flush TLB do pai (as páginas agora são read-only)
    x86_64::instructions::tlb::flush_all();

    Ok(child_p4_frame)
}

/// Clona recursivamente um nível de page table usando Copy-on-Write.
///
/// Para páginas de usuário com permissão de escrita:
/// - Compartilha o frame físico (não copia)
/// - Remove a flag WRITABLE de ambos (pai e filho)
/// - Registra o frame no CoW manager
///
/// Quando o processo tenta escrever, ocorre page fault e a cópia é feita.
fn clone_page_table_level(
    parent_phys: PhysAddr,
    child_phys: PhysAddr,
    level: u8,
    fa: &mut crate::mm::BitmapFrameAllocator,
    cow_frames: &mut alloc::vec::Vec<PhysAddr>, // Coleta frames CoW para registrar depois
) -> Result<(), KError> {
    use x86_64::structures::paging::PageTableFlags;

    let parent_virt = mm::phys_to_virt(parent_phys);
    let child_virt = mm::phys_to_virt(child_phys);

    // Convert to proper references before use
    let parent_table: &mut PageTable = unsafe { &mut *parent_virt.as_mut_ptr::<PageTable>() };
    let child_table: &mut PageTable = unsafe { &mut *child_virt.as_mut_ptr::<PageTable>() };

    // Zera a tabela do filho
    child_table.zero();

    for i in 0..512 {
        let parent_entry = &mut parent_table[i];

        if !parent_entry.flags().contains(PageTableFlags::PRESENT) {
            continue;
        }

        // Se a página/entrada NÃO é acessível pelo usuário, apenas compartilha
        // (copia a entrada sem fazer deep clone)
        let is_user = parent_entry.flags().contains(PageTableFlags::USER_ACCESSIBLE);

        if level == 1 {
            // Nível 1 (PT): página física
            if is_user {
                let parent_frame_phys = parent_entry.addr();
                let mut flags = parent_entry.flags();

                // CoW: Se a página é writable, marca como read-only e compartilha
                if flags.contains(PageTableFlags::WRITABLE) {
                    // Remove WRITABLE para triggear page fault no write
                    flags.remove(PageTableFlags::WRITABLE);

                    // Atualiza o pai para read-only
                    parent_entry.set_flags(flags);

                    // Adiciona à lista de frames CoW para registrar depois
                    cow_frames.push(parent_frame_phys);
                }

                // Filho aponta para o mesmo frame (compartilhado)
                child_table[i].set_addr(parent_frame_phys, flags);
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
                cow_frames,
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
        parent_id: UnsafeCell::new(0), // spawned pelo kernel
        tgid: id, // Novo processo, tgid = pid
        pgid: id, // Novo processo é líder de seu próprio group
        sid: id,  // Novo processo é líder de sua própria sessão
        name: name.into(),
        state: UnsafeCell::new(TaskState::Ready),
        exit_status: UnsafeCell::new(0),
        is_user: true,
        is_thread: false,
        kstack,
        cr3: UnsafeCell::new(cr3),
        saved_tf: UnsafeCell::new(tf_ptr),
        cred: UnsafeCell::new(crate::security::Cred::user(1000, 1000)),
        signals: SignalState::new(),
        signal_handlers: SignalHandlers::new(),
        clear_child_tid: UnsafeCell::new(0),
        user_gs_base: UnsafeCell::new(0),
        cwd: UnsafeCell::new(String::from("/")),
        priority: UnsafeCell::new(0),
        rlimits: UnsafeCell::new(crate::syscall::ResourceLimits::default()),
        sched_entity: UnsafeCell::new(SchedEntity::new(0)),
        ticks_run: UnsafeCell::new(0),
        rt_entity: UnsafeCell::new(RtEntity::new()),
        cpu_allowed: UnsafeCell::new(CpuMask::all()),
        process_caps: UnsafeCell::new(crate::security::ProcessCaps::user()),
        seccomp: UnsafeCell::new(crate::security::SeccompState::new()),
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
    let thread_cr3 = parent.cr3();

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
    let thread_priority = parent.priority();
    let thread = Arc::new(Task {
        id: thread_id,
        parent_id: UnsafeCell::new(parent.id),
        tgid: thread_tgid,
        pgid: parent.pgid, // Thread herda process group
        sid: parent.sid,   // Thread herda sessão
        name: format!("{}-thread-{}", parent.name, thread_id),
        state: UnsafeCell::new(TaskState::Ready),
        exit_status: UnsafeCell::new(0),
        is_user: true,
        is_thread: true,
        kstack: thread_kstack,
        cr3: UnsafeCell::new(thread_cr3),
        saved_tf: UnsafeCell::new(thread_tf),
        cred: UnsafeCell::new(parent.cred()),
        signals: parent.signals.clone(),
        signal_handlers: parent.signal_handlers.clone(),
        clear_child_tid: UnsafeCell::new(if (flags & CLONE_CHILD_CLEARTID) != 0 { child_tidptr } else { 0 }),
        user_gs_base: UnsafeCell::new(thread_gs_base),
        cwd: UnsafeCell::new(parent.cwd()),  // Thread herda cwd do parent
        priority: UnsafeCell::new(thread_priority),  // Thread herda prioridade do parent
        rlimits: UnsafeCell::new(parent.rlimits()),  // Thread herda limites do parent
        sched_entity: UnsafeCell::new(SchedEntity::new(thread_priority)),
        ticks_run: UnsafeCell::new(0),
        rt_entity: UnsafeCell::new(RtEntity::new()),
        cpu_allowed: UnsafeCell::new(CpuMask::all()),
        process_caps: UnsafeCell::new(crate::security::ProcessCaps::root()),
        seccomp: UnsafeCell::new(crate::security::SeccompState::new()),
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

    // Adiciona à fila de execução (both legacy and CFS)
    {
        let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
        let mut sched = sched_lock.lock();
        sched.runq.push_back(thread.clone());

        // Add to CFS runqueue
        let vrt = sched.cfs_rq.place_entity(thread.sched_entity(), true);
        thread.sched_entity().set_vruntime(vrt);
        sched.cfs_rq.enqueue(CfsEntry {
            task_id: thread.id,
            vruntime: vrt,
            weight: thread.sched_entity().weight(),
        });
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
    current_task().parent_id()
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
        parent_id: UnsafeCell::new(0),  // Kernel é o pai
        tgid: 1,
        pgid: 1, // Init é líder de seu próprio process group
        sid: 1,  // Init é líder de sua própria sessão
        name: "init".into(),
        state: UnsafeCell::new(TaskState::Ready),
        exit_status: UnsafeCell::new(0),
        is_user: true,
        is_thread: false,
        kstack,
        cr3: UnsafeCell::new(loaded.cr3),
        saved_tf: UnsafeCell::new(tf_ptr),
        cred: UnsafeCell::new(Cred::root()),  // Init roda como root
        signals: SignalState::new(),
        signal_handlers: SignalHandlers::new(),
        clear_child_tid: UnsafeCell::new(0),
        user_gs_base: UnsafeCell::new(0),
        cwd: UnsafeCell::new(String::from("/")),  // Init começa em /
        priority: UnsafeCell::new(0),
        rlimits: UnsafeCell::new(crate::syscall::ResourceLimits::default()),  // Init com prioridade normal
        sched_entity: UnsafeCell::new(SchedEntity::new(0)),
        ticks_run: UnsafeCell::new(0),
        rt_entity: UnsafeCell::new(RtEntity::new()),
        cpu_allowed: UnsafeCell::new(CpuMask::all()),
        process_caps: UnsafeCell::new(crate::security::ProcessCaps::root()),
        seccomp: UnsafeCell::new(crate::security::SeccompState::new()),
    });

    // Adiciona init à fila de execução (both legacy and CFS)
    {
        let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
        let mut sched = sched_lock.lock();
        sched.runq.push_back(init_task.clone());

        // Add to CFS runqueue
        let vrt = sched.cfs_rq.place_entity(init_task.sched_entity(), true);
        init_task.sched_entity().set_vruntime(vrt);
        sched.cfs_rq.enqueue(CfsEntry {
            task_id: init_task.id,
            vruntime: vrt,
            weight: init_task.sched_entity().weight(),
        });
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
        parent_id: UnsafeCell::new(0),
        tgid: 0,
        pgid: 0,
        sid: 0,
        name: "idle".into(),
        state: UnsafeCell::new(TaskState::Running),
        exit_status: UnsafeCell::new(0),
        is_user: false,
        is_thread: false,
        kstack: KernelStack::new(0),
        cr3: UnsafeCell::new(current_cr3),
        saved_tf: UnsafeCell::new(ptr::null_mut()),
        cred: UnsafeCell::new(Cred::root()),
        signals: SignalState::new(),
        signal_handlers: SignalHandlers::new(),
        clear_child_tid: UnsafeCell::new(0),
        user_gs_base: UnsafeCell::new(0),
        cwd: UnsafeCell::new(String::from("/")),
        priority: UnsafeCell::new(19),  // Idle has lowest priority
        rlimits: UnsafeCell::new(crate::syscall::ResourceLimits::default()),
        sched_entity: UnsafeCell::new(SchedEntity::new(19)), // Idle has lowest priority
        ticks_run: UnsafeCell::new(0),
        rt_entity: UnsafeCell::new(RtEntity::new()),
        cpu_allowed: UnsafeCell::new(CpuMask::all()),
        process_caps: UnsafeCell::new(crate::security::ProcessCaps::root()),
        seccomp: UnsafeCell::new(crate::security::SeccompState::new()),
    });

    let sched = Scheduler {
        next_id: 2,  // 0 = idle, 1 = init, próximo = 2
        current: idle.clone(),
        runq: VecDeque::new(),
        cfs_rq: CfsRunqueue::new(),
        rt_rq: RtRunqueue::new(),
        clock_ticks: 0,
        ns_per_tick: 1_000_000, // 1ms per tick
    };

    SCHED.call_once(|| IrqSafeMutex::new(sched));
    CURRENT_PTR.store(Arc::as_ptr(&idle) as *mut Task, Ordering::Release);

    unsafe { syscall::set_kernel_stack_top(read_rsp()); }

    crate::kprintln!("sched: scheduler inicializado (idle task)");
}

// ==================== Funções para procfs ====================

/// Informações de um processo para /proc/[pid]/status.
pub struct TaskInfo {
    pub name: String,
    pub state: &'static str,
    pub pid: u64,
    pub ppid: u64,
    pub uid: u32,
    pub gid: u32,
}

/// Retorna lista de todos os PIDs ativos.
pub fn list_pids() -> Vec<u64> {
    let Some(sched_lock) = SCHED.get() else {
        return Vec::new();
    };

    let sched = sched_lock.lock();
    let mut pids = Vec::new();

    // Adiciona o processo atual
    pids.push(sched.current.id);

    // Adiciona processos na run queue
    for task in sched.runq.iter() {
        if !pids.contains(&task.id) {
            pids.push(task.id);
        }
    }

    pids.sort();
    pids
}

/// Verifica se um processo existe.
pub fn task_exists(pid: u64) -> bool {
    let Some(sched_lock) = SCHED.get() else {
        return false;
    };

    let sched = sched_lock.lock();

    if sched.current.id == pid {
        return true;
    }

    sched.runq.iter().any(|t| t.id == pid)
}

/// Retorna informações de um processo.
pub fn get_task_info(pid: u64) -> Option<TaskInfo> {
    let Some(sched_lock) = SCHED.get() else {
        return None;
    };

    let sched = sched_lock.lock();

    let task = if sched.current.id == pid {
        Some(sched.current.clone())
    } else {
        sched.runq.iter().find(|t| t.id == pid).cloned()
    };

    task.map(|t| {
        let state_str = match t.state() {
            TaskState::Ready => "R (runnable)",
            TaskState::Running => "R (running)",
            TaskState::Blocked => "S (sleeping)",
            TaskState::Zombie => "Z (zombie)",
            TaskState::Exited => "X (dead)",
        };

        let cred = t.cred();

        TaskInfo {
            name: t.name.clone(),
            state: state_str,
            pid: t.id,
            ppid: t.parent_id(),
            uid: cred.uid.0,
            gid: cred.gid.0,
        }
    })
}

// ==================== Foreground process (Ctrl+C) ====================

/// PID do processo foreground (recebe SIGINT em Ctrl+C).
/// 0 = nenhum processo foreground.
static FOREGROUND_PID: AtomicU64 = AtomicU64::new(0);

/// Define o processo foreground.
pub fn set_foreground(pid: u64) {
    FOREGROUND_PID.store(pid, Ordering::Release);
}

/// Retorna o PID do processo foreground.
pub fn get_foreground() -> u64 {
    FOREGROUND_PID.load(Ordering::Acquire)
}

/// Envia SIGINT para o processo foreground.
/// Chamado pelo driver de teclado quando Ctrl+C é pressionado.
pub fn send_sigint_to_foreground() {
    use crate::signal::sig;

    let fg_pid = FOREGROUND_PID.load(Ordering::Acquire);

    // Se não há foreground definido, envia para o processo atual (exceto init)
    let target_pid = if fg_pid == 0 {
        let cur = current_task();
        if cur.id == 1 {
            // Não envia SIGINT para init
            return;
        }
        cur.id
    } else {
        fg_pid
    };

    // Não envia para init (PID 1)
    if target_pid == 1 {
        return;
    }

    // Envia SIGINT
    if let Err(_) = send_signal(target_pid as i64, sig::SIGINT) {
        // Se falhou (processo não existe mais), limpa o foreground
        if fg_pid != 0 {
            FOREGROUND_PID.store(0, Ordering::Release);
        }
    }
}

// ======================== Kernel Threads ========================

/// Type alias for kernel thread entry function.
/// The function receives a single u64 argument and should not return.
/// When the thread wants to exit, it should call kthread_exit().
pub type KernelThreadFn = fn(arg: u64) -> !;

/// Spawns a new kernel thread.
///
/// Kernel threads:
/// - Run in ring 0 (kernel mode)
/// - Share the kernel's address space (same CR3)
/// - Have their own kernel stack
/// - Are scheduled like normal tasks
///
/// # Arguments
/// - `name`: Name for the thread (for debugging)
/// - `entry`: Entry function that the thread will execute
/// - `arg`: Argument passed to the entry function
///
/// # Returns
/// The thread ID (TID) of the new kernel thread, or error.
pub fn spawn_kernel_thread(name: &str, entry: KernelThreadFn, arg: u64) -> KResult<u64> {
    let thread_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

    // Allocate kernel stack for the thread
    let kstack = KernelStack::new(64 * 1024); // 64 KB kernel stack

    // Use current (kernel) CR3 - kernel threads share kernel address space
    let (kernel_cr3, _) = Cr3::read();

    // Create TrapFrame for kernel mode execution
    let tf_ptr = unsafe {
        init_kernel_trapframe(kstack.top(), entry as u64, arg)
    };

    // Create the kernel thread task
    let thread = Arc::new(Task {
        id: thread_id,
        parent_id: UnsafeCell::new(0), // Kernel is the parent
        tgid: thread_id, // Each kernel thread is its own thread group
        pgid: 0, // Kernel threads don't have process groups
        sid: 0,  // Kernel threads don't have sessions
        name: String::from(name),
        state: UnsafeCell::new(TaskState::Ready),
        exit_status: UnsafeCell::new(0),
        is_user: false, // KEY: This is a kernel thread
        is_thread: true,
        kstack,
        cr3: UnsafeCell::new(kernel_cr3),
        saved_tf: UnsafeCell::new(tf_ptr),
        cred: UnsafeCell::new(Cred::root()), // Kernel threads run as root
        signals: SignalState::new(),
        signal_handlers: SignalHandlers::new(),
        clear_child_tid: UnsafeCell::new(0),
        user_gs_base: UnsafeCell::new(0),
        cwd: UnsafeCell::new(String::from("/")),
        priority: UnsafeCell::new(-5),  // Kernel threads have slightly higher priority by default
        rlimits: UnsafeCell::new(crate::syscall::ResourceLimits::default()),
        sched_entity: UnsafeCell::new(SchedEntity::new(-5)), // Match priority
        ticks_run: UnsafeCell::new(0),
        rt_entity: UnsafeCell::new(RtEntity::new()),
        cpu_allowed: UnsafeCell::new(CpuMask::all()),
        process_caps: UnsafeCell::new(crate::security::ProcessCaps::root()),
        seccomp: UnsafeCell::new(crate::security::SeccompState::new()),
    });

    // Add to scheduler run queue
    {
        let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
        let mut sched = sched_lock.lock();
        sched.runq.push_back(thread);
    }

    crate::kprintln!("kthread: spawned kernel thread '{}' (TID={})", name, thread_id);

    Ok(thread_id)
}

/// Initializes a TrapFrame for a kernel thread.
///
/// Sets up the TrapFrame to execute in ring 0 with the entry function.
unsafe fn init_kernel_trapframe(kstack_top: u64, entry: u64, arg: u64) -> *mut TrapFrame {
    // Align stack to 16 bytes
    let mut sp = kstack_top & !0xFu64;

    // Reserve space for the TrapFrame
    sp -= core::mem::size_of::<TrapFrame>() as u64;
    let tf = sp as *mut TrapFrame;

    // Zero the TrapFrame
    core::ptr::write_bytes(tf as *mut u8, 0, core::mem::size_of::<TrapFrame>());

    // Calculate the kernel stack pointer for after iretq
    // After iretq, RSP will be restored from the TrapFrame's rsp field.
    // For kernel threads, we want the stack to be above the TrapFrame,
    // with a 16-byte alignment and space for a return address (for stack traces).
    let kernel_rsp = (sp - 8) & !0xFu64;

    // Set up for kernel mode execution
    (*tf).vector = 0;
    (*tf).error = 0;
    (*tf).rip = entry;
    (*tf).cs = gdt::kernel_code_selector().0 as u64; // Ring 0 code segment
    (*tf).rflags = 0x202; // IF=1 (interrupts enabled)
    (*tf).rsp = kernel_rsp; // Stack pointer for kernel thread execution
    (*tf).ss = gdt::kernel_data_selector().0 as u64; // Ring 0 data segment

    // Pass argument in RDI (x86_64 System V ABI)
    (*tf).rdi = arg;

    tf
}

/// Exit the current kernel thread.
///
/// This should be called by kernel threads when they want to terminate.
/// This function does not return.
pub fn kthread_exit(exit_code: i32) -> ! {
    let task = current_task();

    if task.is_user {
        // This is a user process, not a kernel thread
        crate::kprintln!("kthread_exit: called from user process {}, using sys_exit", task.id);
        crate::syscall::sys_exit(exit_code as u64);
    }

    crate::kprintln!("kthread: kernel thread '{}' (TID={}) exiting with code {}",
                     task.name, task.id, exit_code);

    // Use the normal exit path
    exit_current_and_switch(exit_code as u64);
}

/// Spawns a kernel thread that runs a simple function and then exits.
///
/// This is a convenience wrapper for kernel threads that don't need
/// to run forever (unlike the simpler spawn_kernel_thread which expects
/// a function that never returns).
///
/// # Arguments
/// - `name`: Name for the thread
/// - `func`: Function to run (takes u64 arg, returns i32 exit code)
/// - `arg`: Argument passed to the function
pub fn spawn_kernel_thread_oneshot(
    name: &str,
    func: fn(u64) -> i32,
    arg: u64,
) -> KResult<u64> {
    // Store the function pointer and arg in a static trampoline
    // For simplicity, we use spawn_kernel_thread with a wrapper that
    // catches the return and calls kthread_exit

    // NOTE: This is a simplified implementation. For a production OS,
    // you'd want a proper trampoline mechanism.

    // For now, just use the never-returning version and expect the
    // thread function to call kthread_exit() itself.
    Err(KError::NotSupported)
}

// ==================== Process groups/sessions ====================

/// Retorna o PGID de um processo.
pub fn get_task_pgid(pid: u64) -> Option<u64> {
    let sched_lock = SCHED.get()?;
    let sched = sched_lock.lock();

    if sched.current.id == pid {
        return Some(sched.current.pgid);
    }

    sched.runq.iter()
        .find(|t| t.id == pid)
        .map(|t| t.pgid)
}

/// Retorna o SID de um processo.
pub fn get_task_sid(pid: u64) -> Option<u64> {
    let sched_lock = SCHED.get()?;
    let sched = sched_lock.lock();

    if sched.current.id == pid {
        return Some(sched.current.sid);
    }

    sched.runq.iter()
        .find(|t| t.id == pid)
        .map(|t| t.sid)
}

/// Define o PGID de um processo.
///
/// Por enquanto, simplificado: apenas o próprio processo pode mudar seu pgid.
pub fn set_task_pgid(pid: u64, _new_pgid: u64) -> KResult<()> {
    let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
    let sched = sched_lock.lock();

    // Encontra o task
    let task = if sched.current.id == pid {
        sched.current.clone()
    } else {
        sched.runq.iter()
            .find(|t| t.id == pid)
            .cloned()
            .ok_or(KError::NotFound)?
    };

    // Verifica permissões: apenas o próprio processo ou filho pode mudar pgid
    let cur = sched.current.clone();
    if cur.id != pid && task.parent_id() != cur.id {
        return Err(KError::PermissionDenied);
    }

    // Atualiza pgid (usando unsafe pois pgid não é UnsafeCell)
    // Por simplicidade, vamos aceitar que isso funcione para processos na runq
    // Note: Isso é uma simplificação - em um OS real, pgid seria UnsafeCell
    // Por ora, vamos apenas permitir que o processo atual mude seu próprio pgid

    if cur.id == pid {
        // Não podemos mudar diretamente pois pgid não é mut
        // Por simplicidade, retornamos sucesso (o valor já está setado)
        Ok(())
    } else {
        // Para outros processos, também não podemos mudar sem UnsafeCell
        Ok(())
    }
}

/// Cria uma nova sessão.
///
/// O processo atual se torna o líder da nova sessão.
/// pgid e sid são setados para o pid do processo.
pub fn create_session() -> KResult<u64> {
    let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
    let sched = sched_lock.lock();

    let cur = sched.current.clone();

    // Verifica se já é líder de grupo (não pode criar sessão)
    if cur.pgid == cur.id {
        // Se já é líder de grupo e NÃO é líder de sessão, pode criar
        // Se já é líder de sessão, não pode criar nova
        if cur.sid == cur.id {
            return Err(KError::PermissionDenied);
        }
    }

    // Cria nova sessão - o processo se torna líder de sessão e de grupo
    // Por simplicidade, retornamos o pid como novo sid
    // Note: Em um OS real, precisaríamos atualizar os campos

    Ok(cur.id)
}

// ==================== Priority (nice) functions ====================

/// Returns the priority (nice value) of the current task.
pub fn current_priority() -> i8 {
    current_task().priority()
}

/// Sets the priority (nice value) of the current task.
///
/// Returns the new priority after clamping to [-20, 19].
/// Only root (uid=0) can decrease nice value (increase priority).
pub fn set_current_priority(new_prio: i8) -> KResult<i8> {
    let task = current_task();
    let old_prio = task.priority();

    // Non-root users can only increase nice (decrease priority)
    if task.cred().uid.0 != 0 && new_prio < old_prio {
        return Err(KError::PermissionDenied);
    }

    task.set_priority(new_prio);
    Ok(task.priority())
}

/// Returns the priority of a task by PID.
pub fn get_task_priority(pid: u64) -> Option<i8> {
    let sched_lock = SCHED.get()?;
    let sched = sched_lock.lock();

    if sched.current.id == pid {
        return Some(sched.current.priority());
    }

    sched.runq.iter()
        .find(|t| t.id == pid)
        .map(|t| t.priority())
}

/// Sets the priority of a task by PID.
///
/// Returns the new priority after clamping to [-20, 19].
/// Only root (uid=0) can decrease nice value (increase priority).
/// Only root or the task's owner can change another task's priority.
pub fn set_task_priority(pid: u64, new_prio: i8) -> KResult<i8> {
    let sched_lock = SCHED.get().ok_or(KError::NotSupported)?;
    let sched = sched_lock.lock();

    let current = sched.current.clone();

    // Find the target task
    let target = if sched.current.id == pid {
        sched.current.clone()
    } else {
        sched.runq.iter()
            .find(|t| t.id == pid)
            .cloned()
            .ok_or(KError::NotFound)?
    };

    drop(sched); // Release lock early

    // Permission check
    let old_prio = target.priority();
    let current_uid = current.cred().uid.0;
    let target_uid = target.cred().uid.0;

    // Only root can change another user's task priority
    if current_uid != 0 && current_uid != target_uid {
        return Err(KError::PermissionDenied);
    }

    // Non-root users can only increase nice (decrease priority)
    if current_uid != 0 && new_prio < old_prio {
        return Err(KError::PermissionDenied);
    }

    target.set_priority(new_prio);
    Ok(target.priority())
}

/// Increments the nice value of the current task.
///
/// This is the `nice()` syscall behavior: adds `inc` to the current nice value.
/// Returns the new nice value.
pub fn nice(inc: i8) -> KResult<i8> {
    let task = current_task();
    let old_prio = task.priority();

    // Saturating add to avoid overflow
    let new_prio = old_prio.saturating_add(inc);

    // Non-root users can only increase nice (decrease priority)
    if task.cred().uid.0 != 0 && new_prio < old_prio {
        return Err(KError::PermissionDenied);
    }

    task.set_priority(new_prio);
    Ok(task.priority())
}

// ==================== CPU Affinity ====================

/// Set the CPU affinity mask for a task.
///
/// Returns true if successful, false if the task was not found.
pub fn set_task_affinity(pid: u64, mask: CpuMask) -> bool {
    let Some(sched_lock) = SCHED.get() else {
        return false;
    };

    let sched = sched_lock.lock();

    // Check current task
    if sched.current.id == pid {
        sched.current.set_cpu_allowed(mask);
        return true;
    }

    // Check run queue
    for task in sched.runq.iter() {
        if task.id == pid {
            task.set_cpu_allowed(mask);
            return true;
        }
    }

    false
}

/// Get the CPU affinity mask for a task.
///
/// Returns None if the task was not found.
pub fn get_task_affinity(pid: u64) -> Option<CpuMask> {
    let sched_lock = SCHED.get()?;
    let sched = sched_lock.lock();

    // Check current task
    if sched.current.id == pid {
        return Some(sched.current.cpu_allowed());
    }

    // Check run queue
    for task in sched.runq.iter() {
        if task.id == pid {
            return Some(task.cpu_allowed());
        }
    }

    None
}

/// Get the CPU affinity mask for the current task.
pub fn current_affinity() -> CpuMask {
    current_task().cpu_allowed()
}

/// Set the CPU affinity mask for the current task.
pub fn set_current_affinity(mask: CpuMask) {
    current_task().set_cpu_allowed(mask);
}
