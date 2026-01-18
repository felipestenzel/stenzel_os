//! Broadcom WiFi driver (brcmfmac-like).
//!
//! This driver supports Broadcom BCM43xx WiFi adapters.
//! These are common in laptops (especially MacBooks), routers, and embedded devices.
//!
//! Features:
//! - PCI/PCIe device enumeration
//! - Basic MAC/PHY initialization
//! - Scan/connect infrastructure

use alloc::string::String;
use alloc::vec::Vec;
use core::ptr;
use crate::drivers::pci;

/// Broadcom vendor ID.
const BROADCOM_VENDOR_ID: u16 = 0x14E4;

/// Supported device IDs.
mod device_ids {
    // BCM4313 (802.11n, single-band, budget)
    pub const BCM4313_PCIE: u16 = 0x4727;

    // BCM4321 (802.11n, dual-band)
    pub const BCM4321_PCIE: u16 = 0x4328;

    // BCM4322 (802.11n, dual-band)
    pub const BCM4322_PCIE: u16 = 0x432B;

    // BCM43224 (802.11n, dual-band)
    pub const BCM43224_PCIE: u16 = 0x4353;

    // BCM43225 (802.11n, single-band)
    pub const BCM43225_PCIE: u16 = 0x4357;

    // BCM43227 (802.11n, single-band)
    pub const BCM43227_PCIE: u16 = 0x4358;

    // BCM43228 (802.11n, dual-band)
    pub const BCM43228_PCIE: u16 = 0x4359;

    // BCM4331 (802.11n, dual-band, MacBooks)
    pub const BCM4331_PCIE: u16 = 0x4331;

    // BCM4352 (802.11ac, dual-band)
    pub const BCM4352_PCIE: u16 = 0x43B1;
    pub const BCM4352_PCIE_ALT: u16 = 0x43A0;

    // BCM4360 (802.11ac, dual-band, MacBooks)
    pub const BCM4360_PCIE: u16 = 0x43A0;

    // BCM4350 (802.11ac, dual-band)
    pub const BCM4350_PCIE: u16 = 0x43A3;

    // BCM43602 (802.11ac, dual-band)
    pub const BCM43602_PCIE: u16 = 0x43BA;

    // BCM4356 (802.11ac, M.2)
    pub const BCM4356_PCIE: u16 = 0x43EC;

    // BCM4358 (802.11ac, dual-band)
    pub const BCM4358_PCIE: u16 = 0x43E9;

    // BCM43142 (802.11n, single-band, budget)
    pub const BCM43142_PCIE: u16 = 0x4365;

    // BCM4365 (802.11ac wave 2)
    pub const BCM4365_PCIE: u16 = 0x43CA;

    // BCM4366 (802.11ac wave 2)
    pub const BCM4366_PCIE: u16 = 0x43C3;

    pub fn is_supported(device_id: u16) -> bool {
        matches!(device_id,
            BCM4313_PCIE | BCM4321_PCIE | BCM4322_PCIE |
            BCM43224_PCIE | BCM43225_PCIE | BCM43227_PCIE | BCM43228_PCIE |
            BCM4331_PCIE | BCM4352_PCIE | BCM4352_PCIE_ALT |
            BCM4360_PCIE | BCM4350_PCIE | BCM43602_PCIE |
            BCM4356_PCIE | BCM4358_PCIE | BCM43142_PCIE |
            BCM4365_PCIE | BCM4366_PCIE
        )
    }

