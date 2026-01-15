//! Sistema de signals POSIX-like.
//!
//! Implementa signals básicos para controle de processos:
//! - SIGKILL/SIGTERM: terminar processo
//! - SIGINT: interrupção (Ctrl+C)
//! - SIGCHLD: filho terminou
//! - SIGPIPE: escrita em pipe sem leitores
//!
//! Signal handlers:
//! - sigaction: registrar handlers customizados
//!
//! NOTA: Constantes e estruturas padrão POSIX mantidas para compatibilidade.

#![allow(dead_code)]
//! - sigreturn: retornar de um signal handler

extern crate alloc;

use core::sync::atomic::{AtomicU64, Ordering};
use core::cell::UnsafeCell;

/// Números de signals (compatível com Linux x86_64).
pub mod sig {
    pub const SIGHUP: u32 = 1;
    pub const SIGINT: u32 = 2;
    pub const SIGQUIT: u32 = 3;
    pub const SIGILL: u32 = 4;
    pub const SIGTRAP: u32 = 5;
    pub const SIGABRT: u32 = 6;
    pub const SIGBUS: u32 = 7;
    pub const SIGFPE: u32 = 8;
    pub const SIGKILL: u32 = 9;
    pub const SIGUSR1: u32 = 10;
    pub const SIGSEGV: u32 = 11;
    pub const SIGUSR2: u32 = 12;
    pub const SIGPIPE: u32 = 13;
    pub const SIGALRM: u32 = 14;
    pub const SIGTERM: u32 = 15;
    pub const SIGSTKFLT: u32 = 16;
    pub const SIGCHLD: u32 = 17;
    pub const SIGCONT: u32 = 18;
    pub const SIGSTOP: u32 = 19;
    pub const SIGTSTP: u32 = 20;
    pub const SIGTTIN: u32 = 21;
    pub const SIGTTOU: u32 = 22;
    pub const SIGURG: u32 = 23;
    pub const SIGXCPU: u32 = 24;
    pub const SIGXFSZ: u32 = 25;
    pub const SIGVTALRM: u32 = 26;
    pub const SIGPROF: u32 = 27;
    pub const SIGWINCH: u32 = 28;
    pub const SIGIO: u32 = 29;
    pub const SIGPWR: u32 = 30;
    pub const SIGSYS: u32 = 31;

    pub const NSIG: u32 = 32;

    // Real-time signals (32-64)
    pub const SIGRTMIN: u32 = 32;
    pub const SIGRTMAX: u32 = 64;
}

/// Flags para sigaction
pub mod sa_flags {
    pub const SA_NOCLDSTOP: u64 = 1;        // Não recebe SIGCHLD quando filho para
    pub const SA_NOCLDWAIT: u64 = 2;        // Não cria zombie
    pub const SA_SIGINFO: u64 = 4;          // Usa sa_sigaction em vez de sa_handler
    pub const SA_ONSTACK: u64 = 0x08000000; // Usa stack alternativo
    pub const SA_RESTART: u64 = 0x10000000; // Reinicia syscalls interrompidas
    pub const SA_NODEFER: u64 = 0x40000000; // Não bloqueia o signal durante handler
    pub const SA_RESETHAND: u64 = 0x80000000; // Reseta para default após entrega
    pub const SA_RESTORER: u64 = 0x04000000; // Usa sa_restorer
}

/// Valores especiais para sa_handler
pub const SIG_DFL: u64 = 0;  // Ação default
pub const SIG_IGN: u64 = 1;  // Ignorar signal
pub const SIG_ERR: u64 = !0; // Erro

/// Estrutura sigaction (compatível com Linux x86_64)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Sigaction {
    /// Endereço do handler (ou SIG_DFL/SIG_IGN)
    pub sa_handler: u64,
    /// Flags de comportamento
    pub sa_flags: u64,
    /// Função para restaurar contexto (sa_restorer)
    pub sa_restorer: u64,
    /// Máscara de signals a bloquear durante execução do handler
    pub sa_mask: u64,
}

impl Sigaction {
    pub const fn default() -> Self {
        Self {
            sa_handler: SIG_DFL,
            sa_flags: 0,
            sa_restorer: 0,
            sa_mask: 0,
        }
    }

    pub fn is_default(&self) -> bool {
        self.sa_handler == SIG_DFL
    }

    pub fn is_ignored(&self) -> bool {
        self.sa_handler == SIG_IGN
    }

    pub fn has_handler(&self) -> bool {
        self.sa_handler > SIG_IGN
    }
}

