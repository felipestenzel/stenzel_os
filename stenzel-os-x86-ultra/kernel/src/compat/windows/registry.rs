//! Windows Registry Emulation
//!
//! Provides registry-like API backed by filesystem storage.
//! Registry hives are stored as directories, keys as subdirectories,
//! and values as files.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

/// Registry root keys
pub mod hkey {
    pub const CLASSES_ROOT: u64 = 0x80000000;
    pub const CURRENT_USER: u64 = 0x80000001;
    pub const LOCAL_MACHINE: u64 = 0x80000002;
    pub const USERS: u64 = 0x80000003;
    pub const CURRENT_CONFIG: u64 = 0x80000005;
}

/// Registry value types
pub mod reg_type {
    pub const REG_NONE: u32 = 0;
    pub const REG_SZ: u32 = 1;           // String
    pub const REG_EXPAND_SZ: u32 = 2;    // Expandable string
    pub const REG_BINARY: u32 = 3;       // Binary data
    pub const REG_DWORD: u32 = 4;        // 32-bit number (little-endian)
    pub const REG_DWORD_BE: u32 = 5;     // 32-bit number (big-endian)
    pub const REG_LINK: u32 = 6;         // Symbolic link
    pub const REG_MULTI_SZ: u32 = 7;     // Multiple strings
    pub const REG_QWORD: u32 = 11;       // 64-bit number
}

/// Registry access rights
pub mod access {
    pub const KEY_QUERY_VALUE: u32 = 0x0001;
    pub const KEY_SET_VALUE: u32 = 0x0002;
    pub const KEY_CREATE_SUB_KEY: u32 = 0x0004;
    pub const KEY_ENUMERATE_SUB_KEYS: u32 = 0x0008;
    pub const KEY_NOTIFY: u32 = 0x0010;
    pub const KEY_CREATE_LINK: u32 = 0x0020;
    pub const KEY_WOW64_64KEY: u32 = 0x0100;
    pub const KEY_WOW64_32KEY: u32 = 0x0200;
    pub const KEY_READ: u32 = 0x20019;
    pub const KEY_WRITE: u32 = 0x20006;
    pub const KEY_ALL_ACCESS: u32 = 0xF003F;
}

/// Registry value
#[derive(Debug, Clone)]
pub struct RegistryValue {
    pub value_type: u32,
    pub data: Vec<u8>,
}

impl RegistryValue {
    /// Create a string value
    pub fn string(s: &str) -> Self {
        let mut data: Vec<u8> = s.bytes().collect();
        data.push(0); // Null terminator
        Self {
            value_type: reg_type::REG_SZ,
            data,
        }
    }

    /// Create a DWORD value
    pub fn dword(value: u32) -> Self {
        Self {
            value_type: reg_type::REG_DWORD,
            data: value.to_le_bytes().to_vec(),
        }
    }

    /// Create a QWORD value
    pub fn qword(value: u64) -> Self {
        Self {
            value_type: reg_type::REG_QWORD,
            data: value.to_le_bytes().to_vec(),
        }
    }

    /// Create a binary value
    pub fn binary(data: Vec<u8>) -> Self {
        Self {
            value_type: reg_type::REG_BINARY,
            data,
        }
    }

    /// Get as string
    pub fn as_string(&self) -> Option<String> {
        if self.value_type == reg_type::REG_SZ || self.value_type == reg_type::REG_EXPAND_SZ {
            // Remove trailing null
            let len = self.data.iter().position(|&b| b == 0).unwrap_or(self.data.len());
            Some(String::from_utf8_lossy(&self.data[..len]).into_owned())
        } else {
            None
        }
    }

    /// Get as DWORD
    pub fn as_dword(&self) -> Option<u32> {
        if self.value_type == reg_type::REG_DWORD && self.data.len() >= 4 {
            Some(u32::from_le_bytes([
                self.data[0], self.data[1], self.data[2], self.data[3]
            ]))
        } else {
            None
        }
    }

