//! VMware Memory Balloon Driver
//!
//! Allows VMware to reclaim memory from the guest.

#![allow(dead_code)]

use alloc::vec::Vec;
use alloc::string::String;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Balloon backdoor commands
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum BalloonCmd {
    Start = 0,
    GetTarget = 1,
    Lock = 2,
    Unlock = 3,
    GetStats = 4,
    GetMask = 5,
}

/// Balloon capabilities
#[derive(Debug, Clone, Copy, Default)]
pub struct BalloonCaps {
    pub basic: bool,
    pub stats: bool,
    pub batched: bool,
    pub huge_pages: bool,
}

/// Balloon state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalloonState {
    Stopped,
    Running,
    Inflating,
    Deflating,
}

/// Balloon statistics
#[derive(Debug, Default)]
pub struct BalloonStats {
    pub current_pages: AtomicU64,
    pub target_pages: AtomicU64,
    pub locked_pages: AtomicU64,
    pub inflate_count: AtomicU64,
    pub deflate_count: AtomicU64,
    pub lock_failures: AtomicU64,
}

/// Locked page
struct LockedPage {
    pfn: u64,
    locked: bool,
}

/// VMware balloon device
pub struct VmwareBalloon {
    /// Balloon state
    state: BalloonState,
    /// Capabilities
    caps: BalloonCaps,
    /// Current balloon size (pages)
    current_pages: u64,
    /// Target balloon size (pages)
    target_pages: u64,
    /// Max pages
    max_pages: u64,
    /// Locked pages
    locked_pages: Vec<LockedPage>,
    /// Statistics
    stats: BalloonStats,
    /// Initialized
    initialized: AtomicBool,
    /// Rate limiting
    rate_alloc: u32,
    rate_free: u32,
}

impl VmwareBalloon {
    /// Page size
    const PAGE_SIZE: u64 = 4096;

    /// Create new balloon
    pub fn new() -> Self {
        Self {
            state: BalloonState::Stopped,
            caps: BalloonCaps::default(),
            current_pages: 0,
            target_pages: 0,
            max_pages: 0,
            locked_pages: Vec::new(),
            stats: BalloonStats::default(),
            initialized: AtomicBool::new(false),
            rate_alloc: 256,
            rate_free: 256,
        }
    }

    /// Execute balloon backdoor
    fn backdoor(&self, cmd: BalloonCmd, arg: u32) -> Option<(u32, u32)> {
        const VMWARE_MAGIC: u32 = 0x564D5868;
        const VMWARE_BDOOR_PORT: u16 = 0x5658;
        const BALLOON_PROTOCOL_VERSION: u32 = 6;

        let mut eax: u32 = VMWARE_MAGIC;
        let mut ebx: u32 = arg | (BALLOON_PROTOCOL_VERSION << 16);
        let ecx: u32 = cmd as u32 | (0x4F << 16); // Balloon command

        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!(
                "push rbx",
                "mov eax, {magic:e}",
                "mov ebx, {arg:e}",
                "mov ecx, {cmd:e}",
                "mov dx, {port:x}",
                "in eax, dx",
                "mov {eax_out:e}, eax",
                "mov {ebx_out:e}, ebx",
                "pop rbx",
                magic = in(reg) VMWARE_MAGIC,
                arg = in(reg) ebx,
                cmd = in(reg) ecx,
                port = in(reg) VMWARE_BDOOR_PORT,
                eax_out = out(reg) eax,
                ebx_out = out(reg) ebx,
                options(nostack, nomem)
            );
        }

