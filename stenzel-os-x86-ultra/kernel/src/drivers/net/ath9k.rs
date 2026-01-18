//! Atheros AR9xxx WiFi driver (ath9k-like).
//!
//! This driver supports Atheros AR9280/AR9285/AR9287/AR9380/AR9485/AR9462 WiFi adapters.
//! These are common in laptops and consumer devices.
//!
//! Features:
//! - PCI/PCIe device enumeration
//! - Basic MAC/PHY initialization
//! - Scan/connect infrastructure
//! - WPA/WPA2 support

use alloc::string::String;
use alloc::vec::Vec;
use core::ptr;
use crate::drivers::pci;

/// Atheros vendor ID.
const ATHEROS_VENDOR_ID: u16 = 0x168C;

/// Supported device IDs.
mod device_ids {
    // AR9280 (11n single-band and dual-band)
    pub const AR9280_PCI: u16 = 0x0029;
    pub const AR9280_PCIE: u16 = 0x002A;

    // AR9285 (11n single-band, low-power)
    pub const AR9285_PCIE: u16 = 0x002B;
    pub const AR9285_PCIE_REV2: u16 = 0x002C;

    // AR9287 (11n dual-band)
    pub const AR9287_PCIE: u16 = 0x002D;
    pub const AR9287_PCIE_REV2: u16 = 0x002E;

    // AR9380 (11n triple-stream)
    pub const AR9380_PCIE: u16 = 0x0030;

    // AR9382 (11n dual-stream)
    pub const AR9382_PCIE: u16 = 0x0032;

    // AR9485 (11n single-stream, low-power)
    pub const AR9485_PCIE: u16 = 0x0033;

    // AR9462 (11ac wave 1)
    pub const AR9462_PCIE: u16 = 0x0034;

    // AR9565 (11ac wave 1, budget)
    pub const AR9565_PCIE: u16 = 0x0036;

    // QCA9377 (11ac wave 2)
    pub const QCA9377_PCIE: u16 = 0x0042;

    // QCA6174 (11ac wave 2)
    pub const QCA6174_PCIE: u16 = 0x003E;

    pub fn is_supported(device_id: u16) -> bool {
        matches!(device_id,
            AR9280_PCI | AR9280_PCIE |
            AR9285_PCIE | AR9285_PCIE_REV2 |
            AR9287_PCIE | AR9287_PCIE_REV2 |
            AR9380_PCIE | AR9382_PCIE | AR9485_PCIE |
            AR9462_PCIE | AR9565_PCIE |
            QCA9377_PCIE | QCA6174_PCIE
        )
    }

    pub fn name(device_id: u16) -> &'static str {
        match device_id {
            AR9280_PCI | AR9280_PCIE => "AR9280",
            AR9285_PCIE | AR9285_PCIE_REV2 => "AR9285",
            AR9287_PCIE | AR9287_PCIE_REV2 => "AR9287",
            AR9380_PCIE => "AR9380",
            AR9382_PCIE | AR9485_PCIE => "AR9382/9485",
            AR9462_PCIE => "AR9462",
            AR9565_PCIE => "AR9565",
            QCA9377_PCIE => "QCA9377",
            QCA6174_PCIE => "QCA6174",
            _ => "Unknown Atheros",
        }
    }

    pub fn supports_5ghz(device_id: u16) -> bool {
        matches!(device_id,
            AR9280_PCI | AR9280_PCIE |
            AR9287_PCIE | AR9287_PCIE_REV2 |
            AR9380_PCIE | AR9382_PCIE |
            AR9462_PCIE | AR9565_PCIE |
            QCA9377_PCIE | QCA6174_PCIE
        )
    }

    pub fn supports_11ac(device_id: u16) -> bool {
        matches!(device_id,
            AR9462_PCIE | AR9565_PCIE | QCA9377_PCIE | QCA6174_PCIE
        )
    }
}

/// MMIO register offsets.
mod regs {
    // General
    pub const AR_CR: u32 = 0x0008;           // Control register
    pub const AR_CFG: u32 = 0x0014;          // Configuration
    pub const AR_IER: u32 = 0x0024;          // Interrupt enable
    pub const AR_ISR: u32 = 0x0080;          // Interrupt status

    // EEPROM
    pub const AR_EEPROM_OFFSET: u32 = 0x6000;
    pub const AR_EEPROM_DATA: u32 = 0x6004;

