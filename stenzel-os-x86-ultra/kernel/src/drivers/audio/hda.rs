//! Intel High Definition Audio (HDA) Driver
//!
//! Implements the Intel HDA specification for audio playback and capture.
//! Supports codecs like Realtek, Conexant, and others.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use core::ptr::{read_volatile, write_volatile};
use spin::Mutex;

use super::{AudioCapabilities, AudioConfig, AudioDevice, SampleFormat, StreamDirection, StreamHandle, StreamState};
use crate::drivers::pci::{self, PciDevice};
use crate::mm;

/// HDA Controller Register Offsets
mod reg {
    pub const GCAP: u16 = 0x00;       // Global Capabilities
    pub const VMIN: u16 = 0x02;       // Minor Version
    pub const VMAJ: u16 = 0x03;       // Major Version
    pub const OUTPAY: u16 = 0x04;     // Output Payload Capability
    pub const INPAY: u16 = 0x06;      // Input Payload Capability
    pub const GCTL: u16 = 0x08;       // Global Control
    pub const WAKEEN: u16 = 0x0C;     // Wake Enable
    pub const STATESTS: u16 = 0x0E;   // State Change Status
    pub const GSTS: u16 = 0x10;       // Global Status
    pub const OUTSTRMPAY: u16 = 0x18; // Output Stream Payload Capability
    pub const INSTRMPAY: u16 = 0x1A;  // Input Stream Payload Capability
    pub const INTCTL: u16 = 0x20;     // Interrupt Control
    pub const INTSTS: u16 = 0x24;     // Interrupt Status
    pub const WALCLK: u16 = 0x30;     // Wall Clock Counter
    pub const SSYNC: u16 = 0x38;      // Stream Synchronization
    pub const CORBLBASE: u16 = 0x40;  // CORB Lower Base Address
    pub const CORBUBASE: u16 = 0x44;  // CORB Upper Base Address
    pub const CORBWP: u16 = 0x48;     // CORB Write Pointer
    pub const CORBRP: u16 = 0x4A;     // CORB Read Pointer
    pub const CORBCTL: u16 = 0x4C;    // CORB Control
    pub const CORBSTS: u16 = 0x4D;    // CORB Status
    pub const CORBSIZE: u16 = 0x4E;   // CORB Size
    pub const RIRBLBASE: u16 = 0x50;  // RIRB Lower Base Address
    pub const RIRBUBASE: u16 = 0x54;  // RIRB Upper Base Address
    pub const RIRBWP: u16 = 0x58;     // RIRB Write Pointer
    pub const RINTCNT: u16 = 0x5A;    // Response Interrupt Count
    pub const RIRBCTL: u16 = 0x5C;    // RIRB Control
    pub const RIRBSTS: u16 = 0x5D;    // RIRB Status
    pub const RIRBSIZE: u16 = 0x5E;   // RIRB Size
    pub const ICOI: u16 = 0x60;       // Immediate Command Output Interface
    pub const ICII: u16 = 0x64;       // Immediate Command Input Interface
    pub const ICIS: u16 = 0x68;       // Immediate Command Status
    pub const DPIBLBASE: u16 = 0x70;  // DMA Position Buffer Lower Base
    pub const DPIBUBASE: u16 = 0x74;  // DMA Position Buffer Upper Base
    pub const SD0CTL: u16 = 0x80;     // Stream Descriptor 0 Control (base for streams)
}

/// GCTL register bits
mod gctl {
    pub const CRST: u32 = 1 << 0;      // Controller Reset
    pub const FCNTRL: u32 = 1 << 1;    // Flush Control
    pub const UNSOL: u32 = 1 << 8;     // Accept Unsolicited Response Enable
}

/// CORBCTL register bits
mod corbctl {
    pub const MEIE: u8 = 1 << 0;       // Memory Error Interrupt Enable
    pub const RUN: u8 = 1 << 1;        // Run
}

/// RIRBCTL register bits
mod rirbctl {
    pub const RINTCTL: u8 = 1 << 0;    // Response Interrupt Control
    pub const DMAEN: u8 = 1 << 1;      // DMA Enable
    pub const OVERRUN_IC: u8 = 1 << 2; // Overrun Interrupt Control
}

/// Stream Descriptor register offsets (relative to SDnCTL base)
mod sd {
    pub const CTL: u16 = 0x00;         // Control
    pub const STS: u16 = 0x03;         // Status
    pub const LPIB: u16 = 0x04;        // Link Position in Buffer
    pub const CBL: u16 = 0x08;         // Cyclic Buffer Length
    pub const LVI: u16 = 0x0C;         // Last Valid Index
    pub const FIFOW: u16 = 0x0E;       // FIFO Watermark
    pub const FIFOS: u16 = 0x10;       // FIFO Size
    pub const FMT: u16 = 0x12;         // Format
    pub const BDPL: u16 = 0x18;        // Buffer Descriptor List Pointer (Lower)
    pub const BDPU: u16 = 0x1C;        // Buffer Descriptor List Pointer (Upper)
}

