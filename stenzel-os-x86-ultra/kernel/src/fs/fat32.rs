//! FAT32 filesystem driver (read-only)
//!
//! FAT32 is commonly used on USB drives, SD cards, and for EFI system partitions.
//! This implementation supports:
//! - FAT32 (not FAT12/FAT16)
//! - Long File Names (LFN)
//! - Read-only access
//!
//! References:
//! - Microsoft FAT Specification
//! - https://wiki.osdev.org/FAT

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

const FAT32_SIGNATURE: u16 = 0xAA55;
const FAT32_FS_TYPE: [u8; 8] = *b"FAT32   ";

// FAT entry values
const FAT32_CLUSTER_FREE: u32 = 0x00000000;
const FAT32_CLUSTER_RESERVED: u32 = 0x0FFFFFF0;
const FAT32_CLUSTER_BAD: u32 = 0x0FFFFFF7;
const FAT32_CLUSTER_END: u32 = 0x0FFFFFF8;

// Directory entry attributes
const ATTR_READ_ONLY: u8 = 0x01;
const ATTR_HIDDEN: u8 = 0x02;
const ATTR_SYSTEM: u8 = 0x04;
const ATTR_VOLUME_ID: u8 = 0x08;
const ATTR_DIRECTORY: u8 = 0x10;
const ATTR_ARCHIVE: u8 = 0x20;
const ATTR_LONG_NAME: u8 = ATTR_READ_ONLY | ATTR_HIDDEN | ATTR_SYSTEM | ATTR_VOLUME_ID;
const ATTR_LONG_NAME_MASK: u8 = ATTR_READ_ONLY | ATTR_HIDDEN | ATTR_SYSTEM | ATTR_VOLUME_ID | ATTR_DIRECTORY | ATTR_ARCHIVE;

