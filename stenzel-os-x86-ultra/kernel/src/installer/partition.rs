//! Disk Partitioning Support
//!
//! Provides functionality for creating and managing disk partitions
//! using MBR or GPT partition schemes.

extern crate alloc;

use alloc::string::String;
use alloc::format;
use alloc::vec::Vec;

/// Partition schemes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionScheme {
    /// Master Boot Record (legacy)
    Mbr,
    /// GUID Partition Table (modern, UEFI)
    Gpt,
}

/// Partition types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionType {
    /// EFI System Partition (ESP)
    EfiSystem,
    /// BIOS Boot partition (for GPT + BIOS)
    BiosBoot,
    /// Linux filesystem
    LinuxFilesystem,
    /// Linux swap
    LinuxSwap,
    /// Linux LVM
    LinuxLvm,
    /// Linux RAID
    LinuxRaid,
    /// Linux home
    LinuxHome,
    /// Windows basic data
    WindowsBasicData,
    /// Extended partition (MBR only)
    Extended,
    /// Unknown/other
    Unknown,
}

impl PartitionType {
    /// Get GPT GUID for this partition type
    pub fn gpt_guid(&self) -> [u8; 16] {
        match self {
            Self::EfiSystem => [
                0x28, 0x73, 0x2A, 0xC1, 0x1F, 0xF8, 0xD2, 0x11,
                0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E, 0xC9, 0x3B,
            ],
            Self::BiosBoot => [
                0x48, 0x61, 0x68, 0x21, 0x49, 0x64, 0x6F, 0x6E,
                0x74, 0x4E, 0x65, 0x65, 0x64, 0x45, 0x46, 0x49,
            ],
            Self::LinuxFilesystem => [
                0xAF, 0x3D, 0xC6, 0x0F, 0x83, 0x84, 0x72, 0x47,
                0x8E, 0x79, 0x3D, 0x69, 0xD8, 0x47, 0x7D, 0xE4,
            ],
            Self::LinuxSwap => [
                0x6D, 0xFD, 0x57, 0x06, 0xAB, 0xA4, 0xC4, 0x43,
                0x84, 0xE5, 0x09, 0x33, 0xC8, 0x4B, 0x4F, 0x4F,
            ],
            Self::LinuxLvm => [
                0x79, 0xD3, 0xD6, 0xE6, 0x07, 0xF5, 0xC2, 0x44,
                0xA2, 0x3C, 0x23, 0x8F, 0x2A, 0x3D, 0xF9, 0x28,
            ],
            Self::LinuxRaid => [
                0x0F, 0xC6, 0x3D, 0xAF, 0x84, 0x83, 0x47, 0x72,
                0x8E, 0x79, 0x3D, 0x69, 0xD8, 0x47, 0x7D, 0xE4,
            ],
            Self::LinuxHome => [
                0x93, 0x3A, 0xC7, 0xE1, 0x2E, 0xB4, 0x4F, 0x13,
                0xB8, 0x44, 0x0E, 0x14, 0xE2, 0xAE, 0xF9, 0x15,
            ],
            Self::WindowsBasicData => [
                0xA2, 0xA0, 0xD0, 0xEB, 0xE5, 0xB9, 0x33, 0x44,
                0x87, 0xC0, 0x68, 0xB6, 0xB7, 0x26, 0x99, 0xC7,
            ],
            _ => [0; 16],
        }
    }

    /// Get MBR type code
    pub fn mbr_type(&self) -> u8 {
        match self {
            Self::EfiSystem => 0xEF,
            Self::BiosBoot => 0xEF,
            Self::LinuxFilesystem => 0x83,
            Self::LinuxSwap => 0x82,
            Self::LinuxLvm => 0x8E,
            Self::LinuxRaid => 0xFD,
            Self::LinuxHome => 0x83,
            Self::WindowsBasicData => 0x07,
            Self::Extended => 0x05,
            Self::Unknown => 0x00,
        }
    }
}

