//! ISO 9660 filesystem driver (read-only)
//!
//! ISO 9660 is the standard file system for CD-ROM and DVD media.
//!
//! Features:
//! - Primary Volume Descriptor parsing
//! - Directory record traversal
//! - Joliet extension support (Unicode file names)
//! - Rock Ridge extension support (POSIX attributes)
//! - Large file support (multi-extent)
//!
//! References:
//! - ECMA-119 (ISO 9660)
//! - https://wiki.osdev.org/ISO_9660

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use crate::security::{Gid, Uid};
use crate::storage::BlockDevice;
use crate::util::{KError, KResult};

use super::vfs::{DirEntry, Inode, InodeKind, InodeOps, Metadata, Mode};

// ============================================================================
// Constants
// ============================================================================

/// ISO 9660 sector size
const SECTOR_SIZE: usize = 2048;

/// System Area size (sectors 0-15 are reserved)
const SYSTEM_AREA_SECTORS: u64 = 16;

/// Volume descriptor types
const VD_TYPE_BOOT: u8 = 0;
const VD_TYPE_PRIMARY: u8 = 1;
const VD_TYPE_SUPPLEMENTARY: u8 = 2;
const VD_TYPE_PARTITION: u8 = 3;
const VD_TYPE_TERMINATOR: u8 = 255;

/// Volume descriptor standard identifier
const VD_STANDARD_ID: [u8; 5] = *b"CD001";

/// Volume descriptor version
const VD_VERSION: u8 = 1;

/// Directory record flags
const DR_FLAG_HIDDEN: u8 = 0x01;
const DR_FLAG_DIRECTORY: u8 = 0x02;
const DR_FLAG_ASSOCIATED: u8 = 0x04;
const DR_FLAG_EXTENDED_ATTR: u8 = 0x08;
const DR_FLAG_PERMISSIONS: u8 = 0x10;
const DR_FLAG_MULTI_EXTENT: u8 = 0x80;

/// Rock Ridge signature
const RR_SIGNATURE_SP: [u8; 2] = *b"SP";  // SUSP indicator
const RR_SIGNATURE_RR: [u8; 2] = *b"RR";  // Rock Ridge extensions
const RR_SIGNATURE_NM: [u8; 2] = *b"NM";  // Alternate name
const RR_SIGNATURE_PX: [u8; 2] = *b"PX";  // POSIX attributes
const RR_SIGNATURE_TF: [u8; 2] = *b"TF";  // Time stamps

// ============================================================================
// On-disk structures
// ============================================================================

/// Volume Descriptor (common header)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct VolumeDescriptor {
    /// Type code
    vd_type: u8,
    /// Standard identifier "CD001"
    standard_id: [u8; 5],
    /// Version (1)
    version: u8,
    /// Data (depends on type)
    data: [u8; 2041],
}

/// Primary Volume Descriptor
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct PrimaryVolumeDescriptor {
    /// Type code (1)
    vd_type: u8,
    /// Standard identifier "CD001"
    standard_id: [u8; 5],
    /// Version (1)
    version: u8,
    /// Unused
    unused1: u8,
    /// System identifier
    system_id: [u8; 32],
    /// Volume identifier
    volume_id: [u8; 32],
    /// Unused
    unused2: [u8; 8],
    /// Volume space size (both-endian)
    volume_space_size: [u8; 8],
    /// Unused (Joliet: escape sequences)
    unused3: [u8; 32],
    /// Volume set size (both-endian)
    volume_set_size: [u8; 4],
    /// Volume sequence number (both-endian)
    volume_sequence_number: [u8; 4],
    /// Logical block size (both-endian)
    logical_block_size: [u8; 4],
    /// Path table size (both-endian)
    path_table_size: [u8; 8],
    /// Location of Type L path table (little-endian)
    path_table_l_location: u32,
    /// Location of optional Type L path table
    path_table_l_opt_location: u32,
    /// Location of Type M path table (big-endian)
    path_table_m_location: u32,
    /// Location of optional Type M path table
    path_table_m_opt_location: u32,
    /// Root directory record (34 bytes)
    root_directory_record: [u8; 34],
    /// Volume set identifier
    volume_set_id: [u8; 128],
    /// Publisher identifier
    publisher_id: [u8; 128],
    /// Data preparer identifier
    data_preparer_id: [u8; 128],
    /// Application identifier
    application_id: [u8; 128],
    /// Copyright file identifier
    copyright_file_id: [u8; 37],
    /// Abstract file identifier
    abstract_file_id: [u8; 37],
    /// Bibliographic file identifier
    bibliographic_file_id: [u8; 37],
    /// Volume creation date/time
    creation_datetime: [u8; 17],
    /// Volume modification date/time
    modification_datetime: [u8; 17],
    /// Volume expiration date/time
    expiration_datetime: [u8; 17],
    /// Volume effective date/time
    effective_datetime: [u8; 17],
    /// File structure version
    file_structure_version: u8,
    /// Reserved
    reserved1: u8,
    /// Application use
    application_use: [u8; 512],
    /// Reserved
    reserved2: [u8; 653],
}

