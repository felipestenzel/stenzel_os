//! LPC (Low Pin Count) and eSPI (Enhanced SPI) Driver
//!
//! Implements support for:
//! - LPC bus interface (legacy)
//! - eSPI (Enhanced SPI) interface (modern)
//! - Super I/O controller access
//! - TPM over LPC/eSPI
//! - EC (Embedded Controller) communication
//! - Legacy device decode

#![allow(dead_code)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use x86_64::instructions::port::Port;

use crate::drivers::pci;
use crate::mm;
use crate::sync::IrqSafeMutex;

/// LPC/eSPI interface type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceType {
    Lpc,
    Espi,
    Unknown,
}

impl InterfaceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            InterfaceType::Lpc => "LPC",
            InterfaceType::Espi => "eSPI",
            InterfaceType::Unknown => "Unknown",
        }
    }
}

/// LPC decode range
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LpcDecodeRange {
    /// Super I/O at 2E/2F or 4E/4F
    SuperIo,
    /// FDD (Floppy Disk) at 3F0-3F7
    Fdd,
    /// LPT1 at 378-37F
    Lpt1,
    /// LPT2 at 278-27F
    Lpt2,
    /// LPT3 at 3BC-3BF
    Lpt3,
    /// COM1 at 3F8-3FF
    Com1,
    /// COM2 at 2F8-2FF
    Com2,
    /// COM3 at 3E8-3EF
    Com3,
    /// COM4 at 2E8-2EF
    Com4,
    /// Keyboard controller at 60/64
    Keyboard,
    /// Game port at 200-207
    GamePort,
    /// Custom range
    Custom(u16, u16),
}

impl LpcDecodeRange {
    pub fn port_range(&self) -> (u16, u16) {
        match self {
            LpcDecodeRange::SuperIo => (0x2E, 0x2F),
            LpcDecodeRange::Fdd => (0x3F0, 0x3F7),
            LpcDecodeRange::Lpt1 => (0x378, 0x37F),
            LpcDecodeRange::Lpt2 => (0x278, 0x27F),
            LpcDecodeRange::Lpt3 => (0x3BC, 0x3BF),
            LpcDecodeRange::Com1 => (0x3F8, 0x3FF),
            LpcDecodeRange::Com2 => (0x2F8, 0x2FF),
            LpcDecodeRange::Com3 => (0x3E8, 0x3EF),
            LpcDecodeRange::Com4 => (0x2E8, 0x2EF),
            LpcDecodeRange::Keyboard => (0x60, 0x64),
            LpcDecodeRange::GamePort => (0x200, 0x207),
            LpcDecodeRange::Custom(start, end) => (*start, *end),
        }
    }
}

/// eSPI channel type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EspiChannel {
    Peripheral,
    VirtualWire,
    Oob,      // Out-of-band
    FlashAccess,
}

impl EspiChannel {
    pub fn as_str(&self) -> &'static str {
        match self {
            EspiChannel::Peripheral => "Peripheral",
            EspiChannel::VirtualWire => "Virtual Wire",
            EspiChannel::Oob => "OOB",
            EspiChannel::FlashAccess => "Flash Access",
        }
    }
}

/// Super I/O chip type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperIoChip {
    Ite8728,
    Ite8783,
    Nct6775,
    Nct6776,
    Nct6779,
    Nct6791,
    Nct6795,
    Fintek8728,
    Nuvoton6106,
    Unknown(u16),
}

impl SuperIoChip {
    pub fn from_id(id: u16) -> Self {
        match id {
            0x8728 => SuperIoChip::Ite8728,
            0x8783 => SuperIoChip::Ite8783,
            0xC330 | 0xC333 => SuperIoChip::Nct6775,
            0xC331 | 0xC334 => SuperIoChip::Nct6776,
            0xC560 | 0xC562 => SuperIoChip::Nct6779,
            0xC800 | 0xC801 => SuperIoChip::Nct6791,
            0xD121 | 0xD122 => SuperIoChip::Nct6795,
            0x0728 => SuperIoChip::Fintek8728,
            _ => SuperIoChip::Unknown(id),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SuperIoChip::Ite8728 => "ITE IT8728F",
            SuperIoChip::Ite8783 => "ITE IT8783E/F",
            SuperIoChip::Nct6775 => "Nuvoton NCT6775",
            SuperIoChip::Nct6776 => "Nuvoton NCT6776",
            SuperIoChip::Nct6779 => "Nuvoton NCT6779",
            SuperIoChip::Nct6791 => "Nuvoton NCT6791",
            SuperIoChip::Nct6795 => "Nuvoton NCT6795",
            SuperIoChip::Fintek8728 => "Fintek F8728",
            SuperIoChip::Nuvoton6106 => "Nuvoton 6106",
            SuperIoChip::Unknown(_) => "Unknown",
        }
    }
}