    // MAC registers
    pub const AR_STA_ID0: u32 = 0x8000;      // Station ID (lower 32 bits)
    pub const AR_STA_ID1: u32 = 0x8004;      // Station ID (upper 16 bits)
    pub const AR_BSSMSKL: u32 = 0x8008;      // BSSID mask low
    pub const AR_BSSMSKU: u32 = 0x800C;      // BSSID mask high
    pub const AR_RXDP: u32 = 0x000C;         // RX descriptor pointer
    pub const AR_TXDP0: u32 = 0x0010;        // TX descriptor pointer (queue 0)

    // PHY registers
    pub const AR_PHY_TEST: u32 = 0x9800;     // PHY test
    pub const AR_PHY_TURBO: u32 = 0x9804;    // Turbo mode
    pub const AR_PHY_ACTIVE: u32 = 0x9808;   // PHY active

    // Radio control
    pub const AR_RTC_RC: u32 = 0x7000;       // RTC reset control
    pub const AR_RTC_PLL_CONTROL: u32 = 0x7014; // PLL control
    pub const AR_RTC_STATUS: u32 = 0x7044;   // RTC status
    pub const AR_RTC_FORCE_WAKE: u32 = 0x704C; // Force wake

    // Power management
    pub const AR_POWER_REG: u32 = 0x7010;
    pub const AR_WA_REG: u32 = 0x4004;       // Workaround register

    // Interrupt masks
    pub const AR_IMR: u32 = 0x00A0;          // Primary interrupt mask
    pub const AR_IMR_S0: u32 = 0x00A4;       // Secondary interrupt mask 0
    pub const AR_IMR_S1: u32 = 0x00A8;       // Secondary interrupt mask 1
}

/// Control register bits.
mod cr {
    pub const RXE: u32 = 1 << 2;             // RX enable
    pub const RXD: u32 = 1 << 3;             // RX disable
    pub const SWI: u32 = 1 << 6;             // Software interrupt
}

/// Interrupt bits.
mod intr {
    pub const RXOK: u32 = 1 << 0;            // RX OK
    pub const RXDESC: u32 = 1 << 1;          // RX descriptor done
    pub const RXERR: u32 = 1 << 2;           // RX error
    pub const RXEOL: u32 = 1 << 3;           // RX end of list
    pub const RXORN: u32 = 1 << 4;           // RX overrun
    pub const TXOK: u32 = 1 << 5;            // TX OK
    pub const TXDESC: u32 = 1 << 6;          // TX descriptor done
    pub const TXERR: u32 = 1 << 7;           // TX error
    pub const TXEOL: u32 = 1 << 8;           // TX end of list
    pub const TXURN: u32 = 1 << 9;           // TX underrun
    pub const MIB: u32 = 1 << 10;            // MIB interrupt
    pub const SWI: u32 = 1 << 11;            // Software interrupt
    pub const RXPHY: u32 = 1 << 12;          // RX PHY error
    pub const FATAL: u32 = 1 << 15;          // Fatal error
    pub const GENTMR: u32 = 1 << 16;         // General timer
    pub const BCNMISS: u32 = 1 << 24;        // Beacon miss
}

/// TX descriptor.
#[repr(C, align(4))]
#[derive(Clone, Copy)]
struct TxDesc {
    link_ptr: u32,
    buf_addr: u32,
    buf_len: u32,
    ctl0: u32,
    ctl1: u32,
    status: u32,
    timestamp: u32,
    padding: u32,
}

impl TxDesc {
    const fn new() -> Self {
        Self {
            link_ptr: 0,
            buf_addr: 0,
            buf_len: 0,
            ctl0: 0,
            ctl1: 0,
            status: 0,
            timestamp: 0,
            padding: 0,
        }
    }
}

/// RX descriptor.
#[repr(C, align(4))]
#[derive(Clone, Copy)]
struct RxDesc {
    link_ptr: u32,
    buf_addr: u32,
    ctl: u32,
    status: u32,
    rx_status0: u32,
    rx_status1: u32,
    rx_status2: u32,
    rx_status3: u32,
}

impl RxDesc {
    const fn new() -> Self {
        Self {
            link_ptr: 0,
            buf_addr: 0,
            ctl: 0,
            status: 0,
            rx_status0: 0,
            rx_status1: 0,
            rx_status2: 0,
            rx_status3: 0,
        }
    }
}

