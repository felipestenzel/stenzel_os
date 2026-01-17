//! exFAT filesystem driver (read-only)
//!
//! exFAT (Extended File Allocation Table) is a file system developed by Microsoft
//! for flash drives. It overcomes the 4GB file size limit of FAT32.
//!
//! Features:
//! - No 4GB file size limit (supports up to 16 EB)
//! - Allocation bitmap for free space tracking
//! - Extended attributes and timestamps
//! - UTF-16 long file names
//! - File integrity checksum
//!
//! References:
//! - Microsoft exFAT specification (available under OIN license)
//! - https://wiki.osdev.org/ExFAT

#![allow(dead_code)]

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use spin::RwLock;

use crate::security::{Gid, Uid};
use crate::storage::BlockDevice;
use crate::util::{KError, KResult};

use super::vfs::{DirEntry, Inode, InodeKind, InodeOps, Metadata, Mode};

// ============================================================================
// Constants
// ============================================================================

/// exFAT signature "EXFAT   " at offset 3
const EXFAT_SIGNATURE: [u8; 8] = *b"EXFAT   ";

/// Boot signature
const BOOT_SIGNATURE: u16 = 0xAA55;

/// Minimum exFAT version supported (1.00)
const MIN_VERSION: u16 = 0x0100;

/// exFAT FAT entry values
const EXFAT_FREE: u32 = 0x00000000;
const EXFAT_BAD: u32 = 0xFFFFFFF7;
const EXFAT_END: u32 = 0xFFFFFFFF;

/// Directory entry types
const ENTRY_TYPE_END_OF_DIR: u8 = 0x00;
const ENTRY_TYPE_ALLOC_BITMAP: u8 = 0x81;
const ENTRY_TYPE_UPCASE_TABLE: u8 = 0x82;
const ENTRY_TYPE_VOLUME_LABEL: u8 = 0x83;
const ENTRY_TYPE_FILE: u8 = 0x85;
const ENTRY_TYPE_STREAM_EXT: u8 = 0xC0;
const ENTRY_TYPE_FILE_NAME: u8 = 0xC1;
const ENTRY_TYPE_VENDOR_EXT: u8 = 0xE0;
const ENTRY_TYPE_VENDOR_ALLOC: u8 = 0xE1;

/// File attributes
const ATTR_READ_ONLY: u16 = 0x0001;
const ATTR_HIDDEN: u16 = 0x0002;
const ATTR_SYSTEM: u16 = 0x0004;
const ATTR_DIRECTORY: u16 = 0x0010;
const ATTR_ARCHIVE: u16 = 0x0020;

/// Stream extension flags
const STREAM_FLAG_ALLOC_POSSIBLE: u8 = 0x01;
const STREAM_FLAG_NO_FAT_CHAIN: u8 = 0x02;

// ============================================================================
// On-disk structures (packed, little-endian)
// ============================================================================

/// Main Boot Sector (sector 0)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct ExfatBootSector {
    /// Jump instruction (0xEB 0x76 0x90)
    jmp_boot: [u8; 3],
    /// File system name "EXFAT   "
    fs_name: [u8; 8],
    /// Must be zero
    must_be_zero: [u8; 53],
    /// Partition offset in sectors
    partition_offset: u64,
    /// Volume length in sectors
    volume_length: u64,
    /// FAT offset in sectors
    fat_offset: u32,
    /// FAT length in sectors
    fat_length: u32,
    /// Cluster heap offset in sectors
    cluster_heap_offset: u32,
    /// Cluster count
    cluster_count: u32,
    /// First cluster of root directory
    root_directory_cluster: u32,
    /// Volume serial number
    volume_serial: u32,
    /// File system revision (high byte = major, low byte = minor)
    fs_revision: u16,
    /// Volume flags
    volume_flags: u16,
    /// Bytes per sector shift (2^n)
    bytes_per_sector_shift: u8,
    /// Sectors per cluster shift (2^n)
    sectors_per_cluster_shift: u8,
    /// Number of FATs (1 or 2)
    number_of_fats: u8,
    /// Drive select (0x80 for HDD)
    drive_select: u8,
    /// Percent in use
    percent_in_use: u8,
    /// Reserved
    reserved: [u8; 7],
    /// Boot code
    boot_code: [u8; 390],
    /// Boot signature (0xAA55)
    boot_signature: u16,
}

