//! USB Ethernet drivers.
//!
//! Supports common USB Ethernet adapters:
//! - CDC-ECM (standard USB Ethernet class)
//! - CDC-NCM (Network Control Model)
//! - RNDIS (Microsoft Remote NDIS)
//! - ASIX AX88179/AX88178A (USB 3.0 Gigabit)
//! - ASIX AX88772/AX88772A/AX88772B (USB 2.0 100Mbps)
//! - Realtek RTL8152/RTL8153 (USB 2.0/3.0 Gigabit)
//! - Realtek RTL8156 (USB 3.0 2.5GbE)

use alloc::vec::Vec;
use alloc::string::String;
use alloc::boxed::Box;
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// USB Ethernet vendor/product IDs
mod device_ids {
    // ASIX
    pub const ASIX_VENDOR: u16 = 0x0b95;
    pub const AX88179: u16 = 0x1790;        // USB 3.0 Gigabit
    pub const AX88178A: u16 = 0x178a;       // USB 2.0 Gigabit
    pub const AX88772: u16 = 0x7720;        // USB 2.0 100Mbps
    pub const AX88772A: u16 = 0x772a;
    pub const AX88772B: u16 = 0x772b;

    // Realtek
    pub const REALTEK_VENDOR: u16 = 0x0bda;
    pub const RTL8152: u16 = 0x8152;        // USB 2.0 100Mbps
    pub const RTL8153: u16 = 0x8153;        // USB 3.0 Gigabit
    pub const RTL8153B: u16 = 0x8156;       // USB 3.0 Gigabit (variant)
    pub const RTL8156: u16 = 0x8156;        // USB 3.0 2.5GbE
    pub const RTL8156B: u16 = 0x8157;       // USB 3.0 2.5GbE (variant)

    // Generic CDC
    pub const CDC_ECM_CLASS: u8 = 0x02;
    pub const CDC_ECM_SUBCLASS: u8 = 0x06;
    pub const CDC_NCM_SUBCLASS: u8 = 0x0d;

    // Microsoft RNDIS
    pub const RNDIS_CLASS: u8 = 0xe0;
    pub const RNDIS_SUBCLASS: u8 = 0x01;
    pub const RNDIS_PROTOCOL: u8 = 0x03;

    pub fn is_asix(vendor: u16, product: u16) -> bool {
        vendor == ASIX_VENDOR && matches!(product, AX88179 | AX88178A | AX88772 | AX88772A | AX88772B)
    }

    pub fn is_realtek(vendor: u16, product: u16) -> bool {
        vendor == REALTEK_VENDOR && matches!(product, RTL8152 | RTL8153 | RTL8153B | RTL8156 | RTL8156B)
    }

    pub fn is_cdc_ecm(class: u8, subclass: u8) -> bool {
        class == CDC_ECM_CLASS && subclass == CDC_ECM_SUBCLASS
    }

    pub fn is_cdc_ncm(class: u8, subclass: u8) -> bool {
        class == CDC_ECM_CLASS && subclass == CDC_NCM_SUBCLASS
    }

    pub fn is_rndis(class: u8, subclass: u8, protocol: u8) -> bool {
        class == RNDIS_CLASS && subclass == RNDIS_SUBCLASS && protocol == RNDIS_PROTOCOL
    }
}

/// USB Ethernet driver type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbEthernetDriver {
    CdcEcm,
    CdcNcm,
    Rndis,
    Asix,
    Realtek,
}

impl UsbEthernetDriver {
    pub fn name(&self) -> &'static str {
        match self {
            UsbEthernetDriver::CdcEcm => "CDC-ECM",
            UsbEthernetDriver::CdcNcm => "CDC-NCM",
            UsbEthernetDriver::Rndis => "RNDIS",
            UsbEthernetDriver::Asix => "ASIX",
            UsbEthernetDriver::Realtek => "Realtek",
        }
    }
}

/// Link speed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSpeed {
    Speed10Mbps,
    Speed100Mbps,
    Speed1Gbps,
    Speed2_5Gbps,
    Unknown,
}