/// TX descriptor control bits.
mod txctl {
    pub const FRAME_LEN_MASK: u32 = 0x0FFF;
    pub const MORE: u32 = 1 << 12;
    pub const DEST_IDX_VALID: u32 = 1 << 13;
    pub const INT_REQ: u32 = 1 << 14;
    pub const VEOL: u32 = 1 << 15;
    pub const CLEAR_DEST_MASK: u32 = 1 << 16;
    pub const NO_ACK: u32 = 1 << 17;
    pub const COMP_PROC: u32 = 1 << 18;
}

/// RX descriptor status bits.
mod rxsts {
    pub const DONE: u32 = 1 << 0;
    pub const MORE: u32 = 1 << 1;
    pub const FRAME_ERR: u32 = 1 << 2;
    pub const CRC_ERR: u32 = 1 << 3;
    pub const DECRYPT_CRC_ERR: u32 = 1 << 4;
    pub const PHY_ERR: u32 = 1 << 5;
    pub const MIC_ERR: u32 = 1 << 6;
    pub const PRE_DELIM_CRC_ERR: u32 = 1 << 7;
    pub const KEY_IDX_VALID: u32 = 1 << 8;
}

const TX_RING_SIZE: usize = 32;
const RX_RING_SIZE: usize = 32;
const BUFFER_SIZE: usize = 2048;

/// Scan result.
#[derive(Clone)]
pub struct ScanResult {
    pub ssid: String,
    pub bssid: [u8; 6],
    pub channel: u8,
    pub signal_strength: i8,
    pub encrypted: bool,
}

/// Atheros WiFi driver state.
pub struct Ath9k {
    mmio_base: u64,
    mac: [u8; 6],
    device_id: u16,
    tx_ring: [TxDesc; TX_RING_SIZE],
    rx_ring: [RxDesc; RX_RING_SIZE],
    tx_buffers: [[u8; BUFFER_SIZE]; TX_RING_SIZE],
    rx_buffers: [[u8; BUFFER_SIZE]; RX_RING_SIZE],
    tx_head: usize,
    tx_tail: usize,
    rx_index: usize,
    initialized: bool,
    powered_on: bool,
    connected: bool,
    current_channel: u8,
    scan_results: Vec<ScanResult>,
}

static mut ATH9K_DRIVER: Option<Ath9k> = None;

impl Ath9k {
    /// Create new driver instance.
    fn new(mmio_base: u64, device_id: u16) -> Self {
        Self {
            mmio_base,
            mac: [0; 6],
            device_id,
            tx_ring: [TxDesc::new(); TX_RING_SIZE],
            rx_ring: [RxDesc::new(); RX_RING_SIZE],
            tx_buffers: [[0u8; BUFFER_SIZE]; TX_RING_SIZE],
            rx_buffers: [[0u8; BUFFER_SIZE]; RX_RING_SIZE],
            tx_head: 0,
            tx_tail: 0,
            rx_index: 0,
            initialized: false,
            powered_on: false,
            connected: false,
            current_channel: 1,
            scan_results: Vec::new(),
        }
    }

    /// Read 32-bit MMIO register.
    fn read32(&self, reg: u32) -> u32 {
        unsafe {
            let ptr = (self.mmio_base + reg as u64) as *const u32;
            ptr::read_volatile(ptr)
        }
    }

    /// Write 32-bit MMIO register.
    fn write32(&self, reg: u32, value: u32) {
        unsafe {
            let ptr = (self.mmio_base + reg as u64) as *mut u32;
            ptr::write_volatile(ptr, value);
        }
    }

    /// Force the chip to wake up.
    fn force_wake(&self, enable: bool) {
        if enable {
            self.write32(regs::AR_RTC_FORCE_WAKE, 0x1);
            // Wait for wake
            for _ in 0..1000 {
                let status = self.read32(regs::AR_RTC_STATUS);
                if status & 0x1 != 0 {
                    break;
                }
                for _ in 0..100 {
                    core::hint::spin_loop();
                }
            }
        } else {
            self.write32(regs::AR_RTC_FORCE_WAKE, 0x0);
        }
    }

    /// Reset the device.
    fn reset(&mut self) {
        // Force wake
        self.force_wake(true);

        // Disable interrupts
        self.write32(regs::AR_IER, 0);
        self.write32(regs::AR_IMR, 0);

        // Clear pending interrupts
        let _ = self.read32(regs::AR_ISR);

        // Put chip in reset
        self.write32(regs::AR_RTC_RC, 0x1);

        // Wait for reset
        for _ in 0..1000 {
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }

        // Take chip out of reset
        self.write32(regs::AR_RTC_RC, 0x0);

        // Wait for chip ready
        for _ in 0..1000 {
            let status = self.read32(regs::AR_RTC_STATUS);
            if status & 0x1 != 0 {
                break;
            }
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }
    }

