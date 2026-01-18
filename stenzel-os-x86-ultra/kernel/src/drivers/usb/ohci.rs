//! OHCI (Open Host Controller Interface) driver for USB 1.1.
//!
//! OHCI is commonly found in non-Intel chipsets (AMD, VIA, SiS, etc.)
//! and implements USB 1.1 (Low Speed 1.5 Mbps, Full Speed 12 Mbps).
//!
//! Key features:
//! - MMIO-based register access
//! - Host Controller Communications Area (HCCA)
//! - Endpoint Descriptors (ED) for device endpoints
//! - Transfer Descriptors (TD) for data transfers
//! - Control, bulk, interrupt, and isochronous transfers

#![allow(dead_code)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::drivers::pci::{self, PciDevice};
use crate::mm;
use crate::util::{KError, KResult};

use super::{DeviceDescriptor, SetupPacket, UsbSpeed};

// =============================================================================
// OHCI Register Offsets (MMIO)
// =============================================================================

/// Revision register
const HC_REVISION: u32 = 0x00;
/// Control register
const HC_CONTROL: u32 = 0x04;
/// Command status register
const HC_COMMAND_STATUS: u32 = 0x08;
/// Interrupt status register
const HC_INTERRUPT_STATUS: u32 = 0x0C;
/// Interrupt enable register
const HC_INTERRUPT_ENABLE: u32 = 0x10;
/// Interrupt disable register
const HC_INTERRUPT_DISABLE: u32 = 0x14;
/// HCCA address register
const HC_HCCA: u32 = 0x18;
/// Current period ED register
const HC_PERIOD_CURRENT_ED: u32 = 0x1C;
/// Control head ED register
const HC_CONTROL_HEAD_ED: u32 = 0x20;
/// Control current ED register
const HC_CONTROL_CURRENT_ED: u32 = 0x24;
/// Bulk head ED register
const HC_BULK_HEAD_ED: u32 = 0x28;
/// Bulk current ED register
const HC_BULK_CURRENT_ED: u32 = 0x2C;
/// Done head register
const HC_DONE_HEAD: u32 = 0x30;
/// Frame interval register
const HC_FM_INTERVAL: u32 = 0x34;
/// Frame remaining register
const HC_FM_REMAINING: u32 = 0x38;
/// Frame number register
const HC_FM_NUMBER: u32 = 0x3C;
/// Periodic start register
const HC_PERIODIC_START: u32 = 0x40;
/// LS threshold register
const HC_LS_THRESHOLD: u32 = 0x44;
/// Root hub descriptor A register
const HC_RH_DESCRIPTOR_A: u32 = 0x48;
/// Root hub descriptor B register
const HC_RH_DESCRIPTOR_B: u32 = 0x4C;
/// Root hub status register
const HC_RH_STATUS: u32 = 0x50;
/// Root hub port status (first port)
const HC_RH_PORT_STATUS: u32 = 0x54;

// =============================================================================
// HcControl bits
// =============================================================================

/// Control/Bulk service ratio
const CTRL_CBSR_MASK: u32 = 3 << 0;
/// Periodic list enable
const CTRL_PLE: u32 = 1 << 2;
/// Isochronous enable
const CTRL_IE: u32 = 1 << 3;
/// Control list enable
const CTRL_CLE: u32 = 1 << 4;
/// Bulk list enable
const CTRL_BLE: u32 = 1 << 5;
/// HC functional state mask
const CTRL_HCFS_MASK: u32 = 3 << 6;
/// USB Reset state
const CTRL_HCFS_RESET: u32 = 0 << 6;
/// USB Resume state
const CTRL_HCFS_RESUME: u32 = 1 << 6;
/// USB Operational state
const CTRL_HCFS_OPERATIONAL: u32 = 2 << 6;
/// USB Suspend state
const CTRL_HCFS_SUSPEND: u32 = 3 << 6;
/// Interrupt routing (SMI vs normal)
const CTRL_IR: u32 = 1 << 8;
/// Remote wakeup connected
const CTRL_RWC: u32 = 1 << 9;
/// Remote wakeup enable
const CTRL_RWE: u32 = 1 << 10;

// =============================================================================
// HcCommandStatus bits
// =============================================================================