    pub fn name(device_id: u16) -> &'static str {
        match device_id {
            BCM4313_PCIE => "BCM4313",
            BCM4321_PCIE => "BCM4321",
            BCM4322_PCIE => "BCM4322",
            BCM43224_PCIE => "BCM43224",
            BCM43225_PCIE => "BCM43225",
            BCM43227_PCIE => "BCM43227",
            BCM43228_PCIE => "BCM43228",
            BCM4331_PCIE => "BCM4331",
            BCM4352_PCIE | BCM4352_PCIE_ALT => "BCM4352",
            BCM4360_PCIE => "BCM4360",
            BCM4350_PCIE => "BCM4350",
            BCM43602_PCIE => "BCM43602",
            BCM4356_PCIE => "BCM4356",
            BCM4358_PCIE => "BCM4358",
            BCM43142_PCIE => "BCM43142",
            BCM4365_PCIE => "BCM4365",
            BCM4366_PCIE => "BCM4366",
            _ => "Unknown Broadcom",
        }
    }

    pub fn supports_5ghz(device_id: u16) -> bool {
        matches!(device_id,
            BCM4321_PCIE | BCM4322_PCIE |
            BCM43224_PCIE | BCM43228_PCIE |
            BCM4331_PCIE | BCM4352_PCIE | BCM4352_PCIE_ALT |
            BCM4360_PCIE | BCM4350_PCIE | BCM43602_PCIE |
            BCM4356_PCIE | BCM4358_PCIE |
            BCM4365_PCIE | BCM4366_PCIE
        )
    }

    pub fn supports_11ac(device_id: u16) -> bool {
        matches!(device_id,
            BCM4352_PCIE | BCM4352_PCIE_ALT |
            BCM4360_PCIE | BCM4350_PCIE | BCM43602_PCIE |
            BCM4356_PCIE | BCM4358_PCIE |
            BCM4365_PCIE | BCM4366_PCIE
        )
    }
}

/// MMIO register offsets (SoftMAC style registers).
mod regs {
    // General control
    pub const BIMC_CTL: u32 = 0x0000;         // Bus interface MAC control
    pub const INTR_STATUS: u32 = 0x0020;      // Interrupt status
    pub const INTR_MASK: u32 = 0x0024;        // Interrupt mask
    pub const GPIO_CTRL: u32 = 0x006C;        // GPIO control
    pub const GPIO_OUT: u32 = 0x0070;         // GPIO output
    pub const GPIO_OUTEN: u32 = 0x0074;       // GPIO output enable

    // SB registers (Silicon Backplane)
    pub const SBIMSTATE: u32 = 0x0F90;        // SB initiator state
    pub const SBINTVEC: u32 = 0x0F94;         // SB interrupt vector
    pub const SBTMSTATELOW: u32 = 0x0F98;     // SB target state low
    pub const SBTMSTATEHIGH: u32 = 0x0F9C;    // SB target state high
    pub const SBBWA0: u32 = 0x0FA0;           // SB bandwidth allocation
    pub const SBIMCONFIGLOW: u32 = 0x0FA8;    // SB IM config low
    pub const SBIMCONFIGHIGH: u32 = 0x0FAC;   // SB IM config high
    pub const SBADMATCH0: u32 = 0x0FB0;       // SB address match 0
    pub const SBTMCONFIGLOW: u32 = 0x0FB8;    // SB TM config low
    pub const SBTMCONFIGHIGH: u32 = 0x0FBC;   // SB TM config high
    pub const SBBCONFIG: u32 = 0x0FC0;        // SB configuration
    pub const SBIDLOW: u32 = 0x0FF8;          // SB identification low
    pub const SBIDHIGH: u32 = 0x0FFC;         // SB identification high

    // MAC control
    pub const MACCONTROL: u32 = 0x0120;       // MAC control
    pub const MACINTMASK: u32 = 0x0124;       // MAC interrupt mask
    pub const MACINTSTATUS: u32 = 0x0128;     // MAC interrupt status
    pub const MACCHANSTATUS: u32 = 0x0140;    // MAC channel status
    pub const PSMDEBUG: u32 = 0x0180;         // PSM debug
    pub const PHYDEBUG: u32 = 0x0184;         // PHY debug

    // SPROM/EEPROM
    pub const SPROM_CTRL: u32 = 0x0088;       // SPROM control
    pub const SPROM_ADDR: u32 = 0x008C;       // SPROM address
    pub const SPROM_DATA: u32 = 0x0090;       // SPROM data