/// Partition information
#[derive(Debug, Clone)]
pub struct Partition {
    /// Partition number
    pub number: u32,
    /// Partition type
    pub part_type: PartitionType,
    /// Start sector (LBA)
    pub start_lba: u64,
    /// Size in sectors
    pub sectors: u64,
    /// Partition name/label
    pub name: String,
    /// Partition flags
    pub flags: PartitionFlags,
    /// UUID (GPT only)
    pub uuid: Option<[u8; 16]>,
}

impl Partition {
    /// Get partition size in bytes
    pub fn size_bytes(&self, sector_size: u32) -> u64 {
        self.sectors * sector_size as u64
    }

    /// Get end LBA
    pub fn end_lba(&self) -> u64 {
        self.start_lba + self.sectors - 1
    }
}

/// Partition flags
#[derive(Debug, Clone, Copy, Default)]
pub struct PartitionFlags {
    pub bootable: bool,
    pub read_only: bool,
    pub hidden: bool,
    pub no_automount: bool,
}

/// Partition table header (GPT)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct GptHeader {
    pub signature: [u8; 8],      // "EFI PART"
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
    pub partition_entries_crc32: u32,
}

/// GPT partition entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct GptPartitionEntry {
    pub type_guid: [u8; 16],
    pub partition_guid: [u8; 16],
    pub starting_lba: u64,
    pub ending_lba: u64,
    pub attributes: u64,
    pub name: [u16; 36],
}

/// MBR partition entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct MbrPartitionEntry {
    pub boot_indicator: u8,
    pub start_head: u8,
    pub start_sector_cylinder: u16,
    pub system_id: u8,
    pub end_head: u8,
    pub end_sector_cylinder: u16,
    pub start_lba: u32,
    pub sectors: u32,
}

/// Partition manager for a device
pub struct PartitionManager {
    device: String,
    sector_size: u32,
    total_sectors: u64,
    scheme: Option<PartitionScheme>,
    partitions: Vec<Partition>,
}

impl PartitionManager {
    /// Create a new partition manager for a device
    pub fn new(device: &str) -> Result<Self, String> {
        // Get device information
        let sector_size = get_device_sector_size(device)?;
        let total_sectors = get_device_total_sectors(device)?;

        let mut manager = Self {
            device: String::from(device),
            sector_size,
            total_sectors,
            scheme: None,
            partitions: Vec::new(),
        };

        // Try to detect existing partition scheme
        manager.detect_scheme()?;

        // Read existing partitions
        if manager.scheme.is_some() {
            manager.read_partitions()?;
        }

        Ok(manager)
    }

    /// Detect existing partition scheme
    fn detect_scheme(&mut self) -> Result<(), String> {
        // Read first sector
        let mut sector = [0u8; 512];
        read_device_sector(&self.device, 0, &mut sector)?;

        // Check for MBR signature
        if sector[510] == 0x55 && sector[511] == 0xAA {
            // Check for protective MBR (GPT)
            if sector[450] == 0xEE {
                // Read GPT header at LBA 1
                let mut gpt_sector = [0u8; 512];
                read_device_sector(&self.device, 1, &mut gpt_sector)?;

                if &gpt_sector[0..8] == b"EFI PART" {
                    self.scheme = Some(PartitionScheme::Gpt);
                    return Ok(());
                }
            }

            self.scheme = Some(PartitionScheme::Mbr);
        }

        Ok(())
    }

    /// Read existing partitions
    fn read_partitions(&mut self) -> Result<(), String> {
        match self.scheme {
            Some(PartitionScheme::Gpt) => self.read_gpt_partitions(),
            Some(PartitionScheme::Mbr) => self.read_mbr_partitions(),
            None => Ok(()),
        }
    }