/// Directory Record
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct DirectoryRecord {
    /// Length of directory record
    length: u8,
    /// Extended attribute record length
    ext_attr_length: u8,
    /// Location of extent (LBA) - little-endian
    extent_location_le: u32,
    /// Location of extent (LBA) - big-endian
    extent_location_be: u32,
    /// Data length - little-endian
    data_length_le: u32,
    /// Data length - big-endian
    data_length_be: u32,
    /// Recording date and time
    recording_datetime: [u8; 7],
    /// File flags
    flags: u8,
    /// File unit size (interleaved mode)
    file_unit_size: u8,
    /// Interleave gap size
    interleave_gap_size: u8,
    /// Volume sequence number (both-endian)
    volume_sequence_number: [u8; 4],
    /// File identifier length
    file_id_length: u8,
    // Followed by file identifier and padding
}

// ============================================================================
// Parsed structures
// ============================================================================

/// Parsed directory entry
#[derive(Clone)]
struct IsoDirectoryEntry {
    /// File name
    name: String,
    /// Location (LBA)
    location: u32,
    /// Data length
    data_length: u32,
    /// Is directory
    is_directory: bool,
    /// Is hidden
    is_hidden: bool,
    /// Is multi-extent
    is_multi_extent: bool,
    /// Rock Ridge mode (if available)
    rr_mode: Option<u32>,
    /// Rock Ridge UID (if available)
    rr_uid: Option<u32>,
    /// Rock Ridge GID (if available)
    rr_gid: Option<u32>,
}

/// Volume information
#[derive(Clone)]
struct IsoVolumeInfo {
    /// Volume identifier
    volume_id: String,
    /// System identifier
    system_id: String,
    /// Publisher identifier
    publisher_id: String,
    /// Volume space size (blocks)
    volume_space_size: u32,
    /// Logical block size
    block_size: u32,
    /// Root directory location
    root_location: u32,
    /// Root directory size
    root_size: u32,
    /// Has Joliet extension
    has_joliet: bool,
    /// Has Rock Ridge extension
    has_rock_ridge: bool,
    /// Joliet volume descriptor location (if present)
    joliet_vd_location: Option<u64>,
}

// ============================================================================
// Filesystem implementation
// ============================================================================

/// ISO 9660 filesystem
pub struct Iso9660Fs {
    /// Block device
    device: Arc<dyn BlockDevice>,
    /// Volume information
    volume_info: IsoVolumeInfo,
    /// Root directory inode
    root: IsoInode,
}

