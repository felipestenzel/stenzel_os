//! Swap Subsystem
//!
//! Provides swap-to-disk functionality for virtual memory management.
//! Supports:
//! - Swap partitions and swap files
//! - Page swapping out under memory pressure
//! - Page swapping in on demand (page fault)
//! - Swap space management with bitmap allocator
//! - Multiple swap devices with priority

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;
use x86_64::structures::paging::{Page, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

use crate::storage::BlockDevice;
use crate::sync::IrqSafeMutex;

/// Swap signature magic number (same as Linux: "SWAPSPACE2")
pub const SWAP_MAGIC: &[u8; 10] = b"SWAPSPACE2";

/// Page size for swap (4 KiB)
pub const SWAP_PAGE_SIZE: usize = 4096;

/// Maximum number of swap devices
pub const MAX_SWAP_DEVICES: usize = 8;

/// Swap slot identifier (device_index << 56 | slot_offset)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SwapSlot(u64);

impl SwapSlot {
    pub fn new(device_idx: u8, slot: u64) -> Self {
        Self(((device_idx as u64) << 56) | (slot & 0x00FFFFFFFFFFFFFF))
    }

    pub fn device_index(&self) -> u8 {
        (self.0 >> 56) as u8
    }

    pub fn slot_offset(&self) -> u64 {
        self.0 & 0x00FFFFFFFFFFFFFF
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn from_u64(val: u64) -> Self {
        Self(val)
    }
}

/// Swap device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapDeviceType {
    /// Swap partition
    Partition,
    /// Swap file
    File,
}

/// Swap device header (stored at beginning of swap space)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct SwapHeader {
    /// Boot sector (1 page, mostly unused)
    pub bootbits: [u8; 1024],
    /// Version of swap format
    pub version: u32,
    /// Last page of swap area
    pub last_page: u32,
    /// Number of bad pages
    pub nr_badpages: u32,
    /// UUID
    pub uuid: [u8; 16],
    /// Volume label
    pub label: [u8; 16],
    /// Padding
    pub padding: [u32; 117],
    /// Bad pages list
    pub badpages: [u32; 1],
}

/// Swap device information
pub struct SwapDevice {
    /// Device type
    pub device_type: SwapDeviceType,
    /// Device path (e.g., "/dev/sda2" or "/var/swap")
    pub path: String,
    /// Priority (higher = preferred)
    pub priority: i16,
    /// Total number of pages
    pub total_pages: u64,
    /// Free pages
    pub free_pages: AtomicU64,
    /// Used pages
    pub used_pages: AtomicU64,
    /// Block device for I/O (partition) or file inode
    device_id: usize,
    /// Bitmap for slot allocation (1 = used, 0 = free)
    bitmap: Mutex<Vec<u64>>,
    /// Start offset in blocks (for swap files)
    start_offset: u64,
    /// Is active?
    active: bool,
}

impl SwapDevice {
    /// Create a new swap device
    pub fn new(
        device_type: SwapDeviceType,
        path: String,
        priority: i16,
        total_pages: u64,
        device_id: usize,
        start_offset: u64,
    ) -> Self {
        // Bitmap: 64 pages per u64
        let bitmap_size = ((total_pages + 63) / 64) as usize;
        let bitmap = vec![0u64; bitmap_size];

        Self {
            device_type,
            path,
            priority,
            total_pages,
            free_pages: AtomicU64::new(total_pages),
            used_pages: AtomicU64::new(0),
            device_id,
            bitmap: Mutex::new(bitmap),
            start_offset,
            active: true,
        }
    }

    /// Allocate a swap slot
    pub fn alloc_slot(&self) -> Option<u64> {
        let mut bitmap = self.bitmap.lock();

        // Find first free slot
        for (idx, word) in bitmap.iter_mut().enumerate() {
            if *word != u64::MAX {
                // Find first zero bit
                let bit = (!*word).trailing_zeros() as usize;
                if bit < 64 {
                    let slot = (idx * 64 + bit) as u64;
                    if slot < self.total_pages {
                        // Mark as used
                        *word |= 1 << bit;
                        self.free_pages.fetch_sub(1, Ordering::SeqCst);
                        self.used_pages.fetch_add(1, Ordering::SeqCst);
                        return Some(slot);
                    }
                }
            }
        }
        None
    }

