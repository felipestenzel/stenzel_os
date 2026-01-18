//! AC'97 Audio Codec driver.
//!
//! Implements support for AC'97 audio codec found in older systems.
//! AC'97 was the standard audio codec before Intel HDA.
//!
//! Features:
//! - Mixer register access
//! - PCM playback/capture
//! - Volume control
//! - Multiple sample rates

#![allow(dead_code)]

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::drivers::pci;
use crate::sync::TicketSpinlock;

/// AC'97 Native Audio Mixer registers (offset from NAMBAR)
pub mod mixer_regs {
    pub const RESET: u16 = 0x00;
    pub const MASTER_VOL: u16 = 0x02;
    pub const AUX_OUT_VOL: u16 = 0x04;
    pub const MONO_VOL: u16 = 0x06;
    pub const MASTER_TONE: u16 = 0x08;
    pub const PC_BEEP_VOL: u16 = 0x0A;
    pub const PHONE_VOL: u16 = 0x0C;
    pub const MIC_VOL: u16 = 0x0E;
    pub const LINE_IN_VOL: u16 = 0x10;
    pub const CD_VOL: u16 = 0x12;
    pub const VIDEO_VOL: u16 = 0x14;
    pub const AUX_IN_VOL: u16 = 0x16;
    pub const PCM_OUT_VOL: u16 = 0x18;
    pub const RECORD_SELECT: u16 = 0x1A;
    pub const RECORD_GAIN: u16 = 0x1C;
    pub const RECORD_GAIN_MIC: u16 = 0x1E;
    pub const GENERAL_PURPOSE: u16 = 0x20;
    pub const THREE_D_CONTROL: u16 = 0x22;
    pub const AUDIO_INT_PAGING: u16 = 0x24;
    pub const POWERDOWN_CTRL: u16 = 0x26;
    pub const EXT_AUDIO_ID: u16 = 0x28;
    pub const EXT_AUDIO_CTRL: u16 = 0x2A;
    pub const PCM_FRONT_DAC_RATE: u16 = 0x2C;
    pub const PCM_SURROUND_DAC_RATE: u16 = 0x2E;
    pub const PCM_LFE_DAC_RATE: u16 = 0x30;
    pub const PCM_ADC_RATE: u16 = 0x32;
    pub const MIC_ADC_RATE: u16 = 0x34;
    pub const CENTER_LFE_VOL: u16 = 0x36;
    pub const SURROUND_VOL: u16 = 0x38;
    pub const S_PDIF_CTRL: u16 = 0x3A;
    pub const VENDOR_ID1: u16 = 0x7C;
    pub const VENDOR_ID2: u16 = 0x7E;
}

/// AC'97 Native Audio Bus Master registers (offset from NABMBAR)
pub mod busmaster_regs {
    // PCM In (capture)
    pub const PI_BDBAR: u16 = 0x00;  // Buffer Descriptor Base Address
    pub const PI_CIV: u16 = 0x04;    // Current Index Value
    pub const PI_LVI: u16 = 0x05;    // Last Valid Index
    pub const PI_SR: u16 = 0x06;     // Status Register
    pub const PI_PICB: u16 = 0x08;   // Position in Current Buffer
    pub const PI_PIV: u16 = 0x0A;    // Prefetched Index Value
    pub const PI_CR: u16 = 0x0B;     // Control Register

    // PCM Out (playback)
    pub const PO_BDBAR: u16 = 0x10;
    pub const PO_CIV: u16 = 0x14;
    pub const PO_LVI: u16 = 0x15;
    pub const PO_SR: u16 = 0x16;
    pub const PO_PICB: u16 = 0x18;
    pub const PO_PIV: u16 = 0x1A;
    pub const PO_CR: u16 = 0x1B;

    // Mic In
    pub const MC_BDBAR: u16 = 0x20;
    pub const MC_CIV: u16 = 0x24;
    pub const MC_LVI: u16 = 0x25;
    pub const MC_SR: u16 = 0x26;
    pub const MC_PICB: u16 = 0x28;
    pub const MC_PIV: u16 = 0x2A;
    pub const MC_CR: u16 = 0x2B;