/// Super I/O Logical Device Number
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperIoLdn {
    Fdc = 0,       // Floppy Disk Controller
    Pp = 1,        // Parallel Port
    Sp1 = 2,       // Serial Port 1
    Sp2 = 3,       // Serial Port 2
    Ec = 4,        // Environment Controller / HW Monitor
    Kbc = 5,       // Keyboard Controller
    Gpio = 7,      // GPIO
    Acpi = 10,     // ACPI
    HwMon = 11,    // Hardware Monitor (some chips)
    Wdt = 8,       // Watchdog Timer
}

/// TPM interface type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpmInterfaceType {
    Tis12,      // TPM Interface Specification 1.2
    Tis20,      // TPM Interface Specification 2.0
    Fifo,       // FIFO interface
    Crb,        // Command Response Buffer
}

/// EC (Embedded Controller) interface type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcInterface {
    Acpi,       // ACPI EC interface (ports 62/66)
    LpcMailbox, // Custom LPC mailbox
    Espi,       // eSPI OOB channel
}

/// LPC/eSPI Controller configuration
#[derive(Debug, Clone)]
pub struct LpcEspiConfig {
    /// Interface type
    pub interface_type: InterfaceType,
    /// Controller MMIO base
    pub mmio_base: u64,
    /// Enabled decode ranges
    pub decode_ranges: Vec<LpcDecodeRange>,
    /// eSPI channels enabled (if eSPI)
    pub espi_channels: Vec<EspiChannel>,
    /// Detected Super I/O chip
    pub superio_chip: Option<SuperIoChip>,
    /// Super I/O base port
    pub superio_port: u16,
    /// TPM present
    pub tpm_present: bool,
    /// TPM interface type
    pub tpm_interface: Option<TpmInterfaceType>,
    /// EC present
    pub ec_present: bool,
    /// EC interface
    pub ec_interface: Option<EcInterface>,
}

impl Default for LpcEspiConfig {
    fn default() -> Self {
        Self {
            interface_type: InterfaceType::Unknown,
            mmio_base: 0,
            decode_ranges: Vec::new(),
            espi_channels: Vec::new(),
            superio_chip: None,
            superio_port: 0x2E,
            tpm_present: false,
            tpm_interface: None,
            ec_present: false,
            ec_interface: None,
        }
    }
}

/// LPC error type
#[derive(Debug, Clone)]
pub enum LpcError {
    NotInitialized,
    InvalidPort,
    DeviceNotFound,
    Timeout,
    InvalidOperation,
    NotSupported,
}

pub type LpcResult<T> = Result<T, LpcError>;

/// Super I/O register access
pub struct SuperIo {
    /// Index port (0x2E or 0x4E)
    index_port: u16,
    /// Data port (0x2F or 0x4F)
    data_port: u16,
    /// Chip type
    chip: SuperIoChip,
    /// Currently selected LDN
    current_ldn: u8,
}

impl SuperIo {
    /// Create a new Super I/O accessor
    pub fn new(base_port: u16, chip: SuperIoChip) -> Self {
        Self {
            index_port: base_port,
            data_port: base_port + 1,
            chip,
            current_ldn: 0xFF,
        }
    }

    /// Enter configuration mode (ITE chips use 87 87)
    pub fn enter_config(&self) {
        unsafe {
            let mut index = Port::<u8>::new(self.index_port);
            // ITE/Nuvoton enter sequence
            index.write(0x87);
            index.write(0x87);
        }
    }

    /// Exit configuration mode
    pub fn exit_config(&self) {
        unsafe {
            let mut index = Port::<u8>::new(self.index_port);
            // Write 0xAA to exit
            index.write(0xAA);
        }
    }

