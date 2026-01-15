//! xHCI (eXtensible Host Controller Interface) driver.
//!
//! Implementa suporte a USB 3.x via xHCI.
//! PCI class 0x0C, subclass 0x03, prog_if 0x30.

#![allow(dead_code)]

use core::ptr::{read_volatile, write_volatile};

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::{Mutex, Once};
use x86_64::VirtAddr;

use crate::drivers::pci::{self, PciDevice};
use crate::mm;
use crate::util::KError;

use super::{UsbDevice, UsbSpeed};

/// xHCI Capability Registers
#[repr(C)]
struct CapRegs {
    caplength: u8,
    _rsvd: u8,
    hciversion: u16,
    hcsparams1: u32,
    hcsparams2: u32,
    hcsparams3: u32,
    hccparams1: u32,
    dboff: u32,
    rtsoff: u32,
    hccparams2: u32,
}

/// xHCI Operational Registers
#[repr(C)]
struct OpRegs {
    usbcmd: u32,
    usbsts: u32,
    pagesize: u32,
    _rsvd1: [u32; 2],
    dnctrl: u32,
    crcr_lo: u32,
    crcr_hi: u32,
    _rsvd2: [u32; 4],
    dcbaap_lo: u32,
    dcbaap_hi: u32,
    config: u32,
}

/// Port Status and Control Register
#[repr(C)]
struct PortRegs {
    portsc: u32,
    portpmsc: u32,
    portli: u32,
    porthlpmc: u32,
}

/// xHCI Runtime Registers (Interrupter)
#[repr(C)]
struct InterrupterRegs {
    iman: u32,
    imod: u32,
    erstsz: u32,
    _rsvd: u32,
    erstba_lo: u32,
    erstba_hi: u32,
    erdp_lo: u32,
    erdp_hi: u32,
}

/// Transfer Request Block (TRB) - 16 bytes
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct Trb {
    param_lo: u32,
    param_hi: u32,
    status: u32,
    control: u32,
}

impl Trb {
    const fn new() -> Self {
        Self {
            param_lo: 0,
            param_hi: 0,
            status: 0,
            control: 0,
        }
    }

    fn set_pointer(&mut self, addr: u64) {
        self.param_lo = addr as u32;
        self.param_hi = (addr >> 32) as u32;
    }

    fn pointer(&self) -> u64 {
        (self.param_lo as u64) | ((self.param_hi as u64) << 32)
    }
}

/// TRB types
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum TrbType {
    Normal = 1,
    SetupStage = 2,
    DataStage = 3,
    StatusStage = 4,
    Isoch = 5,
    Link = 6,
    EventData = 7,
    NoOp = 8,
    EnableSlot = 9,
    DisableSlot = 10,
    AddressDevice = 11,
    ConfigureEndpoint = 12,
    EvaluateContext = 13,
    ResetEndpoint = 14,
    StopEndpoint = 15,
    SetTrDequeuePointer = 16,
    ResetDevice = 17,
    ForceEvent = 18,
    NegotiateBandwidth = 19,
    SetLatencyToleranceValue = 20,
    GetPortBandwidth = 21,
    ForceHeader = 22,
    NoOpCmd = 23,
    TransferEvent = 32,
    CommandCompletion = 33,
    PortStatusChange = 34,
    BandwidthRequest = 35,
    Doorbell = 36,
    HostController = 37,
    DeviceNotification = 38,
    MfindexWrap = 39,
}

/// Completion codes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionCode {
    Invalid = 0,
    Success = 1,
    DataBuffer = 2,
    BabbleDetected = 3,
    UsbTransaction = 4,
    Trb = 5,
    Stall = 6,
    Resource = 7,
    Bandwidth = 8,
    NoSlots = 9,
    InvalidStreamType = 10,
    SlotNotEnabled = 11,
    EndpointNotEnabled = 12,
    ShortPacket = 13,
    RingUnderrun = 14,
    RingOverrun = 15,
    VfEventRingFull = 16,
    Parameter = 17,
    BandwidthOverrun = 18,
    ContextState = 19,
    NoPingResponse = 20,
    EventRingFull = 21,
    IncompatibleDevice = 22,
    MissedService = 23,
    CommandRingStopped = 24,
    CommandAborted = 25,
    Stopped = 26,
    StoppedLengthInvalid = 27,
    StoppedShortPacket = 28,
    MaxExitLatencyTooLarge = 29,
    IsochBuffer = 31,
    EventLost = 32,
    Undefined = 33,
    InvalidStreamId = 34,
    SecondaryBandwidth = 35,
    SplitTransaction = 36,
}

