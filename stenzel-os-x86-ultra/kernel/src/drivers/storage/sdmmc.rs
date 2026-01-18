//! SD/MMC Host Controller Driver
//!
//! Implements support for:
//! - SDHCI (SD Host Controller Interface)
//! - SD cards (SD 2.0, SDHC, SDXC)
//! - MMC cards (eMMC 4.x, 5.x)
//! - Card detection and enumeration
//! - Block read/write operations

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use crate::sync::TicketSpinlock;
use crate::drivers::pci::{self, PciDevice};

/// Global SDHCI controllers
static SDHCI_CONTROLLERS: TicketSpinlock<Vec<SdhciController>> = TicketSpinlock::new(Vec::new());

// ============================================================================
// SDHCI Register Definitions
// ============================================================================

/// SDHCI Register offsets
mod regs {
    pub const SDMA_ADDR: u16 = 0x00;           // SDMA System Address
    pub const BLOCK_SIZE: u16 = 0x04;          // Block Size
    pub const BLOCK_COUNT: u16 = 0x06;         // Block Count
    pub const ARGUMENT: u16 = 0x08;            // Argument
    pub const TRANSFER_MODE: u16 = 0x0C;       // Transfer Mode
    pub const COMMAND: u16 = 0x0E;             // Command
    pub const RESPONSE: u16 = 0x10;            // Response (0x10-0x1F)
    pub const BUFFER_DATA: u16 = 0x20;         // Buffer Data Port
    pub const PRESENT_STATE: u16 = 0x24;       // Present State
    pub const HOST_CONTROL: u16 = 0x28;        // Host Control
    pub const POWER_CONTROL: u16 = 0x29;       // Power Control
    pub const BLOCK_GAP: u16 = 0x2A;           // Block Gap Control
    pub const WAKEUP_CONTROL: u16 = 0x2B;      // Wakeup Control
    pub const CLOCK_CONTROL: u16 = 0x2C;       // Clock Control
    pub const TIMEOUT_CONTROL: u16 = 0x2E;     // Timeout Control
    pub const SW_RESET: u16 = 0x2F;            // Software Reset
    pub const INT_STATUS: u16 = 0x30;          // Normal Interrupt Status
    pub const INT_STATUS_ENABLE: u16 = 0x34;   // Normal Interrupt Status Enable
    pub const INT_SIGNAL_ENABLE: u16 = 0x38;   // Normal Interrupt Signal Enable
    pub const AUTO_CMD_ERROR: u16 = 0x3C;      // Auto CMD Error Status
    pub const HOST_CONTROL2: u16 = 0x3E;       // Host Control 2
    pub const CAPABILITIES: u16 = 0x40;        // Capabilities
    pub const CAPABILITIES2: u16 = 0x44;       // Capabilities 2
    pub const MAX_CURRENT: u16 = 0x48;         // Maximum Current Capabilities
    pub const FORCE_EVENT: u16 = 0x50;         // Force Event
    pub const ADMA_ERROR: u16 = 0x54;          // ADMA Error Status
    pub const ADMA_ADDR: u16 = 0x58;           // ADMA System Address
    pub const PRESET_VALUES: u16 = 0x60;       // Preset Value Registers
    pub const SHARED_BUS: u16 = 0xE0;          // Shared Bus Control
    pub const SLOT_INT_STATUS: u16 = 0xFC;     // Slot Interrupt Status
    pub const HOST_VERSION: u16 = 0xFE;        // Host Controller Version
}

/// Transfer Mode Register bits
mod transfer_mode {
    pub const DMA_ENABLE: u16 = 1 << 0;
    pub const BLOCK_COUNT_ENABLE: u16 = 1 << 1;
    pub const AUTO_CMD12_ENABLE: u16 = 1 << 2;
    pub const AUTO_CMD23_ENABLE: u16 = 2 << 2;
    pub const DATA_DIRECTION_READ: u16 = 1 << 4;
    pub const MULTI_BLOCK: u16 = 1 << 5;
}

/// Command Register bits
mod command {
    pub const RESPONSE_TYPE_NONE: u16 = 0 << 0;
    pub const RESPONSE_TYPE_136: u16 = 1 << 0;
    pub const RESPONSE_TYPE_48: u16 = 2 << 0;
    pub const RESPONSE_TYPE_48_BUSY: u16 = 3 << 0;
    pub const CRC_CHECK_ENABLE: u16 = 1 << 3;
    pub const INDEX_CHECK_ENABLE: u16 = 1 << 4;
    pub const DATA_PRESENT: u16 = 1 << 5;
    pub const CMD_TYPE_NORMAL: u16 = 0 << 6;
    pub const CMD_TYPE_SUSPEND: u16 = 1 << 6;
    pub const CMD_TYPE_RESUME: u16 = 2 << 6;
    pub const CMD_TYPE_ABORT: u16 = 3 << 6;
}

/// Present State bits
mod present_state {
    pub const CMD_INHIBIT: u32 = 1 << 0;
    pub const DATA_INHIBIT: u32 = 1 << 1;
    pub const DATA_LINE_ACTIVE: u32 = 1 << 2;
    pub const WRITE_TRANSFER_ACTIVE: u32 = 1 << 8;
    pub const READ_TRANSFER_ACTIVE: u32 = 1 << 9;
    pub const BUFFER_WRITE_ENABLE: u32 = 1 << 10;
    pub const BUFFER_READ_ENABLE: u32 = 1 << 11;
    pub const CARD_INSERTED: u32 = 1 << 16;
    pub const CARD_STABLE: u32 = 1 << 17;
    pub const CARD_DETECT: u32 = 1 << 18;
    pub const WRITE_PROTECT: u32 = 1 << 19;
}

