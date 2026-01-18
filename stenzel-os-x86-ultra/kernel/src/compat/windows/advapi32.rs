//! ADVAPI32.dll Emulation
//!
//! Advanced Windows API providing:
//! - Registry functions
//! - Security and access control
//! - Cryptographic services
//! - Event logging
//! - Service Control Manager

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU32, Ordering};
use spin::Mutex;

use super::registry::{registry, RegistryValue, reg_type, hkey as reg_hkey, access as reg_access};

// =============================================================================
// Windows Types
// =============================================================================

pub type HKEY = u64;
pub type HANDLE = u64;
pub type DWORD = u32;
pub type LONG = i32;
pub type BOOL = i32;
pub type REGSAM = u32;
pub type LSTATUS = i32;
pub type SC_HANDLE = u64;

// =============================================================================
// Predefined Registry Keys (re-export from registry module)
// =============================================================================

pub mod hkey {
    use super::HKEY;
    pub const CLASSES_ROOT: HKEY = 0x80000000;
    pub const CURRENT_USER: HKEY = 0x80000001;
    pub const LOCAL_MACHINE: HKEY = 0x80000002;
    pub const USERS: HKEY = 0x80000003;
    pub const CURRENT_CONFIG: HKEY = 0x80000005;
}

// =============================================================================
// Error Codes
// =============================================================================

pub mod error {
    pub const ERROR_SUCCESS: i32 = 0;
    pub const ERROR_FILE_NOT_FOUND: i32 = 2;
    pub const ERROR_ACCESS_DENIED: i32 = 5;
    pub const ERROR_INVALID_HANDLE: i32 = 6;
    pub const ERROR_OUTOFMEMORY: i32 = 14;
    pub const ERROR_INVALID_PARAMETER: i32 = 87;
    pub const ERROR_CALL_NOT_IMPLEMENTED: i32 = 120;
    pub const ERROR_MORE_DATA: i32 = 234;
    pub const ERROR_NO_MORE_ITEMS: i32 = 259;
}

// =============================================================================
// Disposition values
// =============================================================================

pub mod disposition {
    pub const REG_CREATED_NEW_KEY: u32 = 1;
    pub const REG_OPENED_EXISTING_KEY: u32 = 2;
}

// =============================================================================
// ADVAPI32 Emulation Context
// =============================================================================

pub struct Advapi32 {
    /// Crypto context handles
    crypto_contexts: BTreeMap<HANDLE, CryptoContext>,
    next_handle: u64,
    /// Event log handles
    event_logs: BTreeMap<HANDLE, EventLog>,
    /// Service handles
    services: BTreeMap<SC_HANDLE, ServiceInfo>,
}

impl Advapi32 {
    pub const fn new() -> Self {
        Advapi32 {
            crypto_contexts: BTreeMap::new(),
            next_handle: 1,
            event_logs: BTreeMap::new(),
            services: BTreeMap::new(),
        }
    }

    // =========================================================================
    // Registry Functions (using existing registry module)
    // =========================================================================

    /// RegOpenKeyExA
    pub fn reg_open_key_ex_a(
        hkey: HKEY,
        sub_key: &str,
        _options: DWORD,
        access: REGSAM,
        phk_result: &mut HKEY,
    ) -> LSTATUS {
        let reg = registry();
        match reg.open_key(hkey, sub_key, access) {
            Some(handle) => {
                *phk_result = handle;
                error::ERROR_SUCCESS
            }
            None => error::ERROR_FILE_NOT_FOUND,
        }
    }

    /// RegCreateKeyExA
    pub fn reg_create_key_ex_a(
        hkey: HKEY,
        sub_key: &str,
        _reserved: DWORD,
        _class: Option<&str>,
        _options: DWORD,
        access: REGSAM,
        _security: u64,
        phk_result: &mut HKEY,
        disposition_out: &mut DWORD,
    ) -> LSTATUS {
        let reg = registry();
        match reg.create_key(hkey, sub_key, access) {
            Some((handle, created)) => {
                *phk_result = handle;
                *disposition_out = if created {
                    disposition::REG_CREATED_NEW_KEY
                } else {
                    disposition::REG_OPENED_EXISTING_KEY
                };
                error::ERROR_SUCCESS
            }
            None => error::ERROR_INVALID_HANDLE,
        }
    }

