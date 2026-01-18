//! Graphics Execution Manager (GEM) and Global Translation Table (GTT)
//!
//! Memory management for Intel graphics:
//! - GEM: Object-based memory management for GPU buffers
//! - GTT: Translation table mapping GPU addresses to physical memory
//! - GGTT: Global GTT for kernel-accessible objects
//! - PPGTT: Per-process GTT for user-space isolation

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::Mutex;
use x86_64::structures::paging::{PhysFrame, Size4KiB};

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static GEM_STATE: Mutex<Option<GemState>> = Mutex::new(None);
static NEXT_HANDLE: AtomicU32 = AtomicU32::new(1);

/// GEM object handle
pub type GemHandle = u32;

/// GEM state manager
#[derive(Debug)]
pub struct GemState {
    /// MMIO base for GTT operations
    pub gtt_base: u64,
    /// GGTT (Global GTT) size in bytes
    pub ggtt_size: usize,
    /// GGTT entries (4KB pages)
    pub ggtt_entries: usize,
    /// Active GEM objects
    pub objects: BTreeMap<GemHandle, GemObject>,
    /// Allocated GGTT regions
    pub ggtt_allocations: Vec<GttAllocation>,
    /// Total stolen memory (pre-allocated by BIOS)
    pub stolen_memory_size: usize,
    /// Aperture base address (for CPU access to GPU memory)
    pub aperture_base: u64,
    /// Aperture size
    pub aperture_size: usize,
}

/// GEM object (GPU buffer)
#[derive(Debug, Clone)]
pub struct GemObject {
    pub handle: GemHandle,
    pub size: usize,
    pub pages: Vec<u64>,         // Physical page addresses
    pub gtt_offset: Option<u64>, // GTT offset if bound
    pub domain: GemDomain,
    pub tiling: GemTiling,
    pub cache_level: CacheLevel,
    pub pinned: bool,
    pub name: Option<String>,
}

/// GTT allocation tracking
#[derive(Debug, Clone)]
pub struct GttAllocation {
    pub offset: u64,
    pub size: usize,
    pub handle: GemHandle,
}

/// GEM memory domain
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GemDomain {
    /// CPU-accessible
    Cpu,
    /// GPU render target
    Render,
    /// GPU sampler (textures)
    Sampler,
    /// Video encode/decode
    Video,
    /// Display scanout
    Display,
}

/// Buffer tiling mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GemTiling {
    /// Linear (no tiling)
    None,
    /// X-tiled (horizontal swizzle)
    X,
    /// Y-tiled (vertical swizzle)
    Y,
    /// W-tiled (for stencil)
    W,
    /// 4-tiled (Gen12+)
    Tile4,
    /// 64-tiled (Gen12+)
    Tile64,
}

/// Cache level for GEM objects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheLevel {
    /// Uncached
    Uncached,
    /// Write-combined
    WriteCombined,
    /// LLC (Last Level Cache)
    Llc,
    /// LLC + eLLC (embedded LLC)
    LlcEllc,
}

/// GTT entry format (for Gen8+)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GttEntry {
    pub value: u64,
}

impl GttEntry {
    /// Create GTT entry for a physical page
    pub fn new(phys_addr: u64, valid: bool, cached: bool) -> Self {
        let mut value = phys_addr & 0x0000_FFFF_FFFF_F000; // Physical address bits
        if valid {
            value |= 1; // Present bit
        }
        if cached {
            value |= 0b110 << 3; // PTE caching (LLC)
        }
        Self { value }
    }

    /// Create invalid entry
    pub fn invalid() -> Self {
        Self { value: 0 }
    }

    pub fn is_valid(&self) -> bool {
        (self.value & 1) != 0
    }

    pub fn physical_address(&self) -> u64 {
        self.value & 0x0000_FFFF_FFFF_F000
    }
}

/// PPGTT (Per-Process GTT) for process isolation
#[derive(Debug)]
pub struct Ppgtt {
    /// PML4 table physical address
    pub pml4_addr: u64,
    /// Page directory pointers
    pub pdps: Vec<u64>,
    /// Allocated virtual ranges
    pub allocations: Vec<(u64, usize)>,
}