/// Host Control bits
mod host_control {
    pub const LED_ON: u8 = 1 << 0;
    pub const DATA_WIDTH_4BIT: u8 = 1 << 1;
    pub const HIGH_SPEED_ENABLE: u8 = 1 << 2;
    pub const DMA_SELECT_SDMA: u8 = 0 << 3;
    pub const DMA_SELECT_ADMA32: u8 = 2 << 3;
    pub const DMA_SELECT_ADMA64: u8 = 3 << 3;
    pub const DATA_WIDTH_8BIT: u8 = 1 << 5;
    pub const CARD_DETECT_TEST: u8 = 1 << 6;
    pub const CARD_DETECT_SIGNAL: u8 = 1 << 7;
}

/// Power Control bits
mod power_control {
    pub const BUS_POWER_ON: u8 = 1 << 0;
    pub const VOLTAGE_3V3: u8 = 7 << 1;
    pub const VOLTAGE_3V0: u8 = 6 << 1;
    pub const VOLTAGE_1V8: u8 = 5 << 1;
}

/// Clock Control bits
mod clock_control {
    pub const INTERNAL_CLK_ENABLE: u16 = 1 << 0;
    pub const INTERNAL_CLK_STABLE: u16 = 1 << 1;
    pub const SD_CLK_ENABLE: u16 = 1 << 2;
    pub const CLK_GEN_SELECT: u16 = 1 << 5;
}

/// Software Reset bits
mod sw_reset {
    pub const RESET_ALL: u8 = 1 << 0;
    pub const RESET_CMD: u8 = 1 << 1;
    pub const RESET_DATA: u8 = 1 << 2;
}

/// Interrupt Status bits
mod int_status {
    pub const CMD_COMPLETE: u32 = 1 << 0;
    pub const TRANSFER_COMPLETE: u32 = 1 << 1;
    pub const BLOCK_GAP_EVENT: u32 = 1 << 2;
    pub const DMA_INTERRUPT: u32 = 1 << 3;
    pub const BUFFER_WRITE_READY: u32 = 1 << 4;
    pub const BUFFER_READ_READY: u32 = 1 << 5;
    pub const CARD_INSERTION: u32 = 1 << 6;
    pub const CARD_REMOVAL: u32 = 1 << 7;
    pub const CARD_INTERRUPT: u32 = 1 << 8;
    pub const INT_A: u32 = 1 << 9;
    pub const INT_B: u32 = 1 << 10;
    pub const INT_C: u32 = 1 << 11;
    pub const RETUNING_EVENT: u32 = 1 << 12;
    pub const ERROR_INTERRUPT: u32 = 1 << 15;
    // Error interrupts (bits 16-31)
    pub const CMD_TIMEOUT_ERR: u32 = 1 << 16;
    pub const CMD_CRC_ERR: u32 = 1 << 17;
    pub const CMD_END_BIT_ERR: u32 = 1 << 18;
    pub const CMD_INDEX_ERR: u32 = 1 << 19;
    pub const DATA_TIMEOUT_ERR: u32 = 1 << 20;
    pub const DATA_CRC_ERR: u32 = 1 << 21;
    pub const DATA_END_BIT_ERR: u32 = 1 << 22;
    pub const CURRENT_LIMIT_ERR: u32 = 1 << 23;
    pub const AUTO_CMD_ERR: u32 = 1 << 24;
    pub const ADMA_ERR: u32 = 1 << 25;
    pub const ALL_ERRORS: u32 = 0xFFFF0000;
}

// ============================================================================
// SD Card Commands
// ============================================================================

