//! UEFI Variables Module
//!
//! High-level interface for reading and writing UEFI variables.
//! Provides access to boot options, secure boot state, and custom variables.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use core::sync::atomic::{AtomicBool, Ordering};
use crate::sync::TicketSpinlock;

use super::uefi_runtime::{self, EfiGuid, EfiStatus};

/// Well-known UEFI GUIDs
pub mod guids {
    use super::EfiGuid;

    /// EFI Global Variable GUID: 8BE4DF61-93CA-11D2-AA0D-00E098032B8C
    pub const EFI_GLOBAL_VARIABLE: EfiGuid = EfiGuid {
        data1: 0x8BE4DF61,
        data2: 0x93CA,
        data3: 0x11D2,
        data4: [0xAA, 0x0D, 0x00, 0xE0, 0x98, 0x03, 0x2B, 0x8C],
    };

    /// EFI Image Security Database GUID: D719B2CB-3D3A-4596-A3BC-DAD00E67656F
    pub const EFI_IMAGE_SECURITY_DATABASE: EfiGuid = EfiGuid {
        data1: 0xD719B2CB,
        data2: 0x3D3A,
        data3: 0x4596,
        data4: [0xA3, 0xBC, 0xDA, 0xD0, 0x0E, 0x67, 0x65, 0x6F],
    };

    /// Microsoft Vendor GUID: 77FA9ABD-0359-4D32-BD60-28F4E78F784B
    pub const EFI_VENDOR_MS: EfiGuid = EfiGuid {
        data1: 0x77FA9ABD,
        data2: 0x0359,
        data3: 0x4D32,
        data4: [0xBD, 0x60, 0x28, 0xF4, 0xE7, 0x8F, 0x78, 0x4B],
    };

    /// Shell Environment Variable GUID
    pub const EFI_SHELL_VARIABLE: EfiGuid = EfiGuid {
        data1: 0x158DEF5A,
        data2: 0xF656,
        data3: 0x419C,
        data4: [0xB0, 0x27, 0x7A, 0x31, 0x92, 0xC0, 0x79, 0xD2],
    };

    /// Custom Stenzel OS GUID for our variables
    pub const STENZEL_OS_GUID: EfiGuid = EfiGuid {
        data1: 0x53544E5A,
        data2: 0x454C,
        data3: 0x4F53,
        data4: [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07],
    };
}

/// Variable attributes
pub mod attrs {
    /// Variable is non-volatile (persists across reboots)
    pub const NON_VOLATILE: u32 = 0x00000001;
    /// Variable is accessible during boot services
    pub const BOOTSERVICE_ACCESS: u32 = 0x00000002;
    /// Variable is accessible during runtime services
    pub const RUNTIME_ACCESS: u32 = 0x00000004;
    /// Hardware error record
    pub const HARDWARE_ERROR_RECORD: u32 = 0x00000008;
    /// Authenticated write access (deprecated)
    pub const AUTHENTICATED_WRITE_ACCESS: u32 = 0x00000010;
    /// Time-based authenticated write access
    pub const TIME_BASED_AUTHENTICATED_WRITE_ACCESS: u32 = 0x00000020;
    /// Append write
    pub const APPEND_WRITE: u32 = 0x00000040;
    /// Enhanced authenticated access
    pub const ENHANCED_AUTHENTICATED_ACCESS: u32 = 0x00000080;

    /// Common combination: Non-volatile, boot service, runtime access
    pub const NV_BS_RT: u32 = NON_VOLATILE | BOOTSERVICE_ACCESS | RUNTIME_ACCESS;
    /// Common combination: Boot service and runtime access (volatile)
    pub const BS_RT: u32 = BOOTSERVICE_ACCESS | RUNTIME_ACCESS;
}

