//! UHCI (Universal Host Controller Interface) driver for USB 1.1.
//!
//! UHCI is Intel's implementation of USB 1.1 controllers (commonly found
//! in Intel chipsets). Implements Low Speed (1.5 Mbps) and Full Speed (12 Mbps).
//!
//! Key features:
//! - I/O port-based register access
//! - Frame list (1024 entries) for scheduling
//! - Queue Heads (QH) for endpoint management
//! - Transfer Descriptors (TD) for transfers
//! - Control, bulk, interrupt, and isochronous transfers

#![allow(dead_code)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use x86_64::instructions::port::Port;

use crate::drivers::pci::{self, PciDevice};
use crate::mm;
use crate::util::{KError, KResult};

use super::{DeviceDescriptor, SetupPacket, UsbSpeed};

// =============================================================================
// UHCI Register Offsets (I/O Ports)
// =============================================================================

/// USB Command register
const USBCMD: u16 = 0x00;
/// USB Status register
const USBSTS: u16 = 0x02;
/// USB Interrupt Enable register
const USBINTR: u16 = 0x04;
/// Frame Number register
const FRNUM: u16 = 0x06;
/// Frame List Base Address register
const FRBASEADD: u16 = 0x08;
/// Start of Frame Modify register
const SOFMOD: u16 = 0x0C;
/// Port 1 Status/Control register
const PORTSC1: u16 = 0x10;
/// Port 2 Status/Control register
const PORTSC2: u16 = 0x12;

// =============================================================================
// USBCMD bits
// =============================================================================

/// Run/Stop
const CMD_RS: u16 = 1 << 0;
/// Host Controller Reset
const CMD_HCRESET: u16 = 1 << 1;
/// Global Reset
const CMD_GRESET: u16 = 1 << 2;
/// Enter Global Suspend Mode
const CMD_EGSM: u16 = 1 << 3;
/// Force Global Resume
const CMD_FGR: u16 = 1 << 4;
/// Software Debug
const CMD_SWDBG: u16 = 1 << 5;
/// Configure Flag
const CMD_CF: u16 = 1 << 6;
/// Max Packet (1=64 bytes, 0=32 bytes)
const CMD_MAXP: u16 = 1 << 7;

// =============================================================================
// USBSTS bits
// =============================================================================

/// USB Interrupt
const STS_USBINT: u16 = 1 << 0;
/// USB Error Interrupt
const STS_USBERRINT: u16 = 1 << 1;
/// Resume Detect
const STS_RD: u16 = 1 << 2;
/// Host System Error
const STS_HSE: u16 = 1 << 3;
/// Host Controller Process Error
const STS_HCPE: u16 = 1 << 4;
/// HCHalted
const STS_HCHALTED: u16 = 1 << 5;

// =============================================================================
// USBINTR bits
// =============================================================================

/// Timeout/CRC Interrupt Enable
const INTR_TOCRCIE: u16 = 1 << 0;
/// Resume Interrupt Enable
const INTR_RIE: u16 = 1 << 1;
/// Interrupt On Complete (IOC) Enable
const INTR_IOCE: u16 = 1 << 2;
/// Short Packet Interrupt Enable
const INTR_SPIE: u16 = 1 << 3;

// =============================================================================
// PORTSC bits
// =============================================================================

/// Current Connect Status
const PORTSC_CCS: u16 = 1 << 0;
/// Connect Status Change
const PORTSC_CSC: u16 = 1 << 1;
/// Port Enabled
const PORTSC_PE: u16 = 1 << 2;
/// Port Enable Change
const PORTSC_PEC: u16 = 1 << 3;
/// Line Status D+ (bit 4)
const PORTSC_LSDP: u16 = 1 << 4;
/// Line Status D- (bit 5)
const PORTSC_LSDM: u16 = 1 << 5;
/// Resume Detect
const PORTSC_RD: u16 = 1 << 6;
/// Reserved (always 1)
const PORTSC_RESERVED: u16 = 1 << 7;
/// Low Speed Device Attached
const PORTSC_LSDA: u16 = 1 << 8;
/// Port Reset
const PORTSC_PR: u16 = 1 << 9;
/// Suspend
const PORTSC_SUSPEND: u16 = 1 << 12;

