//! Syscalls via `SYSCALL/SYSRET` (x86_64).
//!
//! - Entrada: instrução `syscall` no ring3.
//! - Saída: `sysretq`.
//! - Stack: trocado manualmente via GS base (swapgs) + CpuLocal.kernel_stack_top.
//!
//! Referências:
//! - IA32_STAR/IA32_LSTAR/IA32_FMASK e detalhes de CS/SS no SYSRET:
//!   https://wiki.osdev.org/SYSCALL

use core::arch::global_asm;

use x86_64::registers::model_specific::Msr;

use crate::arch::x86_64_arch::gdt;
use super::percpu::{self, PerCpu};

/// Legacy type alias for backward compatibility
pub type CpuLocal = PerCpu;

/// Get pointer to current CPU's per-CPU data
#[inline]
pub fn cpu_local_ptr() -> *mut CpuLocal {
    percpu::cpu_local_ptr()
}

/// Updates the kernel stack used by syscall for the current task.
pub unsafe fn set_kernel_stack_top(top: u64) {
    percpu::set_kernel_stack_top(top);
}

#[repr(C)]
pub struct SyscallFrame {
    // layout corresponde ao push no asm (ver abaixo)
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64, // user rflags (para sysret)
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64, // user RIP (para sysret)
    pub rbx: u64,
    pub rax: u64, // syscall number
    pub _pad: u64,
}

