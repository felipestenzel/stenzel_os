//! General Purpose I/O (GPIO) Driver
//!
//! Implements GPIO support for Intel and AMD platforms:
//! - Intel Sunrise Point (100 series) and later PCH
//! - AMD FCH GPIO
//! - Pin configuration and control
//! - Interrupt support
//! - GPIO-based device detection

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::drivers::pci;
use crate::mm;
use crate::sync::IrqSafeMutex;

/// GPIO pin direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioDirection {
    Input,
    Output,
}

/// GPIO pin value
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioValue {
    Low,
    High,
}

impl From<bool> for GpioValue {
    fn from(v: bool) -> Self {
        if v { GpioValue::High } else { GpioValue::Low }
    }
}

impl From<GpioValue> for bool {
    fn from(v: GpioValue) -> Self {
        v == GpioValue::High
    }
}

/// GPIO pull configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioPull {
    None,
    PullUp,
    PullDown,
}

/// GPIO interrupt trigger mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioTrigger {
    None,
    RisingEdge,
    FallingEdge,
    BothEdges,
    LevelHigh,
    LevelLow,
}

/// GPIO pad ownership
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioPadOwner {
    /// Owned by Host (CPU)
    Host,
    /// Owned by ACPI
    Acpi,
    /// Owned by GPIO driver
    GpioDriver,
}

/// GPIO controller type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioControllerType {
    IntelSunrisePoint,    // 100 series
    IntelCannonLake,      // 300 series
    IntelTigerLake,       // 500 series
    IntelAlderLake,       // 600 series
    IntelRaptorLake,      // 700 series
    AmdFch,               // AMD Fusion Controller Hub
    Unknown,
}

impl GpioControllerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            GpioControllerType::IntelSunrisePoint => "Intel Sunrise Point",
            GpioControllerType::IntelCannonLake => "Intel Cannon Lake",
            GpioControllerType::IntelTigerLake => "Intel Tiger Lake",
            GpioControllerType::IntelAlderLake => "Intel Alder Lake",
            GpioControllerType::IntelRaptorLake => "Intel Raptor Lake",
            GpioControllerType::AmdFch => "AMD FCH",
            GpioControllerType::Unknown => "Unknown",
        }
    }
}

/// GPIO community (Intel) - groups of GPIO pins
#[derive(Debug, Clone)]
pub struct GpioCommunity {
    /// Community index
    pub index: u8,
    /// Community name
    pub name: String,
    /// Base MMIO address
    pub mmio_base: u64,
    /// Number of pads
    pub num_pads: u32,
    /// First pad number
    pub first_pad: u32,
}

/// GPIO pin configuration
#[derive(Debug, Clone)]
pub struct GpioPinConfig {
    /// Pin number (global)
    pub pin: u32,
    /// Direction
    pub direction: GpioDirection,
    /// Pull configuration
    pub pull: GpioPull,
    /// Interrupt trigger
    pub trigger: GpioTrigger,
    /// Current value
    pub value: GpioValue,
    /// Pad owner
    pub owner: GpioPadOwner,
    /// Pin is locked (cannot be reconfigured)
    pub locked: bool,
    /// Native function number (0 = GPIO mode)
    pub native_function: u8,
    /// Pin name/label
    pub label: String,
}

impl GpioPinConfig {
    pub fn new(pin: u32) -> Self {
        Self {
            pin,
            direction: GpioDirection::Input,
            pull: GpioPull::None,
            trigger: GpioTrigger::None,
            value: GpioValue::Low,
            owner: GpioPadOwner::Host,
            locked: false,
            native_function: 0,
            label: String::new(),
        }
    }
}

/// GPIO error type
#[derive(Debug, Clone)]
pub enum GpioError {
    InvalidPin,
    PinLocked,
    NotGpioMode,
    NotConfigured,
    InvalidOperation,
    HardwareError,
    NotSupported,
}

pub type GpioResult<T> = Result<T, GpioError>;