/// Stream Descriptor Control bits
mod sdctl {
    pub const SRST: u32 = 1 << 0;      // Stream Reset
    pub const RUN: u32 = 1 << 1;       // Stream Run
    pub const IOCE: u32 = 1 << 2;      // Interrupt on Completion Enable
    pub const FEIE: u32 = 1 << 3;      // FIFO Error Interrupt Enable
    pub const DEIE: u32 = 1 << 4;      // Descriptor Error Interrupt Enable
    pub const STRIPE_MASK: u32 = 0x3 << 16; // Stripe Control
    pub const TP: u32 = 1 << 18;       // Traffic Priority
    pub const DIR: u32 = 1 << 19;      // Bidirectional Direction Control
    pub const STREAM_MASK: u32 = 0xF << 20; // Stream Number
}

/// HDA Codec Verbs
mod verb {
    // Get Parameter
    pub const GET_PARAM: u32 = 0xF00;
    // Set/Get Converter Format
    pub const SET_CONV_FORMAT: u32 = 0x2;
    pub const GET_CONV_FORMAT: u32 = 0xA;
    // Set/Get Amp Gain/Mute
    pub const SET_AMP_GAIN: u32 = 0x3;
    pub const GET_AMP_GAIN: u32 = 0xB;
    // Set/Get Processing Coefficient
    pub const SET_PROC_COEF: u32 = 0x4;
    pub const GET_PROC_COEF: u32 = 0xC;
    // Set/Get Coefficient Index
    pub const SET_COEF_INDEX: u32 = 0x5;
    pub const GET_COEF_INDEX: u32 = 0xD;
    // Set/Get Connection Select
    pub const SET_CONN_SELECT: u32 = 0x701;
    pub const GET_CONN_SELECT: u32 = 0xF01;
    // Get Connection List Entry
    pub const GET_CONN_LIST: u32 = 0xF02;
    // Set/Get Processing State
    pub const SET_PROC_STATE: u32 = 0x703;
    pub const GET_PROC_STATE: u32 = 0xF03;
    // Set/Get Pin Widget Control
    pub const SET_PIN_CTRL: u32 = 0x707;
    pub const GET_PIN_CTRL: u32 = 0xF07;
    // Set/Get Unsolicited Response
    pub const SET_UNSOL_RESP: u32 = 0x708;
    pub const GET_UNSOL_RESP: u32 = 0xF08;
    // Set/Get Pin Sense
    pub const SET_PIN_SENSE: u32 = 0x709;
    pub const GET_PIN_SENSE: u32 = 0xF09;
    // Set/Get EAPD/BTL Enable
    pub const SET_EAPD: u32 = 0x70C;
    pub const GET_EAPD: u32 = 0xF0C;
    // Set/Get Power State
    pub const SET_POWER_STATE: u32 = 0x705;
    pub const GET_POWER_STATE: u32 = 0xF05;
    // Set/Get Converter Channel Count
    pub const SET_CONV_CHANNEL_COUNT: u32 = 0x72D;
    pub const GET_CONV_CHANNEL_COUNT: u32 = 0xF2D;
    // Set/Get Volume Knob Control
    pub const SET_VOLUME_KNOB: u32 = 0x70F;
    pub const GET_VOLUME_KNOB: u32 = 0xF0F;
    // Set/Get GPIO Data
    pub const SET_GPIO_DATA: u32 = 0x715;
    pub const GET_GPIO_DATA: u32 = 0xF15;
    // Set/Get GPIO Mask
    pub const SET_GPIO_MASK: u32 = 0x716;
    pub const GET_GPIO_MASK: u32 = 0xF16;
    // Set/Get GPIO Direction
    pub const SET_GPIO_DIR: u32 = 0x717;
    pub const GET_GPIO_DIR: u32 = 0xF17;
    // Set/Get Config Default
    pub const SET_CONFIG_DEFAULT: u32 = 0x71C;
    pub const GET_CONFIG_DEFAULT: u32 = 0xF1C;
}