    /// Get as QWORD
    pub fn as_qword(&self) -> Option<u64> {
        if self.value_type == reg_type::REG_QWORD && self.data.len() >= 8 {
            Some(u64::from_le_bytes([
                self.data[0], self.data[1], self.data[2], self.data[3],
                self.data[4], self.data[5], self.data[6], self.data[7]
            ]))
        } else {
            None
        }
    }
}

/// Registry key
#[derive(Debug, Clone)]
pub struct RegistryKey {
    pub name: String,
    pub values: BTreeMap<String, RegistryValue>,
    pub subkeys: BTreeMap<String, RegistryKey>,
}

impl RegistryKey {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            values: BTreeMap::new(),
            subkeys: BTreeMap::new(),
        }
    }

    /// Get a value
    pub fn get_value(&self, name: &str) -> Option<&RegistryValue> {
        self.values.get(name)
    }

    /// Set a value
    pub fn set_value(&mut self, name: &str, value: RegistryValue) {
        self.values.insert(String::from(name), value);
    }

    /// Delete a value
    pub fn delete_value(&mut self, name: &str) -> bool {
        self.values.remove(name).is_some()
    }

    /// Get a subkey
    pub fn get_subkey(&self, name: &str) -> Option<&RegistryKey> {
        self.subkeys.get(&name.to_lowercase())
    }

    /// Get a subkey mutably
    pub fn get_subkey_mut(&mut self, name: &str) -> Option<&mut RegistryKey> {
        self.subkeys.get_mut(&name.to_lowercase())
    }

    /// Create a subkey
    pub fn create_subkey(&mut self, name: &str) -> &mut RegistryKey {
        let lower = name.to_lowercase();
        if !self.subkeys.contains_key(&lower) {
            self.subkeys.insert(lower.clone(), RegistryKey::new(name));
        }
        self.subkeys.get_mut(&lower).unwrap()
    }

    /// Delete a subkey
    pub fn delete_subkey(&mut self, name: &str) -> bool {
        self.subkeys.remove(&name.to_lowercase()).is_some()
    }

    /// Enumerate subkey names
    pub fn enum_subkeys(&self) -> Vec<String> {
        self.subkeys.keys().cloned().collect()
    }

    /// Enumerate value names
    pub fn enum_values(&self) -> Vec<String> {
        self.values.keys().cloned().collect()
    }
}

/// Registry emulator
pub struct Registry {
    /// Registry roots
    hkcr: RegistryKey,  // HKEY_CLASSES_ROOT
    hkcu: RegistryKey,  // HKEY_CURRENT_USER
    hklm: RegistryKey,  // HKEY_LOCAL_MACHINE
    hku: RegistryKey,   // HKEY_USERS
    hkcc: RegistryKey,  // HKEY_CURRENT_CONFIG

    /// Open handles
    handles: BTreeMap<u64, OpenKey>,
    next_handle: u64,
}

#[derive(Debug, Clone)]
struct OpenKey {
    root: u64,
    path: Vec<String>,
}

impl Registry {
    pub fn new() -> Self {
        let mut reg = Self {
            hkcr: RegistryKey::new("HKEY_CLASSES_ROOT"),
            hkcu: RegistryKey::new("HKEY_CURRENT_USER"),
            hklm: RegistryKey::new("HKEY_LOCAL_MACHINE"),
            hku: RegistryKey::new("HKEY_USERS"),
            hkcc: RegistryKey::new("HKEY_CURRENT_CONFIG"),
            handles: BTreeMap::new(),
            next_handle: 0x100,
        };

        // Initialize with some default values
        reg.init_defaults();
        reg
    }