/// Initialize GEM/GTT subsystem
pub fn init(mmio_base: u64, aperture_base: u64, aperture_size: usize) {
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }

    crate::kprintln!("gem: Initializing GEM/GTT memory manager...");

    // Read GTT size from hardware
    let ggtt_size = detect_ggtt_size(mmio_base);
    let ggtt_entries = ggtt_size / 8; // 8 bytes per GTT entry

    crate::kprintln!("gem: GGTT size: {} MB ({} entries)",
        ggtt_size / (1024 * 1024), ggtt_entries);
    crate::kprintln!("gem: Aperture: {:#x} - {:#x}",
        aperture_base, aperture_base + aperture_size as u64);

    // Detect stolen memory
    let stolen_size = detect_stolen_memory(mmio_base);
    crate::kprintln!("gem: Stolen memory: {} MB", stolen_size / (1024 * 1024));

    let state = GemState {
        gtt_base: mmio_base + 0x800000, // GTT typically at offset 8MB
        ggtt_size,
        ggtt_entries,
        objects: BTreeMap::new(),
        ggtt_allocations: Vec::new(),
        stolen_memory_size: stolen_size,
        aperture_base,
        aperture_size,
    };

    *GEM_STATE.lock() = Some(state);

    // Initialize GTT with invalid entries
    init_ggtt();

    crate::kprintln!("gem: GEM/GTT initialized successfully");
}

/// Detect GGTT size from hardware
fn detect_ggtt_size(mmio_base: u64) -> usize {
    // Read GMCH Graphics Control register
    // This varies by generation, using Gen9+ approach
    unsafe {
        let gms = core::ptr::read_volatile((mmio_base + 0x50000) as *const u32);
        let ggms = (gms >> 6) & 0x3;

        match ggms {
            0 => 0,               // No GTT
            1 => 2 * 1024 * 1024, // 2 MB
            2 => 4 * 1024 * 1024, // 4 MB
            3 => 8 * 1024 * 1024, // 8 MB
            _ => 2 * 1024 * 1024, // Default 2 MB
        }
    }
}

/// Detect stolen memory size
fn detect_stolen_memory(mmio_base: u64) -> usize {
    // Read Graphics Stolen Memory Size from GMCH
    unsafe {
        let gms = core::ptr::read_volatile((mmio_base + 0x50000) as *const u32);
        let stolen_size_code = (gms >> 8) & 0xFF;

        // Gen9+ encoding
        if stolen_size_code <= 0xF0 {
            (stolen_size_code as usize) * 32 * 1024 * 1024 // 32 MB units
        } else {
            4 * 1024 * 1024 // Default 4 MB
        }
    }
}

/// Initialize GGTT with invalid entries
fn init_ggtt() {
    let state = GEM_STATE.lock();
    if let Some(ref state) = *state {
        unsafe {
            let gtt_ptr = state.gtt_base as *mut GttEntry;
            for i in 0..state.ggtt_entries {
                core::ptr::write_volatile(gtt_ptr.add(i), GttEntry::invalid());
            }
        }
    }
}

/// Create a new GEM object
pub fn gem_create(size: usize) -> Option<GemHandle> {
    let mut state = GEM_STATE.lock();
    let state = state.as_mut()?;

    // Align size to page boundary
    let aligned_size = (size + 4095) & !4095;
    let num_pages = aligned_size / 4096;

    // Allocate physical pages
    let mut pages = Vec::with_capacity(num_pages);
    for _ in 0..num_pages {
        let frame = crate::mm::alloc_frame()?;
        pages.push(frame.start_address().as_u64());
    }

    let handle = NEXT_HANDLE.fetch_add(1, Ordering::SeqCst);

    let object = GemObject {
        handle,
        size: aligned_size,
        pages,
        gtt_offset: None,
        domain: GemDomain::Cpu,
        tiling: GemTiling::None,
        cache_level: CacheLevel::Llc,
        pinned: false,
        name: None,
    };

    state.objects.insert(handle, object);

    crate::kprintln!("gem: Created object {} ({} bytes, {} pages)",
        handle, aligned_size, num_pages);

    Some(handle)
}