/// siginfo_t - informações sobre o signal
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Siginfo {
    pub si_signo: i32,    // Número do signal
    pub si_errno: i32,    // Errno associado (se houver)
    pub si_code: i32,     // Código do signal (origem)
    _pad: i32,            // Padding para alinhamento
    pub si_pid: i32,      // PID do sender
    pub si_uid: i32,      // UID do sender
    pub si_status: i32,   // Exit status (para SIGCHLD)
    _pad2: i32,
    pub si_addr: u64,     // Endereço da falta (para SIGSEGV/SIGBUS)
    _reserved: [u64; 12], // Reservado para compatibilidade
}

impl Siginfo {
    pub const fn new(signo: i32) -> Self {
        Self {
            si_signo: signo,
            si_errno: 0,
            si_code: 0,
            _pad: 0,
            si_pid: 0,
            si_uid: 0,
            si_status: 0,
            _pad2: 0,
            si_addr: 0,
            _reserved: [0; 12],
        }
    }
}

/// ucontext_t - contexto salvo para signal handler
/// Layout simplificado compatível com x86_64
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UContext {
    pub uc_flags: u64,
    pub uc_link: u64,         // Ponteiro para próximo contexto
    pub uc_stack: StackT,     // Stack info
    pub uc_mcontext: MContext,// Registradores salvos
    pub uc_sigmask: u64,      // Máscara de signals
}

/// stack_t - informações sobre stack
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StackT {
    pub ss_sp: u64,    // Base do stack
    pub ss_flags: i32, // Flags (SS_ONSTACK, SS_DISABLE)
    _pad: i32,
    pub ss_size: u64,  // Tamanho do stack
}

impl StackT {
    pub const fn empty() -> Self {
        Self {
            ss_sp: 0,
            ss_flags: 0,
            _pad: 0,
            ss_size: 0,
        }
    }
}

/// mcontext_t - registradores salvos
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MContext {
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rdx: u64,
    pub rax: u64,
    pub rcx: u64,
    pub rsp: u64,
    pub rip: u64,
    pub rflags: u64,
    pub cs: u16,
    pub gs: u16,
    pub fs: u16,
    _pad: u16,
    pub err: u64,
    pub trapno: u64,
    pub oldmask: u64,
    pub cr2: u64,
    // FPU state seria aqui, mas não implementamos
    pub fpstate: u64,
    _reserved: [u64; 8],
}

impl MContext {
    pub const fn empty() -> Self {
        Self {
            r8: 0, r9: 0, r10: 0, r11: 0, r12: 0, r13: 0, r14: 0, r15: 0,
            rdi: 0, rsi: 0, rbp: 0, rbx: 0, rdx: 0, rax: 0, rcx: 0, rsp: 0,
            rip: 0, rflags: 0, cs: 0, gs: 0, fs: 0, _pad: 0,
            err: 0, trapno: 0, oldmask: 0, cr2: 0, fpstate: 0,
            _reserved: [0; 8],
        }
    }
}

/// Frame completo salvo na stack do usuário para signal handler
#[repr(C)]
pub struct SignalFrame {
    pub restorer: u64,      // Endereço de retorno (chama sigreturn)
    pub info: Siginfo,      // Informações do signal
    pub uc: UContext,       // Contexto salvo
}

/// Tabela de handlers de signals para um processo
pub struct SignalHandlers {
    handlers: UnsafeCell<[Sigaction; 64]>,
}

impl SignalHandlers {
    pub const fn new() -> Self {
        const DEFAULT: Sigaction = Sigaction::default();
        Self {
            handlers: UnsafeCell::new([DEFAULT; 64]),
        }
    }

    /// Obtém o handler para um signal
    pub fn get(&self, signum: u32) -> Option<Sigaction> {
        if signum == 0 || signum >= 64 {
            return None;
        }
        unsafe {
            Some((*self.handlers.get())[signum as usize])
        }
    }

    /// Define o handler para um signal
    pub fn set(&self, signum: u32, action: &Sigaction) -> Option<Sigaction> {
        if signum == 0 || signum >= 64 {
            return None;
        }
        // SIGKILL e SIGSTOP não podem ter handlers
        if signum == sig::SIGKILL || signum == sig::SIGSTOP {
            return None;
        }
        unsafe {
            let old = (*self.handlers.get())[signum as usize];
            (*self.handlers.get())[signum as usize] = *action;
            Some(old)
        }
    }

    /// Reseta um handler para default
    pub fn reset(&self, signum: u32) {
        if signum == 0 || signum >= 64 {
            return;
        }
        unsafe {
            (*self.handlers.get())[signum as usize] = Sigaction::default();
        }
    }
}

// SAFETY: Acessado apenas pelo processo owner
unsafe impl Send for SignalHandlers {}
unsafe impl Sync for SignalHandlers {}

