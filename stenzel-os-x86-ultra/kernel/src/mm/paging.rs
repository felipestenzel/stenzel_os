    #![allow(dead_code)]

    use x86_64::registers::control::Cr3;
    use x86_64::structures::paging::{
        FrameAllocator, Mapper as X86Mapper, OffsetPageTable, Page, PageTable, PageTableFlags,
        PhysFrame, Size4KiB, Size2MiB, Size1GiB, Translate,
    };
    use x86_64::{PhysAddr, VirtAddr};

    use crate::util::{KError, KResult};

    pub type OffsetMapper = OffsetPageTable<'static>;

    pub struct KernelMapper {
        pub(crate) inner: OffsetMapper,
    }

    impl KernelMapper {
        pub fn map_page(
            &mut self,
            page: Page<Size4KiB>,
            frame: PhysFrame<Size4KiB>,
            flags: PageTableFlags,
            frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        ) -> KResult<()> {
            unsafe {
                self.inner
                    .map_to(page, frame, flags, frame_allocator)
                    .map_err(|_| KError::NoMemory)?
                    .flush();
            }
            Ok(())
        }

        pub fn unmap_page(&mut self, page: Page<Size4KiB>) -> KResult<(PhysFrame<Size4KiB>, impl FnOnce())> {
            let (frame, flush) = self.inner.unmap(page).map_err(|_| KError::Invalid)?;
            Ok((frame, move || flush.flush()))
        }

        /// Remove um mapeamento de página sem retornar o frame
        pub fn unmap_page_simple(&mut self, page: Page<Size4KiB>) -> KResult<()> {
            let (_frame, flush) = self.inner.unmap(page).map_err(|_| KError::Invalid)?;
            flush.flush();
            Ok(())
        }

        pub fn translate_addr(&self, addr: VirtAddr) -> Option<PhysAddr> {
            self.inner.translate_addr(addr)
        }

        /// Atualiza as flags de uma página já mapeada
        pub fn update_page_flags(&mut self, page: Page<Size4KiB>, new_flags: PageTableFlags) {
            use x86_64::structures::paging::mapper::{TranslateResult, MappedFrame};

            // Usa translate para obter o frame atual
            match self.inner.translate(page.start_address()) {
                TranslateResult::Mapped { frame, offset: _, flags: _ } => {
                    // Extrai o PhysFrame do MappedFrame (só suportamos 4KiB)
                    let phys_frame: PhysFrame<Size4KiB> = match frame {
                        MappedFrame::Size4KiB(f) => f,
                        _ => return, // Páginas grandes não suportadas aqui
                    };

                    // Remapeia com as novas flags
                    if let Ok((_, flush)) = self.inner.unmap(page) {
                        flush.flush();
                        // Re-mapeia com as novas flags
                        // Precisamos de um frame allocator dummy para isso
                        struct DummyAllocator;
                        unsafe impl FrameAllocator<Size4KiB> for DummyAllocator {
                            fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
                                None // Não aloca novos frames
                            }
                        }
                        let mut dummy = DummyAllocator;
                        unsafe {
                            let _ = self.inner.map_to(page, phys_frame, new_flags, &mut dummy);
                        }
                        // Flush TLB
                        x86_64::instructions::tlb::flush(page.start_address());
                    }
                }
                _ => {
                    // Página não mapeada - ignora
                }
            }
        }

        // ==================================================================
        // Huge Pages Support (2MB and 1GB pages)
        // ==================================================================

        /// Map a 2MB huge page.
        pub fn map_huge_2mb(
            &mut self,
            page: Page<Size2MiB>,
            frame: PhysFrame<Size2MiB>,
            flags: PageTableFlags,
            frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        ) -> KResult<()> {
            // Set the HUGE_PAGE flag for 2MB pages
            let huge_flags = flags | PageTableFlags::HUGE_PAGE;
            unsafe {
                self.inner
                    .map_to(page, frame, huge_flags, frame_allocator)
                    .map_err(|_| KError::NoMemory)?
                    .flush();
            }
            Ok(())
        }

        /// Unmap a 2MB huge page.
        pub fn unmap_huge_2mb(&mut self, page: Page<Size2MiB>) -> KResult<(PhysFrame<Size2MiB>, impl FnOnce())> {
            let (frame, flush) = self.inner.unmap(page).map_err(|_| KError::Invalid)?;
            Ok((frame, move || flush.flush()))
        }

        /// Map a 1GB huge page.
        pub fn map_huge_1gb(
            &mut self,
            page: Page<Size1GiB>,
            frame: PhysFrame<Size1GiB>,
            flags: PageTableFlags,
            frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        ) -> KResult<()> {
            // Set the HUGE_PAGE flag for 1GB pages
            let huge_flags = flags | PageTableFlags::HUGE_PAGE;
            unsafe {
                self.inner
                    .map_to(page, frame, huge_flags, frame_allocator)
                    .map_err(|_| KError::NoMemory)?
                    .flush();
            }
            Ok(())
        }

        /// Unmap a 1GB huge page.
        pub fn unmap_huge_1gb(&mut self, page: Page<Size1GiB>) -> KResult<(PhysFrame<Size1GiB>, impl FnOnce())> {
            let (frame, flush) = self.inner.unmap(page).map_err(|_| KError::Invalid)?;
            Ok((frame, move || flush.flush()))
        }

        /// Mapeia uma faixa de memória virtual [start, start+size) em frames recém-alocados.
        pub fn map_range(
            &mut self,
            start: VirtAddr,
            size: usize,
            flags: PageTableFlags,
            frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        ) -> KResult<()> {
            let start_page: Page<Size4KiB> = Page::containing_address(start);
            let end_addr = start + size as u64 - 1;
            let end_page: Page<Size4KiB> = Page::containing_address(end_addr);

            for page in Page::range_inclusive(start_page, end_page) {
                let frame = frame_allocator.allocate_frame().ok_or(KError::NoMemory)?;
                unsafe {
                    self.inner
                        .map_to(page, frame, flags, frame_allocator)
                        .map_err(|_| KError::NoMemory)?
                        .flush();
                }
            }
            Ok(())
        }
    }

    /// Inicializa o OffsetPageTable a partir do CR3 e do offset de memória física mapeada.
    ///
    /// # Safety
    /// Requer que `physical_memory_offset` esteja correto e que a page table ativa seja válida.
    pub unsafe fn init_offset_page_table(physical_memory_offset: VirtAddr) -> OffsetMapper {
        let (level_4_table_frame, _) = Cr3::read();
        let phys = level_4_table_frame.start_address();
        let virt = physical_memory_offset + phys.as_u64();
        let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
        let level_4_table = &mut *page_table_ptr;
        OffsetPageTable::new(level_4_table, physical_memory_offset)
    }

    /// Helpers de flags comuns.
    pub fn flags_kernel_rw() -> PageTableFlags {
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE
    }

    pub fn flags_kernel_ro() -> PageTableFlags {
        PageTableFlags::PRESENT
    }

    pub fn flags_user_rw() -> PageTableFlags {
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE
    }

    pub fn flags_mmio() -> PageTableFlags {
        // MMIO: não-cacheável seria ideal (PAT/MTRR). Aqui marcamos apenas PRESENT|WRITABLE.
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE
    }

    /// Flags for kernel huge pages (2MB/1GB).
    pub fn flags_kernel_huge_rw() -> PageTableFlags {
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::HUGE_PAGE
    }

    /// Flags for user-accessible huge pages (2MB/1GB).
    pub fn flags_user_huge_rw() -> PageTableFlags {
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::HUGE_PAGE
    }

    /// Traduz um endereço virtual usando a page table atual (CR3).
    /// Usado pelo page fault handler para traduzir endereços de user space.
    pub fn translate_current_cr3(va: VirtAddr, phys_offset: VirtAddr) -> Option<PhysAddr> {
        use x86_64::structures::paging::mapper::{TranslateResult, MappedFrame};

        let (level_4_table_frame, _) = Cr3::read();
        let phys = level_4_table_frame.start_address();
        let virt = phys_offset + phys.as_u64();
        let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

        // SAFETY: Assumimos que o CR3 aponta para uma page table válida
        let mapper = unsafe {
            let level_4_table = &mut *page_table_ptr;
            OffsetPageTable::new(level_4_table, phys_offset)
        };

        match mapper.translate(va) {
            TranslateResult::Mapped { frame, offset, flags: _ } => {
                let frame_addr = match frame {
                    MappedFrame::Size4KiB(f) => f.start_address(),
                    MappedFrame::Size2MiB(f) => f.start_address(),
                    MappedFrame::Size1GiB(f) => f.start_address(),
                };
                Some(frame_addr + offset)
            }
            _ => None,
        }
    }