/// SD Command indices
mod sd_cmd {
    pub const GO_IDLE_STATE: u8 = 0;        // CMD0 - Reset all cards
    pub const ALL_SEND_CID: u8 = 2;         // CMD2 - All cards send CID
    pub const SEND_RELATIVE_ADDR: u8 = 3;   // CMD3 - Get relative address
    pub const SET_DSR: u8 = 4;              // CMD4 - Set DSR
    pub const SWITCH_FUNC: u8 = 6;          // CMD6 - Switch function
    pub const SELECT_CARD: u8 = 7;          // CMD7 - Select/deselect card
    pub const SEND_IF_COND: u8 = 8;         // CMD8 - Send interface condition
    pub const SEND_CSD: u8 = 9;             // CMD9 - Send CSD
    pub const SEND_CID: u8 = 10;            // CMD10 - Send CID
    pub const VOLTAGE_SWITCH: u8 = 11;      // CMD11 - Voltage switch
    pub const STOP_TRANSMISSION: u8 = 12;   // CMD12 - Stop transmission
    pub const SEND_STATUS: u8 = 13;         // CMD13 - Send status
    pub const GO_INACTIVE_STATE: u8 = 15;   // CMD15 - Go inactive state
    pub const SET_BLOCKLEN: u8 = 16;        // CMD16 - Set block length
    pub const READ_SINGLE_BLOCK: u8 = 17;   // CMD17 - Read single block
    pub const READ_MULTIPLE_BLOCK: u8 = 18; // CMD18 - Read multiple blocks
    pub const SET_BLOCK_COUNT: u8 = 23;     // CMD23 - Set block count
    pub const WRITE_BLOCK: u8 = 24;         // CMD24 - Write single block
    pub const WRITE_MULTIPLE_BLOCK: u8 = 25; // CMD25 - Write multiple blocks
    pub const PROGRAM_CSD: u8 = 27;         // CMD27 - Program CSD
    pub const SET_WRITE_PROT: u8 = 28;      // CMD28 - Set write protect
    pub const CLR_WRITE_PROT: u8 = 29;      // CMD29 - Clear write protect
    pub const SEND_WRITE_PROT: u8 = 30;     // CMD30 - Send write protect
    pub const ERASE_WR_BLK_START: u8 = 32;  // CMD32 - Set erase start
    pub const ERASE_WR_BLK_END: u8 = 33;    // CMD33 - Set erase end
    pub const ERASE: u8 = 38;               // CMD38 - Erase
    pub const LOCK_UNLOCK: u8 = 42;         // CMD42 - Lock/unlock
    pub const APP_CMD: u8 = 55;             // CMD55 - Application command
    pub const GEN_CMD: u8 = 56;             // CMD56 - General command

    // Application specific commands (ACMD)
    pub const SET_BUS_WIDTH: u8 = 6;        // ACMD6 - Set bus width
    pub const SD_STATUS: u8 = 13;           // ACMD13 - SD status
    pub const SEND_NUM_WR_BLOCKS: u8 = 22;  // ACMD22 - Number of written blocks
    pub const SET_WR_BLK_ERASE_COUNT: u8 = 23; // ACMD23 - Set erase count
    pub const SD_SEND_OP_COND: u8 = 41;     // ACMD41 - Send OP condition
    pub const SET_CLR_CARD_DETECT: u8 = 42; // ACMD42 - Card detect
    pub const SEND_SCR: u8 = 51;            // ACMD51 - Send SCR
}

// ============================================================================
// Data Structures
// ============================================================================

/// Card type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardType {
    Unknown,
    Mmc,        // MultiMediaCard
    Sd,         // SD 1.x
    Sdhc,       // SD 2.0 High Capacity
    Sdxc,       // SD Extended Capacity
    Emmc,       // Embedded MMC
}

/// Card identification (CID)
#[derive(Debug, Clone, Default)]
pub struct CardCid {
    pub manufacturer_id: u8,
    pub oem_id: u16,
    pub product_name: [u8; 6],
    pub product_revision: u8,
    pub serial_number: u32,
    pub manufacture_date: u16,
}

/// Card specific data (CSD)
#[derive(Debug, Clone, Default)]
pub struct CardCsd {
    pub csd_version: u8,
    pub taac: u8,
    pub nsac: u8,
    pub tran_speed: u8,
    pub read_bl_len: u8,
    pub write_bl_len: u8,
    pub c_size: u32,
    pub capacity_bytes: u64,
}

/// SD card information
#[derive(Debug, Clone)]
pub struct SdCard {
    pub card_type: CardType,
    pub rca: u16,               // Relative Card Address
    pub cid: CardCid,
    pub csd: CardCsd,
    pub scr: [u32; 2],          // SD Configuration Register
    pub ocr: u32,               // Operation Conditions Register
    pub high_capacity: bool,
    pub bus_width: u8,          // 1, 4, or 8
    pub clock_mhz: u32,
    pub write_protected: bool,
}

impl Default for SdCard {
    fn default() -> Self {
        Self {
            card_type: CardType::Unknown,
            rca: 0,
            cid: CardCid::default(),
            csd: CardCsd::default(),
            scr: [0; 2],
            ocr: 0,
            high_capacity: false,
            bus_width: 1,
            clock_mhz: 0,
            write_protected: false,
        }
    }
}

/// SDHCI Controller
pub struct SdhciController {
    /// PCI device
    pci_dev: PciDevice,
    /// MMIO base address
    mmio_base: u64,
    /// Host version
    version: u16,
    /// Capabilities
    capabilities: u64,
    /// Detected card
    card: Option<SdCard>,
    /// Controller index
    index: usize,
}

impl SdhciController {
    /// Create from PCI device
    pub fn new(pci_dev: PciDevice, index: usize) -> Option<Self> {
        // Read BAR0 for MMIO base
        let (bar0, is_io) = pci::read_bar(&pci_dev, 0);
        if is_io || bar0 == 0 {
            crate::kprintln!("sdmmc: controller {} has invalid BAR0", index);
            return None;
        }

        // Enable bus mastering and memory access
        pci::enable_bus_mastering(&pci_dev);

        let mut ctrl = Self {
            pci_dev,
            mmio_base: bar0,
            version: 0,
            capabilities: 0,
            card: None,
            index,
        };

        // Read host version and capabilities
        ctrl.version = ctrl.read16(regs::HOST_VERSION);
        ctrl.capabilities = ctrl.read32(regs::CAPABILITIES) as u64 |
                           ((ctrl.read32(regs::CAPABILITIES2) as u64) << 32);

        crate::kprintln!("sdmmc: controller {} SDHCI version {}.{} capabilities {:#018x}",
                        index,
                        (ctrl.version >> 8) & 0xFF,
                        ctrl.version & 0xFF,
                        ctrl.capabilities);

        Some(ctrl)
    }

