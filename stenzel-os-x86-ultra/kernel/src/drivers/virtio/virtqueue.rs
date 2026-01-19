//! VirtIO Virtqueue Implementation
//!
//! Split and packed virtqueue support.

#![allow(dead_code)]

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU16, Ordering};

/// Virtqueue descriptor flags
pub mod desc_flags {
    pub const VIRTQ_DESC_F_NEXT: u16 = 1;
    pub const VIRTQ_DESC_F_WRITE: u16 = 2;
    pub const VIRTQ_DESC_F_INDIRECT: u16 = 4;
}

/// Virtqueue descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VirtqDesc {
    /// Physical address of buffer
    pub addr: u64,
    /// Length of buffer
    pub len: u32,
    /// Flags
    pub flags: u16,
    /// Next descriptor index (if NEXT flag set)
    pub next: u16,
}

/// Virtqueue available ring
#[repr(C)]
#[derive(Debug)]
pub struct VirtqAvail {
    pub flags: u16,
    pub idx: u16,
    pub ring: [u16; 0], // Variable length
}

/// Virtqueue used element
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VirtqUsedElem {
    pub id: u32,
    pub len: u32,
}

/// Virtqueue used ring
#[repr(C)]
#[derive(Debug)]
pub struct VirtqUsed {
    pub flags: u16,
    pub idx: u16,
    pub ring: [VirtqUsedElem; 0], // Variable length
}

/// Virtqueue configuration
#[derive(Debug, Clone)]
pub struct VirtqueueConfig {
    pub queue_size: u16,
    pub desc_addr: u64,
    pub avail_addr: u64,
    pub used_addr: u64,
}

/// Virtqueue (split ring)
pub struct Virtqueue {
    /// Queue index
    pub index: u16,
    /// Queue size (number of descriptors)
    pub size: u16,
    /// Descriptor table
    descriptors: Vec<VirtqDesc>,
    /// Available ring indices
    avail_ring: Vec<u16>,
    /// Available ring index
    avail_idx: AtomicU16,
    /// Used ring elements
    used_ring: Vec<VirtqUsedElem>,
    /// Last seen used index
    last_used_idx: u16,
    /// Free descriptor list head
    free_head: u16,
    /// Number of free descriptors
    num_free: u16,
    /// Event notification enabled
    notify_enabled: bool,
}

impl Virtqueue {
    /// Create new virtqueue
    pub fn new(index: u16, size: u16) -> Self {
        let mut descriptors = Vec::with_capacity(size as usize);
        let mut avail_ring = Vec::with_capacity(size as usize);
        let mut used_ring = Vec::with_capacity(size as usize);

        // Initialize descriptor chain
        for i in 0..size {
            descriptors.push(VirtqDesc {
                addr: 0,
                len: 0,
                flags: 0,
                next: if i + 1 < size { i + 1 } else { 0 },
            });
            avail_ring.push(0);
            used_ring.push(VirtqUsedElem::default());
        }

        Self {
            index,
            size,
            descriptors,
            avail_ring,
            avail_idx: AtomicU16::new(0),
            used_ring,
            last_used_idx: 0,
            free_head: 0,
            num_free: size,
            notify_enabled: true,
        }
    }

    /// Add buffer to queue
    pub fn add_buffer(&mut self, addr: u64, len: u32, writable: bool) -> Option<u16> {
        if self.num_free == 0 {
            return None;
        }

        let desc_idx = self.free_head;
        let desc = &mut self.descriptors[desc_idx as usize];

        desc.addr = addr;
        desc.len = len;
        desc.flags = if writable { desc_flags::VIRTQ_DESC_F_WRITE } else { 0 };

        self.free_head = desc.next;
        self.num_free -= 1;

        // Add to available ring
        let avail_idx = self.avail_idx.load(Ordering::Relaxed);
        self.avail_ring[(avail_idx % self.size) as usize] = desc_idx;
        self.avail_idx.store(avail_idx.wrapping_add(1), Ordering::Release);

        Some(desc_idx)
    }

    /// Add scatter-gather buffer chain
    pub fn add_chain(&mut self, buffers: &[(u64, u32, bool)]) -> Option<u16> {
        if buffers.is_empty() || self.num_free < buffers.len() as u16 {
            return None;
        }

        let head = self.free_head;
        let mut prev_idx = head;

        for (i, &(addr, len, writable)) in buffers.iter().enumerate() {
            let desc_idx = if i == 0 { head } else { self.descriptors[prev_idx as usize].next };
            let desc = &mut self.descriptors[desc_idx as usize];

            desc.addr = addr;
            desc.len = len;
            desc.flags = if writable { desc_flags::VIRTQ_DESC_F_WRITE } else { 0 };

            if i + 1 < buffers.len() {
                desc.flags |= desc_flags::VIRTQ_DESC_F_NEXT;
            }

            prev_idx = desc_idx;
        }

        self.free_head = self.descriptors[prev_idx as usize].next;
        self.num_free -= buffers.len() as u16;

        // Add to available ring
        let avail_idx = self.avail_idx.load(Ordering::Relaxed);
        self.avail_ring[(avail_idx % self.size) as usize] = head;
        self.avail_idx.store(avail_idx.wrapping_add(1), Ordering::Release);

        Some(head)
    }

    /// Get used buffer
    pub fn get_used(&mut self) -> Option<(u16, u32)> {
        // Check if there are used buffers
        let used_idx = self.used_ring.len() as u16; // Placeholder
        if self.last_used_idx == used_idx {
            return None;
        }

        let elem = &self.used_ring[(self.last_used_idx % self.size) as usize];
        let id = elem.id as u16;
        let len = elem.len;

        self.last_used_idx = self.last_used_idx.wrapping_add(1);

        // Return descriptor to free list
        self.return_descriptor(id);

        Some((id, len))
    }

    /// Return descriptor to free list
    fn return_descriptor(&mut self, mut idx: u16) {
        loop {
            let desc = &self.descriptors[idx as usize];
            let has_next = desc.flags & desc_flags::VIRTQ_DESC_F_NEXT != 0;
            let next = desc.next;

            self.descriptors[idx as usize].next = self.free_head;
            self.free_head = idx;
            self.num_free += 1;

            if !has_next {
                break;
            }
            idx = next;
        }
    }

    /// Check if queue needs notification
    pub fn needs_notify(&self) -> bool {
        self.notify_enabled
    }

    /// Enable/disable notifications
    pub fn set_notify(&mut self, enabled: bool) {
        self.notify_enabled = enabled;
    }

    /// Get number of free descriptors
    pub fn num_free(&self) -> u16 {
        self.num_free
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.num_free == self.size
    }

    /// Check if queue is full
    pub fn is_full(&self) -> bool {
        self.num_free == 0
    }
}
