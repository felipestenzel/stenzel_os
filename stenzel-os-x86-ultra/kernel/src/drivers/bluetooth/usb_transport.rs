//! USB Bluetooth Transport
//!
//! Implements HCI transport over USB for Bluetooth adapters.

extern crate alloc;

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use crate::drivers::usb;

/// USB Bluetooth device IDs
pub mod device_ids {
    // Common Bluetooth USB VID/PIDs

    // Intel
    pub const INTEL_VID: u16 = 0x8087;
    pub const INTEL_7260: u16 = 0x07DC;
    pub const INTEL_7265: u16 = 0x0A2A;
    pub const INTEL_8260: u16 = 0x0A2B;
    pub const INTEL_8265: u16 = 0x0AAA;
    pub const INTEL_9260: u16 = 0x0025;
    pub const INTEL_9560: u16 = 0x0029;
    pub const INTEL_AX200: u16 = 0x0026;
    pub const INTEL_AX201: u16 = 0x0032;
    pub const INTEL_AX210: u16 = 0x0033;

    // Broadcom
    pub const BROADCOM_VID: u16 = 0x0A5C;
    pub const BCM2035: u16 = 0x2035;
    pub const BCM2045: u16 = 0x2045;
    pub const BCM2046: u16 = 0x2046;
    pub const BCM2070: u16 = 0x2070;
    pub const BCM43142: u16 = 0x21E8;

    // Qualcomm Atheros
    pub const ATHEROS_VID: u16 = 0x0CF3;
    pub const ATH3011: u16 = 0x3002;
    pub const ATH3012: u16 = 0x3004;
    pub const ATH9271: u16 = 0x9271;
    pub const QCA6174: u16 = 0xE300;
    pub const QCA9377: u16 = 0xE360;

    // Realtek
    pub const REALTEK_VID: u16 = 0x0BDA;
    pub const RTL8723A: u16 = 0x0723;
    pub const RTL8723B: u16 = 0xB723;
    pub const RTL8761A: u16 = 0x8761;
    pub const RTL8821A: u16 = 0x0821;
    pub const RTL8822B: u16 = 0xB82C;

    // MediaTek
    pub const MEDIATEK_VID: u16 = 0x0E8D;
    pub const MT7921: u16 = 0x0608;

    // Cambridge Silicon Radio (CSR)
    pub const CSR_VID: u16 = 0x0A12;
    pub const CSR_BLUETOOTH: u16 = 0x0001;

    // Generic Bluetooth USB class
    pub const BT_CLASS: u8 = 0xE0;
    pub const BT_SUBCLASS: u8 = 0x01;
    pub const BT_PROTOCOL: u8 = 0x01;
}

/// USB endpoint types for Bluetooth
pub mod endpoints {
    pub const CONTROL: u8 = 0x00;
    pub const INTERRUPT_IN: u8 = 0x81;   // Events
    pub const BULK_OUT: u8 = 0x02;       // ACL TX
    pub const BULK_IN: u8 = 0x82;        // ACL RX
    pub const ISOCH_OUT: u8 = 0x03;      // SCO TX
    pub const ISOCH_IN: u8 = 0x83;       // SCO RX
}

/// USB HCI packet indicator bytes (H4)
pub mod h4 {
    pub const COMMAND: u8 = 0x01;
    pub const ACL_DATA: u8 = 0x02;
    pub const SCO_DATA: u8 = 0x03;
    pub const EVENT: u8 = 0x04;
    pub const ISO_DATA: u8 = 0x05;
}

/// USB Bluetooth transport state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportState {
    Disconnected,
    Connected,
    Initializing,
    Ready,
    Error,
}

/// USB Bluetooth transport
pub struct UsbBluetoothTransport {
    slot_id: u8,
    interface_number: u8,
    state: TransportState,
    event_endpoint: u8,
    acl_in_endpoint: u8,
    acl_out_endpoint: u8,
    sco_in_endpoint: u8,
    sco_out_endpoint: u8,
    rx_queue: VecDeque<Vec<u8>>,
    vendor_id: u16,
    product_id: u16,
}

impl UsbBluetoothTransport {
    pub fn new(slot_id: u8, interface_number: u8, vendor_id: u16, product_id: u16) -> Self {
        Self {
            slot_id,
            interface_number,
            state: TransportState::Disconnected,
            event_endpoint: endpoints::INTERRUPT_IN,
            acl_in_endpoint: endpoints::BULK_IN,
            acl_out_endpoint: endpoints::BULK_OUT,
            sco_in_endpoint: endpoints::ISOCH_IN,
            sco_out_endpoint: endpoints::ISOCH_OUT,
            rx_queue: VecDeque::new(),
            vendor_id,
            product_id,
        }
    }

    /// Initialize the transport
    pub fn init(&mut self) -> Result<(), &'static str> {
        self.state = TransportState::Initializing;

