//! NTFS filesystem driver (read-only)
//!
//! NTFS (New Technology File System) is the standard file system for Windows NT
//! and later Windows operating systems.
//!
//! Features:
//! - MFT (Master File Table) based file system
//! - Attribute-based file storage
//! - Support for large files (up to 16 EB)
//! - Unicode file names
//! - B+ tree directories for fast lookup
//!
//! This driver provides read-only support for:
//! - Reading files
//! - Listing directories
//! - Following non-resident data runlists
//! - UTF-16 file name decoding
//!
//! References:
//! - Linux NTFS driver documentation
//! - https://wiki.osdev.org/NTFS
//! - https://flatcap.github.io/linux-ntfs/ntfs/

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

/// NTFS OEM ID "NTFS    "
const NTFS_OEM_ID: [u8; 8] = *b"NTFS    ";

/// Boot signature
const BOOT_SIGNATURE: u16 = 0xAA55;

/// MFT record signature "FILE"
const MFT_RECORD_SIGNATURE: [u8; 4] = *b"FILE";

/// Index record signature "INDX"
const INDEX_RECORD_SIGNATURE: [u8; 4] = *b"INDX";

/// Default MFT record size (1KB)
const DEFAULT_MFT_RECORD_SIZE: u32 = 1024;

/// Attribute type codes
const AT_STANDARD_INFORMATION: u32 = 0x10;
const AT_ATTRIBUTE_LIST: u32 = 0x20;
const AT_FILE_NAME: u32 = 0x30;
const AT_OBJECT_ID: u32 = 0x40;
const AT_SECURITY_DESCRIPTOR: u32 = 0x50;
const AT_VOLUME_NAME: u32 = 0x60;
const AT_VOLUME_INFORMATION: u32 = 0x70;
const AT_DATA: u32 = 0x80;
const AT_INDEX_ROOT: u32 = 0x90;
const AT_INDEX_ALLOCATION: u32 = 0xA0;
const AT_BITMAP: u32 = 0xB0;
const AT_REPARSE_POINT: u32 = 0xC0;
const AT_EA_INFORMATION: u32 = 0xD0;
const AT_EA: u32 = 0xE0;
const AT_END: u32 = 0xFFFFFFFF;

/// File name namespace types
const FN_POSIX: u8 = 0;
const FN_WIN32: u8 = 1;
const FN_DOS: u8 = 2;
const FN_WIN32_AND_DOS: u8 = 3;

/// MFT special file numbers
const MFT_RECORD_MFT: u64 = 0;
const MFT_RECORD_MFTMIRR: u64 = 1;
const MFT_RECORD_LOGFILE: u64 = 2;
const MFT_RECORD_VOLUME: u64 = 3;
const MFT_RECORD_ATTRDEF: u64 = 4;
const MFT_RECORD_ROOT: u64 = 5;
const MFT_RECORD_BITMAP: u64 = 6;
const MFT_RECORD_BOOT: u64 = 7;
const MFT_RECORD_BADCLUS: u64 = 8;
const MFT_RECORD_SECURE: u64 = 9;
const MFT_RECORD_UPCASE: u64 = 10;
const MFT_RECORD_EXTEND: u64 = 11;

/// File attribute flags
const FILE_ATTR_READONLY: u32 = 0x0001;
const FILE_ATTR_HIDDEN: u32 = 0x0002;
const FILE_ATTR_SYSTEM: u32 = 0x0004;
const FILE_ATTR_DIRECTORY: u32 = 0x0010;
const FILE_ATTR_ARCHIVE: u32 = 0x0020;
const FILE_ATTR_DEVICE: u32 = 0x0040;
const FILE_ATTR_NORMAL: u32 = 0x0080;
const FILE_ATTR_TEMPORARY: u32 = 0x0100;
const FILE_ATTR_SPARSE_FILE: u32 = 0x0200;
const FILE_ATTR_REPARSE_POINT: u32 = 0x0400;
const FILE_ATTR_COMPRESSED: u32 = 0x0800;
const FILE_ATTR_ENCRYPTED: u32 = 0x4000;

/// MFT record flags
const MFT_RECORD_IN_USE: u16 = 0x0001;
const MFT_RECORD_IS_DIRECTORY: u16 = 0x0002;

// ============================================================================
// On-disk structures
// ============================================================================