impl Clone for SignalHandlers {
    fn clone(&self) -> Self {
        let new = Self::new();
        unsafe {
            for i in 0..64 {
                (*new.handlers.get())[i] = (*self.handlers.get())[i];
            }
        }
        new
    }
}

/// Ação padrão para um signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultAction {
    /// Termina o processo.
    Terminate,
    /// Ignora o signal.
    Ignore,
    /// Para o processo (SIGSTOP, SIGTSTP).
    Stop,
    /// Continua o processo (SIGCONT).
    Continue,
    /// Gera core dump e termina.
    CoreDump,
}

/// Retorna a ação padrão para um signal.
pub fn default_action(signum: u32) -> DefaultAction {
    use sig::*;
    match signum {
        SIGHUP | SIGINT | SIGKILL | SIGPIPE | SIGALRM | SIGTERM |
        SIGUSR1 | SIGUSR2 | SIGPROF | SIGVTALRM | SIGSTKFLT |
        SIGIO | SIGPWR => DefaultAction::Terminate,

        SIGQUIT | SIGILL | SIGTRAP | SIGABRT | SIGBUS | SIGFPE |
        SIGSEGV | SIGXCPU | SIGXFSZ | SIGSYS => DefaultAction::CoreDump,

        SIGCHLD | SIGURG | SIGWINCH => DefaultAction::Ignore,

        SIGSTOP | SIGTSTP | SIGTTIN | SIGTTOU => DefaultAction::Stop,

        SIGCONT => DefaultAction::Continue,

        _ => DefaultAction::Terminate,
    }
}

/// Estado de signals de um processo.
///
/// Usa um bitmap de 64 bits para representar signals pendentes.
/// Signals 1-31 são padrão, 32-64 são real-time (não implementado).
#[derive(Debug)]
pub struct SignalState {
    /// Bitmap de signals pendentes.
    pending: AtomicU64,
    /// Bitmap de signals bloqueados (masked).
    blocked: AtomicU64,
}

impl SignalState {
    pub const fn new() -> Self {
        Self {
            pending: AtomicU64::new(0),
            blocked: AtomicU64::new(0),
        }
    }

    /// Verifica se um signal está pendente.
    #[inline]
    pub fn is_pending(&self, signum: u32) -> bool {
        if signum == 0 || signum >= 64 {
            return false;
        }
        let mask = 1u64 << signum;
        (self.pending.load(Ordering::Acquire) & mask) != 0
    }

    /// Envia um signal (marca como pendente).
    #[inline]
    pub fn send(&self, signum: u32) {
        if signum == 0 || signum >= 64 {
            return;
        }
        let mask = 1u64 << signum;
        self.pending.fetch_or(mask, Ordering::AcqRel);
    }

    /// Limpa um signal pendente.
    #[inline]
    pub fn clear(&self, signum: u32) {
        if signum == 0 || signum >= 64 {
            return;
        }
        let mask = 1u64 << signum;
        self.pending.fetch_and(!mask, Ordering::AcqRel);
    }

    /// Retorna o próximo signal pendente não bloqueado, ou None.
    pub fn dequeue(&self) -> Option<u32> {
        let pending = self.pending.load(Ordering::Acquire);
        let blocked = self.blocked.load(Ordering::Acquire);
        let deliverable = pending & !blocked;

        if deliverable == 0 {
            return None;
        }

        // Encontra o primeiro bit setado (menor signal)
        let signum = deliverable.trailing_zeros();
        if signum >= 64 {
            return None;
        }

        // Limpa o signal
        self.clear(signum);
        Some(signum)
    }

    /// Verifica se há algum signal pendente não bloqueado.
    pub fn has_pending(&self) -> bool {
        let pending = self.pending.load(Ordering::Acquire);
        let blocked = self.blocked.load(Ordering::Acquire);
        (pending & !blocked) != 0
    }

    /// Bloqueia um signal.
    pub fn block(&self, signum: u32) {
        if signum == 0 || signum >= 64 {
            return;
        }
        // SIGKILL e SIGSTOP não podem ser bloqueados
        if signum == sig::SIGKILL || signum == sig::SIGSTOP {
            return;
        }
        let mask = 1u64 << signum;
        self.blocked.fetch_or(mask, Ordering::AcqRel);
    }

    /// Desbloqueia um signal.
    pub fn unblock(&self, signum: u32) {
        if signum == 0 || signum >= 64 {
            return;
        }
        let mask = 1u64 << signum;
        self.blocked.fetch_and(!mask, Ordering::AcqRel);
    }

    /// Define a máscara de signals bloqueados.
    pub fn set_blocked_mask(&self, mask: u64) {
        // Remove SIGKILL e SIGSTOP da máscara
        let safe_mask = mask & !((1u64 << sig::SIGKILL) | (1u64 << sig::SIGSTOP));
        self.blocked.store(safe_mask, Ordering::Release);
    }

