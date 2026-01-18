//! UEFI Runtime Services
//!
//! Provides access to UEFI runtime services after ExitBootServices():
//! - GetTime / SetTime
//! - GetVariable / SetVariable / GetNextVariableName
//! - ResetSystem
//! - GetWakeupTime / SetWakeupTime
//! - UpdateCapsule / QueryCapsuleCapabilities

#![allow(dead_code)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::mm;
use crate::sync::IrqSafeMutex;

/// UEFI status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum EfiStatus {
    Success = 0,
    LoadError = 1,
    InvalidParameter = 2,
    Unsupported = 3,
    BadBufferSize = 4,
    BufferTooSmall = 5,
    NotReady = 6,
    DeviceError = 7,
    WriteProtected = 8,
    OutOfResources = 9,
    VolumeCorrupted = 10,
    VolumeFull = 11,
    NoMedia = 12,
    MediaChanged = 13,
    NotFound = 14,
    AccessDenied = 15,
    NoResponse = 16,
    NoMapping = 17,
    Timeout = 18,
    NotStarted = 19,
    AlreadyStarted = 20,
    Aborted = 21,
    SecurityViolation = 26,
    // High bit set = error
    Unknown = 0x8000_0000_0000_0000,
}

impl EfiStatus {
    pub fn is_error(&self) -> bool {
        (*self as u64) & 0x8000_0000_0000_0000 != 0
    }

    pub fn from_raw(raw: u64) -> Self {
        match raw {
            0 => EfiStatus::Success,
            1 => EfiStatus::LoadError,
            2 => EfiStatus::InvalidParameter,
            3 => EfiStatus::Unsupported,
            4 => EfiStatus::BadBufferSize,
            5 => EfiStatus::BufferTooSmall,
            6 => EfiStatus::NotReady,
            7 => EfiStatus::DeviceError,
            8 => EfiStatus::WriteProtected,
            9 => EfiStatus::OutOfResources,
            14 => EfiStatus::NotFound,
            15 => EfiStatus::AccessDenied,
            18 => EfiStatus::Timeout,
            26 => EfiStatus::SecurityViolation,
            _ => EfiStatus::Unknown,
        }
    }
}

/// UEFI reset type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ResetType {
    Cold = 0,
    Warm = 1,
    Shutdown = 2,
    PlatformSpecific = 3,
}

/// EFI Time structure
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct EfiTime {
    pub year: u16,       // 1900 - 9999
    pub month: u8,       // 1 - 12
    pub day: u8,         // 1 - 31
    pub hour: u8,        // 0 - 23
    pub minute: u8,      // 0 - 59
    pub second: u8,      // 0 - 59
    pub pad1: u8,
    pub nanosecond: u32, // 0 - 999,999,999
    pub time_zone: i16,  // -1440 to 1440 or 2047
    pub daylight: u8,
    pub pad2: u8,
}

/// EFI Time Capabilities
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct EfiTimeCapabilities {
    pub resolution: u32,    // 1e-6 parts per million
    pub accuracy: u32,      // hertz
    pub sets_to_zero: bool, // true if time is set to zero on reset
}

/// Variable attributes
pub mod var_attr {
    pub const NON_VOLATILE: u32 = 0x00000001;
    pub const BOOTSERVICE_ACCESS: u32 = 0x00000002;
    pub const RUNTIME_ACCESS: u32 = 0x00000004;
    pub const HARDWARE_ERROR_RECORD: u32 = 0x00000008;
    pub const AUTHENTICATED_WRITE_ACCESS: u32 = 0x00000010;
    pub const TIME_BASED_AUTHENTICATED_WRITE_ACCESS: u32 = 0x00000020;
    pub const APPEND_WRITE: u32 = 0x00000040;
    pub const ENHANCED_AUTHENTICATED_ACCESS: u32 = 0x00000080;
}

/// Well-known EFI GUIDs
pub mod guids {
    use super::EfiGuid;

    pub const EFI_GLOBAL_VARIABLE: EfiGuid = EfiGuid {
        data1: 0x8BE4DF61,
        data2: 0x93CA,
        data3: 0x11D2,
        data4: [0xAA, 0x0D, 0x00, 0xE0, 0x98, 0x03, 0x2B, 0x8C],
    };