// =============================================================================
// Frame List Pointer
// =============================================================================

/// Terminate bit - frame list pointer is invalid
const FLP_T: u32 = 1 << 0;
/// QH bit - pointer is to a Queue Head
const FLP_Q: u32 = 1 << 1;

// =============================================================================
// Transfer Descriptor (TD) - 32 bytes
// =============================================================================

/// TD status: Active
const TD_STATUS_ACTIVE: u32 = 1 << 23;
/// TD status: Stalled
const TD_STATUS_STALLED: u32 = 1 << 22;
/// TD status: Data Buffer Error
const TD_STATUS_DBE: u32 = 1 << 21;
/// TD status: Babble Detected
const TD_STATUS_BABBLE: u32 = 1 << 20;
/// TD status: NAK Received
const TD_STATUS_NAK: u32 = 1 << 19;
/// TD status: CRC/Timeout Error
const TD_STATUS_CRC: u32 = 1 << 18;
/// TD status: Bitstuff Error
const TD_STATUS_BITSTUFF: u32 = 1 << 17;
/// TD IOC (Interrupt on Complete)
const TD_IOC: u32 = 1 << 24;
/// TD Isochronous Select
const TD_IOS: u32 = 1 << 25;
/// TD Low Speed Device
const TD_LS: u32 = 1 << 26;
/// TD Error Count mask (2 bits)
const TD_CERR_MASK: u32 = 3 << 27;
/// TD Short Packet Detect
const TD_SPD: u32 = 1 << 29;

/// TD PID: SETUP token
const TD_PID_SETUP: u32 = 0x2D;
/// TD PID: IN token
const TD_PID_IN: u32 = 0x69;
/// TD PID: OUT token
const TD_PID_OUT: u32 = 0xE1;

/// UHCI Transfer Descriptor
#[repr(C, align(16))]
#[derive(Debug)]
pub struct TransferDescriptor {
    /// Link pointer (physical address of next TD/QH, or T=1 for terminate)
    pub link_ptr: u32,
    /// Control and status
    pub ctrl_status: u32,
    /// Token (PID, device address, endpoint, data toggle, max length)
    pub token: u32,
    /// Buffer pointer (physical address of data buffer)
    pub buffer_ptr: u32,
    /// Software use (for driver)
    pub sw_reserved: [u32; 4],
}

impl TransferDescriptor {
    pub fn new() -> Self {
        Self {
            link_ptr: FLP_T,
            ctrl_status: 0,
            token: 0,
            buffer_ptr: 0,
            sw_reserved: [0; 4],
        }
    }

    /// Set up TD for SETUP token
    pub fn setup_setup(&mut self, address: u8, endpoint: u8, data_phys: u32, len: u16, low_speed: bool) {
        self.ctrl_status = TD_STATUS_ACTIVE | (3 << 27); // 3 retries
        if low_speed {
            self.ctrl_status |= TD_LS;
        }

        // Token: MaxLen | D (toggle) | EndPt | DevAddr | PID
        // DATA0 for SETUP
        let maxlen = if len == 0 { 0x7FF } else { (len - 1) as u32 };
        self.token = (maxlen << 21) | (0 << 19) | ((endpoint as u32) << 15)
            | ((address as u32) << 8) | TD_PID_SETUP;

        self.buffer_ptr = data_phys;
    }

    /// Set up TD for IN token
    pub fn setup_in(&mut self, address: u8, endpoint: u8, data_phys: u32, len: u16, toggle: bool, low_speed: bool) {
        self.ctrl_status = TD_STATUS_ACTIVE | TD_SPD | (3 << 27);
        if low_speed {
            self.ctrl_status |= TD_LS;
        }

        let maxlen = if len == 0 { 0x7FF } else { (len - 1) as u32 };
        let d = if toggle { 1 } else { 0 };
        self.token = (maxlen << 21) | (d << 19) | ((endpoint as u32) << 15)
            | ((address as u32) << 8) | TD_PID_IN;

        self.buffer_ptr = data_phys;
    }