    // Global
    pub const GLOB_CNT: u16 = 0x2C;  // Global Control
    pub const GLOB_STA: u16 = 0x30;  // Global Status
    pub const CAS: u16 = 0x34;       // Codec Access Semaphore
}

/// Status register bits
pub mod status_bits {
    pub const DMACS: u16 = 1 << 0;   // DMA Controller Status
    pub const CELV: u16 = 1 << 1;    // Current Equals Last Valid
    pub const LVBCI: u16 = 1 << 2;   // Last Valid Buffer Completion Interrupt
    pub const BCIS: u16 = 1 << 3;    // Buffer Completion Interrupt Status
    pub const FIFOE: u16 = 1 << 4;   // FIFO Error
}

/// Control register bits
pub mod control_bits {
    pub const RPBM: u8 = 1 << 0;     // Run/Pause Bus Master
    pub const RR: u8 = 1 << 1;       // Reset Registers
    pub const LVBIE: u8 = 1 << 2;    // Last Valid Buffer Interrupt Enable
    pub const FEIE: u8 = 1 << 3;     // FIFO Error Interrupt Enable
    pub const IOCE: u8 = 1 << 4;     // Interrupt on Completion Enable
}

/// Global control bits
pub mod glob_cnt_bits {
    pub const GIE: u32 = 1 << 0;     // Global Interrupt Enable
    pub const COLD: u32 = 1 << 1;    // Cold Reset
    pub const WARM: u32 = 1 << 2;    // Warm Reset
    pub const SHUT: u32 = 1 << 3;    // Shut Down
    pub const PCM_246_MASK: u32 = 3 << 20;
    pub const PCM_2: u32 = 0 << 20;  // 2 channels
    pub const PCM_4: u32 = 1 << 20;  // 4 channels
    pub const PCM_6: u32 = 2 << 20;  // 6 channels
}

/// Global status bits
pub mod glob_sta_bits {
    pub const GSCI: u32 = 1 << 0;    // GPI Status Change Interrupt
    pub const MIINT: u32 = 1 << 1;   // Modem In Interrupt
    pub const MOINT: u32 = 1 << 2;   // Modem Out Interrupt
    pub const PIINT: u32 = 1 << 5;   // PCM In Interrupt
    pub const POINT: u32 = 1 << 6;   // PCM Out Interrupt
    pub const MINT: u32 = 1 << 7;    // Mic In Interrupt
    pub const PCR: u32 = 1 << 8;     // Primary Codec Ready
    pub const SCR: u32 = 1 << 9;     // Secondary Codec Ready
    pub const TCR: u32 = 1 << 10;    // Tertiary Codec Ready
    pub const PRES: u32 = 1 << 20;   // Primary Resume Interrupt
    pub const SRES: u32 = 1 << 21;   // Secondary Resume Interrupt
    pub const TRES: u32 = 1 << 22;   // Tertiary Resume Interrupt
    pub const MD3: u32 = 1 << 30;    // Modem Power Down Semaphore
    pub const AD3: u32 = 1 << 31;    // Audio Power Down Semaphore
}

/// Extended Audio ID bits
pub mod ext_audio_id {
    pub const VRA: u16 = 1 << 0;     // Variable Rate Audio
    pub const DRA: u16 = 1 << 1;     // Double Rate Audio
    pub const SPDIF: u16 = 1 << 2;   // S/PDIF
    pub const VRM: u16 = 1 << 3;     // Variable Rate Mic
    pub const CDAC: u16 = 1 << 6;    // Center DAC
    pub const SDAC: u16 = 1 << 7;    // Surround DAC
    pub const LDAC: u16 = 1 << 8;    // LFE DAC
    pub const AMAP: u16 = 1 << 9;    // Slot/DAC Map
    pub const REV_MASK: u16 = 0x3 << 10;
}

/// Record source selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum RecordSource {
    Mic = 0,
    Cd = 1,
    VideoIn = 2,
    AuxIn = 3,
    LineIn = 4,
    StereoMix = 5,
    MonoMix = 6,
    Phone = 7,
}

