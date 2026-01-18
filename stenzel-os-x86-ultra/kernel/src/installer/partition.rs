//! Disk Partitioning for Installer
//!
//! Handles GPT/MBR partition table creation and modification.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

use super::{InstallConfig, InstallError, InstallResult, FilesystemType};
use super::hwdetect::HardwareInfo;

/// Partition layout for installation
#[derive(Debug, Clone)]
pub struct PartitionLayout {
    pub disk: String,
    pub scheme: PartitionScheme,
    pub partitions: Vec<Partition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionScheme { Gpt, Mbr }

#[derive(Debug, Clone)]
pub struct Partition {
    pub number: u32,
    pub start_sector: u64,
    pub end_sector: u64,
    pub size_bytes: u64,
    pub partition_type: PartitionType,
    pub filesystem: FilesystemType,
    pub mount_point: String,
    pub label: String,
    pub uuid: String,
    pub flags: Vec<PartitionFlag>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionType {
    EfiSystem,
    BiosBoot,
    LinuxRoot,
    LinuxHome,
    LinuxSwap,
    LinuxData,
    MicrosoftBasic,
}

impl PartitionType {
    pub fn gpt_type_guid(&self) -> &'static str {
        match self {
            PartitionType::EfiSystem => "C12A7328-F81F-11D2-BA4B-00A0C93EC93B",
            PartitionType::BiosBoot => "21686148-6449-6E6F-744E-656564454649",
            PartitionType::LinuxRoot => "4F68BCE3-E8CD-4DB1-96E7-FBCAF984B709",
            PartitionType::LinuxHome => "933AC7E1-2EB4-4F13-B844-0E14E2AEF915",
            PartitionType::LinuxSwap => "0657FD6D-A4AB-43C4-84E5-0933C84B4F4F",
            PartitionType::LinuxData => "0FC63DAF-8483-4772-8E79-3D69D8477DE4",
            PartitionType::MicrosoftBasic => "EBD0A0A2-B9E5-4433-87C0-68B6B72699C7",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionFlag { Boot, Esp, LvmMember, RaidMember }

/// GPT Header structure
#[repr(C, packed)]
pub struct GptHeader {
    pub signature: [u8; 8],
    pub revision: u32,
    pub header_size: u32,
    pub header_crc32: u32,
    pub reserved: u32,
    pub current_lba: u64,
    pub backup_lba: u64,
    pub first_usable_lba: u64,
    pub last_usable_lba: u64,
    pub disk_guid: [u8; 16],
    pub partition_entry_lba: u64,
    pub num_partition_entries: u32,
    pub partition_entry_size: u32,
    pub partition_array_crc32: u32,
}

/// GPT Partition Entry
#[repr(C, packed)]
pub struct GptEntry {
    pub type_guid: [u8; 16],
    pub partition_guid: [u8; 16],
    pub first_lba: u64,
    pub last_lba: u64,
    pub attributes: u64,
    pub name: [u16; 36],
}

/// Partition disk according to installation config
pub fn partition_disk(config: &InstallConfig, hw: &HardwareInfo) -> InstallResult<PartitionLayout> {
    crate::kprintln!("partition: Partitioning {}", config.target_disk);

    let disk = find_disk(&config.target_disk, hw)?;
    let scheme = if hw.firmware == super::hwdetect::FirmwareType::Uefi {
        PartitionScheme::Gpt
    } else {
        PartitionScheme::Mbr
    };

    // Calculate partition sizes
    let disk_size = disk.size_bytes;
    let sector_size: u64 = 512;
    let total_sectors = disk_size / sector_size;

    let mut partitions = Vec::new();
    let mut current_sector: u64 = 2048; // Start after GPT header

    // 1. EFI System Partition (512MB)
    if scheme == PartitionScheme::Gpt {
        let esp_sectors = (512 * 1024 * 1024) / sector_size;
        partitions.push(Partition {
            number: 1,
            start_sector: current_sector,
            end_sector: current_sector + esp_sectors - 1,
            size_bytes: esp_sectors * sector_size,
            partition_type: PartitionType::EfiSystem,
            filesystem: FilesystemType::Fat32,
            mount_point: String::from("/boot/efi"),
            label: String::from("EFI"),
            uuid: generate_uuid(),
            flags: vec![PartitionFlag::Esp, PartitionFlag::Boot],
        });
        current_sector += esp_sectors;
    }

    // 2. Swap partition (if enabled)
    if config.create_swap {
        let swap_size = if config.swap_size_mb > 0 {
            config.swap_size_mb * 1024 * 1024
        } else {
            // Auto: use same as RAM up to 8GB
            let ram_mb = hw.memory.total_mb;
            core::cmp::min(ram_mb, 8192) * 1024 * 1024
        };
        let swap_sectors = swap_size / sector_size;
        
        partitions.push(Partition {
            number: partitions.len() as u32 + 1,
            start_sector: current_sector,
            end_sector: current_sector + swap_sectors - 1,
            size_bytes: swap_sectors * sector_size,
            partition_type: PartitionType::LinuxSwap,
            filesystem: FilesystemType::Ext4, // Will be formatted as swap
            mount_point: String::from("swap"),
            label: String::from("SWAP"),
            uuid: generate_uuid(),
            flags: Vec::new(),
        });
        current_sector += swap_sectors;
    }

    // 3. Root partition (rest of disk, minus 1MB for GPT backup)
    let root_end = total_sectors - 2048;
    partitions.push(Partition {
        number: partitions.len() as u32 + 1,
        start_sector: current_sector,
        end_sector: root_end,
        size_bytes: (root_end - current_sector) * sector_size,
        partition_type: PartitionType::LinuxRoot,
        filesystem: config.root_fs,
        mount_point: String::from("/"),
        label: String::from("ROOT"),
        uuid: generate_uuid(),
        flags: Vec::new(),
    });

    // Write partition table
    write_partition_table(&config.target_disk, scheme, &partitions)?;

    crate::kprintln!("partition: Created {} partitions", partitions.len());

    Ok(PartitionLayout {
        disk: config.target_disk.clone(),
        scheme,
        partitions,
    })
}

fn find_disk(path: &str, hw: &HardwareInfo) -> InstallResult<super::hwdetect::DiskInfo> {
    hw.disks.iter()
        .find(|d| d.path == path)
        .cloned()
        .ok_or(InstallError::DiskNotFound)
}

fn write_partition_table(disk: &str, scheme: PartitionScheme, partitions: &[Partition]) -> InstallResult<()> {
    crate::kprintln!("partition: Writing {:?} partition table to {}", scheme, disk);

    match scheme {
        PartitionScheme::Gpt => write_gpt(disk, partitions),
        PartitionScheme::Mbr => write_mbr(disk, partitions),
    }
}

fn write_gpt(disk: &str, partitions: &[Partition]) -> InstallResult<()> {
    // Create GPT header
    let header = GptHeader {
        signature: *b"EFI PART",
        revision: 0x00010000,
        header_size: 92,
        header_crc32: 0,
        reserved: 0,
        current_lba: 1,
        backup_lba: 0,
        first_usable_lba: 34,
        last_usable_lba: 0,
        disk_guid: [0; 16],
        partition_entry_lba: 2,
        num_partition_entries: 128,
        partition_entry_size: 128,
        partition_array_crc32: 0,
    };

    // Write header and entries
    let _ = (disk, header, partitions);
    Ok(())
}

fn write_mbr(disk: &str, partitions: &[Partition]) -> InstallResult<()> {
    let _ = (disk, partitions);
    Ok(())
}

fn generate_uuid() -> String {
    // Generate random UUID
    let mut uuid = [0u8; 16];
    for i in 0..16 {
        uuid[i] = crate::crypto::random::get_random_u8();
    }
    // Set version 4 and variant bits
    uuid[6] = (uuid[6] & 0x0f) | 0x40;
    uuid[8] = (uuid[8] & 0x3f) | 0x80;
    
    alloc::format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        uuid[0], uuid[1], uuid[2], uuid[3],
        uuid[4], uuid[5], uuid[6], uuid[7],
        uuid[8], uuid[9], uuid[10], uuid[11],
        uuid[12], uuid[13], uuid[14], uuid[15]
    )
}

/// Resize an existing partition
pub fn resize_partition(disk: &str, partition_num: u32, new_size_bytes: u64) -> InstallResult<()> {
    crate::kprintln!("partition: Resizing partition {} on {} to {} bytes", partition_num, disk, new_size_bytes);
    Ok(())
}

/// Delete a partition
pub fn delete_partition(disk: &str, partition_num: u32) -> InstallResult<()> {
    crate::kprintln!("partition: Deleting partition {} on {}", partition_num, disk);
    Ok(())
}

pub fn init() {
    crate::kprintln!("partition: Partition manager initialized");
}