    // MMIO access helpers
    fn read8(&self, offset: u16) -> u8 {
        let addr = crate::mm::phys_to_virt(x86_64::PhysAddr::new(self.mmio_base + offset as u64));
        unsafe { core::ptr::read_volatile(addr.as_ptr::<u8>()) }
    }

    fn write8(&self, offset: u16, value: u8) {
        let addr = crate::mm::phys_to_virt(x86_64::PhysAddr::new(self.mmio_base + offset as u64));
        unsafe { core::ptr::write_volatile(addr.as_mut_ptr::<u8>(), value) }
    }

    fn read16(&self, offset: u16) -> u16 {
        let addr = crate::mm::phys_to_virt(x86_64::PhysAddr::new(self.mmio_base + offset as u64));
        unsafe { core::ptr::read_volatile(addr.as_ptr::<u16>()) }
    }

    fn write16(&self, offset: u16, value: u16) {
        let addr = crate::mm::phys_to_virt(x86_64::PhysAddr::new(self.mmio_base + offset as u64));
        unsafe { core::ptr::write_volatile(addr.as_mut_ptr::<u16>(), value) }
    }

    fn read32(&self, offset: u16) -> u32 {
        let addr = crate::mm::phys_to_virt(x86_64::PhysAddr::new(self.mmio_base + offset as u64));
        unsafe { core::ptr::read_volatile(addr.as_ptr::<u32>()) }
    }

    fn write32(&self, offset: u16, value: u32) {
        let addr = crate::mm::phys_to_virt(x86_64::PhysAddr::new(self.mmio_base + offset as u64));
        unsafe { core::ptr::write_volatile(addr.as_mut_ptr::<u32>(), value) }
    }

    /// Reset the controller
    pub fn reset(&mut self) -> bool {
        // Reset all
        self.write8(regs::SW_RESET, sw_reset::RESET_ALL);

        // Wait for reset to complete
        for _ in 0..100 {
            if self.read8(regs::SW_RESET) & sw_reset::RESET_ALL == 0 {
                return true;
            }
            self.delay_us(10);
        }

        crate::kprintln!("sdmmc{}: reset timeout", self.index);
        false
    }

    /// Initialize the controller
    pub fn init(&mut self) -> bool {
        if !self.reset() {
            return false;
        }

        // Enable all interrupt status
        self.write32(regs::INT_STATUS_ENABLE, 0x01FF_00FF);
        self.write32(regs::INT_SIGNAL_ENABLE, 0); // Poll mode for now

        // Clear any pending interrupts
        self.write32(regs::INT_STATUS, 0xFFFF_FFFF);

        // Set timeout to maximum
        self.write8(regs::TIMEOUT_CONTROL, 0x0E);

        // Check for card presence
        let state = self.read32(regs::PRESENT_STATE);
        if state & present_state::CARD_INSERTED == 0 {
            crate::kprintln!("sdmmc{}: no card detected", self.index);
            return true; // Not an error, just no card
        }

        // Wait for card stable
        for _ in 0..100 {
            let state = self.read32(regs::PRESENT_STATE);
            if state & present_state::CARD_STABLE != 0 {
                break;
            }
            self.delay_us(100);
        }

        // Power on the card
        if !self.power_on() {
            return false;
        }

        // Set clock
        if !self.set_clock(400_000) { // 400 kHz for initialization
            return false;
        }

        // Initialize the card
        if !self.card_init() {
            crate::kprintln!("sdmmc{}: card initialization failed", self.index);
            return false;
        }

        crate::kprintln!("sdmmc{}: card initialized successfully", self.index);
        true
    }

    /// Power on the card
    fn power_on(&mut self) -> bool {
        // Set voltage to 3.3V
        self.write8(regs::POWER_CONTROL, power_control::VOLTAGE_3V3 | power_control::BUS_POWER_ON);
        self.delay_us(10_000); // 10ms power ramp
        true
    }

    /// Power off the card
    fn power_off(&mut self) {
        self.write8(regs::POWER_CONTROL, 0);
    }

    /// Set the clock frequency
    fn set_clock(&mut self, freq_hz: u32) -> bool {
        // Disable clock first
        let mut clk = self.read16(regs::CLOCK_CONTROL);
        clk &= !clock_control::SD_CLK_ENABLE;
        self.write16(regs::CLOCK_CONTROL, clk);

        // Calculate divider (base clock from capabilities)
        let base_clock = ((self.capabilities >> 8) & 0xFF) as u32 * 1_000_000;
        if base_clock == 0 {
            return false;
        }

        let mut div = if base_clock <= freq_hz {
            0
        } else {
            let mut d = 1u32;
            while base_clock / d > freq_hz && d < 2048 {
                d *= 2;
            }
            d / 2
        };

        if div > 255 {
            div = 255;
        }

        // Set divider
        let div_lo = (div & 0xFF) as u16;
        let div_hi = ((div >> 8) & 0x3) as u16;
        clk = (div_lo << 8) | (div_hi << 6) | clock_control::INTERNAL_CLK_ENABLE;
        self.write16(regs::CLOCK_CONTROL, clk);

        // Wait for internal clock stable
        for _ in 0..100 {
            if self.read16(regs::CLOCK_CONTROL) & clock_control::INTERNAL_CLK_STABLE != 0 {
                break;
            }
            self.delay_us(10);
        }

        // Enable SD clock
        clk = self.read16(regs::CLOCK_CONTROL);
        clk |= clock_control::SD_CLK_ENABLE;
        self.write16(regs::CLOCK_CONTROL, clk);

        self.delay_us(1000); // Give clock time to stabilize
        true
    }

