//! POSIX Access Control Lists (ACLs)
//!
//! Implementation of POSIX.1e Access Control Lists for fine-grained
//! permission control beyond traditional Unix rwx permissions.
//!
//! References:
//! - POSIX.1e draft standard (withdrawn but widely implemented)
//! - IEEE Std 1003.1e/1003.2c
//! - Linux acl(5) man page

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::{String, ToString};
use alloc::format;

use crate::util::{KResult, KError};
use crate::security::{Cred, Uid, Gid};
use super::xattr::{self, XattrFlags};
use super::vfs::Inode;

/// ACL version constant
pub const ACL_VERSION: u32 = 0x0002;

/// Maximum number of ACL entries
pub const ACL_MAX_ENTRIES: usize = 32;

/// ACL tag types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum AclTag {
    /// ACL_USER_OBJ - permissions for file owner
    UserObj = 0x01,
    /// ACL_USER - permissions for specified user
    User = 0x02,
    /// ACL_GROUP_OBJ - permissions for file group
    GroupObj = 0x04,
    /// ACL_GROUP - permissions for specified group
    Group = 0x08,
    /// ACL_MASK - mask for group class permissions
    Mask = 0x10,
    /// ACL_OTHER - permissions for others
    Other = 0x20,
}

impl AclTag {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0x01 => Some(Self::UserObj),
            0x02 => Some(Self::User),
            0x04 => Some(Self::GroupObj),
            0x08 => Some(Self::Group),
            0x10 => Some(Self::Mask),
            0x20 => Some(Self::Other),
            _ => None,
        }
    }

    pub fn to_u16(self) -> u16 {
        self as u16
    }

    /// Check if this tag type requires a qualifier (uid/gid)
    pub fn requires_qualifier(self) -> bool {
        matches!(self, Self::User | Self::Group)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::UserObj => "user",
            Self::User => "user",
            Self::GroupObj => "group",
            Self::Group => "group",
            Self::Mask => "mask",
            Self::Other => "other",
        }
    }
}

/// ACL permission bits
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AclPerm(u16);

impl AclPerm {
    pub const READ: u16 = 0x04;    // r
    pub const WRITE: u16 = 0x02;   // w
    pub const EXECUTE: u16 = 0x01; // x

    pub const fn new(bits: u16) -> Self {
        Self(bits & 0x07)
    }

    pub fn from_mode(mode: u16) -> Self {
        Self::new(mode & 0x07)
    }

    pub fn bits(&self) -> u16 {
        self.0
    }

    pub fn can_read(&self) -> bool {
        self.0 & Self::READ != 0
    }

    pub fn can_write(&self) -> bool {
        self.0 & Self::WRITE != 0
    }

    pub fn can_execute(&self) -> bool {
        self.0 & Self::EXECUTE != 0
    }

    pub fn to_string(&self) -> String {
        let mut s = String::with_capacity(3);
        s.push(if self.can_read() { 'r' } else { '-' });
        s.push(if self.can_write() { 'w' } else { '-' });
        s.push(if self.can_execute() { 'x' } else { '-' });
        s
    }

    /// Apply mask to permissions
    pub fn masked(&self, mask: AclPerm) -> Self {
        Self::new(self.0 & mask.0)
    }
}

/// Single ACL entry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AclEntry {
    /// Entry type
    pub tag: AclTag,
    /// Permissions
    pub perm: AclPerm,
    /// Qualifier (uid for User, gid for Group, undefined for others)
    pub qualifier: u32,
}

impl AclEntry {
    pub fn new(tag: AclTag, perm: AclPerm, qualifier: u32) -> Self {
        Self { tag, perm, qualifier }
    }

    pub fn user_obj(perm: AclPerm) -> Self {
        Self::new(AclTag::UserObj, perm, 0)
    }

    pub fn user(uid: Uid, perm: AclPerm) -> Self {
        Self::new(AclTag::User, perm, uid.0)
    }

    pub fn group_obj(perm: AclPerm) -> Self {
        Self::new(AclTag::GroupObj, perm, 0)
    }