/// Types of UEFI boot options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootOptionType {
    /// Unknown type
    Unknown,
    /// Hard drive boot
    HardDrive,
    /// CD-ROM boot
    CdRom,
    /// USB boot
    Usb,
    /// Network/PXE boot
    Network,
    /// Firmware volume
    FirmwareVolume,
    /// BIOS boot specification
    BbsBoot,
    /// File path
    FilePath,
}

/// UEFI Boot Option structure
#[derive(Debug, Clone)]
pub struct BootOption {
    /// Boot option number (Boot0000, Boot0001, etc.)
    pub number: u16,
    /// Boot option attributes
    pub attributes: u32,
    /// Description string
    pub description: String,
    /// Device path data
    pub device_path: Vec<u8>,
    /// Optional data
    pub optional_data: Vec<u8>,
    /// Boot option type
    pub option_type: BootOptionType,
}

impl BootOption {
    /// Attribute: Option is active
    pub const LOAD_OPTION_ACTIVE: u32 = 0x00000001;
    /// Attribute: Force reconnect on load
    pub const LOAD_OPTION_FORCE_RECONNECT: u32 = 0x00000002;
    /// Attribute: Hidden from boot menu
    pub const LOAD_OPTION_HIDDEN: u32 = 0x00000008;
    /// Attribute: Category - Boot
    pub const LOAD_OPTION_CATEGORY_BOOT: u32 = 0x00000000;
    /// Attribute: Category - Application
    pub const LOAD_OPTION_CATEGORY_APP: u32 = 0x00000100;

    /// Check if this boot option is active
    pub fn is_active(&self) -> bool {
        self.attributes & Self::LOAD_OPTION_ACTIVE != 0
    }

    /// Check if this boot option is hidden
    pub fn is_hidden(&self) -> bool {
        self.attributes & Self::LOAD_OPTION_HIDDEN != 0
    }

    /// Parse boot option from raw data
    pub fn from_bytes(number: u16, data: &[u8]) -> Option<Self> {
        if data.len() < 6 {
            return None;
        }

        // Parse attributes (4 bytes)
        let attributes = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        // Parse file path list length (2 bytes)
        let path_len = u16::from_le_bytes([data[4], data[5]]) as usize;

        // Parse description (null-terminated UCS-2 string)
        let mut desc_end = 6;
        while desc_end + 1 < data.len() {
            if data[desc_end] == 0 && data[desc_end + 1] == 0 {
                desc_end += 2;
                break;
            }
            desc_end += 2;
        }

        // Convert description from UCS-2 to String
        let desc_data = &data[6..desc_end];
        let mut description = String::new();
        for i in (0..desc_data.len() - 2).step_by(2) {
            let ch = u16::from_le_bytes([desc_data[i], desc_data[i + 1]]);
            if ch == 0 {
                break;
            }
            if let Some(c) = char::from_u32(ch as u32) {
                description.push(c);
            }
        }

        // Parse device path
        let path_start = desc_end;
        let path_end = path_start + path_len.min(data.len() - path_start);
        let device_path = data[path_start..path_end].to_vec();

        // Parse optional data
        let optional_data = if path_end < data.len() {
            data[path_end..].to_vec()
        } else {
            Vec::new()
        };

        // Determine boot option type from device path
        let option_type = Self::detect_type(&device_path);

        Some(Self {
            number,
            attributes,
            description,
            device_path,
            optional_data,
            option_type,
        })
    }

    /// Detect boot option type from device path
    fn detect_type(device_path: &[u8]) -> BootOptionType {
        if device_path.len() < 4 {
            return BootOptionType::Unknown;
        }

        let device_type = device_path[0];
        let sub_type = device_path[1];

        match (device_type, sub_type) {
            (0x01, 0x01) => BootOptionType::HardDrive,  // Media - Hard Drive
            (0x01, 0x02) => BootOptionType::CdRom,      // Media - CD-ROM
            (0x03, 0x05) => BootOptionType::Usb,        // Messaging - USB
            (0x03, 0x0B..=0x0F) => BootOptionType::Network,  // Various network types
            (0x04, _) => BootOptionType::FilePath,      // Media device path
            (0x05, _) => BootOptionType::BbsBoot,       // BBS boot
            (0x07, _) => BootOptionType::FirmwareVolume, // Firmware volume
            _ => BootOptionType::Unknown,
        }
    }