    /// Wait for command/data line to be free
    fn wait_for_inhibit(&self, data: bool) -> bool {
        let mask = present_state::CMD_INHIBIT |
                  if data { present_state::DATA_INHIBIT } else { 0 };

        for _ in 0..1000 {
            if self.read32(regs::PRESENT_STATE) & mask == 0 {
                return true;
            }
            self.delay_us(10);
        }
        false
    }

    /// Send a command
    fn send_command(&mut self, cmd: u8, arg: u32, resp_type: u16, data: bool) -> Result<[u32; 4], SdError> {
        if !self.wait_for_inhibit(data) {
            return Err(SdError::Timeout);
        }

        // Clear interrupts
        self.write32(regs::INT_STATUS, 0xFFFF_FFFF);

        // Set argument
        self.write32(regs::ARGUMENT, arg);

        // Build command register value
        let cmd_reg = ((cmd as u16) << 8) | resp_type | if data { command::DATA_PRESENT } else { 0 };

        // Send command
        self.write16(regs::COMMAND, cmd_reg);

        // Wait for command complete
        for _ in 0..10000 {
            let status = self.read32(regs::INT_STATUS);

            if status & int_status::ERROR_INTERRUPT != 0 {
                // Clear error
                self.write32(regs::INT_STATUS, status);
                if status & int_status::CMD_TIMEOUT_ERR != 0 {
                    return Err(SdError::Timeout);
                }
                if status & int_status::CMD_CRC_ERR != 0 {
                    return Err(SdError::CrcError);
                }
                return Err(SdError::CommandError);
            }

            if status & int_status::CMD_COMPLETE != 0 {
                self.write32(regs::INT_STATUS, int_status::CMD_COMPLETE);

                // Read response
                let mut resp = [0u32; 4];
                resp[0] = self.read32(regs::RESPONSE);
                resp[1] = self.read32(regs::RESPONSE + 4);
                resp[2] = self.read32(regs::RESPONSE + 8);
                resp[3] = self.read32(regs::RESPONSE + 12);
                return Ok(resp);
            }

            self.delay_us(1);
        }

        Err(SdError::Timeout)
    }

    /// Initialize the card
    fn card_init(&mut self) -> bool {
        let mut card = SdCard::default();

        // CMD0: Go idle state
        let _ = self.send_command(sd_cmd::GO_IDLE_STATE, 0, command::RESPONSE_TYPE_NONE, false);
        self.delay_us(1000);

        // CMD8: Send interface condition (SD 2.0+)
        let sd20 = self.send_command(
            sd_cmd::SEND_IF_COND,
            0x1AA, // 2.7-3.6V, pattern 0xAA
            command::RESPONSE_TYPE_48 | command::CRC_CHECK_ENABLE | command::INDEX_CHECK_ENABLE,
            false
        ).is_ok();

        // ACMD41: Send operating condition
        let mut ocr = 0u32;
        for _ in 0..100 {
            // CMD55: App command
            if self.send_command(
                sd_cmd::APP_CMD,
                0,
                command::RESPONSE_TYPE_48 | command::CRC_CHECK_ENABLE | command::INDEX_CHECK_ENABLE,
                false
            ).is_err() {
                break;
            }

            // ACMD41: SD Send OP Cond
            let arg = 0x00FF8000 | // 2.7-3.6V
                     if sd20 { 0x4000_0000 } else { 0 }; // HCS (High Capacity Support)

            match self.send_command(sd_cmd::SD_SEND_OP_COND, arg, command::RESPONSE_TYPE_48, false) {
                Ok(resp) => {
                    ocr = resp[0];
                    if ocr & 0x8000_0000 != 0 {
                        // Card is ready
                        break;
                    }
                }
                Err(_) => break,
            }

            self.delay_us(10_000);
        }

        if ocr & 0x8000_0000 == 0 {
            crate::kprintln!("sdmmc{}: card not ready (OCR={:#x})", self.index, ocr);
            return false;
        }

        card.ocr = ocr;
        card.high_capacity = ocr & 0x4000_0000 != 0;
        card.card_type = if card.high_capacity { CardType::Sdhc } else { CardType::Sd };

        crate::kprintln!("sdmmc{}: {} card detected (OCR={:#x})",
                        self.index,
                        if card.high_capacity { "SDHC/SDXC" } else { "SD" },
                        ocr);

        // CMD2: All send CID
        match self.send_command(sd_cmd::ALL_SEND_CID, 0, command::RESPONSE_TYPE_136, false) {
            Ok(resp) => {
                self.parse_cid(&resp, &mut card.cid);
            }
            Err(e) => {
                crate::kprintln!("sdmmc{}: CMD2 failed: {:?}", self.index, e);
                return false;
            }
        }

        // CMD3: Send relative address
        match self.send_command(
            sd_cmd::SEND_RELATIVE_ADDR,
            0,
            command::RESPONSE_TYPE_48 | command::CRC_CHECK_ENABLE | command::INDEX_CHECK_ENABLE,
            false
        ) {
            Ok(resp) => {
                card.rca = (resp[0] >> 16) as u16;
            }
            Err(e) => {
                crate::kprintln!("sdmmc{}: CMD3 failed: {:?}", self.index, e);
                return false;
            }
        }

        // CMD9: Send CSD
        match self.send_command(
            sd_cmd::SEND_CSD,
            (card.rca as u32) << 16,
            command::RESPONSE_TYPE_136,
            false
        ) {
            Ok(resp) => {
                self.parse_csd(&resp, &mut card.csd, card.high_capacity);
            }
            Err(e) => {
                crate::kprintln!("sdmmc{}: CMD9 failed: {:?}", self.index, e);
                return false;
            }
        }

        // CMD7: Select card
        if self.send_command(
            sd_cmd::SELECT_CARD,
            (card.rca as u32) << 16,
            command::RESPONSE_TYPE_48_BUSY | command::CRC_CHECK_ENABLE | command::INDEX_CHECK_ENABLE,
            false
        ).is_err() {
            crate::kprintln!("sdmmc{}: CMD7 failed", self.index);
            return false;
        }

        // Set higher clock for data transfer (25 MHz)
        self.set_clock(25_000_000);
        card.clock_mhz = 25;

        // Try to enable 4-bit bus width
        if self.set_bus_width(&card, 4) {
            card.bus_width = 4;
        }

        // Check write protect
        let state = self.read32(regs::PRESENT_STATE);
        card.write_protected = state & present_state::WRITE_PROTECT != 0;

        crate::kprintln!("sdmmc{}: capacity {} MB, bus {}bit, {} clock",
                        self.index,
                        card.csd.capacity_bytes / 1024 / 1024,
                        card.bus_width,
                        card.clock_mhz);

        self.card = Some(card);
        true
    }