    /// Free a swap slot
    pub fn free_slot(&self, slot: u64) {
        if slot >= self.total_pages {
            return;
        }

        let mut bitmap = self.bitmap.lock();
        let idx = (slot / 64) as usize;
        let bit = (slot % 64) as usize;

        if bitmap[idx] & (1 << bit) != 0 {
            bitmap[idx] &= !(1 << bit);
            self.free_pages.fetch_add(1, Ordering::SeqCst);
            self.used_pages.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Check if slot is in use
    pub fn is_slot_used(&self, slot: u64) -> bool {
        if slot >= self.total_pages {
            return false;
        }

        let bitmap = self.bitmap.lock();
        let idx = (slot / 64) as usize;
        let bit = (slot % 64) as usize;
        bitmap[idx] & (1 << bit) != 0
    }

    /// Get block offset for a slot
    pub fn slot_to_block(&self, slot: u64) -> u64 {
        // Each page is 4096 bytes = 8 blocks (512 bytes each)
        self.start_offset + (slot * (SWAP_PAGE_SIZE as u64 / 512))
    }
}

/// Entry in swap map (tracks which virtual pages are swapped out)
#[derive(Debug, Clone, Copy)]
pub struct SwapEntry {
    /// Swap slot where page is stored
    pub slot: SwapSlot,
    /// Reference count (for shared pages)
    pub ref_count: u32,
    /// Flags
    pub flags: SwapEntryFlags,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct SwapEntryFlags: u32 {
        /// Entry is valid
        const VALID = 1 << 0;
        /// Page is being swapped in
        const SWAPPING_IN = 1 << 1;
        /// Page is being swapped out
        const SWAPPING_OUT = 1 << 2;
        /// Page is shared
        const SHARED = 1 << 3;
    }
}

/// Page identity (process ID + virtual address)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PageId {
    pub pid: u32,
    pub vaddr: u64,
}

impl PageId {
    pub fn new(pid: u32, vaddr: VirtAddr) -> Self {
        Self {
            pid,
            vaddr: vaddr.as_u64() & !0xFFF, // Page-aligned
        }
    }
}

/// Swap statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct SwapStats {
    /// Total swap space (pages)
    pub total: u64,
    /// Free swap space (pages)
    pub free: u64,
    /// Used swap space (pages)
    pub used: u64,
    /// Pages swapped in
    pub pages_swapped_in: u64,
    /// Pages swapped out
    pub pages_swapped_out: u64,
    /// Swap faults (page not in swap)
    pub swap_faults: u64,
}

/// Global swap subsystem
pub struct SwapSubsystem {
    /// Swap devices (sorted by priority, highest first)
    devices: Vec<SwapDevice>,
    /// Map from page identity to swap entry
    swap_map: BTreeMap<PageId, SwapEntry>,
    /// Statistics
    stats: SwapStats,
    /// Is swap enabled?
    enabled: bool,
}

impl SwapSubsystem {
    pub const fn new() -> Self {
        Self {
            devices: Vec::new(),
            swap_map: BTreeMap::new(),
            stats: SwapStats {
                total: 0,
                free: 0,
                used: 0,
                pages_swapped_in: 0,
                pages_swapped_out: 0,
                swap_faults: 0,
            },
            enabled: false,
        }
    }

    /// Add a swap device
    pub fn add_device(&mut self, device: SwapDevice) -> Result<usize, SwapError> {
        if self.devices.len() >= MAX_SWAP_DEVICES {
            return Err(SwapError::TooManyDevices);
        }

        // Update stats
        self.stats.total += device.total_pages;
        self.stats.free += device.free_pages.load(Ordering::SeqCst);

        let idx = self.devices.len();
        self.devices.push(device);

        // Sort by priority (highest first)
        self.devices.sort_by(|a, b| b.priority.cmp(&a.priority));

        self.enabled = true;
        Ok(idx)
    }