    /// Retorna a máscara de signals bloqueados.
    pub fn blocked_mask(&self) -> u64 {
        self.blocked.load(Ordering::Acquire)
    }

    /// Retorna a máscara de signals pendentes.
    pub fn pending_mask(&self) -> u64 {
        self.pending.load(Ordering::Acquire)
    }
}

impl Clone for SignalState {
    fn clone(&self) -> Self {
        Self {
            pending: AtomicU64::new(0), // Filho começa sem signals pendentes
            blocked: AtomicU64::new(self.blocked.load(Ordering::Acquire)),
        }
    }
}

impl Default for SignalState {
    fn default() -> Self {
        Self::new()
    }
}

/// Constantes para sigprocmask
pub mod sigprocmask {
    pub const SIG_BLOCK: i32 = 0;   // Adiciona signals à máscara
    pub const SIG_UNBLOCK: i32 = 1; // Remove signals da máscara
    pub const SIG_SETMASK: i32 = 2; // Define a máscara inteira
}

/// Prepara a stack do usuário para executar um signal handler.
///
/// Retorna o novo SP (stack pointer) para o usuário.
/// O usuário deve ter a estrutura:
/// - SignalFrame no topo da stack
/// - handler será chamado com (signum, siginfo*, ucontext*)
///
/// Quando o handler retornar, ele chamará sigreturn via sa_restorer.
pub fn setup_signal_frame(
    user_sp: u64,
    signum: u32,
    action: &Sigaction,
    saved_rip: u64,
    saved_rsp: u64,
    saved_rflags: u64,
    regs: &[u64; 16], // r15..rax na ordem do TrapFrame
    old_mask: u64,
) -> Option<(u64, u64)> {
    use core::mem::size_of;

    // Calcula o espaço necessário na stack do usuário
    let frame_size = size_of::<SignalFrame>();

    // Alinha a 16 bytes (requerido pelo ABI x86_64)
    let new_sp = (user_sp - frame_size as u64) & !0xF;

    // Verifica se a stack é válida (acima de um mínimo razoável)
    if new_sp < 0x1000 {
        return None;
    }

    // Escreve o SignalFrame na stack do usuário
    unsafe {
        let frame_ptr = new_sp as *mut SignalFrame;

        // Endereço de retorno (sa_restorer chama sigreturn)
        (*frame_ptr).restorer = action.sa_restorer;

        // Preenche siginfo
        (*frame_ptr).info = Siginfo::new(signum as i32);

        // Preenche ucontext
        (*frame_ptr).uc.uc_flags = 0;
        (*frame_ptr).uc.uc_link = 0;
        (*frame_ptr).uc.uc_stack = StackT::empty();
        (*frame_ptr).uc.uc_sigmask = old_mask;

        // Salva registradores no mcontext
        let mc = &mut (*frame_ptr).uc.uc_mcontext;
        mc.r15 = regs[0];
        mc.r14 = regs[1];
        mc.r13 = regs[2];
        mc.r12 = regs[3];
        mc.r11 = regs[4];
        mc.r10 = regs[5];
        mc.r9 = regs[6];
        mc.r8 = regs[7];
        mc.rbp = regs[8];
        mc.rdi = regs[9];
        mc.rsi = regs[10];
        mc.rdx = regs[11];
        mc.rcx = regs[12];
        mc.rbx = regs[13];
        mc.rax = regs[14];
        mc.rsp = saved_rsp;
        mc.rip = saved_rip;
        mc.rflags = saved_rflags;
    }

    // Retorna (novo SP, endereço do handler)
    Some((new_sp, action.sa_handler))
}

/// Restaura o contexto após retorno de um signal handler (sigreturn).
///
/// Lê o SignalFrame da stack do usuário e restaura os registradores.
/// Retorna (rip, rsp, rflags, regs) para continuar a execução.
pub fn restore_signal_frame(
    frame_ptr: u64,
) -> Option<(u64, u64, u64, [u64; 16], u64)> {
    // Verifica se o ponteiro é válido
    if frame_ptr < 0x1000 {
        return None;
    }

    unsafe {
        let frame = &*(frame_ptr as *const SignalFrame);
        let mc = &frame.uc.uc_mcontext;

        // Extrai registradores
        let regs = [
            mc.r15, mc.r14, mc.r13, mc.r12, mc.r11,
            mc.r10, mc.r9, mc.r8, mc.rbp, mc.rdi,
            mc.rsi, mc.rdx, mc.rcx, mc.rbx, mc.rax, 0,
        ];

        Some((mc.rip, mc.rsp, mc.rflags, regs, frame.uc.uc_sigmask))
    }
}