/// Intel GPIO register offsets (per community)
mod intel_regs {
    pub const PADBAR: u32 = 0x00C;          // Pad base address register
    pub const HOSTSW_OWN: u32 = 0x080;      // Host software ownership
    pub const GPI_IS: u32 = 0x100;          // GPI interrupt status
    pub const GPI_IE: u32 = 0x110;          // GPI interrupt enable
    pub const GPI_GPE_STS: u32 = 0x140;     // GPI GPE status
    pub const GPI_GPE_EN: u32 = 0x150;      // GPI GPE enable
    pub const PAD_CFG_LOCK: u32 = 0x0A0;    // Pad configuration lock
    pub const PAD_CFG_LOCK_TX: u32 = 0x0A4; // Pad configuration lock TX

    // Per-pad registers (offset from PADBAR + pin * 16)
    pub const PAD_CFG_DW0: u32 = 0x00;      // Pad configuration DW0
    pub const PAD_CFG_DW1: u32 = 0x04;      // Pad configuration DW1
}

/// Intel PAD_CFG_DW0 bits
mod pad_cfg_dw0 {
    pub const GPIORXDIS: u32 = 1 << 9;      // RX disable
    pub const GPIOTXDIS: u32 = 1 << 8;      // TX disable
    pub const RXSTATE: u32 = 1 << 1;        // RX state
    pub const GPIOTXSTATE: u32 = 1 << 0;    // TX state

    // Pad mode (bits 12:10)
    pub const PMODE_SHIFT: u32 = 10;
    pub const PMODE_MASK: u32 = 0x7;

    // RX invert (bit 23)
    pub const RXINV: u32 = 1 << 23;

    // RX/TX buffer enable (bits 9:8)
    pub const RXBUF_DIS: u32 = 1 << 9;
    pub const TXBUF_DIS: u32 = 1 << 8;

    // Interrupt configuration (bits 19:17)
    pub const RXEVCFG_SHIFT: u32 = 17;
    pub const RXEVCFG_MASK: u32 = 0x3;
    pub const RXEVCFG_LEVEL: u32 = 0;
    pub const RXEVCFG_EDGE: u32 = 1;
    pub const RXEVCFG_DISABLE: u32 = 2;
    pub const RXEVCFG_BOTH: u32 = 3;
}

/// Intel PAD_CFG_DW1 bits
mod pad_cfg_dw1 {
    // Term (pull-up/down) bits 13:10
    pub const TERM_SHIFT: u32 = 10;
    pub const TERM_MASK: u32 = 0xF;
    pub const TERM_NONE: u32 = 0;
    pub const TERM_5K_PD: u32 = 2;
    pub const TERM_20K_PD: u32 = 4;
    pub const TERM_1K_PU: u32 = 9;
    pub const TERM_2K_PU: u32 = 11;
    pub const TERM_5K_PU: u32 = 10;
    pub const TERM_20K_PU: u32 = 12;
    pub const TERM_667_PU: u32 = 13;
    pub const TERM_NATIVE: u32 = 15;

    // Interrupt select bits 7:0
    pub const INTSEL_SHIFT: u32 = 0;
    pub const INTSEL_MASK: u32 = 0xFF;
}

/// AMD FCH GPIO register offsets
mod amd_regs {
    pub const GPIO_BANK_SELECT: u32 = 0x00;
    pub const GPIO_OUTPUT: u32 = 0x80;
    pub const GPIO_INPUT: u32 = 0xA0;
    pub const GPIO_CONTROL: u32 = 0xC0;
}

/// GPIO Controller
pub struct GpioController {
    /// Controller type
    controller_type: GpioControllerType,
    /// Communities (Intel) or single base (AMD)
    communities: Vec<GpioCommunity>,
    /// Pin configurations
    pins: BTreeMap<u32, GpioPinConfig>,
    /// Total number of pins
    total_pins: u32,
    /// Interrupt handlers
    interrupt_handlers: BTreeMap<u32, fn(u32)>,
    /// Initialized flag
    initialized: AtomicBool,
}