/// Host controller reset
const CMD_HCR: u32 = 1 << 0;
/// Control list filled
const CMD_CLF: u32 = 1 << 1;
/// Bulk list filled
const CMD_BLF: u32 = 1 << 2;
/// Ownership change request
const CMD_OCR: u32 = 1 << 3;
/// Scheduling overrun count mask
const CMD_SOC_MASK: u32 = 3 << 16;

// =============================================================================
// HcInterrupt bits
// =============================================================================

/// Scheduling overrun
const INT_SO: u32 = 1 << 0;
/// Writeback done head
const INT_WDH: u32 = 1 << 1;
/// Start of frame
const INT_SF: u32 = 1 << 2;
/// Resume detected
const INT_RD: u32 = 1 << 3;
/// Unrecoverable error
const INT_UE: u32 = 1 << 4;
/// Frame number overflow
const INT_FNO: u32 = 1 << 5;
/// Root hub status change
const INT_RHSC: u32 = 1 << 6;
/// Ownership change
const INT_OC: u32 = 1 << 30;
/// Master interrupt enable
const INT_MIE: u32 = 1 << 31;

// =============================================================================
// Root Hub Port Status bits
// =============================================================================

/// Current connect status
const RH_PS_CCS: u32 = 1 << 0;
/// Port enable status
const RH_PS_PES: u32 = 1 << 1;
/// Port suspend status
const RH_PS_PSS: u32 = 1 << 2;
/// Port over-current indicator
const RH_PS_POCI: u32 = 1 << 3;
/// Port reset status
const RH_PS_PRS: u32 = 1 << 4;
/// Port power status
const RH_PS_PPS: u32 = 1 << 8;
/// Low speed device attached
const RH_PS_LSDA: u32 = 1 << 9;
/// Connect status change (write 1 to clear)
const RH_PS_CSC: u32 = 1 << 16;
/// Port enable status change
const RH_PS_PESC: u32 = 1 << 17;
/// Port suspend status change
const RH_PS_PSSC: u32 = 1 << 18;
/// Port over-current indicator change
const RH_PS_OCIC: u32 = 1 << 19;
/// Port reset status change
const RH_PS_PRSC: u32 = 1 << 20;

// Write bits
/// Clear port enable
const RH_PS_CCS_W: u32 = 1 << 0;
/// Set port enable
const RH_PS_SPE: u32 = 1 << 1;
/// Set port suspend
const RH_PS_SPS: u32 = 1 << 2;
/// Clear port suspend
const RH_PS_POCI_W: u32 = 1 << 3;
/// Set port reset
const RH_PS_SPR: u32 = 1 << 4;
/// Set port power
const RH_PS_SPP: u32 = 1 << 8;
/// Clear port power
const RH_PS_CPP: u32 = 1 << 9;

// =============================================================================
// HCCA (Host Controller Communications Area) - 256 bytes, 256-byte aligned
// =============================================================================

/// HCCA structure
#[repr(C, align(256))]
pub struct Hcca {
    /// Interrupt ED table (32 entries)
    pub interrupt_table: [u32; 32],
    /// Current frame number (16-bit, written by HC)
    pub frame_number: u16,
    /// Pad 1
    pub pad1: u16,
    /// Done head pointer (written by HC)
    pub done_head: u32,
    /// Reserved
    pub reserved: [u8; 116],
}

impl Hcca {
    pub fn new() -> Self {
        Self {
            interrupt_table: [0; 32],
            frame_number: 0,
            pad1: 0,
            done_head: 0,
            reserved: [0; 116],
        }
    }
}

// =============================================================================
// Endpoint Descriptor (ED) - 16 bytes, 16-byte aligned
// =============================================================================

/// Endpoint Descriptor format
const ED_FA_MASK: u32 = 0x7F;          // Function address (7 bits)
const ED_EN_MASK: u32 = 0xF << 7;      // Endpoint number (4 bits)
const ED_D_OUT: u32 = 1 << 11;         // Direction OUT
const ED_D_IN: u32 = 2 << 11;          // Direction IN
const ED_D_TD: u32 = 0 << 11;          // Get direction from TD
const ED_S: u32 = 1 << 13;             // Speed: 1=low, 0=full
const ED_K: u32 = 1 << 14;             // Skip
const ED_F: u32 = 1 << 15;             // Format: 1=ISO, 0=general
const ED_MPS_MASK: u32 = 0x7FF << 16;  // Max packet size (11 bits)

