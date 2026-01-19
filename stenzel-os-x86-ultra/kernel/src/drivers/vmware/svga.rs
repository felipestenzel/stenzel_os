//! VMware SVGA II Graphics Driver
//!
//! Paravirtualized graphics adapter for VMware.

#![allow(dead_code)]

use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::vec;
use alloc::string::String;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// SVGA PCI device IDs
pub const SVGA_VENDOR_ID: u16 = 0x15AD;
pub const SVGA_DEVICE_ID: u16 = 0x0405;

/// SVGA I/O port offsets
mod ports {
    pub const INDEX: u32 = 0;
    pub const VALUE: u32 = 1;
    pub const BIOS: u32 = 2;
    pub const IRQ: u32 = 8;
}

/// SVGA registers
mod regs {
    pub const ID: u32 = 0;
    pub const ENABLE: u32 = 1;
    pub const WIDTH: u32 = 2;
    pub const HEIGHT: u32 = 3;
    pub const MAX_WIDTH: u32 = 4;
    pub const MAX_HEIGHT: u32 = 5;
    pub const DEPTH: u32 = 6;
    pub const BITS_PER_PIXEL: u32 = 7;
    pub const PSEUDOCOLOR: u32 = 8;
    pub const RED_MASK: u32 = 9;
    pub const GREEN_MASK: u32 = 10;
    pub const BLUE_MASK: u32 = 11;
    pub const BYTES_PER_LINE: u32 = 12;
    pub const FB_START: u32 = 13;
    pub const FB_OFFSET: u32 = 14;
    pub const VRAM_SIZE: u32 = 15;
    pub const FB_SIZE: u32 = 16;
    pub const CAPABILITIES: u32 = 17;
    pub const FIFO_START: u32 = 18;
    pub const FIFO_SIZE: u32 = 19;
    pub const CONFIG_DONE: u32 = 20;
    pub const SYNC: u32 = 21;
    pub const BUSY: u32 = 22;
    pub const GUEST_ID: u32 = 23;
    pub const CURSOR_ID: u32 = 24;
    pub const CURSOR_X: u32 = 25;
    pub const CURSOR_Y: u32 = 26;
    pub const CURSOR_ON: u32 = 27;
    pub const SCRATCH_SIZE: u32 = 28;
    pub const MEM_REGS: u32 = 29;
    pub const NUM_DISPLAYS: u32 = 30;
}

/// SVGA capabilities
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum SvgaCap {
    None = 0,
    RectCopy = 1 << 0,
    Cursor = 1 << 5,
    CursorBypass = 1 << 6,
    CursorBypass2 = 1 << 7,
    Multimon = 1 << 22,
    Pitchlock = 1 << 23,
    IrqMask = 1 << 24,
    DisplayTopology = 1 << 25,
    GmrFb = 1 << 26,
    Traces = 1 << 27,
    Gmr2 = 1 << 28,
    ScreenObject2 = 1 << 29,
}

/// FIFO registers
mod fifo {
    pub const MIN: u32 = 0;
    pub const MAX: u32 = 1;
    pub const NEXT_CMD: u32 = 2;
    pub const STOP: u32 = 3;
}

/// FIFO commands
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum FifoCmd {
    Update = 1,
    RectCopy = 3,
    DefineCursor = 19,
    DefineAlphaCursor = 22,
    UpdateVerbose = 25,
    FrontRopFill = 29,
}

/// Display mode
#[derive(Debug, Clone, Copy)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub bpp: u32,
}

/// Cursor definition
#[derive(Debug, Clone)]
pub struct CursorDef {
    pub id: u32,
    pub hotspot_x: u32,
    pub hotspot_y: u32,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u32>,
}

/// SVGA device
pub struct SvgaDevice {
    /// I/O port base
    io_base: u16,
    /// MMIO FIFO base
    fifo_base: u64,
    /// Framebuffer base
    fb_base: u64,
    /// Current mode
    mode: DisplayMode,
    /// Max mode
    max_mode: DisplayMode,
    /// Capabilities
    caps: u32,
    /// VRAM size
    vram_size: u32,
    /// FIFO size
    fifo_size: u32,
    /// Bytes per line
    bytes_per_line: u32,
    /// Initialized
    initialized: AtomicBool,
    /// Cursor visible
    cursor_visible: bool,
    /// Cursor position
    cursor_x: AtomicU32,
    cursor_y: AtomicU32,
}

impl SvgaDevice {
    /// SVGA ID
    const SVGA_ID_2: u32 = 0x900000 | (2 << 8);