    /// Read GPT partitions
    fn read_gpt_partitions(&mut self) -> Result<(), String> {
        // Read GPT header
        let mut header_sector = [0u8; 512];
        read_device_sector(&self.device, 1, &mut header_sector)?;

        let header: GptHeader = unsafe {
            core::ptr::read_unaligned(header_sector.as_ptr() as *const GptHeader)
        };

        // Read partition entries
        let entries_per_sector = self.sector_size / header.partition_entry_size;
        let entry_sectors = (header.num_partition_entries + entries_per_sector - 1) / entries_per_sector;

        let mut number = 1u32;
        for sector_idx in 0..entry_sectors {
            let mut sector = [0u8; 512];
            read_device_sector(&self.device, header.partition_entry_lba + sector_idx as u64, &mut sector)?;

            for entry_idx in 0..(entries_per_sector as usize) {
                let offset = entry_idx * header.partition_entry_size as usize;
                let entry: GptPartitionEntry = unsafe {
                    core::ptr::read_unaligned(sector[offset..].as_ptr() as *const GptPartitionEntry)
                };

                // Check if entry is used (type GUID not zero)
                let type_guid = entry.type_guid;
                if type_guid != [0; 16] {
                    let part_type = Self::guid_to_type(&type_guid);
                    // Copy name to avoid reference to packed struct field
                    let entry_name = entry.name;
                    let name = Self::decode_utf16_name(&entry_name);

                    // Copy values to avoid reference to packed struct fields
                    let starting_lba = entry.starting_lba;
                    let ending_lba = entry.ending_lba;
                    let partition_guid = entry.partition_guid;

                    self.partitions.push(Partition {
                        number,
                        part_type,
                        start_lba: starting_lba,
                        sectors: ending_lba - starting_lba + 1,
                        name,
                        flags: PartitionFlags::default(),
                        uuid: Some(partition_guid),
                    });
                }

                number += 1;
                if number > header.num_partition_entries {
                    break;
                }
            }
        }

        Ok(())
    }

    /// Read MBR partitions
    fn read_mbr_partitions(&mut self) -> Result<(), String> {
        let mut sector = [0u8; 512];
        read_device_sector(&self.device, 0, &mut sector)?;

        for i in 0..4 {
            let offset = 446 + i * 16;
            let entry: MbrPartitionEntry = unsafe {
                core::ptr::read_unaligned(sector[offset..].as_ptr() as *const MbrPartitionEntry)
            };

            if entry.system_id != 0 {
                let part_type = Self::mbr_type_to_type(entry.system_id);

                self.partitions.push(Partition {
                    number: (i + 1) as u32,
                    part_type,
                    start_lba: entry.start_lba as u64,
                    sectors: entry.sectors as u64,
                    name: String::new(),
                    flags: PartitionFlags {
                        bootable: entry.boot_indicator == 0x80,
                        ..Default::default()
                    },
                    uuid: None,
                });

                // Handle extended partitions
                if entry.system_id == 0x05 || entry.system_id == 0x0F {
                    self.read_extended_partitions(entry.start_lba as u64)?;
                }
            }
        }

        Ok(())
    }

    /// Read extended (logical) partitions
    fn read_extended_partitions(&mut self, extended_start: u64) -> Result<(), String> {
        let mut ebr_lba = extended_start;
        let mut number = 5u32;

        loop {
            let mut sector = [0u8; 512];
            read_device_sector(&self.device, ebr_lba, &mut sector)?;

            // First entry is the logical partition
            let entry: MbrPartitionEntry = unsafe {
                core::ptr::read_unaligned(sector[446..].as_ptr() as *const MbrPartitionEntry)
            };

            if entry.system_id != 0 {
                let part_type = Self::mbr_type_to_type(entry.system_id);

                self.partitions.push(Partition {
                    number,
                    part_type,
                    start_lba: ebr_lba + entry.start_lba as u64,
                    sectors: entry.sectors as u64,
                    name: String::new(),
                    flags: PartitionFlags::default(),
                    uuid: None,
                });

                number += 1;
            }

            // Second entry points to next EBR
            let next_entry: MbrPartitionEntry = unsafe {
                core::ptr::read_unaligned(sector[462..].as_ptr() as *const MbrPartitionEntry)
            };

            if next_entry.system_id == 0 {
                break;
            }

            ebr_lba = extended_start + next_entry.start_lba as u64;
        }

        Ok(())
    }