impl GpioController {
    pub const fn new() -> Self {
        Self {
            controller_type: GpioControllerType::Unknown,
            communities: Vec::new(),
            pins: BTreeMap::new(),
            total_pins: 0,
            interrupt_handlers: BTreeMap::new(),
            initialized: AtomicBool::new(false),
        }
    }

    /// Initialize the GPIO controller
    pub fn init(&mut self) -> GpioResult<()> {
        // Detect controller type from ACPI/PCI
        self.detect_controller()?;

        match self.controller_type {
            GpioControllerType::IntelSunrisePoint
            | GpioControllerType::IntelCannonLake
            | GpioControllerType::IntelTigerLake
            | GpioControllerType::IntelAlderLake
            | GpioControllerType::IntelRaptorLake => {
                self.init_intel()?;
            }
            GpioControllerType::AmdFch => {
                self.init_amd()?;
            }
            GpioControllerType::Unknown => {
                crate::kprintln!("gpio: no supported GPIO controller found");
                return Err(GpioError::NotSupported);
            }
        }

        self.initialized.store(true, Ordering::Release);
        crate::kprintln!(
            "gpio: initialized {} with {} pins",
            self.controller_type.as_str(),
            self.total_pins
        );

        Ok(())
    }

    /// Detect GPIO controller type
    fn detect_controller(&mut self) -> GpioResult<()> {
        // Try to detect from PCI
        let devices = pci::scan();

        let mut has_intel = false;
        let mut has_amd = false;

        for dev in devices {
            // Intel GPIO controllers are in ISA bridge or Serial bus controller
            if dev.class.class_code == 0x0C && dev.class.subclass == 0x80 {
                // Serial Bus Controller - SMBus/GPIO
                let device_family = dev.id.device_id >> 8;
                match (dev.id.vendor_id, device_family) {
                    (0x8086, 0xA1) | (0x8086, 0x9D) => {
                        self.controller_type = GpioControllerType::IntelSunrisePoint;
                        return Ok(());
                    }
                    (0x8086, 0xA3) => {
                        self.controller_type = GpioControllerType::IntelCannonLake;
                        return Ok(());
                    }
                    (0x8086, 0xA0) => {
                        self.controller_type = GpioControllerType::IntelTigerLake;
                        return Ok(());
                    }
                    (0x8086, 0x51) | (0x8086, 0x7A) => {
                        self.controller_type = GpioControllerType::IntelAlderLake;
                        return Ok(());
                    }
                    _ => {}
                }
            }

            // Track vendor presence for fallback
            if dev.id.vendor_id == 0x8086 {
                has_intel = true;
            }

            // AMD FCH
            if dev.id.vendor_id == 0x1022 {
                has_amd = true;
                if dev.class.class_code == 0x06 && dev.class.subclass == 0x01 {
                    // ISA bridge
                    self.controller_type = GpioControllerType::AmdFch;
                    return Ok(());
                }
            }
        }

        // Default based on detected platform
        if has_intel {
            self.controller_type = GpioControllerType::IntelSunrisePoint;
        } else if has_amd {
            self.controller_type = GpioControllerType::AmdFch;
        }

        Ok(())
    }