    // BRCM DMA registers
    pub const DMA64TXREGOFFS: u32 = 0x0200;   // TX DMA registers
    pub const DMA64RXREGOFFS: u32 = 0x0220;   // RX DMA registers
}

/// Interrupt bits.
mod intr {
    pub const MI_MACSSPNDD: u32 = 1 << 0;     // MAC suspended
    pub const MI_BCNTPL: u32 = 1 << 1;        // Beacon template available
    pub const MI_TBTT: u32 = 1 << 2;          // Target beacon transmission time
    pub const MI_BCNSUCCESS: u32 = 1 << 3;    // Beacon successfully transmitted
    pub const MI_BCNCANCLD: u32 = 1 << 4;     // Beacon cancelled
    pub const MI_ATIMWINEND: u32 = 1 << 5;    // ATIM window end
    pub const MI_PMQ: u32 = 1 << 6;           // Power management queue interrupt
    pub const MI_NSPECGEN0: u32 = 1 << 7;     // Non-specific gen-stat interrupt 0
    pub const MI_NSPECGEN1: u32 = 1 << 8;     // Non-specific gen-stat interrupt 1
    pub const MI_MACTXERR: u32 = 1 << 9;      // MAC TX error
    pub const MI_NSPECGEN3: u32 = 1 << 10;    // Non-specific gen-stat interrupt 3
    pub const MI_RXOV: u32 = 1 << 11;         // RX overflow
    pub const MI_TFS: u32 = 1 << 14;          // TX FIFO sync
    pub const MI_GP0: u32 = 1 << 16;          // General-purpose 0
    pub const MI_GP1: u32 = 1 << 17;          // General-purpose 1
    pub const MI_TO: u32 = 1 << 19;           // Timeout
    pub const MI_PHYTXERR: u32 = 1 << 20;     // PHY TX error
    pub const MI_PME: u32 = 1 << 21;          // Power management event
}

/// TX descriptor (D64).
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct D64TxDesc {
    ctrl1: u32,
    ctrl2: u32,
    addr_low: u32,
    addr_high: u32,
}

impl D64TxDesc {
    const fn new() -> Self {
        Self {
            ctrl1: 0,
            ctrl2: 0,
            addr_low: 0,
            addr_high: 0,
        }
    }
}

/// RX descriptor (D64).
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct D64RxDesc {
    ctrl1: u32,
    ctrl2: u32,
    addr_low: u32,
    addr_high: u32,
}

impl D64RxDesc {
    const fn new() -> Self {
        Self {
            ctrl1: 0,
            ctrl2: 0,
            addr_low: 0,
            addr_high: 0,
        }
    }
}

/// D64 descriptor control bits.
mod d64ctl {
    pub const D64_CTRL1_EOT: u32 = 1 << 28;    // End of table
    pub const D64_CTRL1_IOC: u32 = 1 << 29;    // Interrupt on completion
    pub const D64_CTRL1_EOF: u32 = 1 << 30;    // End of frame
    pub const D64_CTRL1_SOF: u32 = 1 << 31;    // Start of frame
    pub const D64_CTRL2_BC_MASK: u32 = 0x7FFF; // Buffer count mask
}

const TX_RING_SIZE: usize = 64;
const RX_RING_SIZE: usize = 64;
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

