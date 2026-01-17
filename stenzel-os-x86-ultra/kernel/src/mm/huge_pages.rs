//! Huge Pages Support (2MB and 1GB pages)
//!
//! This module provides support for huge pages on x86_64, which can significantly
//! reduce TLB pressure and improve performance for applications with large memory
//! footprints.
//!
//! ## Page Sizes
//! - Normal pages: 4 KiB (4096 bytes)
//! - 2MB huge pages: 2 MiB (2097152 bytes) - 512 normal pages
//! - 1GB huge pages: 1 GiB (1073741824 bytes) - 262144 normal pages
//!
//! ## Usage
//! Huge pages are useful for:
//! - Large memory mappings (databases, scientific computing)
//! - Reducing page table memory overhead
//! - Reducing TLB misses

#![allow(dead_code)]

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame, Size2MiB, Size1GiB, Size4KiB, FrameAllocator};
use x86_64::{PhysAddr, VirtAddr};

use crate::util::{KError, KResult};

/// Size of a 2MB huge page in bytes
pub const HUGE_PAGE_SIZE_2MB: u64 = 2 * 1024 * 1024; // 2 MiB
/// Size of a 1GB huge page in bytes
pub const HUGE_PAGE_SIZE_1GB: u64 = 1024 * 1024 * 1024; // 1 GiB
/// Size of a normal 4KB page in bytes
pub const PAGE_SIZE_4KB: u64 = 4096;

/// Huge page size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HugePageSize {
    /// 2MB huge page
    Size2MiB,
    /// 1GB huge page
    Size1GiB,
}

impl HugePageSize {
    /// Get the size in bytes
    pub fn size_bytes(&self) -> u64 {
        match self {
            HugePageSize::Size2MiB => HUGE_PAGE_SIZE_2MB,
            HugePageSize::Size1GiB => HUGE_PAGE_SIZE_1GB,
        }
    }

    /// Get the alignment requirement in bytes
    pub fn alignment(&self) -> u64 {
        self.size_bytes()
    }

    /// Number of 4KB pages contained in this huge page
    pub fn pages_4kb(&self) -> u64 {
        self.size_bytes() / PAGE_SIZE_4KB
    }
}

/// Statistics for huge page usage
#[derive(Debug, Clone, Copy)]
pub struct HugePageStats {
    /// Total 2MB huge pages available
    pub total_2mb: u64,
    /// Free 2MB huge pages
    pub free_2mb: u64,
    /// Used 2MB huge pages
    pub used_2mb: u64,
    /// Total 1GB huge pages available
    pub total_1gb: u64,
    /// Free 1GB huge pages
    pub free_1gb: u64,
    /// Used 1GB huge pages
    pub used_1gb: u64,
}

/// Huge page pool for pre-allocated huge pages
pub struct HugePagePool {
    /// Pre-allocated 2MB huge pages (physical frames)
    pool_2mb: Vec<PhysFrame<Size2MiB>>,
    /// Pre-allocated 1GB huge pages (physical frames)
    pool_1gb: Vec<PhysFrame<Size1GiB>>,
    /// Statistics
    stats: HugePagePoolStats,
}

#[derive(Debug)]
struct HugePagePoolStats {
    allocations_2mb: AtomicU64,
    allocations_1gb: AtomicU64,
    deallocations_2mb: AtomicU64,
    deallocations_1gb: AtomicU64,
    failed_allocations_2mb: AtomicU64,
    failed_allocations_1gb: AtomicU64,
}

impl HugePagePoolStats {
    const fn new() -> Self {
        Self {
            allocations_2mb: AtomicU64::new(0),
            allocations_1gb: AtomicU64::new(0),
            deallocations_2mb: AtomicU64::new(0),
            deallocations_1gb: AtomicU64::new(0),
            failed_allocations_2mb: AtomicU64::new(0),
            failed_allocations_1gb: AtomicU64::new(0),
        }
    }
}

impl HugePagePool {
    /// Create a new empty huge page pool
    pub const fn new() -> Self {
        Self {
            pool_2mb: Vec::new(),
            pool_1gb: Vec::new(),
            stats: HugePagePoolStats::new(),
        }
    }