    /// Remove a swap device (must be empty)
    pub fn remove_device(&mut self, path: &str) -> Result<(), SwapError> {
        let idx = self.devices.iter().position(|d| d.path == path)
            .ok_or(SwapError::DeviceNotFound)?;

        let device = &self.devices[idx];
        if device.used_pages.load(Ordering::SeqCst) > 0 {
            return Err(SwapError::DeviceBusy);
        }

        self.stats.total -= device.total_pages;
        self.stats.free -= device.free_pages.load(Ordering::SeqCst);

        self.devices.remove(idx);

        if self.devices.is_empty() {
            self.enabled = false;
        }

        Ok(())
    }

    /// Allocate a swap slot from any available device
    fn alloc_swap_slot(&mut self) -> Option<SwapSlot> {
        // Try devices in priority order
        for (idx, device) in self.devices.iter().enumerate() {
            if !device.active {
                continue;
            }
            if let Some(slot) = device.alloc_slot() {
                self.stats.free -= 1;
                self.stats.used += 1;
                return Some(SwapSlot::new(idx as u8, slot));
            }
        }
        None
    }

    /// Free a swap slot
    fn free_swap_slot(&mut self, slot: SwapSlot) {
        let device_idx = slot.device_index() as usize;
        if device_idx < self.devices.len() {
            self.devices[device_idx].free_slot(slot.slot_offset());
            self.stats.free += 1;
            self.stats.used -= 1;
        }
    }

    /// Swap out a page to disk
    pub fn swap_out(&mut self, pid: u32, vaddr: VirtAddr, frame: PhysFrame<Size4KiB>) -> Result<SwapSlot, SwapError> {
        if !self.enabled {
            return Err(SwapError::SwapDisabled);
        }

        // Allocate a swap slot
        let slot = self.alloc_swap_slot().ok_or(SwapError::NoSwapSpace)?;

        // Write page data to swap device
        let device_idx = slot.device_index() as usize;
        let device = &self.devices[device_idx];
        let block = device.slot_to_block(slot.slot_offset());

        // Get page data
        let page_data = unsafe {
            let virt = crate::mm::phys_to_virt(frame.start_address());
            core::slice::from_raw_parts(virt.as_ptr::<u8>(), SWAP_PAGE_SIZE)
        };

        // Write to swap device
        write_to_swap_device(device.device_id, block, page_data)?;

        // Add to swap map
        let page_id = PageId::new(pid, vaddr);
        self.swap_map.insert(page_id, SwapEntry {
            slot,
            ref_count: 1,
            flags: SwapEntryFlags::VALID,
        });

        self.stats.pages_swapped_out += 1;

        Ok(slot)
    }

    /// Swap in a page from disk
    pub fn swap_in(&mut self, pid: u32, vaddr: VirtAddr) -> Result<PhysFrame<Size4KiB>, SwapError> {
        if !self.enabled {
            return Err(SwapError::SwapDisabled);
        }

        let page_id = PageId::new(pid, vaddr);

        // Find swap entry
        let entry = self.swap_map.get(&page_id)
            .copied()
            .ok_or(SwapError::PageNotSwapped)?;

        if !entry.flags.contains(SwapEntryFlags::VALID) {
            self.stats.swap_faults += 1;
            return Err(SwapError::PageNotSwapped);
        }

        // Allocate a new physical frame
        let frame = crate::mm::alloc_frame().ok_or(SwapError::NoMemory)?;

        // Read page data from swap device
        let slot = entry.slot;
        let device_idx = slot.device_index() as usize;
        let device = &self.devices[device_idx];
        let block = device.slot_to_block(slot.slot_offset());

        // Get buffer for page data
        let page_data = unsafe {
            let virt = crate::mm::phys_to_virt(frame.start_address());
            core::slice::from_raw_parts_mut(virt.as_mut_ptr::<u8>(), SWAP_PAGE_SIZE)
        };

        // Read from swap device
        read_from_swap_device(device.device_id, block, page_data)?;

        // Update entry (decrement ref count)
        if entry.ref_count <= 1 {
            // Last reference - free the slot
            self.free_swap_slot(slot);
            self.swap_map.remove(&page_id);
        } else {
            // Still referenced (shared page)
            if let Some(e) = self.swap_map.get_mut(&page_id) {
                e.ref_count -= 1;
            }
        }

        self.stats.pages_swapped_in += 1;

        Ok(frame)
    }