    pub const EFI_RUNTIME_SERVICES_TABLE: EfiGuid = EfiGuid {
        data1: 0x8868E871,
        data2: 0xE4F1,
        data3: 0x11D3,
        data4: [0xBC, 0x22, 0x00, 0x80, 0xC7, 0x3C, 0x88, 0x81],
    };

    pub const EFI_ACPI_TABLE: EfiGuid = EfiGuid {
        data1: 0x8868E871,
        data2: 0xE4F1,
        data3: 0x11D3,
        data4: [0xBC, 0x22, 0x00, 0x80, 0xC7, 0x3C, 0x88, 0x81],
    };
}

/// EFI GUID
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EfiGuid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl EfiGuid {
    pub const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        Self { data1, data2, data3, data4 }
    }

    pub fn to_bytes(&self) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&self.data1.to_le_bytes());
        bytes[4..6].copy_from_slice(&self.data2.to_le_bytes());
        bytes[6..8].copy_from_slice(&self.data3.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.data4);
        bytes
    }
}

/// EFI Runtime Services Table
#[repr(C)]
pub struct EfiRuntimeServices {
    pub hdr: EfiTableHeader,
    pub get_time: u64,
    pub set_time: u64,
    pub get_wakeup_time: u64,
    pub set_wakeup_time: u64,
    pub set_virtual_address_map: u64,
    pub convert_pointer: u64,
    pub get_variable: u64,
    pub get_next_variable_name: u64,
    pub set_variable: u64,
    pub get_next_high_mono_count: u64,
    pub reset_system: u64,
    pub update_capsule: u64,
    pub query_capsule_capabilities: u64,
    pub query_variable_info: u64,
}

/// EFI Table Header
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct EfiTableHeader {
    pub signature: u64,
    pub revision: u32,
    pub header_size: u32,
    pub crc32: u32,
    pub reserved: u32,
}

/// Runtime services signature
pub const EFI_RUNTIME_SERVICES_SIGNATURE: u64 = 0x56524553544E5552; // "RUNTSERV"

/// UEFI Runtime Services Manager
pub struct UefiRuntimeManager {
    /// Physical address of runtime services table
    runtime_services_phys: u64,
    /// Virtual address of runtime services table (after SetVirtualAddressMap)
    runtime_services_virt: u64,
    /// Whether virtual address map has been set
    virtual_mode: AtomicBool,
    /// Whether runtime services are available
    available: AtomicBool,
    /// Cached time for when RTC is unavailable
    cached_time: EfiTime,
}

impl UefiRuntimeManager {
    pub const fn new() -> Self {
        Self {
            runtime_services_phys: 0,
            runtime_services_virt: 0,
            virtual_mode: AtomicBool::new(false),
            available: AtomicBool::new(false),
            cached_time: EfiTime {
                year: 2026,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0,
                pad1: 0,
                nanosecond: 0,
                time_zone: 0,
                daylight: 0,
                pad2: 0,
            },
        }
    }

    /// Initialize with runtime services table address
    pub fn init(&mut self, runtime_services_addr: u64) -> Result<(), EfiStatus> {
        if runtime_services_addr == 0 {
            crate::kprintln!("uefi_runtime: no runtime services table provided");
            return Err(EfiStatus::NotFound);
        }

        self.runtime_services_phys = runtime_services_addr;

        // Map and validate the table
        let virt = mm::phys_to_virt(x86_64::PhysAddr::new(runtime_services_addr));
        self.runtime_services_virt = virt.as_u64();

        // Read and validate header
        let header = unsafe {
            core::ptr::read_volatile(virt.as_ptr::<EfiTableHeader>())
        };

        if header.signature != EFI_RUNTIME_SERVICES_SIGNATURE {
            crate::kprintln!("uefi_runtime: invalid signature {:#x}", header.signature);
            return Err(EfiStatus::InvalidParameter);
        }

        self.available.store(true, Ordering::Release);

        crate::kprintln!(
            "uefi_runtime: initialized, revision {}.{}",
            header.revision >> 16,
            header.revision & 0xFFFF
        );

        Ok(())
    }

    /// Check if runtime services are available
    pub fn is_available(&self) -> bool {
        self.available.load(Ordering::Acquire)
    }