    /// Add a 2MB huge page to the pool
    pub fn add_2mb(&mut self, frame: PhysFrame<Size2MiB>) {
        self.pool_2mb.push(frame);
    }

    /// Add a 1GB huge page to the pool
    pub fn add_1gb(&mut self, frame: PhysFrame<Size1GiB>) {
        self.pool_1gb.push(frame);
    }

    /// Take a 2MB huge page from the pool
    pub fn take_2mb(&mut self) -> Option<PhysFrame<Size2MiB>> {
        let frame = self.pool_2mb.pop();
        if frame.is_some() {
            self.stats.allocations_2mb.fetch_add(1, Ordering::Relaxed);
        } else {
            self.stats.failed_allocations_2mb.fetch_add(1, Ordering::Relaxed);
        }
        frame
    }

    /// Take a 1GB huge page from the pool
    pub fn take_1gb(&mut self) -> Option<PhysFrame<Size1GiB>> {
        let frame = self.pool_1gb.pop();
        if frame.is_some() {
            self.stats.allocations_1gb.fetch_add(1, Ordering::Relaxed);
        } else {
            self.stats.failed_allocations_1gb.fetch_add(1, Ordering::Relaxed);
        }
        frame
    }

    /// Return a 2MB huge page to the pool
    pub fn return_2mb(&mut self, frame: PhysFrame<Size2MiB>) {
        self.pool_2mb.push(frame);
        self.stats.deallocations_2mb.fetch_add(1, Ordering::Relaxed);
    }

    /// Return a 1GB huge page to the pool
    pub fn return_1gb(&mut self, frame: PhysFrame<Size1GiB>) {
        self.pool_1gb.push(frame);
        self.stats.deallocations_1gb.fetch_add(1, Ordering::Relaxed);
    }

    /// Get the number of available 2MB huge pages in the pool
    pub fn available_2mb(&self) -> usize {
        self.pool_2mb.len()
    }

    /// Get the number of available 1GB huge pages in the pool
    pub fn available_1gb(&self) -> usize {
        self.pool_1gb.len()
    }
}

/// Global huge page pool
static HUGE_PAGE_POOL: Mutex<Option<HugePagePool>> = Mutex::new(None);

/// Global huge page statistics
static HUGE_PAGES_ALLOCATED_2MB: AtomicU64 = AtomicU64::new(0);
static HUGE_PAGES_ALLOCATED_1GB: AtomicU64 = AtomicU64::new(0);

/// Initialize the huge page subsystem
pub fn init() {
    let mut pool = HUGE_PAGE_POOL.lock();
    *pool = Some(HugePagePool::new());
    crate::kprintln!("huge_pages: initialized");
}

/// Check if the CPU supports 2MB huge pages (always true on x86_64)
pub fn supports_2mb() -> bool {
    // All x86_64 CPUs support 2MB pages via the Page Size Extension (PSE)
    true
}

/// Check if the CPU supports 1GB huge pages
pub fn supports_1gb() -> bool {
    // Check CPUID for 1GB page support (PDPE1GB bit)
    use core::arch::x86_64::__cpuid;

    unsafe {
        let result = __cpuid(0x80000001);
        // Bit 26 of EDX indicates 1GB page support
        (result.edx & (1 << 26)) != 0
    }
}

/// Pre-allocate huge pages into the pool
pub fn preallocate_pool(count_2mb: usize, count_1gb: usize) -> (usize, usize) {
    let mut pool_guard = HUGE_PAGE_POOL.lock();
    let pool = match pool_guard.as_mut() {
        Some(p) => p,
        None => return (0, 0),
    };
    drop(pool_guard);

    let mut allocated_2mb = 0;
    let mut allocated_1gb = 0;

    // Allocate 2MB pages
    for _ in 0..count_2mb {
        if let Some(frame) = alloc_huge_frame_2mb() {
            let mut pool_guard = HUGE_PAGE_POOL.lock();
            if let Some(pool) = pool_guard.as_mut() {
                pool.add_2mb(frame);
                allocated_2mb += 1;
            }
        } else {
            break;
        }
    }

    // Allocate 1GB pages (only if CPU supports them)
    if supports_1gb() {
        for _ in 0..count_1gb {
            if let Some(frame) = alloc_huge_frame_1gb() {
                let mut pool_guard = HUGE_PAGE_POOL.lock();
                if let Some(pool) = pool_guard.as_mut() {
                    pool.add_1gb(frame);
                    allocated_1gb += 1;
                }
            } else {
                break;
            }
        }
    }

    crate::kprintln!(
        "huge_pages: preallocated {} x 2MB, {} x 1GB pages",
        allocated_2mb,
        allocated_1gb
    );

    (allocated_2mb, allocated_1gb)
}