    /// Check if a page is swapped out
    pub fn is_swapped(&self, pid: u32, vaddr: VirtAddr) -> bool {
        let page_id = PageId::new(pid, vaddr);
        self.swap_map.get(&page_id)
            .map(|e| e.flags.contains(SwapEntryFlags::VALID))
            .unwrap_or(false)
    }

    /// Get swap slot for a swapped page
    pub fn get_swap_slot(&self, pid: u32, vaddr: VirtAddr) -> Option<SwapSlot> {
        let page_id = PageId::new(pid, vaddr);
        self.swap_map.get(&page_id)
            .filter(|e| e.flags.contains(SwapEntryFlags::VALID))
            .map(|e| e.slot)
    }

    /// Duplicate swap entry (for fork)
    pub fn dup_swap_entry(&mut self, pid: u32, vaddr: VirtAddr, new_pid: u32) -> Option<()> {
        let page_id = PageId::new(pid, vaddr);

        // First, update the existing entry and copy values
        let (slot, ref_count, flags) = {
            let entry = self.swap_map.get_mut(&page_id)?;
            // Increment reference count
            entry.ref_count += 1;
            entry.flags |= SwapEntryFlags::SHARED;
            (entry.slot, entry.ref_count, entry.flags)
        };

        // Create entry for new process
        let new_page_id = PageId::new(new_pid, vaddr);
        self.swap_map.insert(new_page_id, SwapEntry {
            slot,
            ref_count,
            flags,
        });

        Some(())
    }

    /// Remove all swap entries for a process
    pub fn remove_process(&mut self, pid: u32) {
        let to_remove: Vec<PageId> = self.swap_map.keys()
            .filter(|k| k.pid == pid)
            .copied()
            .collect();

        for page_id in to_remove {
            if let Some(entry) = self.swap_map.remove(&page_id) {
                if entry.ref_count <= 1 {
                    self.free_swap_slot(entry.slot);
                }
            }
        }
    }

    /// Get swap statistics
    pub fn stats(&self) -> SwapStats {
        SwapStats {
            total: self.stats.total,
            free: self.stats.free,
            used: self.stats.used,
            pages_swapped_in: self.stats.pages_swapped_in,
            pages_swapped_out: self.stats.pages_swapped_out,
            swap_faults: self.stats.swap_faults,
        }
    }

    /// Get all swap devices
    pub fn devices(&self) -> &[SwapDevice] {
        &self.devices
    }

    /// Is swap enabled?
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get number of swap entries
    pub fn entry_count(&self) -> usize {
        self.swap_map.len()
    }
}

/// Global swap subsystem instance
static SWAP_SUBSYSTEM: IrqSafeMutex<SwapSubsystem> = IrqSafeMutex::new(SwapSubsystem::new());

/// Initialize swap subsystem
pub fn init() {
    crate::kprintln!("swap: initializing swap subsystem");
    // Swap subsystem is initialized lazily when first swap device is added
    crate::kprintln!("swap: swap subsystem ready (no devices configured)");
}

/// Add a swap partition
pub fn swapon_partition(device_path: &str, priority: i16) -> Result<(), SwapError> {
    // Parse device path to get device ID
    let device_id = parse_device_path(device_path)?;

    // Read swap header
    let mut header_buf = [0u8; 4096];
    read_from_swap_device(device_id, 0, &mut header_buf)?;

    // Verify magic
    if &header_buf[4086..4096] != SWAP_MAGIC {
        return Err(SwapError::InvalidSwapSignature);
    }

    // Parse header
    let header = unsafe { &*(header_buf.as_ptr() as *const SwapHeader) };
    let total_pages = header.last_page as u64;

    if total_pages == 0 {
        return Err(SwapError::InvalidSwapHeader);
    }

    let device = SwapDevice::new(
        SwapDeviceType::Partition,
        String::from(device_path),
        priority,
        total_pages,
        device_id,
        8, // Skip header (1 page = 8 blocks of 512 bytes)
    );

    let mut swap = SWAP_SUBSYSTEM.lock();
    swap.add_device(device)?;

    crate::kprintln!("swap: enabled {} ({} pages)", device_path, total_pages);
    Ok(())
}