/// Broadcom WiFi driver state.
pub struct Brcm {
    mmio_base: u64,
    mac: [u8; 6],
    device_id: u16,
    chip_rev: u8,
    tx_ring: [D64TxDesc; TX_RING_SIZE],
    rx_ring: [D64RxDesc; RX_RING_SIZE],
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

static mut BRCM_DRIVER: Option<Brcm> = None;

impl Brcm {
    /// Create new driver instance.
    fn new(mmio_base: u64, device_id: u16) -> Self {
        Self {
            mmio_base,
            mac: [0; 6],
            device_id,
            chip_rev: 0,
            tx_ring: [D64TxDesc::new(); TX_RING_SIZE],
            rx_ring: [D64RxDesc::new(); RX_RING_SIZE],
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

    /// Read SB identification.
    fn read_chip_info(&mut self) {
        let id_high = self.read32(regs::SBIDHIGH);
        self.chip_rev = ((id_high >> 16) & 0xF) as u8;
    }

    /// Read MAC from SPROM.
    fn read_mac(&mut self) {
        // Broadcom stores MAC in SPROM at various offsets depending on chip
        // Common offset is 0x4F (word 79) for MAC address

        // Read words from SPROM
        let mac_offset = 0x4F;

        for i in 0..3 {
            // Set address
            self.write32(regs::SPROM_ADDR, mac_offset + i);

            // Wait for read
            for _ in 0..1000 {
                let ctrl = self.read32(regs::SPROM_CTRL);
                if ctrl & 0x8000 == 0 {
                    break;
                }
                core::hint::spin_loop();
            }

            // Read data
            let data = self.read32(regs::SPROM_DATA) as u16;
            self.mac[i as usize * 2] = (data >> 8) as u8;
            self.mac[i as usize * 2 + 1] = (data & 0xFF) as u8;
        }

        // Validate MAC
        let all_zero = self.mac.iter().all(|&b| b == 0);
        let all_ff = self.mac.iter().all(|&b| b == 0xFF);
        if all_zero || all_ff {
            // Use fallback MAC
            self.mac = [0x00, 0x10, 0x18, 0x12, 0x34, 0x56];
        }
    }

    /// Reset the device.
    fn reset(&mut self) {
        // Disable interrupts
        self.write32(regs::INTR_MASK, 0);
        self.write32(regs::MACINTMASK, 0);

        // Clear pending interrupts
        let _ = self.read32(regs::INTR_STATUS);
        let _ = self.read32(regs::MACINTSTATUS);

        // Reset SB
        self.write32(regs::SBTMSTATELOW, 0x1);  // Set reset

        // Wait
        for _ in 0..1000 {
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }

        // Clear reset
        self.write32(regs::SBTMSTATELOW, 0x0);

        // Wait for ready
        for _ in 0..1000 {
            let state = self.read32(regs::SBTMSTATEHIGH);
            if state & 0x1 == 0 {
                break;
            }
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }
    }

    /// Initialize TX ring.
    fn init_tx(&mut self) {
        // Setup TX descriptors
        for i in 0..TX_RING_SIZE {
            let buf_addr = self.tx_buffers[i].as_ptr() as u64;
            self.tx_ring[i].addr_low = buf_addr as u32;
            self.tx_ring[i].addr_high = (buf_addr >> 32) as u32;

            // Mark as end of table for last descriptor
            if i == TX_RING_SIZE - 1 {
                self.tx_ring[i].ctrl1 |= d64ctl::D64_CTRL1_EOT;
            }
        }

        // Set TX ring address
        let tx_ring_addr = self.tx_ring.as_ptr() as u64;
        self.write32(regs::DMA64TXREGOFFS, tx_ring_addr as u32);
        self.write32(regs::DMA64TXREGOFFS + 4, (tx_ring_addr >> 32) as u32);

        self.tx_head = 0;
        self.tx_tail = 0;
    }

    /// Initialize RX ring.
    fn init_rx(&mut self) {
        // Setup RX descriptors
        for i in 0..RX_RING_SIZE {
            let buf_addr = self.rx_buffers[i].as_ptr() as u64;
            self.rx_ring[i].addr_low = buf_addr as u32;
            self.rx_ring[i].addr_high = (buf_addr >> 32) as u32;
            self.rx_ring[i].ctrl2 = BUFFER_SIZE as u32;

            // Mark as end of table for last descriptor
            if i == RX_RING_SIZE - 1 {
                self.rx_ring[i].ctrl1 |= d64ctl::D64_CTRL1_EOT;
            }
        }

        // Set RX ring address
        let rx_ring_addr = self.rx_ring.as_ptr() as u64;
        self.write32(regs::DMA64RXREGOFFS, rx_ring_addr as u32);
        self.write32(regs::DMA64RXREGOFFS + 4, (rx_ring_addr >> 32) as u32);

        self.rx_index = 0;
    }

    /// Enable interrupts.
    fn enable_interrupts(&self) {
        self.write32(regs::MACINTMASK,
            intr::MI_TFS |     // TX frame sent
            intr::MI_RXOV |    // RX overflow
            intr::MI_TO        // Timeout
        );
    }

    /// Set channel.
    fn set_channel(&mut self, channel: u8) {
        // Calculate frequency
        let _freq = if channel <= 14 {
            // 2.4 GHz band
            2407 + (channel as u32) * 5
        } else {
            // 5 GHz band
            5000 + (channel as u32) * 5
        };

        // Channel setting involves PHY programming which is complex
        // This is simplified

        self.current_channel = channel;
    }

    /// Initialize the device.
    pub fn init(&mut self) {
        // Read chip info
        self.read_chip_info();

        // Reset
        self.reset();

        // Read MAC
        self.read_mac();

        // Initialize rings
        self.init_tx();
        self.init_rx();

        // Set default channel
        self.set_channel(1);

        // Enable interrupts
        self.enable_interrupts();

        self.initialized = true;
        self.powered_on = true;

        crate::kprintln!("brcm: {} (rev {}) initialized, MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            device_ids::name(self.device_id),
            self.chip_rev,
            self.mac[0], self.mac[1], self.mac[2],
            self.mac[3], self.mac[4], self.mac[5]);
    }

    /// Power off.
    pub fn power_off(&mut self) {
        if !self.initialized {
            return;
        }

        // Disable MAC
        self.write32(regs::MACCONTROL, 0);

        // Disable interrupts
        self.write32(regs::MACINTMASK, 0);
        self.write32(regs::INTR_MASK, 0);

        self.powered_on = false;
    }

    /// Power on.
    pub fn power_on(&mut self) {
        if !self.initialized {
            return;
        }

        // Enable MAC
        self.write32(regs::MACCONTROL, 0x1);

        // Enable interrupts
        self.enable_interrupts();

        self.powered_on = true;
    }

    /// Start a scan.
    pub fn start_scan(&mut self) {
        self.scan_results.clear();

        // Scan 2.4 GHz channels 1-11
        for channel in 1..=11u8 {
            self.set_channel(channel);
            // Wait for beacons
            for _ in 0..10000 {
                core::hint::spin_loop();
            }
            self.process_rx_for_scan();
        }

        // If device supports 5 GHz
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
            let ctrl = self.rx_ring[self.rx_index].ctrl1;

            // Check if frame available (SOF and EOF set)
            if ctrl & (d64ctl::D64_CTRL1_SOF | d64ctl::D64_CTRL1_EOF) == 0 {
                break;
            }

            let len = (self.rx_ring[self.rx_index].ctrl2 & d64ctl::D64_CTRL2_BC_MASK) as usize;
            let idx = self.rx_index;

            if len > 24 && len <= BUFFER_SIZE {
                // Copy frame data
                let mut frame_data = [0u8; BUFFER_SIZE];
                frame_data[..len].copy_from_slice(&self.rx_buffers[idx][..len]);
                let frame = &frame_data[..len];

                // Check if it's a beacon
                if frame.len() >= 2 && (frame[0] & 0x0C) == 0x00 && (frame[0] >> 4) == 8 {
                    if let Some(result) = self.parse_beacon(frame) {
                        if !self.scan_results.iter().any(|r| r.bssid == result.bssid) {
                            self.scan_results.push(result);
                        }
                    }
                }
            }

            // Reset descriptor
            self.rx_ring[self.rx_index].ctrl1 = 0;
            self.rx_ring[self.rx_index].ctrl2 = BUFFER_SIZE as u32;
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
            signal_strength: -50,
            encrypted,
        })
    }