/// HDA Codec Parameters
mod param {
    pub const VENDOR_ID: u8 = 0x00;
    pub const REVISION_ID: u8 = 0x02;
    pub const SUBORD_NODE_COUNT: u8 = 0x04;
    pub const FUNC_GROUP_TYPE: u8 = 0x05;
    pub const AUDIO_FUNC_GROUP_CAP: u8 = 0x08;
    pub const AUDIO_WIDGET_CAP: u8 = 0x09;
    pub const SAMPLE_SIZE_RATE_CAP: u8 = 0x0A;
    pub const STREAM_FORMATS: u8 = 0x0B;
    pub const PIN_CAP: u8 = 0x0C;
    pub const INPUT_AMP_CAP: u8 = 0x0D;
    pub const OUTPUT_AMP_CAP: u8 = 0x12;
    pub const CONN_LIST_LEN: u8 = 0x0E;
    pub const POWER_STATES: u8 = 0x0F;
    pub const PROCESSING_CAP: u8 = 0x10;
    pub const GPIO_COUNT: u8 = 0x11;
    pub const VOLUME_KNOB_CAP: u8 = 0x13;
}

/// Widget types
mod widget_type {
    pub const AUDIO_OUTPUT: u8 = 0x0;
    pub const AUDIO_INPUT: u8 = 0x1;
    pub const AUDIO_MIXER: u8 = 0x2;
    pub const AUDIO_SELECTOR: u8 = 0x3;
    pub const PIN_COMPLEX: u8 = 0x4;
    pub const POWER_WIDGET: u8 = 0x5;
    pub const VOLUME_KNOB: u8 = 0x6;
    pub const BEEP_GENERATOR: u8 = 0x7;
    pub const VENDOR_DEFINED: u8 = 0xF;
}

/// Buffer Descriptor List Entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct BdlEntry {
    address: u64,
    length: u32,
    ioc: u32,  // Interrupt on Completion (bit 0)
}

/// Codec widget info
#[derive(Debug, Clone)]
struct Widget {
    nid: u8,
    widget_type: u8,
    capabilities: u32,
    connections: Vec<u8>,
}

/// Codec info
#[derive(Debug)]
struct Codec {
    address: u8,
    vendor_id: u32,
    subsystem_id: u32,
    widgets: Vec<Widget>,
    afg_nid: u8,  // Audio Function Group NID
    dac_nid: u8,  // Default DAC NID
    adc_nid: u8,  // Default ADC NID
    out_pin_nid: u8,  // Default output pin NID
    in_pin_nid: u8,   // Default input pin NID
}

/// Stream info
struct StreamInfo {
    handle: StreamHandle,
    direction: StreamDirection,
    state: StreamState,
    config: AudioConfig,
    stream_id: u8,
    buffer: Vec<u8>,
    buffer_phys: u64,
    bdl: Vec<BdlEntry>,
    bdl_phys: u64,
    position: usize,
    volume: u8,
    muted: bool,
}

/// Intel HDA Controller
pub struct HdaController {
    /// PCI device info
    pci_device: PciDevice,
    /// Memory-mapped I/O base address
    mmio_base: u64,
    /// CORB (Command Output Ring Buffer)
    corb: Vec<u32>,
    corb_phys: u64,
    corb_size: usize,
    corb_wp: u16,
    /// RIRB (Response Input Ring Buffer)
    rirb: Vec<u64>,
    rirb_phys: u64,
    rirb_size: usize,
    rirb_rp: u16,
    /// Discovered codecs
    codecs: Vec<Codec>,
    /// Active streams
    streams: Vec<StreamInfo>,
    /// Next stream handle
    next_handle: StreamHandle,
    /// Number of output streams
    num_output_streams: u8,
    /// Number of input streams
    num_input_streams: u8,
    /// Number of bidirectional streams
    num_bidir_streams: u8,
    /// 64-bit addressing support
    supports_64bit: bool,
    /// Device name
    name: String,
}

impl HdaController {
    /// Create a new HDA controller from a PCI device
    pub fn new(pci_device: PciDevice) -> Option<Self> {
        let (bar0, _is_mem) = pci::read_bar(&pci_device, 0);
        if bar0 == 0 {
            return None;
        }
        let mmio_base = bar0 & !0xF;

        crate::kprintln!("hda: found controller at {:#x}", mmio_base);

        let mut controller = HdaController {
            pci_device,
            mmio_base,
            corb: Vec::new(),
            corb_phys: 0,
            corb_size: 0,
            corb_wp: 0,
            rirb: Vec::new(),
            rirb_phys: 0,
            rirb_size: 0,
            rirb_rp: 0,
            codecs: Vec::new(),
            streams: Vec::new(),
            next_handle: 1,
            num_output_streams: 0,
            num_input_streams: 0,
            num_bidir_streams: 0,
            supports_64bit: false,
            name: String::from("Intel HDA"),
        };

        // Initialize controller
        if !controller.init() {
            return None;
        }

        Some(controller)
    }

    fn read_reg8(&self, offset: u16) -> u8 {
        let addr = self.mmio_base + offset as u64;
        let virt = mm::phys_to_virt(x86_64::PhysAddr::new(addr));
        unsafe { read_volatile(virt.as_ptr::<u8>()) }
    }