#[no_mangle]
extern "C" fn syscall_dispatch(frame: &mut SyscallFrame) -> u64 {
    use crate::syscall::{nr, errno};

    // Syscall ABI estilo Linux:
    // rax = nro, rdi/rsi/rdx/r10/r8/r9 = args.
    let result: i64 = match frame.rax {
        nr::READ => crate::syscall::sys_read(frame.rdi as i32, frame.rsi, frame.rdx as usize),
        nr::WRITE => crate::syscall::sys_write(frame.rdi as i32, frame.rsi, frame.rdx as usize),
        nr::OPEN => crate::syscall::sys_open(frame.rdi, frame.rsi as u32, frame.rdx as u32),
        nr::CLOSE => crate::syscall::sys_close(frame.rdi as i32),
        nr::STAT => crate::syscall::sys_stat(frame.rdi, frame.rsi),
        nr::FSTAT => crate::syscall::sys_fstat(frame.rdi as i32, frame.rsi),
        nr::LSTAT => crate::syscall::sys_lstat(frame.rdi, frame.rsi),
        nr::LSEEK => crate::syscall::sys_lseek(frame.rdi as i32, frame.rsi as i64, frame.rdx as i32),
        nr::MMAP => crate::syscall::sys_mmap(frame.rdi, frame.rsi as usize, frame.rdx as i32, frame.r10 as i32, frame.r8 as i32, frame.r9 as i64),
        nr::MPROTECT => crate::syscall::sys_mprotect(frame.rdi, frame.rsi as usize, frame.rdx as i32),
        nr::MUNMAP => crate::syscall::sys_munmap(frame.rdi, frame.rsi as usize),
        nr::BRK => crate::syscall::sys_brk(frame.rdi),
        nr::IOCTL => crate::syscall::sys_ioctl(frame.rdi as i32, frame.rsi, frame.rdx),
        nr::PIPE => crate::syscall::sys_pipe(frame.rdi),
        nr::DUP => crate::syscall::sys_dup(frame.rdi as i32),
        nr::DUP2 => crate::syscall::sys_dup2(frame.rdi as i32, frame.rsi as i32),
        nr::GETPID => crate::syscall::sys_getpid(),
        nr::EXIT => {
            // exit não retorna
            crate::syscall::sys_exit(frame.rdi);
        }
        nr::FORK => {
            // fork precisa do frame para copiar o estado
            crate::syscall::sys_fork(frame)
        }
        nr::WAIT4 => {
            // wait4(pid, status, options, rusage)
            crate::syscall::sys_wait4(frame.rdi as i64, frame.rsi, frame.rdx as i32, frame.r10)
        }
        nr::WAITID => {
            // waitid(idtype, id, infop, options)
            crate::syscall::sys_waitid(frame.rdi as i32, frame.rsi, frame.rdx, frame.r10 as i32)
        }
        nr::KILL => {
            // kill(pid, sig)
            crate::syscall::sys_kill(frame.rdi as i64, frame.rsi as i32)
        }
        nr::EXECVE => {
            // execve não retorna em caso de sucesso
            crate::syscall::sys_execve(frame.rdi, frame.rsi, frame.rdx)
        }
        nr::UNAME => crate::syscall::sys_uname(frame.rdi),
        nr::GETCWD => crate::syscall::sys_getcwd(frame.rdi, frame.rsi as usize),
        nr::CHDIR => crate::syscall::sys_chdir(frame.rdi),
        nr::MKDIR => crate::syscall::sys_mkdir(frame.rdi, frame.rsi as u32),
        nr::RMDIR => crate::syscall::sys_rmdir(frame.rdi),
        nr::RENAME => crate::syscall::sys_rename(frame.rdi, frame.rsi),
        nr::UNLINK => crate::syscall::sys_unlink(frame.rdi),
        // Symlink syscalls
        nr::SYMLINK => crate::syscall::sys_symlink(frame.rdi, frame.rsi),
        nr::READLINK => crate::syscall::sys_readlink(frame.rdi, frame.rsi, frame.rdx as usize),
        // Truncate syscalls
        nr::TRUNCATE => crate::syscall::sys_truncate(frame.rdi, frame.rsi as i64),
        nr::FTRUNCATE => crate::syscall::sys_ftruncate(frame.rdi as i32, frame.rsi as i64),
        // Fsync syscalls
        nr::FSYNC => crate::syscall::sys_fsync(frame.rdi as i32),
        nr::FDATASYNC => crate::syscall::sys_fdatasync(frame.rdi as i32),
        // Permission syscalls
        nr::CHMOD => crate::syscall::sys_chmod(frame.rdi, frame.rsi as u32),
        nr::FCHMOD => crate::syscall::sys_fchmod(frame.rdi as i32, frame.rsi as u32),
        nr::CHOWN => crate::syscall::sys_chown(frame.rdi, frame.rsi as u32, frame.rdx as u32),
        nr::FCHOWN => crate::syscall::sys_fchown(frame.rdi as i32, frame.rsi as u32, frame.rdx as u32),
        nr::LCHOWN => crate::syscall::sys_lchown(frame.rdi, frame.rsi as u32, frame.rdx as u32),
        // Access syscalls
        nr::ACCESS => crate::syscall::sys_access(frame.rdi, frame.rsi as u32),
        nr::FACCESSAT => crate::syscall::sys_faccessat(frame.rdi as i32, frame.rsi, frame.rdx as u32, frame.r10 as u32),
        nr::FACCESSAT2 => crate::syscall::sys_faccessat(frame.rdi as i32, frame.rsi, frame.rdx as u32, frame.r10 as u32),
        nr::GETUID => crate::syscall::sys_getuid(),
        nr::GETGID => crate::syscall::sys_getgid(),
        nr::SETUID => crate::syscall::sys_setuid(frame.rdi),
        nr::SETGID => crate::syscall::sys_setgid(frame.rdi),
        nr::GETEUID => crate::syscall::sys_geteuid(),
        nr::GETEGID => crate::syscall::sys_getegid(),
        nr::SETREUID => crate::syscall::sys_setreuid(frame.rdi as i64, frame.rsi as i64),
        nr::SETREGID => crate::syscall::sys_setregid(frame.rdi as i64, frame.rsi as i64),
        nr::GETGROUPS => crate::syscall::sys_getgroups(frame.rdi, frame.rsi),
        nr::SETGROUPS => crate::syscall::sys_setgroups(frame.rdi, frame.rsi),
        nr::SETRESUID => crate::syscall::sys_setresuid(frame.rdi as i64, frame.rsi as i64, frame.rdx as i64),
        nr::GETRESUID => crate::syscall::sys_getresuid(frame.rdi, frame.rsi, frame.rdx),
        nr::SETRESGID => crate::syscall::sys_setresgid(frame.rdi as i64, frame.rsi as i64, frame.rdx as i64),
        nr::GETRESGID => crate::syscall::sys_getresgid(frame.rdi, frame.rsi, frame.rdx),
        nr::ARCH_PRCTL => crate::syscall::sys_arch_prctl(frame.rdi as i32, frame.rsi),
        // Socket syscalls
        nr::SOCKET => crate::syscall::sys_socket(frame.rdi as i32, frame.rsi as i32, frame.rdx as i32),
        nr::BIND => crate::syscall::sys_bind(frame.rdi as i32, frame.rsi, frame.rdx as u32),
        nr::LISTEN => crate::syscall::sys_listen(frame.rdi as i32, frame.rsi as i32),
        nr::ACCEPT => crate::syscall::sys_accept(frame.rdi as i32, frame.rsi, frame.rdx),
        nr::CONNECT => crate::syscall::sys_connect(frame.rdi as i32, frame.rsi, frame.rdx as u32),
        nr::SENDTO => crate::syscall::sys_sendto(frame.rdi as i32, frame.rsi, frame.rdx as usize, frame.r10 as i32, frame.r8, frame.r9 as u32),
        nr::RECVFROM => crate::syscall::sys_recvfrom(frame.rdi as i32, frame.rsi, frame.rdx as usize, frame.r10 as i32, frame.r8, frame.r9),
        nr::SHUTDOWN => crate::syscall::sys_shutdown(frame.rdi as i32, frame.rsi as i32),
        nr::GETSOCKNAME => crate::syscall::sys_getsockname(frame.rdi as i32, frame.rsi, frame.rdx),
        nr::GETPEERNAME => crate::syscall::sys_getpeername(frame.rdi as i32, frame.rsi, frame.rdx),
        nr::SETSOCKOPT => crate::syscall::sys_setsockopt(frame.rdi as i32, frame.rsi as i32, frame.rdx as i32, frame.r10, frame.r8 as u32),
        nr::GETSOCKOPT => crate::syscall::sys_getsockopt(frame.rdi as i32, frame.rsi as i32, frame.rdx as i32, frame.r10, frame.r8),
        // Time syscalls
        nr::NANOSLEEP => crate::syscall::sys_nanosleep(frame.rdi, frame.rsi),
        nr::GETTIMEOFDAY => crate::syscall::sys_gettimeofday(frame.rdi, frame.rsi),
        nr::CLOCK_GETTIME => crate::syscall::sys_clock_gettime(frame.rdi as i32, frame.rsi),
        nr::CLOCK_GETRES => crate::syscall::sys_clock_getres(frame.rdi as i32, frame.rsi),
        // I/O multiplexing syscalls
        nr::POLL => crate::syscall::sys_poll(frame.rdi, frame.rsi, frame.rdx as i32),
        nr::SELECT => crate::syscall::sys_select(frame.rdi as i32, frame.rsi, frame.rdx, frame.r10, frame.r8),
        nr::PSELECT6 => crate::syscall::sys_pselect6(frame.rdi as i32, frame.rsi, frame.rdx, frame.r10, frame.r8, frame.r9),
        nr::PPOLL => crate::syscall::sys_ppoll(frame.rdi, frame.rsi, frame.rdx, frame.r10, frame.r8),
        // Thread syscalls
        nr::CLONE => crate::syscall::sys_clone(frame, frame.rdi, frame.rsi, frame.rdx, frame.r10, frame.r8),
        nr::SET_TID_ADDRESS => crate::syscall::sys_set_tid_address(frame.rdi),
        nr::GETTID => crate::syscall::sys_gettid(),
        nr::GETPPID => crate::syscall::sys_getppid(),
        nr::SCHED_YIELD => crate::syscall::sys_sched_yield(),
        nr::SCHED_SETAFFINITY => crate::syscall::sys_sched_setaffinity(frame.rdi, frame.rsi as usize, frame.rdx),
        nr::SCHED_GETAFFINITY => crate::syscall::sys_sched_getaffinity(frame.rdi, frame.rsi as usize, frame.rdx),
        nr::EXIT_GROUP => {
            crate::syscall::sys_exit_group(frame.rdi);
        }
        nr::TGKILL => crate::syscall::sys_tgkill(frame.rdi as i32, frame.rsi as i32, frame.rdx as i32),
        nr::TKILL => crate::syscall::sys_tkill(frame.rdi as i32, frame.rsi as i32),
        // Signal syscalls
        nr::RT_SIGACTION => crate::syscall::sys_rt_sigaction(frame.rdi as i32, frame.rsi, frame.rdx, frame.r10 as usize),
        nr::RT_SIGPROCMASK => crate::syscall::sys_rt_sigprocmask(frame.rdi as i32, frame.rsi, frame.rdx, frame.r10 as usize),
        nr::RT_SIGRETURN => crate::syscall::sys_rt_sigreturn(frame),
        nr::RT_SIGPENDING => crate::syscall::sys_rt_sigpending(frame.rdi, frame.rsi as usize),
        nr::RT_SIGSUSPEND => crate::syscall::sys_rt_sigsuspend(frame.rdi, frame.rsi as usize),
        nr::SIGALTSTACK => crate::syscall::sys_sigaltstack(frame.rdi, frame.rsi),
        // Futex syscall
        nr::FUTEX => crate::syscall::sys_futex(frame.rdi, frame.rsi as i32, frame.rdx as u32, frame.r10, frame.r8, frame.r9 as u32),
        // Process group/session syscalls
        nr::SETPGID => crate::syscall::sys_setpgid(frame.rdi as i64, frame.rsi as i64),
        nr::GETPGID => crate::syscall::sys_getpgid(frame.rdi as i64),
        nr::GETPGRP => crate::syscall::sys_getpgrp(),
        nr::SETSID => crate::syscall::sys_setsid(),
        nr::GETSID => crate::syscall::sys_getsid(frame.rdi as i64),
        // Directory syscalls
        nr::GETDENTS64 => crate::syscall::sys_getdents64(frame.rdi as i32, frame.rsi, frame.rdx as usize),
        // Special file creation (mknod/mkfifo)
        nr::MKNOD => crate::syscall::sys_mknod(frame.rdi, frame.rsi as u32, frame.rdx),
        nr::MKNODAT => crate::syscall::sys_mknodat(frame.rdi as i32, frame.rsi, frame.rdx as u32, frame.r10),
        // Shared memory syscalls
        nr::SHMGET => crate::ipc::sys_shmget(frame.rdi as i32, frame.rsi as usize, frame.rdx as i32),
        nr::SHMAT => crate::ipc::sys_shmat(frame.rdi as i32, frame.rsi, frame.rdx as i32),
        nr::SHMDT => crate::ipc::sys_shmdt(frame.rdi),
        nr::SHMCTL => crate::ipc::sys_shmctl(frame.rdi as i32, frame.rsi as i32, frame.rdx),
        // Message queue syscalls
        nr::MSGGET => crate::ipc::sys_msgget(frame.rdi as i32, frame.rsi as i32),
        nr::MSGSND => crate::ipc::sys_msgsnd(frame.rdi as i32, frame.rsi, frame.rdx as usize, frame.r10 as i32),
        nr::MSGRCV => crate::ipc::sys_msgrcv(frame.rdi as i32, frame.rsi, frame.rdx as usize, frame.r10 as i64, frame.r8 as i32),
        nr::MSGCTL => crate::ipc::sys_msgctl(frame.rdi as i32, frame.rsi as i32, frame.rdx),
        // eventfd syscalls
        nr::EVENTFD => crate::ipc::sys_eventfd(frame.rdi as u32, 0),
        nr::EVENTFD2 => crate::ipc::sys_eventfd2(frame.rdi as u32, frame.rsi as i32),
        // File descriptor control
        nr::FCNTL => crate::syscall::sys_fcntl(frame.rdi as i32, frame.rsi as i32, frame.rdx),
        // Resource limits
        nr::GETRLIMIT => crate::syscall::sys_getrlimit(frame.rdi as u32, frame.rsi),
        nr::SETRLIMIT => crate::syscall::sys_setrlimit(frame.rdi as u32, frame.rsi),
        nr::PRLIMIT64 => crate::syscall::sys_prlimit64(frame.rdi as i32, frame.rsi as u32, frame.rdx, frame.r10),
        // Time
        nr::SETTIMEOFDAY => crate::syscall::sys_settimeofday(frame.rdi, frame.rsi),
        // Reboot/shutdown
        nr::REBOOT => crate::syscall::sys_reboot(frame.rdi as u32, frame.rsi as u32, frame.rdx as u32, frame.r10),
        _ => errno::ENOSYS,
    };

    // Check for pending signals before returning to userspace
    check_and_deliver_signals(frame, result);

    result as u64
}

