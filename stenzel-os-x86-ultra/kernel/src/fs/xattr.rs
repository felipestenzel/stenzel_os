//! Extended Attributes (xattr) support
//!
//! Implementation of POSIX Extended Attributes for the VFS.
//! Supports user, system, trusted, and security namespaces.
//!
//! References:
//! - POSIX.1e draft standard
//! - Linux xattr(7) man page
//! - FreeBSD extattr(2) man page

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

use spin::RwLock;

use crate::util::{KResult, KError};
use crate::security::{Cred, Uid, Gid};

// Maximum sizes for extended attributes
pub const XATTR_NAME_MAX: usize = 255;
pub const XATTR_SIZE_MAX: usize = 65536;  // 64KB max value size
pub const XATTR_LIST_MAX: usize = 65536;  // 64KB max list size

/// Extended attribute namespace
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum XattrNamespace {
    /// User-defined attributes (user.*)
    /// Accessible to regular users who can read the file
    User,
    /// System attributes (system.*)
    /// Used for POSIX ACLs and other system data
    System,
    /// Trusted attributes (trusted.*)
    /// Only accessible to processes with CAP_SYS_ADMIN
    Trusted,
    /// Security attributes (security.*)
    /// Used by security modules like SELinux
    Security,
}

impl XattrNamespace {
    /// Parse namespace from attribute name prefix
    pub fn from_name(name: &str) -> Option<(Self, &str)> {
        if let Some(suffix) = name.strip_prefix("user.") {
            Some((Self::User, suffix))
        } else if let Some(suffix) = name.strip_prefix("system.") {
            Some((Self::System, suffix))
        } else if let Some(suffix) = name.strip_prefix("trusted.") {
            Some((Self::Trusted, suffix))
        } else if let Some(suffix) = name.strip_prefix("security.") {
            Some((Self::Security, suffix))
        } else {
            None
        }
    }

    /// Get namespace prefix
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::User => "user.",
            Self::System => "system.",
            Self::Trusted => "trusted.",
            Self::Security => "security.",
        }
    }

    /// Build full attribute name from namespace and suffix
    pub fn full_name(&self, suffix: &str) -> String {
        format!("{}{}", self.prefix(), suffix)
    }
}

/// Flags for setxattr operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XattrFlags(u32);

impl XattrFlags {
    /// Create xattr (fail if exists)
    pub const XATTR_CREATE: u32 = 0x1;
    /// Replace xattr (fail if doesn't exist)
    pub const XATTR_REPLACE: u32 = 0x2;

    pub const fn new(flags: u32) -> Self {
        Self(flags)
    }

    pub fn is_create(&self) -> bool {
        self.0 & Self::XATTR_CREATE != 0
    }

    pub fn is_replace(&self) -> bool {
        self.0 & Self::XATTR_REPLACE != 0
    }

    pub fn bits(&self) -> u32 {
        self.0
    }
}

/// Extended attribute entry
#[derive(Debug, Clone)]
pub struct XattrEntry {
    /// Attribute value
    pub value: Vec<u8>,
}

impl XattrEntry {
    pub fn new(value: Vec<u8>) -> Self {
        Self { value }
    }

    pub fn len(&self) -> usize {
        self.value.len()
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}

/// In-memory xattr storage for a single inode
#[derive(Debug, Clone, Default)]
pub struct XattrStorage {
    /// Attributes stored by full name
    attrs: BTreeMap<String, XattrEntry>,
}

impl XattrStorage {
    pub fn new() -> Self {
        Self {
            attrs: BTreeMap::new(),
        }
    }

    /// Get an extended attribute value
    pub fn get(&self, name: &str) -> Option<&XattrEntry> {
        self.attrs.get(name)
    }

    /// Set an extended attribute
    pub fn set(&mut self, name: &str, value: Vec<u8>, flags: XattrFlags) -> KResult<()> {
        // Validate name length
        if name.len() > XATTR_NAME_MAX {
            return Err(KError::Invalid);
        }

        // Validate value size
        if value.len() > XATTR_SIZE_MAX {
            return Err(KError::OutOfRange);
        }

        let exists = self.attrs.contains_key(name);

        // Check flags
        if flags.is_create() && exists {
            return Err(KError::AlreadyExists);
        }
        if flags.is_replace() && !exists {
            return Err(KError::NotFound);
        }

        self.attrs.insert(name.to_string(), XattrEntry::new(value));
        Ok(())
    }