/// Allocate a 2MB huge page frame from the physical allocator
fn alloc_huge_frame_2mb() -> Option<PhysFrame<Size2MiB>> {
    let mut fa = crate::mm::frame_allocator_lock();
    let frame = fa.allocate_huge_2mb()?;
    HUGE_PAGES_ALLOCATED_2MB.fetch_add(1, Ordering::Relaxed);
    Some(frame)
}

/// Allocate a 1GB huge page frame from the physical allocator
fn alloc_huge_frame_1gb() -> Option<PhysFrame<Size1GiB>> {
    let mut fa = crate::mm::frame_allocator_lock();
    let frame = fa.allocate_huge_1gb()?;
    HUGE_PAGES_ALLOCATED_1GB.fetch_add(1, Ordering::Relaxed);
    Some(frame)
}

/// Deallocate a 2MB huge page frame
fn dealloc_huge_frame_2mb(frame: PhysFrame<Size2MiB>) {
    let mut fa = crate::mm::frame_allocator_lock();
    fa.deallocate_huge_2mb(frame);
    HUGE_PAGES_ALLOCATED_2MB.fetch_sub(1, Ordering::Relaxed);
}

/// Deallocate a 1GB huge page frame
fn dealloc_huge_frame_1gb(frame: PhysFrame<Size1GiB>) {
    let mut fa = crate::mm::frame_allocator_lock();
    fa.deallocate_huge_1gb(frame);
    HUGE_PAGES_ALLOCATED_1GB.fetch_sub(1, Ordering::Relaxed);
}

/// Allocate a 2MB huge page from the pool or directly
pub fn alloc_2mb() -> Option<PhysFrame<Size2MiB>> {
    // Try pool first
    {
        let mut pool_guard = HUGE_PAGE_POOL.lock();
        if let Some(pool) = pool_guard.as_mut() {
            if let Some(frame) = pool.take_2mb() {
                return Some(frame);
            }
        }
    }
    // Fall back to direct allocation
    alloc_huge_frame_2mb()
}

/// Allocate a 1GB huge page from the pool or directly
pub fn alloc_1gb() -> Option<PhysFrame<Size1GiB>> {
    if !supports_1gb() {
        return None;
    }
    // Try pool first
    {
        let mut pool_guard = HUGE_PAGE_POOL.lock();
        if let Some(pool) = pool_guard.as_mut() {
            if let Some(frame) = pool.take_1gb() {
                return Some(frame);
            }
        }
    }
    // Fall back to direct allocation
    alloc_huge_frame_1gb()
}

/// Free a 2MB huge page back to the pool
pub fn free_2mb(frame: PhysFrame<Size2MiB>) {
    let mut pool_guard = HUGE_PAGE_POOL.lock();
    if let Some(pool) = pool_guard.as_mut() {
        pool.return_2mb(frame);
    } else {
        drop(pool_guard);
        dealloc_huge_frame_2mb(frame);
    }
}

/// Free a 1GB huge page back to the pool
pub fn free_1gb(frame: PhysFrame<Size1GiB>) {
    let mut pool_guard = HUGE_PAGE_POOL.lock();
    if let Some(pool) = pool_guard.as_mut() {
        pool.return_1gb(frame);
    } else {
        drop(pool_guard);
        dealloc_huge_frame_1gb(frame);
    }
}