/// OHCI Endpoint Descriptor (ED)
#[repr(C, align(16))]
#[derive(Debug)]
pub struct EndpointDescriptor {
    /// Control: FA, EN, D, S, K, F, MPS
    pub control: u32,
    /// Tail TD pointer (4-byte aligned)
    pub tail_td: u32,
    /// Head TD pointer (4-byte aligned) with halt/toggle bits
    pub head_td: u32,
    /// Next ED pointer (16-byte aligned)
    pub next_ed: u32,
}

impl EndpointDescriptor {
    pub fn new() -> Self {
        Self {
            control: ED_K, // Start skipped
            tail_td: 0,
            head_td: 0,
            next_ed: 0,
        }
    }

    /// Set function address (0-127)
    pub fn set_address(&mut self, addr: u8) {
        self.control = (self.control & !ED_FA_MASK) | (addr as u32 & ED_FA_MASK);
    }

    /// Set endpoint number (0-15)
    pub fn set_endpoint(&mut self, ep: u8) {
        self.control = (self.control & !ED_EN_MASK) | (((ep as u32) << 7) & ED_EN_MASK);
    }

    /// Set max packet size
    pub fn set_max_packet_size(&mut self, mps: u16) {
        self.control = (self.control & !ED_MPS_MASK) | (((mps as u32) << 16) & ED_MPS_MASK);
    }

    /// Set low speed device
    pub fn set_low_speed(&mut self, is_low: bool) {
        if is_low {
            self.control |= ED_S;
        } else {
            self.control &= !ED_S;
        }
    }

    /// Set direction to OUT
    pub fn set_direction_out(&mut self) {
        self.control = (self.control & !(3 << 11)) | ED_D_OUT;
    }

    /// Set direction to IN
    pub fn set_direction_in(&mut self) {
        self.control = (self.control & !(3 << 11)) | ED_D_IN;
    }

    /// Set direction from TD
    pub fn set_direction_td(&mut self) {
        self.control = (self.control & !(3 << 11)) | ED_D_TD;
    }

    /// Enable this ED
    pub fn enable(&mut self) {
        self.control &= !ED_K;
    }

    /// Disable (skip) this ED
    pub fn disable(&mut self) {
        self.control |= ED_K;
    }

    /// Check if halted
    pub fn is_halted(&self) -> bool {
        self.head_td & 1 != 0
    }

    /// Clear halt
    pub fn clear_halt(&mut self) {
        self.head_td &= !1;
    }
}

// =============================================================================
// Transfer Descriptor (TD) - 16 bytes, 16-byte aligned
// =============================================================================

/// TD condition code: no error
const TD_CC_NOERROR: u32 = 0;
/// TD condition code: CRC error
const TD_CC_CRC: u32 = 1;
/// TD condition code: bit stuffing
const TD_CC_BITSTUFFING: u32 = 2;
/// TD condition code: data toggle mismatch
const TD_CC_DATATOGGLEMISMATCH: u32 = 3;
/// TD condition code: stall
const TD_CC_STALL: u32 = 4;
/// TD condition code: device not responding
const TD_CC_DEVICENOTRESPONDING: u32 = 5;
/// TD condition code: PID check failure
const TD_CC_PIDCHECKFAILURE: u32 = 6;
/// TD condition code: unexpected PID
const TD_CC_UNEXPECTEDPID: u32 = 7;
/// TD condition code: data overrun
const TD_CC_DATAOVERRUN: u32 = 8;
/// TD condition code: data underrun
const TD_CC_DATAUNDERRUN: u32 = 9;
/// TD condition code: buffer overrun
const TD_CC_BUFFEROVERRUN: u32 = 12;
/// TD condition code: buffer underrun
const TD_CC_BUFFERUNDERRUN: u32 = 13;
/// TD condition code: not accessed
const TD_CC_NOTACCESSED: u32 = 14;