/// Generic directory entry header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct ExfatDirEntry {
    /// Entry type
    entry_type: u8,
    /// Custom data (depends on entry type)
    custom: [u8; 19],
    /// First cluster (for some entry types)
    first_cluster: u32,
    /// Data length
    data_length: u64,
}

/// File directory entry (type 0x85)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct ExfatFileEntry {
    /// Entry type (0x85)
    entry_type: u8,
    /// Secondary count (number of secondary entries)
    secondary_count: u8,
    /// Set checksum
    set_checksum: u16,
    /// File attributes
    file_attributes: u16,
    /// Reserved
    reserved1: u16,
    /// Create timestamp
    create_timestamp: u32,
    /// Last modified timestamp
    modify_timestamp: u32,
    /// Last access timestamp
    access_timestamp: u32,
    /// Create time 10ms units
    create_time_10ms: u8,
    /// Modify time 10ms units
    modify_time_10ms: u8,
    /// Create time UTC offset
    create_utc_offset: u8,
    /// Modify time UTC offset
    modify_utc_offset: u8,
    /// Access time UTC offset
    access_utc_offset: u8,
    /// Reserved
    reserved2: [u8; 7],
}

/// Stream extension entry (type 0xC0)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct ExfatStreamEntry {
    /// Entry type (0xC0)
    entry_type: u8,
    /// General flags
    flags: u8,
    /// Reserved
    reserved1: u8,
    /// Name length in characters
    name_length: u8,
    /// Name hash
    name_hash: u16,
    /// Reserved
    reserved2: u16,
    /// Valid data length
    valid_data_length: u64,
    /// Reserved
    reserved3: u32,
    /// First cluster
    first_cluster: u32,
    /// Data length
    data_length: u64,
}

/// File name entry (type 0xC1)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct ExfatNameEntry {
    /// Entry type (0xC1)
    entry_type: u8,
    /// General flags
    flags: u8,
    /// File name characters (15 UTF-16 characters)
    file_name: [u16; 15],
}

/// Allocation bitmap entry (type 0x81)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct ExfatBitmapEntry {
    /// Entry type (0x81)
    entry_type: u8,
    /// Bitmap flags
    bitmap_flags: u8,
    /// Reserved
    reserved: [u8; 18],
    /// First cluster
    first_cluster: u32,
    /// Data length
    data_length: u64,
}

/// Volume label entry (type 0x83)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct ExfatVolumeLabelEntry {
    /// Entry type (0x83)
    entry_type: u8,
    /// Character count
    char_count: u8,
    /// Volume label (11 UTF-16 characters)
    volume_label: [u16; 11],
    /// Reserved
    reserved: [u8; 8],
}

// ============================================================================
// Filesystem implementation
// ============================================================================

/// Parsed file entry with all associated entries
#[derive(Clone)]
struct ParsedFileEntry {
    /// File attributes
    attributes: u16,
    /// File name
    name: String,
    /// First cluster
    first_cluster: u32,
    /// Data length
    data_length: u64,
    /// Valid data length
    valid_data_length: u64,
    /// Uses FAT chain (false = contiguous)
    uses_fat_chain: bool,
    /// Create timestamp
    create_time: u32,
    /// Modify timestamp
    modify_time: u32,
    /// Access timestamp
    access_time: u32,
}

impl ParsedFileEntry {
    fn is_directory(&self) -> bool {
        (self.attributes & ATTR_DIRECTORY) != 0
    }
}

/// exFAT filesystem
pub struct ExfatFs {
    /// Block device
    device: Arc<dyn BlockDevice>,
    /// Boot sector info
    boot: ExfatBootInfo,
    /// Root directory inode
    root: ExfatInode,
}

/// Extracted boot sector information
#[derive(Clone)]
struct ExfatBootInfo {
    /// Bytes per sector
    bytes_per_sector: u32,
    /// Sectors per cluster
    sectors_per_cluster: u32,
    /// Bytes per cluster
    bytes_per_cluster: u32,
    /// FAT offset in sectors
    fat_offset: u32,
    /// FAT length in sectors
    fat_length: u32,
    /// Cluster heap offset in sectors
    cluster_heap_offset: u32,
    /// Number of clusters
    cluster_count: u32,
    /// Root directory first cluster
    root_cluster: u32,
    /// Volume serial number
    volume_serial: u32,
}