/// Map a 2MB huge page at the given virtual address
pub fn map_huge_2mb(
    virt_addr: VirtAddr,
    flags: PageTableFlags,
) -> KResult<PhysFrame<Size2MiB>> {
    // Ensure virtual address is 2MB-aligned
    if virt_addr.as_u64() % HUGE_PAGE_SIZE_2MB != 0 {
        return Err(KError::Invalid);
    }

    let frame = alloc_2mb().ok_or(KError::NoMemory)?;
    let page: Page<Size2MiB> = Page::containing_address(virt_addr);

    let mut mapper = crate::mm::mapper_lock();
    let mut fa = crate::mm::frame_allocator_lock();

    // Add HUGE_PAGE flag
    let huge_flags = flags | PageTableFlags::HUGE_PAGE;

    mapper.map_huge_2mb(page, frame, huge_flags, &mut *fa)?;

    Ok(frame)
}

/// Map a 1GB huge page at the given virtual address
pub fn map_huge_1gb(
    virt_addr: VirtAddr,
    flags: PageTableFlags,
) -> KResult<PhysFrame<Size1GiB>> {
    if !supports_1gb() {
        return Err(KError::NotSupported);
    }

    // Ensure virtual address is 1GB-aligned
    if virt_addr.as_u64() % HUGE_PAGE_SIZE_1GB != 0 {
        return Err(KError::Invalid);
    }

    let frame = alloc_1gb().ok_or(KError::NoMemory)?;
    let page: Page<Size1GiB> = Page::containing_address(virt_addr);

    let mut mapper = crate::mm::mapper_lock();
    let mut fa = crate::mm::frame_allocator_lock();

    // Add HUGE_PAGE flag
    let huge_flags = flags | PageTableFlags::HUGE_PAGE;

    mapper.map_huge_1gb(page, frame, huge_flags, &mut *fa)?;

    Ok(frame)
}

/// Unmap a 2MB huge page at the given virtual address
pub fn unmap_huge_2mb(virt_addr: VirtAddr) -> KResult<()> {
    // Ensure virtual address is 2MB-aligned
    if virt_addr.as_u64() % HUGE_PAGE_SIZE_2MB != 0 {
        return Err(KError::Invalid);
    }

    let page: Page<Size2MiB> = Page::containing_address(virt_addr);

    let mut mapper = crate::mm::mapper_lock();
    let (frame, flush) = mapper.unmap_huge_2mb(page)?;
    flush();

    free_2mb(frame);
    Ok(())
}

/// Unmap a 1GB huge page at the given virtual address
pub fn unmap_huge_1gb(virt_addr: VirtAddr) -> KResult<()> {
    // Ensure virtual address is 1GB-aligned
    if virt_addr.as_u64() % HUGE_PAGE_SIZE_1GB != 0 {
        return Err(KError::Invalid);
    }

    let page: Page<Size1GiB> = Page::containing_address(virt_addr);

    let mut mapper = crate::mm::mapper_lock();
    let (frame, flush) = mapper.unmap_huge_1gb(page)?;
    flush();

    free_1gb(frame);
    Ok(())
}

/// Get huge page statistics
pub fn get_stats() -> HugePageStats {
    let fa = crate::mm::frame_allocator_lock();
    let free_2mb = fa.count_huge_2mb_available() as u64;
    let free_1gb = fa.count_huge_1gb_available() as u64;

    let used_2mb = HUGE_PAGES_ALLOCATED_2MB.load(Ordering::Relaxed);
    let used_1gb = HUGE_PAGES_ALLOCATED_1GB.load(Ordering::Relaxed);

    HugePageStats {
        total_2mb: free_2mb + used_2mb,
        free_2mb,
        used_2mb,
        total_1gb: free_1gb + used_1gb,
        free_1gb,
        used_1gb,
    }
}

/// Get pool statistics
pub fn get_pool_stats() -> Option<(usize, usize)> {
    let pool_guard = HUGE_PAGE_POOL.lock();
    pool_guard.as_ref().map(|pool| {
        (pool.available_2mb(), pool.available_1gb())
    })
}

/// Align an address up to the nearest 2MB boundary
pub fn align_up_2mb(addr: u64) -> u64 {
    let mask = HUGE_PAGE_SIZE_2MB - 1;
    (addr + mask) & !mask
}