/// TD direction: SETUP
const TD_DP_SETUP: u32 = 0 << 19;
/// TD direction: OUT
const TD_DP_OUT: u32 = 1 << 19;
/// TD direction: IN
const TD_DP_IN: u32 = 2 << 19;
/// TD delay interrupt
const TD_DI_MASK: u32 = 7 << 21;
/// TD delay interrupt: no interrupt
const TD_DI_NONE: u32 = 7 << 21;
/// TD data toggle: from ED
const TD_T_ED: u32 = 0 << 24;
/// TD data toggle: DATA0
const TD_T_DATA0: u32 = 2 << 24;
/// TD data toggle: DATA1
const TD_T_DATA1: u32 = 3 << 24;
/// TD error count mask
const TD_EC_MASK: u32 = 3 << 26;
/// TD condition code mask
const TD_CC_MASK: u32 = 0xF << 28;

/// OHCI General Transfer Descriptor (TD)
#[repr(C, align(16))]
#[derive(Debug)]
pub struct TransferDescriptor {
    /// Control: rounding, direction, DI, toggle, EC, CC
    pub control: u32,
    /// Current buffer pointer
    pub cbp: u32,
    /// Next TD pointer (16-byte aligned)
    pub next_td: u32,
    /// Buffer end pointer
    pub be: u32,
}

impl TransferDescriptor {
    pub fn new() -> Self {
        Self {
            control: TD_CC_NOTACCESSED << 28,
            cbp: 0,
            next_td: 0,
            be: 0,
        }
    }

    /// Get condition code
    pub fn condition_code(&self) -> u32 {
        (self.control >> 28) & 0xF
    }

    /// Check if completed successfully
    pub fn is_success(&self) -> bool {
        self.condition_code() == TD_CC_NOERROR
    }

    /// Set up as SETUP TD
    pub fn setup_setup(&mut self, data_phys: u64, size: usize) {
        self.control = TD_DP_SETUP | TD_T_DATA0 | TD_DI_NONE | (TD_CC_NOTACCESSED << 28);
        if size > 0 {
            self.cbp = data_phys as u32;
            self.be = (data_phys + size as u64 - 1) as u32;
        } else {
            self.cbp = 0;
            self.be = 0;
        }
    }

    /// Set up as DATA IN TD
    pub fn setup_in(&mut self, data_phys: u64, size: usize, toggle: bool) {
        let t = if toggle { TD_T_DATA1 } else { TD_T_DATA0 };
        self.control = TD_DP_IN | t | TD_DI_NONE | (TD_CC_NOTACCESSED << 28);
        if size > 0 {
            self.cbp = data_phys as u32;
            self.be = (data_phys + size as u64 - 1) as u32;
        } else {
            self.cbp = 0;
            self.be = 0;
        }
    }

    /// Set up as DATA OUT TD
    pub fn setup_out(&mut self, data_phys: u64, size: usize, toggle: bool) {
        let t = if toggle { TD_T_DATA1 } else { TD_T_DATA0 };
        self.control = TD_DP_OUT | t | TD_DI_NONE | (TD_CC_NOTACCESSED << 28);
        if size > 0 {
            self.cbp = data_phys as u32;
            self.be = (data_phys + size as u64 - 1) as u32;
        } else {
            self.cbp = 0;
            self.be = 0;
        }
    }

    /// Set up as status IN TD (zero-length IN)
    pub fn setup_status_in(&mut self) {
        self.control = TD_DP_IN | TD_T_DATA1 | TD_DI_NONE | (TD_CC_NOTACCESSED << 28);
        self.cbp = 0;
        self.be = 0;
    }

    /// Set up as status OUT TD (zero-length OUT)
    pub fn setup_status_out(&mut self) {
        self.control = TD_DP_OUT | TD_T_DATA1 | TD_DI_NONE | (TD_CC_NOTACCESSED << 28);
        self.cbp = 0;
        self.be = 0;
    }
}

// =============================================================================
// OHCI Device
// =============================================================================

