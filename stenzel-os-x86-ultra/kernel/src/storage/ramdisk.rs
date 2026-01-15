#![allow(dead_code)]

use alloc::vec::Vec;

use super::block::{check_io_args, BlockDevice, BlockDeviceId};
use crate::sync::IrqSafeMutex;
use crate::util::{KError, KResult};

/// Dispositivo de bloco em RAM (ótimo para testes do stack de FS).
///
/// Implementação *thread-safe* via lock interno.
pub struct RamDisk {
    id: BlockDeviceId,
    block_size: u32,
    data: IrqSafeMutex<Vec<u8>>,
}

impl RamDisk {
    pub fn new(id: u32, block_size: u32, num_blocks: u64) -> Self {
        let total = (block_size as u64 * num_blocks) as usize;
        let mut v = Vec::with_capacity(total);
        v.resize(total, 0);
        Self {
            id: BlockDeviceId(id),
            block_size,
            data: IrqSafeMutex::new(v),
        }
    }

    fn range(&self, lba: u64, count: u32) -> KResult<(usize, usize)> {
        let start = (lba as u128) * (self.block_size as u128);
        let len = (count as u128) * (self.block_size as u128);
        let end = start + len;

        let g = self.data.lock();
        if end > g.len() as u128 {
            return Err(KError::Invalid);
        }
        Ok((start as usize, end as usize))
    }
}

impl BlockDevice for RamDisk {
    fn id(&self) -> BlockDeviceId {
        self.id
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn num_blocks(&self) -> u64 {
        let g = self.data.lock();
        (g.len() as u64) / (self.block_size as u64)
    }

    fn read_blocks(&self, lba: u64, count: u32, out: &mut [u8]) -> KResult<()> {
        check_io_args(self.block_size, count, out.len())?;
        let (s, e) = self.range(lba, count)?;
        let g = self.data.lock();
        out.copy_from_slice(&g[s..e]);
        Ok(())
    }

    fn write_blocks(&self, lba: u64, count: u32, data: &[u8]) -> KResult<()> {
        check_io_args(self.block_size, count, data.len())?;
        let (s, e) = self.range(lba, count)?;
        let mut g = self.data.lock();
        g[s..e].copy_from_slice(data);
        Ok(())
    }
}