    /// Initialize Intel GPIO controller
    fn init_intel(&mut self) -> GpioResult<()> {
        // Get GPIO base from ACPI or P2SB
        // For now, use typical base addresses
        let gpio_bases = match self.controller_type {
            GpioControllerType::IntelSunrisePoint => vec![
                (0xFD6D_0000u64, 24, "GPP_A", 0),  // Community 0
                (0xFD6C_0000u64, 24, "GPP_B", 24), // Community 1
                (0xFD6B_0000u64, 24, "GPP_C", 48), // Community 2
                (0xFD6A_0000u64, 24, "GPP_D", 72), // Community 3
            ],
            GpioControllerType::IntelCannonLake
            | GpioControllerType::IntelTigerLake
            | GpioControllerType::IntelAlderLake
            | GpioControllerType::IntelRaptorLake => vec![
                (0xFD6D_0000u64, 26, "GPP_A", 0),
                (0xFD6C_0000u64, 26, "GPP_B", 26),
                (0xFD6B_0000u64, 26, "GPP_C", 52),
                (0xFD6A_0000u64, 26, "GPP_D", 78),
                (0xFD69_0000u64, 13, "GPP_E", 104),
            ],
            _ => return Err(GpioError::NotSupported),
        };

        for (i, (base, num_pads, name, first_pad)) in gpio_bases.iter().enumerate() {
            let community = GpioCommunity {
                index: i as u8,
                name: String::from(*name),
                mmio_base: *base,
                num_pads: *num_pads,
                first_pad: *first_pad,
            };
            self.communities.push(community);
            self.total_pins += *num_pads;
        }

        // Initialize pin configurations
        for community in &self.communities {
            for pad_offset in 0..community.num_pads {
                let pin = community.first_pad + pad_offset;
                let config = self.read_intel_pad_config(community, pad_offset)?;
                self.pins.insert(pin, config);
            }
        }

        Ok(())
    }

    /// Read Intel pad configuration
    fn read_intel_pad_config(&self, community: &GpioCommunity, pad_offset: u32) -> GpioResult<GpioPinConfig> {
        let pin = community.first_pad + pad_offset;
        let mut config = GpioPinConfig::new(pin);

        // Calculate pad register offset
        let virt = mm::phys_to_virt(x86_64::PhysAddr::new(community.mmio_base));

        // Read PADBAR to get pad config base
        let padbar = unsafe {
            let addr = virt.as_ptr::<u32>().add(intel_regs::PADBAR as usize / 4);
            core::ptr::read_volatile(addr)
        };

        let pad_base = (padbar & 0xFFFF) as u64 + (pad_offset as u64 * 16);
        let pad_virt = mm::phys_to_virt(x86_64::PhysAddr::new(community.mmio_base + pad_base));

        // Read DW0 and DW1
        let dw0 = unsafe {
            core::ptr::read_volatile(pad_virt.as_ptr::<u32>())
        };
        let dw1 = unsafe {
            core::ptr::read_volatile(pad_virt.as_ptr::<u32>().add(1))
        };

        // Parse pad mode
        let pmode = (dw0 >> pad_cfg_dw0::PMODE_SHIFT) & pad_cfg_dw0::PMODE_MASK;
        config.native_function = pmode as u8;

        // Parse direction
        let rx_dis = (dw0 & pad_cfg_dw0::GPIORXDIS) != 0;
        let tx_dis = (dw0 & pad_cfg_dw0::GPIOTXDIS) != 0;

        if tx_dis && !rx_dis {
            config.direction = GpioDirection::Input;
        } else if !tx_dis {
            config.direction = GpioDirection::Output;
        }

        // Parse current value
        if config.direction == GpioDirection::Input {
            config.value = if (dw0 & pad_cfg_dw0::RXSTATE) != 0 {
                GpioValue::High
            } else {
                GpioValue::Low
            };
        } else {
            config.value = if (dw0 & pad_cfg_dw0::GPIOTXSTATE) != 0 {
                GpioValue::High
            } else {
                GpioValue::Low
            };
        }

        // Parse pull configuration
        let term = (dw1 >> pad_cfg_dw1::TERM_SHIFT) & pad_cfg_dw1::TERM_MASK;
        config.pull = match term {
            pad_cfg_dw1::TERM_NONE => GpioPull::None,
            pad_cfg_dw1::TERM_5K_PD | pad_cfg_dw1::TERM_20K_PD => GpioPull::PullDown,
            _ if term >= 9 => GpioPull::PullUp,
            _ => GpioPull::None,
        };

        // Parse interrupt configuration
        let rxevcfg = (dw0 >> pad_cfg_dw0::RXEVCFG_SHIFT) & pad_cfg_dw0::RXEVCFG_MASK;
        config.trigger = match rxevcfg {
            pad_cfg_dw0::RXEVCFG_LEVEL => {
                if (dw0 & pad_cfg_dw0::RXINV) != 0 {
                    GpioTrigger::LevelLow
                } else {
                    GpioTrigger::LevelHigh
                }
            }
            pad_cfg_dw0::RXEVCFG_EDGE => {
                if (dw0 & pad_cfg_dw0::RXINV) != 0 {
                    GpioTrigger::FallingEdge
                } else {
                    GpioTrigger::RisingEdge
                }
            }
            pad_cfg_dw0::RXEVCFG_BOTH => GpioTrigger::BothEdges,
            _ => GpioTrigger::None,
        };

        config.label = alloc::format!("{}_GPIO{}", community.name, pad_offset);

        Ok(config)
    }