    /// Remove an extended attribute
    pub fn remove(&mut self, name: &str) -> KResult<()> {
        if self.attrs.remove(name).is_some() {
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    /// List all extended attribute names
    pub fn list(&self) -> Vec<String> {
        self.attrs.keys().cloned().collect()
    }

    /// List attributes in a specific namespace
    pub fn list_namespace(&self, ns: XattrNamespace) -> Vec<String> {
        let prefix = ns.prefix();
        self.attrs
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }

    /// Get total size of all attribute names (for listxattr)
    pub fn list_size(&self) -> usize {
        self.attrs.keys().map(|k| k.len() + 1).sum() // +1 for null terminator
    }

    /// Check if storage has any attributes
    pub fn is_empty(&self) -> bool {
        self.attrs.is_empty()
    }

    /// Get number of attributes
    pub fn count(&self) -> usize {
        self.attrs.len()
    }

    /// Clear all attributes
    pub fn clear(&mut self) {
        self.attrs.clear();
    }
}

/// Thread-safe xattr storage
#[derive(Debug, Default)]
pub struct SyncXattrStorage {
    inner: RwLock<XattrStorage>,
}

impl SyncXattrStorage {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(XattrStorage::new()),
        }
    }

    pub fn get(&self, name: &str) -> Option<Vec<u8>> {
        self.inner.read().get(name).map(|e| e.value.clone())
    }

    pub fn set(&self, name: &str, value: Vec<u8>, flags: XattrFlags) -> KResult<()> {
        self.inner.write().set(name, value, flags)
    }

    pub fn remove(&self, name: &str) -> KResult<()> {
        self.inner.write().remove(name)
    }

    pub fn list(&self) -> Vec<String> {
        self.inner.read().list()
    }

    pub fn list_namespace(&self, ns: XattrNamespace) -> Vec<String> {
        self.inner.read().list_namespace(ns)
    }

    pub fn list_size(&self) -> usize {
        self.inner.read().list_size()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.read().is_empty()
    }

    pub fn count(&self) -> usize {
        self.inner.read().count()
    }

