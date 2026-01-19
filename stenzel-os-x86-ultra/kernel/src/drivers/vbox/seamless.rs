//! VirtualBox Seamless Windows Mode
//!
//! Seamless integration for guest windows in host desktop.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::{VmmDevRequestHeader, VmmDevRequestType};

/// Seamless mode state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeamlessMode {
    Off,
    Visible,
    FullScreen,
}

/// Rectangle for visible region
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SeamlessRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl SeamlessRect {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.width as i32 &&
        py >= self.y && py < self.y + self.height as i32
    }

    pub fn intersects(&self, other: &SeamlessRect) -> bool {
        self.x < other.x + other.width as i32 &&
        self.x + self.width as i32 > other.x &&
        self.y < other.y + other.height as i32 &&
        self.y + self.height as i32 > other.y
    }
}

/// Window info for seamless mode
#[derive(Debug, Clone)]
pub struct SeamlessWindow {
    pub id: u64,
    pub rect: SeamlessRect,
    pub title: String,
    pub visible: bool,
    pub shaped: bool,
    pub shape_rects: Vec<SeamlessRect>,
}

impl SeamlessWindow {
    pub fn new(id: u64, rect: SeamlessRect, title: String) -> Self {
        Self {
            id,
            rect,
            title,
            visible: true,
            shaped: false,
            shape_rects: Vec::new(),
        }
    }

    pub fn set_shape(&mut self, rects: Vec<SeamlessRect>) {
        self.shape_rects = rects;
        self.shaped = !self.shape_rects.is_empty();
    }
}

/// Get seamless change request
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VmmDevSeamlessChangeRequest {
    pub header: VmmDevRequestHeader,
    pub flags: u32,
    pub event_ack: u32,
}

/// Set visible region request
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VmmDevVideoSetVisibleRegion {
    pub header: VmmDevRequestHeader,
    pub rect_count: u32,
    // Followed by SeamlessRect array
}

/// Seamless statistics
#[derive(Debug, Default)]
pub struct SeamlessStats {
    pub mode_changes: AtomicU64,
    pub region_updates: AtomicU64,
    pub windows_added: AtomicU64,
    pub windows_removed: AtomicU64,
}

/// Seamless service
pub struct SeamlessService {
    /// Current mode
    mode: SeamlessMode,
    /// Host requested seamless
    host_seamless: bool,
    /// Tracked windows
    windows: Vec<SeamlessWindow>,
    /// Next window ID
    next_window_id: u64,
    /// Region needs update
    region_dirty: AtomicBool,
    /// VMMDev MMIO base
    mmio_base: u64,
    /// Initialized flag
    initialized: AtomicBool,
    /// Statistics
    stats: SeamlessStats,
}

impl SeamlessService {
    /// Create new service
    pub fn new(mmio_base: u64) -> Self {
        Self {
            mode: SeamlessMode::Off,
            host_seamless: false,
            windows: Vec::new(),
            next_window_id: 1,
            region_dirty: AtomicBool::new(false),
            mmio_base,
            initialized: AtomicBool::new(false),
            stats: SeamlessStats::default(),
        }
    }