/// Buffer Descriptor Entry (8 bytes)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BufferDescriptor {
    /// Physical address of buffer
    pub address: u32,
    /// Buffer size in samples (not bytes!) and flags
    pub length_flags: u32,
}

impl BufferDescriptor {
    pub const IOC: u32 = 1 << 31;  // Interrupt on Completion
    pub const BUP: u32 = 1 << 30;  // Buffer Underrun Policy

    pub fn new(address: u32, samples: u16, ioc: bool, bup: bool) -> Self {
        let mut flags = samples as u32;
        if ioc { flags |= Self::IOC; }
        if bup { flags |= Self::BUP; }
        Self {
            address,
            length_flags: flags,
        }
    }

    pub fn sample_count(&self) -> u16 {
        (self.length_flags & 0xFFFF) as u16
    }
}

/// AC'97 Codec capabilities
#[derive(Debug, Clone)]
pub struct Ac97Capabilities {
    pub vendor_id: u32,
    pub variable_rate: bool,
    pub double_rate: bool,
    pub variable_rate_mic: bool,
    pub spdif: bool,
    pub center_dac: bool,
    pub surround_dac: bool,
    pub lfe_dac: bool,
    pub max_channels: u8,
    pub supported_rates: Vec<u32>,
}

impl Default for Ac97Capabilities {
    fn default() -> Self {
        Self {
            vendor_id: 0,
            variable_rate: false,
            double_rate: false,
            variable_rate_mic: false,
            spdif: false,
            center_dac: false,
            surround_dac: false,
            lfe_dac: false,
            max_channels: 2,
            supported_rates: vec![48000],
        }
    }
}

/// AC'97 Controller state
#[derive(Debug)]
pub struct Ac97Controller {
    /// Native Audio Mixer BAR (I/O)
    nambar: u16,
    /// Native Audio Bus Master BAR (I/O)
    nabmbar: u16,
    /// PCI location
    bus: u8,
    device: u8,
    function: u8,
    /// Codec capabilities
    capabilities: Ac97Capabilities,
    /// Buffer descriptors for playback (32 entries)
    playback_bd: Box<[BufferDescriptor; 32]>,
    /// Buffer descriptors for capture
    capture_bd: Box<[BufferDescriptor; 32]>,
    /// Playback buffers
    playback_buffers: Vec<Box<[u8; 4096]>>,
    /// Capture buffers
    capture_buffers: Vec<Box<[u8; 4096]>>,
    /// Current sample rate
    sample_rate: u32,
    /// Volume (0-31, 0 = max, 31 = mute)
    master_volume: u8,
    /// Muted
    muted: bool,
    /// Playback active
    playback_active: AtomicBool,
    /// Capture active
    capture_active: AtomicBool,
    /// Initialized
    initialized: bool,
}

impl Ac97Controller {
    pub fn new(bus: u8, device: u8, function: u8, nambar: u16, nabmbar: u16) -> Self {
        Self {
            nambar,
            nabmbar,
            bus,
            device,
            function,
            capabilities: Ac97Capabilities::default(),
            playback_bd: Box::new([BufferDescriptor { address: 0, length_flags: 0 }; 32]),
            capture_bd: Box::new([BufferDescriptor { address: 0, length_flags: 0 }; 32]),
            playback_buffers: Vec::new(),
            capture_buffers: Vec::new(),
            sample_rate: 48000,
            master_volume: 0,
            muted: false,
            playback_active: AtomicBool::new(false),
            capture_active: AtomicBool::new(false),
            initialized: false,
        }
    }

    /// Read from mixer register
    fn read_mixer(&self, reg: u16) -> u16 {
        unsafe {
            let port = self.nambar + reg;
            let value: u16;
            core::arch::asm!(
                "in ax, dx",
                out("ax") value,
                in("dx") port,
                options(nostack, preserves_flags)
            );
            value
        }
    }

    /// Write to mixer register
    fn write_mixer(&self, reg: u16, value: u16) {
        unsafe {
            let port = self.nambar + reg;
            core::arch::asm!(
                "out dx, ax",
                in("dx") port,
                in("ax") value,
                options(nostack, preserves_flags)
            );
        }
    }