    pub fn group(gid: Gid, perm: AclPerm) -> Self {
        Self::new(AclTag::Group, perm, gid.0)
    }

    pub fn mask(perm: AclPerm) -> Self {
        Self::new(AclTag::Mask, perm, 0)
    }

    pub fn other(perm: AclPerm) -> Self {
        Self::new(AclTag::Other, perm, 0)
    }

    /// Format entry for display (like getfacl output)
    pub fn format(&self) -> String {
        let name = self.tag.name();
        let perm = self.perm.to_string();

        match self.tag {
            AclTag::UserObj => format!("{}::{}", name, perm),
            AclTag::User => format!("{}:{}:{}", name, self.qualifier, perm),
            AclTag::GroupObj => format!("{}::{}", name, perm),
            AclTag::Group => format!("{}:{}:{}", name, self.qualifier, perm),
            AclTag::Mask => format!("{}::{}", name, perm),
            AclTag::Other => format!("{}::{}", name, perm),
        }
    }
}

/// POSIX Access Control List
#[derive(Debug, Clone, Default)]
pub struct Acl {
    entries: Vec<AclEntry>,
}

impl Acl {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Create a minimal ACL from traditional Unix mode
    pub fn from_mode(mode: u16) -> Self {
        let mut acl = Self::new();

        // Owner permissions (bits 8-6)
        acl.add_entry(AclEntry::user_obj(AclPerm::from_mode((mode >> 6) & 0o7)));

        // Group permissions (bits 5-3)
        acl.add_entry(AclEntry::group_obj(AclPerm::from_mode((mode >> 3) & 0o7)));

        // Other permissions (bits 2-0)
        acl.add_entry(AclEntry::other(AclPerm::from_mode(mode & 0o7)));

        acl
    }

    /// Create ACL from mode with explicit owner/group
    pub fn from_mode_with_ids(mode: u16, uid: Uid, gid: Gid) -> Self {
        let _ = uid;
        let _ = gid;
        // For minimal ACL, owner/group IDs aren't stored in entries
        Self::from_mode(mode)
    }

    /// Add an entry to the ACL
    pub fn add_entry(&mut self, entry: AclEntry) -> KResult<()> {
        if self.entries.len() >= ACL_MAX_ENTRIES {
            return Err(KError::OutOfRange);
        }

        // Remove existing entry with same tag and qualifier
        self.entries.retain(|e| {
            !(e.tag == entry.tag &&
              (!entry.tag.requires_qualifier() || e.qualifier == entry.qualifier))
        });

        self.entries.push(entry);
        Ok(())
    }

    /// Remove an entry from the ACL
    pub fn remove_entry(&mut self, tag: AclTag, qualifier: u32) -> KResult<()> {
        let initial_len = self.entries.len();
        self.entries.retain(|e| {
            !(e.tag == tag &&
              (!tag.requires_qualifier() || e.qualifier == qualifier))
        });

        if self.entries.len() == initial_len {
            Err(KError::NotFound)
        } else {
            Ok(())
        }
    }

    /// Get an entry by tag and qualifier
    pub fn get_entry(&self, tag: AclTag, qualifier: u32) -> Option<&AclEntry> {
        self.entries.iter().find(|e| {
            e.tag == tag &&
            (!tag.requires_qualifier() || e.qualifier == qualifier)
        })
    }

    /// Get mutable entry
    pub fn get_entry_mut(&mut self, tag: AclTag, qualifier: u32) -> Option<&mut AclEntry> {
        self.entries.iter_mut().find(|e| {
            e.tag == tag &&
            (!tag.requires_qualifier() || e.qualifier == qualifier)
        })
    }

    /// Get all entries
    pub fn entries(&self) -> &[AclEntry] {
        &self.entries
    }

    /// Get the mask entry
    pub fn mask(&self) -> Option<AclPerm> {
        self.get_entry(AclTag::Mask, 0).map(|e| e.perm)
    }