    /// Convert to bytes for writing
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Attributes (4 bytes)
        data.extend_from_slice(&self.attributes.to_le_bytes());

        // Device path length (2 bytes)
        data.extend_from_slice(&(self.device_path.len() as u16).to_le_bytes());

        // Description (UCS-2 null-terminated)
        for ch in self.description.chars() {
            let code = ch as u16;
            data.extend_from_slice(&code.to_le_bytes());
        }
        data.extend_from_slice(&0u16.to_le_bytes()); // Null terminator

        // Device path
        data.extend_from_slice(&self.device_path);

        // Optional data
        data.extend_from_slice(&self.optional_data);

        data
    }
}

/// Cached variable entry
#[derive(Debug, Clone)]
struct CachedVariable {
    name: String,
    guid: EfiGuid,
    data: Vec<u8>,
    attributes: u32,
    dirty: bool,
}

/// UEFI Variables Manager
pub struct UefiVariablesManager {
    /// Cache of recently accessed variables
    cache: Vec<CachedVariable>,
    /// Maximum cache size
    max_cache_size: usize,
    /// Whether caching is enabled
    cache_enabled: bool,
    /// Boot options cache
    boot_options: Vec<BootOption>,
    /// Boot order
    boot_order: Vec<u16>,
    /// Secure boot state cached
    secure_boot_enabled: Option<bool>,
}

impl UefiVariablesManager {
    /// Create a new variables manager
    pub fn new() -> Self {
        Self {
            cache: Vec::new(),
            max_cache_size: 64,
            cache_enabled: true,
            boot_options: Vec::new(),
            boot_order: Vec::new(),
            secure_boot_enabled: None,
        }
    }

    /// Initialize and load common variables
    pub fn init(&mut self) {
        // Load secure boot state
        self.secure_boot_enabled = self.read_secure_boot_state();

        // Load boot order
        if let Ok(order) = self.read_boot_order() {
            self.boot_order = order;
        }

        // Load boot options
        self.load_boot_options();
    }

    /// Convert a Rust string to UCS-2 (UTF-16LE)
    fn string_to_ucs2(s: &str) -> Vec<u16> {
        let mut result: Vec<u16> = s.encode_utf16().collect();
        result.push(0); // Null terminator
        result
    }

    /// Read a variable
    pub fn get_variable(&mut self, name: &str, guid: &EfiGuid) -> Result<(Vec<u8>, u32), EfiStatus> {
        // Check cache first
        if self.cache_enabled {
            for entry in &self.cache {
                if entry.name == name && entry.guid == *guid {
                    return Ok((entry.data.clone(), entry.attributes));
                }
            }
        }

        // Read from UEFI
        let name_ucs2 = Self::string_to_ucs2(name);
        let mut buffer = vec![0u8; 4096];

        match uefi_runtime::get_variable(&name_ucs2, guid, &mut buffer) {
            Ok((attributes, size)) => {
                buffer.truncate(size);

                // Add to cache
                if self.cache_enabled {
                    self.add_to_cache(name.into(), *guid, buffer.clone(), attributes);
                }

                Ok((buffer, attributes))
            }
            Err(e) => Err(e),
        }
    }

    /// Write a variable
    pub fn set_variable(&mut self, name: &str, guid: &EfiGuid, attributes: u32, data: &[u8]) -> Result<(), EfiStatus> {
        let name_ucs2 = Self::string_to_ucs2(name);

        // Write to UEFI
        uefi_runtime::set_variable(&name_ucs2, guid, attributes, data)?;

        // Update cache
        if self.cache_enabled {
            self.add_to_cache(name.into(), *guid, data.to_vec(), attributes);
        }

        Ok(())
    }

