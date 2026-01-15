    #![allow(dead_code)]

    use alloc::collections::BTreeMap;
    use alloc::collections::VecDeque;
    use alloc::vec;
    use alloc::vec::Vec;

    use super::block::{BlockDevice, BlockDeviceId};
    use crate::sync::IrqSafeMutex;
    use crate::util::{KError, KResult};

    /// Cache simples de blocos (LRU). Não é a versão final de alta performance,
    /// mas já define o desenho e evita I/O redundante.
    ///
    /// - key: (dev_id, lba)
    /// - value: bloco completo (block_size)
    pub struct BlockCache {
        capacity: usize,
        inner: IrqSafeMutex<Inner>,
    }

    struct Inner {
        map: BTreeMap<(BlockDeviceId, u64), Vec<u8>>,
        lru: VecDeque<(BlockDeviceId, u64)>,
    }

    impl BlockCache {
        pub fn new(capacity: usize) -> Self {
            Self {
                capacity,
                inner: IrqSafeMutex::new(Inner {
                    map: BTreeMap::new(),
                    lru: VecDeque::new(),
                }),
            }
        }

        pub fn read_block(&self, dev: &dyn BlockDevice, lba: u64) -> KResult<Vec<u8>> {
            let key = (dev.id(), lba);

            {
                let mut g = self.inner.lock();
                if g.map.contains_key(&key) {
                    // atualiza LRU e retorna cópia
                    touch_lru(&mut g.lru, key);
                    return Ok(g.map.get(&key).unwrap().clone());
                }
            }

            // Miss: lê do device
            let mut buf = vec![0u8; dev.block_size() as usize];
            dev.read_blocks(lba, 1, &mut buf)?;

            let mut g = self.inner.lock();
            if g.map.len() >= self.capacity {
                // Evict LRU
                if let Some(old) = g.lru.pop_front() {
                    g.map.remove(&old);
                }
            }
            g.map.insert(key, buf.clone());
            g.lru.push_back(key);

            Ok(buf)
        }

        pub fn write_block(&self, dev: &dyn BlockDevice, lba: u64, data: &[u8]) -> KResult<()> {
            if data.len() != dev.block_size() as usize {
                return Err(KError::Invalid);
            }

            dev.write_blocks(lba, 1, data)?;

            let key = (dev.id(), lba);
            let mut g = self.inner.lock();

            if g.map.len() >= self.capacity && !g.map.contains_key(&key) {
                if let Some(old) = g.lru.pop_front() {
                    g.map.remove(&old);
                }
            }

            g.map.insert(key, data.to_vec());
            touch_lru(&mut g.lru, key);
            Ok(())
        }
    }

    fn touch_lru(lru: &mut VecDeque<(BlockDeviceId, u64)>, key: (BlockDeviceId, u64)) {
        if let Some(pos) = lru.iter().position(|k| *k == key) {
            lru.remove(pos);
        }
        lru.push_back(key);
    }