    fn read_reg16(&self, offset: u16) -> u16 {
        let addr = self.mmio_base + offset as u64;
        let virt = mm::phys_to_virt(x86_64::PhysAddr::new(addr));
        unsafe { read_volatile(virt.as_ptr::<u16>()) }
    }

    fn read_reg32(&self, offset: u16) -> u32 {
        let addr = self.mmio_base + offset as u64;
        let virt = mm::phys_to_virt(x86_64::PhysAddr::new(addr));
        unsafe { read_volatile(virt.as_ptr::<u32>()) }
    }

    fn write_reg8(&mut self, offset: u16, value: u8) {
        let addr = self.mmio_base + offset as u64;
        let virt = mm::phys_to_virt(x86_64::PhysAddr::new(addr));
        unsafe { write_volatile(virt.as_mut_ptr::<u8>(), value) }
    }

    fn write_reg16(&mut self, offset: u16, value: u16) {
        let addr = self.mmio_base + offset as u64;
        let virt = mm::phys_to_virt(x86_64::PhysAddr::new(addr));
        unsafe { write_volatile(virt.as_mut_ptr::<u16>(), value) }
    }

    fn write_reg32(&mut self, offset: u16, value: u32) {
        let addr = self.mmio_base + offset as u64;
        let virt = mm::phys_to_virt(x86_64::PhysAddr::new(addr));
        unsafe { write_volatile(virt.as_mut_ptr::<u32>(), value) }
    }

    /// Initialize the HDA controller
    fn init(&mut self) -> bool {
        // Enable bus mastering and memory space
        let addr = &self.pci_device.addr;
        let cmd = pci::config_read_u16(addr.bus, addr.device, addr.function, 0x04);
        pci::config_write_u16(addr.bus, addr.device, addr.function, 0x04, cmd | 0x6);

        // Read capabilities
        let gcap = self.read_reg16(reg::GCAP);
        self.num_output_streams = ((gcap >> 12) & 0xF) as u8;
        self.num_input_streams = ((gcap >> 8) & 0xF) as u8;
        self.num_bidir_streams = ((gcap >> 3) & 0x1F) as u8;
        self.supports_64bit = (gcap & 1) != 0;

        crate::kprintln!("hda: {} output, {} input, {} bidir streams, 64bit: {}",
            self.num_output_streams, self.num_input_streams,
            self.num_bidir_streams, self.supports_64bit);

        // Reset controller
        if !self.reset() {
            return false;
        }

        // Set up CORB/RIRB
        if !self.setup_corb_rirb() {
            return false;
        }

        // Enable interrupts
        self.write_reg32(reg::INTCTL, 0x80000000 | 0x3FFFFFFF);

        // Enumerate codecs
        self.enumerate_codecs();

        crate::kprintln!("hda: found {} codec(s)", self.codecs.len());

        true
    }

    /// Reset the controller
    fn reset(&mut self) -> bool {
        // Enter reset
        self.write_reg32(reg::GCTL, 0);

        // Wait for reset to take effect
        for _ in 0..1000 {
            if (self.read_reg32(reg::GCTL) & gctl::CRST) == 0 {
                break;
            }
            core::hint::spin_loop();
        }

        // Exit reset
        self.write_reg32(reg::GCTL, gctl::CRST);

        // Wait for controller to come out of reset
        for _ in 0..1000 {
            if (self.read_reg32(reg::GCTL) & gctl::CRST) != 0 {
                break;
            }
            core::hint::spin_loop();
        }

        // Wait for codecs to initialize
        for _ in 0..1000 {
            core::hint::spin_loop();
        }

        (self.read_reg32(reg::GCTL) & gctl::CRST) != 0
    }