        if ebx == VMWARE_MAGIC {
            Some((eax, ebx))
        } else {
            None
        }
    }

    /// Initialize balloon
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Start balloon
        if self.backdoor(BalloonCmd::Start, 0).is_none() {
            return Err("Failed to start balloon");
        }

        // Get capabilities
        if let Some((caps, _)) = self.backdoor(BalloonCmd::GetMask, 0) {
            self.caps.basic = true;
            self.caps.stats = caps & 0x1 != 0;
            self.caps.batched = caps & 0x2 != 0;
            self.caps.huge_pages = caps & 0x4 != 0;
        }

        // Get initial target
        if let Some((target, _)) = self.backdoor(BalloonCmd::GetTarget, 0) {
            self.target_pages = target as u64;
            self.stats.target_pages.store(target as u64, Ordering::Relaxed);
        }

        self.state = BalloonState::Running;
        self.initialized.store(true, Ordering::Release);

        crate::kprintln!("vmware-balloon: Initialized, target={} pages", self.target_pages);
        Ok(())
    }

    /// Get target from host
    pub fn update_target(&mut self) {
        if let Some((target, _)) = self.backdoor(BalloonCmd::GetTarget, 0) {
            self.target_pages = target as u64;
            self.stats.target_pages.store(target as u64, Ordering::Relaxed);
        }
    }

    /// Current balloon size in pages
    pub fn current_pages(&self) -> u64 {
        self.current_pages
    }

    /// Target balloon size in pages
    pub fn target_pages(&self) -> u64 {
        self.target_pages
    }

    /// Current balloon size in bytes
    pub fn current_bytes(&self) -> u64 {
        self.current_pages * Self::PAGE_SIZE
    }

    /// Target balloon size in bytes
    pub fn target_bytes(&self) -> u64 {
        self.target_pages * Self::PAGE_SIZE
    }

    /// Inflate balloon (give memory to host)
    pub fn inflate(&mut self, pages: usize) -> usize {
        if !self.initialized.load(Ordering::Acquire) {
            return 0;
        }

        self.state = BalloonState::Inflating;
        let mut inflated = 0;

        for _ in 0..pages.min(self.rate_alloc as usize) {
            // Allocate page
            if let Some(frame) = crate::mm::alloc_frame() {
                let pfn = frame.start_address().as_u64() / Self::PAGE_SIZE;

                // Lock page with VMware
                if let Some((result, _)) = self.backdoor(BalloonCmd::Lock, pfn as u32) {
                    if result == 0 {
                        self.locked_pages.push(LockedPage { pfn, locked: true });
                        self.current_pages += 1;
                        inflated += 1;
                    } else {
                        // Failed to lock, free the page
                        crate::mm::free_frame(frame);
                        self.stats.lock_failures.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }

        self.stats.current_pages.store(self.current_pages, Ordering::Relaxed);
        self.stats.locked_pages.store(self.locked_pages.len() as u64, Ordering::Relaxed);
        self.stats.inflate_count.fetch_add(1, Ordering::Relaxed);

        self.state = BalloonState::Running;
        inflated
    }

    /// Deflate balloon (reclaim memory from host)
    pub fn deflate(&mut self, pages: usize) -> usize {
        if !self.initialized.load(Ordering::Acquire) {
            return 0;
        }

        self.state = BalloonState::Deflating;
        let mut deflated = 0;

        for _ in 0..pages.min(self.rate_free as usize) {
            if let Some(page) = self.locked_pages.pop() {
                // Unlock page with VMware
                let _ = self.backdoor(BalloonCmd::Unlock, page.pfn as u32);

                // Free the page back
                let addr = x86_64::PhysAddr::new(page.pfn * Self::PAGE_SIZE);
                let frame = x86_64::structures::paging::PhysFrame::containing_address(addr);
                crate::mm::free_frame(frame);

                self.current_pages -= 1;
                deflated += 1;
            } else {
                break;
            }
        }

        self.stats.current_pages.store(self.current_pages, Ordering::Relaxed);
        self.stats.locked_pages.store(self.locked_pages.len() as u64, Ordering::Relaxed);
        self.stats.deflate_count.fetch_add(1, Ordering::Relaxed);

        self.state = BalloonState::Running;
        deflated
    }

    /// Update balloon to match target
    pub fn update(&mut self) {
        self.update_target();

        if self.current_pages < self.target_pages {
            // Need to inflate
            let diff = (self.target_pages - self.current_pages) as usize;
            self.inflate(diff.min(256));
        } else if self.current_pages > self.target_pages {
            // Need to deflate
            let diff = (self.current_pages - self.target_pages) as usize;
            self.deflate(diff.min(256));
        }
    }

    /// Get statistics
    pub fn stats(&self) -> &BalloonStats {
        &self.stats
    }

    /// Get state
    pub fn state(&self) -> BalloonState {
        self.state
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!(
            "VMware Balloon: {}MB / {}MB target, state={:?}",
            self.current_bytes() / (1024 * 1024),
            self.target_bytes() / (1024 * 1024),
            self.state
        )
    }
}

impl Default for VmwareBalloon {
    fn default() -> Self {
        Self::new()
    }
}
