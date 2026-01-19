//! VMware Tools Support
//!
//! Provides VMware guest integration features.

#![allow(dead_code)]

pub mod vmxnet3;
pub mod svga;
pub mod balloon;
pub mod rpc;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;

/// VMware backdoor I/O port
const VMWARE_BDOOR_PORT: u16 = 0x5658;

/// VMware RPC port
const VMWARE_RPC_PORT: u16 = 0x5659;

/// VMware backdoor magic
const VMWARE_MAGIC: u32 = 0x564D5868; // "VMXh"

/// VMware backdoor commands
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmwareCommand {
    GetVersion = 10,
    Message = 30,
    GetMemSize = 14,
    GetCpuSpeed = 8,
    GetCpuIndex = 44,
    GetGuiOptions = 54,
    SetMouseCursor = 46,
    GetTime = 23,
    GetTimeShift = 101,
    GetPtrState = 39,
    SetPtrState = 40,
}

/// VMware version info
#[derive(Debug, Clone, Copy)]
pub struct VmwareVersion {
    pub version: u32,
    pub product_type: u32,
    pub hypervisor: u32,
}

/// VMware feature flags
#[derive(Debug, Clone, Copy, Default)]
pub struct VmwareFeatures {
    pub backdoor: bool,
    pub vmxnet3: bool,
    pub svga: bool,
    pub balloon: bool,
    pub time_sync: bool,
    pub guest_rpc: bool,
    pub drag_drop: bool,
    pub clipboard: bool,
    pub unity: bool,
}

/// VMware guest state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmwareGuestState {
    NotDetected,
    Detected,
    Initialized,
    Running,
    Error,
}

/// VMware Tools statistics
#[derive(Debug, Default)]
pub struct VmwareStats {
    pub backdoor_calls: AtomicU64,
    pub rpc_messages: AtomicU64,
    pub time_syncs: AtomicU64,
    pub balloon_pages: AtomicU64,
}

/// VMware Tools manager
pub struct VmwareTools {
    /// Guest state
    state: VmwareGuestState,
    /// VMware version
    version: Option<VmwareVersion>,
    /// Detected features
    features: VmwareFeatures,
    /// Initialized
    initialized: AtomicBool,
    /// Statistics
    stats: VmwareStats,
    /// Host time offset
    time_offset: i64,
    /// Balloon target
    balloon_target_mb: u32,
}

impl VmwareTools {
    /// Create new VMware tools instance
    pub fn new() -> Self {
        Self {
            state: VmwareGuestState::NotDetected,
            version: None,
            features: VmwareFeatures::default(),
            initialized: AtomicBool::new(false),
            stats: VmwareStats::default(),
            time_offset: 0,
            balloon_target_mb: 0,
        }
    }

    /// Check if running under VMware
    pub fn detect() -> bool {
        // Try the backdoor
        let result = Self::backdoor_command(VmwareCommand::GetVersion as u32, 0);
        result.is_some()
    }

    /// Execute backdoor command
    fn backdoor_command(cmd: u32, param: u32) -> Option<(u32, u32, u32, u32)> {
        let mut eax: u32 = VMWARE_MAGIC;
        let mut ebx: u32 = param;
        let ecx: u32 = cmd;
        let edx: u32 = VMWARE_BDOOR_PORT as u32;

        // Execute backdoor
        // In real implementation, use inline assembly
        // For now, simulate non-VMware environment
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!(
                "push rbx",
                "mov eax, {magic:e}",
                "mov ebx, {param:e}",
                "mov ecx, {cmd:e}",
                "mov dx, {port:x}",
                "in eax, dx",
                "mov {eax_out:e}, eax",
                "mov {ebx_out:e}, ebx",
                "pop rbx",
                magic = in(reg) VMWARE_MAGIC,
                param = in(reg) param,
                cmd = in(reg) cmd,
                port = in(reg) VMWARE_BDOOR_PORT,
                eax_out = out(reg) eax,
                ebx_out = out(reg) ebx,
                options(nostack, nomem)
            );
        }