    /// Read from bus master register (byte)
    fn read_bm8(&self, reg: u16) -> u8 {
        unsafe {
            let port = self.nabmbar + reg;
            let value: u8;
            core::arch::asm!(
                "in al, dx",
                out("al") value,
                in("dx") port,
                options(nostack, preserves_flags)
            );
            value
        }
    }

    /// Write to bus master register (byte)
    fn write_bm8(&self, reg: u16, value: u8) {
        unsafe {
            let port = self.nabmbar + reg;
            core::arch::asm!(
                "out dx, al",
                in("dx") port,
                in("al") value,
                options(nostack, preserves_flags)
            );
        }
    }

    /// Read from bus master register (word)
    fn read_bm16(&self, reg: u16) -> u16 {
        unsafe {
            let port = self.nabmbar + reg;
            let value: u16;
            core::arch::asm!(
                "in ax, dx",
                out("ax") value,
                in("dx") port,
                options(nostack, preserves_flags)
            );
            value
        }
    }

    /// Write to bus master register (word)
    fn write_bm16(&self, reg: u16, value: u16) {
        unsafe {
            let port = self.nabmbar + reg;
            core::arch::asm!(
                "out dx, ax",
                in("dx") port,
                in("ax") value,
                options(nostack, preserves_flags)
            );
        }
    }

    /// Read from bus master register (dword)
    fn read_bm32(&self, reg: u16) -> u32 {
        unsafe {
            let port = self.nabmbar + reg;
            let value: u32;
            core::arch::asm!(
                "in eax, dx",
                out("eax") value,
                in("dx") port,
                options(nostack, preserves_flags)
            );
            value
        }
    }

    /// Write to bus master register (dword)
    fn write_bm32(&self, reg: u16, value: u32) {
        unsafe {
            let port = self.nabmbar + reg;
            core::arch::asm!(
                "out dx, eax",
                in("dx") port,
                in("eax") value,
                options(nostack, preserves_flags)
            );
        }
    }

    /// Initialize the controller
    pub fn init(&mut self) -> Result<(), &'static str> {
        crate::kprintln!("ac97: initializing controller at {:02X}:{:02X}.{}",
            self.bus, self.device, self.function);

        // Cold reset
        self.write_bm32(busmaster_regs::GLOB_CNT, glob_cnt_bits::COLD);

        // Wait for codec ready
        for _ in 0..1000 {
            let status = self.read_bm32(busmaster_regs::GLOB_STA);
            if status & glob_sta_bits::PCR != 0 {
                break;
            }
            // Small delay
            for _ in 0..10000 { core::hint::spin_loop(); }
        }

        let status = self.read_bm32(busmaster_regs::GLOB_STA);
        if status & glob_sta_bits::PCR == 0 {
            return Err("ac97: codec not ready");
        }

        // Reset codec
        self.write_mixer(mixer_regs::RESET, 0);
        for _ in 0..10000 { core::hint::spin_loop(); }

        // Read vendor ID
        let vid1 = self.read_mixer(mixer_regs::VENDOR_ID1);
        let vid2 = self.read_mixer(mixer_regs::VENDOR_ID2);
        self.capabilities.vendor_id = ((vid1 as u32) << 16) | (vid2 as u32);

        crate::kprintln!("ac97: codec vendor ID: {:08X}", self.capabilities.vendor_id);

        // Read extended capabilities
        let ext_id = self.read_mixer(mixer_regs::EXT_AUDIO_ID);
        self.capabilities.variable_rate = ext_id & ext_audio_id::VRA != 0;
        self.capabilities.double_rate = ext_id & ext_audio_id::DRA != 0;
        self.capabilities.variable_rate_mic = ext_id & ext_audio_id::VRM != 0;
        self.capabilities.spdif = ext_id & ext_audio_id::SPDIF != 0;
        self.capabilities.center_dac = ext_id & ext_audio_id::CDAC != 0;
        self.capabilities.surround_dac = ext_id & ext_audio_id::SDAC != 0;
        self.capabilities.lfe_dac = ext_id & ext_audio_id::LDAC != 0;

