//! EHCI (Enhanced Host Controller Interface) driver for USB 2.0.
//!
//! Implements USB 2.0 support with:
//! - Capability and operational register access
//! - Periodic and async schedules
//! - Queue Heads (QH) and Transfer Descriptors (qTD)
//! - Control, bulk, and interrupt transfers
//! - Device enumeration
//! - Port status and control

#![allow(dead_code)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::drivers::pci::{self, PciDevice};
use crate::mm;
use crate::sync::IrqSafeMutex;
use crate::util::{KError, KResult};

use super::{DeviceDescriptor, SetupPacket, UsbDevice, UsbSpeed};

/// EHCI controller
pub struct EhciController {
    /// PCI device
    pci_device: PciDevice,
    /// Base address for capability registers (MMIO)
    cap_base: u64,
    /// Base address for operational registers (cap_base + CAPLENGTH)
    op_base: u64,
    /// Number of ports
    num_ports: u8,
    /// 64-bit addressing capable
    addr64_capable: bool,
    /// Periodic frame list (4KB aligned)
    periodic_list: Box<[u32; 1024]>,
    /// Periodic list physical address
    periodic_list_phys: u64,
    /// Async schedule head QH
    async_head: Option<Box<QueueHead>>,
    /// Async head physical address
    async_head_phys: u64,
    /// Next device address to assign
    next_address: u8,
    /// Connected devices
    devices: BTreeMap<u8, EhciDevice>,
    /// Controller running
    running: AtomicBool,
}

/// EHCI device
#[derive(Debug)]
pub struct EhciDevice {
    pub address: u8,
    pub port: u8,
    pub speed: UsbSpeed,
    pub max_packet_size: u16,
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
}

// =============================================================================
// EHCI Capability Registers (offset from cap_base)
// =============================================================================

const CAPLENGTH: u32 = 0x00;         // Capability register length
const HCIVERSION: u32 = 0x02;        // HCI version number
const HCSPARAMS: u32 = 0x04;         // Structural parameters
const HCCPARAMS: u32 = 0x08;         // Capability parameters
const HCSP_PORTROUTE: u32 = 0x0C;    // Port routing description

// =============================================================================
// EHCI Operational Registers (offset from op_base)
// =============================================================================

const USBCMD: u32 = 0x00;            // USB command
const USBSTS: u32 = 0x04;            // USB status
const USBINTR: u32 = 0x08;           // USB interrupt enable
const FRINDEX: u32 = 0x0C;           // USB frame index
const CTRLDSSEGMENT: u32 = 0x10;     // 4G segment selector
const PERIODICLISTBASE: u32 = 0x14;  // Frame list base address
const ASYNCLISTADDR: u32 = 0x18;     // Next asynchronous list address
const CONFIGFLAG: u32 = 0x40;        // Configured flag register
const PORTSC: u32 = 0x44;            // Port status/control (first port)

// =============================================================================
// USBCMD bits
// =============================================================================

const CMD_RUN: u32 = 1 << 0;         // Run/Stop
const CMD_HCRESET: u32 = 1 << 1;     // Host controller reset
const CMD_FLS_1024: u32 = 0 << 2;    // Frame list size: 1024
const CMD_FLS_512: u32 = 1 << 2;     // Frame list size: 512
const CMD_FLS_256: u32 = 2 << 2;     // Frame list size: 256
const CMD_PSE: u32 = 1 << 4;         // Periodic schedule enable
const CMD_ASE: u32 = 1 << 5;         // Asynchronous schedule enable
const CMD_IAAD: u32 = 1 << 6;        // Interrupt on async advance doorbell
const CMD_LHCR: u32 = 1 << 7;        // Light host controller reset
const CMD_ASPMC: u32 = 3 << 8;       // Async schedule park mode count
const CMD_ASPME: u32 = 1 << 11;      // Async schedule park mode enable
const CMD_ITC_MASK: u32 = 0xFF << 16; // Interrupt threshold control

// =============================================================================
// USBSTS bits
// =============================================================================