    /// Read a register
    pub fn read(&self, reg: u8) -> u8 {
        unsafe {
            let mut index = Port::<u8>::new(self.index_port);
            let mut data = Port::<u8>::new(self.data_port);
            index.write(reg);
            data.read()
        }
    }

    /// Write a register
    pub fn write(&self, reg: u8, value: u8) {
        unsafe {
            let mut index = Port::<u8>::new(self.index_port);
            let mut data = Port::<u8>::new(self.data_port);
            index.write(reg);
            data.write(value);
        }
    }

    /// Select a Logical Device Number
    pub fn select_ldn(&mut self, ldn: u8) {
        self.write(0x07, ldn);
        self.current_ldn = ldn;
    }

    /// Get chip ID
    pub fn get_chip_id(&self) -> u16 {
        let high = self.read(0x20) as u16;
        let low = self.read(0x21) as u16;
        (high << 8) | low
    }

    /// Enable a logical device
    pub fn enable_device(&mut self, ldn: u8) {
        self.select_ldn(ldn);
        let activate = self.read(0x30);
        self.write(0x30, activate | 0x01);
    }

    /// Disable a logical device
    pub fn disable_device(&mut self, ldn: u8) {
        self.select_ldn(ldn);
        let activate = self.read(0x30);
        self.write(0x30, activate & !0x01);
    }

    /// Get device I/O base address
    pub fn get_io_base(&mut self, ldn: u8) -> u16 {
        self.select_ldn(ldn);
        let high = self.read(0x60) as u16;
        let low = self.read(0x61) as u16;
        (high << 8) | low
    }

    /// Set device I/O base address
    pub fn set_io_base(&mut self, ldn: u8, base: u16) {
        self.select_ldn(ldn);
        self.write(0x60, (base >> 8) as u8);
        self.write(0x61, base as u8);
    }

    /// Get device IRQ
    pub fn get_irq(&mut self, ldn: u8) -> u8 {
        self.select_ldn(ldn);
        self.read(0x70)
    }

    /// Set device IRQ
    pub fn set_irq(&mut self, ldn: u8, irq: u8) {
        self.select_ldn(ldn);
        self.write(0x70, irq);
    }
}

/// EC (Embedded Controller) access
pub struct EmbeddedController {
    /// Command port (0x66)
    cmd_port: u16,
    /// Data port (0x62)
    data_port: u16,
}

impl EmbeddedController {
    /// Create a new EC accessor with default ACPI ports
    pub fn new() -> Self {
        Self {
            cmd_port: 0x66,
            data_port: 0x62,
        }
    }

    /// Wait for EC input buffer to be empty
    fn wait_ibf_empty(&self) -> LpcResult<()> {
        for _ in 0..10000 {
            let status = unsafe {
                let mut port = Port::<u8>::new(self.cmd_port);
                port.read()
            };
            if (status & 0x02) == 0 {
                return Ok(());
            }
            core::hint::spin_loop();
        }
        Err(LpcError::Timeout)
    }

    /// Wait for EC output buffer to be full
    fn wait_obf_full(&self) -> LpcResult<()> {
        for _ in 0..10000 {
            let status = unsafe {
                let mut port = Port::<u8>::new(self.cmd_port);
                port.read()
            };
            if (status & 0x01) != 0 {
                return Ok(());
            }
            core::hint::spin_loop();
        }
        Err(LpcError::Timeout)
    }

    /// Read an EC register
    pub fn read(&self, addr: u8) -> LpcResult<u8> {
        // Send read command
        self.wait_ibf_empty()?;
        unsafe {
            let mut port = Port::<u8>::new(self.cmd_port);
            port.write(0x80); // Read command
        }

        // Send address
        self.wait_ibf_empty()?;
        unsafe {
            let mut port = Port::<u8>::new(self.data_port);
            port.write(addr);
        }

        // Read data
        self.wait_obf_full()?;
        let value = unsafe {
            let mut port = Port::<u8>::new(self.data_port);
            port.read()
        };

        Ok(value)
    }