    /// Set up CORB and RIRB
    fn setup_corb_rirb(&mut self) -> bool {
        // Determine CORB size
        let corbsize = self.read_reg8(reg::CORBSIZE);
        self.corb_size = if (corbsize & 0x40) != 0 {
            256
        } else if (corbsize & 0x20) != 0 {
            16
        } else {
            2
        };

        // Allocate CORB
        self.corb = vec![0u32; self.corb_size];
        // In a real implementation, we'd allocate DMA-capable memory
        self.corb_phys = self.corb.as_ptr() as u64;

        // Set up CORB base address
        self.write_reg32(reg::CORBLBASE, self.corb_phys as u32);
        if self.supports_64bit {
            self.write_reg32(reg::CORBUBASE, (self.corb_phys >> 32) as u32);
        }

        // Set CORB size
        let size_bits = match self.corb_size {
            256 => 2,
            16 => 1,
            _ => 0,
        };
        self.write_reg8(reg::CORBSIZE, size_bits);

        // Reset and start CORB
        self.write_reg16(reg::CORBRP, 0x8000);  // Reset read pointer
        for _ in 0..100 {
            if (self.read_reg16(reg::CORBRP) & 0x8000) != 0 {
                break;
            }
            core::hint::spin_loop();
        }
        self.write_reg16(reg::CORBRP, 0);  // Clear reset
        self.write_reg16(reg::CORBWP, 0);  // Reset write pointer
        self.corb_wp = 0;
        self.write_reg8(reg::CORBCTL, corbctl::RUN);

        // Determine RIRB size
        let rirbsize = self.read_reg8(reg::RIRBSIZE);
        self.rirb_size = if (rirbsize & 0x40) != 0 {
            256
        } else if (rirbsize & 0x20) != 0 {
            16
        } else {
            2
        };

        // Allocate RIRB
        self.rirb = vec![0u64; self.rirb_size];
        self.rirb_phys = self.rirb.as_ptr() as u64;

        // Set up RIRB base address
        self.write_reg32(reg::RIRBLBASE, self.rirb_phys as u32);
        if self.supports_64bit {
            self.write_reg32(reg::RIRBUBASE, (self.rirb_phys >> 32) as u32);
        }

        // Set RIRB size
        let size_bits = match self.rirb_size {
            256 => 2,
            16 => 1,
            _ => 0,
        };
        self.write_reg8(reg::RIRBSIZE, size_bits);

        // Reset RIRB write pointer
        self.write_reg16(reg::RIRBWP, 0x8000);  // Reset
        self.rirb_rp = 0;
        self.write_reg16(reg::RINTCNT, 1);  // Interrupt after every response
        self.write_reg8(reg::RIRBCTL, rirbctl::DMAEN | rirbctl::RINTCTL);

        true
    }

    /// Send a verb to a codec
    fn send_verb(&mut self, codec_addr: u8, nid: u8, verb: u32, data: u16) -> Option<u32> {
        // Build command
        let cmd = ((codec_addr as u32) << 28) |
                  ((nid as u32) << 20) |
                  ((verb & 0xFFF) << 8) |
                  (data as u32);

        // Write to CORB
        self.corb_wp = (self.corb_wp + 1) % self.corb_size as u16;
        self.corb[self.corb_wp as usize] = cmd;
        self.write_reg16(reg::CORBWP, self.corb_wp);

        // Wait for response
        for _ in 0..10000 {
            let rirb_wp = self.read_reg16(reg::RIRBWP) & 0xFF;
            if rirb_wp != self.rirb_rp {
                self.rirb_rp = (self.rirb_rp + 1) % self.rirb_size as u16;
                let response = self.rirb[self.rirb_rp as usize];
                return Some((response & 0xFFFFFFFF) as u32);
            }
            core::hint::spin_loop();
        }

        None
    }

    /// Get parameter from a codec node
    fn get_parameter(&mut self, codec_addr: u8, nid: u8, param: u8) -> u32 {
        self.send_verb(codec_addr, nid, verb::GET_PARAM, param as u16)
            .unwrap_or(0)
    }

    /// Enumerate all connected codecs
    fn enumerate_codecs(&mut self) {
        let statests = self.read_reg16(reg::STATESTS);

        for addr in 0..15 {
            if (statests & (1 << addr)) != 0 {
                if let Some(codec) = self.probe_codec(addr as u8) {
                    self.codecs.push(codec);
                }
            }
        }
    }

    /// Probe a codec at the given address
    fn probe_codec(&mut self, addr: u8) -> Option<Codec> {
        // Get vendor ID
        let vendor_id = self.get_parameter(addr, 0, param::VENDOR_ID);
        if vendor_id == 0 || vendor_id == 0xFFFFFFFF {
            return None;
        }

        crate::kprintln!("hda: codec {} vendor ID: {:#010x}", addr, vendor_id);

        // Get subordinate node count for root
        let subord = self.get_parameter(addr, 0, param::SUBORD_NODE_COUNT);
        let start_nid = ((subord >> 16) & 0xFF) as u8;
        let num_nodes = (subord & 0xFF) as u8;

        crate::kprintln!("hda: root has {} nodes starting at {}", num_nodes, start_nid);

        let mut codec = Codec {
            address: addr,
            vendor_id,
            subsystem_id: 0,
            widgets: Vec::new(),
            afg_nid: 0,
            dac_nid: 0,
            adc_nid: 0,
            out_pin_nid: 0,
            in_pin_nid: 0,
        };

        // Find Audio Function Group
        for nid in start_nid..(start_nid + num_nodes) {
            let fg_type = self.get_parameter(addr, nid, param::FUNC_GROUP_TYPE);
            if (fg_type & 0xFF) == 1 {
                // Audio Function Group
                codec.afg_nid = nid;
                self.enumerate_widgets(addr, nid, &mut codec);
                break;
            }
        }

        Some(codec)
    }