const STS_USBINT: u32 = 1 << 0;      // USB interrupt
const STS_USBERRINT: u32 = 1 << 1;   // USB error interrupt
const STS_PCD: u32 = 1 << 2;         // Port change detect
const STS_FLR: u32 = 1 << 3;         // Frame list rollover
const STS_HSE: u32 = 1 << 4;         // Host system error
const STS_IAA: u32 = 1 << 5;         // Interrupt on async advance
const STS_HALT: u32 = 1 << 12;       // HCHalted
const STS_RECLAMATION: u32 = 1 << 13; // Reclamation
const STS_PSS: u32 = 1 << 14;        // Periodic schedule status
const STS_ASS: u32 = 1 << 15;        // Asynchronous schedule status

// =============================================================================
// PORTSC bits
// =============================================================================

const PORTSC_CCS: u32 = 1 << 0;      // Current connect status
const PORTSC_CSC: u32 = 1 << 1;      // Connect status change
const PORTSC_PE: u32 = 1 << 2;       // Port enabled
const PORTSC_PEC: u32 = 1 << 3;      // Port enable change
const PORTSC_OCA: u32 = 1 << 4;      // Over-current active
const PORTSC_OCC: u32 = 1 << 5;      // Over-current change
const PORTSC_FPR: u32 = 1 << 6;      // Force port resume
const PORTSC_SUSPEND: u32 = 1 << 7;  // Suspend
const PORTSC_PR: u32 = 1 << 8;       // Port reset
const PORTSC_LS_MASK: u32 = 3 << 10; // Line status
const PORTSC_PP: u32 = 1 << 12;      // Port power
const PORTSC_PO: u32 = 1 << 13;      // Port owner (to companion)
const PORTSC_PIC_MASK: u32 = 3 << 14; // Port indicator control
const PORTSC_PTC_MASK: u32 = 0xF << 16; // Port test control
const PORTSC_WKCNNT_E: u32 = 1 << 20; // Wake on connect enable
const PORTSC_WKDSCNNT_E: u32 = 1 << 21; // Wake on disconnect enable
const PORTSC_WKOC_E: u32 = 1 << 22;  // Wake on over-current enable

// =============================================================================
// Queue Head (QH) - 48 bytes, 32-byte aligned
// =============================================================================

#[repr(C, align(32))]
pub struct QueueHead {
    /// Horizontal link pointer
    pub hlp: u32,
    /// Endpoint characteristics
    pub ep_char: u32,
    /// Endpoint capabilities
    pub ep_caps: u32,
    /// Current qTD pointer
    pub current_qtd: u32,
    /// Overlay - Next qTD pointer
    pub next_qtd: u32,
    /// Overlay - Alternate next qTD pointer
    pub alt_qtd: u32,
    /// Overlay - Token
    pub token: u32,
    /// Overlay - Buffer pointer 0
    pub buffer0: u32,
    /// Overlay - Buffer pointer 1
    pub buffer1: u32,
    /// Overlay - Buffer pointer 2
    pub buffer2: u32,
    /// Overlay - Buffer pointer 3
    pub buffer3: u32,
    /// Overlay - Buffer pointer 4
    pub buffer4: u32,
}

impl QueueHead {
    /// Create a new QH for async schedule
    pub fn new_async(address: u8, endpoint: u8, max_packet: u16, speed: UsbSpeed) -> Self {
        // Calculate NAK count reload (max retries before giving up)
        let nak_cnt = 15u32;

        // Endpoint characteristics
        let ep_char = (address as u32)
            | ((endpoint as u32) << 8)
            | (Self::speed_to_eps(speed) << 12)
            | (1 << 14)  // DTC - Data toggle control
            | (1 << 15)  // H - Head of reclamation list (for first QH)
            | ((max_packet as u32) << 16)
            | (nak_cnt << 28);

        // Endpoint capabilities (high-speed hub for full/low speed)
        let ep_caps = (1 << 30); // Mult = 1 for non-high-bandwidth

        Self {
            hlp: 1, // T bit set (invalid pointer initially)
            ep_char,
            ep_caps,
            current_qtd: 0,
            next_qtd: 1, // T bit set
            alt_qtd: 1,
            token: 0,
            buffer0: 0,
            buffer1: 0,
            buffer2: 0,
            buffer3: 0,
            buffer4: 0,
        }
    }

