//! VirtIO Console Device Driver
//!
//! Provides serial console access via VirtIO protocol.

#![allow(dead_code)]

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use alloc::collections::VecDeque;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::virtqueue::Virtqueue;
use super::{VirtioDevice, VirtioDeviceType, features};

/// Console device feature flags
pub mod console_features {
    pub const VIRTIO_CONSOLE_F_SIZE: u64 = 1 << 0;
    pub const VIRTIO_CONSOLE_F_MULTIPORT: u64 = 1 << 1;
    pub const VIRTIO_CONSOLE_F_EMERG_WRITE: u64 = 1 << 2;
}

/// Console device configuration
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VirtioConsoleConfig {
    pub cols: u16,
    pub rows: u16,
    pub max_ports: u32,
    pub emerg_wr: u32,
}

/// Console control message types
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleControlType {
    DeviceReady = 0,
    DeviceAdd = 1,
    DeviceRemove = 2,
    PortReady = 3,
    ConsolePort = 4,
    Resize = 5,
    Open = 6,
    Close = 7,
    Name = 8,
}

/// Console control message
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtioConsoleControl {
    pub id: u32,
    pub event: u16,
    pub value: u16,
}

/// Console port
pub struct ConsolePort {
    /// Port ID
    id: u32,
    /// Port name
    name: String,
    /// Is console port
    is_console: bool,
    /// Port is open
    open: bool,
    /// Receive buffer
    rx_buffer: VecDeque<u8>,
    /// Transmit buffer
    tx_buffer: VecDeque<u8>,
    /// Columns (if known)
    cols: u16,
    /// Rows (if known)
    rows: u16,
}