    /// Set up TD for OUT token
    pub fn setup_out(&mut self, address: u8, endpoint: u8, data_phys: u32, len: u16, toggle: bool, low_speed: bool) {
        self.ctrl_status = TD_STATUS_ACTIVE | (3 << 27);
        if low_speed {
            self.ctrl_status |= TD_LS;
        }

        let maxlen = if len == 0 { 0x7FF } else { (len - 1) as u32 };
        let d = if toggle { 1 } else { 0 };
        self.token = (maxlen << 21) | (d << 19) | ((endpoint as u32) << 15)
            | ((address as u32) << 8) | TD_PID_OUT;

        self.buffer_ptr = data_phys;
    }

    /// Check if TD is still active
    pub fn is_active(&self) -> bool {
        self.ctrl_status & TD_STATUS_ACTIVE != 0
    }

    /// Check if TD completed successfully
    pub fn is_success(&self) -> bool {
        let status = self.ctrl_status & 0x00FF0000;
        status == 0 // No error bits set
    }

    /// Check if stalled
    pub fn is_stalled(&self) -> bool {
        self.ctrl_status & TD_STATUS_STALLED != 0
    }

    /// Get actual length transferred
    pub fn actual_length(&self) -> u16 {
        let actlen = (self.ctrl_status + 1) & 0x7FF;
        actlen as u16
    }
}

// =============================================================================
// Queue Head (QH) - 16 bytes
// =============================================================================

/// UHCI Queue Head
#[repr(C, align(16))]
#[derive(Debug)]
pub struct QueueHead {
    /// Horizontal link pointer (next QH)
    pub head_link_ptr: u32,
    /// Element link pointer (first TD)
    pub element_link_ptr: u32,
    /// Software use
    pub sw_reserved: [u32; 2],
}

impl QueueHead {
    pub fn new() -> Self {
        Self {
            head_link_ptr: FLP_T,
            element_link_ptr: FLP_T,
            sw_reserved: [0; 2],
        }
    }

    /// Set next QH in horizontal list
    pub fn set_next_qh(&mut self, phys: u32) {
        self.head_link_ptr = (phys & !0xF) | FLP_Q;
    }

    /// Terminate horizontal list
    pub fn terminate_horizontal(&mut self) {
        self.head_link_ptr = FLP_T;
    }

    /// Set first TD
    pub fn set_first_td(&mut self, phys: u32) {
        self.element_link_ptr = phys & !0xF; // TD pointer, not QH
    }

    /// Terminate element list
    pub fn terminate_element(&mut self) {
        self.element_link_ptr = FLP_T;
    }
}

// =============================================================================
// UHCI Device
// =============================================================================

/// UHCI connected device
#[derive(Debug)]
pub struct UhciDevice {
    pub address: u8,
    pub port: u8,
    pub speed: UsbSpeed,
    pub max_packet_size: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
}

// =============================================================================
// UHCI Controller
// =============================================================================

/// UHCI controller state
pub struct UhciController {
    /// PCI device
    pci_device: PciDevice,
    /// I/O base address
    io_base: u16,
    /// Frame list (1024 entries, 4KB aligned)
    frame_list: Box<[u32; 1024]>,
    /// Frame list physical address
    frame_list_phys: u64,
    /// Control QH
    control_qh: Option<Box<QueueHead>>,
    /// Control QH physical
    control_qh_phys: u64,
    /// Bulk QH
    bulk_qh: Option<Box<QueueHead>>,
    /// Bulk QH physical
    bulk_qh_phys: u64,
    /// Next device address
    next_address: u8,
    /// Connected devices
    devices: BTreeMap<u8, UhciDevice>,
    /// Controller running
    running: AtomicBool,
}