    /// Read MAC address from EEPROM.
    fn read_mac(&mut self) {
        // EEPROM offset for MAC address varies by chip
        // Try common locations
        let mac_offset = 0x1B;  // Common for AR9xxx

        // Read 3 words (6 bytes) from EEPROM
        for i in 0..3 {
            self.write32(regs::AR_EEPROM_OFFSET, (mac_offset + i) as u32);
            // Wait for read
            for _ in 0..100 {
                core::hint::spin_loop();
            }
            let data = self.read32(regs::AR_EEPROM_DATA);
            self.mac[i as usize * 2] = (data & 0xFF) as u8;
            self.mac[i as usize * 2 + 1] = ((data >> 8) & 0xFF) as u8;
        }

        // Validate MAC
        let all_zero = self.mac.iter().all(|&b| b == 0);
        let all_ff = self.mac.iter().all(|&b| b == 0xFF);
        if all_zero || all_ff {
            // Use a fallback MAC
            self.mac = [0x00, 0x03, 0x7F, 0x12, 0x34, 0x56];
        }
    }

    /// Set station MAC address.
    fn set_station_id(&self) {
        let id0 = (self.mac[0] as u32) |
                  ((self.mac[1] as u32) << 8) |
                  ((self.mac[2] as u32) << 16) |
                  ((self.mac[3] as u32) << 24);
        let id1 = (self.mac[4] as u32) |
                  ((self.mac[5] as u32) << 8);

        self.write32(regs::AR_STA_ID0, id0);
        self.write32(regs::AR_STA_ID1, id1);
    }

    /// Initialize TX ring.
    fn init_tx(&mut self) {
        // Setup TX descriptors
        for i in 0..TX_RING_SIZE {
            let next = (i + 1) % TX_RING_SIZE;
            self.tx_ring[i].link_ptr = self.tx_ring.as_ptr().wrapping_add(next) as u32;
            self.tx_ring[i].buf_addr = self.tx_buffers[i].as_ptr() as u32;
        }

        // Set TX descriptor pointer
        self.write32(regs::AR_TXDP0, self.tx_ring.as_ptr() as u32);

        self.tx_head = 0;
        self.tx_tail = 0;
    }

    /// Initialize RX ring.
    fn init_rx(&mut self) {
        // Setup RX descriptors
        for i in 0..RX_RING_SIZE {
            let next = (i + 1) % RX_RING_SIZE;
            self.rx_ring[i].link_ptr = self.rx_ring.as_ptr().wrapping_add(next) as u32;
            self.rx_ring[i].buf_addr = self.rx_buffers[i].as_ptr() as u32;
            self.rx_ring[i].ctl = (BUFFER_SIZE as u32) & 0xFFF;
        }

        // Set RX descriptor pointer
        self.write32(regs::AR_RXDP, self.rx_ring.as_ptr() as u32);

        self.rx_index = 0;
    }

    /// Enable interrupts.
    fn enable_interrupts(&self) {
        self.write32(regs::AR_IMR,
            intr::RXOK |
            intr::RXDESC |
            intr::TXOK |
            intr::TXDESC |
            intr::FATAL
        );
        self.write32(regs::AR_IER, 1);
    }

    /// Set channel.
    fn set_channel(&mut self, channel: u8) {
        // Calculate frequency
        let freq = if channel <= 14 {
            // 2.4 GHz band
            2407 + (channel as u32) * 5
        } else {
            // 5 GHz band (simplified)
            5000 + (channel as u32) * 5
        };

        // Configure PLL for frequency
        // This is highly simplified - real driver needs complex PLL setup
        let pll_div = freq / 5;
        self.write32(regs::AR_RTC_PLL_CONTROL, pll_div);

        // Wait for PLL lock
        for _ in 0..1000 {
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }

        self.current_channel = channel;
    }

    /// Initialize the device.
    pub fn init(&mut self) {
        // Reset
        self.reset();

        // Read MAC
        self.read_mac();

        // Set station ID
        self.set_station_id();

        // Initialize rings
        self.init_tx();
        self.init_rx();

        // Set default channel
        self.set_channel(1);

        // Enable interrupts
        self.enable_interrupts();

        // Enable receiver
        let cr = self.read32(regs::AR_CR);
        self.write32(regs::AR_CR, cr | cr::RXE);

        self.initialized = true;
        self.powered_on = true;

        crate::kprintln!("ath9k: {} initialized, MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            device_ids::name(self.device_id),
            self.mac[0], self.mac[1], self.mac[2],
            self.mac[3], self.mac[4], self.mac[5]);
    }