impl From<u8> for CompletionCode {
    fn from(v: u8) -> Self {
        if v <= 36 {
            unsafe { core::mem::transmute(v) }
        } else {
            CompletionCode::Undefined
        }
    }
}

/// Event Ring Segment Table Entry
#[repr(C, align(64))]
#[derive(Clone, Copy)]
struct ErstEntry {
    ring_segment_base_lo: u32,
    ring_segment_base_hi: u32,
    ring_segment_size: u16,
    _rsvd: [u8; 6],
}

/// Slot Context (32 or 64 bytes depending on CSZ)
#[repr(C, align(32))]
#[derive(Clone, Copy, Default)]
struct SlotContext {
    data: [u32; 8],
}

/// Endpoint Context
#[repr(C, align(32))]
#[derive(Clone, Copy, Default)]
struct EndpointContext {
    data: [u32; 8],
}

/// Input Control Context
#[repr(C, align(32))]
#[derive(Clone, Copy, Default)]
struct InputControlContext {
    drop_flags: u32,
    add_flags: u32,
    _rsvd: [u32; 6],
}

/// Device Context (Slot + 31 Endpoints)
#[repr(C, align(4096))]
struct DeviceContext {
    slot: SlotContext,
    endpoints: [EndpointContext; 31],
}

/// Input Context (Control + Slot + 31 Endpoints)
#[repr(C, align(4096))]
struct InputContext {
    control: InputControlContext,
    slot: SlotContext,
    endpoints: [EndpointContext; 31],
}

/// Command Ring
struct CommandRing {
    trbs: Box<[Trb; 256]>,
    enqueue: usize,
    cycle: bool,
}

impl CommandRing {
    fn new() -> Self {
        let mut trbs = Box::new([Trb::new(); 256]);
        // Link TRB at the end points back to start
        let phys = virt_to_phys(trbs.as_ptr() as u64);
        trbs[255].set_pointer(phys);
        trbs[255].control = (TrbType::Link as u32) << 10 | (1 << 5); // Toggle cycle
        Self {
            trbs,
            enqueue: 0,
            cycle: true,
        }
    }

    fn phys_addr(&self) -> u64 {
        virt_to_phys(self.trbs.as_ptr() as u64)
    }

    fn push(&mut self, mut trb: Trb) -> u64 {
        // Set cycle bit
        if self.cycle {
            trb.control |= 1;
        } else {
            trb.control &= !1;
        }

        self.trbs[self.enqueue] = trb;
        let addr = virt_to_phys(&self.trbs[self.enqueue] as *const _ as u64);

        self.enqueue += 1;
        if self.enqueue >= 255 {
            // Update link TRB cycle bit
            if self.cycle {
                self.trbs[255].control |= 1;
            } else {
                self.trbs[255].control &= !1;
            }
            self.enqueue = 0;
            self.cycle = !self.cycle;
        }

        addr
    }
}

/// Event Ring
struct EventRing {
    trbs: Box<[Trb; 256]>,
    erst: Box<[ErstEntry; 1]>,
    dequeue: usize,
    cycle: bool,
}

impl EventRing {
    fn new() -> Self {
        let trbs = Box::new([Trb::new(); 256]);
        let mut erst = Box::new([ErstEntry {
            ring_segment_base_lo: 0,
            ring_segment_base_hi: 0,
            ring_segment_size: 256,
            _rsvd: [0; 6],
        }; 1]);

        let trb_phys = virt_to_phys(trbs.as_ptr() as u64);
        erst[0].ring_segment_base_lo = trb_phys as u32;
        erst[0].ring_segment_base_hi = (trb_phys >> 32) as u32;

        Self {
            trbs,
            erst,
            dequeue: 0,
            cycle: true,
        }
    }

    fn erst_phys(&self) -> u64 {
        virt_to_phys(self.erst.as_ptr() as u64)
    }

    fn dequeue_phys(&self) -> u64 {
        virt_to_phys(&self.trbs[self.dequeue] as *const _ as u64)
    }