    /// Write an EC register
    pub fn write(&self, addr: u8, value: u8) -> LpcResult<()> {
        // Send write command
        self.wait_ibf_empty()?;
        unsafe {
            let mut port = Port::<u8>::new(self.cmd_port);
            port.write(0x81); // Write command
        }

        // Send address
        self.wait_ibf_empty()?;
        unsafe {
            let mut port = Port::<u8>::new(self.data_port);
            port.write(addr);
        }

        // Send data
        self.wait_ibf_empty()?;
        unsafe {
            let mut port = Port::<u8>::new(self.data_port);
            port.write(value);
        }

        Ok(())
    }

    /// Check if EC is present
    pub fn is_present(&self) -> bool {
        let status = unsafe {
            let mut port = Port::<u8>::new(self.cmd_port);
            port.read()
        };
        // Check for reasonable status value
        status != 0xFF && status != 0x00
    }

    /// Query EC for pending events
    pub fn query_event(&self) -> LpcResult<u8> {
        self.wait_ibf_empty()?;
        unsafe {
            let mut port = Port::<u8>::new(self.cmd_port);
            port.write(0x84); // Query command
        }

        self.wait_obf_full()?;
        let event = unsafe {
            let mut port = Port::<u8>::new(self.data_port);
            port.read()
        };

        Ok(event)
    }
}

/// Intel PCH LPC/eSPI register offsets
mod intel_regs {
    // LPC registers
    pub const LPC_GEN1_DEC: u32 = 0x84;   // Generic decode range 1
    pub const LPC_GEN2_DEC: u32 = 0x88;   // Generic decode range 2
    pub const LPC_GEN3_DEC: u32 = 0x8C;   // Generic decode range 3
    pub const LPC_GEN4_DEC: u32 = 0x90;   // Generic decode range 4
    pub const LPC_IOD: u32 = 0x80;        // I/O Decode ranges
    pub const LPC_IOE: u32 = 0x82;        // I/O Enable

    // eSPI registers
    pub const ESPI_CFG: u32 = 0x00;       // eSPI configuration
    pub const ESPI_CH0_CFG: u32 = 0x04;   // Channel 0 configuration
    pub const ESPI_CH1_CFG: u32 = 0x08;   // Channel 1 configuration
    pub const ESPI_VW_CFG: u32 = 0x10;    // Virtual Wire configuration
    pub const ESPI_OOB_CFG: u32 = 0x14;   // OOB configuration
    pub const ESPI_FLASH_CFG: u32 = 0x18; // Flash Access configuration
}

/// LPC/eSPI Controller
pub struct LpcEspiController {
    /// Configuration
    config: LpcEspiConfig,
    /// Super I/O accessor
    superio: Option<SuperIo>,
    /// EC accessor
    ec: Option<EmbeddedController>,
    /// Initialized flag
    initialized: AtomicBool,
}

impl LpcEspiController {
    pub const fn new() -> Self {
        Self {
            config: LpcEspiConfig {
                interface_type: InterfaceType::Unknown,
                mmio_base: 0,
                decode_ranges: Vec::new(),
                espi_channels: Vec::new(),
                superio_chip: None,
                superio_port: 0x2E,
                tpm_present: false,
                tpm_interface: None,
                ec_present: false,
                ec_interface: None,
            },
            superio: None,
            ec: None,
            initialized: AtomicBool::new(false),
        }
    }

    /// Initialize the controller
    pub fn init(&mut self) -> LpcResult<()> {
        // Detect LPC/eSPI controller
        self.detect_controller()?;

        // Detect Super I/O chip
        self.detect_superio()?;

        // Detect EC
        self.detect_ec()?;

        // Detect TPM
        self.detect_tpm()?;

        self.initialized.store(true, Ordering::Release);

        crate::kprintln!(
            "lpc_espi: {} interface initialized",
            self.config.interface_type.as_str()
        );

        if let Some(chip) = &self.config.superio_chip {
            crate::kprintln!("lpc_espi: Super I/O: {} at {:#x}", chip.as_str(), self.config.superio_port);
        }

        if self.config.ec_present {
            crate::kprintln!("lpc_espi: EC present via ACPI");
        }

        if self.config.tpm_present {
            crate::kprintln!("lpc_espi: TPM present");
        }

        Ok(())
    }