/// NTFS Boot Sector
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct NtfsBootSector {
    /// Jump instruction
    jmp: [u8; 3],
    /// OEM ID "NTFS    "
    oem_id: [u8; 8],
    /// Bytes per sector
    bytes_per_sector: u16,
    /// Sectors per cluster
    sectors_per_cluster: u8,
    /// Reserved sectors (unused in NTFS)
    reserved_sectors: u16,
    /// Always 0 in NTFS
    fats: u8,
    /// Unused
    root_entries: u16,
    /// Unused
    sectors_16: u16,
    /// Media descriptor
    media_type: u8,
    /// Unused
    fat_sectors_16: u16,
    /// Sectors per track
    sectors_per_track: u16,
    /// Number of heads
    heads: u16,
    /// Hidden sectors
    hidden_sectors: u32,
    /// Unused
    sectors_32: u32,
    /// Physical drive number
    physical_drive: u8,
    /// Current head
    current_head: u8,
    /// Extended boot signature
    extended_signature: u8,
    /// Reserved
    reserved2: u8,
    /// Total sectors (64-bit)
    total_sectors: u64,
    /// MFT logical cluster number
    mft_lcn: u64,
    /// MFT mirror logical cluster number
    mft_mirr_lcn: u64,
    /// Clusters per MFT record (can be negative for bytes)
    clusters_per_mft_record: i8,
    /// Reserved
    reserved3: [u8; 3],
    /// Clusters per index record (can be negative for bytes)
    clusters_per_index_record: i8,
    /// Reserved
    reserved4: [u8; 3],
    /// Volume serial number
    volume_serial: u64,
    /// Checksum (unused)
    checksum: u32,
    /// Bootstrap code
    bootstrap: [u8; 426],
    /// Boot signature 0xAA55
    boot_signature: u16,
}

/// MFT Record Header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct MftRecordHeader {
    /// Signature "FILE"
    signature: [u8; 4],
    /// Offset to update sequence
    usa_offset: u16,
    /// Size of update sequence in words
    usa_count: u16,
    /// Log file sequence number
    lsn: u64,
    /// Sequence number
    sequence_number: u16,
    /// Hard link count
    link_count: u16,
    /// Offset to first attribute
    attrs_offset: u16,
    /// Flags (in_use, is_directory)
    flags: u16,
    /// Real size of record
    bytes_used: u32,
    /// Allocated size of record
    bytes_allocated: u32,
    /// Base MFT record (for extension records)
    base_mft_record: u64,
    /// Next attribute ID
    next_attr_id: u16,
    /// Padding (alignment)
    reserved: u16,
    /// MFT record number (NTFS 3.1+)
    mft_record_number: u32,
}

/// Attribute Record Header (common part)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct AttrRecordHeader {
    /// Attribute type
    attr_type: u32,
    /// Total length of attribute
    length: u32,
    /// Non-resident flag
    non_resident: u8,
    /// Name length in characters
    name_length: u8,
    /// Offset to name
    name_offset: u16,
    /// Flags
    flags: u16,
    /// Attribute ID
    attr_id: u16,
}

/// Resident attribute specific part
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct ResidentAttrRecord {
    /// Value length
    value_length: u32,
    /// Offset to value
    value_offset: u16,
    /// Indexed flag
    indexed: u8,
    /// Reserved
    reserved: u8,
}

/// Non-resident attribute specific part
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct NonResidentAttrRecord {
    /// Lowest VCN (virtual cluster number)
    lowest_vcn: u64,
    /// Highest VCN
    highest_vcn: u64,
    /// Offset to mapping pairs (runlist)
    mapping_pairs_offset: u16,
    /// Compression unit size (power of 2)
    compression_unit: u8,
    /// Reserved
    reserved: [u8; 5],
    /// Allocated size
    allocated_size: u64,
    /// Data size
    data_size: u64,
    /// Initialized size
    initialized_size: u64,
}

/// FILE_NAME attribute
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct FileNameAttr {
    /// Parent directory MFT reference
    parent_directory: u64,
    /// Creation time
    creation_time: u64,
    /// Modification time
    modification_time: u64,
    /// MFT modification time
    mft_modification_time: u64,
    /// Access time
    access_time: u64,
    /// Allocated size
    allocated_size: u64,
    /// Data size
    data_size: u64,
    /// File attributes
    file_attributes: u32,
    /// Reparse value or EA size
    reparse_value: u32,
    /// File name length in characters
    file_name_length: u8,
    /// File name type (POSIX, WIN32, DOS, WIN32_AND_DOS)
    file_name_type: u8,
    // Followed by file name in UTF-16
}

/// STANDARD_INFORMATION attribute
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct StandardInfoAttr {
    /// Creation time
    creation_time: u64,
    /// Modification time
    modification_time: u64,
    /// MFT modification time
    mft_modification_time: u64,
    /// Access time
    access_time: u64,
    /// File attributes
    file_attributes: u32,
    /// Maximum versions (unused in NTFS 3.x)
    max_versions: u32,
    /// Version number (unused in NTFS 3.x)
    version_number: u32,
    /// Class ID
    class_id: u32,
    /// Owner ID (NTFS 3.0+)
    owner_id: u32,
    /// Security ID (NTFS 3.0+)
    security_id: u32,
    /// Quota charged (NTFS 3.0+)
    quota_charged: u64,
    /// Update sequence number (NTFS 3.0+)
    usn: u64,
}