    fn pop(&mut self) -> Option<Trb> {
        let trb = self.trbs[self.dequeue];
        let trb_cycle = (trb.control & 1) != 0;

        if trb_cycle != self.cycle {
            return None;
        }

        self.dequeue += 1;
        if self.dequeue >= 256 {
            self.dequeue = 0;
            self.cycle = !self.cycle;
        }

        Some(trb)
    }
}

/// Transfer Ring (per endpoint)
struct TransferRing {
    trbs: Box<[Trb; 256]>,
    enqueue: usize,
    cycle: bool,
}

impl TransferRing {
    fn new() -> Self {
        let mut trbs = Box::new([Trb::new(); 256]);
        let phys = virt_to_phys(trbs.as_ptr() as u64);
        trbs[255].set_pointer(phys);
        trbs[255].control = (TrbType::Link as u32) << 10 | (1 << 5);
        Self {
            trbs,
            enqueue: 0,
            cycle: true,
        }
    }

    fn phys_addr(&self) -> u64 {
        virt_to_phys(self.trbs.as_ptr() as u64)
    }

    fn push(&mut self, mut trb: Trb) {
        if self.cycle {
            trb.control |= 1;
        } else {
            trb.control &= !1;
        }

        self.trbs[self.enqueue] = trb;
        self.enqueue += 1;

        if self.enqueue >= 255 {
            if self.cycle {
                self.trbs[255].control |= 1;
            } else {
                self.trbs[255].control &= !1;
            }
            self.enqueue = 0;
            self.cycle = !self.cycle;
        }
    }
}

/// xHCI Controller
pub struct XhciController {
    base: u64,
    cap_length: u8,
    max_slots: u8,
    max_ports: u8,
    context_size: usize, // 32 or 64 bytes
    scratchpad_count: u16,

    dcbaa: Box<[u64; 256]>,
    device_contexts: Vec<Option<Box<DeviceContext>>>,
    cmd_ring: Mutex<CommandRing>,
    event_ring: Mutex<EventRing>,
    transfer_rings: Mutex<Vec<Option<TransferRing>>>,

    devices: Mutex<Vec<UsbDevice>>,
}

fn virt_to_phys(va: u64) -> u64 {
    mm::virt_to_phys(VirtAddr::new(va))
        .map(|pa| pa.as_u64())
        .unwrap_or(0)
}

impl XhciController {
    /// Cria controller a partir de endereço base MMIO
    unsafe fn new(base: u64) -> Result<Self, KError> {
        // Lê capability registers
        let cap_length = read_volatile(base as *const u8);
        let hcsparams1 = read_volatile((base + 4) as *const u32);
        let hcsparams2 = read_volatile((base + 8) as *const u32);
        let hccparams1 = read_volatile((base + 0x10) as *const u32);

        let max_slots = (hcsparams1 & 0xFF) as u8;
        let max_ports = ((hcsparams1 >> 24) & 0xFF) as u8;
        let context_size = if (hccparams1 & (1 << 2)) != 0 { 64 } else { 32 };
        let max_scratchpad_hi = ((hcsparams2 >> 21) & 0x1F) as u16;
        let max_scratchpad_lo = ((hcsparams2 >> 27) & 0x1F) as u16;
        let scratchpad_count = (max_scratchpad_hi << 5) | max_scratchpad_lo;

        crate::kprintln!(
            "xhci: max_slots={}, max_ports={}, ctx_size={}, scratchpad={}",
            max_slots,
            max_ports,
            context_size,
            scratchpad_count
        );

        // DCBAA (Device Context Base Address Array)
        let dcbaa = Box::new([0u64; 256]);

        let mut ctrl = Self {
            base,
            cap_length,
            max_slots,
            max_ports,
            context_size,
            scratchpad_count,
            dcbaa,
            device_contexts: Vec::new(),
            cmd_ring: Mutex::new(CommandRing::new()),
            event_ring: Mutex::new(EventRing::new()),
            transfer_rings: Mutex::new(Vec::new()),
            devices: Mutex::new(Vec::new()),
        };

        // Inicializa device contexts
        for _ in 0..=max_slots {
            ctrl.device_contexts.push(None);
        }

        // Inicializa transfer rings (2 per slot para control endpoint)
        let mut tr = ctrl.transfer_rings.lock();
        for _ in 0..(max_slots as usize * 32) {
            tr.push(None);
        }
        drop(tr);

        ctrl.reset()?;
        ctrl.init_controller()?;

        Ok(ctrl)
    }