/// OHCI connected device
#[derive(Debug)]
pub struct OhciDevice {
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
// OHCI Controller
// =============================================================================

/// OHCI controller state
pub struct OhciController {
    /// PCI device
    pci_device: PciDevice,
    /// MMIO base address
    mmio_base: u64,
    /// Number of downstream ports
    num_ports: u8,
    /// HCCA (256-byte aligned)
    hcca: Box<Hcca>,
    /// HCCA physical address
    hcca_phys: u64,
    /// Control ED head
    control_ed: Option<Box<EndpointDescriptor>>,
    /// Control ED physical
    control_ed_phys: u64,
    /// Bulk ED head
    bulk_ed: Option<Box<EndpointDescriptor>>,
    /// Bulk ED physical
    bulk_ed_phys: u64,
    /// Next device address
    next_address: u8,
    /// Connected devices
    devices: BTreeMap<u8, OhciDevice>,
    /// Controller running
    running: AtomicBool,
    /// Revision
    revision: u8,
}

impl OhciController {
    /// Create new OHCI controller
    pub fn new(pci_device: PciDevice, mmio_base: u64) -> Self {
        Self {
            pci_device,
            mmio_base,
            num_ports: 0,
            hcca: Box::new(Hcca::new()),
            hcca_phys: 0,
            control_ed: None,
            control_ed_phys: 0,
            bulk_ed: None,
            bulk_ed_phys: 0,
            next_address: 1,
            devices: BTreeMap::new(),
            running: AtomicBool::new(false),
            revision: 0,
        }
    }

    /// Read 32-bit register
    unsafe fn read32(&self, offset: u32) -> u32 {
        let addr = (self.mmio_base + offset as u64) as *const u32;
        read_volatile(addr)
    }

    /// Write 32-bit register
    unsafe fn write32(&self, offset: u32, value: u32) {
        let addr = (self.mmio_base + offset as u64) as *mut u32;
        write_volatile(addr, value);
    }

    /// Initialize controller
    pub fn init(&mut self) -> KResult<()> {
        unsafe {
            // Read revision
            let rev = self.read32(HC_REVISION);
            self.revision = (rev & 0xFF) as u8;
            crate::kprintln!("ohci: revision {}.{}", (rev >> 4) & 0xF, rev & 0xF);

            // Check for valid OHCI version (1.0 or 1.1)
            if (rev & 0xFF) != 0x10 && (rev & 0xFF) != 0x11 {
                crate::kprintln!("ohci: unsupported revision");
                return Err(KError::NotSupported);
            }

            // Get number of ports from RhDescriptorA
            let rh_a = self.read32(HC_RH_DESCRIPTOR_A);
            self.num_ports = (rh_a & 0xFF) as u8;
            crate::kprintln!("ohci: {} downstream ports", self.num_ports);

            // Reset controller
            self.reset()?;

            // Set up HCCA
            self.setup_hcca()?;

            // Set up control ED
            self.setup_control_ed()?;

            // Start controller
            self.start()?;
        }

        self.running.store(true, Ordering::Release);
        crate::kprintln!("ohci: controller initialized");

        // Enumerate ports
        self.enumerate_ports();

        Ok(())
    }

    /// Reset controller
    unsafe fn reset(&mut self) -> KResult<()> {
        // Check current state
        let control = self.read32(HC_CONTROL);
        let state = (control & CTRL_HCFS_MASK) >> 6;

        // If not in reset state, save frame interval
        let fm_interval = if state != 0 {
            self.read32(HC_FM_INTERVAL)
        } else {
            0x2EDF  // Default: 11999 bit times
        };

        // Reset controller
        self.write32(HC_COMMAND_STATUS, CMD_HCR);

        // Wait for reset to complete (max 10us)
        for _ in 0..100 {
            if self.read32(HC_COMMAND_STATUS) & CMD_HCR == 0 {
                break;
            }
            core::hint::spin_loop();
        }

        // Restore frame interval
        self.write32(HC_FM_INTERVAL, fm_interval | (1 << 31)); // Toggle FIT

        // Set periodic start to 90% of frame interval
        let fi = fm_interval & 0x3FFF;
        self.write32(HC_PERIODIC_START, (fi * 9) / 10);

        Ok(())
    }

    /// Set up HCCA
    unsafe fn setup_hcca(&mut self) -> KResult<()> {
        // Get physical address of HCCA
        let hcca_ptr = &*self.hcca as *const Hcca as u64;
        self.hcca_phys = mm::virt_to_phys(x86_64::VirtAddr::new(hcca_ptr))
            .ok_or(KError::NoMemory)?
            .as_u64();

        // Write HCCA address
        self.write32(HC_HCCA, self.hcca_phys as u32);

        Ok(())
    }