/// INDEX_ROOT attribute header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct IndexRootAttr {
    /// Attribute type being indexed
    indexed_attr_type: u32,
    /// Collation rule
    collation_rule: u32,
    /// Index allocation unit size
    index_block_size: u32,
    /// Index blocks per cluster
    clusters_per_index_block: u8,
    /// Reserved
    reserved: [u8; 3],
}

/// Index header (part of INDEX_ROOT and INDEX_ALLOCATION)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct IndexHeader {
    /// Offset to first entry (from this structure)
    entries_offset: u32,
    /// Index size
    index_length: u32,
    /// Allocated size
    allocated_size: u32,
    /// Flags (0x01 = has children)
    flags: u8,
    /// Reserved
    reserved: [u8; 3],
}

/// Index entry (directory entry)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct IndexEntry {
    /// MFT reference for indexed file
    indexed_file: u64,
    /// Length of this entry
    length: u16,
    /// Offset to content (for $I30 this is FILE_NAME offset)
    content_offset: u16,
    /// Flags (0x01 = has sub-node, 0x02 = last entry)
    flags: u32,
    // Followed by content (FILE_NAME for $I30 index)
}

// ============================================================================
// Runtime structures
// ============================================================================

/// Parsed file information
#[derive(Clone)]
struct NtfsFileInfo {
    /// MFT record number
    mft_record: u64,
    /// File name
    name: String,
    /// Is directory
    is_directory: bool,
    /// File attributes
    attributes: u32,
    /// Data size
    data_size: u64,
    /// Creation time (100ns ticks since 1601)
    creation_time: u64,
    /// Modification time
    modification_time: u64,
    /// Access time
    access_time: u64,
}

/// Data run (extent)
#[derive(Debug, Clone, Copy)]
struct DataRun {
    /// Virtual cluster number
    vcn: u64,
    /// Logical cluster number (0 for sparse)
    lcn: u64,
    /// Number of clusters
    length: u64,
    /// Is sparse/hole
    is_sparse: bool,
}

/// Parsed attribute info
#[derive(Clone)]
struct AttributeInfo {
    /// Attribute type
    attr_type: u32,
    /// Attribute name (empty for unnamed)
    name: String,
    /// Is non-resident
    non_resident: bool,
    /// For resident: data bytes
    resident_data: Vec<u8>,
    /// For non-resident: data runs
    data_runs: Vec<DataRun>,
    /// Data size
    data_size: u64,
    /// Allocated size
    allocated_size: u64,
}

/// NTFS volume information
#[derive(Clone)]
struct NtfsVolumeInfo {
    /// Bytes per sector
    bytes_per_sector: u32,
    /// Sectors per cluster
    sectors_per_cluster: u32,
    /// Bytes per cluster
    bytes_per_cluster: u32,
    /// MFT record size
    mft_record_size: u32,
    /// Index record size
    index_record_size: u32,
    /// MFT start cluster
    mft_lcn: u64,
    /// Total sectors
    total_sectors: u64,
    /// Volume serial
    volume_serial: u64,
}

// ============================================================================
// Filesystem implementation
// ============================================================================

/// NTFS filesystem
pub struct NtfsFs {
    /// Block device
    device: Arc<dyn BlockDevice>,
    /// Volume information
    volume_info: NtfsVolumeInfo,
    /// MFT data runs (from $MFT file)
    mft_data_runs: RwLock<Vec<DataRun>>,
    /// Root directory inode
    root: NtfsInode,
}