    /// Enumerate widgets in an Audio Function Group
    fn enumerate_widgets(&mut self, addr: u8, afg_nid: u8, codec: &mut Codec) {
        let subord = self.get_parameter(addr, afg_nid, param::SUBORD_NODE_COUNT);
        let start_nid = ((subord >> 16) & 0xFF) as u8;
        let num_nodes = (subord & 0xFF) as u8;

        crate::kprintln!("hda: AFG {} has {} widgets starting at {}", afg_nid, num_nodes, start_nid);

        for nid in start_nid..(start_nid + num_nodes) {
            let cap = self.get_parameter(addr, nid, param::AUDIO_WIDGET_CAP);
            let widget_type = ((cap >> 20) & 0xF) as u8;

            let widget = Widget {
                nid,
                widget_type,
                capabilities: cap,
                connections: Vec::new(),
            };

            // Record default DAC/ADC/PIN
            match widget_type {
                widget_type::AUDIO_OUTPUT if codec.dac_nid == 0 => {
                    codec.dac_nid = nid;
                }
                widget_type::AUDIO_INPUT if codec.adc_nid == 0 => {
                    codec.adc_nid = nid;
                }
                widget_type::PIN_COMPLEX => {
                    let config = self.send_verb(addr, nid, verb::GET_CONFIG_DEFAULT, 0)
                        .unwrap_or(0);
                    let port_connectivity = (config >> 30) & 0x3;
                    let default_device = (config >> 20) & 0xF;

                    // 0 = connected, 1 = no jack, 2 = fixed function, 3 = both
                    if port_connectivity != 1 {
                        // Check if output (headphones, speaker, etc.)
                        if default_device == 0x0 || default_device == 0x1 || default_device == 0x2 {
                            if codec.out_pin_nid == 0 {
                                codec.out_pin_nid = nid;
                            }
                        }
                        // Check if input (mic, line-in, etc.)
                        if default_device == 0x8 || default_device == 0xA {
                            if codec.in_pin_nid == 0 {
                                codec.in_pin_nid = nid;
                            }
                        }
                    }
                }
                _ => {}
            }

            codec.widgets.push(widget);
        }

        crate::kprintln!("hda: codec {} DAC={} ADC={} OUT_PIN={} IN_PIN={}",
            addr, codec.dac_nid, codec.adc_nid, codec.out_pin_nid, codec.in_pin_nid);
    }

    /// Configure a stream for playback
    fn configure_playback_stream(&mut self, stream_id: u8, config: &AudioConfig) -> bool {
        if self.codecs.is_empty() {
            return false;
        }

        let codec = &self.codecs[0];
        let addr = codec.address;
        let dac_nid = codec.dac_nid;
        let pin_nid = codec.out_pin_nid;

        if dac_nid == 0 || pin_nid == 0 {
            return false;
        }

        // Build format value
        let format = self.build_format(config);

        // Set up DAC
        self.send_verb(addr, dac_nid, verb::SET_CONV_FORMAT, format);

        // Set stream and channel
        let stream_channel = ((stream_id as u16) << 4) | 0;
        self.send_verb(addr, dac_nid, 0x706, stream_channel);  // SET_STREAM

        // Enable output pin
        self.send_verb(addr, pin_nid, verb::SET_PIN_CTRL, 0x40);  // OUT_EN

        // Unmute and set volume
        self.send_verb(addr, dac_nid, verb::SET_AMP_GAIN, 0xB000 | 127);  // Output amp, max gain

        true
    }

    /// Build format word from config
    fn build_format(&self, config: &AudioConfig) -> u16 {
        // Base rate: 48kHz
        let base = match config.sample_rate {
            44100 | 22050 | 11025 => 1,  // 44.1 kHz base
            _ => 0,  // 48 kHz base
        };

        // Rate multiplier/divisor
        let (mult, div) = match config.sample_rate {
            8000 => (0, 6),    // 48000 / 6
            11025 => (0, 4),   // 44100 / 4
            16000 => (0, 3),   // 48000 / 3
            22050 => (0, 2),   // 44100 / 2
            32000 => (2, 3),   // 48000 * 2 / 3
            44100 => (0, 0),   // 44100 / 1
            48000 => (0, 0),   // 48000 / 1
            88200 => (1, 0),   // 44100 * 2
            96000 => (1, 0),   // 48000 * 2
            176400 => (3, 0),  // 44100 * 4
            192000 => (3, 0),  // 48000 * 4
            _ => (0, 0),       // Default 48000
        };

        // Bits per sample
        let bits = match config.format {
            SampleFormat::U8 => 0,
            SampleFormat::S16LE | SampleFormat::S16BE => 1,
            SampleFormat::S24LE | SampleFormat::S24BE => 3,
            SampleFormat::S32LE | SampleFormat::S32BE |
            SampleFormat::F32LE | SampleFormat::F32BE => 4,
        };

        // Channels (0 = mono, 1 = stereo, etc.)
        let channels = config.channels.saturating_sub(1) as u16;

        ((base & 1) << 14) |
        ((mult & 7) << 11) |
        ((div & 7) << 8) |
        ((bits & 7) << 4) |
        (channels & 0xF)
    }