    /// RegCloseKey
    pub fn reg_close_key(hkey: HKEY) -> LSTATUS {
        // Don't close predefined keys
        if hkey >= 0x80000000 && hkey <= 0x80000005 {
            return error::ERROR_SUCCESS;
        }
        let reg = registry();
        if reg.close_key(hkey) {
            error::ERROR_SUCCESS
        } else {
            error::ERROR_INVALID_HANDLE
        }
    }

    /// RegQueryValueExA
    pub fn reg_query_value_ex_a(
        hkey: HKEY,
        value_name: &str,
        _reserved: u64,
        lp_type: &mut DWORD,
        lp_data: &mut [u8],
        lpcb_data: &mut DWORD,
    ) -> LSTATUS {
        let reg = registry();
        match reg.query_value(hkey, value_name) {
            Some(value) => {
                *lp_type = value.value_type;

                if value.data.len() > lp_data.len() {
                    *lpcb_data = value.data.len() as DWORD;
                    return error::ERROR_MORE_DATA;
                }

                lp_data[..value.data.len()].copy_from_slice(&value.data);
                *lpcb_data = value.data.len() as DWORD;
                error::ERROR_SUCCESS
            }
            None => error::ERROR_FILE_NOT_FOUND,
        }
    }

    /// RegSetValueExA
    pub fn reg_set_value_ex_a(
        hkey: HKEY,
        value_name: &str,
        _reserved: DWORD,
        dw_type: DWORD,
        lp_data: &[u8],
        _cb_data: DWORD,
    ) -> LSTATUS {
        let value = RegistryValue {
            value_type: dw_type,
            data: lp_data.to_vec(),
        };

        let reg = registry();
        if reg.set_value(hkey, value_name, value) {
            error::ERROR_SUCCESS
        } else {
            error::ERROR_INVALID_HANDLE
        }
    }

    /// RegDeleteValueA
    pub fn reg_delete_value_a(hkey: HKEY, value_name: &str) -> LSTATUS {
        let reg = registry();
        if reg.delete_value(hkey, value_name) {
            error::ERROR_SUCCESS
        } else {
            error::ERROR_FILE_NOT_FOUND
        }
    }

    /// RegEnumKeyExA
    pub fn reg_enum_key_ex_a(
        hkey: HKEY,
        dw_index: DWORD,
        lp_name: &mut [u8],
        lpcch_name: &mut DWORD,
    ) -> LSTATUS {
        let reg = registry();
        match reg.enum_key(hkey, dw_index) {
            Some(name) => {
                if name.len() + 1 > lp_name.len() {
                    *lpcch_name = (name.len() + 1) as DWORD;
                    return error::ERROR_MORE_DATA;
                }
                lp_name[..name.len()].copy_from_slice(name.as_bytes());
                lp_name[name.len()] = 0;
                *lpcch_name = name.len() as DWORD;
                error::ERROR_SUCCESS
            }
            None => error::ERROR_NO_MORE_ITEMS,
        }
    }

    /// RegEnumValueA
    pub fn reg_enum_value_a(
        hkey: HKEY,
        dw_index: DWORD,
        lp_value_name: &mut [u8],
        lpcch_value_name: &mut DWORD,
    ) -> LSTATUS {
        let reg = registry();
        match reg.enum_value(hkey, dw_index) {
            Some((name, _value)) => {
                if name.len() + 1 > lp_value_name.len() {
                    *lpcch_value_name = (name.len() + 1) as DWORD;
                    return error::ERROR_MORE_DATA;
                }
                lp_value_name[..name.len()].copy_from_slice(name.as_bytes());
                lp_value_name[name.len()] = 0;
                *lpcch_value_name = name.len() as DWORD;
                error::ERROR_SUCCESS
            }
            None => error::ERROR_NO_MORE_ITEMS,
        }
    }