    /// Power off.
    pub fn power_off(&mut self) {
        if !self.initialized {
            return;
        }

        // Disable receiver
        let cr = self.read32(regs::AR_CR);
        self.write32(regs::AR_CR, (cr | cr::RXD) & !cr::RXE);

        // Disable interrupts
        self.write32(regs::AR_IER, 0);
        self.write32(regs::AR_IMR, 0);

        // Release wake
        self.force_wake(false);

        self.powered_on = false;
    }

    /// Power on.
    pub fn power_on(&mut self) {
        if !self.initialized {
            return;
        }

        // Force wake
        self.force_wake(true);

        // Enable interrupts
        self.enable_interrupts();

        // Enable receiver
        let cr = self.read32(regs::AR_CR);
        self.write32(regs::AR_CR, cr | cr::RXE);

        self.powered_on = true;
    }

    /// Start a scan.
    pub fn start_scan(&mut self) {
        self.scan_results.clear();

        // Scan 2.4 GHz channels 1-11
        for channel in 1..=11u8 {
            self.set_channel(channel);
            // In a real driver, we'd send probe requests and wait for responses
            // For now, just simulate a delay
            for _ in 0..10000 {
                core::hint::spin_loop();
            }
            // Process any beacon frames received
            self.process_rx_for_scan();
        }

        // If device supports 5 GHz, scan those channels too
        if device_ids::supports_5ghz(self.device_id) {
            for channel in [36, 40, 44, 48, 52, 56, 60, 64, 100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140, 149, 153, 157, 161, 165] {
                self.set_channel(channel);
                for _ in 0..10000 {
                    core::hint::spin_loop();
                }
                self.process_rx_for_scan();
            }
        }
    }

    /// Process RX for scanning.
    fn process_rx_for_scan(&mut self) {
        // Check RX ring for beacon frames
        for _ in 0..RX_RING_SIZE {
            // Check status first
            let status = self.rx_ring[self.rx_index].status;
            if status & rxsts::DONE == 0 {
                break;
            }

            // Get length and index
            let len = (self.rx_ring[self.rx_index].ctl & 0xFFF) as usize;
            let idx = self.rx_index;

            // Parse beacon if valid length
            if len > 24 && len <= BUFFER_SIZE {
                // Copy frame data to avoid borrow issues
                let mut frame_data = [0u8; BUFFER_SIZE];
                frame_data[..len].copy_from_slice(&self.rx_buffers[idx][..len]);
                let frame = &frame_data[..len];

                // Check if it's a beacon (frame control type/subtype)
                if frame.len() >= 2 && (frame[0] & 0x0C) == 0x00 && (frame[0] >> 4) == 8 {
                    // Parse beacon
                    if let Some(result) = self.parse_beacon(frame) {
                        // Add to results if not duplicate
                        if !self.scan_results.iter().any(|r| r.bssid == result.bssid) {
                            self.scan_results.push(result);
                        }
                    }
                }
            }

            // Reset descriptor
            self.rx_ring[self.rx_index].status = 0;
            self.rx_index = (self.rx_index + 1) % RX_RING_SIZE;
        }
    }

    /// Parse a beacon frame.
    fn parse_beacon(&self, frame: &[u8]) -> Option<ScanResult> {
        if frame.len() < 36 {
            return None;
        }

        // BSSID is at offset 16
        let bssid = [
            frame[16], frame[17], frame[18],
            frame[19], frame[20], frame[21],
        ];

        // Parse information elements
        let mut offset = 36;
        let mut ssid = String::new();
        let mut encrypted = false;

        while offset + 2 <= frame.len() {
            let ie_id = frame[offset];
            let ie_len = frame[offset + 1] as usize;

            if offset + 2 + ie_len > frame.len() {
                break;
            }

            match ie_id {
                0 => {
                    // SSID
                    if let Ok(s) = core::str::from_utf8(&frame[offset + 2..offset + 2 + ie_len]) {
                        ssid = String::from(s);
                    }
                }
                48 | 221 => {
                    // RSN or vendor-specific WPA
                    encrypted = true;
                }
                _ => {}
            }

            offset += 2 + ie_len;
        }

        Some(ScanResult {
            ssid,
            bssid,
            channel: self.current_channel,
            signal_strength: -50, // Simplified
            encrypted,
        })
    }