    /// Delete a variable
    pub fn delete_variable(&mut self, name: &str, guid: &EfiGuid) -> Result<(), EfiStatus> {
        let name_ucs2 = Self::string_to_ucs2(name);

        // Delete by setting empty data with no attributes
        uefi_runtime::set_variable(&name_ucs2, guid, 0, &[])?;

        // Remove from cache
        self.cache.retain(|e| !(e.name == name && e.guid == *guid));

        Ok(())
    }

    /// Add entry to cache
    fn add_to_cache(&mut self, name: String, guid: EfiGuid, data: Vec<u8>, attributes: u32) {
        // Remove existing entry if present
        self.cache.retain(|e| !(e.name == name && e.guid == guid));

        // Check cache size
        if self.cache.len() >= self.max_cache_size {
            self.cache.remove(0); // Remove oldest
        }

        self.cache.push(CachedVariable {
            name,
            guid,
            data,
            attributes,
            dirty: false,
        });
    }

    /// Read boot order variable
    pub fn read_boot_order(&mut self) -> Result<Vec<u16>, EfiStatus> {
        let (data, _) = self.get_variable("BootOrder", &guids::EFI_GLOBAL_VARIABLE)?;

        let mut order = Vec::new();
        for i in (0..data.len()).step_by(2) {
            if i + 1 < data.len() {
                order.push(u16::from_le_bytes([data[i], data[i + 1]]));
            }
        }

        Ok(order)
    }

    /// Write boot order variable
    pub fn write_boot_order(&mut self, order: &[u16]) -> Result<(), EfiStatus> {
        let mut data = Vec::new();
        for &num in order {
            data.extend_from_slice(&num.to_le_bytes());
        }

        self.set_variable("BootOrder", &guids::EFI_GLOBAL_VARIABLE, attrs::NV_BS_RT, &data)?;
        self.boot_order = order.to_vec();

        Ok(())
    }

    /// Read a boot option
    pub fn read_boot_option(&mut self, number: u16) -> Result<BootOption, EfiStatus> {
        let name = format!("Boot{:04X}", number);
        let (data, _) = self.get_variable(&name, &guids::EFI_GLOBAL_VARIABLE)?;

        BootOption::from_bytes(number, &data).ok_or(EfiStatus::InvalidParameter)
    }

    /// Write a boot option
    pub fn write_boot_option(&mut self, option: &BootOption) -> Result<(), EfiStatus> {
        let name = format!("Boot{:04X}", option.number);
        let data = option.to_bytes();

        self.set_variable(&name, &guids::EFI_GLOBAL_VARIABLE, attrs::NV_BS_RT, &data)
    }

    /// Delete a boot option
    pub fn delete_boot_option(&mut self, number: u16) -> Result<(), EfiStatus> {
        let name = format!("Boot{:04X}", number);
        self.delete_variable(&name, &guids::EFI_GLOBAL_VARIABLE)?;

        // Remove from boot order
        self.boot_order.retain(|&n| n != number);
        self.write_boot_order(&self.boot_order.clone())?;

        // Remove from cache
        self.boot_options.retain(|o| o.number != number);

        Ok(())
    }

    /// Load all boot options
    pub fn load_boot_options(&mut self) {
        self.boot_options.clear();

        for &number in &self.boot_order.clone() {
            if let Ok(option) = self.read_boot_option(number) {
                self.boot_options.push(option);
            }
        }
    }

    /// Get all boot options
    pub fn boot_options(&self) -> &[BootOption] {
        &self.boot_options
    }

    /// Get boot order
    pub fn boot_order(&self) -> &[u16] {
        &self.boot_order
    }

    /// Find next available boot option number
    pub fn next_boot_number(&self) -> u16 {
        for i in 0..0xFFFF {
            if !self.boot_order.contains(&i) {
                return i;
            }
        }
        0xFFFF
    }