    /// Initialize default registry entries
    fn init_defaults(&mut self) {
        // HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion
        let software = self.hklm.create_subkey("SOFTWARE");
        let microsoft = software.create_subkey("Microsoft");
        let winnt = microsoft.create_subkey("Windows NT");
        let current = winnt.create_subkey("CurrentVersion");

        current.set_value("ProductName", RegistryValue::string("Stenzel OS"));
        current.set_value("CurrentVersion", RegistryValue::string("10.0"));
        current.set_value("CurrentBuild", RegistryValue::string("19045"));
        current.set_value("CurrentBuildNumber", RegistryValue::string("19045"));
        current.set_value("EditionID", RegistryValue::string("Professional"));

        // HKLM\SYSTEM\CurrentControlSet\Control
        let system = self.hklm.create_subkey("SYSTEM");
        let ccs = system.create_subkey("CurrentControlSet");
        let control = ccs.create_subkey("Control");

        let session = control.create_subkey("Session Manager");
        let env = session.create_subkey("Environment");
        env.set_value("TEMP", RegistryValue::string("C:\\Windows\\Temp"));
        env.set_value("TMP", RegistryValue::string("C:\\Windows\\Temp"));
        env.set_value("PATH", RegistryValue::string("C:\\Windows;C:\\Windows\\System32"));

        // HKCU\Software
        let _cu_software = self.hkcu.create_subkey("Software");

        // HKCU\Environment
        let cu_env = self.hkcu.create_subkey("Environment");
        cu_env.set_value("TEMP", RegistryValue::string("C:\\Users\\user\\Temp"));
        cu_env.set_value("TMP", RegistryValue::string("C:\\Users\\user\\Temp"));
    }

    /// Get root key
    fn get_root(&self, hkey: u64) -> Option<&RegistryKey> {
        match hkey {
            hkey::CLASSES_ROOT => Some(&self.hkcr),
            hkey::CURRENT_USER => Some(&self.hkcu),
            hkey::LOCAL_MACHINE => Some(&self.hklm),
            hkey::USERS => Some(&self.hku),
            hkey::CURRENT_CONFIG => Some(&self.hkcc),
            _ => None,
        }
    }

    /// Get root key mutably
    fn get_root_mut(&mut self, hkey: u64) -> Option<&mut RegistryKey> {
        match hkey {
            hkey::CLASSES_ROOT => Some(&mut self.hkcr),
            hkey::CURRENT_USER => Some(&mut self.hkcu),
            hkey::LOCAL_MACHINE => Some(&mut self.hklm),
            hkey::USERS => Some(&mut self.hku),
            hkey::CURRENT_CONFIG => Some(&mut self.hkcc),
            _ => None,
        }
    }

    /// Navigate to a key by path
    fn navigate<'a>(&'a self, root: &'a RegistryKey, path: &str) -> Option<&'a RegistryKey> {
        let parts: Vec<&str> = path.split('\\').filter(|s| !s.is_empty()).collect();
        let mut current = root;

        for part in parts {
            current = current.get_subkey(part)?;
        }

        Some(current)
    }

