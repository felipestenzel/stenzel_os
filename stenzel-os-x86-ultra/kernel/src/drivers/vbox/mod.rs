//! VirtualBox Guest Additions
//!
//! Provides VirtualBox guest integration features.

#![allow(dead_code)]

pub mod vboxguest;
pub mod vboxsf;
pub mod vboxvideo;
pub mod seamless;

use alloc::string::String;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;

/// VirtualBox PCI IDs
pub const VBOX_VENDOR_ID: u16 = 0x80EE;
pub const VBOX_DEVICE_ID: u16 = 0xCAFE; // VMMDev
pub const VBOX_VIDEO_DEVICE_ID: u16 = 0xBEEF;

/// VMMDev request types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmmDevRequestType {
    GetMouseStatus = 1,
    SetMouseStatus = 2,
    SetPointerShape = 3,
    GetHostVersion = 4,
    Idle = 5,
    GetHostTime = 10,
    GetStatisticsChangeRequest = 11,
    GetHypervisorInfo = 20,
    SetHypervisorInfo = 21,
    PowerStateChange = 31,
    ReportGuestInfo = 50,
    ReportGuestStatus = 54,
    ReportGuestCapabilities = 55,
    SetGuestCapabilities = 56,
    CtlGuestFilterMask = 57,
    ReportGuestInfo2 = 58,
    GetSessionId = 60,
    VideoAccelEnable = 70,
    VideoAccelFlush = 71,
    VideoSetVisibleRegion = 72,
    GetSeamlessChangeRequest = 73,
    SharedFolderConnect = 80,
    SharedFolderDisconnect = 81,
    SharedFolderReadLink = 82,
    SharedFolderInformation = 83,
    SharedFolderCreateSymlink = 84,
    VideoModeSupported = 100,
    GetHeightReduction = 101,
    GetDisplayChangeRequest = 102,
    SetBalloonSize = 110,
    ClipboardConnect = 130,
    ClipboardDisconnect = 131,
    ClipboardSend = 132,
    ClipboardRecv = 133,
    GetVrdpChangeRequest = 170,
}

/// VMMDev request header
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VmmDevRequestHeader {
    pub size: u32,
    pub version: u32,
    pub request_type: u32,
    pub rc: i32,
    pub reserved1: u32,
    pub reserved2: u32,
}

impl VmmDevRequestHeader {
    pub const VERSION: u32 = 0x10001;

    pub fn new(request_type: VmmDevRequestType, size: u32) -> Self {
        Self {
            size,
            version: Self::VERSION,
            request_type: request_type as u32,
            rc: 0,
            reserved1: 0,
            reserved2: 0,
        }
    }
}

/// Guest info report
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VmmDevReportGuestInfo {
    pub header: VmmDevRequestHeader,
    pub interface_version: u32,
    pub os_type: u32,
}

/// Guest info2 report
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VmmDevReportGuestInfo2 {
    pub header: VmmDevRequestHeader,
    pub major: u16,
    pub minor: u16,
    pub build: u32,
    pub revision: u32,
    pub features: u32,
    pub name: [u8; 128],
}

impl Default for VmmDevReportGuestInfo2 {
    fn default() -> Self {
        Self {
            header: VmmDevRequestHeader::default(),
            major: 0,
            minor: 0,
            build: 0,
            revision: 0,
            features: 0,
            name: [0; 128],
        }
    }
}

/// Guest capabilities
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum GuestCaps {
    SeamlessMode = 1 << 0,
    GuestControl = 1 << 1,
    Graphics = 1 << 2,
    AutoLogon = 1 << 3,
    SharedClipboard = 1 << 4,
}

/// Host version request
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VmmDevHostVersion {
    pub header: VmmDevRequestHeader,
    pub major: u16,
    pub minor: u16,
    pub build: u32,
    pub revision: u32,
    pub features: u32,
}

/// Mouse status
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VmmDevMouseStatus {
    pub header: VmmDevRequestHeader,
    pub features: u32,
    pub x: i32,
    pub y: i32,
}

/// Display change request
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VmmDevDisplayChange {
    pub header: VmmDevRequestHeader,
    pub xres: u32,
    pub yres: u32,
    pub bpp: u32,
    pub display: u32,
    pub origin_x: i32,
    pub origin_y: i32,
    pub enabled: u32,
    pub changed: u32,
}

/// VirtualBox guest state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VboxState {
    NotDetected,
    Detected,
    Initializing,
    Running,
    Error,
}

/// VirtualBox features
#[derive(Debug, Clone, Copy, Default)]
pub struct VboxFeatures {
    pub mouse_integration: bool,
    pub shared_folders: bool,
    pub seamless_mode: bool,
    pub shared_clipboard: bool,
    pub guest_control: bool,
    pub video_accel: bool,
    pub auto_resize: bool,
}

/// VirtualBox statistics
#[derive(Debug, Default)]
pub struct VboxStats {
    pub requests_sent: AtomicU64,
    pub requests_completed: AtomicU64,
    pub events_received: AtomicU64,
    pub errors: AtomicU64,
}