// ============================================================================
// On-disk structures (packed, little-endian)
// ============================================================================

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Fat32Bpb {
    jmp_boot: [u8; 3],
    oem_name: [u8; 8],
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sector_count: u16,
    num_fats: u8,
    root_entry_count: u16,  // 0 for FAT32
    total_sectors_16: u16,  // 0 for FAT32
    media: u8,
    fat_size_16: u16,       // 0 for FAT32
    sectors_per_track: u16,
    num_heads: u16,
    hidden_sectors: u32,
    total_sectors_32: u32,
    // FAT32 specific
    fat_size_32: u32,
    ext_flags: u16,
    fs_version: u16,
    root_cluster: u32,
    fs_info: u16,
    backup_boot_sector: u16,
    reserved: [u8; 12],
    drive_number: u8,
    reserved1: u8,
    boot_sig: u8,
    volume_id: u32,
    volume_label: [u8; 11],
    fs_type: [u8; 8],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Fat32DirEntry {
    name: [u8; 11],       // 8.3 filename
    attr: u8,
    nt_reserved: u8,
    create_time_tenths: u8,
    create_time: u16,
    create_date: u16,
    access_date: u16,
    first_cluster_hi: u16,
    write_time: u16,
    write_date: u16,
    first_cluster_lo: u16,
    file_size: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Fat32LfnEntry {
    order: u8,           // Sequence number (1-20), bit 6 = last entry
    name1: [u16; 5],     // Characters 1-5
    attr: u8,            // Always ATTR_LONG_NAME (0x0F)
    lfn_type: u8,        // Always 0
    checksum: u8,        // Checksum of DOS filename
    name2: [u16; 6],     // Characters 6-11
    first_cluster_lo: u16, // Always 0
    name3: [u16; 2],     // Characters 12-13
}

impl Fat32DirEntry {
    fn first_cluster(&self) -> u32 {
        ((self.first_cluster_hi as u32) << 16) | (self.first_cluster_lo as u32)
    }

    fn is_free(&self) -> bool {
        self.name[0] == 0xE5
    }

    fn is_end(&self) -> bool {
        self.name[0] == 0x00
    }

    fn is_lfn(&self) -> bool {
        (self.attr & ATTR_LONG_NAME_MASK) == ATTR_LONG_NAME
    }

    fn is_directory(&self) -> bool {
        (self.attr & ATTR_DIRECTORY) != 0
    }

    fn is_volume_label(&self) -> bool {
        (self.attr & ATTR_VOLUME_ID) != 0 && (self.attr & ATTR_DIRECTORY) == 0
    }

    /// Extract 8.3 filename
    fn short_name(&self) -> String {
        let mut name = String::new();

        // Name part (first 8 chars)
        for i in 0..8 {
            let c = self.name[i];
            if c == b' ' {
                break;
            }
            // Handle special first character
            if i == 0 && c == 0x05 {
                name.push(0xE5 as char);
            } else {
                name.push(c as char);
            }
        }

        // Extension part (last 3 chars)
        let ext_start = 8;
        if self.name[ext_start] != b' ' {
            name.push('.');
            for i in ext_start..11 {
                let c = self.name[i];
                if c == b' ' {
                    break;
                }
                name.push(c as char);
            }
        }

        name
    }
}

impl Fat32LfnEntry {
    fn sequence_number(&self) -> u8 {
        self.order & 0x1F
    }

    fn is_last(&self) -> bool {
        (self.order & 0x40) != 0
    }

    /// Extract characters from LFN entry
    fn chars(&self) -> [u16; 13] {
        let mut chars = [0u16; 13];

        // Read name1 (5 chars)
        for i in 0..5 {
            chars[i] = unsafe {
                core::ptr::read_unaligned(core::ptr::addr_of!(self.name1[i]))
            };
        }

        // Read name2 (6 chars)
        for i in 0..6 {
            chars[5 + i] = unsafe {
                core::ptr::read_unaligned(core::ptr::addr_of!(self.name2[i]))
            };
        }

        // Read name3 (2 chars)
        for i in 0..2 {
            chars[11 + i] = unsafe {
                core::ptr::read_unaligned(core::ptr::addr_of!(self.name3[i]))
            };
        }

        chars
    }
}

// ============================================================================
// Driver structures
// ============================================================================

/// Mounted FAT32 filesystem (read-only)
pub struct Fat32Fs {
    device: Arc<dyn BlockDevice>,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    num_fats: u8,
    fat_size: u32,
    root_cluster: u32,
    total_sectors: u32,
    data_start_sector: u32,
}

impl Fat32Fs {
    /// Mount a FAT32 filesystem from a block device
    pub fn mount(device: Arc<dyn BlockDevice>) -> KResult<Arc<Self>> {
        // Read boot sector (first sector)
        let mut buf = vec![0u8; device.block_size() as usize];
        device.read_blocks(0, 1, &mut buf)?;

        let bpb: Fat32Bpb = unsafe {
            core::ptr::read_unaligned(buf.as_ptr() as *const _)
        };

        // Check signature
        let signature = unsafe {
            core::ptr::read_unaligned(buf.as_ptr().add(510) as *const u16)
        };
        if signature != FAT32_SIGNATURE {
            crate::kprintln!("fat32: invalid signature 0x{:04x}", signature);
            return Err(KError::Invalid);
        }

        // Verify FAT32 (not FAT12/FAT16)
        let bytes_per_sector = unsafe {
            core::ptr::read_unaligned(core::ptr::addr_of!(bpb.bytes_per_sector))
        };
        let sectors_per_cluster = bpb.sectors_per_cluster;
        let reserved_sectors = unsafe {
            core::ptr::read_unaligned(core::ptr::addr_of!(bpb.reserved_sector_count))
        };
        let num_fats = bpb.num_fats;
        let fat_size = unsafe {
            core::ptr::read_unaligned(core::ptr::addr_of!(bpb.fat_size_32))
        };
        let root_cluster = unsafe {
            core::ptr::read_unaligned(core::ptr::addr_of!(bpb.root_cluster))
        };
        let total_sectors = unsafe {
            core::ptr::read_unaligned(core::ptr::addr_of!(bpb.total_sectors_32))
        };

        // Validate parameters
        if bytes_per_sector < 512 || sectors_per_cluster == 0 || num_fats == 0 {
            crate::kprintln!("fat32: invalid BPB parameters");
            return Err(KError::Invalid);
        }

        // Calculate data start sector
        let data_start_sector = reserved_sectors as u32 + (num_fats as u32 * fat_size);

        // Calculate number of data clusters
        let data_sectors = total_sectors - data_start_sector;
        let cluster_count = data_sectors / sectors_per_cluster as u32;

        // FAT32 requires at least 65525 clusters
        if cluster_count < 65525 {
            crate::kprintln!("fat32: cluster count {} < 65525, not FAT32", cluster_count);
            return Err(KError::Invalid);
        }

        crate::kprintln!(
            "fat32: mounted (clusters={}, bytes_per_sector={}, sectors_per_cluster={})",
            cluster_count,
            bytes_per_sector,
            sectors_per_cluster
        );

        Ok(Arc::new(Self {
            device,
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors,
            num_fats,
            fat_size,
            root_cluster,
            total_sectors,
            data_start_sector,
        }))
    }

    /// Read a sector from the device
    fn read_sector(&self, sector: u32, buf: &mut [u8]) -> KResult<()> {
        let dev_block_size = self.device.block_size();
        let factor = self.bytes_per_sector / dev_block_size as u16;
        let lba = sector as u64 * factor as u64;
        self.device.read_blocks(lba, factor as u32, buf)
    }

    /// Get the first sector of a cluster
    fn cluster_to_sector(&self, cluster: u32) -> u32 {
        self.data_start_sector + (cluster - 2) * self.sectors_per_cluster as u32
    }

    /// Read a cluster into buffer
    fn read_cluster(&self, cluster: u32, buf: &mut [u8]) -> KResult<()> {
        let sector = self.cluster_to_sector(cluster);
        let sectors_per_cluster = self.sectors_per_cluster as u32;
        let bytes_per_sector = self.bytes_per_sector as usize;

        for i in 0..sectors_per_cluster {
            let offset = i as usize * bytes_per_sector;
            self.read_sector(sector + i, &mut buf[offset..offset + bytes_per_sector])?;
        }

        Ok(())
    }

    /// Get the next cluster in the chain (from FAT)
    fn next_cluster(&self, cluster: u32) -> KResult<Option<u32>> {
        let fat_offset = cluster * 4;
        let fat_sector = self.reserved_sectors as u32 + (fat_offset / self.bytes_per_sector as u32);
        let offset_in_sector = (fat_offset % self.bytes_per_sector as u32) as usize;

        let mut sector_buf = vec![0u8; self.bytes_per_sector as usize];
        self.read_sector(fat_sector, &mut sector_buf)?;

        let entry = unsafe {
            core::ptr::read_unaligned(sector_buf.as_ptr().add(offset_in_sector) as *const u32)
        };
        let entry = entry & 0x0FFFFFFF; // Mask off reserved bits

        if entry >= FAT32_CLUSTER_END {
            Ok(None) // End of chain
        } else if entry == FAT32_CLUSTER_FREE || entry == FAT32_CLUSTER_BAD {
            Ok(None) // Invalid
        } else {
            Ok(Some(entry))
        }
    }

    /// Read all clusters in a chain
    fn read_cluster_chain(&self, start_cluster: u32) -> KResult<Vec<u8>> {
        let cluster_size = self.bytes_per_sector as usize * self.sectors_per_cluster as usize;
        let mut data = Vec::new();
        let mut current = start_cluster;

        loop {
            let offset = data.len();
            data.resize(offset + cluster_size, 0);
            self.read_cluster(current, &mut data[offset..])?;

            match self.next_cluster(current)? {
                Some(next) => current = next,
                None => break,
            }
        }

        Ok(data)
    }

    /// Read directory entries from a cluster chain
    fn read_directory(&self, start_cluster: u32) -> KResult<Vec<(String, Fat32DirEntry)>> {
        let data = self.read_cluster_chain(start_cluster)?;
        let mut entries = Vec::new();
        let mut lfn_parts: Vec<(u8, [u16; 13])> = Vec::new();

        let entry_size = core::mem::size_of::<Fat32DirEntry>();
        let mut offset = 0;

        while offset + entry_size <= data.len() {
            let entry: Fat32DirEntry = unsafe {
                core::ptr::read_unaligned(data.as_ptr().add(offset) as *const _)
            };

            if entry.is_end() {
                break;
            }

            if entry.is_free() {
                lfn_parts.clear();
                offset += entry_size;
                continue;
            }

            if entry.is_lfn() {
                // Long filename entry
                let lfn: Fat32LfnEntry = unsafe {
                    core::ptr::read_unaligned(data.as_ptr().add(offset) as *const _)
                };
                let seq = lfn.sequence_number();
                let chars = lfn.chars();

                if lfn.is_last() {
                    lfn_parts.clear();
                }
                lfn_parts.push((seq, chars));
            } else if !entry.is_volume_label() {
                // Regular entry
                let name = if !lfn_parts.is_empty() {
                    // Assemble long filename
                    lfn_parts.sort_by_key(|(seq, _)| *seq);
                    let mut name = String::new();
                    for (_, chars) in &lfn_parts {
                        for &c in chars {
                            if c == 0 || c == 0xFFFF {
                                break;
                            }
                            if let Some(ch) = char::from_u32(c as u32) {
                                name.push(ch);
                            }
                        }
                    }
                    lfn_parts.clear();
                    name
                } else {
                    // Use short name
                    entry.short_name()
                };

                // Skip . and ..
                if name != "." && name != ".." {
                    entries.push((name, entry));
                }
            }

            offset += entry_size;
        }

        Ok(entries)
    }

    /// Return root inode
    pub fn root(self: &Arc<Self>) -> Inode {
        Inode(Arc::new(Fat32InodeWrapper {
            fs: Arc::clone(self),
            cluster: self.root_cluster,
            size: 0, // Directory size is dynamic
            is_dir: true,
            parent: None,
        }))
    }
}

// ============================================================================
// InodeOps implementation
// ============================================================================

struct Fat32InodeWrapper {
    fs: Arc<Fat32Fs>,
    cluster: u32,
    size: u32,
    is_dir: bool,
    parent: Option<Inode>,
}

impl InodeOps for Fat32InodeWrapper {
    fn metadata(&self) -> Metadata {
        Metadata::simple(
            Uid(0),
            Gid(0),
            Mode::from_octal(if self.is_dir { 0o755 } else { 0o644 }),
            if self.is_dir { InodeKind::Dir } else { InodeKind::File },
        )
    }

    fn set_metadata(&self, _meta: Metadata) {
        // Read-only filesystem
    }

    fn parent(&self) -> Option<Inode> {
        self.parent.clone()
    }

    fn lookup(&self, name: &str) -> KResult<Inode> {
        if !self.is_dir {
            return Err(KError::NotFound);
        }

        let entries = self.fs.read_directory(self.cluster)?;

        for (entry_name, entry) in entries {
            if entry_name.eq_ignore_ascii_case(name) {
                let child = Arc::new(Fat32InodeWrapper {
                    fs: Arc::clone(&self.fs),
                    cluster: entry.first_cluster(),
                    size: unsafe {
                        core::ptr::read_unaligned(core::ptr::addr_of!(entry.file_size))
                    },
                    is_dir: entry.is_directory(),
                    parent: Some(Inode(Arc::new(Fat32InodeWrapper {
                        fs: Arc::clone(&self.fs),
                        cluster: self.cluster,
                        size: self.size,
                        is_dir: true,
                        parent: self.parent.clone(),
                    }))),
                });
                return Ok(Inode(child));
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
            return Err(KError::Invalid);
        }

        let entries = self.fs.read_directory(self.cluster)?;
        let mut result = Vec::new();

        for (name, entry) in entries {
            result.push(DirEntry {
                name,
                kind: if entry.is_directory() {
                    InodeKind::Dir
                } else {
                    InodeKind::File
                },
            });
        }

        Ok(result)
    }

    fn read_at(&self, offset: usize, out: &mut [u8]) -> KResult<usize> {
        if self.is_dir {
            return Err(KError::Invalid);
        }

        let file_size = self.size as usize;
        if offset >= file_size {
            return Ok(0);
        }

        let to_read = core::cmp::min(out.len(), file_size - offset);

        // Read cluster chain
        let data = self.fs.read_cluster_chain(self.cluster)?;

        if offset + to_read > data.len() {
            return Err(KError::IO);
        }

        out[..to_read].copy_from_slice(&data[offset..offset + to_read]);
        Ok(to_read)
    }

    fn write_at(&self, _offset: usize, _data: &[u8]) -> KResult<usize> {
        // Read-only filesystem
        Err(KError::NotSupported)
    }

    fn truncate(&self, _size: usize) -> KResult<()> {
        Err(KError::NotSupported)
    }

    fn size(&self) -> KResult<usize> {
        Ok(self.size as usize)
    }

    fn unlink(&self, _name: &str) -> KResult<()> {
        Err(KError::NotSupported)
    }

    fn rmdir(&self, _name: &str) -> KResult<()> {
        Err(KError::NotSupported)
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Try to mount FAT32 filesystem
pub fn try_mount(device: Arc<dyn BlockDevice>) -> KResult<Arc<Fat32Fs>> {
    Fat32Fs::mount(device)
}

/// Check if a device contains a FAT32 filesystem
pub fn is_fat32(device: &Arc<dyn BlockDevice>) -> KResult<bool> {
    let mut buf = vec![0u8; device.block_size() as usize];
    device.read_blocks(0, 1, &mut buf)?;

    // Check signature
    let signature = unsafe {
        core::ptr::read_unaligned(buf.as_ptr().add(510) as *const u16)
    };
    if signature != FAT32_SIGNATURE {
        return Ok(false);
    }

    // Check FAT type string (offset 82)
    let fs_type = &buf[82..90];
    Ok(fs_type == &FAT32_FS_TYPE)
}