    /// Initialize AMD GPIO controller
    fn init_amd(&mut self) -> GpioResult<()> {
        // AMD FCH GPIO base is typically at 0xFED8_1500
        let gpio_base = 0xFED8_1500u64;

        let community = GpioCommunity {
            index: 0,
            name: String::from("AMD_GPIO"),
            mmio_base: gpio_base,
            num_pads: 256, // AMD has up to 256 GPIOs
            first_pad: 0,
        };

        self.communities.push(community);
        self.total_pins = 256;

        // Initialize all pins
        for pin in 0..256 {
            let mut config = GpioPinConfig::new(pin);
            config.label = alloc::format!("GPIO{}", pin);
            self.pins.insert(pin, config);
        }

        Ok(())
    }

    /// Get pin direction
    pub fn get_direction(&self, pin: u32) -> GpioResult<GpioDirection> {
        let config = self.pins.get(&pin).ok_or(GpioError::InvalidPin)?;
        Ok(config.direction)
    }

    /// Set pin direction
    pub fn set_direction(&mut self, pin: u32, direction: GpioDirection) -> GpioResult<()> {
        let config = self.pins.get_mut(&pin).ok_or(GpioError::InvalidPin)?;

        if config.locked {
            return Err(GpioError::PinLocked);
        }

        if config.native_function != 0 {
            return Err(GpioError::NotGpioMode);
        }

        config.direction = direction;
        self.write_pin_config(pin)?;
        Ok(())
    }

    /// Read pin value
    pub fn read(&self, pin: u32) -> GpioResult<GpioValue> {
        let config = self.pins.get(&pin).ok_or(GpioError::InvalidPin)?;

        if config.native_function != 0 {
            return Err(GpioError::NotGpioMode);
        }

        // Read actual hardware value
        self.read_pin_value(pin)
    }

    /// Write pin value
    pub fn write(&mut self, pin: u32, value: GpioValue) -> GpioResult<()> {
        let config = self.pins.get_mut(&pin).ok_or(GpioError::InvalidPin)?;

        if config.locked {
            return Err(GpioError::PinLocked);
        }

        if config.native_function != 0 {
            return Err(GpioError::NotGpioMode);
        }

        if config.direction != GpioDirection::Output {
            return Err(GpioError::InvalidOperation);
        }

        config.value = value;
        self.write_pin_value(pin, value)?;
        Ok(())
    }