    /// Handle interrupt.
    pub fn handle_interrupt(&mut self) {
        if !self.initialized {
            return;
        }

        let status = self.read32(regs::MACINTSTATUS);

        if status & intr::MI_RXOV != 0 {
            crate::kprintln!("brcm: RX overflow");
        }

        if status & intr::MI_MACTXERR != 0 {
            crate::kprintln!("brcm: TX error");
        }

        // Clear interrupts
        self.write32(regs::MACINTSTATUS, status);
    }

    /// Receive a frame.
    pub fn recv_frame(&mut self) -> Option<Vec<u8>> {
        if !self.initialized || !self.powered_on {
            return None;
        }

        let ctrl = self.rx_ring[self.rx_index].ctrl1;

        // Check if frame available
        if ctrl & (d64ctl::D64_CTRL1_SOF | d64ctl::D64_CTRL1_EOF) == 0 {
            return None;
        }

        let len = (self.rx_ring[self.rx_index].ctrl2 & d64ctl::D64_CTRL2_BC_MASK) as usize;
        if len == 0 || len > BUFFER_SIZE {
            self.rx_ring[self.rx_index].ctrl1 = 0;
            self.rx_ring[self.rx_index].ctrl2 = BUFFER_SIZE as u32;
            self.rx_index = (self.rx_index + 1) % RX_RING_SIZE;
            return None;
        }

        let mut frame = Vec::with_capacity(len);
        frame.extend_from_slice(&self.rx_buffers[self.rx_index][..len]);

        // Reset descriptor
        self.rx_ring[self.rx_index].ctrl1 = 0;
        self.rx_ring[self.rx_index].ctrl2 = BUFFER_SIZE as u32;
        self.rx_index = (self.rx_index + 1) % RX_RING_SIZE;

        Some(frame)
    }