    /// Create a new boot option
    pub fn create_boot_option(&mut self, description: &str, device_path: &[u8], optional_data: &[u8]) -> Result<u16, EfiStatus> {
        let number = self.next_boot_number();

        let option = BootOption {
            number,
            attributes: BootOption::LOAD_OPTION_ACTIVE,
            description: description.into(),
            device_path: device_path.to_vec(),
            optional_data: optional_data.to_vec(),
            option_type: BootOption::detect_type(device_path),
        };

        self.write_boot_option(&option)?;

        // Add to boot order
        let mut new_order = self.boot_order.clone();
        new_order.push(number);
        self.write_boot_order(&new_order)?;

        self.boot_options.push(option);

        Ok(number)
    }

    /// Set boot option as first in boot order
    pub fn set_first_boot(&mut self, number: u16) -> Result<(), EfiStatus> {
        if !self.boot_order.contains(&number) {
            return Err(EfiStatus::NotFound);
        }

        let mut new_order: Vec<u16> = vec![number];
        for &n in &self.boot_order {
            if n != number {
                new_order.push(n);
            }
        }

        self.write_boot_order(&new_order)
    }

    /// Read secure boot state
    fn read_secure_boot_state(&mut self) -> Option<bool> {
        match self.get_variable("SecureBoot", &guids::EFI_GLOBAL_VARIABLE) {
            Ok((data, _)) if !data.is_empty() => Some(data[0] != 0),
            _ => None,
        }
    }

    /// Check if secure boot is enabled
    pub fn is_secure_boot_enabled(&self) -> bool {
        self.secure_boot_enabled.unwrap_or(false)
    }

    /// Read setup mode state
    pub fn is_setup_mode(&mut self) -> bool {
        match self.get_variable("SetupMode", &guids::EFI_GLOBAL_VARIABLE) {
            Ok((data, _)) if !data.is_empty() => data[0] != 0,
            _ => false,
        }
    }

    /// Read platform key enrolled state
    pub fn is_pk_enrolled(&mut self) -> bool {
        self.get_variable("PK", &guids::EFI_GLOBAL_VARIABLE).is_ok()
    }

    /// Read timeout variable
    pub fn read_timeout(&mut self) -> Option<u16> {
        match self.get_variable("Timeout", &guids::EFI_GLOBAL_VARIABLE) {
            Ok((data, _)) if data.len() >= 2 => {
                Some(u16::from_le_bytes([data[0], data[1]]))
            }
            _ => None,
        }
    }

    /// Write timeout variable
    pub fn write_timeout(&mut self, timeout: u16) -> Result<(), EfiStatus> {
        self.set_variable("Timeout", &guids::EFI_GLOBAL_VARIABLE, attrs::NV_BS_RT, &timeout.to_le_bytes())
    }

    /// Read platform language
    pub fn read_platform_lang(&mut self) -> Option<String> {
        match self.get_variable("PlatformLang", &guids::EFI_GLOBAL_VARIABLE) {
            Ok((data, _)) => {
                String::from_utf8(data.into_iter().take_while(|&b| b != 0).collect()).ok()
            }
            _ => None,
        }
    }

    /// Read OS indications supported
    pub fn read_os_indications_supported(&mut self) -> u64 {
        match self.get_variable("OsIndicationsSupported", &guids::EFI_GLOBAL_VARIABLE) {
            Ok((data, _)) if data.len() >= 8 => {
                u64::from_le_bytes([
                    data[0], data[1], data[2], data[3],
                    data[4], data[5], data[6], data[7],
                ])
            }
            _ => 0,
        }
    }