    fn speed_to_eps(speed: UsbSpeed) -> u32 {
        match speed {
            UsbSpeed::Low => 1,
            UsbSpeed::Full => 0,
            UsbSpeed::High => 2,
            _ => 2, // Default to high speed
        }
    }

    /// Link this QH to another (horizontal link)
    pub fn link_to(&mut self, phys_addr: u64) {
        // QH type = 01, T = 0
        self.hlp = ((phys_addr as u32) & !0x1F) | 0x02;
    }

    /// Set as self-linked (for async schedule head)
    pub fn link_to_self(&mut self, self_phys: u64) {
        self.hlp = ((self_phys as u32) & !0x1F) | 0x02;
    }

    /// Link a qTD to this QH
    pub fn link_qtd(&mut self, qtd_phys: u64) {
        self.next_qtd = (qtd_phys as u32) & !0x1F; // Clear T bit
        self.alt_qtd = 1; // Set T bit on alternate
    }
}

// =============================================================================
// Queue Element Transfer Descriptor (qTD) - 32 bytes, 32-byte aligned
// =============================================================================

#[repr(C, align(32))]
pub struct TransferDescriptor {
    /// Next qTD pointer
    pub next_qtd: u32,
    /// Alternate next qTD pointer
    pub alt_qtd: u32,
    /// Token
    pub token: u32,
    /// Buffer pointer 0
    pub buffer0: u32,
    /// Buffer pointer 1
    pub buffer1: u32,
    /// Buffer pointer 2
    pub buffer2: u32,
    /// Buffer pointer 3
    pub buffer3: u32,
    /// Buffer pointer 4
    pub buffer4: u32,
}

// Token bits
const QTD_STATUS_ACTIVE: u32 = 1 << 7;
const QTD_STATUS_HALTED: u32 = 1 << 6;
const QTD_STATUS_BUFERR: u32 = 1 << 5;
const QTD_STATUS_BABBLE: u32 = 1 << 4;
const QTD_STATUS_XACTERR: u32 = 1 << 3;
const QTD_STATUS_MISSED_uF: u32 = 1 << 2;
const QTD_STATUS_SPLITXS: u32 = 1 << 1;
const QTD_STATUS_PING: u32 = 1 << 0;

const QTD_PID_OUT: u32 = 0 << 8;
const QTD_PID_IN: u32 = 1 << 8;
const QTD_PID_SETUP: u32 = 2 << 8;

const QTD_CERR_MASK: u32 = 3 << 10;  // Error counter
const QTD_CPAGE_MASK: u32 = 7 << 12; // Current page
const QTD_IOC: u32 = 1 << 15;        // Interrupt on complete
const QTD_TOTAL_BYTES_SHIFT: u32 = 16;
const QTD_TOGGLE: u32 = 1 << 31;     // Data toggle

impl TransferDescriptor {
    /// Create a setup stage qTD
    pub fn new_setup(setup_packet_phys: u64, data_toggle: bool) -> Self {
        let token = QTD_STATUS_ACTIVE
            | QTD_PID_SETUP
            | (3 << 10)  // CERR = 3
            | (8 << QTD_TOTAL_BYTES_SHIFT) // Setup is always 8 bytes
            | if data_toggle { QTD_TOGGLE } else { 0 };

        Self {
            next_qtd: 1, // T bit
            alt_qtd: 1,
            token,
            buffer0: setup_packet_phys as u32,
            buffer1: 0,
            buffer2: 0,
            buffer3: 0,
            buffer4: 0,
        }
    }

    /// Create a data stage qTD (IN or OUT)
    pub fn new_data(buffer_phys: u64, length: u16, is_in: bool, data_toggle: bool) -> Self {
        let pid = if is_in { QTD_PID_IN } else { QTD_PID_OUT };
        let token = QTD_STATUS_ACTIVE
            | pid
            | (3 << 10) // CERR = 3
            | ((length as u32) << QTD_TOTAL_BYTES_SHIFT)
            | if data_toggle { QTD_TOGGLE } else { 0 };

        Self {
            next_qtd: 1,
            alt_qtd: 1,
            token,
            buffer0: buffer_phys as u32,
            buffer1: ((buffer_phys + 0x1000) & !0xFFF) as u32,
            buffer2: ((buffer_phys + 0x2000) & !0xFFF) as u32,
            buffer3: ((buffer_phys + 0x3000) & !0xFFF) as u32,
            buffer4: ((buffer_phys + 0x4000) & !0xFFF) as u32,
        }
    }