/// VirtualBox Guest Additions manager
pub struct VboxGuest {
    /// VMMDev MMIO base
    mmio_base: u64,
    /// I/O port base
    io_port: u16,
    /// IRQ number
    irq: u8,
    /// Current state
    state: VboxState,
    /// Host version
    host_version: Option<VmmDevHostVersion>,
    /// Features
    features: VboxFeatures,
    /// Request counter
    request_counter: u32,
    /// Initialized flag
    initialized: AtomicBool,
    /// Statistics
    stats: VboxStats,
}

impl VboxGuest {
    /// VMMDev memory region size
    const VMMDEV_RAM_SIZE: u64 = 4096;

    /// Create new VirtualBox guest manager
    pub fn new() -> Self {
        Self {
            mmio_base: 0,
            io_port: 0,
            irq: 0,
            state: VboxState::NotDetected,
            host_version: None,
            features: VboxFeatures::default(),
            request_counter: 0,
            initialized: AtomicBool::new(false),
            stats: VboxStats::default(),
        }
    }

    /// Configure from PCI device
    pub fn configure(&mut self, mmio_base: u64, io_port: u16, irq: u8) {
        self.mmio_base = mmio_base;
        self.io_port = io_port;
        self.irq = irq;
        self.state = VboxState::Detected;
    }

    /// Write request to VMMDev
    fn send_request<T: Copy>(&mut self, request: &T) -> Result<(), &'static str> {
        if self.mmio_base == 0 {
            return Err("VMMDev not configured");
        }

        let size = core::mem::size_of::<T>();
        let src = request as *const T as *const u8;

        // Copy request to VMMDev memory
        unsafe {
            let dst = self.mmio_base as *mut u8;
            core::ptr::copy_nonoverlapping(src, dst, size);
        }

        // Trigger request by writing to port
        if self.io_port != 0 {
            unsafe {
                let phys_addr = self.mmio_base;
                core::arch::asm!(
                    "out dx, eax",
                    in("dx") self.io_port,
                    in("eax") phys_addr as u32,
                    options(nostack, nomem)
                );
            }
        }

        self.stats.requests_sent.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Read response from VMMDev
    fn read_response<T: Copy + Default>(&self) -> T {
        if self.mmio_base == 0 {
            return T::default();
        }

        unsafe {
            let src = self.mmio_base as *const T;
            core::ptr::read_volatile(src)
        }
    }

    /// Initialize VirtualBox additions
    pub fn init(&mut self) -> Result<(), &'static str> {
        if self.state != VboxState::Detected {
            return Err("VMMDev not detected");
        }

        self.state = VboxState::Initializing;

        // Get host version
        self.query_host_version()?;

        // Report guest info
        self.report_guest_info()?;

        // Report guest capabilities
        self.report_capabilities()?;

        // Enable features
        self.features.mouse_integration = true;
        self.features.shared_clipboard = true;
        self.features.seamless_mode = true;
        self.features.auto_resize = true;

        self.state = VboxState::Running;
        self.initialized.store(true, Ordering::Release);

        if let Some(ver) = &self.host_version {
            crate::kprintln!("vbox: Initialized, host version {}.{}.{}",
                ver.major, ver.minor, ver.build);
        }