    /// Navigate to a key by path (mutable)
    fn navigate_mut<'a>(&'a mut self, hkey: u64, path: &str) -> Option<&'a mut RegistryKey> {
        let parts: Vec<String> = path.split('\\')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase())
            .collect();

        let root = self.get_root_mut(hkey)?;
        let mut current = root;

        for part in parts {
            current = current.get_subkey_mut(&part)?;
        }

        Some(current)
    }

    /// Open a registry key
    pub fn open_key(&mut self, hkey: u64, subkey: &str, _access: u32) -> Option<u64> {
        // First check if it's a root key
        let root = if hkey >= hkey::CLASSES_ROOT && hkey <= hkey::CURRENT_CONFIG {
            hkey
        } else if let Some(open) = self.handles.get(&hkey) {
            open.root
        } else {
            return None;
        };

        // Navigate to the key
        let root_key = self.get_root(root)?;
        let full_path = if let Some(open) = self.handles.get(&hkey) {
            let mut path = open.path.join("\\");
            if !path.is_empty() && !subkey.is_empty() {
                path.push('\\');
            }
            path.push_str(subkey);
            path
        } else {
            String::from(subkey)
        };

        let _ = self.navigate(root_key, &full_path)?;

        // Create handle
        let handle = self.next_handle;
        self.next_handle += 1;

        let path: Vec<String> = full_path.split('\\')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        self.handles.insert(handle, OpenKey { root, path });

        Some(handle)
    }

    /// Create a registry key
    pub fn create_key(&mut self, hkey: u64, subkey: &str, _access: u32) -> Option<(u64, bool)> {
        let root = if hkey >= hkey::CLASSES_ROOT && hkey <= hkey::CURRENT_CONFIG {
            hkey
        } else if let Some(open) = self.handles.get(&hkey) {
            open.root
        } else {
            return None;
        };

        // Build full path
        let full_path = if let Some(open) = self.handles.get(&hkey) {
            let mut path = open.path.join("\\");
            if !path.is_empty() && !subkey.is_empty() {
                path.push('\\');
            }
            path.push_str(subkey);
            path
        } else {
            String::from(subkey)
        };

        let parts: Vec<&str> = full_path.split('\\').filter(|s| !s.is_empty()).collect();

        // Create path
        let root_key = self.get_root_mut(root)?;
        let mut current = root_key;
        let mut created = false;

        for part in &parts {
            if current.get_subkey(part).is_none() {
                current.create_subkey(part);
                created = true;
            }
            current = current.get_subkey_mut(part)?;
        }

        // Create handle
        let handle = self.next_handle;
        self.next_handle += 1;

        let path: Vec<String> = parts.iter().map(|s| s.to_string()).collect();
        self.handles.insert(handle, OpenKey { root, path });

        Some((handle, created))
    }

    /// Close a registry key
    pub fn close_key(&mut self, hkey: u64) -> bool {
        self.handles.remove(&hkey).is_some()
    }

    /// Query a value
    pub fn query_value(&self, hkey: u64, value_name: &str) -> Option<&RegistryValue> {
        let open = self.handles.get(&hkey)?;
        let root = self.get_root(open.root)?;
        let key = self.navigate(root, &open.path.join("\\"))?;
        key.get_value(value_name)
    }

    /// Set a value
    pub fn set_value(&mut self, hkey: u64, value_name: &str, value: RegistryValue) -> bool {
        let open = if let Some(o) = self.handles.get(&hkey) {
            o.clone()
        } else {
            return false;
        };

        if let Some(key) = self.navigate_mut(open.root, &open.path.join("\\")) {
            key.set_value(value_name, value);
            true
        } else {
            false
        }
    }

    /// Delete a value
    pub fn delete_value(&mut self, hkey: u64, value_name: &str) -> bool {
        let open = if let Some(o) = self.handles.get(&hkey) {
            o.clone()
        } else {
            return false;
        };

        if let Some(key) = self.navigate_mut(open.root, &open.path.join("\\")) {
            key.delete_value(value_name)
        } else {
            false
        }
    }

    /// Enumerate subkeys
    pub fn enum_key(&self, hkey: u64, index: u32) -> Option<String> {
        let open = self.handles.get(&hkey)?;
        let root = self.get_root(open.root)?;
        let key = self.navigate(root, &open.path.join("\\"))?;
        let subkeys = key.enum_subkeys();
        subkeys.get(index as usize).cloned()
    }

    /// Enumerate values
    pub fn enum_value(&self, hkey: u64, index: u32) -> Option<(String, RegistryValue)> {
        let open = self.handles.get(&hkey)?;
        let root = self.get_root(open.root)?;
        let key = self.navigate(root, &open.path.join("\\"))?;
        let values: Vec<_> = key.values.iter().collect();
        values.get(index as usize).map(|(k, v)| ((*k).clone(), (*v).clone()))
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global registry instance
static mut REGISTRY: Option<Registry> = None;

/// Initialize registry
pub fn init() {
    unsafe {
        REGISTRY = Some(Registry::new());
    }
    crate::kprintln!("winreg: registry emulation initialized");
}

/// Get registry instance
pub fn registry() -> &'static mut Registry {
    unsafe {
        REGISTRY.as_mut().expect("Registry not initialized")
    }
}