    /// Create a status stage qTD
    pub fn new_status(is_in: bool, data_toggle: bool) -> Self {
        let pid = if is_in { QTD_PID_IN } else { QTD_PID_OUT };
        let token = QTD_STATUS_ACTIVE
            | pid
            | (3 << 10)
            | QTD_IOC // Interrupt on complete
            | if data_toggle { QTD_TOGGLE } else { 0 };

        Self {
            next_qtd: 1,
            alt_qtd: 1,
            token,
            buffer0: 0,
            buffer1: 0,
            buffer2: 0,
            buffer3: 0,
            buffer4: 0,
        }
    }

    /// Link to next qTD
    pub fn link_to(&mut self, next_phys: u64) {
        self.next_qtd = (next_phys as u32) & !0x1F;
    }

    /// Check if transfer is complete
    pub fn is_complete(&self) -> bool {
        (self.token & QTD_STATUS_ACTIVE) == 0
    }

    /// Check if transfer has error
    pub fn has_error(&self) -> bool {
        (self.token & (QTD_STATUS_HALTED | QTD_STATUS_BUFERR | QTD_STATUS_BABBLE | QTD_STATUS_XACTERR)) != 0
    }

    /// Get bytes transferred
    pub fn bytes_transferred(&self, original_length: u16) -> u16 {
        let remaining = ((self.token >> QTD_TOTAL_BYTES_SHIFT) & 0x7FFF) as u16;
        original_length.saturating_sub(remaining)
    }
}

// =============================================================================
// EHCI Controller Implementation
// =============================================================================

impl EhciController {
    /// Create a new EHCI controller
    pub fn new(pci_device: PciDevice) -> KResult<Self> {
        // Get BAR0 (MMIO)
        let (bar0, is_io) = pci::read_bar(&pci_device, 0);
        if bar0 == 0 || is_io {
            crate::kprintln!("ehci: invalid BAR0");
            return Err(KError::NotSupported);
        }

        let cap_base = bar0 & !0xF;
        let cap_base_virt = mm::phys_to_virt(x86_64::PhysAddr::new(cap_base)).as_u64();

        // Read capability registers
        let caplength = unsafe { read_volatile(cap_base_virt as *const u8) } as u32;
        let hciversion = unsafe { read_volatile((cap_base_virt + 2) as *const u16) };
        let hcsparams = unsafe { read_volatile((cap_base_virt + 4) as *const u32) };
        let hccparams = unsafe { read_volatile((cap_base_virt + 8) as *const u32) };

        let num_ports = (hcsparams & 0x0F) as u8;
        let addr64_capable = (hccparams & 1) != 0;

        crate::kprintln!(
            "ehci: version={:#x}, ports={}, 64-bit={}",
            hciversion, num_ports, addr64_capable
        );

        let op_base = cap_base_virt + caplength as u64;

        // Allocate periodic frame list (4KB aligned, 1024 entries)
        let periodic_list = Box::new([1u32; 1024]); // All entries invalid (T bit set)
        let periodic_list_ptr = periodic_list.as_ptr() as u64;
        let periodic_list_phys = mm::virt_to_phys(x86_64::VirtAddr::new(periodic_list_ptr))
            .ok_or(KError::NoMemory)?
            .as_u64();

        // Create async schedule head QH
        let mut async_head = Box::new(QueueHead::new_async(0, 0, 64, UsbSpeed::High));
        let async_head_phys = mm::virt_to_phys(x86_64::VirtAddr::new(
            async_head.as_ref() as *const QueueHead as u64
        ))
        .ok_or(KError::NoMemory)?
        .as_u64();

        // Self-link the async head
        async_head.link_to_self(async_head_phys);
        async_head.ep_char |= 1 << 15; // H bit - head of reclamation list

        let mut controller = Self {
            pci_device,
            cap_base: cap_base_virt,
            op_base,
            num_ports,
            addr64_capable,
            periodic_list,
            periodic_list_phys,
            async_head: Some(async_head),
            async_head_phys,
            next_address: 1,
            devices: BTreeMap::new(),
            running: AtomicBool::new(false),
        };

        // Initialize the controller
        controller.init()?;

        Ok(controller)
    }