    // =========================================================================
    // Security Functions
    // =========================================================================

    /// OpenProcessToken
    pub fn open_process_token(
        process_handle: HANDLE,
        _desired_access: DWORD,
        token_handle: &mut HANDLE,
    ) -> BOOL {
        // Current process handle is -1 or 0xFFFFFFFFFFFFFFFF
        if process_handle == !0u64 || process_handle == 0xFFFFFFFF {
            *token_handle = 0xDEADBEEF; // Dummy token
            1
        } else {
            0
        }
    }

    /// GetTokenInformation
    pub fn get_token_information(
        _token: HANDLE,
        info_class: DWORD,
        info: &mut [u8],
        _info_length: DWORD,
        return_length: &mut DWORD,
    ) -> BOOL {
        match info_class {
            1 => { // TokenUser
                let needed = 28;
                *return_length = needed;
                if info.len() >= needed as usize {
                    return 1;
                }
                0
            }
            _ => {
                *return_length = 0;
                0
            }
        }
    }

    /// AdjustTokenPrivileges
    pub fn adjust_token_privileges(
        _token: HANDLE,
        _disable_all: BOOL,
        _new_state: u64,
        _buffer_length: DWORD,
        _previous: u64,
        _return_length: &mut DWORD,
    ) -> BOOL {
        1 // Always succeed
    }

    /// LookupPrivilegeValueA
    pub fn lookup_privilege_value_a(
        _system: Option<&str>,
        name: &str,
        luid: &mut u64,
    ) -> BOOL {
        let priv_luid = match name {
            "SeDebugPrivilege" => 20,
            "SeBackupPrivilege" => 17,
            "SeRestorePrivilege" => 18,
            "SeShutdownPrivilege" => 19,
            "SeTakeOwnershipPrivilege" => 9,
            "SeSecurityPrivilege" => 8,
            "SeLoadDriverPrivilege" => 10,
            _ => 0,
        };
        if priv_luid > 0 {
            *luid = priv_luid;
            1
        } else {
            0
        }
    }

    /// GetUserNameA
    pub fn get_user_name_a(buffer: &mut [u8], size: &mut DWORD) -> BOOL {
        let name = "Administrator";
        if name.len() + 1 > buffer.len() {
            *size = (name.len() + 1) as DWORD;
            return 0;
        }
        buffer[..name.len()].copy_from_slice(name.as_bytes());
        buffer[name.len()] = 0;
        *size = (name.len() + 1) as DWORD;
        1
    }

    // =========================================================================
    // Cryptographic Functions
    // =========================================================================

    /// CryptAcquireContextA
    pub fn crypt_acquire_context(
        &mut self,
        ph_prov: &mut HANDLE,
        _container: Option<&str>,
        _provider: Option<&str>,
        prov_type: DWORD,
        _flags: DWORD,
    ) -> BOOL {
        let ctx = CryptoContext { provider_type: prov_type };
        let handle = self.next_handle;
        self.next_handle += 1;
        self.crypto_contexts.insert(handle, ctx);
        *ph_prov = handle;
        1
    }

    /// CryptReleaseContext
    pub fn crypt_release_context(&mut self, h_prov: HANDLE, _flags: DWORD) -> BOOL {
        if self.crypto_contexts.remove(&h_prov).is_some() { 1 } else { 0 }
    }