impl NtfsFs {
    /// Mount an NTFS filesystem
    pub fn mount(device: Arc<dyn BlockDevice>) -> KResult<Arc<Self>> {
        // Read boot sector
        let mut boot_buf = vec![0u8; 512];
        device.read_blocks(0, 1, &mut boot_buf)?;

        // Parse boot sector
        let boot = unsafe { &*(boot_buf.as_ptr() as *const NtfsBootSector) };

        // Verify signature
        if boot.oem_id != NTFS_OEM_ID {
            return Err(KError::Invalid);
        }

        if boot.boot_signature != BOOT_SIGNATURE {
            return Err(KError::Invalid);
        }

        // Extract volume info
        let bytes_per_sector = boot.bytes_per_sector as u32;
        let sectors_per_cluster = boot.sectors_per_cluster as u32;
        let bytes_per_cluster = bytes_per_sector * sectors_per_cluster;

        // Calculate MFT record size
        let mft_record_size = if boot.clusters_per_mft_record < 0 {
            // Negative means power of 2 in bytes
            1u32 << (-boot.clusters_per_mft_record as u32)
        } else {
            boot.clusters_per_mft_record as u32 * bytes_per_cluster
        };

        // Calculate index record size
        let index_record_size = if boot.clusters_per_index_record < 0 {
            1u32 << (-boot.clusters_per_index_record as u32)
        } else {
            boot.clusters_per_index_record as u32 * bytes_per_cluster
        };

        let volume_info = NtfsVolumeInfo {
            bytes_per_sector,
            sectors_per_cluster,
            bytes_per_cluster,
            mft_record_size,
            index_record_size,
            mft_lcn: boot.mft_lcn,
            total_sectors: boot.total_sectors,
            volume_serial: boot.volume_serial,
        };

        crate::kprintln!(
            "ntfs: mounted, {} bytes/sector, {} sectors/cluster, MFT at cluster {}",
            volume_info.bytes_per_sector,
            volume_info.sectors_per_cluster,
            volume_info.mft_lcn
        );

        // Read MFT record 0 ($MFT) to get MFT data runs
        let mft_data_runs = Self::read_mft_data_runs(&device, &volume_info)?;

        let root = NtfsInode {
            device: Arc::clone(&device),
            volume_info: volume_info.clone(),
            mft_record: MFT_RECORD_ROOT,
            is_directory: true,
            attributes: FILE_ATTR_DIRECTORY,
            data_size: 0,
            mft_data_runs: mft_data_runs.clone(),
            parent: None,
        };

        let fs = Arc::new(NtfsFs {
            device,
            volume_info,
            mft_data_runs: RwLock::new(mft_data_runs),
            root,
        });

        Ok(fs)
    }

    /// Read MFT data runs from $MFT (record 0)
    fn read_mft_data_runs(device: &Arc<dyn BlockDevice>, vol: &NtfsVolumeInfo) -> KResult<Vec<DataRun>> {
        // Read MFT record 0 directly from known location
        let mft_sector = vol.mft_lcn * vol.sectors_per_cluster as u64;
        let mut mft_buf = vec![0u8; vol.mft_record_size as usize];

        let sectors_per_record = vol.mft_record_size / vol.bytes_per_sector;
        device.read_blocks(mft_sector, sectors_per_record, &mut mft_buf)?;

        // Apply fixup
        apply_fixup(&mut mft_buf, vol.bytes_per_sector)?;

        // Parse MFT record header
        let header = unsafe { &*(mft_buf.as_ptr() as *const MftRecordHeader) };

        if header.signature != MFT_RECORD_SIGNATURE {
            crate::kprintln!("ntfs: invalid MFT record signature");
            return Err(KError::Invalid);
        }

        // Find $DATA attribute
        let attrs_offset = header.attrs_offset as usize;
        let mut offset = attrs_offset;

        while offset + 4 <= mft_buf.len() {
            let attr_type = u32::from_le_bytes([
                mft_buf[offset],
                mft_buf[offset + 1],
                mft_buf[offset + 2],
                mft_buf[offset + 3],
            ]);

            if attr_type == AT_END {
                break;
            }

            if offset + 8 > mft_buf.len() {
                break;
            }

            let attr_length = u32::from_le_bytes([
                mft_buf[offset + 4],
                mft_buf[offset + 5],
                mft_buf[offset + 6],
                mft_buf[offset + 7],
            ]) as usize;

            if attr_length == 0 || attr_length > mft_buf.len() - offset {
                break;
            }

            if attr_type == AT_DATA {
                // Parse data attribute
                let non_resident = mft_buf[offset + 8];
                if non_resident != 0 {
                    // Non-resident $DATA
                    let mapping_offset = u16::from_le_bytes([
                        mft_buf[offset + 0x20],
                        mft_buf[offset + 0x21],
                    ]) as usize;

                    let runs = parse_data_runs(&mft_buf[offset + mapping_offset..], attr_length - mapping_offset)?;
                    return Ok(runs);
                }
            }

            offset += attr_length;
        }

        Err(KError::NotFound)
    }

    /// Get root directory inode
    pub fn root(&self) -> Inode {
        Inode(Arc::new(self.root.clone()))
    }

    /// Read an MFT record
    fn read_mft_record(&self, record_num: u64) -> KResult<Vec<u8>> {
        read_mft_record(
            &self.device,
            &self.volume_info,
            &self.mft_data_runs.read(),
            record_num,
        )
    }
}