/// Check for pending signals and deliver them on syscall return
fn check_and_deliver_signals(frame: &mut SyscallFrame, _result: i64) {
    use crate::signal::{sa_flags, setup_signal_frame};

    let task = crate::sched::current_task();

    // Check if there are any deliverable signals
    if !task.signals().has_pending() {
        return;
    }

    // Try to dequeue and deliver a signal
    while let Some(signum) = task.signals().dequeue() {
        let handler = match task.signal_handlers().get(signum) {
            Some(h) => h,
            None => {
                // No handler - re-queue for default handling
                task.signals().send(signum);
                continue;
            }
        };

        if handler.is_ignored() {
            continue;
        }

        if !handler.has_handler() {
            // SIG_DFL - re-queue for default handling in scheduler
            task.signals().send(signum);
            continue;
        }

        // Has a custom handler - set up signal frame
        // Note: SyscallFrame uses rcx for return RIP and r11 for RFLAGS
        let user_rip = frame.rcx;
        let user_rsp = unsafe { (*cpu_local_ptr()).user_rsp_tmp };
        let user_rflags = frame.r11;

        let regs = [
            frame.r15, frame.r14, frame.r13, frame.r12, frame.r11,
            frame.r10, frame.r9, frame.r8, frame.rbp, frame.rdi,
            frame.rsi, frame.rdx, frame.rcx, frame.rbx, frame.rax, 0,
        ];

        let old_mask = task.signals().blocked_mask();

        if let Some((new_sp, handler_addr)) = setup_signal_frame(
            user_rsp,
            signum,
            &handler,
            user_rip,
            user_rsp,
            user_rflags,
            &regs,
            old_mask,
        ) {
            // Block signals during handler (unless SA_NODEFER)
            if (handler.sa_flags & sa_flags::SA_NODEFER) == 0 {
                let new_mask = old_mask | (1u64 << signum) | handler.sa_mask;
                task.signals().set_blocked_mask(new_mask);
            }

            // Reset handler if SA_RESETHAND
            if (handler.sa_flags & sa_flags::SA_RESETHAND) != 0 {
                task.signal_handlers().reset(signum);
            }

            // Modify SyscallFrame to jump to handler
            // RCX contains the return RIP for sysret
            frame.rcx = handler_addr;
            // Set up handler arguments: RDI=signum, RSI=siginfo, RDX=ucontext
            frame.rdi = signum as u64;
            frame.rsi = new_sp + 8; // &info
            frame.rdx = new_sp + 8 + core::mem::size_of::<crate::signal::Siginfo>() as u64; // &uc

            // Update user RSP
            unsafe {
                (*cpu_local_ptr()).user_rsp_tmp = new_sp;
            }

            return;
        }
    }
}