    /// CryptGenRandom
    pub fn crypt_gen_random(_h_prov: HANDLE, len: DWORD, buffer: &mut [u8]) -> BOOL {
        for i in 0..(len as usize).min(buffer.len()) {
            let rand: u64;
            unsafe {
                let lo: u32;
                let hi: u32;
                core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi);
                rand = ((hi as u64) << 32) | (lo as u64);
            }
            buffer[i] = (rand ^ (i as u64 * 0x5DEECE66D)) as u8;
        }
        1
    }

    // =========================================================================
    // Event Logging
    // =========================================================================

    /// RegisterEventSourceA
    pub fn register_event_source(&mut self, _server: Option<&str>, source: &str) -> HANDLE {
        let handle = self.next_handle;
        self.next_handle += 1;
        self.event_logs.insert(handle, EventLog { source: source.to_string() });
        handle
    }

    /// DeregisterEventSource
    pub fn deregister_event_source(&mut self, handle: HANDLE) -> BOOL {
        if self.event_logs.remove(&handle).is_some() { 1 } else { 0 }
    }

    /// ReportEventA
    pub fn report_event(
        &self,
        handle: HANDLE,
        event_type: u16,
        _category: u16,
        event_id: DWORD,
        _sid: u64,
        _num_strings: u16,
        _data_size: DWORD,
        _strings: u64,
        _data: u64,
    ) -> BOOL {
        if let Some(log) = self.event_logs.get(&handle) {
            crate::kprintln!("advapi32: Event[{}] type={} id={}", log.source, event_type, event_id);
            1
        } else {
            0
        }
    }

    // =========================================================================
    // Service Control Manager
    // =========================================================================

    /// OpenSCManagerA
    pub fn open_sc_manager(_machine: Option<&str>, _database: Option<&str>, _access: DWORD) -> SC_HANDLE {
        0x5C000001 // Dummy SCM handle
    }

    /// CloseServiceHandle
    pub fn close_service_handle(&mut self, handle: SC_HANDLE) -> BOOL {
        if handle == 0x5C000001 { return 1; }
        if self.services.remove(&handle).is_some() { 1 } else { 0 }
    }

    /// OpenServiceA
    pub fn open_service(&mut self, _scm: SC_HANDLE, name: &str, _access: DWORD) -> SC_HANDLE {
        let handle = self.next_handle;
        self.next_handle += 1;
        self.services.insert(handle, ServiceInfo {
            name: name.to_string(),
            status: ServiceStatus::Stopped,
        });
        handle
    }

    /// StartServiceA
    pub fn start_service(&mut self, handle: SC_HANDLE, _argc: DWORD, _argv: u64) -> BOOL {
        if let Some(svc) = self.services.get_mut(&handle) {
            svc.status = ServiceStatus::Running;
            crate::kprintln!("advapi32: Started service '{}'", svc.name);
            1
        } else {
            0
        }
    }

    /// ControlService
    pub fn control_service(&mut self, handle: SC_HANDLE, control: DWORD, _status: u64) -> BOOL {
        if let Some(svc) = self.services.get_mut(&handle) {
            match control {
                1 => svc.status = ServiceStatus::Stopped,  // STOP
                2 => svc.status = ServiceStatus::Paused,   // PAUSE
                3 => svc.status = ServiceStatus::Running,  // CONTINUE
                _ => {}
            }
            1
        } else {
            0
        }
    }

    /// QueryServiceStatus
    pub fn query_service_status(&self, handle: SC_HANDLE, status: &mut [u8]) -> BOOL {
        if let Some(svc) = self.services.get(&handle) {
            if status.len() >= 28 {
                let state: u32 = match svc.status {
                    ServiceStatus::Stopped => 1,
                    ServiceStatus::Running => 4,
                    ServiceStatus::Paused => 7,
                };
                // SERVICE_STATUS structure
                status[0..4].copy_from_slice(&16u32.to_le_bytes()); // dwServiceType
                status[4..8].copy_from_slice(&state.to_le_bytes()); // dwCurrentState
                status[8..12].copy_from_slice(&7u32.to_le_bytes()); // dwControlsAccepted
                return 1;
            }
        }
        0
    }
}

// =============================================================================
// Helper Structures
// =============================================================================

struct CryptoContext {
    provider_type: DWORD,
}

struct EventLog {
    source: String,
}

struct ServiceInfo {
    name: String,
    status: ServiceStatus,
}

#[derive(Clone, Copy)]
enum ServiceStatus {
    Stopped,
    Running,
    Paused,
}

// =============================================================================
// Global Instance
// =============================================================================