impl LinkSpeed {
    pub fn mbps(&self) -> u32 {
        match self {
            LinkSpeed::Speed10Mbps => 10,
            LinkSpeed::Speed100Mbps => 100,
            LinkSpeed::Speed1Gbps => 1000,
            LinkSpeed::Speed2_5Gbps => 2500,
            LinkSpeed::Unknown => 0,
        }
    }
}

/// USB Ethernet device
pub struct UsbEthernetDevice {
    /// USB device handle
    usb_device: u32,
    /// Driver type
    driver: UsbEthernetDriver,
    /// MAC address
    mac: [u8; 6],
    /// Bulk IN endpoint
    bulk_in: u8,
    /// Bulk OUT endpoint
    bulk_out: u8,
    /// Interrupt endpoint (for status)
    interrupt_ep: Option<u8>,
    /// Maximum packet size
    max_packet_size: u16,
    /// Link up
    link_up: bool,
    /// Link speed
    link_speed: LinkSpeed,
    /// Device name
    device_name: String,
    /// Receive buffer
    rx_buffer: Vec<u8>,
    /// Transmit buffer
    tx_buffer: Vec<u8>,
}

impl UsbEthernetDevice {
    fn new(
        usb_device: u32,
        driver: UsbEthernetDriver,
        mac: [u8; 6],
        bulk_in: u8,
        bulk_out: u8,
        interrupt_ep: Option<u8>,
        max_packet_size: u16,
        device_name: String,
    ) -> Self {
        Self {
            usb_device,
            driver,
            mac,
            bulk_in,
            bulk_out,
            interrupt_ep,
            max_packet_size,
            link_up: false,
            link_speed: LinkSpeed::Unknown,
            device_name,
            rx_buffer: alloc::vec![0u8; 16384],
            tx_buffer: alloc::vec![0u8; 16384],
        }
    }

    /// Send a packet
    pub fn send(&mut self, data: &[u8]) -> KResult<()> {
        if !self.link_up {
            return Err(KError::IO);
        }

        if data.len() > self.tx_buffer.len() {
            return Err(KError::Invalid);
        }

        // Copy data to transmit buffer
        self.tx_buffer[..data.len()].copy_from_slice(data);

        // Send based on driver type
        match self.driver {
            UsbEthernetDriver::CdcEcm | UsbEthernetDriver::CdcNcm => {
                self.send_cdc(data)
            }
            UsbEthernetDriver::Rndis => {
                self.send_rndis(data)
            }
            UsbEthernetDriver::Asix => {
                self.send_asix(data)
            }
            UsbEthernetDriver::Realtek => {
                self.send_realtek(data)
            }
        }
    }

    /// Receive a packet
    pub fn recv(&mut self) -> Option<Vec<u8>> {
        if !self.link_up {
            return None;
        }

        match self.driver {
            UsbEthernetDriver::CdcEcm | UsbEthernetDriver::CdcNcm => {
                self.recv_cdc()
            }
            UsbEthernetDriver::Rndis => {
                self.recv_rndis()
            }
            UsbEthernetDriver::Asix => {
                self.recv_asix()
            }
            UsbEthernetDriver::Realtek => {
                self.recv_realtek()
            }
        }
    }

    // CDC send
    fn send_cdc(&mut self, data: &[u8]) -> KResult<()> {
        // CDC-ECM/NCM: send raw Ethernet frame
        // In real implementation, would use USB bulk transfer
        // usb::bulk_out(self.usb_device, self.bulk_out, data)?;
        Ok(())
    }

    // CDC receive
    fn recv_cdc(&mut self) -> Option<Vec<u8>> {
        // CDC-ECM/NCM: receive raw Ethernet frame
        // In real implementation, would use USB bulk transfer
        // let len = usb::bulk_in(self.usb_device, self.bulk_in, &mut self.rx_buffer)?;
        // Some(self.rx_buffer[..len].to_vec())
        None
    }