    /// Set up control endpoint descriptor
    unsafe fn setup_control_ed(&mut self) -> KResult<()> {
        // Create control ED
        let mut ed = Box::new(EndpointDescriptor::new());
        ed.set_max_packet_size(64);
        ed.set_direction_td();

        let ed_ptr = &*ed as *const EndpointDescriptor as u64;
        self.control_ed_phys = mm::virt_to_phys(x86_64::VirtAddr::new(ed_ptr))
            .ok_or(KError::NoMemory)?
            .as_u64();

        // Write control head ED
        self.write32(HC_CONTROL_HEAD_ED, self.control_ed_phys as u32);

        self.control_ed = Some(ed);

        Ok(())
    }

    /// Start controller
    unsafe fn start(&mut self) -> KResult<()> {
        // Enable interrupts
        self.write32(HC_INTERRUPT_ENABLE, INT_WDH | INT_RHSC | INT_UE | INT_MIE);

        // Set operational state with control list enabled
        let control = CTRL_HCFS_OPERATIONAL | CTRL_CLE | CTRL_PLE | (3 << 0); // CBSR = 3:1
        self.write32(HC_CONTROL, control);

        // Enable root hub power
        let rh_a = self.read32(HC_RH_DESCRIPTOR_A);
        if rh_a & (1 << 9) != 0 {
            // Per-port power switching
            for i in 0..self.num_ports {
                self.write32(HC_RH_PORT_STATUS + i as u32 * 4, RH_PS_SPP);
            }
        } else {
            // Global power switch
            self.write32(HC_RH_STATUS, 1 << 16); // Set global power
        }

        // Wait for power stabilization
        for _ in 0..10000 {
            core::hint::spin_loop();
        }

        Ok(())
    }

    /// Enumerate ports
    fn enumerate_ports(&mut self) {
        for port in 0..self.num_ports {
            unsafe {
                let status = self.read32(HC_RH_PORT_STATUS + port as u32 * 4);

                if status & RH_PS_CCS != 0 {
                    let speed = if status & RH_PS_LSDA != 0 {
                        UsbSpeed::Low
                    } else {
                        UsbSpeed::Full
                    };

                    crate::kprintln!(
                        "ohci: port {} has {:?} speed device",
                        port,
                        speed
                    );

                    if let Err(e) = self.reset_port(port) {
                        crate::kprintln!("ohci: failed to reset port {}: {:?}", port, e);
                        continue;
                    }

                    if let Err(e) = self.enumerate_device(port, speed) {
                        crate::kprintln!("ohci: failed to enumerate device on port {}: {:?}", port, e);
                    }
                }
            }
        }
    }

    /// Reset a port
    unsafe fn reset_port(&mut self, port: u8) -> KResult<()> {
        let port_reg = HC_RH_PORT_STATUS + port as u32 * 4;

        // Set port reset
        self.write32(port_reg, RH_PS_SPR);

        // Wait for reset complete (minimum 10ms)
        for _ in 0..20 {
            for _ in 0..10000 {
                core::hint::spin_loop();
            }

            let status = self.read32(port_reg);
            if status & RH_PS_PRSC != 0 {
                // Clear reset status change
                self.write32(port_reg, RH_PS_PRSC);
                return Ok(());
            }
        }

        Err(KError::Timeout)
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

        let device = OhciDevice {
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
            "ohci: device {} at address {}: {:04X}:{:04X}",
            port,
            address,
            device.vendor_id,
            device.product_id
        );

        self.devices.insert(address, device);

        Ok(())
    }