/// Close (free) a GEM object
pub fn gem_close(handle: GemHandle) -> bool {
    let mut state = GEM_STATE.lock();
    let state = match state.as_mut() {
        Some(s) => s,
        None => return false,
    };

    if let Some(object) = state.objects.remove(&handle) {
        // Unbind from GTT if bound
        if object.gtt_offset.is_some() {
            unbind_from_gtt_internal(state, handle);
        }

        // Free physical pages
        for page_addr in object.pages {
            let frame = PhysFrame::<Size4KiB>::containing_address(x86_64::PhysAddr::new(page_addr));
            crate::mm::free_frame(frame);
        }

        crate::kprintln!("gem: Closed object {}", handle);
        true
    } else {
        false
    }
}

/// Bind GEM object to GTT (make it GPU-accessible)
pub fn gem_bind(handle: GemHandle) -> Option<u64> {
    let mut state = GEM_STATE.lock();
    let state = state.as_mut()?;

    let object = state.objects.get(&handle)?;

    // Already bound?
    if let Some(offset) = object.gtt_offset {
        return Some(offset);
    }

    // Find free space in GGTT
    let size = object.size;
    let offset = find_ggtt_space(state, size)?;

    // Write GTT entries
    let gtt_ptr = state.gtt_base as *mut GttEntry;
    let start_entry = (offset / 4096) as usize;
    let cached = object.cache_level != CacheLevel::Uncached;

    for (i, &page) in object.pages.iter().enumerate() {
        let entry = GttEntry::new(page, true, cached);
        unsafe {
            core::ptr::write_volatile(gtt_ptr.add(start_entry + i), entry);
        }
    }

    // Record allocation
    state.ggtt_allocations.push(GttAllocation {
        offset,
        size,
        handle,
    });

    // Update object
    if let Some(obj) = state.objects.get_mut(&handle) {
        obj.gtt_offset = Some(offset);
    }

    crate::kprintln!("gem: Bound object {} at GTT offset {:#x}", handle, offset);

    Some(offset)
}

/// Unbind GEM object from GTT
pub fn gem_unbind(handle: GemHandle) -> bool {
    let mut state = GEM_STATE.lock();
    let state = match state.as_mut() {
        Some(s) => s,
        None => return false,
    };

    unbind_from_gtt_internal(state, handle)
}

fn unbind_from_gtt_internal(state: &mut GemState, handle: GemHandle) -> bool {
    let object = match state.objects.get(&handle) {
        Some(o) => o,
        None => return false,
    };

    let offset = match object.gtt_offset {
        Some(o) => o,
        None => return true, // Already unbound
    };

    // Clear GTT entries
    let gtt_ptr = state.gtt_base as *mut GttEntry;
    let start_entry = (offset / 4096) as usize;
    let num_entries = object.pages.len();

    for i in 0..num_entries {
        unsafe {
            core::ptr::write_volatile(gtt_ptr.add(start_entry + i), GttEntry::invalid());
        }
    }

    // Remove allocation record
    state.ggtt_allocations.retain(|a| a.handle != handle);

    // Update object
    if let Some(obj) = state.objects.get_mut(&handle) {
        obj.gtt_offset = None;
    }

    crate::kprintln!("gem: Unbound object {}", handle);
    true
}

/// Find free space in GGTT
fn find_ggtt_space(state: &GemState, size: usize) -> Option<u64> {
    let mut offset: u64 = 0;
    let max_offset = (state.ggtt_entries * 4096) as u64;

    // Simple first-fit allocator
    'outer: loop {
        if offset + size as u64 > max_offset {
            return None;
        }

        // Check if this region overlaps with any allocation
        for alloc in &state.ggtt_allocations {
            if offset < alloc.offset + alloc.size as u64 && offset + size as u64 > alloc.offset {
                // Overlap, skip past this allocation
                offset = alloc.offset + alloc.size as u64;
                offset = (offset + 4095) & !4095; // Align to page
                continue 'outer;
            }
        }

        return Some(offset);
    }
}

/// Set GEM object tiling mode
pub fn gem_set_tiling(handle: GemHandle, tiling: GemTiling, stride: u32) -> bool {
    let mut state = GEM_STATE.lock();
    let state = match state.as_mut() {
        Some(s) => s,
        None => return false,
    };

    if let Some(obj) = state.objects.get_mut(&handle) {
        obj.tiling = tiling;
        crate::kprintln!("gem: Set object {} tiling to {:?} (stride {})",
            handle, tiling, stride);
        true
    } else {
        false
    }
}