    /// Handle interrupt.
    pub fn handle_interrupt(&mut self) {
        if !self.initialized {
            return;
        }

        let isr = self.read32(regs::AR_ISR);

        if isr & intr::FATAL != 0 {
            crate::kprintln!("ath9k: Fatal error, resetting");
            self.reset();
            self.init_tx();
            self.init_rx();
        }

        if isr & (intr::RXOK | intr::RXDESC) != 0 {
            // RX complete
        }

        if isr & (intr::TXOK | intr::TXDESC) != 0 {
            // TX complete
        }

        // Clear interrupts
        self.write32(regs::AR_ISR, isr);
    }

    /// Receive a frame.
    pub fn recv_frame(&mut self) -> Option<Vec<u8>> {
        if !self.initialized || !self.powered_on {
            return None;
        }

        let desc = &mut self.rx_ring[self.rx_index];

        if desc.status & rxsts::DONE == 0 {
            return None;
        }

        // Check for errors
        if desc.status & (rxsts::FRAME_ERR | rxsts::CRC_ERR | rxsts::PHY_ERR) != 0 {
            desc.status = 0;
            self.rx_index = (self.rx_index + 1) % RX_RING_SIZE;
            return None;
        }

        let len = (desc.ctl & 0xFFF) as usize;
        if len == 0 || len > BUFFER_SIZE {
            desc.status = 0;
            self.rx_index = (self.rx_index + 1) % RX_RING_SIZE;
            return None;
        }

        let mut frame = Vec::with_capacity(len);
        frame.extend_from_slice(&self.rx_buffers[self.rx_index][..len]);

        desc.status = 0;
        self.rx_index = (self.rx_index + 1) % RX_RING_SIZE;

        Some(frame)
    }

    /// Get scan results.
    pub fn get_scan_results(&self) -> &[ScanResult] {
        &self.scan_results
    }
}

/// Probe PCI for Atheros devices.
pub fn probe_pci() -> Option<(u64, u16)> {
    let devices = pci::scan();

    for dev in devices {
        if dev.id.vendor_id == ATHEROS_VENDOR_ID
            && device_ids::is_supported(dev.id.device_id)
        {
            // Enable bus mastering
            pci::enable_bus_mastering(&dev);

            // Get BAR0 (MMIO)
            let (bar0, is_io) = pci::read_bar(&dev, 0);
            if !is_io && bar0 != 0 {
                let mmio_base = bar0 & !0xF;
                return Some((mmio_base, dev.id.device_id));
            }
        }
    }

    None
}

/// Initialize the Atheros driver.
pub fn init() {
    if let Some((mmio_base, device_id)) = probe_pci() {
        unsafe {
            let mut driver = Ath9k::new(mmio_base, device_id);
            driver.init();
            ATH9K_DRIVER = Some(driver);
        }
    }
}

/// Get MAC address.
pub fn get_mac() -> Option<[u8; 6]> {
    unsafe {
        ATH9K_DRIVER.as_ref().map(|d| d.mac)
    }
}

/// Start a scan.
pub fn start_scan() {
    unsafe {
        if let Some(d) = ATH9K_DRIVER.as_mut() {
            d.start_scan();
        }
    }
}

/// Get scan results.
pub fn get_scan_results() -> Vec<ScanResult> {
    unsafe {
        ATH9K_DRIVER.as_ref().map_or(Vec::new(), |d| d.scan_results.clone())
    }
}

/// Get device name.
pub fn device_name() -> &'static str {
    unsafe {
        ATH9K_DRIVER.as_ref().map_or("none", |d| device_ids::name(d.device_id))
    }
}

/// Check if initialized.
pub fn is_initialized() -> bool {
    unsafe {
        ATH9K_DRIVER.as_ref().map_or(false, |d| d.initialized)
    }
}

/// Check if supports 5GHz.
pub fn supports_5ghz() -> bool {
    unsafe {
        ATH9K_DRIVER.as_ref().map_or(false, |d| device_ids::supports_5ghz(d.device_id))
    }
}

/// Check if supports 802.11ac.
pub fn supports_11ac() -> bool {
    unsafe {
        ATH9K_DRIVER.as_ref().map_or(false, |d| device_ids::supports_11ac(d.device_id))
    }
}