    /// Initialize service
    pub fn init(&mut self) -> Result<(), &'static str> {
        self.initialized.store(true, Ordering::Release);
        crate::kprintln!("vbox-seamless: Initialized");
        Ok(())
    }

    /// Check for seamless mode change from host
    pub fn check_mode_change(&mut self) -> Option<SeamlessMode> {
        if !self.initialized.load(Ordering::Acquire) || self.mmio_base == 0 {
            return None;
        }

        let request = VmmDevSeamlessChangeRequest {
            header: VmmDevRequestHeader::new(
                VmmDevRequestType::GetSeamlessChangeRequest,
                core::mem::size_of::<VmmDevSeamlessChangeRequest>() as u32
            ),
            flags: 0,
            event_ack: 1,
        };

        // Send request
        unsafe {
            let dst = self.mmio_base as *mut VmmDevSeamlessChangeRequest;
            core::ptr::write_volatile(dst, request);
        }

        // Read response
        let response: VmmDevSeamlessChangeRequest = unsafe {
            core::ptr::read_volatile(self.mmio_base as *const VmmDevSeamlessChangeRequest)
        };

        if response.header.rc == 0 {
            let new_mode = match response.flags {
                0 => SeamlessMode::Off,
                1 => SeamlessMode::Visible,
                2 => SeamlessMode::FullScreen,
                _ => return None,
            };

            if new_mode != self.mode {
                let old_mode = self.mode;
                self.mode = new_mode;
                self.host_seamless = new_mode != SeamlessMode::Off;
                self.stats.mode_changes.fetch_add(1, Ordering::Relaxed);

                crate::kprintln!("vbox-seamless: Mode changed {:?} -> {:?}", old_mode, new_mode);
                return Some(new_mode);
            }
        }

        None
    }

    /// Get current mode
    pub fn mode(&self) -> SeamlessMode {
        self.mode
    }

    /// Is seamless mode active?
    pub fn is_active(&self) -> bool {
        self.mode != SeamlessMode::Off && self.host_seamless
    }

    /// Add window to tracking
    pub fn add_window(&mut self, rect: SeamlessRect, title: String) -> u64 {
        let id = self.next_window_id;
        self.next_window_id += 1;

        let window = SeamlessWindow::new(id, rect, title);
        self.windows.push(window);

        self.region_dirty.store(true, Ordering::Release);
        self.stats.windows_added.fetch_add(1, Ordering::Relaxed);

        id
    }

    /// Remove window from tracking
    pub fn remove_window(&mut self, id: u64) {
        if let Some(idx) = self.windows.iter().position(|w| w.id == id) {
            self.windows.remove(idx);
            self.region_dirty.store(true, Ordering::Release);
            self.stats.windows_removed.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Update window position/size
    pub fn update_window(&mut self, id: u64, rect: SeamlessRect) {
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            window.rect = rect;
            self.region_dirty.store(true, Ordering::Release);
        }
    }

    /// Set window visibility
    pub fn set_window_visible(&mut self, id: u64, visible: bool) {
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            if window.visible != visible {
                window.visible = visible;
                self.region_dirty.store(true, Ordering::Release);
            }
        }
    }

    /// Set window shape (non-rectangular window)
    pub fn set_window_shape(&mut self, id: u64, rects: Vec<SeamlessRect>) {
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            window.set_shape(rects);
            self.region_dirty.store(true, Ordering::Release);
        }
    }

    /// Get window by ID
    pub fn get_window(&self, id: u64) -> Option<&SeamlessWindow> {
        self.windows.iter().find(|w| w.id == id)
    }

    /// Get all windows
    pub fn windows(&self) -> &[SeamlessWindow] {
        &self.windows
    }

    /// Update visible region to host
    pub fn update_visible_region(&mut self) -> Result<(), &'static str> {
        if !self.initialized.load(Ordering::Acquire) || self.mmio_base == 0 {
            return Err("Service not initialized");
        }

        if !self.is_active() {
            return Ok(());
        }

        if !self.region_dirty.swap(false, Ordering::AcqRel) {
            return Ok(()); // No update needed
        }

        // Collect visible rectangles
        let rects = self.collect_visible_rects();

        if rects.is_empty() {
            return Ok(());
        }

        // In real implementation, allocate buffer for request + rects
        // and send to VMMDev

        self.stats.region_updates.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Collect all visible rectangles
    fn collect_visible_rects(&self) -> Vec<SeamlessRect> {
        let mut rects = Vec::new();

        for window in &self.windows {
            if !window.visible {
                continue;
            }

            if window.shaped {
                // Use shape rectangles
                for shape_rect in &window.shape_rects {
                    rects.push(SeamlessRect {
                        x: window.rect.x + shape_rect.x,
                        y: window.rect.y + shape_rect.y,
                        width: shape_rect.width,
                        height: shape_rect.height,
                    });
                }
            } else {
                // Use window rectangle
                rects.push(window.rect);
            }
        }

        rects
    }

    /// Request to enter seamless mode
    pub fn request_seamless(&mut self, enable: bool) {
        if enable && self.mode == SeamlessMode::Off {
            // Request seamless mode from host
            // In real implementation, send capability update
        } else if !enable && self.mode != SeamlessMode::Off {
            // Request to exit seamless mode
        }
    }

    /// Get statistics
    pub fn stats(&self) -> &SeamlessStats {
        &self.stats
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!(
            "VBox Seamless: mode={:?} windows={} updates={}",
            self.mode, self.windows.len(),
            self.stats.region_updates.load(Ordering::Relaxed)
        )
    }
}

impl Default for SeamlessService {
    fn default() -> Self {
        Self::new(0)
    }
}

// Global seamless service
static SEAMLESS: crate::sync::IrqSafeMutex<Option<SeamlessService>> =
    crate::sync::IrqSafeMutex::new(None);

/// Initialize seamless service
pub fn init(mmio_base: u64) -> Result<(), &'static str> {
    let mut service = SeamlessService::new(mmio_base);
    service.init()?;
    *SEAMLESS.lock() = Some(service);
    Ok(())
}

/// Check for mode change
pub fn check_mode_change() -> Option<SeamlessMode> {
    SEAMLESS.lock()
        .as_mut()
        .and_then(|s| s.check_mode_change())
}

/// Is seamless active?
pub fn is_active() -> bool {
    SEAMLESS.lock()
        .as_ref()
        .map(|s| s.is_active())
        .unwrap_or(false)
}

/// Get status
pub fn status() -> String {
    SEAMLESS.lock()
        .as_ref()
        .map(|s| s.format_status())
        .unwrap_or_else(|| "Seamless not initialized".into())
}