/// Set GEM object cache level
pub fn gem_set_cache_level(handle: GemHandle, level: CacheLevel) -> bool {
    let mut state = GEM_STATE.lock();
    let state = match state.as_mut() {
        Some(s) => s,
        None => return false,
    };

    if let Some(obj) = state.objects.get_mut(&handle) {
        obj.cache_level = level;
        true
    } else {
        false
    }
}

/// Pin GEM object (prevent unbinding)
pub fn gem_pin(handle: GemHandle) -> bool {
    let mut state = GEM_STATE.lock();
    let state = match state.as_mut() {
        Some(s) => s,
        None => return false,
    };

    if let Some(obj) = state.objects.get_mut(&handle) {
        obj.pinned = true;
        true
    } else {
        false
    }
}

/// Unpin GEM object
pub fn gem_unpin(handle: GemHandle) -> bool {
    let mut state = GEM_STATE.lock();
    let state = match state.as_mut() {
        Some(s) => s,
        None => return false,
    };

    if let Some(obj) = state.objects.get_mut(&handle) {
        obj.pinned = false;
        true
    } else {
        false
    }
}

/// Map GEM object for CPU access
pub fn gem_mmap(handle: GemHandle) -> Option<*mut u8> {
    let state = GEM_STATE.lock();
    let state = state.as_ref()?;

    let object = state.objects.get(&handle)?;

    // Object must be bound to GTT
    let gtt_offset = object.gtt_offset?;

    // Return aperture address for CPU access
    let cpu_addr = state.aperture_base + gtt_offset;
    Some(cpu_addr as *mut u8)
}

/// Get GEM object info
pub fn gem_get_info(handle: GemHandle) -> Option<GemObject> {
    let state = GEM_STATE.lock();
    let state = state.as_ref()?;
    state.objects.get(&handle).cloned()
}

/// Get total allocated GEM memory
pub fn gem_get_allocated_memory() -> usize {
    let state = GEM_STATE.lock();
    match state.as_ref() {
        Some(s) => s.objects.values().map(|o| o.size).sum(),
        None => 0,
    }
}

/// Flush GPU caches for a GEM object
pub fn gem_flush_caches(handle: GemHandle) {
    // In a real implementation, this would issue GPU commands
    // to flush caches for the specified object
    let _ = handle;
}

/// Create a PPGTT for process isolation
pub fn ppgtt_create() -> Option<Ppgtt> {
    // Allocate PML4 table
    let frame = crate::mm::alloc_frame()?;
    let pml4_addr = frame.start_address().as_u64();

    // Zero the page - use virtual address via phys_to_virt
    let virt = crate::mm::phys_to_virt(frame.start_address());
    unsafe {
        core::ptr::write_bytes(virt.as_mut_ptr::<u8>(), 0, 4096);
    }

    Some(Ppgtt {
        pml4_addr,
        pdps: Vec::new(),
        allocations: Vec::new(),
    })
}

/// Destroy PPGTT
pub fn ppgtt_destroy(ppgtt: &Ppgtt) {
    let frame = PhysFrame::<Size4KiB>::containing_address(x86_64::PhysAddr::new(ppgtt.pml4_addr));
    crate::mm::free_frame(frame);
    for &pdp in &ppgtt.pdps {
        let frame = PhysFrame::<Size4KiB>::containing_address(x86_64::PhysAddr::new(pdp));
        crate::mm::free_frame(frame);
    }
}

/// Get statistics
pub fn get_stats() -> Option<GemStats> {
    let state = GEM_STATE.lock();
    let state = state.as_ref()?;

    Some(GemStats {
        total_objects: state.objects.len(),
        total_memory: state.objects.values().map(|o| o.size).sum(),
        bound_objects: state.objects.values().filter(|o| o.gtt_offset.is_some()).count(),
        ggtt_used: state.ggtt_allocations.iter().map(|a| a.size).sum(),
        ggtt_total: state.ggtt_size,
    })
}

/// GEM statistics
#[derive(Debug, Clone)]
pub struct GemStats {
    pub total_objects: usize,
    pub total_memory: usize,
    pub bound_objects: usize,
    pub ggtt_used: usize,
    pub ggtt_total: usize,
}
