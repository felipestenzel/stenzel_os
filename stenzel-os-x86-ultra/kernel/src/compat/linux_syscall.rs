//! Linux Syscall Compatibility
//!
//! Tracks Linux-specific syscall compatibility.

use super::{CompatLevel, FeatureStatus};
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

/// Get Linux compatibility status
pub fn get_compat_status() -> Vec<FeatureStatus> {
    vec![
        // =========================================================
        // Linux-specific System Calls
        // =========================================================

        // Process
        FeatureStatus {
            name: String::from("clone"),
            level: CompatLevel::Full,
            notes: Some(String::from("CLONE_VM, CLONE_THREAD, CLONE_FS, etc.")),
        },
        FeatureStatus {
            name: String::from("clone3"),
            level: CompatLevel::Stub,
            notes: Some(String::from("Planned")),
        },
        FeatureStatus {
            name: String::from("vfork"),
            level: CompatLevel::Full,
            notes: Some(String::from("Implemented via clone")),
        },
        FeatureStatus {
            name: String::from("set_tid_address"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("futex"),
            level: CompatLevel::Full,
            notes: Some(String::from("FUTEX_WAIT, FUTEX_WAKE")),
        },
        FeatureStatus {
            name: String::from("prctl"),
            level: CompatLevel::Partial,
            notes: Some(String::from("PR_SET_NAME, PR_GET_NAME")),
        },
        FeatureStatus {
            name: String::from("arch_prctl"),
            level: CompatLevel::Full,
            notes: Some(String::from("ARCH_SET_FS, ARCH_SET_GS")),
        },
        FeatureStatus {
            name: String::from("set_robust_list"),
            level: CompatLevel::Stub,
            notes: None,
        },
        FeatureStatus {
            name: String::from("get_robust_list"),
            level: CompatLevel::Stub,
            notes: None,
        },

        // File System
        FeatureStatus {
            name: String::from("openat"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("mkdirat"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("mknodat"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("fchownat"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("fstatat"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("unlinkat"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("renameat"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("linkat"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("symlinkat"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("readlinkat"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("fchmodat"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("faccessat"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("getdents64"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("statx"),
            level: CompatLevel::Stub,
            notes: Some(String::from("Planned")),
        },
        FeatureStatus {
            name: String::from("dup3"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("pipe2"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("eventfd/eventfd2"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("timerfd_create"),
            level: CompatLevel::Stub,
            notes: Some(String::from("Planned")),
        },
        FeatureStatus {
            name: String::from("signalfd"),
            level: CompatLevel::Stub,
            notes: Some(String::from("Planned")),
        },
        FeatureStatus {
            name: String::from("inotify_*"),
            level: CompatLevel::Stub,
            notes: Some(String::from("Planned")),
        },

        // I/O
        FeatureStatus {
            name: String::from("pread64/pwrite64"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("readv/writev"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("preadv/pwritev"),
            level: CompatLevel::Partial,
            notes: None,
        },
        FeatureStatus {
            name: String::from("sendfile"),
            level: CompatLevel::Stub,
            notes: Some(String::from("Planned")),
        },
        FeatureStatus {
            name: String::from("splice/tee"),
            level: CompatLevel::Stub,
            notes: Some(String::from("Planned")),
        },

        // Sockets
        FeatureStatus {
            name: String::from("accept4"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("recvmsg/sendmsg"),
            level: CompatLevel::Partial,
            notes: None,
        },
        FeatureStatus {
            name: String::from("recvmmsg/sendmmsg"),
            level: CompatLevel::Stub,
            notes: None,
        },
        FeatureStatus {
            name: String::from("socketpair"),
            level: CompatLevel::Full,
            notes: None,
        },

        // I/O Multiplexing
        FeatureStatus {
            name: String::from("epoll_create"),
            level: CompatLevel::Partial,
            notes: Some(String::from("Basic support")),
        },
        FeatureStatus {
            name: String::from("epoll_ctl"),
            level: CompatLevel::Partial,
            notes: None,
        },
        FeatureStatus {
            name: String::from("epoll_wait"),
            level: CompatLevel::Partial,
            notes: None,
        },
        FeatureStatus {
            name: String::from("ppoll"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("pselect6"),
            level: CompatLevel::Full,
            notes: None,
        },

        // Signals
        FeatureStatus {
            name: String::from("rt_sigaction"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("rt_sigprocmask"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("rt_sigreturn"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("tgkill"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("tkill"),
            level: CompatLevel::Full,
            notes: None,
        },

        // Memory
        FeatureStatus {
            name: String::from("mremap"),
            level: CompatLevel::Stub,
            notes: Some(String::from("Planned")),
        },
        FeatureStatus {
            name: String::from("madvise"),
            level: CompatLevel::Stub,
            notes: Some(String::from("No-op accepted")),
        },
        FeatureStatus {
            name: String::from("mincore"),
            level: CompatLevel::Stub,
            notes: None,
        },
        FeatureStatus {
            name: String::from("mlock/munlock"),
            level: CompatLevel::Stub,
            notes: None,
        },

        // System Info
        FeatureStatus {
            name: String::from("uname"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("sysinfo"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("getrandom"),
            level: CompatLevel::Full,
            notes: None,
        },

        // User/Group
        FeatureStatus {
            name: String::from("setresuid/getresuid"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("setresgid/getresgid"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("setreuid/setregid"),
            level: CompatLevel::Full,
            notes: None,
        },

        // Scheduling
        FeatureStatus {
            name: String::from("sched_yield"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("sched_getaffinity"),
            level: CompatLevel::Stub,
            notes: Some(String::from("Planned for SMP")),
        },
        FeatureStatus {
            name: String::from("sched_setaffinity"),
            level: CompatLevel::Stub,
            notes: Some(String::from("Planned for SMP")),
        },

        // Misc
        FeatureStatus {
            name: String::from("mount/umount2"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("reboot"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("sethostname"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("gethostname"),
            level: CompatLevel::Full,
            notes: None,
        },
    ]
}

/// Get Linux syscall compatibility percentage
pub fn get_compat_percentage() -> (usize, usize, usize) {
    let status = get_compat_status();
    let total = status.len();
    let full = status.iter().filter(|f| f.level == CompatLevel::Full).count();
    let partial = status.iter().filter(|f| f.level == CompatLevel::Partial).count();
    (full, partial, total)
}

/// Print compatibility summary
pub fn print_compat_summary() {
    let (full, partial, total) = get_compat_percentage();
    crate::kprintln!("Linux Syscall Compatibility: {}/{} full, {}/{} partial",
                     full, total, partial, total);
}