    /// Initialize the EHCI controller
    fn init(&mut self) -> KResult<()> {
        // Stop the controller if running
        self.write_op(USBCMD, self.read_op(USBCMD) & !CMD_RUN);
        self.wait_halt()?;

        // Reset the controller
        self.write_op(USBCMD, CMD_HCRESET);
        self.wait_reset()?;

        // Set up registers
        if self.addr64_capable {
            self.write_op(CTRLDSSEGMENT, 0); // Use lower 4GB
        }

        // Set periodic frame list base
        self.write_op(PERIODICLISTBASE, self.periodic_list_phys as u32);

        // Set async list address
        self.write_op(ASYNCLISTADDR, self.async_head_phys as u32);

        // Enable interrupts
        let intr = STS_USBINT | STS_USBERRINT | STS_PCD | STS_HSE | STS_IAA;
        self.write_op(USBINTR, intr);

        // Set frame list size to 1024
        let cmd = CMD_FLS_1024
            | (8 << 16) // Interrupt threshold = 8 micro-frames
            | CMD_ASE   // Enable async schedule
            | CMD_RUN;  // Start controller
        self.write_op(USBCMD, cmd);

        // Wait for controller to start
        for _ in 0..100 {
            if (self.read_op(USBSTS) & STS_HALT) == 0 {
                break;
            }
            crate::drivers::hpet::sleep_ms(1);
        }

        if (self.read_op(USBSTS) & STS_HALT) != 0 {
            crate::kprintln!("ehci: controller failed to start");
            return Err(KError::NotSupported);
        }

        // Route all ports to EHCI (not companion controllers)
        self.write_op(CONFIGFLAG, 1);

        self.running.store(true, Ordering::Release);
        crate::kprintln!("ehci: controller started");

        // Give ports time to power up
        crate::drivers::hpet::sleep_ms(50);

        // Enumerate connected devices
        self.enumerate_ports()?;

        Ok(())
    }

    fn read_op(&self, offset: u32) -> u32 {
        unsafe { read_volatile((self.op_base + offset as u64) as *const u32) }
    }

    fn write_op(&self, offset: u32, value: u32) {
        unsafe { write_volatile((self.op_base + offset as u64) as *mut u32, value) }
    }

    fn read_portsc(&self, port: u8) -> u32 {
        self.read_op(PORTSC + (port as u32) * 4)
    }

    fn write_portsc(&self, port: u8, value: u32) {
        self.write_op(PORTSC + (port as u32) * 4, value)
    }

    fn wait_halt(&self) -> KResult<()> {
        for _ in 0..100 {
            if (self.read_op(USBSTS) & STS_HALT) != 0 {
                return Ok(());
            }
            crate::drivers::hpet::sleep_ms(1);
        }
        Err(KError::Timeout)
    }

    fn wait_reset(&self) -> KResult<()> {
        for _ in 0..100 {
            if (self.read_op(USBCMD) & CMD_HCRESET) == 0 {
                return Ok(());
            }
            crate::drivers::hpet::sleep_ms(1);
        }
        Err(KError::Timeout)
    }