    /// Check if ACL is minimal (no named users/groups)
    pub fn is_minimal(&self) -> bool {
        !self.entries.iter().any(|e| {
            matches!(e.tag, AclTag::User | AclTag::Group | AclTag::Mask)
        })
    }

    /// Convert ACL back to traditional Unix mode
    pub fn to_mode(&self) -> u16 {
        let mut mode: u16 = 0;

        // Owner permissions
        if let Some(e) = self.get_entry(AclTag::UserObj, 0) {
            mode |= (e.perm.bits() & 0o7) << 6;
        }

        // Group permissions - if mask exists, use mask; otherwise use group_obj
        if let Some(mask) = self.mask() {
            mode |= (mask.bits() & 0o7) << 3;
        } else if let Some(e) = self.get_entry(AclTag::GroupObj, 0) {
            mode |= (e.perm.bits() & 0o7) << 3;
        }

        // Other permissions
        if let Some(e) = self.get_entry(AclTag::Other, 0) {
            mode |= e.perm.bits() & 0o7;
        }

        mode
    }

    /// Validate ACL structure
    pub fn validate(&self) -> KResult<()> {
        let mut has_user_obj = false;
        let mut has_group_obj = false;
        let mut has_other = false;
        let mut has_mask = false;
        let mut has_named = false;

        for entry in &self.entries {
            match entry.tag {
                AclTag::UserObj => {
                    if has_user_obj {
                        return Err(KError::Invalid);
                    }
                    has_user_obj = true;
                }
                AclTag::GroupObj => {
                    if has_group_obj {
                        return Err(KError::Invalid);
                    }
                    has_group_obj = true;
                }
                AclTag::Other => {
                    if has_other {
                        return Err(KError::Invalid);
                    }
                    has_other = true;
                }
                AclTag::Mask => {
                    if has_mask {
                        return Err(KError::Invalid);
                    }
                    has_mask = true;
                }
                AclTag::User | AclTag::Group => {
                    has_named = true;
                }
            }
        }

        // Must have required entries
        if !has_user_obj || !has_group_obj || !has_other {
            return Err(KError::Invalid);
        }

        // Must have mask if any named entries exist
        if has_named && !has_mask {
            return Err(KError::Invalid);
        }

        Ok(())
    }

    /// Calculate effective permissions for mask
    pub fn recalculate_mask(&mut self) {
        // Mask should be union of all group class permissions
        let mut mask_perm: u16 = 0;

        for entry in &self.entries {
            match entry.tag {
                AclTag::GroupObj | AclTag::Group | AclTag::User => {
                    if entry.tag != AclTag::UserObj {
                        mask_perm |= entry.perm.bits();
                    }
                }
                _ => {}
            }
        }

        // Update or add mask entry
        if let Some(mask) = self.get_entry_mut(AclTag::Mask, 0) {
            mask.perm = AclPerm::new(mask_perm);
        } else if mask_perm != 0 {
            let _ = self.add_entry(AclEntry::mask(AclPerm::new(mask_perm)));
        }
    }

    /// Serialize ACL to xattr format
    pub fn to_xattr(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + self.entries.len() * 8);

        // Header: version (little endian)
        buf.extend_from_slice(&ACL_VERSION.to_le_bytes());

        // Entries
        for entry in &self.entries {
            // Tag (2 bytes)
            buf.extend_from_slice(&entry.tag.to_u16().to_le_bytes());
            // Perm (2 bytes)
            buf.extend_from_slice(&entry.perm.bits().to_le_bytes());
            // Qualifier (4 bytes)
            buf.extend_from_slice(&entry.qualifier.to_le_bytes());
        }

