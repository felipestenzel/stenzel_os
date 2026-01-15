    use alloc::sync::Arc;
    use crate::util::{KError, KResult};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub struct BlockDeviceId(pub u32);

    /// Interface genérica de dispositivo de bloco (SSD/HDD/virtio/ramdisk).
    pub trait BlockDevice: Send + Sync {
        fn id(&self) -> BlockDeviceId;

        /// Tamanho de bloco (normalmente 512 ou 4096).
        fn block_size(&self) -> u32;

        /// Número total de blocos.
        fn num_blocks(&self) -> u64;

        /// Lê `count` blocos a partir de `lba` para `out`.
        /// `out.len()` deve ser `count * block_size`.
        fn read_blocks(&self, lba: u64, count: u32, out: &mut [u8]) -> KResult<()>;

        /// Escreve `count` blocos a partir de `lba` de `data`.
        fn write_blocks(&self, lba: u64, count: u32, data: &[u8]) -> KResult<()>;
    }

    pub fn check_io_args(block_size: u32, count: u32, buf_len: usize) -> KResult<()> {
        let expected = (block_size as usize)
            .checked_mul(count as usize)
            .ok_or(KError::Invalid)?;
        if expected != buf_len {
            return Err(KError::Invalid);
        }
        Ok(())
    }

    /// Wrapper que representa uma partição dentro de um BlockDevice.
    /// Traduz LBAs relativos à partição para LBAs absolutos no disco.
    pub struct PartitionBlockDevice {
        inner: Arc<dyn BlockDevice>,
        start_lba: u64,
        num_blocks: u64,
        id: BlockDeviceId,
    }

    impl PartitionBlockDevice {
        pub fn new(inner: Arc<dyn BlockDevice>, start_lba: u64, end_lba: u64, id: BlockDeviceId) -> Self {
            let num_blocks = end_lba.saturating_sub(start_lba) + 1;
            Self {
                inner,
                start_lba,
                num_blocks,
                id,
            }
        }
    }

    impl BlockDevice for PartitionBlockDevice {
        fn id(&self) -> BlockDeviceId {
            self.id
        }

        fn block_size(&self) -> u32 {
            self.inner.block_size()
        }

        fn num_blocks(&self) -> u64 {
            self.num_blocks
        }

        fn read_blocks(&self, lba: u64, count: u32, out: &mut [u8]) -> KResult<()> {
            // Verifica limites
            if lba >= self.num_blocks || lba + count as u64 > self.num_blocks {
                return Err(KError::OutOfRange);
            }
            // Traduz LBA
            let abs_lba = self.start_lba + lba;
            self.inner.read_blocks(abs_lba, count, out)
        }

        fn write_blocks(&self, lba: u64, count: u32, data: &[u8]) -> KResult<()> {
            // Verifica limites
            if lba >= self.num_blocks || lba + count as u64 > self.num_blocks {
                return Err(KError::OutOfRange);
            }
            // Traduz LBA
            let abs_lba = self.start_lba + lba;
            self.inner.write_blocks(abs_lba, count, data)
        }
    }