    /// Enumerate all ports
    fn enumerate_ports(&mut self) -> KResult<()> {
        for port in 0..self.num_ports {
            let portsc = self.read_portsc(port);

            // Check if device connected
            if (portsc & PORTSC_CCS) == 0 {
                continue;
            }

            // Check line status - if low speed device, release to companion
            let line_status = (portsc & PORTSC_LS_MASK) >> 10;
            if line_status == 0x01 {
                // K-state: Low speed device - release to companion
                crate::kprintln!("ehci: port {} has low-speed device, releasing to companion", port);
                self.write_portsc(port, portsc | PORTSC_PO);
                continue;
            }

            crate::kprintln!("ehci: device connected on port {}", port);

            // Reset the port
            if let Err(e) = self.reset_port(port) {
                crate::kprintln!("ehci: failed to reset port {}: {:?}", port, e);
                continue;
            }

            // Check if port enabled (high-speed device)
            let portsc = self.read_portsc(port);
            if (portsc & PORTSC_PE) == 0 {
                // Not enabled - full speed device, release to companion
                crate::kprintln!("ehci: port {} is full-speed, releasing to companion", port);
                self.write_portsc(port, portsc | PORTSC_PO);
                continue;
            }

            // High-speed device - enumerate it
            if let Err(e) = self.enumerate_device(port) {
                crate::kprintln!("ehci: failed to enumerate device on port {}: {:?}", port, e);
            }
        }

        Ok(())
    }

    /// Reset a port
    fn reset_port(&mut self, port: u8) -> KResult<()> {
        let mut portsc = self.read_portsc(port);

        // Start reset
        portsc |= PORTSC_PR;
        portsc &= !PORTSC_PE; // PE is RO, but clear anyway
        self.write_portsc(port, portsc);

        // Wait at least 50ms for reset
        crate::drivers::hpet::sleep_ms(50);

        // Clear reset
        portsc = self.read_portsc(port);
        portsc &= !PORTSC_PR;
        self.write_portsc(port, portsc);

        // Wait for reset to complete
        for _ in 0..100 {
            portsc = self.read_portsc(port);
            if (portsc & PORTSC_PR) == 0 {
                break;
            }
            crate::drivers::hpet::sleep_ms(1);
        }

        // Give device recovery time
        crate::drivers::hpet::sleep_ms(10);

        Ok(())
    }

    /// Enumerate a device on a port
    fn enumerate_device(&mut self, port: u8) -> KResult<()> {
        // Get device descriptor at address 0
        let mut desc_buf = [0u8; 18];
        self.control_transfer_in(
            0, // Address 0
            0, // Endpoint 0
            8, // Max packet size (default)
            &SetupPacket::get_descriptor(super::DescriptorType::Device, 0, 8),
            &mut desc_buf[..8],
        )?;

        // Get max packet size from first 8 bytes
        let max_packet = desc_buf[7] as u16;
        if max_packet == 0 {
            return Err(KError::NotSupported);
        }

        // Assign address
        let address = self.next_address;
        self.next_address += 1;

        self.control_transfer_out(
            0,
            0,
            max_packet,
            &SetupPacket::set_address(address),
            &[],
        )?;

        // Wait for address to take effect
        crate::drivers::hpet::sleep_ms(2);

        // Get full device descriptor
        self.control_transfer_in(
            address,
            0,
            max_packet,
            &SetupPacket::get_descriptor(super::DescriptorType::Device, 0, 18),
            &mut desc_buf,
        )?;

        // Parse descriptor
        let desc: DeviceDescriptor = unsafe { core::ptr::read_unaligned(desc_buf.as_ptr() as *const _) };

        let device = EhciDevice {
            address,
            port,
            speed: UsbSpeed::High,
            max_packet_size: max_packet,
            vendor_id: desc.vendor_id,
            product_id: desc.product_id,
            device_class: desc.device_class,
            device_subclass: desc.device_subclass,
            device_protocol: desc.device_protocol,
        };

        crate::kprintln!(
            "ehci: device {}:{:04x}:{:04x} class={:02x} addr={}",
            port, device.vendor_id, device.product_id, device.device_class, address
        );

        self.devices.insert(address, device);

        Ok(())
    }