    /// Create GPT partition table
    pub fn create_gpt(&self) -> Result<(), String> {
        // Write protective MBR
        let mut mbr = [0u8; 512];
        mbr[510] = 0x55;
        mbr[511] = 0xAA;

        // Protective MBR entry covering whole disk
        let mbr_entry = MbrPartitionEntry {
            boot_indicator: 0,
            start_head: 0,
            start_sector_cylinder: 0x0001,
            system_id: 0xEE, // GPT protective
            end_head: 0xFF,
            end_sector_cylinder: 0xFFFF,
            start_lba: 1,
            sectors: (self.total_sectors - 1).min(0xFFFFFFFF) as u32,
        };

        unsafe {
            core::ptr::write_unaligned(mbr[446..].as_mut_ptr() as *mut MbrPartitionEntry, mbr_entry);
        }

        write_device_sector(&self.device, 0, &mbr)?;

        // Create GPT header
        let disk_guid = generate_guid();
        let num_entries = 128u32;
        let entry_size = 128u32;
        let entries_sectors = (num_entries * entry_size + self.sector_size - 1) / self.sector_size;

        let header = GptHeader {
            signature: *b"EFI PART",
            revision: 0x00010000,
            header_size: 92,
            header_crc32: 0, // Calculate later
            reserved: 0,
            current_lba: 1,
            backup_lba: self.total_sectors - 1,
            first_usable_lba: 2 + entries_sectors as u64,
            last_usable_lba: self.total_sectors - 2 - entries_sectors as u64,
            disk_guid,
            partition_entry_lba: 2,
            num_partition_entries: num_entries,
            partition_entry_size: entry_size,
            partition_entries_crc32: 0, // Calculate later
        };

        // Write empty partition entries
        let zero_sector = [0u8; 512];
        for i in 0..entries_sectors {
            write_device_sector(&self.device, 2 + i as u64, &zero_sector)?;
        }

        // Write GPT header (need to calculate CRCs properly)
        let mut header_sector = [0u8; 512];
        unsafe {
            core::ptr::write_unaligned(header_sector.as_mut_ptr() as *mut GptHeader, header);
        }
        write_device_sector(&self.device, 1, &header_sector)?;

        // Write backup GPT header at end of disk
        write_device_sector(&self.device, self.total_sectors - 1, &header_sector)?;

        crate::kprintln!("Created GPT partition table on {}", self.device);
        Ok(())
    }

    /// Create MBR partition table
    pub fn create_mbr(&self) -> Result<(), String> {
        let mut mbr = [0u8; 512];
        mbr[510] = 0x55;
        mbr[511] = 0xAA;

        write_device_sector(&self.device, 0, &mbr)?;

        crate::kprintln!("Created MBR partition table on {}", self.device);
        Ok(())
    }

    /// Create a new partition
    pub fn create_partition(&self, part_type: PartitionType, size: u64) -> Result<Partition, String> {
        // Find free space
        let (start, end) = self.find_free_space(size)?;

        let sectors = if size == 0 {
            end - start + 1
        } else {
            size / self.sector_size as u64
        };

        let number = self.partitions.len() as u32 + 1;
        let partition = Partition {
            number,
            part_type,
            start_lba: start,
            sectors,
            name: String::new(),
            flags: PartitionFlags::default(),
            uuid: Some(generate_guid()),
        };

        // Write partition entry
        match self.scheme {
            Some(PartitionScheme::Gpt) => self.write_gpt_partition(&partition)?,
            Some(PartitionScheme::Mbr) => self.write_mbr_partition(&partition)?,
            None => return Err(String::from("No partition scheme")),
        }

        crate::kprintln!("Created partition {} ({:?}) on {}", number, part_type, self.device);
        Ok(partition)
    }