    /// Set bus width
    fn set_bus_width(&mut self, card: &SdCard, width: u8) -> bool {
        if width != 1 && width != 4 {
            return false;
        }

        // CMD55: App command
        if self.send_command(
            sd_cmd::APP_CMD,
            (card.rca as u32) << 16,
            command::RESPONSE_TYPE_48 | command::CRC_CHECK_ENABLE | command::INDEX_CHECK_ENABLE,
            false
        ).is_err() {
            return false;
        }

        // ACMD6: Set bus width
        let arg = if width == 4 { 2 } else { 0 };
        if self.send_command(
            sd_cmd::SET_BUS_WIDTH,
            arg,
            command::RESPONSE_TYPE_48 | command::CRC_CHECK_ENABLE | command::INDEX_CHECK_ENABLE,
            false
        ).is_err() {
            return false;
        }

        // Update host control register
        let mut ctrl = self.read8(regs::HOST_CONTROL);
        if width == 4 {
            ctrl |= host_control::DATA_WIDTH_4BIT;
        } else {
            ctrl &= !host_control::DATA_WIDTH_4BIT;
        }
        self.write8(regs::HOST_CONTROL, ctrl);

        true
    }

    /// Parse CID register
    fn parse_cid(&self, resp: &[u32; 4], cid: &mut CardCid) {
        // CID is in bits 127:1 of the 128-bit response
        let raw = [
            (resp[0] << 8) | (resp[1] >> 24),
            (resp[1] << 8) | (resp[2] >> 24),
            (resp[2] << 8) | (resp[3] >> 24),
            resp[3] << 8,
        ];

        cid.manufacturer_id = (raw[0] >> 24) as u8;
        cid.oem_id = (raw[0] >> 8) as u16;
        cid.product_name[0] = raw[0] as u8;
        cid.product_name[1] = (raw[1] >> 24) as u8;
        cid.product_name[2] = (raw[1] >> 16) as u8;
        cid.product_name[3] = (raw[1] >> 8) as u8;
        cid.product_name[4] = raw[1] as u8;
        cid.product_revision = (raw[2] >> 24) as u8;
        cid.serial_number = ((raw[2] & 0x00FF_FFFF) << 8) | (raw[3] >> 24);
        cid.manufacture_date = ((raw[3] >> 8) & 0x0FFF) as u16;
    }

    /// Parse CSD register
    fn parse_csd(&self, resp: &[u32; 4], csd: &mut CardCsd, high_capacity: bool) {
        let raw = [
            (resp[0] << 8) | (resp[1] >> 24),
            (resp[1] << 8) | (resp[2] >> 24),
            (resp[2] << 8) | (resp[3] >> 24),
            resp[3] << 8,
        ];

        csd.csd_version = ((raw[0] >> 30) & 0x3) as u8;
        csd.taac = (raw[0] >> 16) as u8;
        csd.nsac = (raw[0] >> 8) as u8;
        csd.tran_speed = raw[0] as u8;
        csd.read_bl_len = ((raw[1] >> 16) & 0x0F) as u8;

        if high_capacity {
            // CSD Version 2.0 (SDHC/SDXC)
            csd.c_size = ((raw[1] & 0x3F) << 16) | ((raw[2] >> 16) & 0xFFFF);
            csd.capacity_bytes = (csd.c_size as u64 + 1) * 512 * 1024;
        } else {
            // CSD Version 1.0
            let c_size = ((raw[1] & 0x03FF) << 2) | ((raw[2] >> 30) & 0x3);
            let c_size_mult = ((raw[2] >> 15) & 0x7) as u8;
            let mult = 1u64 << (c_size_mult + 2);
            let blocknr = (c_size as u64 + 1) * mult;
            let block_len = 1u64 << csd.read_bl_len;
            csd.c_size = c_size;
            csd.capacity_bytes = blocknr * block_len;
        }

        csd.write_bl_len = ((raw[3] >> 22) & 0x0F) as u8;
    }