        // Check magic response
        if ebx == VMWARE_MAGIC {
            Some((eax, ebx, 0, 0))
        } else {
            None
        }
    }

    /// Initialize VMware tools
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Detect VMware
        if !Self::detect() {
            self.state = VmwareGuestState::NotDetected;
            return Err("Not running under VMware");
        }

        self.state = VmwareGuestState::Detected;

        // Get version
        if let Some((version, product, _, _)) = Self::backdoor_command(VmwareCommand::GetVersion as u32, 0) {
            self.version = Some(VmwareVersion {
                version,
                product_type: product,
                hypervisor: 0,
            });
        }

        // Detect features
        self.features.backdoor = true;
        self.features.time_sync = true;
        self.features.guest_rpc = true;
        self.features.balloon = true;

        // Initial time sync
        self.sync_time();

        self.state = VmwareGuestState::Initialized;
        self.initialized.store(true, Ordering::Release);

        crate::kprintln!("vmware: Tools initialized");
        Ok(())
    }

    /// Get VMware version
    pub fn version(&self) -> Option<&VmwareVersion> {
        self.version.as_ref()
    }

    /// Get features
    pub fn features(&self) -> &VmwareFeatures {
        &self.features
    }

    /// Get current state
    pub fn state(&self) -> VmwareGuestState {
        self.state
    }

    /// Synchronize time with host
    pub fn sync_time(&mut self) {
        if let Some((time_low, time_high, _, _)) = Self::backdoor_command(VmwareCommand::GetTime as u32, 0) {
            let host_time = ((time_high as u64) << 32) | (time_low as u64);
            // Convert to local time offset
            self.time_offset = host_time as i64;
            self.stats.time_syncs.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get host time
    pub fn get_host_time(&self) -> i64 {
        self.time_offset
    }

    /// Get memory size
    pub fn get_memory_size(&mut self) -> Option<u32> {
        Self::backdoor_command(VmwareCommand::GetMemSize as u32, 0)
            .map(|(mem_mb, _, _, _)| mem_mb)
    }

    /// Get CPU speed
    pub fn get_cpu_speed(&mut self) -> Option<u32> {
        Self::backdoor_command(VmwareCommand::GetCpuSpeed as u32, 0)
            .map(|(speed_mhz, _, _, _)| speed_mhz)
    }

    /// Set balloon target (memory to give back)
    pub fn set_balloon_target(&mut self, target_mb: u32) {
        self.balloon_target_mb = target_mb;
        self.stats.balloon_pages.store((target_mb as u64) * 256, Ordering::Relaxed);
    }

    /// Get statistics
    pub fn stats(&self) -> &VmwareStats {
        &self.stats
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        if let Some(ver) = &self.version {
            alloc::format!(
                "VMware Tools: v{} state={:?}",
                ver.version, self.state
            )
        } else {
            alloc::format!("VMware Tools: state={:?}", self.state)
        }
    }
}

impl Default for VmwareTools {
    fn default() -> Self {
        Self::new()
    }
}

// Global VMware Tools manager
static VMWARE_TOOLS: IrqSafeMutex<Option<VmwareTools>> = IrqSafeMutex::new(None);

/// Initialize VMware tools
pub fn init() {
    let mut tools = VmwareTools::new();
    let result = tools.init();

    let status = tools.format_status();
    *VMWARE_TOOLS.lock() = Some(tools);

    match result {
        Ok(_) => crate::kprintln!("{}", status),
        Err(_) => crate::kprintln!("vmware: Not detected (not running under VMware)"),
    }
}

/// Check if running under VMware
pub fn is_vmware() -> bool {
    VMWARE_TOOLS.lock()
        .as_ref()
        .map(|t| t.state != VmwareGuestState::NotDetected)
        .unwrap_or(false)
}

/// Get status string
pub fn status() -> String {
    VMWARE_TOOLS.lock()
        .as_ref()
        .map(|t| t.format_status())
        .unwrap_or_else(|| "VMware Tools not initialized".into())
}

/// Sync time with host
pub fn sync_time() {
    if let Some(ref mut tools) = *VMWARE_TOOLS.lock() {
        tools.sync_time();
    }
}