        Ok(())
    }

    /// Query host version
    fn query_host_version(&mut self) -> Result<(), &'static str> {
        let request = VmmDevHostVersion {
            header: VmmDevRequestHeader::new(
                VmmDevRequestType::GetHostVersion,
                core::mem::size_of::<VmmDevHostVersion>() as u32
            ),
            ..Default::default()
        };

        self.send_request(&request)?;

        // Read response
        let response: VmmDevHostVersion = self.read_response();

        if response.header.rc == 0 {
            self.host_version = Some(response);
            self.stats.requests_completed.fetch_add(1, Ordering::Relaxed);
            Ok(())
        } else {
            self.stats.errors.fetch_add(1, Ordering::Relaxed);
            Err("Failed to get host version")
        }
    }

    /// Report guest info to host
    fn report_guest_info(&mut self) -> Result<(), &'static str> {
        let request = VmmDevReportGuestInfo {
            header: VmmDevRequestHeader::new(
                VmmDevRequestType::ReportGuestInfo,
                core::mem::size_of::<VmmDevReportGuestInfo>() as u32
            ),
            interface_version: 0x00010001, // Guest Additions version
            os_type: 0x00100, // Linux-like
        };

        self.send_request(&request)?;

        let response: VmmDevReportGuestInfo = self.read_response();

        if response.header.rc == 0 {
            self.stats.requests_completed.fetch_add(1, Ordering::Relaxed);
            Ok(())
        } else {
            Err("Failed to report guest info")
        }
    }

    /// Report guest capabilities
    fn report_capabilities(&mut self) -> Result<(), &'static str> {
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct CapRequest {
            header: VmmDevRequestHeader,
            caps: u32,
        }

        let request = CapRequest {
            header: VmmDevRequestHeader::new(
                VmmDevRequestType::ReportGuestCapabilities,
                core::mem::size_of::<CapRequest>() as u32
            ),
            caps: GuestCaps::SeamlessMode as u32 |
                  GuestCaps::SharedClipboard as u32 |
                  GuestCaps::Graphics as u32,
        };

        self.send_request(&request)?;
        self.stats.requests_completed.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Get display change request
    pub fn get_display_change(&mut self) -> Option<VmmDevDisplayChange> {
        if !self.initialized.load(Ordering::Acquire) {
            return None;
        }

        let request = VmmDevDisplayChange {
            header: VmmDevRequestHeader::new(
                VmmDevRequestType::GetDisplayChangeRequest,
                core::mem::size_of::<VmmDevDisplayChange>() as u32
            ),
            ..Default::default()
        };

        if self.send_request(&request).is_err() {
            return None;
        }

        let response: VmmDevDisplayChange = self.read_response();

        if response.header.rc == 0 && response.changed != 0 {
            self.stats.events_received.fetch_add(1, Ordering::Relaxed);
            Some(response)
        } else {
            None
        }
    }

    /// Get mouse status
    pub fn get_mouse_status(&mut self) -> Option<(i32, i32)> {
        if !self.initialized.load(Ordering::Acquire) {
            return None;
        }

        let request = VmmDevMouseStatus {
            header: VmmDevRequestHeader::new(
                VmmDevRequestType::GetMouseStatus,
                core::mem::size_of::<VmmDevMouseStatus>() as u32
            ),
            ..Default::default()
        };

        if self.send_request(&request).is_err() {
            return None;
        }

        let response: VmmDevMouseStatus = self.read_response();

        if response.header.rc == 0 {
            Some((response.x, response.y))
        } else {
            None
        }
    }

    /// Set mouse integration
    pub fn set_mouse_integration(&mut self, enabled: bool) {
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct MouseCaps {
            header: VmmDevRequestHeader,
            features: u32,
        }

        let request = MouseCaps {
            header: VmmDevRequestHeader::new(
                VmmDevRequestType::SetMouseStatus,
                core::mem::size_of::<MouseCaps>() as u32
            ),
            features: if enabled { 0x01 } else { 0x00 },
        };

        let _ = self.send_request(&request);
        self.features.mouse_integration = enabled;
    }

    /// Get host version
    pub fn host_version(&self) -> Option<&VmmDevHostVersion> {
        self.host_version.as_ref()
    }

    /// Get features
    pub fn features(&self) -> &VboxFeatures {
        &self.features
    }

    /// Get current state
    pub fn state(&self) -> VboxState {
        self.state
    }

    /// Get statistics
    pub fn stats(&self) -> &VboxStats {
        &self.stats
    }

    /// Handle IRQ
    pub fn handle_interrupt(&mut self) {
        self.stats.events_received.fetch_add(1, Ordering::Relaxed);
        // Check for pending events
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        if let Some(ver) = &self.host_version {
            alloc::format!(
                "VirtualBox: v{}.{}.{} state={:?} mouse={} clipboard={}",
                ver.major, ver.minor, ver.build,
                self.state, self.features.mouse_integration,
                self.features.shared_clipboard
            )
        } else {
            alloc::format!("VirtualBox: state={:?}", self.state)
        }
    }
}

impl Default for VboxGuest {
    fn default() -> Self {
        Self::new()
    }
}

// Global VirtualBox manager
static VBOX_GUEST: IrqSafeMutex<Option<VboxGuest>> = IrqSafeMutex::new(None);

/// Initialize VirtualBox additions
pub fn init() {
    let mut guest = VboxGuest::new();

    // Would scan PCI for VMMDev here
    // For now, check if detected via other means
    // guest.configure(mmio_base, io_port, irq);

    let result = if guest.state == VboxState::Detected {
        guest.init()
    } else {
        Err("VirtualBox not detected")
    };

    let status = guest.format_status();
    *VBOX_GUEST.lock() = Some(guest);

    match result {
        Ok(_) => crate::kprintln!("{}", status),
        Err(_) => crate::kprintln!("vbox: Not detected (not running under VirtualBox)"),
    }
}

/// Check if running under VirtualBox
pub fn is_vbox() -> bool {
    VBOX_GUEST.lock()
        .as_ref()
        .map(|g| g.state != VboxState::NotDetected)
        .unwrap_or(false)
}

/// Get status string
pub fn status() -> String {
    VBOX_GUEST.lock()
        .as_ref()
        .map(|g| g.format_status())
        .unwrap_or_else(|| "VirtualBox not initialized".into())
}

/// Get display change
pub fn get_display_change() -> Option<VmmDevDisplayChange> {
    VBOX_GUEST.lock()
        .as_mut()
        .and_then(|g| g.get_display_change())
}