        // For some adapters, we may need to load firmware
        if self.needs_firmware() {
            self.load_firmware()?;
        }

        self.state = TransportState::Ready;
        Ok(())
    }

    /// Check if adapter needs firmware loading
    fn needs_firmware(&self) -> bool {
        // Intel adapters often need firmware
        if self.vendor_id == device_ids::INTEL_VID {
            return true;
        }

        // Some Broadcom/Realtek also need firmware
        if self.vendor_id == device_ids::BROADCOM_VID {
            return true;
        }

        if self.vendor_id == device_ids::REALTEK_VID {
            return true;
        }

        false
    }

    /// Load firmware (placeholder - actual implementation would read from files)
    fn load_firmware(&mut self) -> Result<(), &'static str> {
        // Firmware loading is vendor-specific
        // Intel: Uses HCI vendor commands to send firmware
        // Broadcom: Uses vendor-specific protocol
        // Realtek: Uses vendor-specific protocol

        crate::kprintln!("bluetooth: firmware loading not yet implemented for {:04X}:{:04X}",
            self.vendor_id, self.product_id);

        // For now, assume firmware is already loaded (built into adapter)
        Ok(())
    }

    /// Send HCI command via USB control transfer
    pub fn send_command(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if self.state != TransportState::Ready && self.state != TransportState::Initializing {
            return Err("Transport not ready");
        }

        // Skip H4 packet indicator if present
        let cmd_data = if data.first() == Some(&h4::COMMAND) {
            &data[1..]
        } else {
            data
        };

        // USB Bluetooth uses control transfer for HCI commands
        // bmRequestType: 0x20 (host-to-device, class, interface)
        // bRequest: 0x00
        // wValue: 0x0000
        // wIndex: interface number
        // wLength: command length

        // In a full implementation, this would call the USB driver
        // to perform a control transfer to the device
        crate::kprintln!("bluetooth: send command {} bytes to slot {}", cmd_data.len(), self.slot_id);

        Ok(())
    }

    /// Send ACL data via USB bulk transfer
    pub fn send_acl(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if self.state != TransportState::Ready {
            return Err("Transport not ready");
        }

        // Skip H4 packet indicator if present
        let acl_data = if data.first() == Some(&h4::ACL_DATA) {
            &data[1..]
        } else {
            data
        };

        // USB Bluetooth uses bulk OUT endpoint for ACL data
        crate::kprintln!("bluetooth: send ACL {} bytes to slot {}", acl_data.len(), self.slot_id);

        Ok(())
    }

    /// Send SCO data via USB isochronous transfer
    pub fn send_sco(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if self.state != TransportState::Ready {
            return Err("Transport not ready");
        }

        // Skip H4 packet indicator if present
        let sco_data = if data.first() == Some(&h4::SCO_DATA) {
            &data[1..]
        } else {
            data
        };

        // USB Bluetooth uses isochronous OUT endpoint for SCO data
        crate::kprintln!("bluetooth: send SCO {} bytes to slot {}", sco_data.len(), self.slot_id);

        Ok(())
    }

    /// Receive HCI event (from interrupt IN endpoint)
    pub fn receive_event(&mut self) -> Option<Vec<u8>> {
        // In a full implementation, this would read from the USB interrupt endpoint
        // For now, check our internal queue
        self.rx_queue.pop_front()
    }

    /// Receive ACL data (from bulk IN endpoint)
    pub fn receive_acl(&mut self) -> Option<Vec<u8>> {
        // In a full implementation, this would read from the USB bulk IN endpoint
        None
    }

    /// Queue received data
    pub fn queue_received(&mut self, data: Vec<u8>) {
        self.rx_queue.push_back(data);
    }

    /// Poll for incoming data
    pub fn poll(&mut self) -> bool {
        // In a full implementation, this would check USB endpoints for data
        !self.rx_queue.is_empty()
    }

    /// Get transport state
    pub fn state(&self) -> TransportState {
        self.state
    }

    /// Close transport
    pub fn close(&mut self) {
        self.state = TransportState::Disconnected;
        self.rx_queue.clear();
    }
}

/// Check if USB device is a Bluetooth adapter
pub fn is_bluetooth_device(class: u8, subclass: u8, protocol: u8) -> bool {
    class == device_ids::BT_CLASS &&
    subclass == device_ids::BT_SUBCLASS &&
    protocol == device_ids::BT_PROTOCOL
}