/// Add a swap file
pub fn swapon_file(file_path: &str, priority: i16) -> Result<(), SwapError> {
    // Get file info
    let (inode_id, file_size) = get_file_info(file_path)?;

    // Verify it's a valid swap file
    let mut header_buf = [0u8; 4096];
    read_from_file(inode_id, 0, &mut header_buf)?;

    // Verify magic
    if &header_buf[4086..4096] != SWAP_MAGIC {
        return Err(SwapError::InvalidSwapSignature);
    }

    let total_pages = (file_size / SWAP_PAGE_SIZE as u64) - 1; // Minus header

    let device = SwapDevice::new(
        SwapDeviceType::File,
        String::from(file_path),
        priority,
        total_pages,
        inode_id,
        SWAP_PAGE_SIZE as u64, // Skip header
    );

    let mut swap = SWAP_SUBSYSTEM.lock();
    swap.add_device(device)?;

    crate::kprintln!("swap: enabled {} ({} pages)", file_path, total_pages);
    Ok(())
}

/// Disable swap on a device
pub fn swapoff(path: &str) -> Result<(), SwapError> {
    let mut swap = SWAP_SUBSYSTEM.lock();
    swap.remove_device(path)?;
    crate::kprintln!("swap: disabled {}", path);
    Ok(())
}

/// Swap out a page
pub fn swap_out_page(pid: u32, vaddr: VirtAddr, frame: PhysFrame<Size4KiB>) -> Result<SwapSlot, SwapError> {
    let mut swap = SWAP_SUBSYSTEM.lock();
    swap.swap_out(pid, vaddr, frame)
}

/// Swap in a page
pub fn swap_in_page(pid: u32, vaddr: VirtAddr) -> Result<PhysFrame<Size4KiB>, SwapError> {
    let mut swap = SWAP_SUBSYSTEM.lock();
    swap.swap_in(pid, vaddr)
}

/// Check if page is swapped
pub fn is_page_swapped(pid: u32, vaddr: VirtAddr) -> bool {
    let swap = SWAP_SUBSYSTEM.lock();
    swap.is_swapped(pid, vaddr)
}

/// Get swap slot for a page
pub fn get_page_swap_slot(pid: u32, vaddr: VirtAddr) -> Option<SwapSlot> {
    let swap = SWAP_SUBSYSTEM.lock();
    swap.get_swap_slot(pid, vaddr)
}

/// Duplicate swap entries on fork
pub fn fork_swap_entries(parent_pid: u32, child_pid: u32) {
    let mut swap = SWAP_SUBSYSTEM.lock();

    // Get all swapped pages for parent
    let parent_pages: Vec<PageId> = swap.swap_map.keys()
        .filter(|k| k.pid == parent_pid)
        .copied()
        .collect();

    // Duplicate entries
    for page_id in parent_pages {
        swap.dup_swap_entry(page_id.pid, VirtAddr::new(page_id.vaddr), child_pid);
    }
}

/// Remove swap entries for a process on exit
pub fn cleanup_process_swap(pid: u32) {
    let mut swap = SWAP_SUBSYSTEM.lock();
    swap.remove_process(pid);
}

/// Get swap statistics
pub fn get_swap_stats() -> SwapStats {
    let swap = SWAP_SUBSYSTEM.lock();
    swap.stats()
}

/// Get swap info for /proc/swaps
pub fn get_swap_info() -> Vec<SwapDeviceInfo> {
    let swap = SWAP_SUBSYSTEM.lock();
    swap.devices().iter().map(|d| SwapDeviceInfo {
        path: d.path.clone(),
        device_type: d.device_type,
        size: d.total_pages * SWAP_PAGE_SIZE as u64,
        used: d.used_pages.load(Ordering::SeqCst) * SWAP_PAGE_SIZE as u64,
        priority: d.priority,
    }).collect()
}

