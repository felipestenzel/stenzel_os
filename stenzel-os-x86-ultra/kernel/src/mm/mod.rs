    #![allow(dead_code)]

    use bootloader_api::BootInfo;
    use spin::Once;
    use x86_64::VirtAddr;

    use crate::sync::{IrqSafeGuard, IrqSafeMutex};

    mod heap;
    mod paging;
    mod phys;
    pub mod vma;

    pub use phys::{BitmapFrameAllocator, BootInfoFrameAllocator};

    static MAPPER: Once<IrqSafeMutex<paging::KernelMapper>> = Once::new();
    static FRAME_ALLOC: Once<IrqSafeMutex<BitmapFrameAllocator>> = Once::new();
    static PHYS_OFFSET: Once<VirtAddr> = Once::new();

    pub fn init(boot_info: &'static mut BootInfo) {
        let phys_offset = boot_info
            .physical_memory_offset
            .into_option()
            .map(VirtAddr::new)
            .expect("bootloader não forneceu physical_memory_offset (config.mappings.physical_memory)");

        PHYS_OFFSET.call_once(|| phys_offset);

        // Mapper inicial (page table ativa)
        let mut mapper = unsafe { paging::init_offset_page_table(phys_offset) };

        // Alocador físico early (linear) para mapear heap e estruturas iniciais.
        let mut early = unsafe { BootInfoFrameAllocator::new(&boot_info.memory_regions) };

        // Mapeia heap e inicializa allocator de heap.
        heap::init_heap(&mut mapper, &mut early).expect("falha ao inicializar heap");

        // Agora que temos heap, construímos o alocador físico definitivo (bitmap).
        let bitmap = BitmapFrameAllocator::from_memory_regions(&boot_info.memory_regions, &early);

        // Publica globais.
        MAPPER.call_once(|| IrqSafeMutex::new(paging::KernelMapper { inner: mapper }));
        FRAME_ALLOC.call_once(|| IrqSafeMutex::new(bitmap));

        let stats = frame_allocator_stats();
        crate::kprintln!(
            "mm: heap ok; frames total={} free={} used={}",
            stats.total, stats.free, stats.used
        );
    }

    /// Offset virtual onde a memória física está mapeada (physical_memory_offset do bootloader).
    pub fn physical_memory_offset() -> VirtAddr {
        *PHYS_OFFSET.get().expect("PHYS_OFFSET não inicializado")
    }

    /// Converte endereço físico -> virtual usando o mapeamento de offset.
    #[inline]
    pub fn phys_to_virt(pa: x86_64::PhysAddr) -> VirtAddr {
        physical_memory_offset() + pa.as_u64()
    }

    /// Converte virtual -> físico consultando o mapper ativo.
    pub fn virt_to_phys(va: VirtAddr) -> Option<x86_64::PhysAddr> {
        let m = mapper_lock();
        m.translate_addr(va)
    }

    /// Aloca `pages` frames 4KiB *contíguos fisicamente* e retorna:
    /// - o primeiro frame físico
    /// - o endereço virtual (via offset) para acesso no kernel
    pub fn alloc_contiguous_pages(pages: usize) -> Option<(x86_64::structures::paging::PhysFrame<x86_64::structures::paging::Size4KiB>, VirtAddr)> {
        let mut fa = frame_allocator_lock();
        let first = fa.allocate_contiguous(pages)?;
        let virt = phys_to_virt(first.start_address());
        Some((first, virt))
    }

    pub fn mapper_lock() -> IrqSafeGuard<'static, paging::KernelMapper> {
        let m = MAPPER.call_once(|| panic!("mapper não inicializado"));
        m.lock()
    }

    pub fn frame_allocator_lock() -> IrqSafeGuard<'static, BitmapFrameAllocator> {
        let fa = FRAME_ALLOC.call_once(|| panic!("frame allocator não inicializado"));
        fa.lock()
    }

    pub fn frame_allocator_stats() -> FrameAllocatorStatsView {
        let fa = FRAME_ALLOC.call_once(|| panic!("frame allocator não inicializado"));
        let g = fa.lock();
        FrameAllocatorStatsView {
            total: g.total_frames(),
            free: g.free_frames(),
            used: g.used_frames(),
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct FrameAllocatorStatsView {
        pub total: usize,
        pub free: usize,
        pub used: usize,
    }

    /// Aloca um único frame 4KiB.
    pub fn alloc_frame() -> Option<x86_64::structures::paging::PhysFrame<x86_64::structures::paging::Size4KiB>> {
        frame_allocator_lock().allocate()
    }

    /// Mapeia uma região MMIO para acesso do kernel.
    /// Como usamos physical_memory_offset do bootloader, a região já está acessível
    /// desde que esteja dentro do espaço mapeado.
    pub fn map_mmio(phys_base: u64, _size: u64) -> Result<(), crate::util::KError> {
        // Com o mapeamento de physical_memory_offset do bootloader,
        // toda a memória física está mapeada. Apenas verificamos se
        // o offset está configurado.
        if PHYS_OFFSET.get().is_none() {
            return Err(crate::util::KError::NotSupported);
        }

        // Para regiões MMIO muito altas (acima do espaço mapeado pelo bootloader),
        // precisaríamos criar mapeamentos explícitos. Por ora, assumimos que
        // o bootloader mapeia toda a memória física necessária.
        let _virt = phys_to_virt(x86_64::PhysAddr::new(phys_base));
        Ok(())
    }

    /// Retorna o endereço virtual para acesso a uma região MMIO.
    pub fn mmio_virt_addr(phys_base: u64) -> VirtAddr {
        phys_to_virt(x86_64::PhysAddr::new(phys_base))
    }