/// Check if device is a known Bluetooth adapter by VID/PID
pub fn is_known_bluetooth_adapter(vendor_id: u16, product_id: u16) -> bool {
    match vendor_id {
        device_ids::INTEL_VID => matches!(
            product_id,
            device_ids::INTEL_7260 |
            device_ids::INTEL_7265 |
            device_ids::INTEL_8260 |
            device_ids::INTEL_8265 |
            device_ids::INTEL_9260 |
            device_ids::INTEL_9560 |
            device_ids::INTEL_AX200 |
            device_ids::INTEL_AX201 |
            device_ids::INTEL_AX210
        ),
        device_ids::BROADCOM_VID => matches!(
            product_id,
            device_ids::BCM2035 |
            device_ids::BCM2045 |
            device_ids::BCM2046 |
            device_ids::BCM2070 |
            device_ids::BCM43142
        ),
        device_ids::ATHEROS_VID => matches!(
            product_id,
            device_ids::ATH3011 |
            device_ids::ATH3012 |
            device_ids::ATH9271 |
            device_ids::QCA6174 |
            device_ids::QCA9377
        ),
        device_ids::REALTEK_VID => matches!(
            product_id,
            device_ids::RTL8723A |
            device_ids::RTL8723B |
            device_ids::RTL8761A |
            device_ids::RTL8821A |
            device_ids::RTL8822B
        ),
        device_ids::CSR_VID => product_id == device_ids::CSR_BLUETOOTH,
        device_ids::MEDIATEK_VID => product_id == device_ids::MT7921,
        _ => false,
    }
}

/// Get Bluetooth adapter name from VID/PID
pub fn get_adapter_name(vendor_id: u16, product_id: u16) -> &'static str {
    match vendor_id {
        device_ids::INTEL_VID => match product_id {
            device_ids::INTEL_7260 => "Intel Wireless 7260",
            device_ids::INTEL_7265 => "Intel Wireless 7265",
            device_ids::INTEL_8260 => "Intel Wireless 8260",
            device_ids::INTEL_8265 => "Intel Wireless 8265",
            device_ids::INTEL_9260 => "Intel Wireless 9260",
            device_ids::INTEL_9560 => "Intel Wireless 9560",
            device_ids::INTEL_AX200 => "Intel AX200",
            device_ids::INTEL_AX201 => "Intel AX201",
            device_ids::INTEL_AX210 => "Intel AX210",
            _ => "Intel Bluetooth",
        },
        device_ids::BROADCOM_VID => match product_id {
            device_ids::BCM2035 => "Broadcom BCM2035",
            device_ids::BCM2045 => "Broadcom BCM2045",
            device_ids::BCM2046 => "Broadcom BCM2046",
            device_ids::BCM2070 => "Broadcom BCM2070",
            device_ids::BCM43142 => "Broadcom BCM43142",
            _ => "Broadcom Bluetooth",
        },
        device_ids::ATHEROS_VID => match product_id {
            device_ids::ATH3011 => "Qualcomm Atheros AR3011",
            device_ids::ATH3012 => "Qualcomm Atheros AR3012",
            device_ids::ATH9271 => "Qualcomm Atheros AR9271",
            device_ids::QCA6174 => "Qualcomm QCA6174",
            device_ids::QCA9377 => "Qualcomm QCA9377",
            _ => "Qualcomm Atheros Bluetooth",
        },
        device_ids::REALTEK_VID => match product_id {
            device_ids::RTL8723A => "Realtek RTL8723A",
            device_ids::RTL8723B => "Realtek RTL8723B",
            device_ids::RTL8761A => "Realtek RTL8761A",
            device_ids::RTL8821A => "Realtek RTL8821A",
            device_ids::RTL8822B => "Realtek RTL8822B",
            _ => "Realtek Bluetooth",
        },
        device_ids::CSR_VID => "CSR Bluetooth",
        device_ids::MEDIATEK_VID => match product_id {
            device_ids::MT7921 => "MediaTek MT7921",
            _ => "MediaTek Bluetooth",
        },
        _ => "Unknown Bluetooth Adapter",
    }
}

/// Scan for USB Bluetooth adapters
pub fn scan_usb_adapters() {
    crate::kprintln!("bluetooth: scanning for USB adapters");

    // In a full implementation, this would:
    // 1. Query the USB subsystem for all connected devices
    // 2. Check each device's class/subclass/protocol
    // 3. Check VID/PID against known adapters
    // 4. Create UsbBluetoothTransport for each adapter found
    // 5. Register with the BluetoothManager

    // For now, we'll create a placeholder that doesn't find any devices
    // since we don't have actual USB enumeration integrated yet

    let devices = usb::list_devices();
    for (id, info) in devices.iter().enumerate() {
        if is_bluetooth_device(info.device_class, info.device_subclass, info.device_protocol) ||
           is_known_bluetooth_adapter(info.vendor_id, info.product_id) {

            crate::kprintln!("bluetooth: found adapter {:04X}:{:04X} - {}",
                info.vendor_id,
                info.product_id,
                get_adapter_name(info.vendor_id, info.product_id)
            );

            // Create transport and register with manager
            let transport = UsbBluetoothTransport::new(
                info.slot_id,
                0, // Interface number
                info.vendor_id,
                info.product_id,
            );

            let mut manager = super::BLUETOOTH_MANAGER.lock();
            manager.register_controller(transport);
        }
    }
}