    /// Detect LPC/eSPI controller
    fn detect_controller(&mut self) -> LpcResult<()> {
        let devices = pci::scan();

        for dev in devices {
            // LPC/eSPI controller is typically ISA bridge (class 06, subclass 01)
            if dev.class.class_code == 0x06 && dev.class.subclass == 0x01 {
                // Intel LPC/eSPI
                if dev.id.vendor_id == 0x8086 {
                    // Check if it's eSPI or LPC based on device ID range
                    let is_espi = dev.id.device_id >= 0xA300; // Cannon Lake and newer

                    self.config.interface_type = if is_espi {
                        InterfaceType::Espi
                    } else {
                        InterfaceType::Lpc
                    };

                    // Get MMIO base (typically from RCBA or P2SB)
                    self.config.mmio_base = 0xFED0_0000; // Default Intel LPC base

                    // Enable standard decode ranges
                    self.config.decode_ranges = alloc::vec![
                        LpcDecodeRange::SuperIo,
                        LpcDecodeRange::Keyboard,
                        LpcDecodeRange::Com1,
                        LpcDecodeRange::Com2,
                    ];

                    if is_espi {
                        self.config.espi_channels = alloc::vec![
                            EspiChannel::Peripheral,
                            EspiChannel::VirtualWire,
                        ];
                    }

                    return Ok(());
                }

                // AMD FCH LPC
                if dev.id.vendor_id == 0x1022 || dev.id.vendor_id == 0x1002 {
                    self.config.interface_type = InterfaceType::Lpc;
                    self.config.mmio_base = 0xFEC0_0000; // AMD LPC base

                    self.config.decode_ranges = alloc::vec![
                        LpcDecodeRange::SuperIo,
                        LpcDecodeRange::Keyboard,
                        LpcDecodeRange::Com1,
                    ];

                    return Ok(());
                }
            }
        }

        // Default to LPC if we can't detect
        self.config.interface_type = InterfaceType::Lpc;
        Ok(())
    }

    /// Detect Super I/O chip
    fn detect_superio(&mut self) -> LpcResult<()> {
        // Try standard ports 0x2E and 0x4E
        for port in [0x2E, 0x4E] {
            let mut sio = SuperIo::new(port, SuperIoChip::Unknown(0));

            // Enter config mode
            sio.enter_config();

            // Read chip ID
            let chip_id = sio.get_chip_id();

            // Exit config mode
            sio.exit_config();

            // Check if valid ID
            if chip_id != 0xFFFF && chip_id != 0x0000 {
                let chip = SuperIoChip::from_id(chip_id);
                crate::kprintln!("lpc_espi: detected Super I/O chip ID {:#06x} at {:#x}", chip_id, port);

                self.config.superio_chip = Some(chip);
                self.config.superio_port = port;
                self.superio = Some(SuperIo::new(port, chip));
                return Ok(());
            }
        }

        Ok(()) // No Super I/O is not an error
    }

    /// Detect Embedded Controller
    fn detect_ec(&mut self) -> LpcResult<()> {
        let ec = EmbeddedController::new();

        if ec.is_present() {
            self.config.ec_present = true;
            self.config.ec_interface = Some(EcInterface::Acpi);
            self.ec = Some(ec);
        }

        Ok(())
    }

    /// Detect TPM
    fn detect_tpm(&mut self) -> LpcResult<()> {
        // TPM TIS is at 0xFED4_0000
        let tpm_base = 0xFED4_0000u64;
        let virt = mm::phys_to_virt(x86_64::PhysAddr::new(tpm_base));

        // Read TPM_DID_VID register
        let did_vid = unsafe {
            core::ptr::read_volatile(virt.as_ptr::<u32>().add(0xF00 / 4))
        };

        // Check if valid TPM vendor ID
        if did_vid != 0xFFFF_FFFF && did_vid != 0 {
            self.config.tpm_present = true;

            // Detect TPM interface type
            let interface_id = unsafe {
                core::ptr::read_volatile(virt.as_ptr::<u32>().add(0x30 / 4))
            };

            self.config.tpm_interface = Some(if (interface_id & 0x0F) == 0 {
                TpmInterfaceType::Fifo
            } else {
                TpmInterfaceType::Crb
            });
        }

        Ok(())
    }