    fn op_regs(&self) -> *mut OpRegs {
        (self.base + self.cap_length as u64) as *mut OpRegs
    }

    fn port_regs(&self, port: u8) -> *mut PortRegs {
        let op_base = self.base + self.cap_length as u64;
        (op_base + 0x400 + (port as u64 - 1) * 0x10) as *mut PortRegs
    }

    fn doorbell(&self, slot: u8) -> *mut u32 {
        let dboff = unsafe { read_volatile((self.base + 0x14) as *const u32) };
        (self.base + dboff as u64 + slot as u64 * 4) as *mut u32
    }

    fn runtime_regs(&self) -> u64 {
        let rtsoff = unsafe { read_volatile((self.base + 0x18) as *const u32) };
        self.base + rtsoff as u64
    }

    fn interrupter(&self, n: u32) -> *mut InterrupterRegs {
        (self.runtime_regs() + 0x20 + n as u64 * 0x20) as *mut InterrupterRegs
    }

    unsafe fn reset(&mut self) -> Result<(), KError> {
        let op = self.op_regs();

        // Stop controller
        let cmd = read_volatile(&(*op).usbcmd);
        write_volatile(&mut (*op).usbcmd, cmd & !1);

        // Wait for halt
        for _ in 0..1000 {
            if read_volatile(&(*op).usbsts) & 1 != 0 {
                break;
            }
            for _ in 0..10000 {
                core::hint::spin_loop();
            }
        }

        // Reset
        let cmd = read_volatile(&(*op).usbcmd);
        write_volatile(&mut (*op).usbcmd, cmd | (1 << 1));

        // Wait for reset complete
        for _ in 0..1000 {
            if read_volatile(&(*op).usbcmd) & (1 << 1) == 0 {
                break;
            }
            for _ in 0..10000 {
                core::hint::spin_loop();
            }
        }

        if read_volatile(&(*op).usbcmd) & (1 << 1) != 0 {
            return Err(KError::Timeout);
        }

        Ok(())
    }

    unsafe fn init_controller(&mut self) -> Result<(), KError> {
        let op = self.op_regs();

        // Set max slots
        write_volatile(&mut (*op).config, self.max_slots as u32);

        // Setup scratchpad buffers if needed
        if self.scratchpad_count > 0 {
            let scratchpad_array = Box::new([0u64; 256]);
            let array_phys = virt_to_phys(scratchpad_array.as_ptr() as u64);

            // Allocate scratchpad pages
            for i in 0..self.scratchpad_count as usize {
                if let Some(frame) = mm::alloc_frame() {
                    let page_phys: u64 = frame.start_address().as_u64();
                    // Write to array (keeping box alive)
                    let ptr = scratchpad_array.as_ptr() as *mut u64;
                    write_volatile(ptr.add(i), page_phys);
                }
            }

            self.dcbaa[0] = array_phys;
            // Leak the box to keep it alive
            Box::leak(scratchpad_array);
        }

        // Set DCBAA pointer
        let dcbaa_phys = virt_to_phys(self.dcbaa.as_ptr() as u64);
        write_volatile(&mut (*op).dcbaap_lo, dcbaa_phys as u32);
        write_volatile(&mut (*op).dcbaap_hi, (dcbaa_phys >> 32) as u32);

        // Setup command ring
        let cmd_ring = self.cmd_ring.lock();
        let crcr = cmd_ring.phys_addr() | 1; // RCS = 1
        write_volatile(&mut (*op).crcr_lo, crcr as u32);
        write_volatile(&mut (*op).crcr_hi, (crcr >> 32) as u32);
        drop(cmd_ring);

        // Setup event ring
        let event_ring = self.event_ring.lock();
        let ir = self.interrupter(0);

        // ERSTSZ
        write_volatile(&mut (*ir).erstsz, 1);

        // ERDP
        let erdp = event_ring.dequeue_phys() | (1 << 3); // EHB
        write_volatile(&mut (*ir).erdp_lo, erdp as u32);
        write_volatile(&mut (*ir).erdp_hi, (erdp >> 32) as u32);

        // ERSTBA
        let erstba = event_ring.erst_phys();
        write_volatile(&mut (*ir).erstba_lo, erstba as u32);
        write_volatile(&mut (*ir).erstba_hi, (erstba >> 32) as u32);

        // Enable interrupter
        let iman = read_volatile(&(*ir).iman);
        write_volatile(&mut (*ir).iman, iman | 0x3); // IE + IP

        drop(event_ring);

        // Start controller
        let cmd = read_volatile(&(*op).usbcmd);
        write_volatile(&mut (*op).usbcmd, cmd | 1 | (1 << 2)); // RS + INTE

        // Wait for running
        for _ in 0..100 {
            if read_volatile(&(*op).usbsts) & 1 == 0 {
                break;
            }
            for _ in 0..10000 {
                core::hint::spin_loop();
            }
        }

        if read_volatile(&(*op).usbsts) & 1 != 0 {
            return Err(KError::Timeout);
        }

        crate::kprintln!("xhci: controller iniciado");
        Ok(())
    }