    pub fn clear(&self) {
        self.inner.write().clear();
    }
}

impl Clone for SyncXattrStorage {
    fn clone(&self) -> Self {
        Self {
            inner: RwLock::new(self.inner.read().clone()),
        }
    }
}

/// Check permission to access extended attribute
pub fn check_xattr_permission(
    cred: &Cred,
    file_uid: Uid,
    file_gid: Gid,
    file_mode: u16,
    namespace: XattrNamespace,
    is_write: bool,
) -> KResult<()> {
    // Root can always access
    if cred.uid == Uid(0) {
        return Ok(());
    }

    match namespace {
        XattrNamespace::User => {
            // User namespace requires read/write permission on the file
            if is_write {
                // Need write permission
                if cred.uid == file_uid {
                    if file_mode & 0o200 == 0 {
                        return Err(KError::PermissionDenied);
                    }
                } else if cred.gid == file_gid {
                    if file_mode & 0o020 == 0 {
                        return Err(KError::PermissionDenied);
                    }
                } else if file_mode & 0o002 == 0 {
                    return Err(KError::PermissionDenied);
                }
            } else {
                // Need read permission
                if cred.uid == file_uid {
                    if file_mode & 0o400 == 0 {
                        return Err(KError::PermissionDenied);
                    }
                } else if cred.gid == file_gid {
                    if file_mode & 0o040 == 0 {
                        return Err(KError::PermissionDenied);
                    }
                } else if file_mode & 0o004 == 0 {
                    return Err(KError::PermissionDenied);
                }
            }
            Ok(())
        }
        XattrNamespace::Trusted | XattrNamespace::Security => {
            // Only root can access trusted and security namespaces
            Err(KError::PermissionDenied)
        }
        XattrNamespace::System => {
            // System namespace (ACLs) has special rules
            // For now, allow root only for writes
            if is_write {
                Err(KError::PermissionDenied)
            } else {
                // Read follows file permissions
                if cred.uid == file_uid {
                    if file_mode & 0o400 == 0 {
                        return Err(KError::PermissionDenied);
                    }
                } else if cred.gid == file_gid {
                    if file_mode & 0o040 == 0 {
                        return Err(KError::PermissionDenied);
                    }
                } else if file_mode & 0o004 == 0 {
                    return Err(KError::PermissionDenied);
                }
                Ok(())
            }
        }
    }
}

/// Well-known system attribute names
pub mod system_attrs {
    /// POSIX ACL access
    pub const POSIX_ACL_ACCESS: &str = "system.posix_acl_access";
    /// POSIX ACL default (for directories)
    pub const POSIX_ACL_DEFAULT: &str = "system.posix_acl_default";
}

/// Well-known security attribute names
pub mod security_attrs {
    /// SELinux security context
    pub const SELINUX: &str = "security.selinux";
    /// AppArmor profile
    pub const APPARMOR: &str = "security.apparmor";
    /// Capability attribute
    pub const CAPABILITY: &str = "security.capability";
    /// IMA measurement
    pub const IMA: &str = "security.ima";
    /// EVM signature
    pub const EVM: &str = "security.evm";
}

/// Well-known trusted attribute names
pub mod trusted_attrs {
    /// Overlay filesystem opaque marker
    pub const OVERLAY_OPAQUE: &str = "trusted.overlay.opaque";
    /// Overlay filesystem redirect
    pub const OVERLAY_REDIRECT: &str = "trusted.overlay.redirect";
}

// ============================================================================
// Syscall implementations
// ============================================================================

use crate::fs::vfs::{Inode, Vfs, InodeKind};
use crate::sync::IrqSafeMutex;

/// Global VFS instance for xattr operations
static mut VFS_INSTANCE: Option<*mut Vfs> = None;

/// Initialize xattr subsystem with VFS reference
pub fn init(vfs: &mut Vfs) {
    unsafe {
        VFS_INSTANCE = Some(vfs as *mut Vfs);
    }
}

/// Get extended attribute value (follows symlinks)
pub fn getxattr(cred: &Cred, path: &str, name: &str, buf: &mut [u8]) -> KResult<usize> {
    // Validate name
    let (namespace, _suffix) = XattrNamespace::from_name(name)
        .ok_or(KError::NotSupported)?;

    // Resolve path (follows symlinks)
    let vfs = unsafe { VFS_INSTANCE.as_ref().ok_or(KError::Invalid)? };
    let vfs = unsafe { &**vfs };
    let inode = vfs.resolve(path, cred)?;

    // Check permission
    let meta = inode.metadata();
    check_xattr_permission(cred, meta.uid, meta.gid, meta.mode.bits(), namespace, false)?;

    // Get attribute
    let value = inode.0.getxattr(name)?;

    if buf.is_empty() {
        // Return size only
        return Ok(value.len());
    }

    if value.len() > buf.len() {
        return Err(KError::OutOfRange);
    }

    buf[..value.len()].copy_from_slice(&value);
    Ok(value.len())
}

/// Get extended attribute value (doesn't follow symlinks)
pub fn lgetxattr(cred: &Cred, path: &str, name: &str, buf: &mut [u8]) -> KResult<usize> {
    // For now, same as getxattr (proper implementation would not follow final symlink)
    getxattr(cred, path, name, buf)
}

/// Get extended attribute on file descriptor
pub fn fgetxattr(_cred: &Cred, _fd: i32, _name: &str, _buf: &mut [u8]) -> KResult<usize> {
    // TODO: Implement when we have proper fd->inode mapping
    Err(KError::NotSupported)
}

/// Set extended attribute value (follows symlinks)
pub fn setxattr(cred: &Cred, path: &str, name: &str, value: &[u8], flags: u32) -> KResult<()> {
    // Validate name
    let (namespace, _suffix) = XattrNamespace::from_name(name)
        .ok_or(KError::NotSupported)?;

    // Validate value size
    if value.len() > XATTR_SIZE_MAX {
        return Err(KError::OutOfRange);
    }

    // Resolve path
    let vfs = unsafe { VFS_INSTANCE.as_ref().ok_or(KError::Invalid)? };
    let vfs = unsafe { &**vfs };
    let inode = vfs.resolve(path, cred)?;

    // Check permission
    let meta = inode.metadata();
    check_xattr_permission(cred, meta.uid, meta.gid, meta.mode.bits(), namespace, true)?;

    // Set attribute
    inode.0.setxattr(name, value.to_vec(), XattrFlags::new(flags))
}

/// Set extended attribute value (doesn't follow symlinks)
pub fn lsetxattr(cred: &Cred, path: &str, name: &str, value: &[u8], flags: u32) -> KResult<()> {
    // For now, same as setxattr
    setxattr(cred, path, name, value, flags)
}

/// Set extended attribute on file descriptor
pub fn fsetxattr(_cred: &Cred, _fd: i32, _name: &str, _value: &[u8], _flags: u32) -> KResult<()> {
    // TODO: Implement when we have proper fd->inode mapping
    Err(KError::NotSupported)
}

/// Remove extended attribute (follows symlinks)
pub fn removexattr(cred: &Cred, path: &str, name: &str) -> KResult<()> {
    // Validate name
    let (namespace, _suffix) = XattrNamespace::from_name(name)
        .ok_or(KError::NotSupported)?;

    // Resolve path
    let vfs = unsafe { VFS_INSTANCE.as_ref().ok_or(KError::Invalid)? };
    let vfs = unsafe { &**vfs };
    let inode = vfs.resolve(path, cred)?;

    // Check permission
    let meta = inode.metadata();
    check_xattr_permission(cred, meta.uid, meta.gid, meta.mode.bits(), namespace, true)?;

    // Remove attribute
    inode.0.removexattr(name)
}

/// Remove extended attribute (doesn't follow symlinks)
pub fn lremovexattr(cred: &Cred, path: &str, name: &str) -> KResult<()> {
    // For now, same as removexattr
    removexattr(cred, path, name)
}

/// Remove extended attribute on file descriptor
pub fn fremovexattr(_cred: &Cred, _fd: i32, _name: &str) -> KResult<()> {
    // TODO: Implement when we have proper fd->inode mapping
    Err(KError::NotSupported)
}

/// List extended attributes (follows symlinks)
pub fn listxattr(cred: &Cred, path: &str, buf: &mut [u8]) -> KResult<usize> {
    // Resolve path
    let vfs = unsafe { VFS_INSTANCE.as_ref().ok_or(KError::Invalid)? };
    let vfs = unsafe { &**vfs };
    let inode = vfs.resolve(path, cred)?;

    // Get list of attributes
    let names = inode.0.listxattr()?;

    // Calculate total size needed
    let total_size: usize = names.iter().map(|n| n.len() + 1).sum();

    if buf.is_empty() {
        // Return size only
        return Ok(total_size);
    }

    if total_size > buf.len() {
        return Err(KError::OutOfRange);
    }

    // Copy names with null terminators
    let mut offset = 0;
    for name in names {
        // Filter based on permissions
        if let Some((namespace, _)) = XattrNamespace::from_name(&name) {
            let meta = inode.metadata();
            if check_xattr_permission(cred, meta.uid, meta.gid, meta.mode.bits(), namespace, false).is_ok() {
                buf[offset..offset + name.len()].copy_from_slice(name.as_bytes());
                offset += name.len();
                buf[offset] = 0;
                offset += 1;
            }
        }
    }

    Ok(offset)
}

/// List extended attributes (doesn't follow symlinks)
pub fn llistxattr(cred: &Cred, path: &str, buf: &mut [u8]) -> KResult<usize> {
    // For now, same as listxattr
    listxattr(cred, path, buf)
}

/// List extended attributes on file descriptor
pub fn flistxattr(_cred: &Cred, _fd: i32, _buf: &mut [u8]) -> KResult<usize> {
    // TODO: Implement when we have proper fd->inode mapping
    Err(KError::NotSupported)
}

// ============================================================================
// Convenience functions
// ============================================================================

/// Set a user extended attribute
pub fn set_user_xattr(inode: &Inode, name: &str, value: &[u8]) -> KResult<()> {
    let full_name = XattrNamespace::User.full_name(name);
    inode.0.setxattr(&full_name, value.to_vec(), XattrFlags::new(0))
}

/// Get a user extended attribute
pub fn get_user_xattr(inode: &Inode, name: &str) -> KResult<Vec<u8>> {
    let full_name = XattrNamespace::User.full_name(name);
    inode.0.getxattr(&full_name)
}

/// Remove a user extended attribute
pub fn remove_user_xattr(inode: &Inode, name: &str) -> KResult<()> {
    let full_name = XattrNamespace::User.full_name(name);
    inode.0.removexattr(&full_name)
}

/// Set a security extended attribute (requires root)
pub fn set_security_xattr(inode: &Inode, name: &str, value: &[u8]) -> KResult<()> {
    let full_name = XattrNamespace::Security.full_name(name);
    inode.0.setxattr(&full_name, value.to_vec(), XattrFlags::new(0))
}

/// Get a security extended attribute
pub fn get_security_xattr(inode: &Inode, name: &str) -> KResult<Vec<u8>> {
    let full_name = XattrNamespace::Security.full_name(name);
    inode.0.getxattr(&full_name)
}

/// Copy extended attributes from one inode to another
pub fn copy_xattrs(src: &Inode, dst: &Inode) -> KResult<()> {
    let names = src.0.listxattr()?;

    for name in names {
        if let Ok(value) = src.0.getxattr(&name) {
            // Ignore errors on copy (some attrs may not be copyable)
            let _ = dst.0.setxattr(&name, value, XattrFlags::new(0));
        }
    }

    Ok(())
}

// ============================================================================
// Debug / Status
// ============================================================================

/// Format xattr info for an inode
pub fn format_xattr_info(inode: &Inode) -> String {
    let names = match inode.0.listxattr() {
        Ok(n) => n,
        Err(_) => return String::from("xattr: not supported"),
    };

    if names.is_empty() {
        return String::from("xattr: (none)");
    }

    let mut result = format!("xattr: {} attributes\n", names.len());

    for name in names {
        if let Ok(value) = inode.0.getxattr(&name) {
            result.push_str(&format!("  {}: {} bytes\n", name, value.len()));
        }
    }

    result
}

/// Format xattr status
pub fn format_status() -> String {
    format!(
        "Extended Attributes:\n\
         - Max name size: {} bytes\n\
         - Max value size: {} bytes\n\
         - Max list size: {} bytes\n\
         - Namespaces: user, system, trusted, security\n",
        XATTR_NAME_MAX,
        XATTR_SIZE_MAX,
        XATTR_LIST_MAX
    )
}