    /// Get Super I/O accessor
    pub fn superio(&mut self) -> Option<&mut SuperIo> {
        self.superio.as_mut()
    }

    /// Get EC accessor
    pub fn ec(&self) -> Option<&EmbeddedController> {
        self.ec.as_ref()
    }

    /// Check if TPM is present
    pub fn has_tpm(&self) -> bool {
        self.config.tpm_present
    }

    /// Check if EC is present
    pub fn has_ec(&self) -> bool {
        self.config.ec_present
    }

    /// Get configuration
    pub fn config(&self) -> &LpcEspiConfig {
        &self.config
    }

    /// Add a custom decode range
    pub fn add_decode_range(&mut self, range: LpcDecodeRange) {
        if !self.config.decode_ranges.contains(&range) {
            self.config.decode_ranges.push(range);
        }
    }

    /// Format status as string
    pub fn format_status(&self) -> String {
        let mut output = String::new();

        output.push_str(&alloc::format!(
            "LPC/eSPI Controller: {}\n",
            self.config.interface_type.as_str()
        ));

        output.push_str(&alloc::format!(
            "  MMIO Base: {:#x}\n",
            self.config.mmio_base
        ));

        if let Some(chip) = &self.config.superio_chip {
            output.push_str(&alloc::format!(
                "  Super I/O: {} at {:#x}\n",
                chip.as_str(),
                self.config.superio_port
            ));
        }

        output.push_str(&alloc::format!(
            "  EC: {}\n",
            if self.config.ec_present { "Present" } else { "Not detected" }
        ));

        output.push_str(&alloc::format!(
            "  TPM: {}\n",
            if self.config.tpm_present {
                match &self.config.tpm_interface {
                    Some(TpmInterfaceType::Fifo) => "Present (FIFO)",
                    Some(TpmInterfaceType::Crb) => "Present (CRB)",
                    Some(TpmInterfaceType::Tis12) => "Present (TIS 1.2)",
                    Some(TpmInterfaceType::Tis20) => "Present (TIS 2.0)",
                    None => "Present",
                }
            } else {
                "Not detected"
            }
        ));

        if !self.config.decode_ranges.is_empty() {
            output.push_str("  Decode Ranges:\n");
            for range in &self.config.decode_ranges {
                let (start, end) = range.port_range();
                output.push_str(&alloc::format!("    {:#x}-{:#x}\n", start, end));
            }
        }

        output
    }
}

// =============================================================================
// Global State
// =============================================================================

static LPC_ESPI_CONTROLLER: IrqSafeMutex<LpcEspiController> = IrqSafeMutex::new(LpcEspiController::new());

/// Initialize LPC/eSPI subsystem
pub fn init() {
    let mut ctrl = LPC_ESPI_CONTROLLER.lock();
    match ctrl.init() {
        Ok(()) => {}
        Err(e) => {
            crate::kprintln!("lpc_espi: initialization failed: {:?}", e);
        }
    }
}

/// Get controller reference
pub fn controller() -> impl core::ops::DerefMut<Target = LpcEspiController> {
    LPC_ESPI_CONTROLLER.lock()
}

/// Check if TPM is present
pub fn has_tpm() -> bool {
    LPC_ESPI_CONTROLLER.lock().has_tpm()
}

/// Check if EC is present
pub fn has_ec() -> bool {
    LPC_ESPI_CONTROLLER.lock().has_ec()
}

/// Read EC register
pub fn ec_read(addr: u8) -> LpcResult<u8> {
    let ctrl = LPC_ESPI_CONTROLLER.lock();
    if let Some(ec) = ctrl.ec() {
        ec.read(addr)
    } else {
        Err(LpcError::DeviceNotFound)
    }
}

/// Write EC register
pub fn ec_write(addr: u8, value: u8) -> LpcResult<()> {
    let ctrl = LPC_ESPI_CONTROLLER.lock();
    if let Some(ec) = ctrl.ec() {
        ec.write(addr, value)
    } else {
        Err(LpcError::DeviceNotFound)
    }
}

/// Format status
pub fn format_status() -> String {
    LPC_ESPI_CONTROLLER.lock().format_status()
}