// ---------------- Assembly entry ----------------

global_asm!(r#"
.section .text
.global stenzel_syscall_entry

// Syscall entry (SYSCALL -> ring0)
stenzel_syscall_entry:
    // Troca GS base: de user -> kernel (IA32_KERNEL_GS_BASE)
    swapgs

    // Salva RSP do user em cpu_local.user_rsp_tmp (sem clobber em RAX)
    movq %rsp, %gs:8

    // Troca para kernel stack do task atual
    movq %gs:0, %rsp

    // Garante alinhamento: 16 pushes no total (inclui padding)
    pushq $0

    // Salva registradores (ordem escolhida para preservar retorno em rax)
    // push rax..r15 (rax primeiro, r15 por último)
    push %rax
    push %rbx
    push %rcx
    push %rdx
    push %rsi
    push %rdi
    push %rbp
    push %r8
    push %r9
    push %r10
    push %r11
    push %r12
    push %r13
    push %r14
    push %r15

    // Agora o topo da stack aponta para r15 (SyscallFrame.r15)
    mov %rsp, %rdi

    call syscall_dispatch
    // rax = retorno

    // Restaura regs (exceto rax que manteremos como retorno)
    pop %r15
    pop %r14
    pop %r13
    pop %r12
    pop %r11
    pop %r10
    pop %r9
    pop %r8
    pop %rbp
    pop %rdi
    pop %rsi
    pop %rdx
    pop %rcx
    pop %rbx

    // Descarta rax salvo (syscall number) + pad
    add $16, %rsp

    // Restaura RSP do user
    movq %gs:8, %rdx
    swapgs
    mov %rdx, %rsp
    sysretq
"#, options(att_syntax));

extern "C" {
    fn stenzel_syscall_entry();
}

/// Inicializa MSRs necessários para SYSCALL/SYSRET.
pub fn init() {
    // habilita SCE em IA32_EFER
    const IA32_EFER: u32 = 0xC0000080;
    const IA32_STAR: u32 = 0xC0000081;
    const IA32_LSTAR: u32 = 0xC0000082;
    const IA32_FMASK: u32 = 0xC0000084;
    const IA32_KERNEL_GS_BASE: u32 = 0xC0000102;

    unsafe {
        // IA32_EFER.SCE (bit 0)
        let mut efer = Msr::new(IA32_EFER);
        let mut val = efer.read();
        val |= 1;
        efer.write(val);

        // STAR: kernel CS em [47:32], user base em [63:48]
        let kcs = gdt::kernel_code_selector().0 as u64;
        let ucs = gdt::user_code_selector().0 as u64;
        let user_base = ucs.saturating_sub(16);
        let star = (kcs << 32) | (user_base << 48);
        Msr::new(IA32_STAR).write(star);

        // LSTAR: entrypoint
        Msr::new(IA32_LSTAR).write(stenzel_syscall_entry as *const () as u64);

        // FMASK: limpa IF (bit 9) na entrada
        let fmask = 1u64 << 9;
        Msr::new(IA32_FMASK).write(fmask);

        // KERNEL_GS_BASE -> CpuLocal
        Msr::new(IA32_KERNEL_GS_BASE).write(cpu_local_ptr() as u64);
    }

    crate::kprintln!("syscall: SYSCALL/SYSRET habilitado");
}