/// NTFS inode
#[derive(Clone)]
pub struct NtfsInode {
    device: Arc<dyn BlockDevice>,
    volume_info: NtfsVolumeInfo,
    mft_record: u64,
    is_directory: bool,
    attributes: u32,
    data_size: u64,
    mft_data_runs: Vec<DataRun>,
    parent: Option<Arc<NtfsInode>>,
}

impl NtfsInode {
    /// Read MFT record for this inode
    fn read_mft_record(&self) -> KResult<Vec<u8>> {
        read_mft_record(&self.device, &self.volume_info, &self.mft_data_runs, self.mft_record)
    }

    /// Get all attributes from MFT record
    fn get_attributes(&self) -> KResult<Vec<AttributeInfo>> {
        let mft_buf = self.read_mft_record()?;
        parse_attributes(&mft_buf, &self.volume_info)
    }

    /// Read file data
    fn read_file_data(&self) -> KResult<Vec<u8>> {
        let attrs = self.get_attributes()?;

        // Find unnamed $DATA attribute
        for attr in attrs {
            if attr.attr_type == AT_DATA && attr.name.is_empty() {
                if attr.non_resident {
                    // Non-resident: read from data runs
                    return read_data_runs(&self.device, &self.volume_info, &attr.data_runs, attr.data_size);
                } else {
                    // Resident: data is in attribute
                    return Ok(attr.resident_data);
                }
            }
        }

        Ok(Vec::new())
    }