impl ConsolePort {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            name: String::new(),
            is_console: false,
            open: false,
            rx_buffer: VecDeque::with_capacity(4096),
            tx_buffer: VecDeque::with_capacity(4096),
            cols: 80,
            rows: 25,
        }
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = String::from(name);
    }

    pub fn set_console(&mut self, is_console: bool) {
        self.is_console = is_console;
    }

    pub fn set_size(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn write(&mut self, data: &[u8]) -> usize {
        if !self.open {
            return 0;
        }
        for &byte in data {
            self.tx_buffer.push_back(byte);
        }
        data.len()
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> usize {
        if !self.open {
            return 0;
        }
        let mut count = 0;
        for byte in buffer.iter_mut() {
            if let Some(b) = self.rx_buffer.pop_front() {
                *byte = b;
                count += 1;
            } else {
                break;
            }
        }
        count
    }

    pub fn receive_data(&mut self, data: &[u8]) {
        for &byte in data {
            self.rx_buffer.push_back(byte);
        }
    }

    pub fn available(&self) -> usize {
        self.rx_buffer.len()
    }

    pub fn flush(&mut self) -> Vec<u8> {
        self.tx_buffer.drain(..).collect()
    }
}

/// Console statistics
#[derive(Debug, Default)]
pub struct ConsoleStats {
    pub rx_bytes: AtomicU64,
    pub tx_bytes: AtomicU64,
    pub rx_errors: AtomicU64,
    pub tx_errors: AtomicU64,
}

/// VirtIO console device
pub struct VirtioConsoleDevice {
    /// Device configuration
    config: VirtioConsoleConfig,
    /// Receive queue
    rx_queue: Virtqueue,
    /// Transmit queue
    tx_queue: Virtqueue,
    /// Control receive queue (multiport)
    ctrl_rx_queue: Option<Virtqueue>,
    /// Control transmit queue (multiport)
    ctrl_tx_queue: Option<Virtqueue>,
    /// Negotiated features
    features: u64,
    /// Initialized
    initialized: AtomicBool,
    /// Console ports
    ports: Vec<ConsolePort>,
    /// Statistics
    stats: ConsoleStats,
    /// Multiport mode
    multiport: bool,
}

impl VirtioConsoleDevice {
    /// Create new console device
    pub fn new(queue_size: u16) -> Self {
        Self {
            config: VirtioConsoleConfig::default(),
            rx_queue: Virtqueue::new(0, queue_size),
            tx_queue: Virtqueue::new(1, queue_size),
            ctrl_rx_queue: None,
            ctrl_tx_queue: None,
            features: 0,
            initialized: AtomicBool::new(false),
            ports: Vec::new(),
            stats: ConsoleStats::default(),
            multiport: false,
        }
    }

    /// Get console size
    pub fn size(&self) -> (u16, u16) {
        (self.config.cols, self.config.rows)
    }

    /// Check if multiport
    pub fn is_multiport(&self) -> bool {
        self.multiport
    }

    /// Get port count
    pub fn port_count(&self) -> usize {
        self.ports.len()
    }

    /// Get port by ID
    pub fn get_port(&mut self, id: u32) -> Option<&mut ConsolePort> {
        self.ports.iter_mut().find(|p| p.id == id)
    }

    /// Get console port
    pub fn get_console_port(&mut self) -> Option<&mut ConsolePort> {
        self.ports.iter_mut().find(|p| p.is_console)
    }

    /// Write to console
    pub fn write(&mut self, data: &[u8]) -> usize {
        if !self.initialized.load(Ordering::Acquire) {
            return 0;
        }

        // Find the port index first
        let port_idx = if self.multiport {
            self.ports.iter().position(|p| p.is_console)
        } else {
            if self.ports.is_empty() { None } else { Some(0) }
        };

        if let Some(idx) = port_idx {
            let written = self.ports[idx].write(data);
            self.stats.tx_bytes.fetch_add(written as u64, Ordering::Relaxed);

            // Flush to virtqueue
            let tx_data = self.ports[idx].flush();
            if !tx_data.is_empty() {
                // In real implementation:
                // 1. Copy to DMA buffer
                // 2. Add to TX queue
                // 3. Notify device
            }

            written
        } else {
            0
        }
    }

    /// Write string
    pub fn write_str(&mut self, s: &str) -> usize {
        self.write(s.as_bytes())
    }

    /// Read from console
    pub fn read(&mut self, buffer: &mut [u8]) -> usize {
        if !self.initialized.load(Ordering::Acquire) {
            return 0;
        }

        // Find the port index first
        let port_idx = if self.multiport {
            self.ports.iter().position(|p| p.is_console)
        } else {
            if self.ports.is_empty() { None } else { Some(0) }
        };

        if let Some(idx) = port_idx {
            let bytes_read = self.ports[idx].read(buffer);
            self.stats.rx_bytes.fetch_add(bytes_read as u64, Ordering::Relaxed);
            bytes_read
        } else {
            0
        }
    }

    /// Check if data available
    pub fn available(&self) -> usize {
        if let Some(port) = self.ports.iter().find(|p| p.is_console || !self.multiport) {
            port.available()
        } else {
            0
        }
    }

    /// Emergency write (direct)
    pub fn emergency_write(&mut self, byte: u8) {
        if self.features & console_features::VIRTIO_CONSOLE_F_EMERG_WRITE != 0 {
            // Write directly to emerg_wr register
            // In real implementation, write to MMIO/PCI config space
            let _ = byte;
        }
    }

    /// Process control message
    fn process_control(&mut self, ctrl: &VirtioConsoleControl) {
        match ctrl.event {
            x if x == ConsoleControlType::DeviceAdd as u16 => {
                // Add new port
                let port = ConsolePort::new(ctrl.id);
                self.ports.push(port);
            }
            x if x == ConsoleControlType::DeviceRemove as u16 => {
                // Remove port
                self.ports.retain(|p| p.id != ctrl.id);
            }
            x if x == ConsoleControlType::ConsolePort as u16 => {
                // Mark as console port
                if let Some(port) = self.get_port(ctrl.id) {
                    port.set_console(ctrl.value != 0);
                }
            }
            x if x == ConsoleControlType::Open as u16 => {
                if let Some(port) = self.get_port(ctrl.id) {
                    port.open();
                }
            }
            x if x == ConsoleControlType::Close as u16 => {
                if let Some(port) = self.get_port(ctrl.id) {
                    port.close();
                }
            }
            x if x == ConsoleControlType::Resize as u16 => {
                // Size change - would need to read new size from config
            }
            _ => {}
        }
    }

    /// Get statistics
    pub fn stats(&self) -> &ConsoleStats {
        &self.stats
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!(
            "VirtIO Console: {}x{} ports={} multiport={}",
            self.config.cols, self.config.rows,
            self.ports.len(), self.multiport
        )
    }
}

impl VirtioDevice for VirtioConsoleDevice {
    fn device_type(&self) -> VirtioDeviceType {
        VirtioDeviceType::Console
    }

    fn init(&mut self) -> Result<(), &'static str> {
        // Read configuration
        self.config.cols = 80;
        self.config.rows = 25;

        // Create default port
        let mut port = ConsolePort::new(0);
        port.set_console(true);
        port.open();
        self.ports.push(port);

        Ok(())
    }

    fn reset(&mut self) {
        self.initialized.store(false, Ordering::Release);
        self.ports.clear();
        self.rx_queue = Virtqueue::new(0, self.rx_queue.size);
        self.tx_queue = Virtqueue::new(1, self.tx_queue.size);
    }

    fn negotiate_features(&mut self, offered: u64) -> u64 {
        let mut wanted = features::VIRTIO_F_VERSION_1;

        if offered & console_features::VIRTIO_CONSOLE_F_SIZE != 0 {
            wanted |= console_features::VIRTIO_CONSOLE_F_SIZE;
        }
        if offered & console_features::VIRTIO_CONSOLE_F_MULTIPORT != 0 {
            wanted |= console_features::VIRTIO_CONSOLE_F_MULTIPORT;
            self.multiport = true;
            // Create control queues
            self.ctrl_rx_queue = Some(Virtqueue::new(2, 64));
            self.ctrl_tx_queue = Some(Virtqueue::new(3, 64));
        }
        if offered & console_features::VIRTIO_CONSOLE_F_EMERG_WRITE != 0 {
            wanted |= console_features::VIRTIO_CONSOLE_F_EMERG_WRITE;
        }

        self.features = wanted & offered;
        self.features
    }

    fn activate(&mut self) -> Result<(), &'static str> {
        self.initialized.store(true, Ordering::Release);
        crate::kprintln!("virtio-console: Activated, {}x{}", self.config.cols, self.config.rows);
        Ok(())
    }

    fn handle_interrupt(&mut self) {
        // Process RX data
        while let Some((desc_id, len)) = self.rx_queue.get_used() {
            // In real implementation, copy data from buffer to port
            let _ = (desc_id, len);
        }

        // Process control messages
        if let Some(ref mut ctrl_queue) = self.ctrl_rx_queue {
            while let Some((_desc_id, _len)) = ctrl_queue.get_used() {
                // Parse and process control message
            }
        }
    }
}

/// Console device manager
pub struct VirtioConsoleManager {
    devices: Vec<VirtioConsoleDevice>,
}

impl VirtioConsoleManager {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    pub fn add_device(&mut self, device: VirtioConsoleDevice) -> usize {
        let idx = self.devices.len();
        self.devices.push(device);
        idx
    }

    pub fn get_device(&mut self, idx: usize) -> Option<&mut VirtioConsoleDevice> {
        self.devices.get_mut(idx)
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

impl Default for VirtioConsoleManager {
    fn default() -> Self {
        Self::new()
    }
}