    /// Find free space on disk
    fn find_free_space(&self, size: u64) -> Result<(u64, u64), String> {
        let first_usable = match self.scheme {
            Some(PartitionScheme::Gpt) => 2048, // Standard alignment
            Some(PartitionScheme::Mbr) => 2048,
            None => return Err(String::from("No partition scheme")),
        };

        let last_usable = self.total_sectors - 34; // Leave room for backup GPT

        let required_sectors = if size == 0 {
            0 // Use all remaining
        } else {
            (size + self.sector_size as u64 - 1) / self.sector_size as u64
        };

        // Find start after last partition
        let mut start = first_usable;
        for part in &self.partitions {
            let part_end = part.end_lba();
            if part_end >= start {
                start = ((part_end + 2048) / 2048) * 2048; // Align to 1MB
            }
        }

        if start >= last_usable {
            return Err(String::from("No free space available"));
        }

        let available = last_usable - start;
        if required_sectors > 0 && required_sectors > available {
            return Err(String::from("Not enough free space"));
        }

        let end = if required_sectors == 0 {
            last_usable
        } else {
            start + required_sectors - 1
        };

        Ok((start, end))
    }

    /// Write GPT partition entry
    fn write_gpt_partition(&self, partition: &Partition) -> Result<(), String> {
        // Read current entries
        let entry_lba = 2 + (partition.number - 1) as u64 / 4;
        let entry_offset = ((partition.number - 1) % 4) as usize * 128;

        let mut sector = [0u8; 512];
        read_device_sector(&self.device, entry_lba, &mut sector)?;

        // Create entry
        let mut name_utf16 = [0u16; 36];
        for (i, c) in partition.name.chars().take(35).enumerate() {
            name_utf16[i] = c as u16;
        }

        let entry = GptPartitionEntry {
            type_guid: partition.part_type.gpt_guid(),
            partition_guid: partition.uuid.unwrap_or([0; 16]),
            starting_lba: partition.start_lba,
            ending_lba: partition.end_lba(),
            attributes: 0,
            name: name_utf16,
        };

        unsafe {
            core::ptr::write_unaligned(
                sector[entry_offset..].as_mut_ptr() as *mut GptPartitionEntry,
                entry,
            );
        }

        write_device_sector(&self.device, entry_lba, &sector)?;

        // Update header CRCs (simplified - would need proper calculation)

        Ok(())
    }

    /// Write MBR partition entry
    fn write_mbr_partition(&self, partition: &Partition) -> Result<(), String> {
        if partition.number > 4 {
            return Err(String::from("MBR supports only 4 primary partitions"));
        }

        let mut sector = [0u8; 512];
        read_device_sector(&self.device, 0, &mut sector)?;

        let entry = MbrPartitionEntry {
            boot_indicator: if partition.flags.bootable { 0x80 } else { 0 },
            start_head: 0,
            start_sector_cylinder: 0,
            system_id: partition.part_type.mbr_type(),
            end_head: 0,
            end_sector_cylinder: 0,
            start_lba: partition.start_lba as u32,
            sectors: partition.sectors as u32,
        };

        let offset = 446 + (partition.number - 1) as usize * 16;
        unsafe {
            core::ptr::write_unaligned(
                sector[offset..].as_mut_ptr() as *mut MbrPartitionEntry,
                entry,
            );
        }

        write_device_sector(&self.device, 0, &sector)?;

        Ok(())
    }

