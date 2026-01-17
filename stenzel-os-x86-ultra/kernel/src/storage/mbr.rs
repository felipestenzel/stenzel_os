//! MBR (Master Boot Record) partition table support.
//!
//! MBR is the legacy partitioning scheme that supports up to 4 primary partitions.
//! It uses LBA addressing with 32-bit sector numbers (max ~2TB disks).

#![allow(dead_code)]

use alloc::vec::Vec;

use super::block::BlockDevice;
use crate::util::{KError, KResult};

/// Common partition types for MBR
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionType {
    Empty = 0x00,
    Fat12 = 0x01,
    Fat16Small = 0x04,
    Extended = 0x05,
    Fat16 = 0x06,
    Ntfs = 0x07,
    Fat32 = 0x0B,
    Fat32Lba = 0x0C,
    Fat16Lba = 0x0E,
    ExtendedLba = 0x0F,
    LinuxSwap = 0x82,
    Linux = 0x83,
    LinuxExtended = 0x85,
    LinuxLvm = 0x8E,
    FreeBsd = 0xA5,
    OpenBsd = 0xA6,
    NetBsd = 0xA9,
    Efi = 0xEF,
    Unknown(u8),
}

impl From<u8> for PartitionType {
    fn from(val: u8) -> Self {
        match val {
            0x00 => PartitionType::Empty,
            0x01 => PartitionType::Fat12,
            0x04 => PartitionType::Fat16Small,
            0x05 => PartitionType::Extended,
            0x06 => PartitionType::Fat16,
            0x07 => PartitionType::Ntfs,
            0x0B => PartitionType::Fat32,
            0x0C => PartitionType::Fat32Lba,
            0x0E => PartitionType::Fat16Lba,
            0x0F => PartitionType::ExtendedLba,
            0x82 => PartitionType::LinuxSwap,
            0x83 => PartitionType::Linux,
            0x85 => PartitionType::LinuxExtended,
            0x8E => PartitionType::LinuxLvm,
            0xA5 => PartitionType::FreeBsd,
            0xA6 => PartitionType::OpenBsd,
            0xA9 => PartitionType::NetBsd,
            0xEF => PartitionType::Efi,
            other => PartitionType::Unknown(other),
        }
    }
}

impl PartitionType {
    pub fn as_u8(&self) -> u8 {
        match *self {
            PartitionType::Empty => 0x00,
            PartitionType::Fat12 => 0x01,
            PartitionType::Fat16Small => 0x04,
            PartitionType::Extended => 0x05,
            PartitionType::Fat16 => 0x06,
            PartitionType::Ntfs => 0x07,
            PartitionType::Fat32 => 0x0B,
            PartitionType::Fat32Lba => 0x0C,
            PartitionType::Fat16Lba => 0x0E,
            PartitionType::ExtendedLba => 0x0F,
            PartitionType::LinuxSwap => 0x82,
            PartitionType::Linux => 0x83,
            PartitionType::LinuxExtended => 0x85,
            PartitionType::LinuxLvm => 0x8E,
            PartitionType::FreeBsd => 0xA5,
            PartitionType::OpenBsd => 0xA6,
            PartitionType::NetBsd => 0xA9,
            PartitionType::Efi => 0xEF,
            PartitionType::Unknown(v) => v,
        }
    }

    /// Returns true if this is a FAT filesystem type
    pub fn is_fat(&self) -> bool {
        matches!(
            self,
            PartitionType::Fat12
                | PartitionType::Fat16Small
                | PartitionType::Fat16
                | PartitionType::Fat32
                | PartitionType::Fat32Lba
                | PartitionType::Fat16Lba
        )
    }

    /// Returns true if this is a Linux filesystem type
    pub fn is_linux(&self) -> bool {
        matches!(
            self,
            PartitionType::Linux | PartitionType::LinuxExtended | PartitionType::LinuxLvm
        )
    }

    /// Returns true if this is an extended partition
    pub fn is_extended(&self) -> bool {
        matches!(
            self,
            PartitionType::Extended | PartitionType::ExtendedLba | PartitionType::LinuxExtended
        )
    }
}

/// An MBR partition entry
#[derive(Debug, Clone)]
pub struct MbrPartition {
    /// Boot indicator (0x80 = bootable, 0x00 = not bootable)
    pub bootable: bool,
    /// Partition type
    pub partition_type: PartitionType,
    /// Starting LBA
    pub first_lba: u32,
    /// Number of sectors
    pub num_sectors: u32,
    /// CHS start (cylinder, head, sector) - for legacy systems
    pub chs_start: (u16, u8, u8),
    /// CHS end (cylinder, head, sector) - for legacy systems
    pub chs_end: (u16, u8, u8),
}