    /// Get stream descriptor base address for a stream
    fn stream_base(&self, stream_id: u8) -> u16 {
        reg::SD0CTL + (stream_id as u16 * 0x20)
    }
}

impl AudioDevice for HdaController {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> AudioCapabilities {
        AudioCapabilities {
            name: self.name.clone(),
            supported_rates: vec![8000, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000],
            supported_formats: vec![SampleFormat::S16LE, SampleFormat::S24LE, SampleFormat::S32LE],
            max_channels: 8,
            can_playback: self.num_output_streams > 0 || self.num_bidir_streams > 0,
            can_capture: self.num_input_streams > 0 || self.num_bidir_streams > 0,
        }
    }

    fn open_playback(&mut self, config: &AudioConfig) -> Result<StreamHandle, &'static str> {
        let handle = self.next_handle;
        self.next_handle += 1;

        // Allocate stream ID
        let stream_id = self.streams.len() as u8 + 1;

        // Allocate buffer
        let buffer_size = config.buffer_size;
        let buffer = vec![0u8; buffer_size];

        // Allocate BDL
        let num_entries = (buffer_size / config.period_size).max(2);
        let bdl: Vec<BdlEntry> = (0..num_entries).map(|i| {
            BdlEntry {
                address: 0, // Will be set when actually playing
                length: config.period_size as u32,
                ioc: if i == num_entries - 1 { 1 } else { 0 },
            }
        }).collect();

        let stream = StreamInfo {
            handle,
            direction: StreamDirection::Playback,
            state: StreamState::Stopped,
            config: config.clone(),
            stream_id,
            buffer,
            buffer_phys: 0,
            bdl,
            bdl_phys: 0,
            position: 0,
            volume: 100,
            muted: false,
        };

        self.streams.push(stream);

        // Configure the hardware
        self.configure_playback_stream(stream_id, config);