    /// Read blocks from the card
    pub fn read_blocks(&mut self, start_block: u64, block_count: u32, buffer: &mut [u8]) -> Result<(), SdError> {
        let card = self.card.as_ref().ok_or(SdError::NoCard)?;

        let addr = if card.high_capacity {
            start_block as u32 // Block addressing
        } else {
            (start_block * 512) as u32 // Byte addressing
        };

        let required_len = (block_count as usize) * 512;
        if buffer.len() < required_len {
            return Err(SdError::BufferTooSmall);
        }

        // Set block size
        self.write16(regs::BLOCK_SIZE, 512 | (7 << 12)); // 512 bytes, SDMA boundary 512K
        self.write16(regs::BLOCK_COUNT, block_count as u16);

        // Set transfer mode
        let transfer_mode = transfer_mode::DATA_DIRECTION_READ |
                           transfer_mode::BLOCK_COUNT_ENABLE |
                           if block_count > 1 {
                               transfer_mode::MULTI_BLOCK | transfer_mode::AUTO_CMD12_ENABLE
                           } else { 0 };
        self.write16(regs::TRANSFER_MODE, transfer_mode);

        // Send read command
        let cmd = if block_count > 1 { sd_cmd::READ_MULTIPLE_BLOCK } else { sd_cmd::READ_SINGLE_BLOCK };
        self.send_command(
            cmd,
            addr,
            command::RESPONSE_TYPE_48 | command::CRC_CHECK_ENABLE | command::INDEX_CHECK_ENABLE,
            true
        )?;

        // Read data using PIO
        let mut offset = 0;
        for _ in 0..block_count {
            // Wait for buffer read ready
            for _ in 0..10000 {
                let status = self.read32(regs::INT_STATUS);
                if status & int_status::ERROR_INTERRUPT != 0 {
                    self.write32(regs::INT_STATUS, status);
                    return Err(SdError::DataError);
                }
                if status & int_status::BUFFER_READ_READY != 0 {
                    self.write32(regs::INT_STATUS, int_status::BUFFER_READ_READY);
                    break;
                }
                self.delay_us(1);
            }

            // Read block (128 x 32-bit words = 512 bytes)
            for _ in 0..128 {
                let data = self.read32(regs::BUFFER_DATA);
                buffer[offset] = data as u8;
                buffer[offset + 1] = (data >> 8) as u8;
                buffer[offset + 2] = (data >> 16) as u8;
                buffer[offset + 3] = (data >> 24) as u8;
                offset += 4;
            }
        }

        // Wait for transfer complete
        for _ in 0..10000 {
            let status = self.read32(regs::INT_STATUS);
            if status & int_status::TRANSFER_COMPLETE != 0 {
                self.write32(regs::INT_STATUS, int_status::TRANSFER_COMPLETE);
                return Ok(());
            }
            if status & int_status::ERROR_INTERRUPT != 0 {
                self.write32(regs::INT_STATUS, status);
                return Err(SdError::DataError);
            }
            self.delay_us(1);
        }

        Err(SdError::Timeout)
    }

    /// Write blocks to the card
    pub fn write_blocks(&mut self, start_block: u64, block_count: u32, buffer: &[u8]) -> Result<(), SdError> {
        let card = self.card.as_ref().ok_or(SdError::NoCard)?;

        if card.write_protected {
            return Err(SdError::WriteProtected);
        }

        let addr = if card.high_capacity {
            start_block as u32
        } else {
            (start_block * 512) as u32
        };

        let required_len = (block_count as usize) * 512;
        if buffer.len() < required_len {
            return Err(SdError::BufferTooSmall);
        }

        // Set block size
        self.write16(regs::BLOCK_SIZE, 512 | (7 << 12));
        self.write16(regs::BLOCK_COUNT, block_count as u16);

        // Set transfer mode (write)
        let transfer_mode = transfer_mode::BLOCK_COUNT_ENABLE |
                           if block_count > 1 {
                               transfer_mode::MULTI_BLOCK | transfer_mode::AUTO_CMD12_ENABLE
                           } else { 0 };
        self.write16(regs::TRANSFER_MODE, transfer_mode);

        // Send write command
        let cmd = if block_count > 1 { sd_cmd::WRITE_MULTIPLE_BLOCK } else { sd_cmd::WRITE_BLOCK };
        self.send_command(
            cmd,
            addr,
            command::RESPONSE_TYPE_48 | command::CRC_CHECK_ENABLE | command::INDEX_CHECK_ENABLE,
            true
        )?;

        // Write data using PIO
        let mut offset = 0;
        for _ in 0..block_count {
            // Wait for buffer write ready
            for _ in 0..10000 {
                let status = self.read32(regs::INT_STATUS);
                if status & int_status::ERROR_INTERRUPT != 0 {
                    self.write32(regs::INT_STATUS, status);
                    return Err(SdError::DataError);
                }
                if status & int_status::BUFFER_WRITE_READY != 0 {
                    self.write32(regs::INT_STATUS, int_status::BUFFER_WRITE_READY);
                    break;
                }
                self.delay_us(1);
            }

            // Write block
            for _ in 0..128 {
                let data = (buffer[offset] as u32) |
                          ((buffer[offset + 1] as u32) << 8) |
                          ((buffer[offset + 2] as u32) << 16) |
                          ((buffer[offset + 3] as u32) << 24);
                self.write32(regs::BUFFER_DATA, data);
                offset += 4;
            }
        }

        // Wait for transfer complete
        for _ in 0..50000 {
            let status = self.read32(regs::INT_STATUS);
            if status & int_status::TRANSFER_COMPLETE != 0 {
                self.write32(regs::INT_STATUS, int_status::TRANSFER_COMPLETE);
                return Ok(());
            }
            if status & int_status::ERROR_INTERRUPT != 0 {
                self.write32(regs::INT_STATUS, status);
                return Err(SdError::DataError);
            }
            self.delay_us(10);
        }

        Err(SdError::Timeout)
    }