        // Determine max channels
        self.capabilities.max_channels = 2;
        if self.capabilities.surround_dac { self.capabilities.max_channels = 4; }
        if self.capabilities.center_dac && self.capabilities.lfe_dac {
            self.capabilities.max_channels = 6;
        }

        // Enable variable rate audio if supported
        if self.capabilities.variable_rate {
            let ext_ctrl = self.read_mixer(mixer_regs::EXT_AUDIO_CTRL);
            self.write_mixer(mixer_regs::EXT_AUDIO_CTRL, ext_ctrl | ext_audio_id::VRA);

            // Supported rates with VRA
            self.capabilities.supported_rates = vec![8000, 11025, 16000, 22050, 32000, 44100, 48000];
        }

        crate::kprintln!("ac97: VRA={}, channels={}, rates={:?}",
            self.capabilities.variable_rate,
            self.capabilities.max_channels,
            self.capabilities.supported_rates);

        // Set default volumes
        self.write_mixer(mixer_regs::MASTER_VOL, 0x0000);  // Max volume
        self.write_mixer(mixer_regs::PCM_OUT_VOL, 0x0808); // -12dB
        self.write_mixer(mixer_regs::MIC_VOL, 0x8008);     // Mic muted by default
        self.write_mixer(mixer_regs::LINE_IN_VOL, 0x8808); // Line in muted
        self.write_mixer(mixer_regs::CD_VOL, 0x8808);      // CD muted

        // Allocate buffers
        self.allocate_buffers();

        // Enable global interrupt
        let glob_cnt = self.read_bm32(busmaster_regs::GLOB_CNT);
        self.write_bm32(busmaster_regs::GLOB_CNT, glob_cnt | glob_cnt_bits::GIE);

        self.initialized = true;
        crate::kprintln!("ac97: initialized successfully");