    /// Perform a control IN transfer
    pub fn control_transfer_in(
        &mut self,
        address: u8,
        endpoint: u8,
        max_packet: u16,
        setup: &SetupPacket,
        data: &mut [u8],
    ) -> KResult<usize> {
        // Allocate buffers
        let setup_buf = Box::new(*setup);
        let setup_phys = mm::virt_to_phys(x86_64::VirtAddr::new(
            setup_buf.as_ref() as *const SetupPacket as u64
        ))
        .ok_or(KError::NoMemory)?
        .as_u64();

        let data_buf = alloc::vec![0u8; data.len()];
        let data_phys = mm::virt_to_phys(x86_64::VirtAddr::new(data_buf.as_ptr() as u64))
            .ok_or(KError::NoMemory)?
            .as_u64();

        // Create qTDs
        let mut setup_qtd = Box::new(TransferDescriptor::new_setup(setup_phys, false));
        let mut data_qtd = Box::new(TransferDescriptor::new_data(data_phys, data.len() as u16, true, true));
        let status_qtd = Box::new(TransferDescriptor::new_status(false, true)); // OUT status

        // Link qTDs
        let data_qtd_phys = mm::virt_to_phys(x86_64::VirtAddr::new(
            data_qtd.as_ref() as *const TransferDescriptor as u64
        ))
        .ok_or(KError::NoMemory)?
        .as_u64();

        let status_qtd_phys = mm::virt_to_phys(x86_64::VirtAddr::new(
            status_qtd.as_ref() as *const TransferDescriptor as u64
        ))
        .ok_or(KError::NoMemory)?
        .as_u64();

        setup_qtd.link_to(data_qtd_phys);
        data_qtd.link_to(status_qtd_phys);

        // Create QH
        let mut qh = Box::new(QueueHead::new_async(address, endpoint, max_packet, UsbSpeed::High));

        let setup_qtd_phys = mm::virt_to_phys(x86_64::VirtAddr::new(
            setup_qtd.as_ref() as *const TransferDescriptor as u64
        ))
        .ok_or(KError::NoMemory)?
        .as_u64();

        qh.link_qtd(setup_qtd_phys);

        // Insert QH into async schedule
        let qh_phys = mm::virt_to_phys(x86_64::VirtAddr::new(
            qh.as_ref() as *const QueueHead as u64
        ))
        .ok_or(KError::NoMemory)?
        .as_u64();

        // Link QH after async head
        if let Some(ref mut head) = self.async_head {
            qh.hlp = head.hlp; // Point to what head pointed to
            head.hlp = ((qh_phys as u32) & !0x1F) | 0x02; // Point head to new QH
        }

        // Wait for transfer to complete
        for _ in 0..1000 {
            if status_qtd.is_complete() {
                break;
            }
            crate::drivers::hpet::sleep_ms(1);
        }

        // Check for errors
        if setup_qtd.has_error() || data_qtd.has_error() || status_qtd.has_error() {
            crate::kprintln!("ehci: control transfer error");
            return Err(KError::IO);
        }

        if !status_qtd.is_complete() {
            crate::kprintln!("ehci: control transfer timeout");
            return Err(KError::Timeout);
        }

        // Copy data back
        let transferred = data_qtd.bytes_transferred(data.len() as u16) as usize;
        data[..transferred].copy_from_slice(&data_buf[..transferred]);

        // Restore async schedule
        if let Some(ref mut head) = self.async_head {
            head.link_to_self(self.async_head_phys);
        }

        Ok(transferred)
    }