    /// Create new device
    pub fn new(io_base: u16, fifo_base: u64, fb_base: u64) -> Self {
        Self {
            io_base,
            fifo_base,
            fb_base,
            mode: DisplayMode { width: 0, height: 0, bpp: 0 },
            max_mode: DisplayMode { width: 0, height: 0, bpp: 0 },
            caps: 0,
            vram_size: 0,
            fifo_size: 0,
            bytes_per_line: 0,
            initialized: AtomicBool::new(false),
            cursor_visible: false,
            cursor_x: AtomicU32::new(0),
            cursor_y: AtomicU32::new(0),
        }
    }

    /// Read SVGA register
    fn read_reg(&self, index: u32) -> u32 {
        unsafe {
            let index_port = self.io_base + ports::INDEX as u16;
            let value_port = self.io_base + ports::VALUE as u16;

            core::arch::asm!(
                "out dx, eax",
                in("dx") index_port,
                in("eax") index,
            );

            let mut value: u32;
            core::arch::asm!(
                "in eax, dx",
                in("dx") value_port,
                out("eax") value,
            );

            value
        }
    }

    /// Write SVGA register
    fn write_reg(&self, index: u32, value: u32) {
        unsafe {
            let index_port = self.io_base + ports::INDEX as u16;
            let value_port = self.io_base + ports::VALUE as u16;

            core::arch::asm!(
                "out dx, eax",
                in("dx") index_port,
                in("eax") index,
            );

            core::arch::asm!(
                "out dx, eax",
                in("dx") value_port,
                in("eax") value,
            );
        }
    }

    /// Read FIFO register
    fn read_fifo(&self, offset: u32) -> u32 {
        unsafe {
            let ptr = (self.fifo_base + (offset as u64 * 4)) as *const u32;
            core::ptr::read_volatile(ptr)
        }
    }

    /// Write FIFO register
    fn write_fifo(&self, offset: u32, value: u32) {
        unsafe {
            let ptr = (self.fifo_base + (offset as u64 * 4)) as *mut u32;
            core::ptr::write_volatile(ptr, value);
        }
    }

    /// Initialize device
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Set version
        self.write_reg(regs::ID, Self::SVGA_ID_2);

        // Check version response
        let id = self.read_reg(regs::ID);
        if id != Self::SVGA_ID_2 {
            return Err("SVGA version mismatch");
        }

        // Read capabilities
        self.caps = self.read_reg(regs::CAPABILITIES);
        self.vram_size = self.read_reg(regs::VRAM_SIZE);
        self.fifo_size = self.read_reg(regs::FIFO_SIZE);

        // Read max dimensions
        self.max_mode.width = self.read_reg(regs::MAX_WIDTH);
        self.max_mode.height = self.read_reg(regs::MAX_HEIGHT);
        self.max_mode.bpp = 32;

        // Initialize FIFO
        let fifo_start = 4 * 4; // Skip FIFO registers
        self.write_fifo(fifo::MIN, fifo_start);
        self.write_fifo(fifo::MAX, self.fifo_size);
        self.write_fifo(fifo::NEXT_CMD, fifo_start);
        self.write_fifo(fifo::STOP, fifo_start);

        // Enable config done
        self.write_reg(regs::CONFIG_DONE, 1);

        self.initialized.store(true, Ordering::Release);

        // Set default mode
        self.set_mode(1024, 768, 32)?;

        crate::kprintln!("vmware-svga: Initialized, max {}x{}, VRAM {}MB",
            self.max_mode.width, self.max_mode.height,
            self.vram_size / (1024 * 1024));