/// Align an address down to the nearest 2MB boundary
pub fn align_down_2mb(addr: u64) -> u64 {
    addr & !(HUGE_PAGE_SIZE_2MB - 1)
}

/// Align an address up to the nearest 1GB boundary
pub fn align_up_1gb(addr: u64) -> u64 {
    let mask = HUGE_PAGE_SIZE_1GB - 1;
    (addr + mask) & !mask
}

/// Align an address down to the nearest 1GB boundary
pub fn align_down_1gb(addr: u64) -> u64 {
    addr & !(HUGE_PAGE_SIZE_1GB - 1)
}

/// Check if an address is 2MB-aligned
pub fn is_aligned_2mb(addr: u64) -> bool {
    addr % HUGE_PAGE_SIZE_2MB == 0
}

/// Check if an address is 1GB-aligned
pub fn is_aligned_1gb(addr: u64) -> bool {
    addr % HUGE_PAGE_SIZE_1GB == 0
}

/// Calculate the number of 2MB huge pages needed to cover a region
pub fn pages_2mb_for_region(size: u64) -> u64 {
    (size + HUGE_PAGE_SIZE_2MB - 1) / HUGE_PAGE_SIZE_2MB
}

/// Calculate the number of 1GB huge pages needed to cover a region
pub fn pages_1gb_for_region(size: u64) -> u64 {
    (size + HUGE_PAGE_SIZE_1GB - 1) / HUGE_PAGE_SIZE_1GB
}

/// Format huge page info for /proc/meminfo
pub fn format_meminfo() -> alloc::string::String {
    use alloc::format;

    let stats = get_stats();
    let (pool_2mb, pool_1gb) = get_pool_stats().unwrap_or((0, 0));

    format!(
        "HugePages_Total_2MB: {}\n\
         HugePages_Free_2MB:  {}\n\
         HugePages_Used_2MB:  {}\n\
         HugePages_Pool_2MB:  {}\n\
         HugePages_Total_1GB: {}\n\
         HugePages_Free_1GB:  {}\n\
         HugePages_Used_1GB:  {}\n\
         HugePages_Pool_1GB:  {}\n\
         Hugepagesize_2MB:    {} kB\n\
         Hugepagesize_1GB:    {} kB\n\
         1GB_Support:         {}\n",
        stats.total_2mb,
        stats.free_2mb,
        stats.used_2mb,
        pool_2mb,
        stats.total_1gb,
        stats.free_1gb,
        stats.used_1gb,
        pool_1gb,
        HUGE_PAGE_SIZE_2MB / 1024,
        HUGE_PAGE_SIZE_1GB / 1024,
        if supports_1gb() { "yes" } else { "no" }
    )
}

/// Transparent Huge Pages (THP) policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThpPolicy {
    /// Never use huge pages automatically
    Never,
    /// Always try to use huge pages
    Always,
    /// Only use huge pages when explicitly requested via madvise
    Madvise,
}

static THP_POLICY: AtomicU64 = AtomicU64::new(0); // Default: Never

/// Get the current THP policy
pub fn get_thp_policy() -> ThpPolicy {
    match THP_POLICY.load(Ordering::Relaxed) {
        0 => ThpPolicy::Never,
        1 => ThpPolicy::Always,
        2 => ThpPolicy::Madvise,
        _ => ThpPolicy::Never,
    }
}

/// Set the THP policy
pub fn set_thp_policy(policy: ThpPolicy) {
    let val = match policy {
        ThpPolicy::Never => 0,
        ThpPolicy::Always => 1,
        ThpPolicy::Madvise => 2,
    };
    THP_POLICY.store(val, Ordering::Relaxed);
}

/// Check if a virtual address range should use huge pages based on THP policy
pub fn should_use_huge_pages(size: u64, madvise_requested: bool) -> bool {
    match get_thp_policy() {
        ThpPolicy::Never => false,
        ThpPolicy::Always => size >= HUGE_PAGE_SIZE_2MB,
        ThpPolicy::Madvise => madvise_requested && size >= HUGE_PAGE_SIZE_2MB,
    }
}