pub static ADVAPI32: Mutex<Advapi32> = Mutex::new(Advapi32::new());

pub fn init() {
    crate::kprintln!("advapi32: initializing ADVAPI32 emulation");
    crate::kprintln!("advapi32: ADVAPI32 emulation ready");
}

// =============================================================================
// C-style API Wrappers
// =============================================================================

pub fn RegOpenKeyExA(hkey: HKEY, sub_key: &str, options: DWORD, sam: REGSAM, result: &mut HKEY) -> LSTATUS {
    Advapi32::reg_open_key_ex_a(hkey, sub_key, options, sam, result)
}

pub fn RegCloseKey(hkey: HKEY) -> LSTATUS {
    Advapi32::reg_close_key(hkey)
}

pub fn RegQueryValueExA(hkey: HKEY, name: &str, reserved: u64, lp_type: &mut DWORD, data: &mut [u8], cb_data: &mut DWORD) -> LSTATUS {
    Advapi32::reg_query_value_ex_a(hkey, name, reserved, lp_type, data, cb_data)
}

pub fn RegSetValueExA(hkey: HKEY, name: &str, reserved: DWORD, dw_type: DWORD, data: &[u8], cb_data: DWORD) -> LSTATUS {
    Advapi32::reg_set_value_ex_a(hkey, name, reserved, dw_type, data, cb_data)
}

pub fn GetUserNameA(buffer: &mut [u8], size: &mut DWORD) -> BOOL {
    Advapi32::get_user_name_a(buffer, size)
}

// =============================================================================
// Export Table
// =============================================================================

