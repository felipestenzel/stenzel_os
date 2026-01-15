    use linked_list_allocator::LockedHeap;
    use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB};
    use x86_64::VirtAddr;

    use crate::mm::paging;
    use crate::util::{KError, KResult};

    // Heap do kernel em uma faixa fixa (alta o suficiente para evitar colisões com outras regiões).
    pub const HEAP_START: u64 = 0xFFFF_9000_0000_0000;
    pub const HEAP_SIZE: usize = 64 * 1024 * 1024; // 64 MiB (aumentado para suportar bitmap grande)

    #[global_allocator]
    static ALLOCATOR: LockedHeap = LockedHeap::empty();

    pub fn init_heap(
        mapper: &mut paging::OffsetMapper,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> KResult<()> {
        let heap_start = VirtAddr::new(HEAP_START);
        let heap_end = heap_start + (HEAP_SIZE as u64) - 1u64;

        let start_page: Page<Size4KiB> = Page::containing_address(heap_start);
        let end_page: Page<Size4KiB> = Page::containing_address(heap_end);

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        for page in Page::range_inclusive(start_page, end_page) {
            let frame = frame_allocator.allocate_frame().ok_or(KError::NoMemory)?;
            unsafe {
                mapper
                    .map_to(page, frame, flags, frame_allocator)
                    .map_err(|_| KError::NoMemory)?
                    .flush();
            }
        }

        unsafe {
            ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
        }

        Ok(())
    }
