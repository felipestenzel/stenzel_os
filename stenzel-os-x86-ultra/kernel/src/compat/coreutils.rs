//! GNU Coreutils and Busybox Compatibility
//!
//! Tracks compatibility with common Unix utilities.

use super::{CompatLevel, FeatureStatus};
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

/// Get GNU Coreutils compatibility status
pub fn get_coreutils_status() -> Vec<FeatureStatus> {
    vec![
        // File utilities
        FeatureStatus {
            name: String::from("ls"),
            level: CompatLevel::Full,
            notes: Some(String::from("Basic options: -l, -a, -h, -R")),
        },
        FeatureStatus {
            name: String::from("cp"),
            level: CompatLevel::Full,
            notes: Some(String::from("-r, -p, -a supported")),
        },
        FeatureStatus {
            name: String::from("mv"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("rm"),
            level: CompatLevel::Full,
            notes: Some(String::from("-r, -f supported")),
        },
        FeatureStatus {
            name: String::from("mkdir"),
            level: CompatLevel::Full,
            notes: Some(String::from("-p supported")),
        },
        FeatureStatus {
            name: String::from("rmdir"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("touch"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("cat"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("head"),
            level: CompatLevel::Full,
            notes: Some(String::from("-n supported")),
        },
        FeatureStatus {
            name: String::from("tail"),
            level: CompatLevel::Full,
            notes: Some(String::from("-n, -f supported")),
        },
        FeatureStatus {
            name: String::from("wc"),
            level: CompatLevel::Full,
            notes: Some(String::from("-l, -w, -c supported")),
        },
        FeatureStatus {
            name: String::from("sort"),
            level: CompatLevel::Partial,
            notes: Some(String::from("Basic sorting")),
        },
        FeatureStatus {
            name: String::from("uniq"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("cut"),
            level: CompatLevel::Partial,
            notes: Some(String::from("-d, -f supported")),
        },
        FeatureStatus {
            name: String::from("tr"),
            level: CompatLevel::Partial,
            notes: None,
        },
        FeatureStatus {
            name: String::from("grep"),
            level: CompatLevel::Partial,
            notes: Some(String::from("Basic patterns, -i, -v, -n")),
        },
        FeatureStatus {
            name: String::from("find"),
            level: CompatLevel::Partial,
            notes: Some(String::from("-name, -type")),
        },
        FeatureStatus {
            name: String::from("xargs"),
            level: CompatLevel::Partial,
            notes: None,
        },

        // Text utilities
        FeatureStatus {
            name: String::from("echo"),
            level: CompatLevel::Full,
            notes: Some(String::from("-n, -e supported")),
        },
        FeatureStatus {
            name: String::from("printf"),
            level: CompatLevel::Partial,
            notes: Some(String::from("Basic format strings")),
        },
        FeatureStatus {
            name: String::from("yes"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("true"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("false"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("test"),
            level: CompatLevel::Full,
            notes: Some(String::from("Also [ ]")),
        },
        FeatureStatus {
            name: String::from("expr"),
            level: CompatLevel::Partial,
            notes: None,
        },
        FeatureStatus {
            name: String::from("seq"),
            level: CompatLevel::Full,
            notes: None,
        },

        // File info
        FeatureStatus {
            name: String::from("stat"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("file"),
            level: CompatLevel::Partial,
            notes: Some(String::from("Basic magic detection")),
        },
        FeatureStatus {
            name: String::from("du"),
            level: CompatLevel::Full,
            notes: Some(String::from("-s, -h supported")),
        },
        FeatureStatus {
            name: String::from("df"),
            level: CompatLevel::Full,
            notes: Some(String::from("-h supported")),
        },
        FeatureStatus {
            name: String::from("ln"),
            level: CompatLevel::Full,
            notes: Some(String::from("-s for symlinks")),
        },
        FeatureStatus {
            name: String::from("readlink"),
            level: CompatLevel::Full,
            notes: Some(String::from("-f supported")),
        },
        FeatureStatus {
            name: String::from("realpath"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("basename"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("dirname"),
            level: CompatLevel::Full,
            notes: None,
        },

        // Permissions
        FeatureStatus {
            name: String::from("chmod"),
            level: CompatLevel::Full,
            notes: Some(String::from("Numeric and symbolic modes")),
        },
        FeatureStatus {
            name: String::from("chown"),
            level: CompatLevel::Full,
            notes: Some(String::from("-R supported")),
        },
        FeatureStatus {
            name: String::from("chgrp"),
            level: CompatLevel::Full,
            notes: None,
        },

        // User info
        FeatureStatus {
            name: String::from("id"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("whoami"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("groups"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("users"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("who"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("w"),
            level: CompatLevel::Partial,
            notes: None,
        },

        // System info
        FeatureStatus {
            name: String::from("uname"),
            level: CompatLevel::Full,
            notes: Some(String::from("-a supported")),
        },
        FeatureStatus {
            name: String::from("hostname"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("uptime"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("date"),
            level: CompatLevel::Full,
            notes: Some(String::from("Basic format strings")),
        },
        FeatureStatus {
            name: String::from("cal"),
            level: CompatLevel::Full,
            notes: None,
        },

        // Process
        FeatureStatus {
            name: String::from("ps"),
            level: CompatLevel::Full,
            notes: Some(String::from("-aux supported")),
        },
        FeatureStatus {
            name: String::from("kill"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("killall"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("nohup"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("nice"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("sleep"),
            level: CompatLevel::Full,
            notes: None,
        },

        // Shell builtins (for busybox sh)
        FeatureStatus {
            name: String::from("cd"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("pwd"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("export"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("env"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("set"),
            level: CompatLevel::Partial,
            notes: None,
        },
        FeatureStatus {
            name: String::from("unset"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("exit"),
            level: CompatLevel::Full,
            notes: None,
        },

        // Network (busybox)
        FeatureStatus {
            name: String::from("ping"),
            level: CompatLevel::Full,
            notes: Some(String::from("-c supported")),
        },
        FeatureStatus {
            name: String::from("ifconfig"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("route"),
            level: CompatLevel::Partial,
            notes: None,
        },
        FeatureStatus {
            name: String::from("netstat"),
            level: CompatLevel::Partial,
            notes: None,
        },
        FeatureStatus {
            name: String::from("wget"),
            level: CompatLevel::Partial,
            notes: Some(String::from("HTTP/HTTPS")),
        },

        // Archive
        FeatureStatus {
            name: String::from("tar"),
            level: CompatLevel::Partial,
            notes: Some(String::from("-c, -x, -t, -z, -v, -f")),
        },
        FeatureStatus {
            name: String::from("gzip"),
            level: CompatLevel::Partial,
            notes: None,
        },
        FeatureStatus {
            name: String::from("gunzip"),
            level: CompatLevel::Partial,
            notes: None,
        },

        // Misc
        FeatureStatus {
            name: String::from("clear"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("tee"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("dd"),
            level: CompatLevel::Partial,
            notes: Some(String::from("if=, of=, bs=, count=")),
        },
        FeatureStatus {
            name: String::from("mount"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("umount"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("dmesg"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("reboot"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("poweroff"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("halt"),
            level: CompatLevel::Full,
            notes: None,
        },
    ]
}

/// Get Busybox applet compatibility percentage
pub fn get_busybox_compat_percentage() -> (usize, usize, usize) {
    let status = get_coreutils_status();
    let total = status.len();
    let full = status.iter().filter(|f| f.level == CompatLevel::Full).count();
    let partial = status.iter().filter(|f| f.level == CompatLevel::Partial).count();
    (full, partial, total)
}

/// Print compatibility summary
pub fn print_compat_summary() {
    let (full, partial, total) = get_busybox_compat_percentage();
    crate::kprintln!("Coreutils/Busybox Compatibility: {}/{} full, {}/{} partial",
                     full, total, partial, total);
}