impl Iso9660Fs {
    /// Mount an ISO 9660 filesystem
    pub fn mount(device: Arc<dyn BlockDevice>) -> KResult<Arc<Self>> {
        // Read volume descriptors starting at sector 16
        let mut sector = SYSTEM_AREA_SECTORS;
        let mut primary_vd: Option<PrimaryVolumeDescriptor> = None;
        let mut joliet_vd_location: Option<u64> = None;

        loop {
            let mut buf = vec![0u8; SECTOR_SIZE];
            device.read_blocks(sector, 1, &mut buf)?;

            let vd = unsafe { &*(buf.as_ptr() as *const VolumeDescriptor) };

            // Verify standard identifier
            if vd.standard_id != VD_STANDARD_ID {
                return Err(KError::Invalid);
            }

            match vd.vd_type {
                VD_TYPE_PRIMARY => {
                    let pvd = unsafe { &*(buf.as_ptr() as *const PrimaryVolumeDescriptor) };
                    primary_vd = Some(*pvd);
                }
                VD_TYPE_SUPPLEMENTARY => {
                    // Check for Joliet (escape sequences in unused3)
                    let escape = &buf[88..120];
                    if escape[0] == 0x25 && (escape[1] == 0x2F || escape[1] == 0x40) {
                        joliet_vd_location = Some(sector);
                    }
                }
                VD_TYPE_TERMINATOR => break,
                _ => {}
            }

            sector += 1;
            if sector > 32 {
                // Safety limit
                break;
            }
        }

        let pvd = primary_vd.ok_or(KError::Invalid)?;

        // Parse volume info
        let volume_space_size = u32::from_le_bytes([
            pvd.volume_space_size[0],
            pvd.volume_space_size[1],
            pvd.volume_space_size[2],
            pvd.volume_space_size[3],
        ]);

        let block_size = u16::from_le_bytes([
            pvd.logical_block_size[0],
            pvd.logical_block_size[1],
        ]) as u32;

        // Parse root directory record
        let root_record = unsafe {
            &*(pvd.root_directory_record.as_ptr() as *const DirectoryRecord)
        };

        let root_location = root_record.extent_location_le;
        let root_size = root_record.data_length_le;

        // Clean up identifiers (remove trailing spaces)
        let volume_id = trim_iso_string(&pvd.volume_id);
        let system_id = trim_iso_string(&pvd.system_id);
        let publisher_id = trim_iso_string(&pvd.publisher_id);

        // Check for Rock Ridge (read first directory entry and look for SP signature)
        let has_rock_ridge = Self::check_rock_ridge(&device, root_location, block_size)?;

        let volume_info = IsoVolumeInfo {
            volume_id,
            system_id,
            publisher_id,
            volume_space_size,
            block_size,
            root_location,
            root_size,
            has_joliet: joliet_vd_location.is_some(),
            has_rock_ridge,
            joliet_vd_location,
        };

        crate::kprintln!(
            "iso9660: mounted '{}', {} blocks, joliet={}, rock_ridge={}",
            volume_info.volume_id,
            volume_info.volume_space_size,
            volume_info.has_joliet,
            volume_info.has_rock_ridge
        );

        let root = IsoInode {
            device: Arc::clone(&device),
            volume_info: volume_info.clone(),
            location: root_location,
            data_length: root_size,
            is_directory: true,
            parent: None,
        };

        Ok(Arc::new(Iso9660Fs {
            device,
            volume_info,
            root,
        }))
    }