    /// OS Indication: Boot to firmware UI
    pub const OS_INDICATION_BOOT_TO_FW_UI: u64 = 0x0000000000000001;
    /// OS Indication: Timestamp revocation supported
    pub const OS_INDICATION_TIMESTAMP_REVOCATION: u64 = 0x0000000000000002;
    /// OS Indication: File capsule delivery supported
    pub const OS_INDICATION_FILE_CAPSULE_DELIVERY: u64 = 0x0000000000000004;
    /// OS Indication: FMP capsule supported
    pub const OS_INDICATION_FMP_CAPSULE: u64 = 0x0000000000000008;
    /// OS Indication: Capsule result variable supported
    pub const OS_INDICATION_CAPSULE_RESULT: u64 = 0x0000000000000010;
    /// OS Indication: Start OS recovery supported
    pub const OS_INDICATION_START_OS_RECOVERY: u64 = 0x0000000000000020;
    /// OS Indication: Start platform recovery supported
    pub const OS_INDICATION_START_PLATFORM_RECOVERY: u64 = 0x0000000000000040;
    /// OS Indication: JSON config data refresh supported
    pub const OS_INDICATION_JSON_CONFIG_REFRESH: u64 = 0x0000000000000080;

    /// Request boot to firmware UI on next reboot
    pub fn request_firmware_ui(&mut self) -> Result<(), EfiStatus> {
        let supported = self.read_os_indications_supported();
        if supported & Self::OS_INDICATION_BOOT_TO_FW_UI == 0 {
            return Err(EfiStatus::Unsupported);
        }

        self.set_variable(
            "OsIndications",
            &guids::EFI_GLOBAL_VARIABLE,
            attrs::NV_BS_RT,
            &Self::OS_INDICATION_BOOT_TO_FW_UI.to_le_bytes(),
        )
    }

    /// Read a Stenzel OS specific variable
    pub fn get_stenzel_var(&mut self, name: &str) -> Result<Vec<u8>, EfiStatus> {
        let (data, _) = self.get_variable(name, &guids::STENZEL_OS_GUID)?;
        Ok(data)
    }

    /// Write a Stenzel OS specific variable
    pub fn set_stenzel_var(&mut self, name: &str, data: &[u8]) -> Result<(), EfiStatus> {
        self.set_variable(name, &guids::STENZEL_OS_GUID, attrs::NV_BS_RT, data)
    }

    /// Clear variable cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        (self.cache.len(), self.max_cache_size)
    }

    /// Format status for display
    pub fn format_status(&self) -> String {
        let mut s = String::from("UEFI Variables:\n");
        s.push_str(&format!("  Secure Boot: {}\n", if self.is_secure_boot_enabled() { "Enabled" } else { "Disabled" }));
        s.push_str(&format!("  Boot Options: {}\n", self.boot_options.len()));
        s.push_str(&format!("  Boot Order: {:?}\n", self.boot_order));
        s.push_str(&format!("  Cache: {}/{}\n", self.cache.len(), self.max_cache_size));

        if !self.boot_options.is_empty() {
            s.push_str("  Boot Entries:\n");
            for opt in &self.boot_options {
                let status = if opt.is_active() { "active" } else { "inactive" };
                let hidden = if opt.is_hidden() { " (hidden)" } else { "" };
                s.push_str(&format!("    Boot{:04X}: {} [{}]{}\n",
                    opt.number, opt.description, status, hidden));
            }
        }

        s
    }
}

/// Global UEFI variables manager
static UEFI_VARS_INIT: AtomicBool = AtomicBool::new(false);
static UEFI_VARS: TicketSpinlock<Option<UefiVariablesManager>> = TicketSpinlock::new(None);

/// Initialize the UEFI variables manager
pub fn init() {
    if UEFI_VARS_INIT.swap(true, Ordering::SeqCst) {
        return; // Already initialized
    }

    let mut manager = UefiVariablesManager::new();

    if uefi_runtime::is_available() {
        manager.init();
    }

    *UEFI_VARS.lock() = Some(manager);
}

/// Check if UEFI variables are available
pub fn is_available() -> bool {
    uefi_runtime::is_available()
}