        buf
    }

    /// Deserialize ACL from xattr format
    pub fn from_xattr(data: &[u8]) -> KResult<Self> {
        if data.len() < 4 {
            return Err(KError::Invalid);
        }

        // Check version
        let version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if version != ACL_VERSION {
            return Err(KError::NotSupported);
        }

        let entry_data = &data[4..];
        if entry_data.len() % 8 != 0 {
            return Err(KError::Invalid);
        }

        let mut acl = Acl::new();
        let entry_count = entry_data.len() / 8;

        for i in 0..entry_count {
            let offset = i * 8;
            let tag_raw = u16::from_le_bytes([entry_data[offset], entry_data[offset + 1]]);
            let perm_raw = u16::from_le_bytes([entry_data[offset + 2], entry_data[offset + 3]]);
            let qualifier = u32::from_le_bytes([
                entry_data[offset + 4],
                entry_data[offset + 5],
                entry_data[offset + 6],
                entry_data[offset + 7],
            ]);

            let tag = AclTag::from_u16(tag_raw).ok_or(KError::Invalid)?;
            let perm = AclPerm::new(perm_raw);

            acl.add_entry(AclEntry::new(tag, perm, qualifier))?;
        }

        Ok(acl)
    }

    /// Format ACL for display (like getfacl output)
    pub fn format(&self) -> String {
        let mut lines = Vec::new();

        // Sort entries by tag type
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by_key(|e| e.tag.to_u16());

        for entry in sorted {
            lines.push(entry.format());
        }

        lines.join("\n")
    }
}

// ============================================================================
// ACL Permission Checking
// ============================================================================

/// Check access permissions using ACL
pub fn check_acl_access(
    acl: &Acl,
    cred: &Cred,
    file_uid: Uid,
    file_gid: Gid,
    requested: AclPerm,
) -> KResult<()> {
    // Root bypasses all permission checks
    if cred.uid == Uid(0) {
        return Ok(());
    }

    // Step 1: Check if user is owner
    if cred.uid == file_uid {
        if let Some(entry) = acl.get_entry(AclTag::UserObj, 0) {
            if (entry.perm.bits() & requested.bits()) == requested.bits() {
                return Ok(());
            }
            return Err(KError::PermissionDenied);
        }
    }

    // Step 2: Check named user entries
    if let Some(entry) = acl.get_entry(AclTag::User, cred.uid.0) {
        let effective = if let Some(mask) = acl.mask() {
            entry.perm.masked(mask)
        } else {
            entry.perm
        };

        if (effective.bits() & requested.bits()) == requested.bits() {
            return Ok(());
        }
        return Err(KError::PermissionDenied);
    }

    // Step 3: Check group permissions
    let mut group_matched = false;
    let mut group_permitted = false;

    // Check owning group
    if cred.gid == file_gid || cred.groups.contains(&file_gid) {
        group_matched = true;
        if let Some(entry) = acl.get_entry(AclTag::GroupObj, 0) {
            let effective = if let Some(mask) = acl.mask() {
                entry.perm.masked(mask)
            } else {
                entry.perm
            };

            if (effective.bits() & requested.bits()) == requested.bits() {
                group_permitted = true;
            }
        }
    }

    // Check named group entries
    for entry in acl.entries() {
        if entry.tag == AclTag::Group {
            let gid = Gid(entry.qualifier);
            if cred.gid == gid || cred.groups.contains(&gid) {
                group_matched = true;
                let effective = if let Some(mask) = acl.mask() {
                    entry.perm.masked(mask)
                } else {
                    entry.perm
                };

                if (effective.bits() & requested.bits()) == requested.bits() {
                    group_permitted = true;
                }
            }
        }
    }

    if group_matched {
        if group_permitted {
            return Ok(());
        }
        return Err(KError::PermissionDenied);
    }

    // Step 4: Check other entry
    if let Some(entry) = acl.get_entry(AclTag::Other, 0) {
        if (entry.perm.bits() & requested.bits()) == requested.bits() {
            return Ok(());
        }
    }

    Err(KError::PermissionDenied)
}

// ============================================================================
// Default ACLs for directories
// ============================================================================

/// Default ACL for new files/directories created in a directory
#[derive(Debug, Clone, Default)]
pub struct DefaultAcl {
    acl: Acl,
}

impl DefaultAcl {
    pub fn new() -> Self {
        Self { acl: Acl::new() }
    }

    pub fn from_acl(acl: Acl) -> Self {
        Self { acl }
    }

    pub fn acl(&self) -> &Acl {
        &self.acl
    }