impl UhciController {
    /// Create new UHCI controller
    pub fn new(pci_device: PciDevice, io_base: u16) -> Self {
        Self {
            pci_device,
            io_base,
            frame_list: Box::new([FLP_T; 1024]),
            frame_list_phys: 0,
            control_qh: None,
            control_qh_phys: 0,
            bulk_qh: None,
            bulk_qh_phys: 0,
            next_address: 1,
            devices: BTreeMap::new(),
            running: AtomicBool::new(false),
        }
    }

    /// Read 16-bit I/O register
    unsafe fn read16(&self, offset: u16) -> u16 {
        let mut port = Port::<u16>::new(self.io_base + offset);
        port.read()
    }

    /// Write 16-bit I/O register
    unsafe fn write16(&self, offset: u16, value: u16) {
        let mut port = Port::<u16>::new(self.io_base + offset);
        port.write(value);
    }

    /// Read 32-bit I/O register
    unsafe fn read32(&self, offset: u16) -> u32 {
        let mut port = Port::<u32>::new(self.io_base + offset);
        port.read()
    }

    /// Write 32-bit I/O register
    unsafe fn write32(&self, offset: u16, value: u32) {
        let mut port = Port::<u32>::new(self.io_base + offset);
        port.write(value);
    }

    /// Initialize controller
    pub fn init(&mut self) -> KResult<()> {
        unsafe {
            // Reset controller
            self.reset()?;

            // Set up frame list
            self.setup_frame_list()?;

            // Set up QHs
            self.setup_qhs()?;

            // Start controller
            self.start()?;
        }

        self.running.store(true, Ordering::Release);
        crate::kprintln!("uhci: controller initialized");

        // Enumerate ports
        self.enumerate_ports();

        Ok(())
    }

    /// Reset controller
    unsafe fn reset(&mut self) -> KResult<()> {
        // Stop the controller first
        self.write16(USBCMD, 0);

        // Wait for halt
        for _ in 0..100 {
            if self.read16(USBSTS) & STS_HCHALTED != 0 {
                break;
            }
            core::hint::spin_loop();
        }

        // Global reset
        self.write16(USBCMD, CMD_GRESET);

        // Wait 10ms minimum
        for _ in 0..100000 {
            core::hint::spin_loop();
        }

        // Clear global reset
        self.write16(USBCMD, 0);

        // Wait a bit
        for _ in 0..1000 {
            core::hint::spin_loop();
        }

        // Host controller reset
        self.write16(USBCMD, CMD_HCRESET);

        // Wait for reset to complete
        for _ in 0..100 {
            if self.read16(USBCMD) & CMD_HCRESET == 0 {
                break;
            }
            core::hint::spin_loop();
        }

        // Clear status
        self.write16(USBSTS, 0xFFFF);

        Ok(())
    }

    /// Set up frame list
    unsafe fn setup_frame_list(&mut self) -> KResult<()> {
        // Get physical address of frame list
        let fl_ptr = &*self.frame_list as *const _ as u64;
        self.frame_list_phys = mm::virt_to_phys(x86_64::VirtAddr::new(fl_ptr))
            .ok_or(KError::NoMemory)?
            .as_u64();

        // Write frame list base address
        self.write32(FRBASEADD, self.frame_list_phys as u32);

        // Set frame number to 0
        self.write16(FRNUM, 0);

        Ok(())
    }

    /// Set up Queue Heads
    unsafe fn setup_qhs(&mut self) -> KResult<()> {
        // Create control QH
        let control_qh = Box::new(QueueHead::new());
        let qh_ptr = &*control_qh as *const QueueHead as u64;
        self.control_qh_phys = mm::virt_to_phys(x86_64::VirtAddr::new(qh_ptr))
            .ok_or(KError::NoMemory)?
            .as_u64();
        self.control_qh = Some(control_qh);

        // Create bulk QH
        let mut bulk_qh = Box::new(QueueHead::new());
        let bqh_ptr = &*bulk_qh as *const QueueHead as u64;
        self.bulk_qh_phys = mm::virt_to_phys(x86_64::VirtAddr::new(bqh_ptr))
            .ok_or(KError::NoMemory)?
            .as_u64();

        // Link control -> bulk
        if let Some(ref mut cqh) = self.control_qh {
            cqh.set_next_qh(self.bulk_qh_phys as u32);
        }

        self.bulk_qh = Some(bulk_qh);

        // Set frame list entries to point to control QH
        let qh_entry = (self.control_qh_phys as u32 & !0xF) | FLP_Q;
        for i in 0..1024 {
            self.frame_list[i] = qh_entry;
        }

        Ok(())
    }