    /// Processa eventos pendentes
    pub fn poll_events(&self) {
        let mut event_ring = self.event_ring.lock();

        while let Some(trb) = event_ring.pop() {
            let trb_type = ((trb.control >> 10) & 0x3F) as u8;
            let completion = CompletionCode::from(((trb.status >> 24) & 0xFF) as u8);

            match trb_type {
                x if x == TrbType::PortStatusChange as u8 => {
                    let port_id = ((trb.param_lo >> 24) & 0xFF) as u8;
                    crate::kprintln!("xhci: port {} status change", port_id);
                    // Acknowledge port change
                    unsafe {
                        let pr = self.port_regs(port_id);
                        let portsc = read_volatile(&(*pr).portsc);
                        // Clear change bits by writing 1 to them (preserve RW1C bits)
                        write_volatile(&mut (*pr).portsc, portsc | (1 << 17) | (1 << 18) | (1 << 19) | (1 << 20) | (1 << 21) | (1 << 22));
                    }
                }
                x if x == TrbType::CommandCompletion as u8 => {
                    crate::kprintln!("xhci: command completion: {:?}", completion);
                }
                x if x == TrbType::TransferEvent as u8 => {
                    if completion != CompletionCode::Success && completion != CompletionCode::ShortPacket {
                        crate::kprintln!("xhci: transfer error: {:?}", completion);
                    }
                }
                _ => {
                    crate::kprintln!("xhci: unknown event type {}", trb_type);
                }
            }
        }

        // Update ERDP
        unsafe {
            let ir = self.interrupter(0);
            let erdp = event_ring.dequeue_phys() | (1 << 3);
            write_volatile(&mut (*ir).erdp_lo, erdp as u32);
            write_volatile(&mut (*ir).erdp_hi, (erdp >> 32) as u32);
        }
    }

    /// Enumera portas e dispositivos conectados
    pub fn enumerate_ports(&self) {
        for port in 1..=self.max_ports {
            unsafe {
                let pr = self.port_regs(port);
                let portsc = read_volatile(&(*pr).portsc);

                // Check CCS (Current Connect Status)
                if portsc & 1 != 0 {
                    let speed = ((portsc >> 10) & 0xF) as u8;
                    let usb_speed = UsbSpeed::from_xhci_speed(speed);
                    crate::kprintln!(
                        "xhci: port {} connected, speed={:?}, portsc={:#x}",
                        port,
                        usb_speed,
                        portsc
                    );

                    // Check PED (Port Enabled/Disabled)
                    if portsc & (1 << 1) != 0 {
                        crate::kprintln!("xhci: port {} enabled", port);
                    } else {
                        // Reset port to enable it
                        crate::kprintln!("xhci: resetting port {}...", port);
                        write_volatile(&mut (*pr).portsc, portsc | (1 << 4)); // PR = 1

                        // Wait for reset complete
                        for _ in 0..1000 {
                            let ps = read_volatile(&(*pr).portsc);
                            if ps & (1 << 4) == 0 {
                                break;
                            }
                            for _ in 0..10000 {
                                core::hint::spin_loop();
                            }
                        }

                        let ps = read_volatile(&(*pr).portsc);
                        if ps & (1 << 1) != 0 {
                            crate::kprintln!("xhci: port {} enabled after reset", port);
                        }
                    }
                }
            }
        }
    }