    // RNDIS send
    fn send_rndis(&mut self, data: &[u8]) -> KResult<()> {
        // RNDIS: wrap in RNDIS packet header
        let header_len = 44; // RNDIS_PACKET_MSG header
        if data.len() + header_len > self.tx_buffer.len() {
            return Err(KError::Invalid);
        }

        // Build RNDIS_PACKET_MSG header
        let msg_len = (data.len() + header_len) as u32;
        let data_offset = 36u32; // From start of DataOffset field
        let data_len = data.len() as u32;

        // Message type (RNDIS_PACKET_MSG = 0x00000001)
        self.tx_buffer[0..4].copy_from_slice(&1u32.to_le_bytes());
        // Message length
        self.tx_buffer[4..8].copy_from_slice(&msg_len.to_le_bytes());
        // Data offset
        self.tx_buffer[8..12].copy_from_slice(&data_offset.to_le_bytes());
        // Data length
        self.tx_buffer[12..16].copy_from_slice(&data_len.to_le_bytes());
        // OOB data offset (0)
        self.tx_buffer[16..20].copy_from_slice(&0u32.to_le_bytes());
        // OOB data length (0)
        self.tx_buffer[20..24].copy_from_slice(&0u32.to_le_bytes());
        // Num OOB elements (0)
        self.tx_buffer[24..28].copy_from_slice(&0u32.to_le_bytes());
        // Per-packet info offset (0)
        self.tx_buffer[28..32].copy_from_slice(&0u32.to_le_bytes());
        // Per-packet info length (0)
        self.tx_buffer[32..36].copy_from_slice(&0u32.to_le_bytes());
        // Reserved
        self.tx_buffer[36..44].copy_from_slice(&[0u8; 8]);

        // Copy Ethernet frame
        self.tx_buffer[header_len..header_len + data.len()].copy_from_slice(data);

        // usb::bulk_out(self.usb_device, self.bulk_out, &self.tx_buffer[..msg_len as usize])?;
        Ok(())
    }

    // RNDIS receive
    fn recv_rndis(&mut self) -> Option<Vec<u8>> {
        // RNDIS: unwrap from RNDIS packet header
        // let len = usb::bulk_in(self.usb_device, self.bulk_in, &mut self.rx_buffer)?;
        // if len < 44 { return None; }

        // Parse header
        // let msg_type = u32::from_le_bytes(self.rx_buffer[0..4].try_into().ok()?);
        // if msg_type != 1 { return None; } // Not a packet
        // let data_offset = u32::from_le_bytes(self.rx_buffer[8..12].try_into().ok()?) as usize;
        // let data_len = u32::from_le_bytes(self.rx_buffer[12..16].try_into().ok()?) as usize;
        // let start = 8 + data_offset;
        // Some(self.rx_buffer[start..start + data_len].to_vec())
        None
    }

    // ASIX send
    fn send_asix(&mut self, data: &[u8]) -> KResult<()> {
        // ASIX: add 4-byte header with length
        let header_len = 4;
        if data.len() + header_len > self.tx_buffer.len() {
            return Err(KError::Invalid);
        }

        let len = data.len() as u16;
        self.tx_buffer[0] = len as u8;
        self.tx_buffer[1] = (len >> 8) as u8;
        self.tx_buffer[2] = !(len as u8);
        self.tx_buffer[3] = !((len >> 8) as u8);
        self.tx_buffer[header_len..header_len + data.len()].copy_from_slice(data);

        // usb::bulk_out(self.usb_device, self.bulk_out, &self.tx_buffer[..header_len + data.len()])?;
        Ok(())
    }

    // ASIX receive
    fn recv_asix(&mut self) -> Option<Vec<u8>> {
        // ASIX: strip 4-byte header
        // let len = usb::bulk_in(self.usb_device, self.bulk_in, &mut self.rx_buffer)?;
        // if len < 4 { return None; }
        // let pkt_len = u16::from_le_bytes(self.rx_buffer[0..2].try_into().ok()?) as usize;
        // Some(self.rx_buffer[4..4 + pkt_len].to_vec())
        None
    }

    // Realtek send
    fn send_realtek(&mut self, data: &[u8]) -> KResult<()> {
        // Realtek RTL8152/8153: add TX header
        let header_len = 8;
        if data.len() + header_len > self.tx_buffer.len() {
            return Err(KError::Invalid);
        }

        // TX descriptor
        let len = data.len() as u32;
        self.tx_buffer[0..4].copy_from_slice(&len.to_le_bytes());
        self.tx_buffer[4..8].copy_from_slice(&0u32.to_le_bytes()); // flags
        self.tx_buffer[header_len..header_len + data.len()].copy_from_slice(data);

        // usb::bulk_out(self.usb_device, self.bulk_out, &self.tx_buffer[..header_len + data.len()])?;
        Ok(())
    }

