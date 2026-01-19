//! Microsoft Hyper-V Guest Integration
//!
//! Provides Hyper-V guest support for running under Windows Hyper-V hypervisor.

#![allow(dead_code)]

pub mod vmbus;
pub mod netvsc;
pub mod storvsc;
pub mod timesync;
pub mod shutdown;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;

/// Hyper-V CPUID leaves
mod cpuid {
    pub const HYPERV_CPUID_VENDOR: u32 = 0x40000000;
    pub const HYPERV_CPUID_INTERFACE: u32 = 0x40000001;
    pub const HYPERV_CPUID_VERSION: u32 = 0x40000002;
    pub const HYPERV_CPUID_FEATURES: u32 = 0x40000003;
    pub const HYPERV_CPUID_RECOMMENDATIONS: u32 = 0x40000004;
    pub const HYPERV_CPUID_LIMITS: u32 = 0x40000005;
    pub const HYPERV_CPUID_HARDWARE_FEATURES: u32 = 0x40000006;
}

/// Hyper-V MSRs
mod msr {
    pub const HV_X64_MSR_GUEST_OS_ID: u32 = 0x40000000;
    pub const HV_X64_MSR_HYPERCALL: u32 = 0x40000001;
    pub const HV_X64_MSR_VP_INDEX: u32 = 0x40000002;
    pub const HV_X64_MSR_RESET: u32 = 0x40000003;
    pub const HV_X64_MSR_VP_RUNTIME: u32 = 0x40000010;
    pub const HV_X64_MSR_TIME_REF_COUNT: u32 = 0x40000020;
    pub const HV_X64_MSR_REFERENCE_TSC: u32 = 0x40000021;
    pub const HV_X64_MSR_TSC_FREQUENCY: u32 = 0x40000022;
    pub const HV_X64_MSR_APIC_FREQUENCY: u32 = 0x40000023;
    pub const HV_X64_MSR_SCONTROL: u32 = 0x40000080;
    pub const HV_X64_MSR_SVERSION: u32 = 0x40000081;
    pub const HV_X64_MSR_SIEFP: u32 = 0x40000082;
    pub const HV_X64_MSR_SIMP: u32 = 0x40000083;
    pub const HV_X64_MSR_EOM: u32 = 0x40000084;
    pub const HV_X64_MSR_SINT0: u32 = 0x40000090;
}

/// Hypercall codes
#[repr(u16)]
#[derive(Debug, Clone, Copy)]
pub enum HypercallCode {
    PostMessage = 0x005C,
    SignalEvent = 0x005D,
    PostDebugData = 0x0069,
    RetrieveDebugData = 0x006A,
    ResetDebugSession = 0x006B,
    MapGpaPages = 0x006C,
    UnmapGpaPages = 0x006D,
    InstallIntercept = 0x004D,
    FlushVirtualAddressSpace = 0x0002,
    FlushVirtualAddressList = 0x0003,
    NotifyLongSpinWait = 0x0008,
}

/// Hypercall status codes
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypercallStatus {
    Success = 0x0000,
    InvalidHypercallCode = 0x0002,
    InvalidHypercallInput = 0x0003,
    InvalidAlignment = 0x0004,
    InvalidParameter = 0x0005,
    AccessDenied = 0x0006,
    InvalidPartitionState = 0x0007,
    OperationDenied = 0x0008,
    UnknownProperty = 0x0009,
    PropertyValueOutOfRange = 0x000A,
    InsufficientMemory = 0x000B,
    PartitionTooDeep = 0x000C,
    InvalidPartitionId = 0x000D,
    InvalidVpIndex = 0x000E,
    InvalidPortId = 0x0011,
    InvalidConnectionId = 0x0012,
    InsufficientBuffers = 0x0013,
    NotAcknowledged = 0x0014,
    InvalidVpState = 0x0015,
    Acknowledged = 0x0016,
    InvalidSaveRestoreState = 0x0017,
    InvalidSynicState = 0x0018,
    ObjectInUse = 0x0019,
    InvalidProximityDomainInfo = 0x001A,
    NoData = 0x001B,
    Inactive = 0x001C,
    NoResources = 0x001D,
    FeatureUnavailable = 0x001E,
}