    /// Read pin value from hardware
    fn read_pin_value(&self, pin: u32) -> GpioResult<GpioValue> {
        let (community, pad_offset) = self.find_community(pin)?;

        match self.controller_type {
            GpioControllerType::IntelSunrisePoint
            | GpioControllerType::IntelCannonLake
            | GpioControllerType::IntelTigerLake
            | GpioControllerType::IntelAlderLake
            | GpioControllerType::IntelRaptorLake => {
                let virt = mm::phys_to_virt(x86_64::PhysAddr::new(community.mmio_base));

                let padbar = unsafe {
                    let addr = virt.as_ptr::<u32>().add(intel_regs::PADBAR as usize / 4);
                    core::ptr::read_volatile(addr)
                };

                let pad_base = (padbar & 0xFFFF) as u64 + (pad_offset as u64 * 16);
                let pad_virt = mm::phys_to_virt(x86_64::PhysAddr::new(community.mmio_base + pad_base));

                let dw0 = unsafe {
                    core::ptr::read_volatile(pad_virt.as_ptr::<u32>())
                };

                Ok(if (dw0 & pad_cfg_dw0::RXSTATE) != 0 {
                    GpioValue::High
                } else {
                    GpioValue::Low
                })
            }
            GpioControllerType::AmdFch => {
                let virt = mm::phys_to_virt(x86_64::PhysAddr::new(community.mmio_base));
                let bank = pin / 32;
                let bit = pin % 32;

                let input_reg = unsafe {
                    let addr = virt.as_ptr::<u32>().add((amd_regs::GPIO_INPUT / 4 + bank) as usize);
                    core::ptr::read_volatile(addr)
                };

                Ok(if (input_reg & (1 << bit)) != 0 {
                    GpioValue::High
                } else {
                    GpioValue::Low
                })
            }
            GpioControllerType::Unknown => Err(GpioError::NotSupported),
        }
    }

    /// Write pin value to hardware
    fn write_pin_value(&self, pin: u32, value: GpioValue) -> GpioResult<()> {
        let (community, pad_offset) = self.find_community(pin)?;

        match self.controller_type {
            GpioControllerType::IntelSunrisePoint
            | GpioControllerType::IntelCannonLake
            | GpioControllerType::IntelTigerLake
            | GpioControllerType::IntelAlderLake
            | GpioControllerType::IntelRaptorLake => {
                let virt = mm::phys_to_virt(x86_64::PhysAddr::new(community.mmio_base));

                let padbar = unsafe {
                    let addr = virt.as_ptr::<u32>().add(intel_regs::PADBAR as usize / 4);
                    core::ptr::read_volatile(addr)
                };

                let pad_base = (padbar & 0xFFFF) as u64 + (pad_offset as u64 * 16);
                let pad_virt = mm::phys_to_virt(x86_64::PhysAddr::new(community.mmio_base + pad_base));

                let mut dw0 = unsafe {
                    core::ptr::read_volatile(pad_virt.as_ptr::<u32>())
                };

                if value == GpioValue::High {
                    dw0 |= pad_cfg_dw0::GPIOTXSTATE;
                } else {
                    dw0 &= !pad_cfg_dw0::GPIOTXSTATE;
                }

                unsafe {
                    core::ptr::write_volatile(pad_virt.as_mut_ptr::<u32>(), dw0);
                }

                Ok(())
            }
            GpioControllerType::AmdFch => {
                let virt = mm::phys_to_virt(x86_64::PhysAddr::new(community.mmio_base));
                let bank = pin / 32;
                let bit = pin % 32;

                let output_addr = unsafe {
                    virt.as_mut_ptr::<u32>().add((amd_regs::GPIO_OUTPUT / 4 + bank) as usize)
                };

                let mut output_reg = unsafe { core::ptr::read_volatile(output_addr) };

                if value == GpioValue::High {
                    output_reg |= 1 << bit;
                } else {
                    output_reg &= !(1 << bit);
                }

                unsafe {
                    core::ptr::write_volatile(output_addr, output_reg);
                }

                Ok(())
            }
            GpioControllerType::Unknown => Err(GpioError::NotSupported),
        }
    }