    /// Get the runtime services table pointer
    fn get_rt(&self) -> Option<&EfiRuntimeServices> {
        if !self.is_available() {
            return None;
        }

        let addr = if self.virtual_mode.load(Ordering::Acquire) {
            self.runtime_services_virt
        } else {
            mm::phys_to_virt(x86_64::PhysAddr::new(self.runtime_services_phys)).as_u64()
        };

        Some(unsafe { &*(addr as *const EfiRuntimeServices) })
    }

    /// Get current time from UEFI
    pub fn get_time(&self) -> Result<EfiTime, EfiStatus> {
        let rt = self.get_rt().ok_or(EfiStatus::NotReady)?;

        if rt.get_time == 0 {
            return Err(EfiStatus::Unsupported);
        }

        let mut time = EfiTime::default();
        let mut capabilities = EfiTimeCapabilities::default();

        // Call UEFI GetTime
        let status = unsafe {
            let get_time: extern "efiapi" fn(*mut EfiTime, *mut EfiTimeCapabilities) -> u64 =
                core::mem::transmute(rt.get_time);
            get_time(&mut time, &mut capabilities)
        };

        let status = EfiStatus::from_raw(status);
        if status != EfiStatus::Success {
            return Err(status);
        }

        Ok(time)
    }

    /// Set current time via UEFI
    pub fn set_time(&self, time: &EfiTime) -> Result<(), EfiStatus> {
        let rt = self.get_rt().ok_or(EfiStatus::NotReady)?;

        if rt.set_time == 0 {
            return Err(EfiStatus::Unsupported);
        }

        let status = unsafe {
            let set_time: extern "efiapi" fn(*const EfiTime) -> u64 =
                core::mem::transmute(rt.set_time);
            set_time(time)
        };

        let status = EfiStatus::from_raw(status);
        if status != EfiStatus::Success {
            return Err(status);
        }

        Ok(())
    }

    /// Get wakeup time
    pub fn get_wakeup_time(&self) -> Result<(bool, bool, EfiTime), EfiStatus> {
        let rt = self.get_rt().ok_or(EfiStatus::NotReady)?;

        if rt.get_wakeup_time == 0 {
            return Err(EfiStatus::Unsupported);
        }

        let mut enabled: u8 = 0;
        let mut pending: u8 = 0;
        let mut time = EfiTime::default();

        let status = unsafe {
            let get_wakeup_time: extern "efiapi" fn(*mut u8, *mut u8, *mut EfiTime) -> u64 =
                core::mem::transmute(rt.get_wakeup_time);
            get_wakeup_time(&mut enabled, &mut pending, &mut time)
        };

        let status = EfiStatus::from_raw(status);
        if status != EfiStatus::Success {
            return Err(status);
        }

        Ok((enabled != 0, pending != 0, time))
    }

    /// Set wakeup time
    pub fn set_wakeup_time(&self, enable: bool, time: Option<&EfiTime>) -> Result<(), EfiStatus> {
        let rt = self.get_rt().ok_or(EfiStatus::NotReady)?;

        if rt.set_wakeup_time == 0 {
            return Err(EfiStatus::Unsupported);
        }

        let time_ptr = time.map(|t| t as *const EfiTime).unwrap_or(core::ptr::null());

        let status = unsafe {
            let set_wakeup_time: extern "efiapi" fn(u8, *const EfiTime) -> u64 =
                core::mem::transmute(rt.set_wakeup_time);
            set_wakeup_time(if enable { 1 } else { 0 }, time_ptr)
        };

        let status = EfiStatus::from_raw(status);
        if status != EfiStatus::Success {
            return Err(status);
        }

        Ok(())
    }

    /// Get a UEFI variable
    pub fn get_variable(
        &self,
        name: &[u16],
        guid: &EfiGuid,
        data: &mut [u8],
    ) -> Result<(u32, usize), EfiStatus> {
        let rt = self.get_rt().ok_or(EfiStatus::NotReady)?;

        if rt.get_variable == 0 {
            return Err(EfiStatus::Unsupported);
        }

        let mut attributes: u32 = 0;
        let mut data_size: u64 = data.len() as u64;

        let status = unsafe {
            let get_variable: extern "efiapi" fn(
                *const u16,
                *const EfiGuid,
                *mut u32,
                *mut u64,
                *mut u8,
            ) -> u64 = core::mem::transmute(rt.get_variable);
            get_variable(
                name.as_ptr(),
                guid,
                &mut attributes,
                &mut data_size,
                data.as_mut_ptr(),
            )
        };

        let status = EfiStatus::from_raw(status);
        if status != EfiStatus::Success {
            return Err(status);
        }

        Ok((attributes, data_size as usize))
    }