        Ok(())
    }

    /// Allocate DMA buffers
    fn allocate_buffers(&mut self) {
        // Allocate 32 playback buffers
        for _ in 0..32 {
            self.playback_buffers.push(Box::new([0u8; 4096]));
        }

        // Allocate 32 capture buffers
        for _ in 0..32 {
            self.capture_buffers.push(Box::new([0u8; 4096]));
        }
    }

    /// Set sample rate
    pub fn set_sample_rate(&mut self, rate: u32) -> Result<(), &'static str> {
        if !self.capabilities.variable_rate && rate != 48000 {
            return Err("ac97: VRA not supported, only 48kHz available");
        }

        if !self.capabilities.supported_rates.contains(&rate) {
            return Err("ac97: unsupported sample rate");
        }

        // Set DAC rate
        self.write_mixer(mixer_regs::PCM_FRONT_DAC_RATE, rate as u16);

        // Verify
        let actual = self.read_mixer(mixer_regs::PCM_FRONT_DAC_RATE) as u32;
        if actual != rate {
            crate::kprintln!("ac97: requested {}Hz, got {}Hz", rate, actual);
        }

        self.sample_rate = actual;
        Ok(())
    }

    /// Set master volume (0-100%)
    pub fn set_volume(&mut self, percent: u8) {
        // AC'97 volume: 0 = max, 63 = -94.5dB (1.5dB steps)
        // We use 0-31 range for simplicity (0 = max, 31 = -46.5dB)
        let vol = if percent >= 100 {
            0
        } else if percent == 0 {
            0x8000 // Mute bit
        } else {
            let attenuation = ((100 - percent) * 31) / 100;
            (attenuation as u16) | ((attenuation as u16) << 8)
        };

        self.write_mixer(mixer_regs::MASTER_VOL, vol);
        self.master_volume = if percent >= 100 { 0 } else { ((100 - percent) * 31 / 100) as u8 };
    }

    /// Get master volume (0-100%)
    pub fn get_volume(&self) -> u8 {
        let vol = self.read_mixer(mixer_regs::MASTER_VOL);
        if vol & 0x8000 != 0 {
            return 0; // Muted
        }
        let attenuation = (vol & 0x1F) as u8;
        100 - (attenuation * 100 / 31)
    }

    /// Set mute
    pub fn set_mute(&mut self, mute: bool) {
        let vol = self.read_mixer(mixer_regs::MASTER_VOL);
        if mute {
            self.write_mixer(mixer_regs::MASTER_VOL, vol | 0x8000);
        } else {
            self.write_mixer(mixer_regs::MASTER_VOL, vol & !0x8000);
        }
        self.muted = mute;
    }

    /// Is muted
    pub fn is_muted(&self) -> bool {
        self.read_mixer(mixer_regs::MASTER_VOL) & 0x8000 != 0
    }

    /// Set PCM volume
    pub fn set_pcm_volume(&mut self, percent: u8) {
        let vol = if percent >= 100 {
            0
        } else if percent == 0 {
            0x8000
        } else {
            let attenuation = ((100 - percent) * 31) / 100;
            (attenuation as u16) | ((attenuation as u16) << 8)
        };
        self.write_mixer(mixer_regs::PCM_OUT_VOL, vol);
    }

    /// Set record source
    pub fn set_record_source(&mut self, source: RecordSource) {
        let val = (source as u16) | ((source as u16) << 8);
        self.write_mixer(mixer_regs::RECORD_SELECT, val);
    }

    /// Set record gain (0-15)
    pub fn set_record_gain(&mut self, gain: u8) {
        let g = (gain & 0x0F) as u16;
        self.write_mixer(mixer_regs::RECORD_GAIN, g | (g << 8));
    }

    /// Start playback
    pub fn start_playback(&mut self) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("ac97: not initialized");
        }

        // Stop if already running
        self.stop_playback();

        // Setup buffer descriptors
        for (i, buffer) in self.playback_buffers.iter().enumerate() {
            let phys_addr = buffer.as_ptr() as u32; // Assume identity mapped for now
            let samples = 2048; // 4096 bytes / 2 bytes per sample
            self.playback_bd[i] = BufferDescriptor::new(phys_addr, samples, true, false);
        }

        // Set buffer descriptor base address
        let bd_phys = self.playback_bd.as_ptr() as u32;
        self.write_bm32(busmaster_regs::PO_BDBAR, bd_phys);

        // Set last valid index
        self.write_bm8(busmaster_regs::PO_LVI, 31);

        // Clear status
        self.write_bm16(busmaster_regs::PO_SR,
            status_bits::LVBCI | status_bits::BCIS | status_bits::FIFOE);

        // Start DMA
        self.write_bm8(busmaster_regs::PO_CR,
            control_bits::RPBM | control_bits::IOCE | control_bits::FEIE);

        self.playback_active.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Stop playback
    pub fn stop_playback(&mut self) {
        // Clear run bit
        self.write_bm8(busmaster_regs::PO_CR, 0);

        // Reset
        self.write_bm8(busmaster_regs::PO_CR, control_bits::RR);

        // Wait for reset complete
        for _ in 0..1000 {
            if self.read_bm8(busmaster_regs::PO_CR) & control_bits::RR == 0 {
                break;
            }
            core::hint::spin_loop();
        }

        self.playback_active.store(false, Ordering::SeqCst);
    }

    /// Start capture
    pub fn start_capture(&mut self) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("ac97: not initialized");
        }

        self.stop_capture();

        // Setup buffer descriptors
        for (i, buffer) in self.capture_buffers.iter().enumerate() {
            let phys_addr = buffer.as_ptr() as u32;
            let samples = 2048;
            self.capture_bd[i] = BufferDescriptor::new(phys_addr, samples, true, false);
        }

        // Set buffer descriptor base address
        let bd_phys = self.capture_bd.as_ptr() as u32;
        self.write_bm32(busmaster_regs::PI_BDBAR, bd_phys);

        // Set last valid index
        self.write_bm8(busmaster_regs::PI_LVI, 31);

        // Clear status
        self.write_bm16(busmaster_regs::PI_SR,
            status_bits::LVBCI | status_bits::BCIS | status_bits::FIFOE);

        // Start DMA
        self.write_bm8(busmaster_regs::PI_CR,
            control_bits::RPBM | control_bits::IOCE | control_bits::FEIE);

        self.capture_active.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Stop capture
    pub fn stop_capture(&mut self) {
        self.write_bm8(busmaster_regs::PI_CR, 0);
        self.write_bm8(busmaster_regs::PI_CR, control_bits::RR);

        for _ in 0..1000 {
            if self.read_bm8(busmaster_regs::PI_CR) & control_bits::RR == 0 {
                break;
            }
            core::hint::spin_loop();
        }

        self.capture_active.store(false, Ordering::SeqCst);
    }

    /// Handle interrupt
    pub fn handle_interrupt(&mut self) {
        let glob_sta = self.read_bm32(busmaster_regs::GLOB_STA);

        if glob_sta & glob_sta_bits::POINT != 0 {
            // Playback interrupt
            let status = self.read_bm16(busmaster_regs::PO_SR);
            self.write_bm16(busmaster_regs::PO_SR, status); // Clear

            if status & status_bits::BCIS != 0 {
                // Buffer completed
                let civ = self.read_bm8(busmaster_regs::PO_CIV);
                let lvi = self.read_bm8(busmaster_regs::PO_LVI);

                // Advance LVI to keep playing
                let next_lvi = (lvi + 1) % 32;
                self.write_bm8(busmaster_regs::PO_LVI, next_lvi);
            }
        }

        if glob_sta & glob_sta_bits::PIINT != 0 {
            // Capture interrupt
            let status = self.read_bm16(busmaster_regs::PI_SR);
            self.write_bm16(busmaster_regs::PI_SR, status);

            if status & status_bits::BCIS != 0 {
                let lvi = self.read_bm8(busmaster_regs::PI_LVI);
                let next_lvi = (lvi + 1) % 32;
                self.write_bm8(busmaster_regs::PI_LVI, next_lvi);
            }
        }
    }

    /// Get capabilities
    pub fn capabilities(&self) -> &Ac97Capabilities {
        &self.capabilities
    }

    /// Is playback active
    pub fn is_playback_active(&self) -> bool {
        self.playback_active.load(Ordering::SeqCst)
    }

    /// Is capture active
    pub fn is_capture_active(&self) -> bool {
        self.capture_active.load(Ordering::SeqCst)
    }

    /// Get current sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