    /// Write pin configuration to hardware
    fn write_pin_config(&self, pin: u32) -> GpioResult<()> {
        let config = self.pins.get(&pin).ok_or(GpioError::InvalidPin)?;
        let (community, pad_offset) = self.find_community(pin)?;

        match self.controller_type {
            GpioControllerType::IntelSunrisePoint
            | GpioControllerType::IntelCannonLake
            | GpioControllerType::IntelTigerLake
            | GpioControllerType::IntelAlderLake
            | GpioControllerType::IntelRaptorLake => {
                let virt = mm::phys_to_virt(x86_64::PhysAddr::new(community.mmio_base));

                let padbar = unsafe {
                    let addr = virt.as_ptr::<u32>().add(intel_regs::PADBAR as usize / 4);
                    core::ptr::read_volatile(addr)
                };

                let pad_base = (padbar & 0xFFFF) as u64 + (pad_offset as u64 * 16);
                let pad_virt = mm::phys_to_virt(x86_64::PhysAddr::new(community.mmio_base + pad_base));

                let mut dw0 = unsafe {
                    core::ptr::read_volatile(pad_virt.as_ptr::<u32>())
                };

                // Set direction
                match config.direction {
                    GpioDirection::Input => {
                        dw0 |= pad_cfg_dw0::GPIOTXDIS;  // Disable TX
                        dw0 &= !pad_cfg_dw0::GPIORXDIS; // Enable RX
                    }
                    GpioDirection::Output => {
                        dw0 &= !pad_cfg_dw0::GPIOTXDIS; // Enable TX
                        dw0 |= pad_cfg_dw0::GPIORXDIS;  // Disable RX
                    }
                }

                // Set trigger mode
                dw0 &= !((pad_cfg_dw0::RXEVCFG_MASK << pad_cfg_dw0::RXEVCFG_SHIFT) | pad_cfg_dw0::RXINV);
                match config.trigger {
                    GpioTrigger::None => {
                        dw0 |= pad_cfg_dw0::RXEVCFG_DISABLE << pad_cfg_dw0::RXEVCFG_SHIFT;
                    }
                    GpioTrigger::RisingEdge => {
                        dw0 |= pad_cfg_dw0::RXEVCFG_EDGE << pad_cfg_dw0::RXEVCFG_SHIFT;
                    }
                    GpioTrigger::FallingEdge => {
                        dw0 |= (pad_cfg_dw0::RXEVCFG_EDGE << pad_cfg_dw0::RXEVCFG_SHIFT) | pad_cfg_dw0::RXINV;
                    }
                    GpioTrigger::BothEdges => {
                        dw0 |= pad_cfg_dw0::RXEVCFG_BOTH << pad_cfg_dw0::RXEVCFG_SHIFT;
                    }
                    GpioTrigger::LevelHigh => {
                        dw0 |= pad_cfg_dw0::RXEVCFG_LEVEL << pad_cfg_dw0::RXEVCFG_SHIFT;
                    }
                    GpioTrigger::LevelLow => {
                        dw0 |= (pad_cfg_dw0::RXEVCFG_LEVEL << pad_cfg_dw0::RXEVCFG_SHIFT) | pad_cfg_dw0::RXINV;
                    }
                }

                unsafe {
                    core::ptr::write_volatile(pad_virt.as_mut_ptr::<u32>(), dw0);
                }

                Ok(())
            }
            GpioControllerType::AmdFch => {
                // AMD FCH GPIO configuration
                let virt = mm::phys_to_virt(x86_64::PhysAddr::new(community.mmio_base));

                // Configure pin control register
                let control_addr = unsafe {
                    virt.as_mut_ptr::<u8>().add(amd_regs::GPIO_CONTROL as usize + pin as usize)
                };

                let mut control: u8 = 0;
                if config.direction == GpioDirection::Output {
                    control |= 0x40; // Output enable
                }

                unsafe {
                    core::ptr::write_volatile(control_addr, control);
                }

                Ok(())
            }
            GpioControllerType::Unknown => Err(GpioError::NotSupported),
        }
    }

