//! USB subsystem.
//!
//! Suporta:
//! - EHCI (USB 2.0) para hardware com controladores EHCI
//! - xHCI (USB 3.x) para hardware moderno
//! - Unified device enumeration and management
//!
//! NOTA: Este módulo está preparado para features futuras.

#![allow(dead_code)]

pub mod audio;
pub mod devices;
pub mod ehci;
pub mod hid;
pub mod hub;
pub mod ohci;
pub mod storage;
pub mod uhci;
pub mod video;
pub mod xhci;

// Re-export device management
pub use devices::{
    UsbDeviceManager, UsbDeviceInfo, UsbDeviceId, UsbDeviceState,
    UsbInterface, UsbConfiguration, ControllerType, UsbEvent, UsbStats,
    list_devices, find_by_class, find_by_ids, find_by_interface_class,
    format_all_devices, format_stats, device_count,
};


/// Velocidade USB
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbSpeed {
    Low,       // 1.5 Mbps (USB 1.0)
    Full,      // 12 Mbps (USB 1.1)
    High,      // 480 Mbps (USB 2.0)
    Super,     // 5 Gbps (USB 3.0)
    SuperPlus, // 10 Gbps (USB 3.1)
}

impl UsbSpeed {
    pub fn from_xhci_speed(speed: u8) -> Self {
        match speed {
            1 => UsbSpeed::Full,
            2 => UsbSpeed::Low,
            3 => UsbSpeed::High,
            4 => UsbSpeed::Super,
            5 => UsbSpeed::SuperPlus,
            _ => UsbSpeed::Full,
        }
    }
}

/// Tipos de endpoint USB
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointType {
    Control,
    Isochronous,
    Bulk,
    Interrupt,
}

/// Direção do endpoint
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointDirection {
    Out, // Host -> Device
    In,  // Device -> Host
}

/// Descritor de dispositivo USB (primeiro descritor retornado)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DeviceDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub usb_version: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
    pub max_packet_size0: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_version: u16,
    pub manufacturer_index: u8,
    pub product_index: u8,
    pub serial_index: u8,
    pub num_configurations: u8,
}

/// Descritor de configuração USB
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ConfigDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub total_length: u16,
    pub num_interfaces: u8,
    pub configuration_value: u8,
    pub configuration_index: u8,
    pub attributes: u8,
    pub max_power: u8,
}

/// Descritor de interface USB
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct InterfaceDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub interface_number: u8,
    pub alternate_setting: u8,
    pub num_endpoints: u8,
    pub interface_class: u8,
    pub interface_subclass: u8,
    pub interface_protocol: u8,
    pub interface_index: u8,
}

/// Descritor de endpoint USB
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct EndpointDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub endpoint_address: u8,
    pub attributes: u8,
    pub max_packet_size: u16,
    pub interval: u8,
}

impl EndpointDescriptor {
    pub fn endpoint_number(&self) -> u8 {
        self.endpoint_address & 0x0F
    }

    pub fn direction(&self) -> EndpointDirection {
        if self.endpoint_address & 0x80 != 0 {
            EndpointDirection::In
        } else {
            EndpointDirection::Out
        }
    }

    pub fn transfer_type(&self) -> EndpointType {
        match self.attributes & 0x03 {
            0 => EndpointType::Control,
            1 => EndpointType::Isochronous,
            2 => EndpointType::Bulk,
            3 => EndpointType::Interrupt,
            _ => EndpointType::Control,
        }
    }
}

/// Tipos de request USB
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum RequestType {
    Standard = 0,
    Class = 1,
    Vendor = 2,
}

/// Requests USB padrão
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum StandardRequest {
    GetStatus = 0,
    ClearFeature = 1,
    SetFeature = 3,
    SetAddress = 5,
    GetDescriptor = 6,
    SetDescriptor = 7,
    GetConfiguration = 8,
    SetConfiguration = 9,
    GetInterface = 10,
    SetInterface = 11,
    SynchFrame = 12,
}

/// Tipos de descritores USB
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum DescriptorType {
    Device = 1,
    Configuration = 2,
    String = 3,
    Interface = 4,
    Endpoint = 5,
    DeviceQualifier = 6,
    OtherSpeedConfig = 7,
    InterfacePower = 8,
    Otg = 9,
    Debug = 10,
    InterfaceAssociation = 11,
    Bos = 15,
    DeviceCapability = 16,
    Hid = 0x21,
    Report = 0x22,
    Physical = 0x23,
    Hub = 0x29,
    SuperSpeedHub = 0x2A,
    SsEndpointCompanion = 48,
}

/// Setup packet USB (8 bytes)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct SetupPacket {
    pub request_type: u8,
    pub request: u8,
    pub value: u16,
    pub index: u16,
    pub length: u16,
}

impl SetupPacket {
    pub fn get_descriptor(desc_type: DescriptorType, index: u8, length: u16) -> Self {
        Self {
            request_type: 0x80, // Device to Host, Standard, Device
            request: StandardRequest::GetDescriptor as u8,
            value: ((desc_type as u16) << 8) | (index as u16),
            index: 0,
            length,
        }
    }

    pub fn set_address(address: u8) -> Self {
        Self {
            request_type: 0x00, // Host to Device, Standard, Device
            request: StandardRequest::SetAddress as u8,
            value: address as u16,
            index: 0,
            length: 0,
        }
    }

    pub fn set_configuration(config: u8) -> Self {
        Self {
            request_type: 0x00,
            request: StandardRequest::SetConfiguration as u8,
            value: config as u16,
            index: 0,
            length: 0,
        }
    }
}

/// Representa um dispositivo USB enumerado
#[derive(Debug)]
pub struct UsbDevice {
    pub slot_id: u8,
    pub speed: UsbSpeed,
    pub address: u8,
    pub port: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
}

/// Classes USB comuns
pub mod class {
    pub const INTERFACE_CLASS: u8 = 0x00;
    pub const AUDIO: u8 = 0x01;
    pub const CDC: u8 = 0x02;
    pub const HID: u8 = 0x03;
    pub const PHYSICAL: u8 = 0x05;
    pub const IMAGE: u8 = 0x06;
    pub const PRINTER: u8 = 0x07;
    pub const MASS_STORAGE: u8 = 0x08;
    pub const HUB: u8 = 0x09;
    pub const CDC_DATA: u8 = 0x0A;
    pub const SMART_CARD: u8 = 0x0B;
    pub const CONTENT_SECURITY: u8 = 0x0D;
    pub const VIDEO: u8 = 0x0E;
    pub const HEALTHCARE: u8 = 0x0F;
    pub const AUDIO_VIDEO: u8 = 0x10;
    pub const BILLBOARD: u8 = 0x11;
    pub const TYPE_C_BRIDGE: u8 = 0x12;
    pub const DIAGNOSTIC: u8 = 0xDC;
    pub const WIRELESS: u8 = 0xE0;
    pub const MISC: u8 = 0xEF;
    pub const APPLICATION: u8 = 0xFE;
    pub const VENDOR: u8 = 0xFF;
}

/// Subclasses de Mass Storage
pub mod mass_storage {
    pub const SCSI: u8 = 0x06;
    pub const BULK_ONLY: u8 = 0x50;
}

/// Inicializa o subsistema USB
pub fn init() {
    crate::kprintln!("usb: inicializando...");
    // Initialize device manager first
    devices::init();
    // Initialize EHCI first (USB 2.0)
    ehci::init();
    // Then xHCI (USB 3.x)
    xhci::init();
    // Initialize device drivers
    hid::init();
    hub::init();
    storage::init();
    audio::init();
    video::init();
}