impl MbrPartition {
    /// Returns the last LBA of this partition (inclusive)
    pub fn last_lba(&self) -> u32 {
        if self.num_sectors == 0 {
            self.first_lba
        } else {
            self.first_lba + self.num_sectors - 1
        }
    }

    /// Returns the size in bytes
    pub fn size_bytes(&self) -> u64 {
        (self.num_sectors as u64) * 512
    }
}

/// Raw MBR partition entry (16 bytes)
#[repr(C, packed)]
struct RawMbrEntry {
    boot_indicator: u8,
    chs_start: [u8; 3],
    partition_type: u8,
    chs_end: [u8; 3],
    lba_start: u32,
    num_sectors: u32,
}

const MBR_SIGNATURE: u16 = 0xAA55;
const MBR_PARTITION_TABLE_OFFSET: usize = 446;
const MBR_SIGNATURE_OFFSET: usize = 510;
const MBR_ENTRY_SIZE: usize = 16;
const MBR_MAX_ENTRIES: usize = 4;

/// Decodes CHS value from 3-byte format
/// Format: [head, sector | cyl_high, cyl_low]
fn decode_chs(chs: [u8; 3]) -> (u16, u8, u8) {
    let head = chs[0];
    let sector = chs[1] & 0x3F;
    let cylinder = ((chs[1] as u16 & 0xC0) << 2) | (chs[2] as u16);
    (cylinder, head, sector)
}

/// Read MBR partition table from block device
pub fn read_mbr(dev: &dyn BlockDevice) -> KResult<Vec<MbrPartition>> {
    let bs = dev.block_size();
    if bs < 512 {
        return Err(KError::Invalid);
    }

    // MBR is at LBA 0
    let mut buf = alloc::vec![0u8; bs as usize];
    dev.read_blocks(0, 1, &mut buf)?;

    // Check MBR signature (0x55 0xAA at offset 510-511)
    let sig = u16::from_le_bytes([buf[MBR_SIGNATURE_OFFSET], buf[MBR_SIGNATURE_OFFSET + 1]]);
    if sig != MBR_SIGNATURE {
        return Err(KError::NotFound);
    }

    let mut partitions = Vec::new();

    // Parse the 4 partition entries
    for i in 0..MBR_MAX_ENTRIES {
        let offset = MBR_PARTITION_TABLE_OFFSET + i * MBR_ENTRY_SIZE;

        // Read entry fields manually to avoid alignment issues
        let boot_indicator = buf[offset];
        let chs_start = [buf[offset + 1], buf[offset + 2], buf[offset + 3]];
        let partition_type = buf[offset + 4];
        let chs_end = [buf[offset + 5], buf[offset + 6], buf[offset + 7]];
        let lba_start =
            u32::from_le_bytes([buf[offset + 8], buf[offset + 9], buf[offset + 10], buf[offset + 11]]);
        let num_sectors = u32::from_le_bytes([
            buf[offset + 12],
            buf[offset + 13],
            buf[offset + 14],
            buf[offset + 15],
        ]);

        // Skip empty entries
        if partition_type == 0x00 || num_sectors == 0 {
            continue;
        }

        partitions.push(MbrPartition {
            bootable: boot_indicator == 0x80,
            partition_type: PartitionType::from(partition_type),
            first_lba: lba_start,
            num_sectors,
            chs_start: decode_chs(chs_start),
            chs_end: decode_chs(chs_end),
        });
    }

    Ok(partitions)
}

/// Check if device has MBR partition table
pub fn has_mbr(dev: &dyn BlockDevice) -> bool {
    read_mbr(dev).is_ok()
}

/// Read MBR and any logical partitions from extended partitions
pub fn read_mbr_with_logical(dev: &dyn BlockDevice) -> KResult<Vec<MbrPartition>> {
    let mut partitions = read_mbr(dev)?;

    // Find extended partitions and read logical partitions
    let extended: Vec<_> = partitions
        .iter()
        .filter(|p| p.partition_type.is_extended())
        .cloned()
        .collect();

    for ext in extended {
        if let Ok(logical) = read_extended_partitions(dev, ext.first_lba, ext.first_lba) {
            partitions.extend(logical);
        }
    }

    Ok(partitions)
}