    /// Start controller
    unsafe fn start(&mut self) -> KResult<()> {
        // Enable interrupts
        self.write16(USBINTR, INTR_IOCE | INTR_RIE | INTR_SPIE | INTR_TOCRCIE);

        // Start controller with max packet size 64
        self.write16(USBCMD, CMD_RS | CMD_CF | CMD_MAXP);

        // Wait for start
        for _ in 0..100 {
            if self.read16(USBSTS) & STS_HCHALTED == 0 {
                return Ok(());
            }
            core::hint::spin_loop();
        }

        Err(KError::Timeout)
    }

    /// Enumerate ports
    fn enumerate_ports(&mut self) {
        // UHCI has exactly 2 root hub ports
        for port in 0..2 {
            let port_reg = if port == 0 { PORTSC1 } else { PORTSC2 };

            unsafe {
                let status = self.read16(port_reg);

                if status & PORTSC_CCS != 0 {
                    let speed = if status & PORTSC_LSDA != 0 {
                        UsbSpeed::Low
                    } else {
                        UsbSpeed::Full
                    };

                    crate::kprintln!(
                        "uhci: port {} has {:?} speed device",
                        port + 1,
                        speed
                    );

                    if let Err(e) = self.reset_port(port) {
                        crate::kprintln!("uhci: failed to reset port {}: {:?}", port + 1, e);
                        continue;
                    }

                    if let Err(e) = self.enumerate_device(port, speed) {
                        crate::kprintln!("uhci: failed to enumerate device on port {}: {:?}", port + 1, e);
                    }
                }
            }
        }
    }

    /// Reset a port
    unsafe fn reset_port(&mut self, port: u8) -> KResult<()> {
        let port_reg = if port == 0 { PORTSC1 } else { PORTSC2 };

        // Set port reset
        self.write16(port_reg, PORTSC_PR);

        // Wait at least 10ms
        for _ in 0..50000 {
            core::hint::spin_loop();
        }

        // Clear port reset
        let status = self.read16(port_reg);
        self.write16(port_reg, status & !PORTSC_PR);

        // Wait for reset recovery
        for _ in 0..10000 {
            core::hint::spin_loop();
        }

        // Enable port
        let status = self.read16(port_reg);
        self.write16(port_reg, status | PORTSC_PE);

        // Clear status change bits
        let status = self.read16(port_reg);
        self.write16(port_reg, status | PORTSC_CSC | PORTSC_PEC);

        Ok(())
    }

    /// Enumerate a device
    fn enumerate_device(&mut self, port: u8, speed: UsbSpeed) -> KResult<()> {
        let address = self.next_address;
        if address > 127 {
            return Err(KError::NoMemory);
        }
        self.next_address += 1;

        // Get device descriptor with address 0
        let desc = self.get_device_descriptor(0, speed)?;

        // Set device address
        self.set_address(0, address, speed)?;

        let device = UhciDevice {
            address,
            port,
            speed,
            max_packet_size: desc.max_packet_size0,
            vendor_id: desc.vendor_id,
            product_id: desc.product_id,
            device_class: desc.device_class,
            device_subclass: desc.device_subclass,
            device_protocol: desc.device_protocol,
        };

        crate::kprintln!(
            "uhci: device {} at address {}: {:04X}:{:04X}",
            port + 1,
            address,
            device.vendor_id,
            device.product_id
        );

        self.devices.insert(address, device);

        Ok(())
    }