/// AC'97 driver state
pub struct Ac97Driver {
    controllers: Vec<Ac97Controller>,
    default_controller: Option<usize>,
    initialized: bool,
}

impl Ac97Driver {
    pub const fn new() -> Self {
        Self {
            controllers: Vec::new(),
            default_controller: None,
            initialized: false,
        }
    }

    pub fn init(&mut self) {
        crate::kprintln!("ac97: scanning for AC'97 controllers...");

        // Scan PCI for AC'97 controllers
        // Class 0x04 (Multimedia), Subclass 0x01 (Audio)
        for bus in 0..=255u8 {
            for device in 0..32u8 {
                for function in 0..8u8 {
                    let vendor_id = pci::read_u16(bus, device, function, 0x00);
                    if vendor_id == 0xFFFF {
                        continue;
                    }

                    let class_code = pci::read_u16(bus, device, function, 0x0A);
                    let class = (class_code >> 8) as u8;
                    let subclass = (class_code & 0xFF) as u8;

                    // AC'97 audio controller
                    if class == 0x04 && subclass == 0x01 {
                        let device_id = pci::read_u16(bus, device, function, 0x02);

                        // Check for known AC'97 controllers
                        if is_ac97_controller(vendor_id, device_id) {
                            // Read BARs
                            let bar0 = pci::read_u32(bus, device, function, 0x10);
                            let bar1 = pci::read_u32(bus, device, function, 0x14);

                            // AC'97 uses I/O ports
                            let nambar = (bar0 & 0xFFFC) as u16;
                            let nabmbar = (bar1 & 0xFFFC) as u16;

                            if nambar != 0 && nabmbar != 0 {
                                crate::kprintln!("ac97: found {:04X}:{:04X} at {:02X}:{:02X}.{}",
                                    vendor_id, device_id, bus, device, function);

                                // Enable I/O space and bus mastering
                                let cmd = pci::read_u16(bus, device, function, 0x04);
                                pci::write_u16(bus, device, function, 0x04, cmd | 0x05);

                                let mut controller = Ac97Controller::new(
                                    bus, device, function, nambar, nabmbar
                                );

                                if controller.init().is_ok() {
                                    let idx = self.controllers.len();
                                    self.controllers.push(controller);
                                    if self.default_controller.is_none() {
                                        self.default_controller = Some(idx);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        self.initialized = true;
        crate::kprintln!("ac97: found {} controller(s)", self.controllers.len());
    }

    pub fn controller_count(&self) -> usize {
        self.controllers.len()
    }

    pub fn get_controller(&mut self, index: usize) -> Option<&mut Ac97Controller> {
        self.controllers.get_mut(index)
    }

    pub fn default_controller(&mut self) -> Option<&mut Ac97Controller> {
        self.default_controller.and_then(move |i| self.controllers.get_mut(i))
    }
}

/// Check if device is a known AC'97 controller
fn is_ac97_controller(vendor_id: u16, device_id: u16) -> bool {
    match vendor_id {
        // Intel
        0x8086 => matches!(device_id,
            0x2415 | // 82801AA
            0x2425 | // 82801AB
            0x2445 | // 82801BA
            0x2485 | // ICH3
            0x24C5 | // ICH4
            0x24D5 | // ICH5
            0x266E | // ICH6
            0x27DE | // ICH7
            0x269A   // ESB2
        ),
        // VIA
        0x1106 => matches!(device_id,
            0x3058 | // VT82C686
            0x3059   // VT8233
        ),
        // SiS
        0x1039 => matches!(device_id,
            0x7012   // SiS7012
        ),
        // nVidia
        0x10DE => matches!(device_id,
            0x01B1 | // nForce
            0x006A | // nForce2
            0x00DA | // nForce3
            0x00EA   // CK804
        ),
        // AMD
        0x1022 => matches!(device_id,
            0x764D | // AMD8111
            0x7445   // AMD768
        ),
        _ => false,
    }
}

/// Global AC'97 driver
static AC97_DRIVER: TicketSpinlock<Ac97Driver> = TicketSpinlock::new(Ac97Driver::new());

/// Initialize AC'97 driver
pub fn init() {
    AC97_DRIVER.lock().init();
}

/// Get controller count
pub fn controller_count() -> usize {
    AC97_DRIVER.lock().controller_count()
}

/// Set master volume on default controller
pub fn set_volume(percent: u8) {
    if let Some(ctrl) = AC97_DRIVER.lock().default_controller() {
        ctrl.set_volume(percent);
    }
}

/// Get master volume from default controller
pub fn get_volume() -> u8 {
    AC97_DRIVER.lock().default_controller()
        .map(|c| c.get_volume())
        .unwrap_or(0)
}

/// Set mute on default controller
pub fn set_mute(mute: bool) {
    if let Some(ctrl) = AC97_DRIVER.lock().default_controller() {
        ctrl.set_mute(mute);
    }
}

/// Is muted
pub fn is_muted() -> bool {
    AC97_DRIVER.lock().default_controller()
        .map(|c| c.is_muted())
        .unwrap_or(true)
}

/// Format driver info
pub fn format_info() -> String {
    use core::fmt::Write;
    let mut output = String::new();
    let driver = AC97_DRIVER.lock();

    writeln!(output, "AC'97 Audio Controllers: {}", driver.controllers.len()).ok();

    for (i, ctrl) in driver.controllers.iter().enumerate() {
        let default = driver.default_controller == Some(i);
        writeln!(output, "  [{}]{} {:02X}:{:02X}.{}",
            i,
            if default { " (default)" } else { "" },
            ctrl.bus, ctrl.device, ctrl.function
        ).ok();
        writeln!(output, "      Vendor: {:08X}", ctrl.capabilities.vendor_id).ok();
        writeln!(output, "      VRA: {}, Channels: {}",
            ctrl.capabilities.variable_rate,
            ctrl.capabilities.max_channels
        ).ok();
        writeln!(output, "      Sample Rate: {} Hz", ctrl.sample_rate).ok();
        writeln!(output, "      Volume: {}%{}",
            ctrl.get_volume(),
            if ctrl.is_muted() { " (muted)" } else { "" }
        ).ok();
    }

    output
}