    pub fn acl_mut(&mut self) -> &mut Acl {
        &mut self.acl
    }

    /// Inherit ACL for new file creation
    pub fn inherit_for_file(&self, umask: u16) -> Acl {
        let mut acl = self.acl.clone();

        // Apply umask to inherited permissions
        for entry in acl.entries.iter_mut() {
            let umask_bits = match entry.tag {
                AclTag::UserObj => (umask >> 6) & 0o7,
                AclTag::GroupObj | AclTag::Group | AclTag::User => (umask >> 3) & 0o7,
                AclTag::Other => umask & 0o7,
                AclTag::Mask => continue,
            };
            entry.perm = AclPerm::new(entry.perm.bits() & !umask_bits);
        }

        // Recalculate mask
        acl.recalculate_mask();
        acl
    }

    /// Inherit ACL for new directory creation
    pub fn inherit_for_dir(&self, umask: u16) -> (Acl, DefaultAcl) {
        let access_acl = self.inherit_for_file(umask);
        let default_acl = DefaultAcl::from_acl(self.acl.clone());
        (access_acl, default_acl)
    }

    pub fn to_xattr(&self) -> Vec<u8> {
        self.acl.to_xattr()
    }

    pub fn from_xattr(data: &[u8]) -> KResult<Self> {
        Ok(Self { acl: Acl::from_xattr(data)? })
    }
}

// ============================================================================
// Inode ACL operations
// ============================================================================

/// Get access ACL from inode
pub fn get_acl(inode: &Inode) -> KResult<Acl> {
    let data = inode.0.getxattr(xattr::system_attrs::POSIX_ACL_ACCESS)?;
    Acl::from_xattr(&data)
}

/// Set access ACL on inode
pub fn set_acl(inode: &Inode, acl: &Acl) -> KResult<()> {
    acl.validate()?;
    let data = acl.to_xattr();
    inode.0.setxattr(xattr::system_attrs::POSIX_ACL_ACCESS, data, XattrFlags::new(0))
}

/// Remove access ACL from inode (revert to mode-only)
pub fn remove_acl(inode: &Inode) -> KResult<()> {
    inode.0.removexattr(xattr::system_attrs::POSIX_ACL_ACCESS)
}

/// Get default ACL from directory
pub fn get_default_acl(inode: &Inode) -> KResult<DefaultAcl> {
    let data = inode.0.getxattr(xattr::system_attrs::POSIX_ACL_DEFAULT)?;
    DefaultAcl::from_xattr(&data)
}

/// Set default ACL on directory
pub fn set_default_acl(inode: &Inode, acl: &DefaultAcl) -> KResult<()> {
    acl.acl().validate()?;
    let data = acl.to_xattr();
    inode.0.setxattr(xattr::system_attrs::POSIX_ACL_DEFAULT, data, XattrFlags::new(0))
}

/// Remove default ACL from directory
pub fn remove_default_acl(inode: &Inode) -> KResult<()> {
    inode.0.removexattr(xattr::system_attrs::POSIX_ACL_DEFAULT)
}

/// Check if inode has an ACL
pub fn has_acl(inode: &Inode) -> bool {
    inode.0.getxattr(xattr::system_attrs::POSIX_ACL_ACCESS).is_ok()
}

/// Check if directory has default ACL
pub fn has_default_acl(inode: &Inode) -> bool {
    inode.0.getxattr(xattr::system_attrs::POSIX_ACL_DEFAULT).is_ok()
}

// ============================================================================
// Convenience functions
// ============================================================================

/// Grant read access to a user
pub fn grant_user_read(inode: &Inode, uid: Uid) -> KResult<()> {
    let mut acl = get_acl(inode).unwrap_or_else(|_| {
        let meta = inode.metadata();
        Acl::from_mode(meta.mode.bits())
    });

    acl.add_entry(AclEntry::user(uid, AclPerm::new(AclPerm::READ)))?;
    acl.recalculate_mask();
    set_acl(inode, &acl)
}