    /// Ring doorbell
    fn ring_doorbell(&self, slot: u8, target: u8) {
        unsafe {
            let db = self.doorbell(slot);
            write_volatile(db, target as u32);
        }
    }

    /// Envia comando e aguarda completion
    fn send_command(&self, trb: Trb) -> Result<Trb, KError> {
        let mut cmd_ring = self.cmd_ring.lock();
        cmd_ring.push(trb);
        drop(cmd_ring);

        // Ring host controller doorbell (slot 0, target 0)
        self.ring_doorbell(0, 0);

        // Poll for completion
        for _ in 0..10000 {
            let mut event_ring = self.event_ring.lock();
            if let Some(event) = event_ring.pop() {
                let trb_type = ((event.control >> 10) & 0x3F) as u8;
                if trb_type == TrbType::CommandCompletion as u8 {
                    // Update ERDP
                    unsafe {
                        let ir = self.interrupter(0);
                        let erdp = event_ring.dequeue_phys() | (1 << 3);
                        write_volatile(&mut (*ir).erdp_lo, erdp as u32);
                        write_volatile(&mut (*ir).erdp_hi, (erdp >> 32) as u32);
                    }
                    return Ok(event);
                }
            }
            drop(event_ring);

            for _ in 0..1000 {
                core::hint::spin_loop();
            }
        }

        Err(KError::Timeout)
    }

    /// Enable Slot command
    pub fn enable_slot(&self) -> Result<u8, KError> {
        let mut trb = Trb::new();
        trb.control = (TrbType::EnableSlot as u32) << 10;

        let event = self.send_command(trb)?;
        let completion = CompletionCode::from(((event.status >> 24) & 0xFF) as u8);

        if completion != CompletionCode::Success {
            crate::kprintln!("xhci: enable_slot failed: {:?}", completion);
            return Err(KError::IO);
        }

        let slot_id = ((event.control >> 24) & 0xFF) as u8;
        crate::kprintln!("xhci: enabled slot {}", slot_id);
        Ok(slot_id)
    }

    /// Address Device command (BSR=1 para não enviar SET_ADDRESS ainda)
    pub fn address_device(&mut self, slot_id: u8, port: u8, speed: UsbSpeed) -> Result<(), KError> {
        // Allocate device context
        let dev_ctx = Box::new(DeviceContext {
            slot: SlotContext::default(),
            endpoints: [EndpointContext::default(); 31],
        });
        let dev_ctx_phys = virt_to_phys(dev_ctx.as_ref() as *const _ as u64);

        // Create input context
        let mut input_ctx = Box::new(InputContext {
            control: InputControlContext::default(),
            slot: SlotContext::default(),
            endpoints: [EndpointContext::default(); 31],
        });

        // Add flags: Slot + EP0
        input_ctx.control.add_flags = 0x3; // Slot context (A0) + EP0 (A1)

        // Setup slot context
        // Route string = 0 for root hub port
        // Speed, context entries = 1 (only control endpoint)
        let route_string = 0u32;
        let speed_val = match speed {
            UsbSpeed::Full => 1,
            UsbSpeed::Low => 2,
            UsbSpeed::High => 3,
            UsbSpeed::Super => 4,
            UsbSpeed::SuperPlus => 5,
        };

        input_ctx.slot.data[0] = route_string | ((speed_val as u32) << 20) | (1 << 27); // context entries
        input_ctx.slot.data[1] = (port as u32) << 16; // Root hub port number

        // Setup EP0 context
        // Max packet size based on speed
        let max_packet = match speed {
            UsbSpeed::Low => 8,
            UsbSpeed::Full => 8, // Will be updated after GET_DESCRIPTOR
            UsbSpeed::High => 64,
            UsbSpeed::Super | UsbSpeed::SuperPlus => 512,
        };

        // Allocate transfer ring for EP0
        let ep0_ring = TransferRing::new();
        let ep0_ring_phys = ep0_ring.phys_addr();

        let mut tr = self.transfer_rings.lock();
        let tr_index = (slot_id as usize - 1) * 32; // EP0 = index 0
        if tr_index < tr.len() {
            tr[tr_index] = Some(ep0_ring);
        }
        drop(tr);

        // EP0 context
        // EP type = Control Bidirectional (4), CErr = 3
        input_ctx.endpoints[0].data[1] = (3 << 1) | (4 << 3) | ((max_packet as u32) << 16);
        input_ctx.endpoints[0].data[2] = (ep0_ring_phys as u32) | 1; // DCS = 1
        input_ctx.endpoints[0].data[3] = (ep0_ring_phys >> 32) as u32;
        input_ctx.endpoints[0].data[4] = 8; // Average TRB length

        let input_ctx_phys = virt_to_phys(input_ctx.as_ref() as *const _ as u64);

        // Update DCBAA
        self.dcbaa[slot_id as usize] = dev_ctx_phys;

        // Send Address Device command with BSR=1
        let mut trb = Trb::new();
        trb.set_pointer(input_ctx_phys);
        trb.control = (TrbType::AddressDevice as u32) << 10 | ((slot_id as u32) << 24) | (1 << 9); // BSR=1

        let event = self.send_command(trb)?;
        let completion = CompletionCode::from(((event.status >> 24) & 0xFF) as u8);

        if completion != CompletionCode::Success {
            crate::kprintln!("xhci: address_device failed: {:?}", completion);
            return Err(KError::IO);
        }

        // Store contexts
        self.device_contexts[slot_id as usize] = Some(dev_ctx);
        Box::leak(input_ctx); // Keep input context alive

        crate::kprintln!("xhci: device addressed (slot {})", slot_id);
        Ok(())
    }
}