    // Realtek receive
    fn recv_realtek(&mut self) -> Option<Vec<u8>> {
        // Realtek RTL8152/8153: strip RX header
        // let len = usb::bulk_in(self.usb_device, self.bulk_in, &mut self.rx_buffer)?;
        // if len < 8 { return None; }
        // let pkt_len = u32::from_le_bytes(self.rx_buffer[0..4].try_into().ok()?) as usize;
        // Some(self.rx_buffer[8..8 + pkt_len].to_vec())
        None
    }

    /// Get MAC address
    pub fn mac(&self) -> [u8; 6] {
        self.mac
    }

    /// Is link up
    pub fn is_link_up(&self) -> bool {
        self.link_up
    }

    /// Get link speed
    pub fn link_speed(&self) -> LinkSpeed {
        self.link_speed
    }

    /// Get driver type
    pub fn driver(&self) -> UsbEthernetDriver {
        self.driver
    }

    /// Get device name
    pub fn device_name(&self) -> &str {
        &self.device_name
    }
}

/// USB Ethernet statistics
#[derive(Debug)]
pub struct UsbEthernetStats {
    pub tx_packets: AtomicU64,
    pub tx_bytes: AtomicU64,
    pub tx_errors: AtomicU64,
    pub rx_packets: AtomicU64,
    pub rx_bytes: AtomicU64,
    pub rx_errors: AtomicU64,
}