/// Grant read/write access to a user
pub fn grant_user_rw(inode: &Inode, uid: Uid) -> KResult<()> {
    let mut acl = get_acl(inode).unwrap_or_else(|_| {
        let meta = inode.metadata();
        Acl::from_mode(meta.mode.bits())
    });

    acl.add_entry(AclEntry::user(uid, AclPerm::new(AclPerm::READ | AclPerm::WRITE)))?;
    acl.recalculate_mask();
    set_acl(inode, &acl)
}

/// Grant full access to a user
pub fn grant_user_full(inode: &Inode, uid: Uid) -> KResult<()> {
    let mut acl = get_acl(inode).unwrap_or_else(|_| {
        let meta = inode.metadata();
        Acl::from_mode(meta.mode.bits())
    });

    acl.add_entry(AclEntry::user(uid, AclPerm::new(AclPerm::READ | AclPerm::WRITE | AclPerm::EXECUTE)))?;
    acl.recalculate_mask();
    set_acl(inode, &acl)
}

/// Grant read access to a group
pub fn grant_group_read(inode: &Inode, gid: Gid) -> KResult<()> {
    let mut acl = get_acl(inode).unwrap_or_else(|_| {
        let meta = inode.metadata();
        Acl::from_mode(meta.mode.bits())
    });

    acl.add_entry(AclEntry::group(gid, AclPerm::new(AclPerm::READ)))?;
    acl.recalculate_mask();
    set_acl(inode, &acl)
}

/// Grant read/write access to a group
pub fn grant_group_rw(inode: &Inode, gid: Gid) -> KResult<()> {
    let mut acl = get_acl(inode).unwrap_or_else(|_| {
        let meta = inode.metadata();
        Acl::from_mode(meta.mode.bits())
    });

    acl.add_entry(AclEntry::group(gid, AclPerm::new(AclPerm::READ | AclPerm::WRITE)))?;
    acl.recalculate_mask();
    set_acl(inode, &acl)
}

/// Revoke access for a specific user
pub fn revoke_user(inode: &Inode, uid: Uid) -> KResult<()> {
    let mut acl = get_acl(inode)?;
    acl.remove_entry(AclTag::User, uid.0)?;
    acl.recalculate_mask();

    // If ACL is now minimal, remove it entirely
    if acl.is_minimal() {
        remove_acl(inode)
    } else {
        set_acl(inode, &acl)
    }
}

/// Revoke access for a specific group
pub fn revoke_group(inode: &Inode, gid: Gid) -> KResult<()> {
    let mut acl = get_acl(inode)?;
    acl.remove_entry(AclTag::Group, gid.0)?;
    acl.recalculate_mask();

    // If ACL is now minimal, remove it entirely
    if acl.is_minimal() {
        remove_acl(inode)
    } else {
        set_acl(inode, &acl)
    }
}

// ============================================================================
// Debug / Status
// ============================================================================

/// Format ACL info for an inode (like getfacl)
pub fn format_acl_info(inode: &Inode) -> String {
    let mut result = String::new();

    match get_acl(inode) {
        Ok(acl) => {
            result.push_str("# access ACL:\n");
            result.push_str(&acl.format());
            result.push('\n');
        }
        Err(_) => {
            result.push_str("# access ACL: (none - using mode)\n");
        }
    }

    match get_default_acl(inode) {
        Ok(default_acl) => {
            result.push_str("\n# default ACL:\n");
            for entry in default_acl.acl().entries() {
                result.push_str("default:");
                result.push_str(&entry.format());
                result.push('\n');
            }
        }
        Err(_) => {
            // No default ACL (normal for files)
        }
    }

    result
}

/// Format ACL status
pub fn format_status() -> String {
    format!(
        "Access Control Lists:\n\
         - ACL version: {}\n\
         - Max entries: {}\n\
         - Entry types: user, group, mask, other\n\
         - Stored as: extended attributes\n\
         - xattr names: system.posix_acl_access, system.posix_acl_default\n",
        ACL_VERSION,
        ACL_MAX_ENTRIES
    )
}