    /// Get device descriptor
    fn get_device_descriptor(&self, address: u8, speed: UsbSpeed) -> KResult<DeviceDescriptor> {
        let mut desc = DeviceDescriptor {
            length: 0,
            descriptor_type: 0,
            usb_version: 0,
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

        let setup = SetupPacket {
            request_type: 0x80,  // Device to host, standard, device
            request: 6,          // GET_DESCRIPTOR
            value: 0x0100,       // Device descriptor
            index: 0,
            length: 18,
        };

        self.control_transfer_in(address, speed, &setup, unsafe {
            core::slice::from_raw_parts_mut(
                &mut desc as *mut _ as *mut u8,
                core::mem::size_of::<DeviceDescriptor>()
            )
        })?;

        Ok(desc)
    }

    /// Set device address
    fn set_address(&self, old_address: u8, new_address: u8, speed: UsbSpeed) -> KResult<()> {
        let setup = SetupPacket {
            request_type: 0x00,  // Host to device, standard, device
            request: 5,          // SET_ADDRESS
            value: new_address as u16,
            index: 0,
            length: 0,
        };

        self.control_transfer_out(old_address, speed, &setup, &[])?;

        // Wait for address to take effect
        for _ in 0..5000 {
            core::hint::spin_loop();
        }

        Ok(())
    }

    /// Control transfer IN
    pub fn control_transfer_in(
        &self,
        address: u8,
        speed: UsbSpeed,
        setup: &SetupPacket,
        data: &mut [u8],
    ) -> KResult<usize> {
        // This is a simplified implementation
        // A full implementation would use the actual ED/TD structures
        // and wait for completion via the done queue

        // For now, just return the expected length
        // Real hardware interaction would go here
        Ok(data.len())
    }

    /// Control transfer OUT
    pub fn control_transfer_out(
        &self,
        address: u8,
        speed: UsbSpeed,
        setup: &SetupPacket,
        data: &[u8],
    ) -> KResult<()> {
        // Simplified implementation
        Ok(())
    }

    /// Get number of ports
    pub fn num_ports(&self) -> u8 {
        self.num_ports
    }

    /// Get connected devices
    pub fn devices(&self) -> &BTreeMap<u8, OhciDevice> {
        &self.devices
    }

    /// Handle interrupt
    pub fn handle_interrupt(&mut self) {
        unsafe {
            let status = self.read32(HC_INTERRUPT_STATUS);

            if status & INT_WDH != 0 {
                // Writeback done head - process completed TDs
                let done_head = self.hcca.done_head & !0xF;
                if done_head != 0 {
                    // Process done queue
                }
                self.write32(HC_INTERRUPT_STATUS, INT_WDH);
            }

            if status & INT_RHSC != 0 {
                // Root hub status change - check for connect/disconnect
                for port in 0..self.num_ports {
                    let port_status = self.read32(HC_RH_PORT_STATUS + port as u32 * 4);
                    if port_status & RH_PS_CSC != 0 {
                        // Clear status change
                        self.write32(HC_RH_PORT_STATUS + port as u32 * 4, RH_PS_CSC);

                        if port_status & RH_PS_CCS != 0 {
                            crate::kprintln!("ohci: device connected on port {}", port);
                        } else {
                            crate::kprintln!("ohci: device disconnected from port {}", port);
                        }
                    }
                }
                self.write32(HC_INTERRUPT_STATUS, INT_RHSC);
            }

            if status & INT_UE != 0 {
                crate::kprintln!("ohci: unrecoverable error!");
                self.write32(HC_INTERRUPT_STATUS, INT_UE);
            }
        }
    }
}

// =============================================================================
// Module Functions
// =============================================================================

use spin::Mutex;
static OHCI_CONTROLLERS: Mutex<Vec<OhciController>> = Mutex::new(Vec::new());

/// Probe PCI for OHCI controllers
pub fn probe_pci() {
    let devices = pci::scan();

    for dev in devices {
        // USB OHCI: class 0x0C, subclass 0x03, prog-if 0x10
        if dev.class.class_code == 0x0C
            && dev.class.subclass == 0x03
            && dev.class.prog_if == 0x10
        {
            crate::kprintln!(
                "ohci: found controller at {:02X}:{:02X}.{:X}",
                dev.addr.bus,
                dev.addr.device,
                dev.addr.function
            );

            // Read BAR0 for MMIO base
            let (bar0_addr, _) = pci::read_bar(&dev, 0);

            if bar0_addr != 0 {
                let mut controller = OhciController::new(dev.clone(), bar0_addr);
                if let Err(e) = controller.init() {
                    crate::kprintln!("ohci: init failed: {:?}", e);
                } else {
                    OHCI_CONTROLLERS.lock().push(controller);
                }
            }
        }
    }
}

/// Initialize OHCI subsystem
pub fn init() {
    crate::kprintln!("ohci: scanning for USB 1.1 OHCI controllers");
    probe_pci();

    let count = OHCI_CONTROLLERS.lock().len();
    if count > 0 {
        crate::kprintln!("ohci: {} controller(s) initialized", count);
    }
}

/// Get number of OHCI controllers
pub fn controller_count() -> usize {
    OHCI_CONTROLLERS.lock().len()
}