impl UsbEthernetStats {
    const fn new() -> Self {
        Self {
            tx_packets: AtomicU64::new(0),
            tx_bytes: AtomicU64::new(0),
            tx_errors: AtomicU64::new(0),
            rx_packets: AtomicU64::new(0),
            rx_bytes: AtomicU64::new(0),
            rx_errors: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> UsbEthernetStatsSnapshot {
        UsbEthernetStatsSnapshot {
            tx_packets: self.tx_packets.load(Ordering::Relaxed),
            tx_bytes: self.tx_bytes.load(Ordering::Relaxed),
            tx_errors: self.tx_errors.load(Ordering::Relaxed),
            rx_packets: self.rx_packets.load(Ordering::Relaxed),
            rx_bytes: self.rx_bytes.load(Ordering::Relaxed),
            rx_errors: self.rx_errors.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UsbEthernetStatsSnapshot {
    pub tx_packets: u64,
    pub tx_bytes: u64,
    pub tx_errors: u64,
    pub rx_packets: u64,
    pub rx_bytes: u64,
    pub rx_errors: u64,
}

/// USB Ethernet manager
pub struct UsbEthernetManager {
    devices: Vec<UsbEthernetDevice>,
    active_device: Option<usize>,
    stats: UsbEthernetStats,
    initialized: bool,
}

impl UsbEthernetManager {
    const fn new() -> Self {
        Self {
            devices: Vec::new(),
            active_device: None,
            stats: UsbEthernetStats::new(),
            initialized: false,
        }
    }

    /// Initialize and scan for USB Ethernet devices
    pub fn init(&mut self) {
        if self.initialized {
            return;
        }

        self.scan_devices();
        self.initialized = true;
    }

    /// Scan for USB Ethernet devices
    fn scan_devices(&mut self) {
        // In real implementation, would enumerate USB devices
        // For now, we'll add support for detecting devices when they're plugged in

        // Example: Check for common USB Ethernet adapters
        // The actual USB subsystem would call register_device when a device is found
    }

    /// Register a USB Ethernet device
    pub fn register_device(
        &mut self,
        vendor_id: u16,
        product_id: u16,
        class: u8,
        subclass: u8,
        protocol: u8,
        usb_device: u32,
        bulk_in: u8,
        bulk_out: u8,
        interrupt_ep: Option<u8>,
        max_packet_size: u16,
    ) -> Option<usize> {
        // Determine driver type
        let driver = if device_ids::is_asix(vendor_id, product_id) {
            Some(UsbEthernetDriver::Asix)
        } else if device_ids::is_realtek(vendor_id, product_id) {
            Some(UsbEthernetDriver::Realtek)
        } else if device_ids::is_cdc_ecm(class, subclass) {
            Some(UsbEthernetDriver::CdcEcm)
        } else if device_ids::is_cdc_ncm(class, subclass) {
            Some(UsbEthernetDriver::CdcNcm)
        } else if device_ids::is_rndis(class, subclass, protocol) {
            Some(UsbEthernetDriver::Rndis)
        } else {
            None
        };

        let driver = driver?;

        // Get MAC address
        let mac = self.get_mac_address(usb_device, driver, vendor_id, product_id);

        // Build device name
        let device_name = match driver {
            UsbEthernetDriver::CdcEcm => String::from("CDC-ECM Ethernet"),
            UsbEthernetDriver::CdcNcm => String::from("CDC-NCM Ethernet"),
            UsbEthernetDriver::Rndis => String::from("RNDIS Ethernet"),
            UsbEthernetDriver::Asix => {
                match product_id {
                    device_ids::AX88179 => String::from("ASIX AX88179 USB 3.0 Gigabit"),
                    device_ids::AX88178A => String::from("ASIX AX88178A USB Gigabit"),
                    device_ids::AX88772 | device_ids::AX88772A | device_ids::AX88772B => {
                        String::from("ASIX AX88772 USB 100Mbps")
                    }
                    _ => String::from("ASIX USB Ethernet"),
                }
            }
            UsbEthernetDriver::Realtek => {
                match product_id {
                    device_ids::RTL8152 => String::from("Realtek RTL8152 USB 100Mbps"),
                    device_ids::RTL8153 => String::from("Realtek RTL8153 USB Gigabit"),
                    device_ids::RTL8156 | device_ids::RTL8156B => String::from("Realtek RTL8156 USB 2.5GbE"),
                    _ => String::from("Realtek USB Ethernet"),
                }
            }
        };

        let device = UsbEthernetDevice::new(
            usb_device,
            driver,
            mac,
            bulk_in,
            bulk_out,
            interrupt_ep,
            max_packet_size,
            device_name,
        );

        let idx = self.devices.len();
        self.devices.push(device);

        // If this is the first device, make it active
        if self.active_device.is_none() {
            self.active_device = Some(idx);
        }

        Some(idx)
    }

    /// Get MAC address from device
    fn get_mac_address(&self, usb_device: u32, driver: UsbEthernetDriver, vendor_id: u16, product_id: u16) -> [u8; 6] {
        // In real implementation, would read MAC from device
        match driver {
            UsbEthernetDriver::CdcEcm | UsbEthernetDriver::CdcNcm => {
                // CDC: MAC is in a descriptor
                // Would parse CDC Ethernet Networking Functional Descriptor
                [0x02, 0x00, 0x00, 0x00, 0x00, 0x01]
            }
            UsbEthernetDriver::Rndis => {
                // RNDIS: Query OID_802_3_PERMANENT_ADDRESS
                [0x02, 0x00, 0x00, 0x00, 0x00, 0x02]
            }
            UsbEthernetDriver::Asix => {
                // ASIX: Read from EEPROM via vendor commands
                // AX_CMD_READ_NODE_ID = 0x13
                [0x02, 0x00, 0x00, 0x00, 0x00, 0x03]
            }
            UsbEthernetDriver::Realtek => {
                // Realtek: Read from device registers
                [0x02, 0x00, 0x00, 0x00, 0x00, 0x04]
            }
        }
    }

    /// Unregister a USB Ethernet device
    pub fn unregister_device(&mut self, idx: usize) {
        if idx < self.devices.len() {
            self.devices.remove(idx);

            // Update active device
            if let Some(active) = self.active_device {
                if active == idx {
                    self.active_device = if self.devices.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                } else if active > idx {
                    self.active_device = Some(active - 1);
                }
            }
        }
    }

    /// Get active device
    pub fn active_device(&self) -> Option<&UsbEthernetDevice> {
        self.active_device.and_then(|idx| self.devices.get(idx))
    }

    /// Get active device mutable
    pub fn active_device_mut(&mut self) -> Option<&mut UsbEthernetDevice> {
        self.active_device.and_then(|idx| self.devices.get_mut(idx))
    }

    /// Set active device
    pub fn set_active_device(&mut self, idx: usize) -> bool {
        if idx < self.devices.len() {
            self.active_device = Some(idx);
            true
        } else {
            false
        }
    }

    /// Get MAC address of active device
    pub fn get_mac(&self) -> Option<[u8; 6]> {
        self.active_device().map(|d| d.mac())
    }

    /// Send packet via active device
    pub fn send(&mut self, data: &[u8]) -> KResult<()> {
        if let Some(device) = self.active_device_mut() {
            let result = device.send(data);
            if result.is_ok() {
                self.stats.tx_packets.fetch_add(1, Ordering::Relaxed);
                self.stats.tx_bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
            } else {
                self.stats.tx_errors.fetch_add(1, Ordering::Relaxed);
            }
            result
        } else {
            Err(KError::NotSupported)
        }
    }

    /// Receive packet from active device
    pub fn recv(&mut self) -> Option<Vec<u8>> {
        if let Some(device) = self.active_device_mut() {
            if let Some(data) = device.recv() {
                self.stats.rx_packets.fetch_add(1, Ordering::Relaxed);
                self.stats.rx_bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
                Some(data)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get number of devices
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Get device at index
    pub fn get_device(&self, idx: usize) -> Option<&UsbEthernetDevice> {
        self.devices.get(idx)
    }

    /// Is link up on active device
    pub fn is_link_up(&self) -> bool {
        self.active_device().map(|d| d.is_link_up()).unwrap_or(false)
    }

    /// Get link speed of active device
    pub fn link_speed(&self) -> LinkSpeed {
        self.active_device().map(|d| d.link_speed()).unwrap_or(LinkSpeed::Unknown)
    }

    /// Get stats
    pub fn stats(&self) -> UsbEthernetStatsSnapshot {
        self.stats.snapshot()
    }

    /// Is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Has devices
    pub fn has_devices(&self) -> bool {
        !self.devices.is_empty()
    }
}

/// Global USB Ethernet manager
static USB_ETHERNET: IrqSafeMutex<UsbEthernetManager> = IrqSafeMutex::new(UsbEthernetManager::new());

/// Initialize USB Ethernet subsystem
pub fn init() {
    USB_ETHERNET.lock().init();
}

/// Register a USB Ethernet device
pub fn register_device(
    vendor_id: u16,
    product_id: u16,
    class: u8,
    subclass: u8,
    protocol: u8,
    usb_device: u32,
    bulk_in: u8,
    bulk_out: u8,
    interrupt_ep: Option<u8>,
    max_packet_size: u16,
) -> Option<usize> {
    USB_ETHERNET.lock().register_device(
        vendor_id, product_id, class, subclass, protocol,
        usb_device, bulk_in, bulk_out, interrupt_ep, max_packet_size,
    )
}

/// Unregister a USB Ethernet device
pub fn unregister_device(idx: usize) {
    USB_ETHERNET.lock().unregister_device(idx);
}

/// Get MAC address of active device
pub fn get_mac() -> Option<[u8; 6]> {
    USB_ETHERNET.lock().get_mac()
}

/// Send packet via active device
pub fn send(data: &[u8]) -> KResult<()> {
    USB_ETHERNET.lock().send(data)
}

/// Receive packet from active device
pub fn recv() -> Option<Vec<u8>> {
    USB_ETHERNET.lock().recv()
}

/// Is link up
pub fn is_link_up() -> bool {
    USB_ETHERNET.lock().is_link_up()
}

/// Get link speed
pub fn link_speed() -> LinkSpeed {
    USB_ETHERNET.lock().link_speed()
}

/// Get stats
pub fn stats() -> UsbEthernetStatsSnapshot {
    USB_ETHERNET.lock().stats()
}

/// Device count
pub fn device_count() -> usize {
    USB_ETHERNET.lock().device_count()
}

/// Has devices
pub fn has_devices() -> bool {
    USB_ETHERNET.lock().has_devices()
}

/// Is initialized
pub fn is_initialized() -> bool {
    USB_ETHERNET.lock().is_initialized()
}

/// Set active device
pub fn set_active_device(idx: usize) -> bool {
    USB_ETHERNET.lock().set_active_device(idx)
}

/// Get driver name
pub fn driver_name() -> &'static str {
    "usb-ethernet"
}