        Ok(())
    }

    /// Set display mode
    pub fn set_mode(&mut self, width: u32, height: u32, bpp: u32) -> Result<(), &'static str> {
        if width > self.max_mode.width || height > self.max_mode.height {
            return Err("Resolution exceeds maximum");
        }

        if bpp != 8 && bpp != 16 && bpp != 24 && bpp != 32 {
            return Err("Unsupported bit depth");
        }

        // Disable display
        self.write_reg(regs::ENABLE, 0);

        // Set mode
        self.write_reg(regs::WIDTH, width);
        self.write_reg(regs::HEIGHT, height);
        self.write_reg(regs::BITS_PER_PIXEL, bpp);

        // Enable display
        self.write_reg(regs::ENABLE, 1);

        // Read back values
        self.mode.width = self.read_reg(regs::WIDTH);
        self.mode.height = self.read_reg(regs::HEIGHT);
        self.mode.bpp = self.read_reg(regs::BITS_PER_PIXEL);
        self.bytes_per_line = self.read_reg(regs::BYTES_PER_LINE);

        Ok(())
    }

    /// Get current mode
    pub fn mode(&self) -> &DisplayMode {
        &self.mode
    }

    /// Get framebuffer address
    pub fn framebuffer(&self) -> u64 {
        self.fb_base + self.read_reg(regs::FB_OFFSET) as u64
    }

    /// Get framebuffer size
    pub fn framebuffer_size(&self) -> usize {
        self.read_reg(regs::FB_SIZE) as usize
    }

    /// Get bytes per line (pitch)
    pub fn pitch(&self) -> u32 {
        self.bytes_per_line
    }

    /// Update display region
    pub fn update(&self, x: u32, y: u32, width: u32, height: u32) {
        self.fifo_write_cmd(FifoCmd::Update, &[x, y, width, height]);
    }

    /// Update entire display
    pub fn update_full(&self) {
        self.update(0, 0, self.mode.width, self.mode.height);
    }

    /// Rectangle copy
    pub fn rect_copy(&self, src_x: u32, src_y: u32, dst_x: u32, dst_y: u32, width: u32, height: u32) {
        if self.caps & SvgaCap::RectCopy as u32 != 0 {
            self.fifo_write_cmd(FifoCmd::RectCopy, &[src_x, src_y, dst_x, dst_y, width, height]);
        }
    }

    /// Write FIFO command
    fn fifo_write_cmd(&self, cmd: FifoCmd, args: &[u32]) {
        let next_cmd = self.read_fifo(fifo::NEXT_CMD);
        let max = self.read_fifo(fifo::MAX);
        let min = self.read_fifo(fifo::MIN);

        // Calculate space needed
        let cmd_size = (1 + args.len()) * 4;

        // Wait for space
        loop {
            let stop = self.read_fifo(fifo::STOP);
            let free = if next_cmd >= stop {
                (max - next_cmd) + (stop - min)
            } else {
                stop - next_cmd
            };

            if free >= cmd_size as u32 {
                break;
            }

            self.write_reg(regs::SYNC, 1);
            while self.read_reg(regs::BUSY) != 0 {}
        }

        // Write command
        let mut offset = next_cmd;
        self.write_fifo(offset / 4, cmd as u32);
        offset += 4;

        for &arg in args {
            if offset >= max {
                offset = min;
            }
            self.write_fifo(offset / 4, arg);
            offset += 4;
        }

        if offset >= max {
            offset = min;
        }

        self.write_fifo(fifo::NEXT_CMD, offset);
    }

    /// Set cursor position
    pub fn set_cursor_pos(&self, x: u32, y: u32) {
        self.cursor_x.store(x, Ordering::Relaxed);
        self.cursor_y.store(y, Ordering::Relaxed);
        self.write_reg(regs::CURSOR_X, x);
        self.write_reg(regs::CURSOR_Y, y);
    }

    /// Show/hide cursor
    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
        self.write_reg(regs::CURSOR_ON, if visible { 1 } else { 0 });
    }

    /// Define cursor
    pub fn define_cursor(&self, cursor: &CursorDef) {
        if self.caps & SvgaCap::Cursor as u32 == 0 {
            return;
        }

        let mut args = vec![
            cursor.id,
            cursor.hotspot_x,
            cursor.hotspot_y,
            cursor.width,
            cursor.height,
            1, // AND mask depth
            32, // XOR mask depth
        ];

        // AND mask (all 0s for now)
        let mask_size = ((cursor.width + 31) / 32) * cursor.height;
        for _ in 0..mask_size {
            args.push(0);
        }

        // XOR mask (cursor data)
        args.extend(&cursor.data);

        self.fifo_write_cmd(FifoCmd::DefineCursor, &args);
    }

    /// Has capability
    pub fn has_cap(&self, cap: SvgaCap) -> bool {
        self.caps & cap as u32 != 0
    }

    /// Get capabilities
    pub fn capabilities(&self) -> u32 {
        self.caps
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!(
            "VMware SVGA: {}x{}x{} pitch={} VRAM={}MB",
            self.mode.width, self.mode.height, self.mode.bpp,
            self.bytes_per_line, self.vram_size / (1024 * 1024)
        )
    }
}

/// SVGA device manager
pub struct SvgaManager {
    devices: Vec<SvgaDevice>,
}

impl SvgaManager {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    pub fn add_device(&mut self, device: SvgaDevice) -> usize {
        let idx = self.devices.len();
        self.devices.push(device);
        idx
    }

    pub fn get_device(&mut self, idx: usize) -> Option<&mut SvgaDevice> {
        self.devices.get_mut(idx)
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

impl Default for SvgaManager {
    fn default() -> Self {
        Self::new()
    }
}