    /// Get scan results.
    pub fn get_scan_results(&self) -> &[ScanResult] {
        &self.scan_results
    }
}

/// Probe PCI for Broadcom devices.
pub fn probe_pci() -> Option<(u64, u16)> {
    let devices = pci::scan();

    for dev in devices {
        if dev.id.vendor_id == BROADCOM_VENDOR_ID
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

/// Initialize the Broadcom driver.
pub fn init() {
    if let Some((mmio_base, device_id)) = probe_pci() {
        unsafe {
            let mut driver = Brcm::new(mmio_base, device_id);
            driver.init();
            BRCM_DRIVER = Some(driver);
        }
    }
}

/// Get MAC address.
pub fn get_mac() -> Option<[u8; 6]> {
    unsafe {
        BRCM_DRIVER.as_ref().map(|d| d.mac)
    }
}

/// Start a scan.
pub fn start_scan() {
    unsafe {
        if let Some(d) = BRCM_DRIVER.as_mut() {
            d.start_scan();
        }
    }
}

/// Get scan results.
pub fn get_scan_results() -> Vec<ScanResult> {
    unsafe {
        BRCM_DRIVER.as_ref().map_or(Vec::new(), |d| d.scan_results.clone())
    }
}

/// Get device name.
pub fn device_name() -> &'static str {
    unsafe {
        BRCM_DRIVER.as_ref().map_or("none", |d| device_ids::name(d.device_id))
    }
}

/// Check if initialized.
pub fn is_initialized() -> bool {
    unsafe {
        BRCM_DRIVER.as_ref().map_or(false, |d| d.initialized)
    }
}

/// Check if supports 5GHz.
pub fn supports_5ghz() -> bool {
    unsafe {
        BRCM_DRIVER.as_ref().map_or(false, |d| device_ids::supports_5ghz(d.device_id))
    }
}

/// Check if supports 802.11ac.
pub fn supports_11ac() -> bool {
    unsafe {
        BRCM_DRIVER.as_ref().map_or(false, |d| device_ids::supports_11ac(d.device_id))
    }
}