// Global controller reference
static XHCI_CONTROLLER: Once<Arc<Mutex<XhciController>>> = Once::new();

/// Probe para dispositivo xHCI
pub fn probe(dev: &PciDevice) -> Option<&PciDevice> {
    // xHCI: class 0x0C, subclass 0x03, prog_if 0x30
    if dev.class.class_code == 0x0C
        && dev.class.subclass == 0x03
        && dev.class.prog_if == 0x30
    {
        Some(dev)
    } else {
        None
    }
}

/// Inicializa controller xHCI a partir de PciDevice
pub fn init_from_pci(dev: &PciDevice) -> Result<(), KError> {
    crate::kprintln!(
        "xhci: inicializando @ {:02x}:{:02x}.{}",
        dev.addr.bus,
        dev.addr.device,
        dev.addr.function
    );

    // Enable bus mastering and memory
    pci::enable_bus_mastering(dev);

    // Read BAR0 (MMIO)
    let (bar0, is_io) = pci::read_bar(dev, 0);
    if is_io {
        crate::kprintln!("xhci: BAR0 is I/O space (unexpected)");
        return Err(KError::NotSupported);
    }

    // Check for 64-bit BAR
    let bar0_raw = pci::read_u32(dev.addr.bus, dev.addr.device, dev.addr.function, 0x10);
    let is_64bit = (bar0_raw & 0x6) == 0x4;

    let mmio_base = if is_64bit {
        let bar1 = pci::read_u32(dev.addr.bus, dev.addr.device, dev.addr.function, 0x14) as u64;
        (bar0 & 0xFFFF_FFF0) | (bar1 << 32)
    } else {
        bar0 & 0xFFFF_FFF0
    };

    crate::kprintln!("xhci: MMIO base = {:#x}", mmio_base);

    // Map MMIO region
    let mmio_size = 0x10000; // 64KB should be enough
    mm::map_mmio(mmio_base, mmio_size)?;

    // Create controller
    let controller = unsafe { XhciController::new(mmio_base)? };
    let controller = Arc::new(Mutex::new(controller));

    XHCI_CONTROLLER.call_once(|| controller.clone());

    // Enumerate ports
    {
        let ctrl = controller.lock();
        ctrl.enumerate_ports();
        ctrl.poll_events();
    }

    Ok(())
}

/// Inicializa subsistema xHCI
pub fn init() {
    let pci_devs = crate::drivers::pci::scan();

    for dev in &pci_devs {
        if let Some(xhci) = probe(dev) {
            match init_from_pci(xhci) {
                Ok(()) => {
                    crate::kprintln!("xhci: controller inicializado com sucesso");
                    return;
                }
                Err(e) => {
                    crate::kprintln!("xhci: falha ao inicializar: {:?}", e);
                }
            }
        }
    }

    crate::kprintln!("xhci: nenhum controller encontrado");
}

/// Retorna referência ao controller xHCI (se disponível)
pub fn controller() -> Option<&'static Arc<Mutex<XhciController>>> {
    XHCI_CONTROLLER.get()
}