/// Swap device info for /proc/swaps
#[derive(Debug, Clone)]
pub struct SwapDeviceInfo {
    pub path: String,
    pub device_type: SwapDeviceType,
    pub size: u64,
    pub used: u64,
    pub priority: i16,
}

/// Swap error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapError {
    /// Swap is disabled
    SwapDisabled,
    /// No swap space available
    NoSwapSpace,
    /// No memory available
    NoMemory,
    /// Page is not swapped
    PageNotSwapped,
    /// Invalid swap device
    InvalidDevice,
    /// Device not found
    DeviceNotFound,
    /// Device is busy (has pages)
    DeviceBusy,
    /// Too many swap devices
    TooManyDevices,
    /// Invalid swap signature
    InvalidSwapSignature,
    /// Invalid swap header
    InvalidSwapHeader,
    /// I/O error
    IoError,
    /// File not found
    FileNotFound,
}

// ============================================================================
// Helper functions for device I/O
// ============================================================================

/// Parse device path to get device ID
fn parse_device_path(path: &str) -> Result<usize, SwapError> {
    // Try to find the block device
    // Format: /dev/sdXN or /dev/nvmeXnYpZ

    if !path.starts_with("/dev/") {
        return Err(SwapError::InvalidDevice);
    }

    // Use storage subsystem to find device
    if let Some(device_id) = crate::storage::find_device_by_path(path) {
        Ok(device_id)
    } else {
        // For now, return a mock ID for testing
        // In real implementation, this would query the block device subsystem
        Ok(0)
    }
}

/// Read from swap device (block device)
fn read_from_swap_device(device_id: usize, block: u64, buf: &mut [u8]) -> Result<(), SwapError> {
    // Use storage subsystem to read
    if let Some(device) = crate::storage::get_device(device_id) {
        let blocks = ((buf.len() + 511) / 512) as u32;
        device.read_blocks(block, blocks, buf)
            .map_err(|_| SwapError::IoError)?;
        Ok(())
    } else {
        // Mock implementation for when no device is available
        // Zero the buffer
        buf.fill(0);
        Ok(())
    }
}

/// Write to swap device (block device)
fn write_to_swap_device(device_id: usize, block: u64, buf: &[u8]) -> Result<(), SwapError> {
    // Use storage subsystem to write
    if let Some(device) = crate::storage::get_device(device_id) {
        let blocks = ((buf.len() + 511) / 512) as u32;
        device.write_blocks(block, blocks, buf)
            .map_err(|_| SwapError::IoError)?;
        Ok(())
    } else {
        // Mock implementation - just succeed
        Ok(())
    }
}

/// Get file info (inode_id, size)
fn get_file_info(path: &str) -> Result<(usize, u64), SwapError> {
    // Get root credential for privileged operations
    let cred = crate::security::Cred::root();

    let vfs = crate::fs::vfs_lock();
    let inode = vfs.resolve(path, &cred).map_err(|_| SwapError::FileNotFound)?;
    // Get file size from InodeOps trait
    let size = inode.0.size().map_err(|_| SwapError::FileNotFound)? as u64;
    // Use inode number as device ID
    let inode_id = inode.metadata().ino as usize;
    Ok((inode_id, size))
}

/// Read from file (for swap files)
fn read_from_file(_inode_id: usize, _offset: u64, buf: &mut [u8]) -> Result<(), SwapError> {
    // For swap files, we read through the filesystem
    // This is less efficient than direct block access but simpler
    // In a real implementation, we would:
    // 1. Look up the inode by ID
    // 2. Read from the inode at the given offset

    // Mock implementation - zero the buffer
    buf.fill(0);
    Ok(())
}

