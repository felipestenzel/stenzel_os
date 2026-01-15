    use alloc::vec;
    use alloc::vec::Vec;
    use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
    use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};
    use x86_64::PhysAddr;

    /// Alocador físico early: percorre o mapa de memória e entrega frames `Usable` sequencialmente.
    /// É simples e determinístico, bom para bootstrap.
    pub struct BootInfoFrameAllocator {
        memory_regions: &'static MemoryRegions,
        next: usize,
    }

    impl BootInfoFrameAllocator {
        /// # Safety
        /// O chamador deve garantir que o mapa de memória fornecido é válido e que
        /// frames `Usable` não serão alocados duas vezes de forma concorrente.
        pub unsafe fn new(memory_regions: &'static MemoryRegions) -> Self {
            Self {
                memory_regions,
                next: 0,
            }
        }

        pub fn allocated_count(&self) -> usize {
            self.next
        }

        fn usable_frames(&self) -> impl Iterator<Item = PhysFrame<Size4KiB>> {
            let regions = self.memory_regions.iter();
            let usable = regions.filter(|r| r.kind == MemoryRegionKind::Usable);
            let addr_ranges = usable.flat_map(|r| {
                let start = r.start;
                let end = r.end;
                (start..end).step_by(4096)
            });
            addr_ranges.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
        }
    }

    unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
        fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
            let frame = self.usable_frames().nth(self.next);
            self.next += 1;
            frame
        }
    }

    /// Alocador físico definitivo: bitmap de frames 4KiB.
    ///
    /// - bit=1 => usado
    /// - bit=0 => livre
    pub struct BitmapFrameAllocator {
        bits: Vec<u64>,
        total_frames: usize,
        free_frames: usize,
        cursor_word: usize,
    }

    impl BitmapFrameAllocator {
        pub fn from_memory_regions(
            memory_regions: &'static MemoryRegions,
            early: &BootInfoFrameAllocator,
        ) -> Self {
            let max_end = memory_regions
                .iter()
                .map(|r| r.end)
                .max()
                .unwrap_or(0);

            let total_frames = ((max_end + 4095) / 4096) as usize;
            let words = (total_frames + 63) / 64;

            // Começa tudo como "usado", e libera apenas regiões Usable.
            let mut bits = vec![u64::MAX; words];

            // Marca frames em regiões Usable como livres.
            for region in memory_regions.iter() {
                if region.kind != MemoryRegionKind::Usable {
                    continue;
                }
                let start_frame = (region.start / 4096) as usize;
                let end_frame = ((region.end + 4095) / 4096) as usize;
                for f in start_frame..end_frame {
                    clear_bit(&mut bits, f);
                }
            }

            // Marca como usados os frames já consumidos pelo alocador early.
            // Como o early aloca o "n-ésimo frame Usable", nós repetimos a iteração.
            let mut tmp_early = BootInfoFrameAllocator {
                memory_regions,
                next: 0,
            };
            for _ in 0..early.allocated_count() {
                if let Some(frame) = FrameAllocator::<Size4KiB>::allocate_frame(&mut tmp_early) {
                    let idx = (frame.start_address().as_u64() / 4096) as usize;
                    set_bit(&mut bits, idx);
                }
            }

            // Conta livres.
            let mut free_frames = 0usize;
            for i in 0..total_frames {
                if !get_bit(&bits, i) {
                    free_frames += 1;
                }
            }

            Self {
                bits,
                total_frames,
                free_frames,
                cursor_word: 0,
            }
        }

        pub fn total_frames(&self) -> usize {
            self.total_frames
        }

        pub fn free_frames(&self) -> usize {
            self.free_frames
        }

        pub fn used_frames(&self) -> usize {
            self.total_frames - self.free_frames
        }

        fn find_free(&mut self) -> Option<usize> {
            let n = self.bits.len();
            for step in 0..n {
                let wi = (self.cursor_word + step) % n;
                let word = self.bits[wi];
                if word == u64::MAX {
                    continue; // tudo usado
                }
                // Temos algum bit 0 (livre). Encontra o primeiro.
                let inv = !word;
                let bit = inv.trailing_zeros() as usize;
                let idx = wi * 64 + bit;
                if idx < self.total_frames {
                    self.cursor_word = wi;
                    return Some(idx);
                }
            }
            None
        }

        pub fn allocate(&mut self) -> Option<PhysFrame<Size4KiB>> {
            let idx = self.find_free()?;
            set_bit(&mut self.bits, idx);
            self.free_frames = self.free_frames.saturating_sub(1);
            Some(PhysFrame::containing_address(PhysAddr::new((idx as u64) * 4096)))
        }

        /// Aloca `n` frames 4KiB contíguos.
        ///
        /// Isso é necessário para estruturas que o hardware espera como uma região
        /// linear (ex.: virtqueue legacy do virtio).
        pub fn allocate_contiguous(&mut self, n: usize) -> Option<PhysFrame<Size4KiB>> {
            if n == 0 {
                return None;
            }

            let total = self.total_frames;
            // Varre a bitmap procurando uma janela de `n` bits livres.
            // Estratégia simples: first-fit a partir do cursor.
            let mut start = self.cursor_word * 64;
            if start >= total {
                start = 0;
            }

            let mut i = start;
            let mut scanned = 0usize;
            while scanned < total {
                // encontra primeiro bit livre
                while i < total && get_bit(&self.bits, i) {
                    i += 1;
                    scanned += 1;
                    if scanned >= total {
                        return None;
                    }
                }
                if i + n > total {
                    return None;
                }

                // testa janela [i, i+n)
                let mut ok = true;
                for j in 0..n {
                    if get_bit(&self.bits, i + j) {
                        ok = false;
                        i = i + j + 1;
                        scanned += j + 1;
                        break;
                    }
                }
                if !ok {
                    continue;
                }

                // marca usados
                for j in 0..n {
                    set_bit(&mut self.bits, i + j);
                }
                self.free_frames = self.free_frames.saturating_sub(n);
                self.cursor_word = (i / 64) % self.bits.len();
                let pa = PhysAddr::new((i as u64) * 4096);
                return Some(PhysFrame::containing_address(pa));
            }
            None
        }

        pub fn deallocate(&mut self, frame: PhysFrame<Size4KiB>) {
            let idx = (frame.start_address().as_u64() / 4096) as usize;
            if idx >= self.total_frames {
                return;
            }
            if get_bit(&self.bits, idx) {
                clear_bit(&mut self.bits, idx);
                self.free_frames += 1;
            }
        }
    }

    unsafe impl FrameAllocator<Size4KiB> for BitmapFrameAllocator {
        fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
            self.allocate()
        }
    }

    #[inline]
    fn word_bit(i: usize) -> (usize, u64) {
        let word = i / 64;
        let bit = (i % 64) as u64;
        (word, 1u64 << bit)
    }

    #[inline]
    fn get_bit(bits: &[u64], i: usize) -> bool {
        let (w, m) = word_bit(i);
        (bits[w] & m) != 0
    }

    #[inline]
    fn set_bit(bits: &mut [u64], i: usize) {
        let (w, m) = word_bit(i);
        bits[w] |= m;
    }

    #[inline]
    fn clear_bit(bits: &mut [u64], i: usize) {
        let (w, m) = word_bit(i);
        bits[w] &= !m;
    }