impl ExfatFs {
    /// Mount an exFAT filesystem
    pub fn mount(device: Arc<dyn BlockDevice>) -> KResult<Arc<Self>> {
        // Read boot sector
        let mut boot_buf = vec![0u8; 512];
        device.read_blocks(0, 1, &mut boot_buf)?;

        // Parse boot sector
        let boot_sector = unsafe { &*(boot_buf.as_ptr() as *const ExfatBootSector) };

        // Verify signature
        if boot_sector.fs_name != EXFAT_SIGNATURE {
            return Err(KError::Invalid);
        }

        if boot_sector.boot_signature != BOOT_SIGNATURE {
            return Err(KError::Invalid);
        }

        // Check version
        let version = boot_sector.fs_revision;
        if version < MIN_VERSION {
            crate::kprintln!("exfat: unsupported version {}.{}", version >> 8, version & 0xFF);
            return Err(KError::NotSupported);
        }

        // Extract boot info
        let bytes_per_sector = 1u32 << boot_sector.bytes_per_sector_shift;
        let sectors_per_cluster = 1u32 << boot_sector.sectors_per_cluster_shift;
        let bytes_per_cluster = bytes_per_sector * sectors_per_cluster;

        let boot_info = ExfatBootInfo {
            bytes_per_sector,
            sectors_per_cluster,
            bytes_per_cluster,
            fat_offset: boot_sector.fat_offset,
            fat_length: boot_sector.fat_length,
            cluster_heap_offset: boot_sector.cluster_heap_offset,
            cluster_count: boot_sector.cluster_count,
            root_cluster: boot_sector.root_directory_cluster,
            volume_serial: boot_sector.volume_serial,
        };

        crate::kprintln!(
            "exfat: mounted, {} sectors/cluster, {} clusters, root cluster {}",
            boot_info.sectors_per_cluster,
            boot_info.cluster_count,
            boot_info.root_cluster
        );

        let root = ExfatInode {
            device: Arc::clone(&device),
            boot: boot_info.clone(),
            first_cluster: boot_info.root_cluster,
            data_length: 0, // Directory size determined by entries
            valid_data_length: 0,
            uses_fat_chain: true, // Root directory uses FAT chain
            is_dir: true,
            attributes: ATTR_DIRECTORY,
            parent: None,
        };

        let fs = Arc::new(ExfatFs {
            device,
            boot: boot_info,
            root,
        });

        Ok(fs)
    }

    /// Get root directory inode
    pub fn root(&self) -> Inode {
        Inode(Arc::new(self.root.clone()))
    }

    /// Read FAT entry
    fn read_fat_entry(&self, cluster: u32) -> KResult<u32> {
        let fat_offset = self.boot.fat_offset as u64 * self.boot.bytes_per_sector as u64;
        let entry_offset = fat_offset + (cluster as u64 * 4);
        let sector = (entry_offset / self.boot.bytes_per_sector as u64) as u64;
        let offset_in_sector = (entry_offset % self.boot.bytes_per_sector as u64) as usize;

        let mut buf = vec![0u8; self.boot.bytes_per_sector as usize];
        self.device.read_blocks(sector, 1, &mut buf)?;

        let entry = u32::from_le_bytes([
            buf[offset_in_sector],
            buf[offset_in_sector + 1],
            buf[offset_in_sector + 2],
            buf[offset_in_sector + 3],
        ]);

        Ok(entry)
    }

    /// Convert cluster number to sector
    fn cluster_to_sector(&self, cluster: u32) -> u64 {
        // Cluster numbering starts at 2
        let cluster_offset = (cluster - 2) as u64;
        self.boot.cluster_heap_offset as u64 + cluster_offset * self.boot.sectors_per_cluster as u64
    }
}

/// exFAT inode
#[derive(Clone)]
pub struct ExfatInode {
    device: Arc<dyn BlockDevice>,
    boot: ExfatBootInfo,
    first_cluster: u32,
    data_length: u64,
    valid_data_length: u64,
    uses_fat_chain: bool,
    is_dir: bool,
    attributes: u16,
    parent: Option<Arc<ExfatInode>>,
}