/// Get ADVAPI32 exports
pub fn get_exports() -> BTreeMap<String, u64> {
    let mut exports = BTreeMap::new();
    let mut addr = 0x7FF3_0000_u64;

    // Registry functions
    exports.insert("RegOpenKeyExA".into(), addr); addr += 0x100;
    exports.insert("RegOpenKeyExW".into(), addr); addr += 0x100;
    exports.insert("RegCreateKeyExA".into(), addr); addr += 0x100;
    exports.insert("RegCreateKeyExW".into(), addr); addr += 0x100;
    exports.insert("RegCloseKey".into(), addr); addr += 0x100;
    exports.insert("RegQueryValueExA".into(), addr); addr += 0x100;
    exports.insert("RegQueryValueExW".into(), addr); addr += 0x100;
    exports.insert("RegSetValueExA".into(), addr); addr += 0x100;
    exports.insert("RegSetValueExW".into(), addr); addr += 0x100;
    exports.insert("RegDeleteValueA".into(), addr); addr += 0x100;
    exports.insert("RegDeleteValueW".into(), addr); addr += 0x100;
    exports.insert("RegDeleteKeyA".into(), addr); addr += 0x100;
    exports.insert("RegDeleteKeyW".into(), addr); addr += 0x100;
    exports.insert("RegEnumKeyExA".into(), addr); addr += 0x100;
    exports.insert("RegEnumKeyExW".into(), addr); addr += 0x100;
    exports.insert("RegEnumValueA".into(), addr); addr += 0x100;
    exports.insert("RegEnumValueW".into(), addr); addr += 0x100;
    exports.insert("RegQueryInfoKeyA".into(), addr); addr += 0x100;
    exports.insert("RegQueryInfoKeyW".into(), addr); addr += 0x100;
    exports.insert("RegFlushKey".into(), addr); addr += 0x100;
    exports.insert("RegNotifyChangeKeyValue".into(), addr); addr += 0x100;
    exports.insert("RegLoadKeyA".into(), addr); addr += 0x100;
    exports.insert("RegLoadKeyW".into(), addr); addr += 0x100;
    exports.insert("RegUnLoadKeyA".into(), addr); addr += 0x100;
    exports.insert("RegUnLoadKeyW".into(), addr); addr += 0x100;
    exports.insert("RegSaveKeyA".into(), addr); addr += 0x100;
    exports.insert("RegSaveKeyW".into(), addr); addr += 0x100;
    exports.insert("RegRestoreKeyA".into(), addr); addr += 0x100;
    exports.insert("RegRestoreKeyW".into(), addr); addr += 0x100;
    exports.insert("RegReplaceKeyA".into(), addr); addr += 0x100;
    exports.insert("RegReplaceKeyW".into(), addr); addr += 0x100;
    exports.insert("RegConnectRegistryA".into(), addr); addr += 0x100;
    exports.insert("RegConnectRegistryW".into(), addr); addr += 0x100;

    // Security functions
    exports.insert("OpenProcessToken".into(), addr); addr += 0x100;
    exports.insert("OpenThreadToken".into(), addr); addr += 0x100;
    exports.insert("GetTokenInformation".into(), addr); addr += 0x100;
    exports.insert("SetTokenInformation".into(), addr); addr += 0x100;
    exports.insert("AdjustTokenPrivileges".into(), addr); addr += 0x100;
    exports.insert("AdjustTokenGroups".into(), addr); addr += 0x100;
    exports.insert("LookupPrivilegeValueA".into(), addr); addr += 0x100;
    exports.insert("LookupPrivilegeValueW".into(), addr); addr += 0x100;
    exports.insert("LookupPrivilegeNameA".into(), addr); addr += 0x100;
    exports.insert("LookupPrivilegeNameW".into(), addr); addr += 0x100;
    exports.insert("LookupAccountSidA".into(), addr); addr += 0x100;
    exports.insert("LookupAccountSidW".into(), addr); addr += 0x100;
    exports.insert("LookupAccountNameA".into(), addr); addr += 0x100;
    exports.insert("LookupAccountNameW".into(), addr); addr += 0x100;
    exports.insert("GetUserNameA".into(), addr); addr += 0x100;
    exports.insert("GetUserNameW".into(), addr); addr += 0x100;
    exports.insert("GetSecurityDescriptorDacl".into(), addr); addr += 0x100;
    exports.insert("SetSecurityDescriptorDacl".into(), addr); addr += 0x100;
    exports.insert("InitializeSecurityDescriptor".into(), addr); addr += 0x100;
    exports.insert("MakeAbsoluteSD".into(), addr); addr += 0x100;
    exports.insert("MakeSelfRelativeSD".into(), addr); addr += 0x100;
    exports.insert("IsValidSecurityDescriptor".into(), addr); addr += 0x100;
    exports.insert("GetSecurityInfo".into(), addr); addr += 0x100;
    exports.insert("SetSecurityInfo".into(), addr); addr += 0x100;
    exports.insert("ConvertSidToStringSidA".into(), addr); addr += 0x100;
    exports.insert("ConvertSidToStringSidW".into(), addr); addr += 0x100;
    exports.insert("ConvertStringSidToSidA".into(), addr); addr += 0x100;
    exports.insert("ConvertStringSidToSidW".into(), addr); addr += 0x100;
    exports.insert("AllocateAndInitializeSid".into(), addr); addr += 0x100;
    exports.insert("FreeSid".into(), addr); addr += 0x100;
    exports.insert("EqualSid".into(), addr); addr += 0x100;
    exports.insert("CopySid".into(), addr); addr += 0x100;
    exports.insert("GetLengthSid".into(), addr); addr += 0x100;
    exports.insert("IsValidSid".into(), addr); addr += 0x100;
    exports.insert("AccessCheck".into(), addr); addr += 0x100;
    exports.insert("PrivilegeCheck".into(), addr); addr += 0x100;
    exports.insert("ImpersonateLoggedOnUser".into(), addr); addr += 0x100;
    exports.insert("RevertToSelf".into(), addr); addr += 0x100;
    exports.insert("LogonUserA".into(), addr); addr += 0x100;
    exports.insert("LogonUserW".into(), addr); addr += 0x100;
    exports.insert("DuplicateToken".into(), addr); addr += 0x100;
    exports.insert("DuplicateTokenEx".into(), addr); addr += 0x100;
    exports.insert("CreateRestrictedToken".into(), addr); addr += 0x100;

    // Cryptography functions
    exports.insert("CryptAcquireContextA".into(), addr); addr += 0x100;
    exports.insert("CryptAcquireContextW".into(), addr); addr += 0x100;
    exports.insert("CryptReleaseContext".into(), addr); addr += 0x100;
    exports.insert("CryptGenRandom".into(), addr); addr += 0x100;
    exports.insert("CryptGenKey".into(), addr); addr += 0x100;
    exports.insert("CryptDestroyKey".into(), addr); addr += 0x100;
    exports.insert("CryptDuplicateKey".into(), addr); addr += 0x100;
    exports.insert("CryptExportKey".into(), addr); addr += 0x100;
    exports.insert("CryptImportKey".into(), addr); addr += 0x100;
    exports.insert("CryptGetKeyParam".into(), addr); addr += 0x100;
    exports.insert("CryptSetKeyParam".into(), addr); addr += 0x100;
    exports.insert("CryptDeriveKey".into(), addr); addr += 0x100;
    exports.insert("CryptEncrypt".into(), addr); addr += 0x100;
    exports.insert("CryptDecrypt".into(), addr); addr += 0x100;
    exports.insert("CryptCreateHash".into(), addr); addr += 0x100;
    exports.insert("CryptDestroyHash".into(), addr); addr += 0x100;
    exports.insert("CryptDuplicateHash".into(), addr); addr += 0x100;
    exports.insert("CryptHashData".into(), addr); addr += 0x100;
    exports.insert("CryptHashSessionKey".into(), addr); addr += 0x100;
    exports.insert("CryptGetHashParam".into(), addr); addr += 0x100;
    exports.insert("CryptSetHashParam".into(), addr); addr += 0x100;
    exports.insert("CryptSignHashA".into(), addr); addr += 0x100;
    exports.insert("CryptSignHashW".into(), addr); addr += 0x100;
    exports.insert("CryptVerifySignatureA".into(), addr); addr += 0x100;
    exports.insert("CryptVerifySignatureW".into(), addr); addr += 0x100;
    exports.insert("CryptGetDefaultProviderA".into(), addr); addr += 0x100;
    exports.insert("CryptGetDefaultProviderW".into(), addr); addr += 0x100;
    exports.insert("CryptSetProviderA".into(), addr); addr += 0x100;
    exports.insert("CryptSetProviderW".into(), addr); addr += 0x100;
    exports.insert("CryptEnumProvidersA".into(), addr); addr += 0x100;
    exports.insert("CryptEnumProvidersW".into(), addr); addr += 0x100;
    exports.insert("CryptEnumProviderTypesA".into(), addr); addr += 0x100;
    exports.insert("CryptEnumProviderTypesW".into(), addr); addr += 0x100;
    exports.insert("CryptContextAddRef".into(), addr); addr += 0x100;
    exports.insert("CryptGetProvParam".into(), addr); addr += 0x100;
    exports.insert("CryptSetProvParam".into(), addr); addr += 0x100;
    exports.insert("CryptGetUserKey".into(), addr); addr += 0x100;

    // Event logging
    exports.insert("RegisterEventSourceA".into(), addr); addr += 0x100;
    exports.insert("RegisterEventSourceW".into(), addr); addr += 0x100;
    exports.insert("DeregisterEventSource".into(), addr); addr += 0x100;
    exports.insert("ReportEventA".into(), addr); addr += 0x100;
    exports.insert("ReportEventW".into(), addr); addr += 0x100;
    exports.insert("OpenEventLogA".into(), addr); addr += 0x100;
    exports.insert("OpenEventLogW".into(), addr); addr += 0x100;
    exports.insert("ClearEventLogA".into(), addr); addr += 0x100;
    exports.insert("ClearEventLogW".into(), addr); addr += 0x100;
    exports.insert("CloseEventLog".into(), addr); addr += 0x100;
    exports.insert("ReadEventLogA".into(), addr); addr += 0x100;
    exports.insert("ReadEventLogW".into(), addr); addr += 0x100;
    exports.insert("GetNumberOfEventLogRecords".into(), addr); addr += 0x100;
    exports.insert("GetOldestEventLogRecord".into(), addr); addr += 0x100;
    exports.insert("BackupEventLogA".into(), addr); addr += 0x100;
    exports.insert("BackupEventLogW".into(), addr); addr += 0x100;

    // Service Control Manager
    exports.insert("OpenSCManagerA".into(), addr); addr += 0x100;
    exports.insert("OpenSCManagerW".into(), addr); addr += 0x100;
    exports.insert("OpenServiceA".into(), addr); addr += 0x100;
    exports.insert("OpenServiceW".into(), addr); addr += 0x100;
    exports.insert("CloseServiceHandle".into(), addr); addr += 0x100;
    exports.insert("CreateServiceA".into(), addr); addr += 0x100;
    exports.insert("CreateServiceW".into(), addr); addr += 0x100;
    exports.insert("DeleteService".into(), addr); addr += 0x100;
    exports.insert("StartServiceA".into(), addr); addr += 0x100;
    exports.insert("StartServiceW".into(), addr); addr += 0x100;
    exports.insert("ControlService".into(), addr); addr += 0x100;
    exports.insert("QueryServiceStatus".into(), addr); addr += 0x100;
    exports.insert("QueryServiceStatusEx".into(), addr); addr += 0x100;
    exports.insert("QueryServiceConfigA".into(), addr); addr += 0x100;
    exports.insert("QueryServiceConfigW".into(), addr); addr += 0x100;
    exports.insert("ChangeServiceConfigA".into(), addr); addr += 0x100;
    exports.insert("ChangeServiceConfigW".into(), addr); addr += 0x100;
    exports.insert("EnumServicesStatusA".into(), addr); addr += 0x100;
    exports.insert("EnumServicesStatusW".into(), addr); addr += 0x100;
    exports.insert("EnumDependentServicesA".into(), addr); addr += 0x100;
    exports.insert("EnumDependentServicesW".into(), addr); addr += 0x100;
    exports.insert("GetServiceDisplayNameA".into(), addr); addr += 0x100;
    exports.insert("GetServiceDisplayNameW".into(), addr); addr += 0x100;
    exports.insert("GetServiceKeyNameA".into(), addr); addr += 0x100;
    exports.insert("GetServiceKeyNameW".into(), addr); addr += 0x100;
    exports.insert("LockServiceDatabase".into(), addr); addr += 0x100;
    exports.insert("UnlockServiceDatabase".into(), addr); addr += 0x100;
    exports.insert("QueryServiceLockStatusA".into(), addr); addr += 0x100;
    exports.insert("QueryServiceLockStatusW".into(), addr); addr += 0x100;
    exports.insert("SetServiceStatus".into(), addr); addr += 0x100;
    exports.insert("RegisterServiceCtrlHandlerA".into(), addr); addr += 0x100;
    exports.insert("RegisterServiceCtrlHandlerW".into(), addr); addr += 0x100;
    exports.insert("RegisterServiceCtrlHandlerExA".into(), addr); addr += 0x100;
    exports.insert("RegisterServiceCtrlHandlerExW".into(), addr); addr += 0x100;
    exports.insert("StartServiceCtrlDispatcherA".into(), addr); addr += 0x100;
    exports.insert("StartServiceCtrlDispatcherW".into(), addr); addr += 0x100;
    exports.insert("SetServiceObjectSecurity".into(), addr); addr += 0x100;
    exports.insert("QueryServiceObjectSecurity".into(), addr); addr += 0x100;
    exports.insert("NotifyServiceStatusChangeA".into(), addr); addr += 0x100;
    exports.insert("NotifyServiceStatusChangeW".into(), addr); addr += 0x100;

    // Misc
    exports.insert("GetCurrentHwProfileA".into(), addr); addr += 0x100;
    exports.insert("GetCurrentHwProfileW".into(), addr); addr += 0x100;
    exports.insert("SystemFunction036".into(), addr); // RtlGenRandom

    exports
}
