//! Compatibility Layer
//!
//! Provides POSIX, Linux, and Windows compatibility for applications.

extern crate alloc;

pub mod posix;
pub mod linux_syscall;
pub mod linux;
pub mod coreutils;
pub mod windows;
pub mod containers;
pub mod flatpak;
pub mod appimage;

use alloc::string::String;
use alloc::vec::Vec;

/// Compatibility level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatLevel {
    /// Fully implemented
    Full,
    /// Partially implemented
    Partial,
    /// Stub only (returns ENOSYS)
    Stub,
    /// Not implemented
    None,
}

/// Feature status
#[derive(Debug, Clone)]
pub struct FeatureStatus {
    pub name: String,
    pub level: CompatLevel,
    pub notes: Option<String>,
}

/// Get overall POSIX compliance status
pub fn posix_compliance_summary() -> Vec<FeatureStatus> {
    posix::get_compliance_status()
}

/// Get overall Linux syscall compatibility status
pub fn linux_syscall_summary() -> Vec<FeatureStatus> {
    linux_syscall::get_compat_status()
}

/// Get overall Linux compatibility layer status
pub fn linux_compat_summary() -> Vec<FeatureStatus> {
    linux::get_linux_compat_layer_status()
}

/// Check if a specific feature is available
pub fn has_feature(feature: &str) -> CompatLevel {
    // Check POSIX features
    if let Some(status) = posix::get_compliance_status()
        .iter()
        .find(|f| f.name == feature)
    {
        return status.level;
    }

    // Check Linux syscall features
    if let Some(status) = linux_syscall::get_compat_status()
        .iter()
        .find(|f| f.name == feature)
    {
        return status.level;
    }

    // Check Linux layer features
    if let Some(status) = linux::get_linux_compat_layer_status()
        .iter()
        .find(|f| f.name == feature)
    {
        return status.level;
    }

    // Check Windows features
    if let Some(status) = windows::get_windows_compat_status()
        .iter()
        .find(|f| f.name == feature)
    {
        return status.level;
    }

    CompatLevel::None
}

/// Get Windows compatibility status
pub fn windows_compat_summary() -> Vec<FeatureStatus> {
    windows::get_windows_compat_status()
}

/// Initialize Windows compatibility layer
pub fn init_windows() {
    windows::init();
}

/// Initialize Linux compatibility layer
pub fn init_linux() {
    linux::init();
}

/// Initialize container runtime
pub fn init_containers() {
    containers::init();
}

/// Initialize Flatpak compatibility layer
pub fn init_flatpak() {
    flatpak::init();
}

/// Initialize AppImage support
pub fn init_appimage() {
    appimage::init();
}

/// Initialize all compatibility layers
pub fn init_all() {
    init_windows();
    init_linux();
    init_containers();
    init_flatpak();
    init_appimage();
}