/// Read logical partitions from an extended partition
fn read_extended_partitions(
    dev: &dyn BlockDevice,
    ebr_lba: u32,
    extended_start: u32,
) -> KResult<Vec<MbrPartition>> {
    let bs = dev.block_size();
    let mut buf = alloc::vec![0u8; bs as usize];
    let mut partitions = Vec::new();
    let mut current_lba = ebr_lba;

    // Safety limit to prevent infinite loops
    const MAX_LOGICAL: usize = 128;

    for _ in 0..MAX_LOGICAL {
        dev.read_blocks(current_lba as u64, 1, &mut buf)?;

        // Check signature
        let sig = u16::from_le_bytes([buf[MBR_SIGNATURE_OFFSET], buf[MBR_SIGNATURE_OFFSET + 1]]);
        if sig != MBR_SIGNATURE {
            break;
        }

        // First entry is the logical partition
        let offset = MBR_PARTITION_TABLE_OFFSET;
        let partition_type = buf[offset + 4];
        let lba_start = u32::from_le_bytes([
            buf[offset + 8],
            buf[offset + 9],
            buf[offset + 10],
            buf[offset + 11],
        ]);
        let num_sectors = u32::from_le_bytes([
            buf[offset + 12],
            buf[offset + 13],
            buf[offset + 14],
            buf[offset + 15],
        ]);

        if partition_type != 0x00 && num_sectors > 0 {
            let chs_start = [buf[offset + 1], buf[offset + 2], buf[offset + 3]];
            let chs_end = [buf[offset + 5], buf[offset + 6], buf[offset + 7]];

            partitions.push(MbrPartition {
                bootable: buf[offset] == 0x80,
                partition_type: PartitionType::from(partition_type),
                first_lba: current_lba + lba_start,
                num_sectors,
                chs_start: decode_chs(chs_start),
                chs_end: decode_chs(chs_end),
            });
        }

        // Second entry points to next EBR (if any)
        let offset2 = MBR_PARTITION_TABLE_OFFSET + MBR_ENTRY_SIZE;
        let next_type = buf[offset2 + 4];
        let next_lba = u32::from_le_bytes([
            buf[offset2 + 8],
            buf[offset2 + 9],
            buf[offset2 + 10],
            buf[offset2 + 11],
        ]);

        if next_type == 0x00 || next_lba == 0 {
            break;
        }

        // Next EBR is relative to start of extended partition
        current_lba = extended_start + next_lba;
    }

    Ok(partitions)
}

/// Print MBR partition table info
pub fn print_mbr_info(partitions: &[MbrPartition]) {
    crate::kprintln!("mbr: {} partições", partitions.len());
    for (i, p) in partitions.iter().enumerate() {
        let boot_flag = if p.bootable { "*" } else { " " };
        let type_str = match p.partition_type {
            PartitionType::Empty => "Empty",
            PartitionType::Fat12 => "FAT12",
            PartitionType::Fat16Small => "FAT16",
            PartitionType::Fat16 => "FAT16",
            PartitionType::Fat32 => "FAT32",
            PartitionType::Fat32Lba => "FAT32 LBA",
            PartitionType::Fat16Lba => "FAT16 LBA",
            PartitionType::Ntfs => "NTFS",
            PartitionType::Linux => "Linux",
            PartitionType::LinuxSwap => "Linux swap",
            PartitionType::LinuxLvm => "Linux LVM",
            PartitionType::Extended | PartitionType::ExtendedLba => "Extended",
            PartitionType::LinuxExtended => "Linux ext",
            PartitionType::Efi => "EFI",
            PartitionType::FreeBsd => "FreeBSD",
            PartitionType::OpenBsd => "OpenBSD",
            PartitionType::NetBsd => "NetBSD",
            PartitionType::Unknown(v) => {
                crate::kprintln!(
                    "  {}{}: type=0x{:02x} lba_start={} sectors={} ({} MB)",
                    boot_flag,
                    i,
                    v,
                    p.first_lba,
                    p.num_sectors,
                    p.size_bytes() / (1024 * 1024)
                );
                continue;
            }
        };
        crate::kprintln!(
            "  {}{}: {} lba_start={} sectors={} ({} MB)",
            boot_flag,
            i,
            type_str,
            p.first_lba,
            p.num_sectors,
            p.size_bytes() / (1024 * 1024)
        );
    }
}