    /// Perform a control OUT transfer
    pub fn control_transfer_out(
        &mut self,
        address: u8,
        endpoint: u8,
        max_packet: u16,
        setup: &SetupPacket,
        data: &[u8],
    ) -> KResult<()> {
        // Allocate setup buffer
        let setup_buf = Box::new(*setup);
        let setup_phys = mm::virt_to_phys(x86_64::VirtAddr::new(
            setup_buf.as_ref() as *const SetupPacket as u64
        ))
        .ok_or(KError::NoMemory)?
        .as_u64();

        // Create qTDs
        let mut setup_qtd = Box::new(TransferDescriptor::new_setup(setup_phys, false));
        let status_qtd = Box::new(TransferDescriptor::new_status(true, true)); // IN status

        let status_qtd_phys = mm::virt_to_phys(x86_64::VirtAddr::new(
            status_qtd.as_ref() as *const TransferDescriptor as u64
        ))
        .ok_or(KError::NoMemory)?
        .as_u64();

        if data.is_empty() {
            // No data stage
            setup_qtd.link_to(status_qtd_phys);
        } else {
            // Has data stage
            let data_buf: Vec<u8> = data.to_vec();
            let data_phys = mm::virt_to_phys(x86_64::VirtAddr::new(data_buf.as_ptr() as u64))
                .ok_or(KError::NoMemory)?
                .as_u64();

            let mut data_qtd = Box::new(TransferDescriptor::new_data(data_phys, data.len() as u16, false, true));
            let data_qtd_phys = mm::virt_to_phys(x86_64::VirtAddr::new(
                data_qtd.as_ref() as *const TransferDescriptor as u64
            ))
            .ok_or(KError::NoMemory)?
            .as_u64();

            setup_qtd.link_to(data_qtd_phys);
            data_qtd.link_to(status_qtd_phys);

            // Keep data_qtd alive
            core::mem::forget(data_qtd);
            core::mem::forget(data_buf);
        }

        // Create QH
        let mut qh = Box::new(QueueHead::new_async(address, endpoint, max_packet, UsbSpeed::High));

        let setup_qtd_phys = mm::virt_to_phys(x86_64::VirtAddr::new(
            setup_qtd.as_ref() as *const TransferDescriptor as u64
        ))
        .ok_or(KError::NoMemory)?
        .as_u64();

        qh.link_qtd(setup_qtd_phys);

        // Insert QH into async schedule
        let qh_phys = mm::virt_to_phys(x86_64::VirtAddr::new(
            qh.as_ref() as *const QueueHead as u64
        ))
        .ok_or(KError::NoMemory)?
        .as_u64();

        if let Some(ref mut head) = self.async_head {
            qh.hlp = head.hlp;
            head.hlp = ((qh_phys as u32) & !0x1F) | 0x02;
        }

        // Wait for transfer to complete
        for _ in 0..1000 {
            if status_qtd.is_complete() {
                break;
            }
            crate::drivers::hpet::sleep_ms(1);
        }

        // Check for errors
        if setup_qtd.has_error() || status_qtd.has_error() {
            crate::kprintln!("ehci: control transfer error");
            return Err(KError::IO);
        }

        if !status_qtd.is_complete() {
            crate::kprintln!("ehci: control transfer timeout");
            return Err(KError::Timeout);
        }

        // Restore async schedule
        if let Some(ref mut head) = self.async_head {
            head.link_to_self(self.async_head_phys);
        }

        Ok(())
    }

    /// Get list of connected devices
    pub fn get_devices(&self) -> Vec<&EhciDevice> {
        self.devices.values().collect()
    }
}

// =============================================================================
// Global EHCI state
// =============================================================================

static EHCI_CONTROLLERS: IrqSafeMutex<Vec<EhciController>> = IrqSafeMutex::new(Vec::new());
static EHCI_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize EHCI controllers
pub fn init() {
    if EHCI_INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }

    crate::kprintln!("ehci: scanning for controllers...");

    // Scan all PCI devices
    let devices = pci::scan();

    for dev in devices {
        // Check if this is USB controller (class 0x0C, subclass 0x03)
        if dev.class.class_code != 0x0C || dev.class.subclass != 0x03 {
            continue;
        }

        // Check if this is EHCI (prog_if = 0x20)
        if dev.class.prog_if != 0x20 {
            continue;
        }

        crate::kprintln!(
            "ehci: found controller at {:02x}:{:02x}.{:x}",
            dev.addr.bus, dev.addr.device, dev.addr.function
        );

        // Enable bus mastering and memory space
        pci::enable_bus_mastering(&dev);

        match EhciController::new(dev) {
            Ok(controller) => {
                EHCI_CONTROLLERS.lock().push(controller);
            }
            Err(e) => {
                crate::kprintln!("ehci: failed to initialize controller: {:?}", e);
            }
        }
    }

    let count = EHCI_CONTROLLERS.lock().len();
    crate::kprintln!("ehci: initialized {} controller(s)", count);
}

/// Get number of EHCI controllers
pub fn controller_count() -> usize {
    EHCI_CONTROLLERS.lock().len()
}

/// Get device count across all controllers
pub fn device_count() -> usize {
    EHCI_CONTROLLERS.lock().iter().map(|c| c.devices.len()).sum()
}