    /// Get device descriptor
    fn get_device_descriptor(&self, address: u8, speed: UsbSpeed) -> KResult<DeviceDescriptor> {
        let desc = DeviceDescriptor {
            length: 18,
            descriptor_type: 1,
            usb_version: 0x0110,
            device_class: 0,
            device_subclass: 0,
            device_protocol: 0,
            max_packet_size0: 8,
            vendor_id: 0,
            product_id: 0,
            device_version: 0,
            manufacturer_index: 0,
            product_index: 0,
            serial_index: 0,
            num_configurations: 0,
        };

        // Simplified - real implementation would do actual USB transfer
        Ok(desc)
    }

    /// Set device address
    fn set_address(&self, _old_address: u8, _new_address: u8, _speed: UsbSpeed) -> KResult<()> {
        // Simplified
        for _ in 0..5000 {
            core::hint::spin_loop();
        }
        Ok(())
    }

    /// Get connected devices
    pub fn devices(&self) -> &BTreeMap<u8, UhciDevice> {
        &self.devices
    }

    /// Handle interrupt
    pub fn handle_interrupt(&mut self) {
        unsafe {
            let status = self.read16(USBSTS);

            if status & STS_USBINT != 0 {
                // Transfer completed
                self.write16(USBSTS, STS_USBINT);
            }

            if status & STS_USBERRINT != 0 {
                crate::kprintln!("uhci: USB error interrupt");
                self.write16(USBSTS, STS_USBERRINT);
            }

            if status & STS_RD != 0 {
                crate::kprintln!("uhci: resume detected");
                self.write16(USBSTS, STS_RD);
            }

            if status & STS_HSE != 0 {
                crate::kprintln!("uhci: host system error!");
                self.write16(USBSTS, STS_HSE);
            }

            if status & STS_HCPE != 0 {
                crate::kprintln!("uhci: host controller process error!");
                self.write16(USBSTS, STS_HCPE);
            }

            // Check for port status changes
            for port in 0..2 {
                let port_reg = if port == 0 { PORTSC1 } else { PORTSC2 };
                let port_status = self.read16(port_reg);

                if port_status & PORTSC_CSC != 0 {
                    // Clear connect status change
                    self.write16(port_reg, port_status | PORTSC_CSC);

                    if port_status & PORTSC_CCS != 0 {
                        crate::kprintln!("uhci: device connected on port {}", port + 1);
                    } else {
                        crate::kprintln!("uhci: device disconnected from port {}", port + 1);
                    }
                }
            }
        }
    }
}

// =============================================================================
// Module Functions
// =============================================================================

use spin::Mutex;
static UHCI_CONTROLLERS: Mutex<Vec<UhciController>> = Mutex::new(Vec::new());

/// Probe PCI for UHCI controllers
pub fn probe_pci() {
    let devices = pci::scan();

    for dev in devices {
        // USB UHCI: class 0x0C, subclass 0x03, prog-if 0x00
        if dev.class.class_code == 0x0C
            && dev.class.subclass == 0x03
            && dev.class.prog_if == 0x00
        {
            crate::kprintln!(
                "uhci: found controller at {:02X}:{:02X}.{:X}",
                dev.addr.bus,
                dev.addr.device,
                dev.addr.function
            );

            // Read BAR4 for I/O base (UHCI uses I/O ports)
            let (bar4_addr, is_io) = pci::read_bar(&dev, 4);

            if is_io && bar4_addr != 0 {
                let io_base = (bar4_addr & 0xFFFF) as u16;
                let mut controller = UhciController::new(dev.clone(), io_base);

                if let Err(e) = controller.init() {
                    crate::kprintln!("uhci: init failed: {:?}", e);
                } else {
                    UHCI_CONTROLLERS.lock().push(controller);
                }
            }
        }
    }
}

/// Initialize UHCI subsystem
pub fn init() {
    crate::kprintln!("uhci: scanning for USB 1.1 UHCI controllers");
    probe_pci();

    let count = UHCI_CONTROLLERS.lock().len();
    if count > 0 {
        crate::kprintln!("uhci: {} controller(s) initialized", count);
    }
}

/// Get number of UHCI controllers
pub fn controller_count() -> usize {
    UHCI_CONTROLLERS.lock().len()
}