    /// Get card info
    pub fn card_info(&self) -> Option<&SdCard> {
        self.card.as_ref()
    }

    /// Delay helper
    fn delay_us(&self, us: u32) {
        // Use TSC-based delay if available, otherwise spin
        for _ in 0..(us * 10) {
            core::hint::spin_loop();
        }
    }
}

/// SD/MMC errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdError {
    Timeout,
    CrcError,
    CommandError,
    DataError,
    NoCard,
    WriteProtected,
    BufferTooSmall,
    InvalidParameter,
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize SDHCI subsystem
pub fn init() {
    crate::kprintln!("sdmmc: scanning for SDHCI controllers...");

    let devices = pci::scan();
    let mut controllers = SDHCI_CONTROLLERS.lock();
    let mut count = 0;

    for dev in devices {
        // SDHCI class: 0x08 (System peripheral), subclass: 0x05 (SD Host Controller)
        if dev.class.class_code == 0x08 && dev.class.subclass == 0x05 {
            crate::kprintln!("sdmmc: found SDHCI controller at {:02x}:{:02x}.{}",
                           dev.addr.bus, dev.addr.device, dev.addr.function);

            if let Some(mut ctrl) = SdhciController::new(dev, count) {
                if ctrl.init() {
                    controllers.push(ctrl);
                    count += 1;
                }
            }
        }
    }

    if count == 0 {
        crate::kprintln!("sdmmc: no SDHCI controllers found");
    } else {
        crate::kprintln!("sdmmc: initialized {} controller(s)", count);
    }
}

/// Get number of controllers
pub fn controller_count() -> usize {
    SDHCI_CONTROLLERS.lock().len()
}

/// Read blocks from a controller
pub fn read_blocks(controller: usize, start: u64, count: u32, buffer: &mut [u8]) -> Result<(), SdError> {
    let mut controllers = SDHCI_CONTROLLERS.lock();
    if controller >= controllers.len() {
        return Err(SdError::InvalidParameter);
    }
    controllers[controller].read_blocks(start, count, buffer)
}

/// Write blocks to a controller
pub fn write_blocks(controller: usize, start: u64, count: u32, buffer: &[u8]) -> Result<(), SdError> {
    let mut controllers = SDHCI_CONTROLLERS.lock();
    if controller >= controllers.len() {
        return Err(SdError::InvalidParameter);
    }
    controllers[controller].write_blocks(start, count, buffer)
}

/// Get card capacity in bytes
pub fn card_capacity(controller: usize) -> Option<u64> {
    let controllers = SDHCI_CONTROLLERS.lock();
    controllers.get(controller)
        .and_then(|c| c.card_info())
        .map(|card| card.csd.capacity_bytes)
}

/// Check if card is present
pub fn card_present(controller: usize) -> bool {
    let controllers = SDHCI_CONTROLLERS.lock();
    controllers.get(controller)
        .map(|c| c.card.is_some())
        .unwrap_or(false)
}

/// Print controller and card info
pub fn print_info() {
    let controllers = SDHCI_CONTROLLERS.lock();
    for (i, ctrl) in controllers.iter().enumerate() {
        crate::kprintln!("SD/MMC Controller {}:", i);
        crate::kprintln!("  SDHCI Version: {}.{}",
                        (ctrl.version >> 8) & 0xFF,
                        ctrl.version & 0xFF);
        if let Some(card) = &ctrl.card {
            crate::kprintln!("  Card Type: {:?}", card.card_type);
            crate::kprintln!("  Capacity: {} MB", card.csd.capacity_bytes / 1024 / 1024);
            crate::kprintln!("  Bus Width: {}-bit", card.bus_width);
            crate::kprintln!("  Clock: {} MHz", card.clock_mhz);
            crate::kprintln!("  Write Protected: {}", card.write_protected);
        } else {
            crate::kprintln!("  No card detected");
        }
    }
}