impl ExfatInode {
    /// Read cluster data
    fn read_cluster(&self, cluster: u32) -> KResult<Vec<u8>> {
        let sector = cluster_to_sector(&self.boot, cluster);
        let mut buf = vec![0u8; self.boot.bytes_per_cluster as usize];
        self.device.read_blocks(sector, self.boot.sectors_per_cluster, &mut buf)?;
        Ok(buf)
    }

    /// Read all clusters of this inode
    fn read_all_data(&self) -> KResult<Vec<u8>> {
        let mut data = Vec::new();
        let max_len = self.data_length as usize;

        if self.uses_fat_chain {
            // Follow FAT chain
            let mut cluster = self.first_cluster;
            while cluster >= 2 && cluster < self.boot.cluster_count + 2 && data.len() < max_len {
                let chunk = self.read_cluster(cluster)?;
                let remaining = max_len - data.len();
                let to_copy = remaining.min(chunk.len());
                data.extend_from_slice(&chunk[..to_copy]);

                // Get next cluster from FAT
                let next = read_fat_entry(&self.device, &self.boot, cluster)?;
                if next >= EXFAT_END - 7 {
                    break;
                }
                cluster = next;
            }
        } else {
            // Contiguous allocation
            let num_clusters = (max_len as u64 + self.boot.bytes_per_cluster as u64 - 1)
                / self.boot.bytes_per_cluster as u64;
            for i in 0..num_clusters {
                let cluster = self.first_cluster + i as u32;
                let chunk = self.read_cluster(cluster)?;
                let remaining = max_len - data.len();
                let to_copy = remaining.min(chunk.len());
                data.extend_from_slice(&chunk[..to_copy]);
            }
        }

        Ok(data)
    }

    /// Parse directory entries
    fn parse_dir_entries(&self) -> KResult<Vec<ParsedFileEntry>> {
        let data = self.read_all_data()?;
        let mut entries = Vec::new();
        let mut i = 0;

        while i + 32 <= data.len() {
            let entry_type = data[i];

            // End of directory
            if entry_type == ENTRY_TYPE_END_OF_DIR {
                break;
            }

            // Skip deleted entries (type with cleared bit 7)
            if entry_type & 0x80 == 0 {
                i += 32;
                continue;
            }

            // File entry
            if entry_type == ENTRY_TYPE_FILE {
                if let Some(parsed) = self.parse_file_entry_set(&data[i..])? {
                    let secondary_count = data[i + 1] as usize;
                    entries.push(parsed);
                    i += 32 * (1 + secondary_count);
                    continue;
                }
            }

            i += 32;
        }

        Ok(entries)
    }

    /// Parse a complete file entry set (file + stream + name entries)
    fn parse_file_entry_set(&self, data: &[u8]) -> KResult<Option<ParsedFileEntry>> {
        if data.len() < 32 {
            return Ok(None);
        }

        let file_entry = unsafe { &*(data.as_ptr() as *const ExfatFileEntry) };
        let secondary_count = file_entry.secondary_count as usize;

        if data.len() < 32 * (1 + secondary_count) {
            return Ok(None);
        }

        // Find stream extension entry
        let mut stream_entry: Option<&ExfatStreamEntry> = None;
        let mut name_chars = Vec::new();

        for j in 0..secondary_count {
            let offset = 32 * (1 + j);
            let entry_type = data[offset];

            if entry_type == ENTRY_TYPE_STREAM_EXT {
                stream_entry = Some(unsafe { &*(data[offset..].as_ptr() as *const ExfatStreamEntry) });
            } else if entry_type == ENTRY_TYPE_FILE_NAME {
                let name_entry = unsafe { &*(data[offset..].as_ptr() as *const ExfatNameEntry) };
                // Copy file_name array to avoid unaligned reference
                let file_name: [u16; 15] = unsafe {
                    core::ptr::read_unaligned(core::ptr::addr_of!(name_entry.file_name))
                };
                for c in file_name {
                    if c == 0 {
                        break;
                    }
                    name_chars.push(c);
                }
            }
        }

        let stream = match stream_entry {
            Some(s) => s,
            None => return Ok(None),
        };

        // Convert name to String
        let name = String::from_utf16_lossy(&name_chars);

        Ok(Some(ParsedFileEntry {
            attributes: file_entry.file_attributes,
            name,
            first_cluster: stream.first_cluster,
            data_length: stream.data_length,
            valid_data_length: stream.valid_data_length,
            uses_fat_chain: (stream.flags & STREAM_FLAG_NO_FAT_CHAIN) == 0,
            create_time: file_entry.create_timestamp,
            modify_time: file_entry.modify_timestamp,
            access_time: file_entry.access_timestamp,
        }))
    }
}

