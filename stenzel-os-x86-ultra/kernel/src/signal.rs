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

/// si_code values for SIGSEGV
pub mod segv_code {
    pub const SEGV_MAPERR: i32 = 1; // Address not mapped
    pub const SEGV_ACCERR: i32 = 2; // Invalid permissions
}

/// si_code values for SIGBUS
pub mod bus_code {
    pub const BUS_ADRALN: i32 = 1; // Invalid address alignment
    pub const BUS_ADRERR: i32 = 2; // Non-existent physical address
    pub const BUS_OBJERR: i32 = 3; // Object-specific hardware error
}

/// si_code values for general signals
pub mod si_code {
    pub const SI_USER: i32 = 0;    // Sent by kill, sigsend, raise
    pub const SI_KERNEL: i32 = 128; // Sent by kernel
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

    /// Creates a Siginfo for a memory fault (SIGSEGV/SIGBUS) with the fault address.
    pub fn fault(signo: i32, code: i32, addr: u64) -> Self {
        Self {
            si_signo: signo,
            si_errno: 0,
            si_code: code,
            _pad: 0,
            si_pid: 0,
            si_uid: 0,
            si_status: 0,
            _pad2: 0,
            si_addr: addr,
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

/// Prepara a stack do usuário para executar um signal handler with custom siginfo.
///
/// This variant allows passing a pre-filled Siginfo (useful for fault signals like SIGSEGV).
pub fn setup_signal_frame_with_info(
    user_sp: u64,
    _signum: u32,
    action: &Sigaction,
    siginfo: Siginfo,
    saved_rip: u64,
    saved_rsp: u64,
    saved_rflags: u64,
    regs: &[u64; 16],
    old_mask: u64,
) -> Option<(u64, u64)> {
    use core::mem::size_of;

    let frame_size = size_of::<SignalFrame>();
    let new_sp = (user_sp - frame_size as u64) & !0xF;

    if new_sp < 0x1000 {
        return None;
    }

    unsafe {
        let frame_ptr = new_sp as *mut SignalFrame;

        (*frame_ptr).restorer = action.sa_restorer;
        (*frame_ptr).info = siginfo;  // Use the provided siginfo

        (*frame_ptr).uc.uc_flags = 0;
        (*frame_ptr).uc.uc_link = 0;
        (*frame_ptr).uc.uc_stack = StackT::empty();
        (*frame_ptr).uc.uc_sigmask = old_mask;

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

// ============================================================================
// signalfd - File descriptor for signals
// ============================================================================

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

/// signalfd flags
pub mod sfd_flags {
    /// Non-blocking mode
    pub const SFD_NONBLOCK: u32 = 0x00800;
    /// Close-on-exec
    pub const SFD_CLOEXEC: u32 = 0x80000;
}

/// Structure returned when reading from signalfd
/// This is signalfd_siginfo from Linux
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SignalfdSiginfo {
    /// Signal number
    pub ssi_signo: u32,
    /// Error number (unused for now)
    pub ssi_errno: i32,
    /// Signal code
    pub ssi_code: i32,
    /// PID of sender
    pub ssi_pid: u32,
    /// UID of sender
    pub ssi_uid: u32,
    /// File descriptor (for SIGIO)
    pub ssi_fd: i32,
    /// Kernel timer ID (for POSIX timers)
    pub ssi_tid: u32,
    /// Band event (for SIGIO)
    pub ssi_band: u32,
    /// Timer overrun count
    pub ssi_overrun: u32,
    /// Trap number
    pub ssi_trapno: u32,
    /// Exit status or signal (for SIGCHLD)
    pub ssi_status: i32,
    /// Integer sent by sigqueue
    pub ssi_int: i32,
    /// Pointer sent by sigqueue
    pub ssi_ptr: u64,
    /// User CPU time consumed (for SIGCHLD)
    pub ssi_utime: u64,
    /// System CPU time consumed (for SIGCHLD)
    pub ssi_stime: u64,
    /// Address that generated signal (for SIGSEGV, SIGBUS, etc.)
    pub ssi_addr: u64,
    /// Least significant bit of address (for SIGSEGV, SIGBUS)
    pub ssi_addr_lsb: u16,
    /// Padding
    _pad2: u16,
    /// System call number (for SIGSYS)
    pub ssi_syscall: i32,
    /// System call address (for SIGSYS)
    pub ssi_call_addr: u64,
    /// System call architecture (for SIGSYS)
    pub ssi_arch: u32,
    /// Padding to 128 bytes
    _pad: [u8; 28],
}

impl SignalfdSiginfo {
    /// Size of the structure (128 bytes)
    pub const SIZE: usize = 128;

    /// Create from Siginfo
    pub fn from_siginfo(info: &Siginfo) -> Self {
        Self {
            ssi_signo: info.si_signo as u32,
            ssi_errno: info.si_errno,
            ssi_code: info.si_code,
            ssi_pid: info.si_pid as u32,
            ssi_uid: info.si_uid as u32,
            ssi_fd: -1,
            ssi_tid: 0,
            ssi_band: 0,
            ssi_overrun: 0,
            ssi_trapno: 0,
            ssi_status: info.si_status,
            ssi_int: 0,
            ssi_ptr: 0,
            ssi_utime: 0,
            ssi_stime: 0,
            ssi_addr: info.si_addr,
            ssi_addr_lsb: 0,
            _pad2: 0,
            ssi_syscall: 0,
            ssi_call_addr: 0,
            ssi_arch: 0,
            _pad: [0; 28],
        }
    }

    /// Convert to bytes for reading
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        unsafe {
            core::ptr::copy_nonoverlapping(
                self as *const Self as *const u8,
                bytes.as_mut_ptr(),
                Self::SIZE,
            );
        }
        bytes
    }
}

/// A signalfd instance
pub struct SignalFd {
    /// Signal mask (which signals to accept)
    mask: AtomicU64,
    /// Flags (SFD_NONBLOCK, SFD_CLOEXEC)
    flags: u32,
    /// Queued signals
    queue: Mutex<VecDeque<SignalfdSiginfo>>,
    /// Maximum queue size
    max_queue: usize,
}

impl SignalFd {
    /// Create a new signalfd
    ///
    /// # Arguments
    /// * `mask` - Signal mask (which signals to accept)
    /// * `flags` - Flags (SFD_NONBLOCK, SFD_CLOEXEC)
    pub fn new(mask: u64, flags: u32) -> Self {
        Self {
            mask: AtomicU64::new(mask),
            flags,
            queue: Mutex::new(VecDeque::with_capacity(32)),
            max_queue: 256,
        }
    }

    /// Update the signal mask
    pub fn set_mask(&self, mask: u64) {
        self.mask.store(mask, Ordering::SeqCst);
    }

    /// Get the current signal mask
    pub fn get_mask(&self) -> u64 {
        self.mask.load(Ordering::SeqCst)
    }

    /// Check if a signal is in the mask
    pub fn accepts_signal(&self, signum: u32) -> bool {
        if signum == 0 || signum >= 64 {
            return false;
        }
        let mask = self.mask.load(Ordering::SeqCst);
        (mask & (1u64 << signum)) != 0
    }

    /// Queue a signal for reading
    ///
    /// Returns true if the signal was queued, false if queue is full
    pub fn queue_signal(&self, info: &Siginfo) -> bool {
        let signum = info.si_signo as u32;
        if !self.accepts_signal(signum) {
            return false;
        }

        let mut queue = self.queue.lock();
        if queue.len() >= self.max_queue {
            return false;
        }

        queue.push_back(SignalfdSiginfo::from_siginfo(info));
        true
    }

    /// Read signals from the fd
    ///
    /// Returns the number of signals read, or error
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, SignalFdError> {
        if buf.len() < SignalfdSiginfo::SIZE {
            return Err(SignalFdError::InvalidSize);
        }

        let mut queue = self.queue.lock();

        if queue.is_empty() {
            if (self.flags & sfd_flags::SFD_NONBLOCK) != 0 {
                return Err(SignalFdError::WouldBlock);
            }
            // Would block - in a real implementation we'd wait here
            return Err(SignalFdError::WouldBlock);
        }

        let mut bytes_read = 0;
        let max_signals = buf.len() / SignalfdSiginfo::SIZE;

        for _ in 0..max_signals {
            if let Some(info) = queue.pop_front() {
                let bytes = info.to_bytes();
                buf[bytes_read..bytes_read + SignalfdSiginfo::SIZE].copy_from_slice(&bytes);
                bytes_read += SignalfdSiginfo::SIZE;
            } else {
                break;
            }
        }

        if bytes_read == 0 {
            Err(SignalFdError::WouldBlock)
        } else {
            Ok(bytes_read)
        }
    }

    /// Check if there are signals ready to read
    pub fn is_readable(&self) -> bool {
        !self.queue.lock().is_empty()
    }

    /// Get the number of queued signals
    pub fn queued_count(&self) -> usize {
        self.queue.lock().len()
    }

    /// Check if non-blocking mode is enabled
    pub fn is_nonblock(&self) -> bool {
        (self.flags & sfd_flags::SFD_NONBLOCK) != 0
    }

    /// Check if close-on-exec is enabled
    pub fn is_cloexec(&self) -> bool {
        (self.flags & sfd_flags::SFD_CLOEXEC) != 0
    }
}

/// SignalFd errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalFdError {
    /// Would block (non-blocking mode, no signals available)
    WouldBlock,
    /// Buffer too small
    InvalidSize,
    /// Invalid signal mask
    InvalidMask,
}

/// Global registry of signalfd instances (for delivering signals)
static SIGNALFD_REGISTRY: spin::Once<Mutex<Vec<Arc<SignalFd>>>> = spin::Once::new();

fn signalfd_registry() -> &'static Mutex<Vec<Arc<SignalFd>>> {
    SIGNALFD_REGISTRY.call_once(|| Mutex::new(Vec::new()))
}

/// Register a signalfd for signal delivery
pub fn register_signalfd(sfd: Arc<SignalFd>) {
    signalfd_registry().lock().push(sfd);
}

/// Unregister a signalfd
pub fn unregister_signalfd(sfd: &Arc<SignalFd>) {
    let mut registry = signalfd_registry().lock();
    registry.retain(|s| !Arc::ptr_eq(s, sfd));
}

/// Deliver a signal to all matching signalfds
///
/// Returns true if at least one signalfd accepted the signal
pub fn deliver_to_signalfds(info: &Siginfo) -> bool {
    let registry = signalfd_registry().lock();
    let mut delivered = false;

    for sfd in registry.iter() {
        if sfd.queue_signal(info) {
            delivered = true;
        }
    }

    delivered
}

/// Create a new signalfd
///
/// This is the implementation of the signalfd() syscall
pub fn sys_signalfd(fd: i32, mask: u64, flags: u32) -> Result<Arc<SignalFd>, SignalFdError> {
    // Validate flags
    let valid_flags = sfd_flags::SFD_NONBLOCK | sfd_flags::SFD_CLOEXEC;
    if (flags & !valid_flags) != 0 {
        return Err(SignalFdError::InvalidMask);
    }

    if fd == -1 {
        // Create new signalfd
        let sfd = Arc::new(SignalFd::new(mask, flags));
        register_signalfd(Arc::clone(&sfd));
        Ok(sfd)
    } else {
        // Update existing signalfd - would need to lookup by fd
        // For now, just create a new one
        let sfd = Arc::new(SignalFd::new(mask, flags));
        register_signalfd(Arc::clone(&sfd));
        Ok(sfd)
    }
}

/// Initialize signalfd subsystem
pub fn init_signalfd() {
    SIGNALFD_REGISTRY.call_once(|| Mutex::new(Vec::new()));
    crate::kprintln!("signalfd: subsystem initialized");
}