/// Hyper-V features
#[derive(Debug, Clone, Copy, Default)]
pub struct HypervFeatures {
    /// VP runtime MSR
    pub vp_runtime: bool,
    /// Time reference count
    pub time_ref_count: bool,
    /// Reference TSC
    pub reference_tsc: bool,
    /// APIC access
    pub apic_access: bool,
    /// Hypercall MSR
    pub hypercall_msr: bool,
    /// VP index MSR
    pub vp_index_msr: bool,
    /// Virtual system reset
    pub virt_system_reset: bool,
    /// Frequency MSRs
    pub frequency_msrs: bool,
    /// SynIC available
    pub synic: bool,
    /// Synthetic timer
    pub stimer: bool,
    /// APIC EOI avoidance
    pub apic_eoi: bool,
    /// Debug features
    pub debug: bool,
}

/// Hyper-V recommendations
#[derive(Debug, Clone, Copy, Default)]
pub struct HypervRecommendations {
    /// Use hypercall for address space switching
    pub use_hypercall_address_switch: bool,
    /// Use hypercall for local TLB flush
    pub use_hypercall_local_flush: bool,
    /// Use hypercall for remote TLB flush
    pub use_hypercall_remote_flush: bool,
    /// Use MSRs for APIC access
    pub use_msr_apic: bool,
    /// Use MSR for system reset
    pub use_msr_reset: bool,
    /// Relaxed timing
    pub relaxed_timing: bool,
    /// Use DMA remapping
    pub use_dma_remapping: bool,
    /// Use interrupt remapping
    pub use_interrupt_remapping: bool,
    /// Use x2APIC MSRs
    pub use_x2apic_msrs: bool,
    /// Deprecate AutoEOI
    pub deprecate_autoeoi: bool,
    /// Use SynIC auto EOI
    pub use_synic_autoeoi: bool,
    /// Use relaxed timing for HPET
    pub use_relaxed_timing: bool,
    /// Use long spin wait
    pub use_long_spin_wait: bool,
}

/// Hyper-V version info
#[derive(Debug, Clone, Copy)]
pub struct HypervVersion {
    pub build: u32,
    pub minor: u16,
    pub major: u16,
    pub service_pack: u32,
    pub service_branch: u8,
    pub service_number: u32,
}

/// Hyper-V guest state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypervState {
    NotDetected,
    Detected,
    Initializing,
    Running,
    Error,
}

/// Hyper-V statistics
#[derive(Debug, Default)]
pub struct HypervStats {
    pub hypercalls: AtomicU64,
    pub vmbus_messages: AtomicU64,
    pub interrupts: AtomicU64,
    pub errors: AtomicU64,
}

/// Hyper-V guest manager
pub struct HypervGuest {
    /// Current state
    state: HypervState,
    /// Version info
    version: Option<HypervVersion>,
    /// Features
    features: HypervFeatures,
    /// Recommendations
    recommendations: HypervRecommendations,
    /// Hypercall page physical address
    hypercall_page: u64,
    /// SynIC enabled
    synic_enabled: bool,
    /// Initialized flag
    initialized: AtomicBool,
    /// Statistics
    stats: HypervStats,
}

impl HypervGuest {
    /// Hyper-V signature "Microsoft Hv"
    const HYPERV_SIGNATURE: &'static [u8] = b"Microsoft Hv";

    /// Create new Hyper-V guest manager
    pub fn new() -> Self {
        Self {
            state: HypervState::NotDetected,
            version: None,
            features: HypervFeatures::default(),
            recommendations: HypervRecommendations::default(),
            hypercall_page: 0,
            synic_enabled: false,
            initialized: AtomicBool::new(false),
            stats: HypervStats::default(),
        }
    }