/// Helper function to read FAT entry
fn read_fat_entry(device: &Arc<dyn BlockDevice>, boot: &ExfatBootInfo, cluster: u32) -> KResult<u32> {
    let fat_offset = boot.fat_offset as u64 * boot.bytes_per_sector as u64;
    let entry_offset = fat_offset + (cluster as u64 * 4);
    let sector = entry_offset / boot.bytes_per_sector as u64;
    let offset_in_sector = (entry_offset % boot.bytes_per_sector as u64) as usize;

    let mut buf = vec![0u8; boot.bytes_per_sector as usize];
    device.read_blocks(sector, 1, &mut buf)?;

    let entry = u32::from_le_bytes([
        buf[offset_in_sector],
        buf[offset_in_sector + 1],
        buf[offset_in_sector + 2],
        buf[offset_in_sector + 3],
    ]);

    Ok(entry)
}

/// Helper function to convert cluster to sector
fn cluster_to_sector(boot: &ExfatBootInfo, cluster: u32) -> u64 {
    let cluster_offset = (cluster - 2) as u64;
    boot.cluster_heap_offset as u64 + cluster_offset * boot.sectors_per_cluster as u64
}

impl InodeOps for ExfatInode {
    fn metadata(&self) -> Metadata {
        let kind = if self.is_dir {
            InodeKind::Dir
        } else {
            InodeKind::File
        };

        let mode = if self.is_dir {
            Mode::from_octal(0o755)
        } else if (self.attributes & ATTR_READ_ONLY) != 0 {
            Mode::from_octal(0o444)
        } else {
            Mode::from_octal(0o644)
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
        if !self.is_dir {
            return Err(KError::NotADirectory);
        }

        let entries = self.parse_dir_entries()?;

        for entry in entries {
            // Case-insensitive comparison (exFAT is case-preserving but case-insensitive)
            if entry.name.eq_ignore_ascii_case(name) {
                let inode = ExfatInode {
                    device: Arc::clone(&self.device),
                    boot: self.boot.clone(),
                    first_cluster: entry.first_cluster,
                    data_length: entry.data_length,
                    valid_data_length: entry.valid_data_length,
                    uses_fat_chain: entry.uses_fat_chain,
                    is_dir: entry.is_directory(),
                    attributes: entry.attributes,
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
        if !self.is_dir {
            return Err(KError::NotADirectory);
        }

        let entries = self.parse_dir_entries()?;
        let mut result = Vec::new();

        for entry in entries {
            // Skip "." and ".." (exFAT doesn't have them)
            if entry.name == "." || entry.name == ".." {
                continue;
            }

            let kind = if entry.is_directory() {
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
        if self.is_dir {
            return Err(KError::IsADirectory);
        }

        let file_size = self.valid_data_length as usize;
        if offset >= file_size {
            return Ok(0);
        }

        let data = self.read_all_data()?;
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
        if self.is_dir {
            Ok(0)
        } else {
            Ok(self.valid_data_length as usize)
        }
    }
}

// ============================================================================
// Detection and mounting helpers
// ============================================================================

/// Check if a device contains an exFAT filesystem
pub fn is_exfat(device: &Arc<dyn BlockDevice>) -> KResult<bool> {
    let mut buf = vec![0u8; 512];
    device.read_blocks(0, 1, &mut buf)?;

    // Check signature at offset 3
    if buf[3..11] == EXFAT_SIGNATURE {
        // Also verify boot signature
        let boot_sig = u16::from_le_bytes([buf[510], buf[511]]);
        return Ok(boot_sig == BOOT_SIGNATURE);
    }

    Ok(false)
}

/// Mount exFAT filesystem
pub fn mount(device: Arc<dyn BlockDevice>) -> KResult<Arc<ExfatFs>> {
    ExfatFs::mount(device)
}
