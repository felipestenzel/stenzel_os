//! Windows Compatibility Layer
//!
//! Provides Wine-like compatibility for running Windows PE executables.
//! This implements:
//! - PE/COFF executable loader
//! - Windows API emulation (NTDLL, KERNEL32, USER32, GDI32, etc.)
//! - Registry emulation
//! - Windows filesystem path translation

extern crate alloc;

pub mod pe;
pub mod ntdll;
pub mod kernel32;
pub mod msvcrt;
pub mod user32;
pub mod gdi32;
pub mod advapi32;
pub mod shell32;
pub mod comctl32;
pub mod ole32;
pub mod d3d9;
pub mod clr;
pub mod loader;
pub mod registry;
pub mod fs_translate;
pub mod ntsyscall;

use alloc::string::String;
use alloc::vec::Vec;
use super::{CompatLevel, FeatureStatus};

/// Windows subsystem type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsSubsystem {
    Unknown,
    Native,
    WindowsGui,
    WindowsCui,  // Console
    Os2Cui,
    PosixCui,
    WindowsCeGui,
    EfiApplication,
    EfiBootServiceDriver,
    EfiRuntimeDriver,
    EfiRom,
    Xbox,
    WindowsBootApplication,
}

impl WindowsSubsystem {
    pub fn from_u16(value: u16) -> Self {
        match value {
            0 => WindowsSubsystem::Unknown,
            1 => WindowsSubsystem::Native,
            2 => WindowsSubsystem::WindowsGui,
            3 => WindowsSubsystem::WindowsCui,
            5 => WindowsSubsystem::Os2Cui,
            7 => WindowsSubsystem::PosixCui,
            9 => WindowsSubsystem::WindowsCeGui,
            10 => WindowsSubsystem::EfiApplication,
            11 => WindowsSubsystem::EfiBootServiceDriver,
            12 => WindowsSubsystem::EfiRuntimeDriver,
            13 => WindowsSubsystem::EfiRom,
            14 => WindowsSubsystem::Xbox,
            16 => WindowsSubsystem::WindowsBootApplication,
            _ => WindowsSubsystem::Unknown,
        }
    }
}

/// Windows compatibility status
pub fn get_windows_compat_status() -> Vec<FeatureStatus> {
    alloc::vec![
        // Core PE/COFF Support
        FeatureStatus {
            name: String::from("PE32 Loader"),
            level: CompatLevel::Full,
            notes: Some(String::from("32-bit PE executables")),
        },
        FeatureStatus {
            name: String::from("PE32+ Loader"),
            level: CompatLevel::Full,
            notes: Some(String::from("64-bit PE executables")),
        },
        FeatureStatus {
            name: String::from("DLL Loading"),
            level: CompatLevel::Full,
            notes: Some(String::from("Import resolution, built-in DLLs")),
        },
        FeatureStatus {
            name: String::from("Import Table"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("Export Table"),
            level: CompatLevel::Full,
            notes: None,
        },
        FeatureStatus {
            name: String::from("Relocation"),
            level: CompatLevel::Full,
            notes: Some(String::from("Base relocation support")),
        },
        FeatureStatus {
            name: String::from("TLS"),
            level: CompatLevel::Full,
            notes: Some(String::from("Thread-local storage via TlsAlloc/TlsFree")),
        },
        FeatureStatus {
            name: String::from("Resources"),
            level: CompatLevel::Partial,
            notes: Some(String::from("Basic resource parsing")),
        },

        // NT Syscalls
        FeatureStatus {
            name: String::from("NT Syscalls"),
            level: CompatLevel::Full,
            notes: Some(String::from("40+ syscalls: process, thread, memory, file, sync")),
        },

        // API DLLs
        FeatureStatus {
            name: String::from("NTDLL"),
            level: CompatLevel::Full,
            notes: Some(String::from("Core NT functions, Rtl/Ldr/Nt APIs")),
        },
        FeatureStatus {
            name: String::from("KERNEL32"),
            level: CompatLevel::Full,
            notes: Some(String::from("File, process, memory, console, TLS APIs")),
        },
        FeatureStatus {
            name: String::from("MSVCRT"),
            level: CompatLevel::Full,
            notes: Some(String::from("C runtime: stdio, stdlib, string, time")),
        },
        FeatureStatus {
            name: String::from("USER32"),
            level: CompatLevel::Full,
            notes: Some(String::from("Window, message, input, dialog APIs")),
        },
        FeatureStatus {
            name: String::from("GDI32"),
            level: CompatLevel::Full,
            notes: Some(String::from("DC, pen, brush, bitmap, text APIs")),
        },
        FeatureStatus {
            name: String::from("ADVAPI32"),
            level: CompatLevel::Full,
            notes: Some(String::from("Registry, security, crypto, services")),
        },
        FeatureStatus {
            name: String::from("SHELL32"),
            level: CompatLevel::Full,
            notes: Some(String::from("Shell folders, file ops, icons, drag-drop, paths")),
        },
        FeatureStatus {
            name: String::from("COMCTL32"),
            level: CompatLevel::Full,
            notes: Some(String::from("Common controls: ListView, TreeView, TabControl, etc.")),
        },
        FeatureStatus {
            name: String::from("OLE32"),
            level: CompatLevel::Full,
            notes: Some(String::from("COM/OLE: CoCreateInstance, IUnknown, storage, monikers")),
        },
        FeatureStatus {
            name: String::from("D3D9"),
            level: CompatLevel::Full,
            notes: Some(String::from("Direct3D 9: device, textures, shaders, rendering")),
        },
        FeatureStatus {
            name: String::from(".NET CLR"),
            level: CompatLevel::Partial,
            notes: Some(String::from("Basic CLI runtime: IL execution, type system, GC")),
        },

        // System Features
        FeatureStatus {
            name: String::from("Registry"),
            level: CompatLevel::Full,
            notes: Some(String::from("Full emulation with HKLM/HKCU/HKCR")),
        },
        FeatureStatus {
            name: String::from("Windows Paths"),
            level: CompatLevel::Full,
            notes: Some(String::from("C:\\ → /mnt/c translation")),
        },
        FeatureStatus {
            name: String::from("SEH"),
            level: CompatLevel::Partial,
            notes: Some(String::from("Basic exception handling")),
        },
    ]
}

/// Get Windows compatibility percentage
pub fn get_compat_percentage() -> (usize, usize, usize) {
    let status = get_windows_compat_status();
    let total = status.len();
    let full = status.iter().filter(|f| f.level == CompatLevel::Full).count();
    let partial = status.iter().filter(|f| f.level == CompatLevel::Partial).count();
    (full, partial, total)
}

/// Print Windows compatibility summary
pub fn print_compat_summary() {
    let (full, partial, total) = get_compat_percentage();
    crate::kprintln!("Windows Compatibility: {}/{} full, {}/{} partial",
                     full, total, partial, total);
}

/// Initialize Windows compatibility layer
pub fn init() {
    crate::kprintln!("wincompat: initializing Windows compatibility layer");

    // Initialize subsystems
    registry::init();
    fs_translate::init();
    advapi32::init();
    shell32::init();
    comctl32::init();
    ole32::init();
    d3d9::init();
    ntsyscall::init();
    loader::init();
    clr::init();

    let (full, partial, total) = get_compat_percentage();
    crate::kprintln!("wincompat: Windows compatibility layer ready ({}/{} full, {}/{} partial)",
                     full, total, partial, total);
}