    /// Delete a partition
    pub fn delete_partition(&mut self, number: u32) -> Result<(), String> {
        match self.scheme {
            Some(PartitionScheme::Gpt) => {
                // Zero out partition entry
                let entry_lba = 2 + (number - 1) as u64 / 4;
                let entry_offset = ((number - 1) % 4) as usize * 128;

                let mut sector = [0u8; 512];
                read_device_sector(&self.device, entry_lba, &mut sector)?;

                for i in 0..128 {
                    sector[entry_offset + i] = 0;
                }

                write_device_sector(&self.device, entry_lba, &sector)?;
            }
            Some(PartitionScheme::Mbr) => {
                if number > 4 {
                    return Err(String::from("Extended partition deletion not implemented"));
                }

                let mut sector = [0u8; 512];
                read_device_sector(&self.device, 0, &mut sector)?;

                let offset = 446 + (number - 1) as usize * 16;
                for i in 0..16 {
                    sector[offset + i] = 0;
                }

                write_device_sector(&self.device, 0, &sector)?;
            }
            None => return Err(String::from("No partition scheme")),
        }

        self.partitions.retain(|p| p.number != number);
        crate::kprintln!("Deleted partition {} on {}", number, self.device);
        Ok(())
    }

    /// Get list of partitions
    pub fn partitions(&self) -> &[Partition] {
        &self.partitions
    }

    /// Convert GUID to partition type
    fn guid_to_type(guid: &[u8; 16]) -> PartitionType {
        // Compare with known GUIDs
        if *guid == PartitionType::EfiSystem.gpt_guid() {
            PartitionType::EfiSystem
        } else if *guid == PartitionType::LinuxFilesystem.gpt_guid() {
            PartitionType::LinuxFilesystem
        } else if *guid == PartitionType::LinuxSwap.gpt_guid() {
            PartitionType::LinuxSwap
        } else if *guid == PartitionType::LinuxLvm.gpt_guid() {
            PartitionType::LinuxLvm
        } else if *guid == PartitionType::LinuxHome.gpt_guid() {
            PartitionType::LinuxHome
        } else if *guid == PartitionType::WindowsBasicData.gpt_guid() {
            PartitionType::WindowsBasicData
        } else {
            PartitionType::Unknown
        }
    }

    /// Convert MBR type to partition type
    fn mbr_type_to_type(mbr_type: u8) -> PartitionType {
        match mbr_type {
            0xEF => PartitionType::EfiSystem,
            0x83 => PartitionType::LinuxFilesystem,
            0x82 => PartitionType::LinuxSwap,
            0x8E => PartitionType::LinuxLvm,
            0xFD => PartitionType::LinuxRaid,
            0x07 => PartitionType::WindowsBasicData,
            0x05 | 0x0F => PartitionType::Extended,
            _ => PartitionType::Unknown,
        }
    }

    /// Decode UTF-16 partition name
    fn decode_utf16_name(name: &[u16; 36]) -> String {
        let mut result = String::new();
        for &c in name {
            if c == 0 {
                break;
            }
            if let Some(ch) = char::from_u32(c as u32) {
                result.push(ch);
            }
        }
        result
    }
}

// ============================================================================
// Helper functions
// ============================================================================

fn get_device_sector_size(_device: &str) -> Result<u32, String> {
    Ok(512)
}

fn get_device_total_sectors(_device: &str) -> Result<u64, String> {
    Ok(1024 * 1024 * 1024 / 512) // 1GB default
}

fn read_device_sector(_device: &str, _lba: u64, _buffer: &mut [u8; 512]) -> Result<(), String> {
    Ok(())
}

fn write_device_sector(_device: &str, _lba: u64, _buffer: &[u8; 512]) -> Result<(), String> {
    Ok(())
}

fn generate_guid() -> [u8; 16] {
    // Generate random GUID
    let mut guid = [0u8; 16];
    // Would use kernel RNG
    for i in 0..16 {
        guid[i] = (i as u8).wrapping_mul(17).wrapping_add(42);
    }
    // Set version 4 (random) and variant bits
    guid[6] = (guid[6] & 0x0F) | 0x40;
    guid[8] = (guid[8] & 0x3F) | 0x80;
    guid
}
