    #![allow(dead_code)]

    use alloc::vec::Vec;

    use super::block::BlockDevice;
    use crate::util::{KError, KResult};

    #[derive(Debug, Clone)]
    pub struct GptPartition {
        pub first_lba: u64,
        pub last_lba: u64,
        pub name_utf16: [u16; 36],
        pub type_guid: [u8; 16],
        pub unique_guid: [u8; 16],
        pub attributes: u64,
    }

    #[derive(Debug, Clone)]
    pub struct Gpt {
        pub header_lba: u64,
        pub partitions: Vec<GptPartition>,
    }

    #[repr(C, packed)]
    struct RawGptHeader {
        signature: [u8; 8], // "EFI PART"
        revision: u32,
        header_size: u32,
        header_crc32: u32,
        reserved: u32,
        current_lba: u64,
        backup_lba: u64,
        first_usable_lba: u64,
        last_usable_lba: u64,
        disk_guid: [u8; 16],
        partition_entries_lba: u64,
        num_partition_entries: u32,
        size_of_partition_entry: u32,
        partition_entries_crc32: u32,
        // rest ignored
    }

    #[repr(C, packed)]
    struct RawGptEntry {
        type_guid: [u8; 16],
        unique_guid: [u8; 16],
        first_lba: u64,
        last_lba: u64,
        attributes: u64,
        name_utf16: [u16; 36],
    }

    /// Lê e retorna as partições GPT do dispositivo.
    pub fn read_gpt_partitions(dev: &dyn BlockDevice) -> KResult<Vec<GptPartition>> {
        let gpt = read_gpt(dev)?;
        Ok(gpt.partitions)
    }

    pub fn read_gpt(dev: &dyn BlockDevice) -> KResult<Gpt> {
        let bs = dev.block_size();
        if bs < 512 {
            return Err(KError::Invalid);
        }

        // GPT header fica em LBA1
        let mut buf = alloc::vec![0u8; bs as usize];
        dev.read_blocks(1, 1, &mut buf)?;

        if buf.len() < core::mem::size_of::<RawGptHeader>() {
            return Err(KError::Invalid);
        }

        let hdr = unsafe { &*(buf.as_ptr() as *const RawGptHeader) };
        if &hdr.signature != b"EFI PART" {
            return Err(KError::NotFound);
        }

        // Por enquanto não validamos CRC (exige crc32).
        let entry_size = hdr.size_of_partition_entry as usize;
        let num_entries = hdr.num_partition_entries as usize;
        if entry_size < core::mem::size_of::<RawGptEntry>() {
            return Err(KError::Invalid);
        }

        // Leitura dos entries: pode ocupar múltiplos LBAs.
        let entries_bytes = num_entries
            .checked_mul(entry_size)
            .ok_or(KError::Invalid)?;

        let blocks_needed = ((entries_bytes + (bs as usize) - 1) / (bs as usize)) as u32;

        let mut entries_buf = alloc::vec![0u8; (blocks_needed as usize) * (bs as usize)];
        dev.read_blocks(hdr.partition_entries_lba, blocks_needed, &mut entries_buf)?;

        let mut partitions = Vec::new();
        for i in 0..num_entries {
            let off = i * entry_size;
            let end = off + core::mem::size_of::<RawGptEntry>();
            if end > entries_buf.len() {
                break;
            }
            let ent = unsafe { &*(entries_buf[off..].as_ptr() as *const RawGptEntry) };
            if ent.type_guid == [0u8; 16] {
                continue; // entry vazio
            }
            partitions.push(GptPartition {
                first_lba: ent.first_lba,
                last_lba: ent.last_lba,
                name_utf16: ent.name_utf16,
                type_guid: ent.type_guid,
                unique_guid: ent.unique_guid,
                attributes: ent.attributes,
            });
        }

        Ok(Gpt {
            header_lba: hdr.current_lba,
            partitions,
        })
    }