/// Get a variable
pub fn get_variable(name: &str, guid: &EfiGuid) -> Result<(Vec<u8>, u32), EfiStatus> {
    let mut guard = UEFI_VARS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.get_variable(name, guid)
    } else {
        Err(EfiStatus::NotReady)
    }
}

/// Set a variable
pub fn set_variable(name: &str, guid: &EfiGuid, attributes: u32, data: &[u8]) -> Result<(), EfiStatus> {
    let mut guard = UEFI_VARS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.set_variable(name, guid, attributes, data)
    } else {
        Err(EfiStatus::NotReady)
    }
}

/// Delete a variable
pub fn delete_variable(name: &str, guid: &EfiGuid) -> Result<(), EfiStatus> {
    let mut guard = UEFI_VARS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.delete_variable(name, guid)
    } else {
        Err(EfiStatus::NotReady)
    }
}

/// Get boot order
pub fn get_boot_order() -> Result<Vec<u16>, EfiStatus> {
    let mut guard = UEFI_VARS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.read_boot_order()
    } else {
        Err(EfiStatus::NotReady)
    }
}

/// Set boot order
pub fn set_boot_order(order: &[u16]) -> Result<(), EfiStatus> {
    let mut guard = UEFI_VARS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.write_boot_order(order)
    } else {
        Err(EfiStatus::NotReady)
    }
}

/// Get a boot option
pub fn get_boot_option(number: u16) -> Result<BootOption, EfiStatus> {
    let mut guard = UEFI_VARS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.read_boot_option(number)
    } else {
        Err(EfiStatus::NotReady)
    }
}

/// Create a new boot option
pub fn create_boot_option(description: &str, device_path: &[u8], optional_data: &[u8]) -> Result<u16, EfiStatus> {
    let mut guard = UEFI_VARS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.create_boot_option(description, device_path, optional_data)
    } else {
        Err(EfiStatus::NotReady)
    }
}

/// Delete a boot option
pub fn delete_boot_option(number: u16) -> Result<(), EfiStatus> {
    let mut guard = UEFI_VARS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.delete_boot_option(number)
    } else {
        Err(EfiStatus::NotReady)
    }
}

/// Check if secure boot is enabled
pub fn is_secure_boot_enabled() -> bool {
    let guard = UEFI_VARS.lock();
    if let Some(manager) = guard.as_ref() {
        manager.is_secure_boot_enabled()
    } else {
        false
    }
}

/// Request boot to firmware UI
pub fn request_firmware_ui() -> Result<(), EfiStatus> {
    let mut guard = UEFI_VARS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.request_firmware_ui()
    } else {
        Err(EfiStatus::NotReady)
    }
}

/// Get timeout value
pub fn get_timeout() -> Option<u16> {
    let mut guard = UEFI_VARS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.read_timeout()
    } else {
        None
    }
}

/// Set timeout value
pub fn set_timeout(timeout: u16) -> Result<(), EfiStatus> {
    let mut guard = UEFI_VARS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.write_timeout(timeout)
    } else {
        Err(EfiStatus::NotReady)
    }
}

/// Get Stenzel OS variable
pub fn get_stenzel_var(name: &str) -> Result<Vec<u8>, EfiStatus> {
    let mut guard = UEFI_VARS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.get_stenzel_var(name)
    } else {
        Err(EfiStatus::NotReady)
    }
}

/// Set Stenzel OS variable
pub fn set_stenzel_var(name: &str, data: &[u8]) -> Result<(), EfiStatus> {
    let mut guard = UEFI_VARS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.set_stenzel_var(name, data)
    } else {
        Err(EfiStatus::NotReady)
    }
}

/// Format status for display
pub fn format_status() -> String {
    let guard = UEFI_VARS.lock();
    if let Some(manager) = guard.as_ref() {
        manager.format_status()
    } else {
        String::from("UEFI Variables: Not initialized\n")
    }
}