    /// Set a UEFI variable
    pub fn set_variable(
        &self,
        name: &[u16],
        guid: &EfiGuid,
        attributes: u32,
        data: &[u8],
    ) -> Result<(), EfiStatus> {
        let rt = self.get_rt().ok_or(EfiStatus::NotReady)?;

        if rt.set_variable == 0 {
            return Err(EfiStatus::Unsupported);
        }

        let status = unsafe {
            let set_variable: extern "efiapi" fn(
                *const u16,
                *const EfiGuid,
                u32,
                u64,
                *const u8,
            ) -> u64 = core::mem::transmute(rt.set_variable);
            set_variable(
                name.as_ptr(),
                guid,
                attributes,
                data.len() as u64,
                data.as_ptr(),
            )
        };

        let status = EfiStatus::from_raw(status);
        if status != EfiStatus::Success {
            return Err(status);
        }

        Ok(())
    }

    /// Get next variable name (for enumeration)
    pub fn get_next_variable_name(
        &self,
        name_size: &mut usize,
        name: &mut [u16],
        guid: &mut EfiGuid,
    ) -> Result<(), EfiStatus> {
        let rt = self.get_rt().ok_or(EfiStatus::NotReady)?;

        if rt.get_next_variable_name == 0 {
            return Err(EfiStatus::Unsupported);
        }

        let mut size = *name_size as u64;

        let status = unsafe {
            let get_next_variable_name: extern "efiapi" fn(
                *mut u64,
                *mut u16,
                *mut EfiGuid,
            ) -> u64 = core::mem::transmute(rt.get_next_variable_name);
            get_next_variable_name(&mut size, name.as_mut_ptr(), guid)
        };

        *name_size = size as usize;

        let status = EfiStatus::from_raw(status);
        if status != EfiStatus::Success {
            return Err(status);
        }

        Ok(())
    }

    /// Query variable storage info
    pub fn query_variable_info(&self, attributes: u32) -> Result<(u64, u64, u64), EfiStatus> {
        let rt = self.get_rt().ok_or(EfiStatus::NotReady)?;

        if rt.query_variable_info == 0 {
            return Err(EfiStatus::Unsupported);
        }

        let mut max_storage: u64 = 0;
        let mut remaining_storage: u64 = 0;
        let mut max_variable_size: u64 = 0;

        let status = unsafe {
            let query_variable_info: extern "efiapi" fn(
                u32,
                *mut u64,
                *mut u64,
                *mut u64,
            ) -> u64 = core::mem::transmute(rt.query_variable_info);
            query_variable_info(
                attributes,
                &mut max_storage,
                &mut remaining_storage,
                &mut max_variable_size,
            )
        };

        let status = EfiStatus::from_raw(status);
        if status != EfiStatus::Success {
            return Err(status);
        }

        Ok((max_storage, remaining_storage, max_variable_size))
    }

    /// Reset the system
    pub fn reset_system(&self, reset_type: ResetType, status: EfiStatus, data: Option<&[u8]>) -> ! {
        if let Some(rt) = self.get_rt() {
            if rt.reset_system != 0 {
                let (data_ptr, data_size) = match data {
                    Some(d) => (d.as_ptr(), d.len() as u64),
                    None => (core::ptr::null(), 0),
                };

                unsafe {
                    let reset_system: extern "efiapi" fn(u32, u64, u64, *const u8) -> ! =
                        core::mem::transmute(rt.reset_system);
                    reset_system(reset_type as u32, status as u64, data_size, data_ptr);
                }
            }
        }

        // Fallback: triple fault
        crate::kprintln!("uefi_runtime: reset failed, using fallback");
        unsafe {
            // Write to ACPI reset port (0xCF9)
            x86_64::instructions::port::Port::<u8>::new(0xCF9).write(0x0E);
        }

        loop {
            x86_64::instructions::hlt();
        }
    }