        Ok(handle)
    }

    fn open_capture(&mut self, config: &AudioConfig) -> Result<StreamHandle, &'static str> {
        let handle = self.next_handle;
        self.next_handle += 1;

        let stream_id = (self.streams.len() as u8) + 1 + self.num_output_streams;

        let buffer = vec![0u8; config.buffer_size];
        let num_entries = (config.buffer_size / config.period_size).max(2);
        let bdl: Vec<BdlEntry> = (0..num_entries).map(|i| {
            BdlEntry {
                address: 0,
                length: config.period_size as u32,
                ioc: if i == num_entries - 1 { 1 } else { 0 },
            }
        }).collect();

        let stream = StreamInfo {
            handle,
            direction: StreamDirection::Capture,
            state: StreamState::Stopped,
            config: config.clone(),
            stream_id,
            buffer,
            buffer_phys: 0,
            bdl,
            bdl_phys: 0,
            position: 0,
            volume: 100,
            muted: false,
        };

        self.streams.push(stream);

        Ok(handle)
    }

    fn close_stream(&mut self, handle: StreamHandle) -> Result<(), &'static str> {
        if let Some(idx) = self.streams.iter().position(|s| s.handle == handle) {
            self.stop_stream(handle)?;
            self.streams.remove(idx);
            Ok(())
        } else {
            Err("Invalid stream handle")
        }
    }

    fn start_stream(&mut self, handle: StreamHandle) -> Result<(), &'static str> {
        // Find stream and get info
        let stream_id = self.streams.iter()
            .find(|s| s.handle == handle)
            .map(|s| s.stream_id)
            .ok_or("Invalid stream handle")?;

        let base = self.stream_base(stream_id);

        // Start the stream
        let mut ctl = self.read_reg32(base + sd::CTL);
        ctl |= sdctl::RUN | sdctl::IOCE;
        self.write_reg32(base + sd::CTL, ctl);

        // Update state
        if let Some(stream) = self.streams.iter_mut().find(|s| s.handle == handle) {
            stream.state = StreamState::Running;
        }
        Ok(())
    }

    fn stop_stream(&mut self, handle: StreamHandle) -> Result<(), &'static str> {
        // Find stream and get info
        let stream_id = self.streams.iter()
            .find(|s| s.handle == handle)
            .map(|s| s.stream_id)
            .ok_or("Invalid stream handle")?;

        let base = self.stream_base(stream_id);

        // Stop the stream
        let mut ctl = self.read_reg32(base + sd::CTL);
        ctl &= !sdctl::RUN;
        self.write_reg32(base + sd::CTL, ctl);

        // Update state
        if let Some(stream) = self.streams.iter_mut().find(|s| s.handle == handle) {
            stream.state = StreamState::Stopped;
        }
        Ok(())
    }

    fn write(&mut self, handle: StreamHandle, data: &[u8]) -> Result<usize, &'static str> {
        let stream = self.streams.iter_mut().find(|s| s.handle == handle)
            .ok_or("Invalid stream handle")?;

        if stream.direction != StreamDirection::Playback {
            return Err("Not a playback stream");
        }

        let available = stream.buffer.len() - stream.position;
        let to_write = data.len().min(available);

        stream.buffer[stream.position..stream.position + to_write]
            .copy_from_slice(&data[..to_write]);
        stream.position += to_write;

        // Wrap around
        if stream.position >= stream.buffer.len() {
            stream.position = 0;
        }

        Ok(to_write)
    }

    fn read(&mut self, handle: StreamHandle, data: &mut [u8]) -> Result<usize, &'static str> {
        let stream = self.streams.iter_mut().find(|s| s.handle == handle)
            .ok_or("Invalid stream handle")?;

        if stream.direction != StreamDirection::Capture {
            return Err("Not a capture stream");
        }

        let available = stream.position;
        let to_read = data.len().min(available);

        data[..to_read].copy_from_slice(&stream.buffer[..to_read]);

        // Shift remaining data
        stream.buffer.copy_within(to_read..stream.position, 0);
        stream.position -= to_read;

        Ok(to_read)
    }

    fn stream_state(&self, handle: StreamHandle) -> Option<StreamState> {
        self.streams.iter().find(|s| s.handle == handle).map(|s| s.state)
    }

    fn available(&self, handle: StreamHandle) -> usize {
        self.streams.iter().find(|s| s.handle == handle)
            .map(|s| s.buffer.len() - s.position)
            .unwrap_or(0)
    }

    fn set_volume(&mut self, handle: StreamHandle, volume: u8) -> Result<(), &'static str> {
        let stream = self.streams.iter_mut().find(|s| s.handle == handle)
            .ok_or("Invalid stream handle")?;

        stream.volume = volume.min(100);

        // Update hardware volume
        if let Some(codec) = self.codecs.first() {
            let gain = (stream.volume as u16) * 127 / 100;
            let addr = codec.address;
            let dac_nid = codec.dac_nid;
            if dac_nid != 0 {
                self.send_verb(addr, dac_nid, verb::SET_AMP_GAIN, 0xB000 | gain);
            }
        }

        Ok(())
    }

    fn get_volume(&self, handle: StreamHandle) -> u8 {
        self.streams.iter().find(|s| s.handle == handle)
            .map(|s| s.volume)
            .unwrap_or(0)
    }

    fn set_mute(&mut self, handle: StreamHandle, muted: bool) -> Result<(), &'static str> {
        let stream = self.streams.iter_mut().find(|s| s.handle == handle)
            .ok_or("Invalid stream handle")?;

        stream.muted = muted;

        // Update hardware mute
        if let Some(codec) = self.codecs.first() {
            let addr = codec.address;
            let dac_nid = codec.dac_nid;
            if dac_nid != 0 {
                let gain = if muted { 0x8000 } else { 0xB000 | ((stream.volume as u16) * 127 / 100) };
                self.send_verb(addr, dac_nid, verb::SET_AMP_GAIN, gain);
            }
        }

        Ok(())
    }

    fn is_muted(&self, handle: StreamHandle) -> bool {
        self.streams.iter().find(|s| s.handle == handle)
            .map(|s| s.muted)
            .unwrap_or(false)
    }
}

/// Global list of discovered HDA controllers
static HDA_CONTROLLERS: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// Initialize HDA audio subsystem
pub fn init() {
    crate::kprintln!("hda: scanning for Intel HDA controllers");

    // Scan PCI for HDA controllers
    // Class 0x04 = Multimedia, Subclass 0x03 = Audio device
    let all_devices = pci::scan();
    let audio_devices: Vec<_> = all_devices.into_iter()
        .filter(|d| d.class.class_code == 0x04 && d.class.subclass == 0x03)
        .collect();

    for device in audio_devices {
        crate::kprintln!("hda: found audio device {:04x}:{:04x} at {:02x}:{:02x}.{}",
            device.id.vendor_id, device.id.device_id,
            device.addr.bus, device.addr.device, device.addr.function);

        if let Some(controller) = HdaController::new(device) {
            super::AUDIO_SYSTEM.lock().register_device(controller);
        }
    }

    let count = super::AUDIO_SYSTEM.lock().device_count();
    crate::kprintln!("hda: initialized {} audio device(s)", count);
}