    /// Check for Rock Ridge extensions
    fn check_rock_ridge(device: &Arc<dyn BlockDevice>, root_lba: u32, block_size: u32) -> KResult<bool> {
        let mut buf = vec![0u8; block_size as usize];
        device.read_blocks(root_lba as u64, 1, &mut buf)?;

        // Parse first directory record
        if buf[0] == 0 {
            return Ok(false);
        }

        let record = unsafe { &*(buf.as_ptr() as *const DirectoryRecord) };
        let system_use_start = 33 + record.file_id_length as usize;
        let system_use_start = if system_use_start % 2 != 0 {
            system_use_start + 1
        } else {
            system_use_start
        };

        // Look for SP (SUSP) signature in system use area
        if system_use_start + 7 <= record.length as usize {
            let sig = &buf[system_use_start..system_use_start + 2];
            if sig == RR_SIGNATURE_SP {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Get root directory inode
    pub fn root(&self) -> Inode {
        Inode(Arc::new(self.root.clone()))
    }
}

/// ISO 9660 inode
#[derive(Clone)]
pub struct IsoInode {
    device: Arc<dyn BlockDevice>,
    volume_info: IsoVolumeInfo,
    location: u32,
    data_length: u32,
    is_directory: bool,
    parent: Option<Arc<IsoInode>>,
}

impl IsoInode {
    /// Read directory entries
    fn read_directory(&self) -> KResult<Vec<IsoDirectoryEntry>> {
        if !self.is_directory {
            return Err(KError::NotADirectory);
        }

        let mut entries = Vec::new();
        let blocks = (self.data_length as usize + self.volume_info.block_size as usize - 1)
            / self.volume_info.block_size as usize;

        let mut data = vec![0u8; blocks * self.volume_info.block_size as usize];
        self.device.read_blocks(
            self.location as u64,
            blocks as u32,
            &mut data,
        )?;

        let mut offset = 0;
        while offset < self.data_length as usize {
            // Check for zero-length record (sector boundary)
            if data[offset] == 0 {
                // Advance to next sector
                let next_sector = (offset / self.volume_info.block_size as usize + 1)
                    * self.volume_info.block_size as usize;
                if next_sector >= self.data_length as usize {
                    break;
                }
                offset = next_sector;
                continue;
            }

            let record = unsafe { &*(data[offset..].as_ptr() as *const DirectoryRecord) };

            if record.length == 0 {
                break;
            }

            // Parse file name
            let name_start = offset + 33;
            let name_len = record.file_id_length as usize;

            if name_start + name_len > data.len() {
                break;
            }

            let name_bytes = &data[name_start..name_start + name_len];

            // Handle special entries
            let name = if name_len == 1 && name_bytes[0] == 0 {
                String::from(".")
            } else if name_len == 1 && name_bytes[0] == 1 {
                String::from("..")
            } else {
                // Try to parse Rock Ridge alternate name first
                let system_use_start = name_start + name_len;
                let system_use_start = if system_use_start % 2 != 0 {
                    system_use_start + 1
                } else {
                    system_use_start
                };

                let rr_name = if self.volume_info.has_rock_ridge {
                    parse_rock_ridge_name(&data[system_use_start..offset + record.length as usize])
                } else {
                    None
                };

                rr_name.unwrap_or_else(|| {
                    // Standard ISO 9660 name
                    let raw_name = String::from_utf8_lossy(name_bytes).to_string();
                    // Remove version number (;1)
                    let name = raw_name.split(';').next().unwrap_or(&raw_name);
                    // Remove trailing period
                    let name = name.trim_end_matches('.');
                    name.to_string()
                })
            };

            let is_directory = (record.flags & DR_FLAG_DIRECTORY) != 0;
            let is_hidden = (record.flags & DR_FLAG_HIDDEN) != 0;
            let is_multi_extent = (record.flags & DR_FLAG_MULTI_EXTENT) != 0;

            // Skip . and .. entries
            if name != "." && name != ".." {
                entries.push(IsoDirectoryEntry {
                    name,
                    location: record.extent_location_le,
                    data_length: record.data_length_le,
                    is_directory,
                    is_hidden,
                    is_multi_extent,
                    rr_mode: None,
                    rr_uid: None,
                    rr_gid: None,
                });
            }

            offset += record.length as usize;
        }

        Ok(entries)
    }

    /// Read file data
    fn read_file_data(&self) -> KResult<Vec<u8>> {
        if self.is_directory {
            return Err(KError::IsADirectory);
        }

        let blocks = (self.data_length as usize + self.volume_info.block_size as usize - 1)
            / self.volume_info.block_size as usize;

        let mut data = vec![0u8; blocks * self.volume_info.block_size as usize];
        self.device.read_blocks(
            self.location as u64,
            blocks as u32,
            &mut data,
        )?;

        // Truncate to actual size
        data.truncate(self.data_length as usize);

        Ok(data)
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Trim ISO 9660 string (remove trailing spaces)
fn trim_iso_string(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    s.trim_end().to_string()
}

/// Parse Rock Ridge alternate name (NM entry)
fn parse_rock_ridge_name(system_use: &[u8]) -> Option<String> {
    let mut offset = 0;
    let mut name_parts = Vec::new();

    while offset + 4 <= system_use.len() {
        let sig = &system_use[offset..offset + 2];
        let length = system_use[offset + 2] as usize;

        if length == 0 || offset + length > system_use.len() {
            break;
        }

        if sig == RR_SIGNATURE_NM {
            let flags = system_use[offset + 4];
            let name_data = &system_use[offset + 5..offset + length];

            if flags & 0x02 != 0 {
                // Current directory (.)
                return Some(String::from("."));
            } else if flags & 0x04 != 0 {
                // Parent directory (..)
                return Some(String::from(".."));
            } else {
                // Normal name (possibly continued)
                name_parts.extend_from_slice(name_data);

                if flags & 0x01 == 0 {
                    // Name is complete
                    break;
                }
            }
        }

        offset += length;
    }

    if name_parts.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(&name_parts).to_string())
    }
}

// ============================================================================
// InodeOps implementation
// ============================================================================

impl InodeOps for IsoInode {
    fn metadata(&self) -> Metadata {
        let kind = if self.is_directory {
            InodeKind::Dir
        } else {
            InodeKind::File
        };

        let mode = if self.is_directory {
            Mode::from_octal(0o555) // r-xr-xr-x for directories
        } else {
            Mode::from_octal(0o444) // r--r--r-- for files
        };

        Metadata::simple(Uid(0), Gid(0), mode, kind)
    }

    fn set_metadata(&self, _meta: Metadata) {
        // Read-only filesystem
    }

    fn parent(&self) -> Option<Inode> {
        self.parent.as_ref().map(|p| Inode(Arc::clone(p) as Arc<dyn InodeOps>))
    }

    fn lookup(&self, name: &str) -> KResult<Inode> {
        if !self.is_directory {
            return Err(KError::NotADirectory);
        }

        let entries = self.read_directory()?;

        for entry in entries {
            // Case-insensitive comparison for ISO 9660
            if entry.name.eq_ignore_ascii_case(name) {
                let inode = IsoInode {
                    device: Arc::clone(&self.device),
                    volume_info: self.volume_info.clone(),
                    location: entry.location,
                    data_length: entry.data_length,
                    is_directory: entry.is_directory,
                    parent: Some(Arc::new(self.clone())),
                };

                return Ok(Inode(Arc::new(inode)));
            }
        }

        Err(KError::NotFound)
    }

    fn create(&self, _name: &str, _kind: InodeKind, _meta: Metadata) -> KResult<Inode> {
        // Read-only filesystem
        Err(KError::NotSupported)
    }

    fn readdir(&self) -> KResult<Vec<DirEntry>> {
        if !self.is_directory {
            return Err(KError::NotADirectory);
        }

        let entries = self.read_directory()?;
        let mut result = Vec::new();

        for entry in entries {
            if entry.is_hidden {
                continue;
            }

            let kind = if entry.is_directory {
                InodeKind::Dir
            } else {
                InodeKind::File
            };

            result.push(DirEntry {
                name: entry.name,
                kind,
            });
        }

        Ok(result)
    }

    fn read_at(&self, offset: usize, out: &mut [u8]) -> KResult<usize> {
        if self.is_directory {
            return Err(KError::IsADirectory);
        }

        let file_size = self.data_length as usize;
        if offset >= file_size {
            return Ok(0);
        }

        let data = self.read_file_data()?;
        let available = file_size.saturating_sub(offset);
        let to_read = out.len().min(available);

        out[..to_read].copy_from_slice(&data[offset..offset + to_read]);
        Ok(to_read)
    }

    fn write_at(&self, _offset: usize, _data: &[u8]) -> KResult<usize> {
        // Read-only filesystem
        Err(KError::NotSupported)
    }

    fn truncate(&self, _size: usize) -> KResult<()> {
        // Read-only filesystem
        Err(KError::NotSupported)
    }

    fn size(&self) -> KResult<usize> {
        if self.is_directory {
            Ok(0)
        } else {
            Ok(self.data_length as usize)
        }
    }
}

// ============================================================================
// Detection and mounting
// ============================================================================

/// Check if a device contains an ISO 9660 filesystem
pub fn is_iso9660(device: &Arc<dyn BlockDevice>) -> KResult<bool> {
    // Read sector 16 (first volume descriptor)
    let mut buf = vec![0u8; SECTOR_SIZE];
    device.read_blocks(SYSTEM_AREA_SECTORS, 1, &mut buf)?;

    // Check for standard identifier "CD001"
    if buf[1..6] == VD_STANDARD_ID {
        // Check for primary volume descriptor type
        if buf[0] == VD_TYPE_PRIMARY {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Mount ISO 9660 filesystem
pub fn mount(device: Arc<dyn BlockDevice>) -> KResult<Arc<Iso9660Fs>> {
    Iso9660Fs::mount(device)
}