    /// Set virtual address map (called during early boot)
    pub fn set_virtual_address_map(
        &mut self,
        memory_map_size: usize,
        descriptor_size: usize,
        descriptor_version: u32,
        virtual_map: *mut u8,
    ) -> Result<(), EfiStatus> {
        let rt = self.get_rt().ok_or(EfiStatus::NotReady)?;

        if rt.set_virtual_address_map == 0 {
            return Err(EfiStatus::Unsupported);
        }

        let status = unsafe {
            let set_virtual_address_map: extern "efiapi" fn(
                u64,
                u64,
                u32,
                *mut u8,
            ) -> u64 = core::mem::transmute(rt.set_virtual_address_map);
            set_virtual_address_map(
                memory_map_size as u64,
                descriptor_size as u64,
                descriptor_version,
                virtual_map,
            )
        };

        let status = EfiStatus::from_raw(status);
        if status != EfiStatus::Success {
            return Err(status);
        }

        self.virtual_mode.store(true, Ordering::Release);
        Ok(())
    }

    /// Format status as string
    pub fn format_status(&self) -> String {
        let mut output = String::new();

        output.push_str("UEFI Runtime Services:\n");
        output.push_str(&alloc::format!(
            "  Available: {}\n",
            if self.is_available() { "Yes" } else { "No" }
        ));
        output.push_str(&alloc::format!(
            "  Virtual Mode: {}\n",
            if self.virtual_mode.load(Ordering::Relaxed) { "Yes" } else { "No" }
        ));

        if self.is_available() {
            if let Some(rt) = self.get_rt() {
                output.push_str(&alloc::format!(
                    "  Revision: {}.{}\n",
                    rt.hdr.revision >> 16,
                    rt.hdr.revision & 0xFFFF
                ));
                output.push_str("  Services:\n");
                output.push_str(&alloc::format!("    GetTime: {}\n", if rt.get_time != 0 { "Yes" } else { "No" }));
                output.push_str(&alloc::format!("    SetTime: {}\n", if rt.set_time != 0 { "Yes" } else { "No" }));
                output.push_str(&alloc::format!("    GetVariable: {}\n", if rt.get_variable != 0 { "Yes" } else { "No" }));
                output.push_str(&alloc::format!("    SetVariable: {}\n", if rt.set_variable != 0 { "Yes" } else { "No" }));
                output.push_str(&alloc::format!("    ResetSystem: {}\n", if rt.reset_system != 0 { "Yes" } else { "No" }));
            }
        }

        output
    }
}

// =============================================================================
// Global State
// =============================================================================

static UEFI_RUNTIME: IrqSafeMutex<UefiRuntimeManager> = IrqSafeMutex::new(UefiRuntimeManager::new());

/// Initialize UEFI runtime services
pub fn init(runtime_services_addr: u64) {
    let mut mgr = UEFI_RUNTIME.lock();
    match mgr.init(runtime_services_addr) {
        Ok(()) => {}
        Err(e) => {
            crate::kprintln!("uefi_runtime: initialization failed: {:?}", e);
        }
    }
}

/// Check if runtime services are available
pub fn is_available() -> bool {
    UEFI_RUNTIME.lock().is_available()
}

/// Get current time
pub fn get_time() -> Result<EfiTime, EfiStatus> {
    UEFI_RUNTIME.lock().get_time()
}

/// Set current time
pub fn set_time(time: &EfiTime) -> Result<(), EfiStatus> {
    UEFI_RUNTIME.lock().set_time(time)
}

/// Get a UEFI variable
pub fn get_variable(name: &[u16], guid: &EfiGuid, data: &mut [u8]) -> Result<(u32, usize), EfiStatus> {
    UEFI_RUNTIME.lock().get_variable(name, guid, data)
}

/// Set a UEFI variable
pub fn set_variable(name: &[u16], guid: &EfiGuid, attributes: u32, data: &[u8]) -> Result<(), EfiStatus> {
    UEFI_RUNTIME.lock().set_variable(name, guid, attributes, data)
}

/// Reset the system
pub fn reset_system(reset_type: ResetType) -> ! {
    UEFI_RUNTIME.lock().reset_system(reset_type, EfiStatus::Success, None)
}

/// Shutdown the system
pub fn shutdown() -> ! {
    reset_system(ResetType::Shutdown)
}

/// Reboot the system
pub fn reboot() -> ! {
    reset_system(ResetType::Cold)
}

/// Format status
pub fn format_status() -> String {
    UEFI_RUNTIME.lock().format_status()
}