    /// Check if running under Hyper-V
    pub fn detect() -> bool {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            let eax: u32;
            let ebx: u32;
            let ecx: u32;
            let edx: u32;

            core::arch::asm!(
                "push rbx",
                "mov eax, {leaf:e}",
                "cpuid",
                "mov {ebx_out:e}, ebx",
                "pop rbx",
                leaf = in(reg) cpuid::HYPERV_CPUID_VENDOR,
                ebx_out = out(reg) ebx,
                out("eax") eax,
                out("ecx") ecx,
                out("edx") edx,
                options(nostack)
            );

            // Check signature "Microsoft Hv"
            let sig_bytes = [
                ebx.to_le_bytes(),
                ecx.to_le_bytes(),
                edx.to_le_bytes(),
            ];

            let sig: [u8; 12] = [
                sig_bytes[0][0], sig_bytes[0][1], sig_bytes[0][2], sig_bytes[0][3],
                sig_bytes[1][0], sig_bytes[1][1], sig_bytes[1][2], sig_bytes[1][3],
                sig_bytes[2][0], sig_bytes[2][1], sig_bytes[2][2], sig_bytes[2][3],
            ];

            sig == *Self::HYPERV_SIGNATURE
        }

        #[cfg(not(target_arch = "x86_64"))]
        false
    }

    /// Get Hyper-V version
    fn get_version() -> Option<HypervVersion> {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            let eax: u32;
            let ebx: u32;
            let ecx: u32;
            let edx: u32;

            core::arch::asm!(
                "push rbx",
                "mov eax, {leaf:e}",
                "cpuid",
                "mov {ebx_out:e}, ebx",
                "pop rbx",
                leaf = in(reg) cpuid::HYPERV_CPUID_VERSION,
                ebx_out = out(reg) ebx,
                out("eax") eax,
                out("ecx") ecx,
                out("edx") edx,
                options(nostack)
            );

            Some(HypervVersion {
                build: eax,
                minor: (ebx & 0xFFFF) as u16,
                major: (ebx >> 16) as u16,
                service_pack: ecx,
                service_branch: (edx >> 24) as u8,
                service_number: edx & 0xFFFFFF,
            })
        }

        #[cfg(not(target_arch = "x86_64"))]
        None
    }

    /// Get features from CPUID
    fn get_features() -> HypervFeatures {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            let eax: u32;
            let _ebx: u32;
            let _ecx: u32;
            let edx: u32;

            core::arch::asm!(
                "push rbx",
                "mov eax, {leaf:e}",
                "cpuid",
                "mov {ebx_out:e}, ebx",
                "pop rbx",
                leaf = in(reg) cpuid::HYPERV_CPUID_FEATURES,
                ebx_out = out(reg) _ebx,
                out("eax") eax,
                out("ecx") _ecx,
                out("edx") edx,
                options(nostack)
            );

            HypervFeatures {
                vp_runtime: eax & (1 << 0) != 0,
                time_ref_count: eax & (1 << 1) != 0,
                reference_tsc: eax & (1 << 9) != 0,
                apic_access: eax & (1 << 3) != 0,
                hypercall_msr: eax & (1 << 5) != 0,
                vp_index_msr: eax & (1 << 6) != 0,
                virt_system_reset: eax & (1 << 7) != 0,
                frequency_msrs: eax & (1 << 11) != 0,
                synic: eax & (1 << 2) != 0,
                stimer: eax & (1 << 3) != 0,
                apic_eoi: eax & (1 << 4) != 0,
                debug: edx & (1 << 0) != 0,
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        HypervFeatures::default()
    }

    /// Get recommendations
    fn get_recommendations() -> HypervRecommendations {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            let eax: u32;
            let _ebx: u32;
            let _ecx: u32;
            let _edx: u32;

            core::arch::asm!(
                "push rbx",
                "mov eax, {leaf:e}",
                "cpuid",
                "mov {ebx_out:e}, ebx",
                "pop rbx",
                leaf = in(reg) cpuid::HYPERV_CPUID_RECOMMENDATIONS,
                ebx_out = out(reg) _ebx,
                out("eax") eax,
                out("ecx") _ecx,
                out("edx") _edx,
                options(nostack)
            );

            HypervRecommendations {
                use_hypercall_address_switch: eax & (1 << 0) != 0,
                use_hypercall_local_flush: eax & (1 << 1) != 0,
                use_hypercall_remote_flush: eax & (1 << 2) != 0,
                use_msr_apic: eax & (1 << 3) != 0,
                use_msr_reset: eax & (1 << 4) != 0,
                relaxed_timing: eax & (1 << 5) != 0,
                use_dma_remapping: eax & (1 << 6) != 0,
                use_interrupt_remapping: eax & (1 << 7) != 0,
                use_x2apic_msrs: eax & (1 << 8) != 0,
                deprecate_autoeoi: eax & (1 << 9) != 0,
                use_synic_autoeoi: eax & (1 << 10) != 0,
                use_relaxed_timing: eax & (1 << 11) != 0,
                use_long_spin_wait: eax & (1 << 18) != 0,
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        HypervRecommendations::default()
    }

    /// Read MSR
    #[cfg(target_arch = "x86_64")]
    unsafe fn read_msr(msr: u32) -> u64 {
        let lo: u32;
        let hi: u32;
        core::arch::asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") lo,
            out("edx") hi,
            options(nostack, nomem)
        );
        ((hi as u64) << 32) | (lo as u64)
    }

    /// Write MSR
    #[cfg(target_arch = "x86_64")]
    unsafe fn write_msr(msr: u32, value: u64) {
        let lo = value as u32;
        let hi = (value >> 32) as u32;
        core::arch::asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") lo,
            in("edx") hi,
            options(nostack, nomem)
        );
    }

    /// Initialize Hyper-V guest support
    pub fn init(&mut self) -> Result<(), &'static str> {
        if !Self::detect() {
            self.state = HypervState::NotDetected;
            return Err("Not running under Hyper-V");
        }

        self.state = HypervState::Detected;

        // Get version
        self.version = Self::get_version();

        // Get features
        self.features = Self::get_features();

        // Get recommendations
        self.recommendations = Self::get_recommendations();

        self.state = HypervState::Initializing;

        // Set guest OS ID
        #[cfg(target_arch = "x86_64")]
        unsafe {
            // Format: vendor(8) | os_id(8) | major(8) | minor(8) | sp(8) | build(24)
            let os_id: u64 = (0x01 << 56) |  // Open source
                            (0x01 << 48) |   // Linux-like
                            (0x01 << 40) |   // Major version
                            (0x00 << 32) |   // Minor version
                            (0x00 << 24) |   // Service pack
                            0x000001;        // Build
            Self::write_msr(msr::HV_X64_MSR_GUEST_OS_ID, os_id);
        }

        // Set up hypercall page
        if self.features.hypercall_msr {
            #[cfg(target_arch = "x86_64")]
            if let Some(frame) = crate::mm::alloc_frame() {
                let page_addr = frame.start_address().as_u64();
                self.hypercall_page = page_addr;

                unsafe {
                    // Enable hypercall page
                    let hypercall_msr = page_addr | 1;
                    Self::write_msr(msr::HV_X64_MSR_HYPERCALL, hypercall_msr);
                }
            }
        }

        // Enable SynIC if available
        if self.features.synic {
            #[cfg(target_arch = "x86_64")]
            unsafe {
                // Enable SynIC
                let scontrol = Self::read_msr(msr::HV_X64_MSR_SCONTROL);
                Self::write_msr(msr::HV_X64_MSR_SCONTROL, scontrol | 1);
                self.synic_enabled = true;
            }
        }

        self.state = HypervState::Running;
        self.initialized.store(true, Ordering::Release);

        if let Some(ver) = &self.version {
            crate::kprintln!("hyperv: Initialized, version {}.{}.{}",
                ver.major, ver.minor, ver.build);
        } else {
            crate::kprintln!("hyperv: Initialized");
        }

        Ok(())
    }

    /// Execute hypercall
    pub fn hypercall(&self, code: HypercallCode, input: u64, output: u64) -> HypercallStatus {
        if self.hypercall_page == 0 {
            return HypercallStatus::InvalidHypercallCode;
        }

        self.stats.hypercalls.fetch_add(1, Ordering::Relaxed);

        #[cfg(target_arch = "x86_64")]
        unsafe {
            let control = (code as u64) & 0xFFFF;
            let result: u64;

            // Call through hypercall page
            let hypercall_fn: extern "C" fn(u64, u64, u64) -> u64 =
                core::mem::transmute(self.hypercall_page as *const ());

            result = hypercall_fn(control, input, output);

            let status = (result & 0xFFFF) as u16;
            match status {
                0x0000 => HypercallStatus::Success,
                0x0002 => HypercallStatus::InvalidHypercallCode,
                0x0003 => HypercallStatus::InvalidHypercallInput,
                0x0004 => HypercallStatus::InvalidAlignment,
                0x0005 => HypercallStatus::InvalidParameter,
                0x0006 => HypercallStatus::AccessDenied,
                0x000B => HypercallStatus::InsufficientMemory,
                0x001D => HypercallStatus::NoResources,
                _ => HypercallStatus::InvalidHypercallCode,
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        HypercallStatus::InvalidHypercallCode
    }

    /// Get reference time count
    pub fn get_time_ref_count(&self) -> u64 {
        if !self.features.time_ref_count {
            return 0;
        }

        #[cfg(target_arch = "x86_64")]
        unsafe {
            Self::read_msr(msr::HV_X64_MSR_TIME_REF_COUNT)
        }

        #[cfg(not(target_arch = "x86_64"))]
        0
    }

    /// Get VP (Virtual Processor) index
    pub fn get_vp_index(&self) -> u32 {
        if !self.features.vp_index_msr {
            return 0;
        }

        #[cfg(target_arch = "x86_64")]
        unsafe {
            Self::read_msr(msr::HV_X64_MSR_VP_INDEX) as u32
        }

        #[cfg(not(target_arch = "x86_64"))]
        0
    }

    /// Notify long spin wait
    pub fn notify_long_spin_wait(&self) {
        if self.recommendations.use_long_spin_wait {
            let _ = self.hypercall(HypercallCode::NotifyLongSpinWait, 0, 0);
        }
    }

    /// Get version info
    pub fn version(&self) -> Option<&HypervVersion> {
        self.version.as_ref()
    }

    /// Get features
    pub fn features(&self) -> &HypervFeatures {
        &self.features
    }

    /// Get recommendations
    pub fn recommendations(&self) -> &HypervRecommendations {
        &self.recommendations
    }

    /// Get current state
    pub fn state(&self) -> HypervState {
        self.state
    }

    /// Get statistics
    pub fn stats(&self) -> &HypervStats {
        &self.stats
    }

    /// Is SynIC enabled?
    pub fn synic_enabled(&self) -> bool {
        self.synic_enabled
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        if let Some(ver) = &self.version {
            alloc::format!(
                "Hyper-V: v{}.{}.{} state={:?} synic={}",
                ver.major, ver.minor, ver.build,
                self.state, self.synic_enabled
            )
        } else {
            alloc::format!("Hyper-V: state={:?}", self.state)
        }
    }
}

impl Default for HypervGuest {
    fn default() -> Self {
        Self::new()
    }
}

// Global Hyper-V manager
static HYPERV_GUEST: IrqSafeMutex<Option<HypervGuest>> = IrqSafeMutex::new(None);

/// Initialize Hyper-V support
pub fn init() {
    let mut guest = HypervGuest::new();
    let result = guest.init();

    let status = guest.format_status();
    *HYPERV_GUEST.lock() = Some(guest);

    match result {
        Ok(_) => crate::kprintln!("{}", status),
        Err(_) => crate::kprintln!("hyperv: Not detected (not running under Hyper-V)"),
    }
}

/// Check if running under Hyper-V
pub fn is_hyperv() -> bool {
    HYPERV_GUEST.lock()
        .as_ref()
        .map(|g| g.state != HypervState::NotDetected)
        .unwrap_or(false)
}

/// Get status string
pub fn status() -> String {
    HYPERV_GUEST.lock()
        .as_ref()
        .map(|g| g.format_status())
        .unwrap_or_else(|| "Hyper-V not initialized".into())
}

/// Get time reference count
pub fn get_time_ref_count() -> u64 {
    HYPERV_GUEST.lock()
        .as_ref()
        .map(|g| g.get_time_ref_count())
        .unwrap_or(0)
}