    /// Parse directory entries
    fn parse_directory(&self) -> KResult<Vec<NtfsFileInfo>> {
        let attrs = self.get_attributes()?;
        let mut entries = Vec::new();

        // Find $INDEX_ROOT attribute for $I30 (filename index)
        for attr in &attrs {
            if attr.attr_type == AT_INDEX_ROOT && (attr.name == "$I30" || attr.name.is_empty()) {
                // Parse index root
                let root_entries = parse_index_entries(&attr.resident_data, 0x10)?; // Skip INDEX_ROOT header
                for entry in root_entries {
                    if let Some(info) = entry {
                        entries.push(info);
                    }
                }
            }
        }

        // Find $INDEX_ALLOCATION for large directories
        for attr in &attrs {
            if attr.attr_type == AT_INDEX_ALLOCATION && (attr.name == "$I30" || attr.name.is_empty()) {
                if attr.non_resident {
                    // Read index allocation blocks
                    let data = read_data_runs(&self.device, &self.volume_info, &attr.data_runs, attr.allocated_size)?;

                    // Parse each index record
                    let mut offset = 0;
                    while offset + self.volume_info.index_record_size as usize <= data.len() {
                        let record = &data[offset..offset + self.volume_info.index_record_size as usize];

                        // Check signature
                        if record[0..4] == INDEX_RECORD_SIGNATURE {
                            // Apply fixup to a copy
                            let mut record_copy = record.to_vec();
                            if apply_fixup(&mut record_copy, self.volume_info.bytes_per_sector).is_ok() {
                                // Parse index entries (skip header at 0x18 + INDEX_HEADER)
                                if let Ok(block_entries) = parse_index_entries(&record_copy, 0x18 + 0x10) {
                                    for entry in block_entries {
                                        if let Some(info) = entry {
                                            // Avoid duplicates
                                            if !entries.iter().any(|e| e.name == info.name) {
                                                entries.push(info);
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        offset += self.volume_info.index_record_size as usize;
                    }
                }
            }
        }

        Ok(entries)
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Apply NTFS fixup (update sequence)
fn apply_fixup(data: &mut [u8], sector_size: u32) -> KResult<()> {
    if data.len() < 6 {
        return Err(KError::Invalid);
    }

    let usa_offset = u16::from_le_bytes([data[4], data[5]]) as usize;
    let usa_count = u16::from_le_bytes([data[6], data[7]]) as usize;

    if usa_offset + usa_count * 2 > data.len() {
        return Err(KError::Invalid);
    }

    // Get update sequence array
    let usn = u16::from_le_bytes([data[usa_offset], data[usa_offset + 1]]);

    // Apply fixups to each sector
    for i in 1..usa_count {
        let sector_end = (i as u32 * sector_size - 2) as usize;
        if sector_end + 2 > data.len() {
            break;
        }

        // Verify USN matches
        let current = u16::from_le_bytes([data[sector_end], data[sector_end + 1]]);
        if current != usn {
            crate::kprintln!("ntfs: fixup mismatch at sector {}", i);
            return Err(KError::Invalid);
        }

        // Replace with original value
        let original_offset = usa_offset + i * 2;
        data[sector_end] = data[original_offset];
        data[sector_end + 1] = data[original_offset + 1];
    }

    Ok(())
}

/// Parse data runs (runlist) from mapping pairs
fn parse_data_runs(data: &[u8], max_len: usize) -> KResult<Vec<DataRun>> {
    let mut runs = Vec::new();
    let mut offset = 0;
    let mut current_lcn: i64 = 0;
    let mut current_vcn: u64 = 0;

    while offset < data.len().min(max_len) {
        let header = data[offset];
        if header == 0 {
            break;
        }

        let length_size = (header & 0x0F) as usize;
        let offset_size = ((header >> 4) & 0x0F) as usize;

        offset += 1;

        if offset + length_size + offset_size > data.len() {
            break;
        }

        // Read length
        let mut length: u64 = 0;
        for i in 0..length_size {
            length |= (data[offset + i] as u64) << (i * 8);
        }
        offset += length_size;

        // Read offset (signed)
        let mut lcn_delta: i64 = 0;
        if offset_size > 0 {
            for i in 0..offset_size {
                lcn_delta |= (data[offset + i] as i64) << (i * 8);
            }
            // Sign extend if negative
            let sign_bit = 1i64 << (offset_size * 8 - 1);
            if lcn_delta & sign_bit != 0 {
                lcn_delta |= !0i64 << (offset_size * 8);
            }
            offset += offset_size;

            current_lcn += lcn_delta;
        }

        let is_sparse = offset_size == 0;

        runs.push(DataRun {
            vcn: current_vcn,
            lcn: if is_sparse { 0 } else { current_lcn as u64 },
            length,
            is_sparse,
        });

        current_vcn += length;
    }

    Ok(runs)
}

/// Read MFT record by number
fn read_mft_record(
    device: &Arc<dyn BlockDevice>,
    vol: &NtfsVolumeInfo,
    mft_runs: &[DataRun],
    record_num: u64,
) -> KResult<Vec<u8>> {
    let record_size = vol.mft_record_size as usize;
    let records_per_cluster = vol.bytes_per_cluster as usize / record_size;

    // Calculate which cluster and offset
    let cluster_num = record_num as usize / records_per_cluster;
    let record_in_cluster = record_num as usize % records_per_cluster;

    // Find the cluster in data runs
    let mut vcn: u64 = 0;
    for run in mft_runs {
        if run.is_sparse {
            vcn += run.length;
            continue;
        }

        if cluster_num as u64 >= vcn && (cluster_num as u64) < vcn + run.length {
            // Found the run
            let offset_in_run = cluster_num as u64 - vcn;
            let lcn = run.lcn + offset_in_run;

            // Read the cluster
            let sector = lcn * vol.sectors_per_cluster as u64;
            let mut buf = vec![0u8; vol.bytes_per_cluster as usize];
            device.read_blocks(sector, vol.sectors_per_cluster, &mut buf)?;

            // Extract the record
            let start = record_in_cluster * record_size;
            let mut record = buf[start..start + record_size].to_vec();

            // Apply fixup
            apply_fixup(&mut record, vol.bytes_per_sector)?;

            // Verify signature
            if record[0..4] != MFT_RECORD_SIGNATURE {
                return Err(KError::Invalid);
            }

            return Ok(record);
        }

        vcn += run.length;
    }

    Err(KError::NotFound)
}

/// Parse attributes from MFT record
fn parse_attributes(mft_buf: &[u8], vol: &NtfsVolumeInfo) -> KResult<Vec<AttributeInfo>> {
    let header = unsafe { &*(mft_buf.as_ptr() as *const MftRecordHeader) };
    let mut attrs = Vec::new();
    let mut offset = header.attrs_offset as usize;

    while offset + 4 <= mft_buf.len() {
        let attr_type = u32::from_le_bytes([
            mft_buf[offset],
            mft_buf[offset + 1],
            mft_buf[offset + 2],
            mft_buf[offset + 3],
        ]);

        if attr_type == AT_END {
            break;
        }

        if offset + 16 > mft_buf.len() {
            break;
        }

        let attr_length = u32::from_le_bytes([
            mft_buf[offset + 4],
            mft_buf[offset + 5],
            mft_buf[offset + 6],
            mft_buf[offset + 7],
        ]) as usize;

        if attr_length == 0 || attr_length > mft_buf.len() - offset {
            break;
        }

        let non_resident = mft_buf[offset + 8] != 0;
        let name_length = mft_buf[offset + 9] as usize;
        let name_offset = u16::from_le_bytes([mft_buf[offset + 10], mft_buf[offset + 11]]) as usize;

        // Get attribute name
        let name = if name_length > 0 && offset + name_offset + name_length * 2 <= mft_buf.len() {
            let name_bytes = &mft_buf[offset + name_offset..offset + name_offset + name_length * 2];
            let mut chars = Vec::with_capacity(name_length);
            for i in 0..name_length {
                chars.push(u16::from_le_bytes([name_bytes[i * 2], name_bytes[i * 2 + 1]]));
            }
            String::from_utf16_lossy(&chars)
        } else {
            String::new()
        };

        let attr_info = if non_resident {
            // Non-resident attribute
            let mapping_offset = u16::from_le_bytes([
                mft_buf[offset + 0x20],
                mft_buf[offset + 0x21],
            ]) as usize;

            let data_size = u64::from_le_bytes([
                mft_buf[offset + 0x30], mft_buf[offset + 0x31],
                mft_buf[offset + 0x32], mft_buf[offset + 0x33],
                mft_buf[offset + 0x34], mft_buf[offset + 0x35],
                mft_buf[offset + 0x36], mft_buf[offset + 0x37],
            ]);

            let allocated_size = u64::from_le_bytes([
                mft_buf[offset + 0x28], mft_buf[offset + 0x29],
                mft_buf[offset + 0x2A], mft_buf[offset + 0x2B],
                mft_buf[offset + 0x2C], mft_buf[offset + 0x2D],
                mft_buf[offset + 0x2E], mft_buf[offset + 0x2F],
            ]);

            let runs = if mapping_offset < attr_length {
                parse_data_runs(&mft_buf[offset + mapping_offset..], attr_length - mapping_offset)?
            } else {
                Vec::new()
            };

            AttributeInfo {
                attr_type,
                name,
                non_resident: true,
                resident_data: Vec::new(),
                data_runs: runs,
                data_size,
                allocated_size,
            }
        } else {
            // Resident attribute
            let value_length = u32::from_le_bytes([
                mft_buf[offset + 0x10],
                mft_buf[offset + 0x11],
                mft_buf[offset + 0x12],
                mft_buf[offset + 0x13],
            ]) as usize;

            let value_offset = u16::from_le_bytes([
                mft_buf[offset + 0x14],
                mft_buf[offset + 0x15],
            ]) as usize;

            let data = if value_offset + value_length <= attr_length {
                mft_buf[offset + value_offset..offset + value_offset + value_length].to_vec()
            } else {
                Vec::new()
            };

            AttributeInfo {
                attr_type,
                name,
                non_resident: false,
                resident_data: data,
                data_runs: Vec::new(),
                data_size: value_length as u64,
                allocated_size: value_length as u64,
            }
        };

        attrs.push(attr_info);
        offset += attr_length;
    }

    Ok(attrs)
}

/// Read data from data runs
fn read_data_runs(
    device: &Arc<dyn BlockDevice>,
    vol: &NtfsVolumeInfo,
    runs: &[DataRun],
    size: u64,
) -> KResult<Vec<u8>> {
    let mut data = Vec::with_capacity(size as usize);

    for run in runs {
        if data.len() >= size as usize {
            break;
        }

        let bytes_to_read = ((run.length * vol.bytes_per_cluster as u64) as usize)
            .min(size as usize - data.len());

        if run.is_sparse {
            // Sparse run: fill with zeros
            data.resize(data.len() + bytes_to_read, 0);
        } else {
            // Read from disk
            let sector = run.lcn * vol.sectors_per_cluster as u64;
            let sectors = (bytes_to_read + vol.bytes_per_sector as usize - 1)
                / vol.bytes_per_sector as usize;

            let mut buf = vec![0u8; sectors * vol.bytes_per_sector as usize];
            device.read_blocks(sector, sectors as u32, &mut buf)?;

            data.extend_from_slice(&buf[..bytes_to_read]);
        }
    }

    // Truncate to exact size
    data.truncate(size as usize);

    Ok(data)
}

/// Parse index entries from index data
fn parse_index_entries(data: &[u8], start_offset: usize) -> KResult<Vec<Option<NtfsFileInfo>>> {
    let mut entries = Vec::new();

    if start_offset >= data.len() {
        return Ok(entries);
    }

    let mut offset = start_offset;

    while offset + 16 <= data.len() {
        // Read index entry header
        let entry_length = u16::from_le_bytes([data[offset + 8], data[offset + 9]]) as usize;
        let flags = u32::from_le_bytes([
            data[offset + 12], data[offset + 13],
            data[offset + 14], data[offset + 15],
        ]);

        if entry_length == 0 || offset + entry_length > data.len() {
            break;
        }

        // Check for last entry
        if flags & 0x02 != 0 {
            break;
        }

        // Parse FILE_NAME attribute in entry
        let content_offset = u16::from_le_bytes([data[offset + 10], data[offset + 11]]) as usize;

        if offset + content_offset + 66 <= data.len() {
            let mft_ref = u64::from_le_bytes([
                data[offset], data[offset + 1],
                data[offset + 2], data[offset + 3],
                data[offset + 4], data[offset + 5],
                data[offset + 6], data[offset + 7],
            ]);

            // MFT record number is lower 48 bits
            let mft_record = mft_ref & 0x0000_FFFF_FFFF_FFFF;

            let fn_offset = offset + content_offset;

            let file_attributes = u32::from_le_bytes([
                data[fn_offset + 56], data[fn_offset + 57],
                data[fn_offset + 58], data[fn_offset + 59],
            ]);

            let name_length = data[fn_offset + 64] as usize;
            let name_type = data[fn_offset + 65];

            // Skip DOS-only names
            if name_type == FN_DOS {
                offset += entry_length;
                continue;
            }

            // Read file name
            if fn_offset + 66 + name_length * 2 <= data.len() {
                let name_bytes = &data[fn_offset + 66..fn_offset + 66 + name_length * 2];
                let mut chars = Vec::with_capacity(name_length);
                for i in 0..name_length {
                    chars.push(u16::from_le_bytes([name_bytes[i * 2], name_bytes[i * 2 + 1]]));
                }
                let name = String::from_utf16_lossy(&chars);

                // Skip . and .. and system files starting with $
                if name != "." && name != ".." && !name.starts_with('$') {
                    let data_size = u64::from_le_bytes([
                        data[fn_offset + 48], data[fn_offset + 49],
                        data[fn_offset + 50], data[fn_offset + 51],
                        data[fn_offset + 52], data[fn_offset + 53],
                        data[fn_offset + 54], data[fn_offset + 55],
                    ]);

                    entries.push(Some(NtfsFileInfo {
                        mft_record,
                        name,
                        is_directory: (file_attributes & FILE_ATTR_DIRECTORY) != 0,
                        attributes: file_attributes,
                        data_size,
                        creation_time: 0,
                        modification_time: 0,
                        access_time: 0,
                    }));
                }
            }
        }

        offset += entry_length;
    }

    Ok(entries)
}

// ============================================================================
// InodeOps implementation
// ============================================================================

impl InodeOps for NtfsInode {
    fn metadata(&self) -> Metadata {
        let kind = if self.is_directory {
            InodeKind::Dir
        } else {
            InodeKind::File
        };

        let mode = if self.is_directory {
            Mode::from_octal(0o755)
        } else if (self.attributes & FILE_ATTR_READONLY) != 0 {
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
        if !self.is_directory {
            return Err(KError::NotADirectory);
        }

        let entries = self.parse_directory()?;

        for entry in entries {
            // Case-insensitive comparison (NTFS is case-preserving but case-insensitive)
            if entry.name.eq_ignore_ascii_case(name) {
                // Read the MFT record to get full info
                let mft_buf = read_mft_record(
                    &self.device,
                    &self.volume_info,
                    &self.mft_data_runs,
                    entry.mft_record,
                )?;

                let header = unsafe { &*(mft_buf.as_ptr() as *const MftRecordHeader) };
                let is_directory = (header.flags & MFT_RECORD_IS_DIRECTORY) != 0;

                // Get data size from $DATA attribute
                let mut data_size = entry.data_size;
                if let Ok(attrs) = parse_attributes(&mft_buf, &self.volume_info) {
                    for attr in attrs {
                        if attr.attr_type == AT_DATA && attr.name.is_empty() {
                            data_size = attr.data_size;
                            break;
                        }
                    }
                }

                let inode = NtfsInode {
                    device: Arc::clone(&self.device),
                    volume_info: self.volume_info.clone(),
                    mft_record: entry.mft_record,
                    is_directory,
                    attributes: entry.attributes,
                    data_size,
                    mft_data_runs: self.mft_data_runs.clone(),
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

        let entries = self.parse_directory()?;
        let mut result = Vec::new();

        for entry in entries {
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

        let data = self.read_file_data()?;
        let file_size = data.len();

        if offset >= file_size {
            return Ok(0);
        }

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
            Ok(self.data_size as usize)
        }
    }
}

// ============================================================================
// Detection and mounting
// ============================================================================

/// Check if a device contains an NTFS filesystem
pub fn is_ntfs(device: &Arc<dyn BlockDevice>) -> KResult<bool> {
    let mut buf = vec![0u8; 512];
    device.read_blocks(0, 1, &mut buf)?;

    // Check OEM ID at offset 3
    if buf[3..11] == NTFS_OEM_ID {
        // Also verify boot signature
        let boot_sig = u16::from_le_bytes([buf[510], buf[511]]);
        return Ok(boot_sig == BOOT_SIGNATURE);
    }

    Ok(false)
}

/// Mount NTFS filesystem
pub fn mount(device: Arc<dyn BlockDevice>) -> KResult<Arc<NtfsFs>> {
    NtfsFs::mount(device)
}