    /// Find community and pad offset for a pin
    fn find_community(&self, pin: u32) -> GpioResult<(&GpioCommunity, u32)> {
        for community in &self.communities {
            if pin >= community.first_pad && pin < community.first_pad + community.num_pads {
                return Ok((community, pin - community.first_pad));
            }
        }
        Err(GpioError::InvalidPin)
    }

    /// Set pull configuration
    pub fn set_pull(&mut self, pin: u32, pull: GpioPull) -> GpioResult<()> {
        let config = self.pins.get_mut(&pin).ok_or(GpioError::InvalidPin)?;

        if config.locked {
            return Err(GpioError::PinLocked);
        }

        config.pull = pull;
        // Note: Pull configuration write not implemented in detail
        Ok(())
    }

    /// Set interrupt trigger
    pub fn set_trigger(&mut self, pin: u32, trigger: GpioTrigger) -> GpioResult<()> {
        let config = self.pins.get_mut(&pin).ok_or(GpioError::InvalidPin)?;

        if config.locked {
            return Err(GpioError::PinLocked);
        }

        config.trigger = trigger;
        self.write_pin_config(pin)?;
        Ok(())
    }

    /// Register interrupt handler
    pub fn set_interrupt_handler(&mut self, pin: u32, handler: fn(u32)) -> GpioResult<()> {
        if !self.pins.contains_key(&pin) {
            return Err(GpioError::InvalidPin);
        }
        self.interrupt_handlers.insert(pin, handler);
        Ok(())
    }

    /// Handle interrupt for a pin
    pub fn handle_interrupt(&self, pin: u32) {
        if let Some(handler) = self.interrupt_handlers.get(&pin) {
            handler(pin);
        }
    }

    /// Get pin configuration
    pub fn get_pin_config(&self, pin: u32) -> Option<&GpioPinConfig> {
        self.pins.get(&pin)
    }

    /// List all GPIO pins
    pub fn list_pins(&self) -> Vec<u32> {
        self.pins.keys().copied().collect()
    }

    /// Get controller type
    pub fn controller_type(&self) -> GpioControllerType {
        self.controller_type
    }

    /// Format status as string
    pub fn format_status(&self) -> String {
        let mut output = String::new();

        output.push_str(&alloc::format!(
            "GPIO Controller: {} ({} pins)\n",
            self.controller_type.as_str(),
            self.total_pins
        ));

        for community in &self.communities {
            output.push_str(&alloc::format!(
                "  Community {}: {} - {} pins at {:#x}\n",
                community.index, community.name, community.num_pads, community.mmio_base
            ));
        }

        output
    }
}

// =============================================================================
// Global State
// =============================================================================

static GPIO_CONTROLLER: IrqSafeMutex<GpioController> = IrqSafeMutex::new(GpioController::new());

/// Initialize GPIO subsystem
pub fn init() {
    let mut gpio = GPIO_CONTROLLER.lock();
    match gpio.init() {
        Ok(()) => {}
        Err(e) => {
            crate::kprintln!("gpio: initialization failed: {:?}", e);
        }
    }
}

/// Get a reference to the GPIO controller
pub fn controller() -> impl core::ops::DerefMut<Target = GpioController> {
    GPIO_CONTROLLER.lock()
}

/// Read a GPIO pin
pub fn read(pin: u32) -> GpioResult<GpioValue> {
    GPIO_CONTROLLER.lock().read(pin)
}

/// Write to a GPIO pin
pub fn write(pin: u32, value: GpioValue) -> GpioResult<()> {
    GPIO_CONTROLLER.lock().write(pin, value)
}

/// Set GPIO pin direction
pub fn set_direction(pin: u32, direction: GpioDirection) -> GpioResult<()> {
    GPIO_CONTROLLER.lock().set_direction(pin, direction)
}

/// Get GPIO pin direction
pub fn get_direction(pin: u32) -> GpioResult<GpioDirection> {
    GPIO_CONTROLLER.lock().get_direction(pin)
}

/// Format status
pub fn format_status() -> String {
    GPIO_CONTROLLER.lock().format_status()
}