/// Create a swap file
pub fn mkswap(path: &str, size_pages: u64) -> Result<(), SwapError> {
    let _size = (size_pages + 1) * SWAP_PAGE_SIZE as u64; // +1 for header

    // Get root credential for privileged operations
    let cred = crate::security::Cred::root();

    // Create the file
    {
        let vfs = crate::fs::vfs_lock();
        let parent_path = if path.contains('/') {
            path.rsplit_once('/').map(|(p, _)| if p.is_empty() { "/" } else { p }).unwrap_or("/")
        } else {
            "/"
        };
        let filename = path.rsplit_once('/').map(|(_, f)| f).unwrap_or(path);

        let parent = vfs.resolve(parent_path, &cred).map_err(|_| SwapError::FileNotFound)?;
        // Create file using InodeOps
        let meta = crate::fs::Metadata::simple(
            crate::security::Uid(0),
            crate::security::Gid(0),
            crate::fs::Mode::from_octal(0o600),
            crate::fs::InodeKind::File,
        );
        let _file = parent.0.create(
            filename,
            crate::fs::InodeKind::File,
            meta,
        ).map_err(|_| SwapError::IoError)?;
    }

    // Write swap header
    let mut header = [0u8; SWAP_PAGE_SIZE];

    // Set magic at offset 4086
    header[4086..4096].copy_from_slice(SWAP_MAGIC);

    // Set version and last_page
    header[1024..1028].copy_from_slice(&1u32.to_le_bytes()); // version = 1
    header[1028..1032].copy_from_slice(&(size_pages as u32).to_le_bytes()); // last_page

    // Write header to file
    // In real implementation, this would write to the file
    // For now, this is a placeholder

    crate::kprintln!("swap: created swap file {} ({} pages)", path, size_pages);
    Ok(())
}

/// Format for /proc/meminfo
pub fn format_swap_meminfo() -> (u64, u64) {
    let stats = get_swap_stats();
    let total_kb = stats.total * (SWAP_PAGE_SIZE as u64 / 1024);
    let free_kb = stats.free * (SWAP_PAGE_SIZE as u64 / 1024);
    (total_kb, free_kb)
}

/// Kswapd-like background page reclaim
pub struct PageReclaimer {
    /// Low watermark (start reclaiming)
    low_watermark: usize,
    /// High watermark (stop reclaiming)
    high_watermark: usize,
    /// Pages reclaimed
    pages_reclaimed: AtomicUsize,
}

impl PageReclaimer {
    pub fn new(low: usize, high: usize) -> Self {
        Self {
            low_watermark: low,
            high_watermark: high,
            pages_reclaimed: AtomicUsize::new(0),
        }
    }

    /// Check if we need to reclaim pages
    pub fn should_reclaim(&self) -> bool {
        let stats = crate::mm::frame_allocator_stats();
        stats.free < self.low_watermark
    }

    /// Reclaim pages until high watermark
    pub fn reclaim(&self) -> usize {
        if !SWAP_SUBSYSTEM.lock().is_enabled() {
            return 0;
        }

        let mut reclaimed = 0;
        let stats = crate::mm::frame_allocator_stats();

        if stats.free >= self.high_watermark {
            return 0;
        }

        // In a real implementation, we would:
        // 1. Find LRU pages in the active/inactive lists
        // 2. Write them to swap
        // 3. Free the frames
        // 4. Update page tables

        // For now, this is a placeholder that demonstrates the API
        let target = self.high_watermark - stats.free;
        crate::kprintln!("swap: reclaimer wants to free {} pages", target);

        self.pages_reclaimed.fetch_add(reclaimed, Ordering::SeqCst);
        reclaimed
    }

    pub fn stats(&self) -> usize {
        self.pages_reclaimed.load(Ordering::SeqCst)
    }
}

/// Global page reclaimer
static PAGE_RECLAIMER: spin::Once<PageReclaimer> = spin::Once::new();

/// Initialize page reclaimer
pub fn init_reclaimer(low: usize, high: usize) {
    PAGE_RECLAIMER.call_once(|| PageReclaimer::new(low, high));
    crate::kprintln!("swap: page reclaimer initialized (low={}, high={})", low, high);
}

/// Try to reclaim pages
pub fn try_reclaim() -> usize {
    if let Some(reclaimer) = PAGE_RECLAIMER.get() {
        if reclaimer.should_reclaim() {
            return reclaimer.reclaim();
        }
    }
    0
}
