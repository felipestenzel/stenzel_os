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

/// 64-byte aligned TRB ring (required by xHCI spec)
#[repr(C, align(64))]
struct TrbRing256 {
    trbs: [Trb; 256],
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
    ring: Box<TrbRing256>,
    enqueue: usize,
    cycle: bool,
}

impl CommandRing {
    fn new() -> Self {
        let mut ring = Box::new(TrbRing256 {
            trbs: [Trb::new(); 256],
        });
        // Link TRB at the end points back to start
        let phys = virt_to_phys(ring.trbs.as_ptr() as u64);
        ring.trbs[255].set_pointer(phys);
        ring.trbs[255].control = (TrbType::Link as u32) << 10 | (1 << 5); // Toggle cycle
        Self {
            ring,
            enqueue: 0,
            cycle: true,
        }
    }

    fn phys_addr(&self) -> u64 {
        virt_to_phys(self.ring.trbs.as_ptr() as u64)
    }

    fn push(&mut self, mut trb: Trb) -> u64 {
        // Set cycle bit
        if self.cycle {
            trb.control |= 1;
        } else {
            trb.control &= !1;
        }

        self.ring.trbs[self.enqueue] = trb;
        let addr = virt_to_phys(&self.ring.trbs[self.enqueue] as *const _ as u64);

        self.enqueue += 1;
        if self.enqueue >= 255 {
            // Update link TRB cycle bit
            if self.cycle {
                self.ring.trbs[255].control |= 1;
            } else {
                self.ring.trbs[255].control &= !1;
            }
            self.enqueue = 0;
            self.cycle = !self.cycle;
        }

        addr
    }
}

/// 64-byte aligned ERST (Event Ring Segment Table)
#[repr(C, align(64))]
struct AlignedErst {
    entries: [ErstEntry; 1],
}

/// Event Ring
struct EventRing {
    ring: Box<TrbRing256>,
    erst: Box<AlignedErst>,
    dequeue: usize,
    cycle: bool,
}

impl EventRing {
    fn new() -> Self {
        let ring = Box::new(TrbRing256 {
            trbs: [Trb::new(); 256],
        });
        let mut erst = Box::new(AlignedErst {
            entries: [ErstEntry {
                ring_segment_base_lo: 0,
                ring_segment_base_hi: 0,
                ring_segment_size: 256,
                _rsvd: [0; 6],
            }; 1],
        });

        let trb_phys = virt_to_phys(ring.trbs.as_ptr() as u64);
        erst.entries[0].ring_segment_base_lo = trb_phys as u32;
        erst.entries[0].ring_segment_base_hi = (trb_phys >> 32) as u32;

        Self {
            ring,
            erst,
            dequeue: 0,
            cycle: true,
        }
    }

    fn erst_phys(&self) -> u64 {
        virt_to_phys(self.erst.entries.as_ptr() as u64)
    }

    fn dequeue_phys(&self) -> u64 {
        virt_to_phys(&self.ring.trbs[self.dequeue] as *const _ as u64)
    }

    fn pop(&mut self) -> Option<Trb> {
        let trb = self.ring.trbs[self.dequeue];
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

/// Transfer Ring (per endpoint) - uses 64-byte aligned TrbRing256
struct TransferRing {
    ring: Box<TrbRing256>,
    enqueue: usize,
    cycle: bool,
}

impl TransferRing {
    fn new() -> Self {
        let mut ring = Box::new(TrbRing256 {
            trbs: [Trb::new(); 256],
        });
        let phys = virt_to_phys(ring.trbs.as_ptr() as u64);
        ring.trbs[255].set_pointer(phys);
        ring.trbs[255].control = (TrbType::Link as u32) << 10 | (1 << 5);
        Self {
            ring,
            enqueue: 0,
            cycle: true,
        }
    }

    fn phys_addr(&self) -> u64 {
        virt_to_phys(self.ring.trbs.as_ptr() as u64)
    }

    fn push(&mut self, mut trb: Trb) {
        if self.cycle {
            trb.control |= 1;
        } else {
            trb.control &= !1;
        }

        self.ring.trbs[self.enqueue] = trb;
        self.enqueue += 1;

        if self.enqueue >= 255 {
            if self.cycle {
                self.ring.trbs[255].control |= 1;
            } else {
                self.ring.trbs[255].control &= !1;
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
        let cmd_ring_phys = cmd_ring.phys_addr();
        crate::kprintln!("xhci: command ring phys addr = {:#x}", cmd_ring_phys);
        let crcr = cmd_ring_phys | 1; // RCS = 1
        write_volatile(&mut (*op).crcr_lo, crcr as u32);
        write_volatile(&mut (*op).crcr_hi, (crcr >> 32) as u32);
        drop(cmd_ring);

        // Setup event ring
        let event_ring = self.event_ring.lock();
        let ir = self.interrupter(0);

        // ERSTSZ
        write_volatile(&mut (*ir).erstsz, 1);

        // ERDP
        let erdp_phys = event_ring.dequeue_phys();
        crate::kprintln!("xhci: event ring dequeue phys = {:#x}", erdp_phys);
        let erdp = erdp_phys | (1 << 3); // EHB
        write_volatile(&mut (*ir).erdp_lo, erdp as u32);
        write_volatile(&mut (*ir).erdp_hi, (erdp >> 32) as u32);

        // ERSTBA
        let erstba = event_ring.erst_phys();
        crate::kprintln!("xhci: ERST phys = {:#x}", erstba);
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

        // Debug: print register values
        let usbcmd = read_volatile(&(*op).usbcmd);
        let usbsts = read_volatile(&(*op).usbsts);
        let crcr_lo = read_volatile(&(*op).crcr_lo);
        let crcr_hi = read_volatile(&(*op).crcr_hi);
        crate::kprintln!("xhci: USBCMD={:#x}, USBSTS={:#x}, CRCR={:#x}{:08x}",
            usbcmd, usbsts, crcr_hi, crcr_lo);

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
        let _cmd_addr = cmd_ring.push(trb);
        drop(cmd_ring);

        // Ring host controller doorbell (slot 0, target 0)
        self.ring_doorbell(0, 0);

        // Poll for completion
        let mut events_received = 0u32;
        for iteration in 0..10000 {
            let mut event_ring = self.event_ring.lock();

            // On first iteration, debug the event ring state
            if iteration == 0 {
                let first_trb = event_ring.ring.trbs[event_ring.dequeue];
                crate::kprintln!("xhci: event_ring[{}] control={:#x}, cycle_expected={}",
                    event_ring.dequeue, first_trb.control, event_ring.cycle as u8);
            }

            if let Some(event) = event_ring.pop() {
                events_received += 1;
                let trb_type = ((event.control >> 10) & 0x3F) as u8;

                // Update ERDP for ALL events to advance the event ring
                unsafe {
                    let ir = self.interrupter(0);
                    let erdp = event_ring.dequeue_phys() | (1 << 3);
                    write_volatile(&mut (*ir).erdp_lo, erdp as u32);
                    write_volatile(&mut (*ir).erdp_hi, (erdp >> 32) as u32);
                }

                if trb_type == TrbType::CommandCompletion as u8 {
                    return Ok(event);
                } else if trb_type == TrbType::PortStatusChange as u8 {
                    // Handle port status change - acknowledge it
                    let port_id = ((event.param_lo >> 24) & 0xFF) as u8;
                    crate::kprintln!("xhci: send_command: got port {} status change", port_id);
                    unsafe {
                        let pr = self.port_regs(port_id);
                        let portsc = read_volatile(&(*pr).portsc);
                        // Clear change bits by writing 1 to them
                        write_volatile(&mut (*pr).portsc, portsc | (0x3F << 17));
                    }
                    // Continue waiting for command completion
                } else {
                    crate::kprintln!("xhci: send_command: got event type {}", trb_type);
                }
                // For other events, continue waiting
            }
            drop(event_ring);

            // Every 2500 iterations, print status
            if iteration > 0 && iteration % 2500 == 0 {
                crate::kprintln!("xhci: send_command: iteration {}, events_received={}", iteration, events_received);
            }

            for _ in 0..1000 {
                core::hint::spin_loop();
            }
        }

        crate::kprintln!("xhci: send_command: timeout after 10000 iterations, {} events received", events_received);
        Err(KError::Timeout)
    }

    /// Enable Slot command
    pub fn enable_slot(&self) -> Result<u8, KError> {
        crate::kprintln!("xhci: enable_slot: sending command...");
        let mut trb = Trb::new();
        trb.control = (TrbType::EnableSlot as u32) << 10;

        let event = self.send_command(trb)?;
        crate::kprintln!("xhci: enable_slot: got completion event");
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

        // Send Address Device command with BSR=0 to actually address the device
        // (BSR=1 would only allocate slot without addressing, requiring a second command)
        let mut trb = Trb::new();
        trb.set_pointer(input_ctx_phys);
        trb.control = (TrbType::AddressDevice as u32) << 10 | ((slot_id as u32) << 24); // BSR=0

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

    crate::kprintln!("xhci: MMIO base (phys) = {:#x}", mmio_base);

    // Map MMIO region
    let mmio_size = 0x10000; // 64KB should be enough
    mm::map_mmio(mmio_base, mmio_size)?;

    // Convert physical address to virtual address
    let mmio_virt = mm::mmio_virt_addr(mmio_base).as_u64();
    crate::kprintln!("xhci: MMIO base (virt) = {:#x}", mmio_virt);

    // Create controller
    let controller = unsafe { XhciController::new(mmio_virt)? };
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

// ============================================================================
// USB Control Transfer and Enumeration
// ============================================================================

impl XhciController {
    /// Perform a control transfer (Setup -> Data -> Status)
    pub fn control_transfer(
        &mut self,
        slot_id: u8,
        setup: &super::SetupPacket,
        data: Option<&mut [u8]>,
        direction_in: bool,
    ) -> Result<usize, KError> {
        let tr_index = (slot_id as usize - 1) * 32; // EP0
        let mut tr = self.transfer_rings.lock();

        let transfer_ring = tr.get_mut(tr_index)
            .ok_or(KError::Invalid)?
            .as_mut()
            .ok_or(KError::Invalid)?;

        // Setup Stage TRB
        let mut setup_trb = Trb::new();
        setup_trb.param_lo = (setup.request_type as u32)
            | ((setup.request as u32) << 8)
            | ((setup.value as u32) << 16);
        setup_trb.param_hi = (setup.index as u32)
            | ((setup.length as u32) << 16);
        setup_trb.status = 8; // Transfer length = 8 (setup packet size)

        // TRT (Transfer Type): 0 = No Data, 2 = OUT, 3 = IN
        let trt = if data.is_none() {
            0
        } else if direction_in {
            3
        } else {
            2
        };
        setup_trb.control = (TrbType::SetupStage as u32) << 10
            | (1 << 6)  // IDT (Immediate Data)
            | (trt << 16);

        transfer_ring.push(setup_trb);

        // Data Stage TRB (if there's data)
        let data_len = if let Some(buf) = &data {
            // Allocate DMA buffer for data transfer
            let buf_phys = virt_to_phys(buf.as_ptr() as u64);

            let mut data_trb = Trb::new();
            data_trb.set_pointer(buf_phys);
            data_trb.status = buf.len() as u32;
            data_trb.control = (TrbType::DataStage as u32) << 10
                | (if direction_in { 1 << 16 } else { 0 }); // DIR: 1 = IN, 0 = OUT

            transfer_ring.push(data_trb);
            buf.len()
        } else {
            0
        };

        // Status Stage TRB
        let mut status_trb = Trb::new();
        status_trb.control = (TrbType::StatusStage as u32) << 10
            | (1 << 5)  // IOC (Interrupt on Completion)
            | (if direction_in || data.is_none() { 0 } else { 1 << 16 }); // DIR opposite of data

        transfer_ring.push(status_trb);
        drop(tr);

        // Ring doorbell for EP0 (target = 1 for EP0)
        self.ring_doorbell(slot_id, 1);

        // Wait for completion
        for _ in 0..10000 {
            let mut event_ring = self.event_ring.lock();
            if let Some(event) = event_ring.pop() {
                let trb_type = ((event.control >> 10) & 0x3F) as u8;
                if trb_type == TrbType::TransferEvent as u8 {
                    let completion = CompletionCode::from(((event.status >> 24) & 0xFF) as u8);
                    let residual = event.status & 0xFFFFFF;

                    // Update ERDP
                    unsafe {
                        let ir = self.interrupter(0);
                        let erdp = event_ring.dequeue_phys() | (1 << 3);
                        write_volatile(&mut (*ir).erdp_lo, erdp as u32);
                        write_volatile(&mut (*ir).erdp_hi, (erdp >> 32) as u32);
                    }

                    if completion == CompletionCode::Success || completion == CompletionCode::ShortPacket {
                        return Ok(data_len - residual as usize);
                    } else {
                        crate::kprintln!("xhci: control transfer failed: {:?}", completion);
                        return Err(KError::IO);
                    }
                }
            }
            drop(event_ring);

            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }

        Err(KError::Timeout)
    }

    /// Get device descriptor
    pub fn get_device_descriptor(&mut self, slot_id: u8) -> Result<super::DeviceDescriptor, KError> {
        let mut buf = [0u8; 18];
        let setup = super::SetupPacket::get_descriptor(
            super::DescriptorType::Device,
            0,
            18,
        );

        self.control_transfer(slot_id, &setup, Some(&mut buf), true)?;

        // Parse descriptor
        Ok(unsafe { core::ptr::read_unaligned(buf.as_ptr() as *const super::DeviceDescriptor) })
    }

    /// Get configuration descriptor (including all interface and endpoint descriptors)
    pub fn get_config_descriptor(&mut self, slot_id: u8, config_index: u8) -> Result<Vec<u8>, KError> {
        // First, get just the config descriptor header to know total length
        let mut header = [0u8; 9];
        let setup = super::SetupPacket::get_descriptor(
            super::DescriptorType::Configuration,
            config_index,
            9,
        );

        self.control_transfer(slot_id, &setup, Some(&mut header), true)?;

        // Get total length from header
        let total_length = u16::from_le_bytes([header[2], header[3]]) as usize;

        // Now get the full configuration
        let mut buf = alloc::vec![0u8; total_length];
        let setup = super::SetupPacket::get_descriptor(
            super::DescriptorType::Configuration,
            config_index,
            total_length as u16,
        );

        self.control_transfer(slot_id, &setup, Some(&mut buf), true)?;

        Ok(buf)
    }

    /// Set device configuration
    pub fn set_configuration(&mut self, slot_id: u8, config_value: u8) -> Result<(), KError> {
        let setup = super::SetupPacket::set_configuration(config_value);
        self.control_transfer(slot_id, &setup, None, false)?;
        Ok(())
    }

    /// Set device address (complete the address_device command)
    pub fn complete_address(&mut self, slot_id: u8) -> Result<(), KError> {
        // We already did address_device with BSR=1 in address_device()
        // Now we need to do it with BSR=0 to actually set the address
        // But xHCI handles this automatically, so we just need to verify the slot is addressed
        Ok(())
    }

    /// Configure endpoint for interrupt transfers
    pub fn configure_interrupt_endpoint(
        &mut self,
        slot_id: u8,
        endpoint_num: u8,
        direction_in: bool,
        max_packet_size: u16,
        interval: u8,
    ) -> Result<(), KError> {
        // Calculate endpoint context index
        // EP0 OUT = 1, EP0 IN = 1 (bidirectional)
        // EP1 OUT = 2, EP1 IN = 3
        // EPn OUT = 2n, EPn IN = 2n+1
        let ep_index = if direction_in {
            endpoint_num as usize * 2 + 1
        } else {
            endpoint_num as usize * 2
        };

        // Allocate transfer ring for this endpoint
        let ep_ring = TransferRing::new();
        let ep_ring_phys = ep_ring.phys_addr();

        let mut tr = self.transfer_rings.lock();
        let tr_index = (slot_id as usize - 1) * 32 + ep_index;
        if tr_index < tr.len() {
            tr[tr_index] = Some(ep_ring);
        }
        drop(tr);

        // Create input context for Configure Endpoint
        let mut input_ctx = Box::new(InputContext {
            control: InputControlContext::default(),
            slot: SlotContext::default(),
            endpoints: [EndpointContext::default(); 31],
        });

        // Set A0 (slot) and An for the endpoint
        input_ctx.control.add_flags = 1 | (1 << ep_index);

        // Update slot context: context entries = max endpoint index + 1
        if let Some(dev_ctx) = &self.device_contexts[slot_id as usize] {
            input_ctx.slot = dev_ctx.slot;
            // Update context entries if needed
            let current_entries = (input_ctx.slot.data[0] >> 27) & 0x1F;
            if ep_index as u32 > current_entries {
                input_ctx.slot.data[0] = (input_ctx.slot.data[0] & !(0x1F << 27)) | ((ep_index as u32) << 27);
            }
        }

        // Setup endpoint context for interrupt endpoint
        // EP Type: 7 = Interrupt IN, 3 = Interrupt OUT
        let ep_type = if direction_in { 7 } else { 3 };

        // Calculate interval (xHCI uses 2^(interval-1) * 125us)
        // For USB 2.0 devices, interval is in frames (1ms), so we convert
        let xhci_interval = if interval == 0 { 1 } else { interval.min(16) };

        input_ctx.endpoints[ep_index - 1].data[0] = ((xhci_interval as u32) << 16);
        input_ctx.endpoints[ep_index - 1].data[1] = (3 << 1) // CErr = 3
            | (ep_type << 3)
            | ((max_packet_size as u32) << 16);
        input_ctx.endpoints[ep_index - 1].data[2] = (ep_ring_phys as u32) | 1; // DCS = 1
        input_ctx.endpoints[ep_index - 1].data[3] = (ep_ring_phys >> 32) as u32;
        input_ctx.endpoints[ep_index - 1].data[4] = 8; // Average TRB length

        let input_ctx_phys = virt_to_phys(input_ctx.as_ref() as *const _ as u64);

        // Send Configure Endpoint command
        let mut trb = Trb::new();
        trb.set_pointer(input_ctx_phys);
        trb.control = (TrbType::ConfigureEndpoint as u32) << 10 | ((slot_id as u32) << 24);

        let event = self.send_command(trb)?;
        let completion = CompletionCode::from(((event.status >> 24) & 0xFF) as u8);

        if completion != CompletionCode::Success {
            crate::kprintln!("xhci: configure_endpoint failed: {:?}", completion);
            return Err(KError::IO);
        }

        Box::leak(input_ctx); // Keep alive
        crate::kprintln!("xhci: configured endpoint {} (index {}) for slot {}", endpoint_num, ep_index, slot_id);
        Ok(())
    }

    /// Queue an interrupt IN transfer
    pub fn queue_interrupt_in(
        &mut self,
        slot_id: u8,
        endpoint_num: u8,
        buffer: &mut [u8],
    ) -> Result<(), KError> {
        let ep_index = endpoint_num as usize * 2 + 1; // IN endpoint
        let tr_index = (slot_id as usize - 1) * 32 + ep_index;

        let mut tr = self.transfer_rings.lock();
        let transfer_ring = tr.get_mut(tr_index)
            .ok_or(KError::Invalid)?
            .as_mut()
            .ok_or(KError::Invalid)?;

        let buf_phys = virt_to_phys(buffer.as_ptr() as u64);

        let mut trb = Trb::new();
        trb.set_pointer(buf_phys);
        trb.status = buffer.len() as u32;
        trb.control = (TrbType::Normal as u32) << 10
            | (1 << 5); // IOC

        transfer_ring.push(trb);
        drop(tr);

        // Ring doorbell (target = endpoint DCI = ep_index + 1, but for doorbell it's ep_index)
        self.ring_doorbell(slot_id, ep_index as u8);

        Ok(())
    }

    /// Configure endpoint for bulk transfers (USB Mass Storage, etc.)
    pub fn configure_bulk_endpoint(
        &mut self,
        slot_id: u8,
        endpoint_num: u8,
        direction_in: bool,
        max_packet_size: u16,
    ) -> Result<(), KError> {
        // Calculate endpoint context index
        let ep_index = if direction_in {
            endpoint_num as usize * 2 + 1
        } else {
            endpoint_num as usize * 2
        };

        // Allocate transfer ring for this endpoint
        let ep_ring = TransferRing::new();
        let ep_ring_phys = ep_ring.phys_addr();

        let mut tr = self.transfer_rings.lock();
        let tr_index = (slot_id as usize - 1) * 32 + ep_index;
        if tr_index < tr.len() {
            tr[tr_index] = Some(ep_ring);
        }
        drop(tr);

        // Create input context for Configure Endpoint
        let mut input_ctx = Box::new(InputContext {
            control: InputControlContext::default(),
            slot: SlotContext::default(),
            endpoints: [EndpointContext::default(); 31],
        });

        // Set A0 (slot) and An for the endpoint
        input_ctx.control.add_flags = 1 | (1 << ep_index);

        // Update slot context
        if let Some(dev_ctx) = &self.device_contexts[slot_id as usize] {
            input_ctx.slot = dev_ctx.slot;
            let current_entries = (input_ctx.slot.data[0] >> 27) & 0x1F;
            if ep_index as u32 > current_entries {
                input_ctx.slot.data[0] = (input_ctx.slot.data[0] & !(0x1F << 27)) | ((ep_index as u32) << 27);
            }
        }

        // Setup endpoint context for bulk endpoint
        // EP Type: 6 = Bulk IN, 2 = Bulk OUT
        let ep_type = if direction_in { 6 } else { 2 };

        input_ctx.endpoints[ep_index - 1].data[0] = 0; // No interval for bulk
        input_ctx.endpoints[ep_index - 1].data[1] = (3 << 1) // CErr = 3
            | (ep_type << 3)
            | ((max_packet_size as u32) << 16);
        input_ctx.endpoints[ep_index - 1].data[2] = (ep_ring_phys as u32) | 1; // DCS = 1
        input_ctx.endpoints[ep_index - 1].data[3] = (ep_ring_phys >> 32) as u32;
        input_ctx.endpoints[ep_index - 1].data[4] = 0; // Average TRB length (0 for bulk)

        let input_ctx_phys = virt_to_phys(input_ctx.as_ref() as *const _ as u64);

        // Send Configure Endpoint command
        let mut trb = Trb::new();
        trb.set_pointer(input_ctx_phys);
        trb.control = (TrbType::ConfigureEndpoint as u32) << 10 | ((slot_id as u32) << 24);

        let event = self.send_command(trb)?;
        let completion = CompletionCode::from(((event.status >> 24) & 0xFF) as u8);

        if completion != CompletionCode::Success {
            crate::kprintln!("xhci: configure_bulk_endpoint failed: {:?}", completion);
            return Err(KError::IO);
        }

        Box::leak(input_ctx);
        crate::kprintln!("xhci: configured bulk endpoint {} (index {}) for slot {}", endpoint_num, ep_index, slot_id);
        Ok(())
    }

    /// Perform a bulk OUT transfer (send data)
    pub fn bulk_transfer_out(
        &mut self,
        slot_id: u8,
        endpoint_num: u8,
        data: &[u8],
    ) -> Result<usize, KError> {
        let ep_index = endpoint_num as usize * 2; // OUT endpoint
        let tr_index = (slot_id as usize - 1) * 32 + ep_index;

        let mut tr = self.transfer_rings.lock();
        let transfer_ring = tr.get_mut(tr_index)
            .ok_or(KError::Invalid)?
            .as_mut()
            .ok_or(KError::Invalid)?;

        let buf_phys = virt_to_phys(data.as_ptr() as u64);

        let mut trb = Trb::new();
        trb.set_pointer(buf_phys);
        trb.status = data.len() as u32;
        trb.control = (TrbType::Normal as u32) << 10
            | (1 << 5); // IOC (Interrupt on Completion)

        transfer_ring.push(trb);
        drop(tr);

        // Ring doorbell
        self.ring_doorbell(slot_id, ep_index as u8);

        // Wait for completion
        for _ in 0..100000 {
            let mut event_ring = self.event_ring.lock();
            if let Some(event) = event_ring.pop() {
                let trb_type = ((event.control >> 10) & 0x3F) as u8;
                if trb_type == TrbType::TransferEvent as u8 {
                    let completion = CompletionCode::from(((event.status >> 24) & 0xFF) as u8);
                    let residual = event.status & 0xFFFFFF;

                    // Update ERDP
                    unsafe {
                        let ir = self.interrupter(0);
                        let erdp = event_ring.dequeue_phys() | (1 << 3);
                        write_volatile(&mut (*ir).erdp_lo, erdp as u32);
                        write_volatile(&mut (*ir).erdp_hi, (erdp >> 32) as u32);
                    }

                    if completion == CompletionCode::Success || completion == CompletionCode::ShortPacket {
                        return Ok(data.len() - residual as usize);
                    } else {
                        return Err(KError::IO);
                    }
                }
            }
            drop(event_ring);
            for _ in 0..10 { core::hint::spin_loop(); }
        }

        Err(KError::Timeout)
    }

    /// Perform a bulk IN transfer (receive data)
    pub fn bulk_transfer_in(
        &mut self,
        slot_id: u8,
        endpoint_num: u8,
        buffer: &mut [u8],
    ) -> Result<usize, KError> {
        let ep_index = endpoint_num as usize * 2 + 1; // IN endpoint
        let tr_index = (slot_id as usize - 1) * 32 + ep_index;

        let mut tr = self.transfer_rings.lock();
        let transfer_ring = tr.get_mut(tr_index)
            .ok_or(KError::Invalid)?
            .as_mut()
            .ok_or(KError::Invalid)?;

        let buf_phys = virt_to_phys(buffer.as_ptr() as u64);

        let mut trb = Trb::new();
        trb.set_pointer(buf_phys);
        trb.status = buffer.len() as u32;
        trb.control = (TrbType::Normal as u32) << 10
            | (1 << 5); // IOC

        transfer_ring.push(trb);
        drop(tr);

        // Ring doorbell
        self.ring_doorbell(slot_id, ep_index as u8);

        // Wait for completion
        for _ in 0..100000 {
            let mut event_ring = self.event_ring.lock();
            if let Some(event) = event_ring.pop() {
                let trb_type = ((event.control >> 10) & 0x3F) as u8;
                if trb_type == TrbType::TransferEvent as u8 {
                    let completion = CompletionCode::from(((event.status >> 24) & 0xFF) as u8);
                    let residual = event.status & 0xFFFFFF;

                    // Update ERDP
                    unsafe {
                        let ir = self.interrupter(0);
                        let erdp = event_ring.dequeue_phys() | (1 << 3);
                        write_volatile(&mut (*ir).erdp_lo, erdp as u32);
                        write_volatile(&mut (*ir).erdp_hi, (erdp >> 32) as u32);
                    }

                    if completion == CompletionCode::Success || completion == CompletionCode::ShortPacket {
                        return Ok(buffer.len() - residual as usize);
                    } else {
                        return Err(KError::IO);
                    }
                }
            }
            drop(event_ring);
            for _ in 0..10 { core::hint::spin_loop(); }
        }

        Err(KError::Timeout)
    }

    /// Poll for interrupt transfer completion (non-blocking)
    pub fn poll_interrupt_transfer(&self) -> Option<(u8, u8, usize)> {
        let mut event_ring = self.event_ring.lock();

        if let Some(event) = event_ring.pop() {
            let trb_type = ((event.control >> 10) & 0x3F) as u8;

            if trb_type == TrbType::TransferEvent as u8 {
                let completion = CompletionCode::from(((event.status >> 24) & 0xFF) as u8);
                let residual = (event.status & 0xFFFFFF) as usize;
                let slot_id = ((event.control >> 24) & 0xFF) as u8;
                let ep_id = ((event.control >> 16) & 0x1F) as u8;

                // Update ERDP
                unsafe {
                    let ir = self.interrupter(0);
                    let erdp = event_ring.dequeue_phys() | (1 << 3);
                    write_volatile(&mut (*ir).erdp_lo, erdp as u32);
                    write_volatile(&mut (*ir).erdp_hi, (erdp >> 32) as u32);
                }

                if completion == CompletionCode::Success || completion == CompletionCode::ShortPacket {
                    return Some((slot_id, ep_id, residual));
                }
            } else if trb_type == TrbType::PortStatusChange as u8 {
                let port_id = ((event.param_lo >> 24) & 0xFF) as u8;
                crate::kprintln!("xhci: port {} status change", port_id);
                // Acknowledge
                unsafe {
                    let pr = self.port_regs(port_id);
                    let portsc = read_volatile(&(*pr).portsc);
                    write_volatile(&mut (*pr).portsc, portsc | (0x3F << 17)); // Clear change bits
                }
                // Update ERDP
                unsafe {
                    let ir = self.interrupter(0);
                    let erdp = event_ring.dequeue_phys() | (1 << 3);
                    write_volatile(&mut (*ir).erdp_lo, erdp as u32);
                    write_volatile(&mut (*ir).erdp_hi, (erdp >> 32) as u32);
                }
            }
        }

        None
    }

    /// Enumerate a newly connected device on a port
    pub fn enumerate_device(&mut self, port: u8) -> Result<u8, KError> {
        // 1. Enable a slot
        let slot_id = self.enable_slot()?;

        // 2. Get port speed
        let speed = unsafe {
            let pr = self.port_regs(port);
            let portsc = read_volatile(&(*pr).portsc);
            UsbSpeed::from_xhci_speed(((portsc >> 10) & 0xF) as u8)
        };

        // 3. Address the device (BSR=1)
        self.address_device(slot_id, port, speed)?;

        // 4. Now we need to set BSR=0 to actually assign address
        // This is done by another Address Device command with BSR=0
        // But first, let's update the device context

        crate::kprintln!("xhci: device enumerated on port {}, slot {}", port, slot_id);
        Ok(slot_id)
    }
}

// ============================================================================
// USB Device Enumeration and HID Setup
// ============================================================================

/// Enumerate all connected USB devices and setup HID devices
pub fn enumerate_all_devices() {
    if let Some(ctrl_arc) = controller() {
        let mut ctrl = ctrl_arc.lock();

        for port in 1..=ctrl.max_ports {
            unsafe {
                let pr = ctrl.port_regs(port);
                let portsc = read_volatile(&(*pr).portsc);

                // Check if device is connected and port is enabled
                if (portsc & 1) != 0 && (portsc & (1 << 1)) != 0 {
                    crate::kprintln!("xhci: device detected on port {}", port);

                    match ctrl.enumerate_device(port) {
                        Ok(slot_id) => {
                            // Get device descriptor
                            match ctrl.get_device_descriptor(slot_id) {
                                Ok(desc) => {
                                    // Copy packed struct fields to avoid unaligned access
                                    let vendor_id = { desc.vendor_id };
                                    let product_id = { desc.product_id };
                                    let device_class = { desc.device_class };
                                    crate::kprintln!(
                                        "xhci: device {:04x}:{:04x} class={:02x}",
                                        vendor_id, product_id, device_class
                                    );

                                    // Get configuration descriptor
                                    if let Ok(config) = ctrl.get_config_descriptor(slot_id, 0) {
                                        setup_device_from_config(&mut ctrl, slot_id, &config);
                                    }
                                }
                                Err(e) => {
                                    crate::kprintln!("xhci: failed to get device descriptor: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            crate::kprintln!("xhci: failed to enumerate device on port {}: {:?}", port, e);
                        }
                    }
                }
            }
        }
    }
}

/// Endpoint info collected during config parsing
struct EndpointInfo {
    endpoint_number: u8,
    direction_in: bool,
    max_packet_size: u16,
    interval: u8,
    interface_number: u8,
    interface_protocol: u8,
}

/// Parse configuration descriptor and setup device
fn setup_device_from_config(ctrl: &mut XhciController, slot_id: u8, config: &[u8]) {
    if config.len() < 9 {
        return;
    }

    let config_value = config[5]; // bConfigurationValue
    let mut offset = 9; // Skip config descriptor

    let mut current_interface: Option<super::InterfaceDescriptor> = None;
    let mut hid_endpoints: Vec<EndpointInfo> = Vec::new();
    let mut is_hub = false;

    // Phase 1: Parse the configuration to collect all endpoints and detect hubs
    while offset + 2 <= config.len() {
        let len = config[offset] as usize;
        let desc_type = config[offset + 1];

        if len == 0 || offset + len > config.len() {
            break;
        }

        match desc_type {
            4 => {
                // Interface descriptor
                if len >= 9 {
                    let iface = unsafe {
                        core::ptr::read_unaligned(config[offset..].as_ptr() as *const super::InterfaceDescriptor)
                    };
                    // Check if this is a hub interface
                    if iface.interface_class == super::class::HUB {
                        is_hub = true;
                        crate::kprintln!("xhci: detected USB hub interface");
                    }
                    current_interface = Some(iface);
                }
            }
            5 => {
                // Endpoint descriptor
                if len >= 7 {
                    let ep_desc = unsafe {
                        core::ptr::read_unaligned(config[offset..].as_ptr() as *const super::EndpointDescriptor)
                    };

                    if let Some(ref iface) = current_interface {
                        // Check if this is a HID interface with interrupt IN endpoint
                        if super::hid::is_hid_interface(iface)
                            && ep_desc.transfer_type() == super::EndpointType::Interrupt
                            && ep_desc.direction() == super::EndpointDirection::In
                        {
                            crate::kprintln!(
                                "xhci: found HID interface {} protocol {}",
                                iface.interface_number,
                                iface.interface_protocol
                            );

                            hid_endpoints.push(EndpointInfo {
                                endpoint_number: ep_desc.endpoint_number(),
                                direction_in: true,
                                max_packet_size: ep_desc.max_packet_size,
                                interval: ep_desc.interval,
                                interface_number: iface.interface_number,
                                interface_protocol: iface.interface_protocol,
                            });
                        }
                    }
                }
            }
            _ => {}
        }

        offset += len;
    }

    // Phase 2: Set configuration first (USB request to device)
    if let Err(e) = ctrl.set_configuration(slot_id, config_value) {
        crate::kprintln!("xhci: failed to set configuration: {:?}", e);
        return;
    }
    crate::kprintln!("xhci: device configuration {} set successfully", config_value);

    // If this is a hub, handle it specially
    if is_hub {
        if let Err(e) = setup_hub_device(ctrl, slot_id) {
            crate::kprintln!("xhci: failed to setup hub: {:?}", e);
        }
        return;
    }

    // Phase 3: Configure endpoints in host controller (device is now in Configured state)
    for ep_info in &hid_endpoints {
        if let Err(e) = ctrl.configure_interrupt_endpoint(
            slot_id,
            ep_info.endpoint_number,
            ep_info.direction_in,
            ep_info.max_packet_size,
            ep_info.interval,
        ) {
            crate::kprintln!("xhci: failed to configure endpoint {}: {:?}", ep_info.endpoint_number, e);
            continue;
        }

        // Register HID device
        // Re-parse to get the interface and endpoint descriptors for the HID module
        let iface_desc = super::InterfaceDescriptor {
            length: 9,
            descriptor_type: 4,
            interface_number: ep_info.interface_number,
            alternate_setting: 0,
            num_endpoints: 1,
            interface_class: 3, // HID
            interface_subclass: 1, // Boot interface
            interface_protocol: ep_info.interface_protocol,
            interface_index: 0,
        };
        let ep_desc = super::EndpointDescriptor {
            length: 7,
            descriptor_type: 5,
            endpoint_address: ep_info.endpoint_number | 0x80, // IN
            attributes: 3, // Interrupt
            max_packet_size: ep_info.max_packet_size,
            interval: ep_info.interval,
        };

        if let Err(e) = super::hid::configure_device(slot_id, config, &iface_desc, &ep_desc) {
            crate::kprintln!("xhci: failed to configure HID device: {:?}", e);
        }
    }

    // Phase 4: Set boot protocol and idle rate for HID interfaces
    for ep_info in &hid_endpoints {
        // Set boot protocol (0x0B with wValue=0)
        if let Err(e) = set_hid_boot_protocol(ctrl, slot_id, ep_info.interface_number) {
            crate::kprintln!("xhci: failed to set boot protocol for interface {}: {:?}", ep_info.interface_number, e);
        }

        // Set idle rate to 0 (infinite - report only on change)
        if let Err(e) = set_hid_idle(ctrl, slot_id, ep_info.interface_number) {
            crate::kprintln!("xhci: failed to set idle for interface {}: {:?}", ep_info.interface_number, e);
        }
    }
}

/// Send SET_PROTOCOL request to put HID device in boot protocol mode
pub fn set_hid_boot_protocol(ctrl: &mut XhciController, slot_id: u8, interface: u8) -> Result<(), KError> {
    // SET_PROTOCOL: bmRequestType=0x21, bRequest=0x0B, wValue=0 (boot protocol), wIndex=interface
    let setup = super::SetupPacket {
        request_type: 0x21, // Host to Device, Class, Interface
        request: 0x0B,      // SET_PROTOCOL
        value: 0,           // 0 = Boot Protocol
        index: interface as u16,
        length: 0,
    };
    ctrl.control_transfer(slot_id, &setup, None, false)?;
    crate::kprintln!("xhci: set boot protocol for interface {}", interface);
    Ok(())
}

/// Send SET_IDLE request to set idle rate (0 = report only on change)
pub fn set_hid_idle(ctrl: &mut XhciController, slot_id: u8, interface: u8) -> Result<(), KError> {
    // SET_IDLE: bmRequestType=0x21, bRequest=0x0A, wValue=0 (infinite), wIndex=interface
    let setup = super::SetupPacket {
        request_type: 0x21, // Host to Device, Class, Interface
        request: 0x0A,      // SET_IDLE
        value: 0,           // 0 = infinite (report only on change)
        index: interface as u16,
        length: 0,
    };
    ctrl.control_transfer(slot_id, &setup, None, false)?;
    crate::kprintln!("xhci: set idle rate for interface {}", interface);
    Ok(())
}

/// Setup a USB hub device
fn setup_hub_device(ctrl: &mut XhciController, slot_id: u8) -> Result<(), KError> {
    crate::kprintln!("xhci: setting up USB hub on slot {}", slot_id);

    // Get hub descriptor
    let setup = super::hub::UsbHub::get_hub_descriptor_setup(8, false);
    let mut hub_desc = [0u8; 8];

    ctrl.control_transfer(slot_id, &setup, Some(&mut hub_desc), true)?;

    let num_ports = hub_desc[2];
    let characteristics = (hub_desc[3] as u16) | ((hub_desc[4] as u16) << 8);
    let power_on_time = hub_desc[5]; // in 2ms units

    crate::kprintln!(
        "xhci: hub has {} ports, characteristics=0x{:04x}, power_on_time={}ms",
        num_ports,
        characteristics,
        (power_on_time as u16) * 2
    );

    // Register the hub
    super::hub::setup_hub(slot_id, false, num_ports, power_on_time, 0);

    // Power on each port
    for port in 1..=num_ports {
        let setup = super::hub::UsbHub::set_port_feature_setup(port, super::hub::hub_feature::PORT_POWER);
        if let Err(e) = ctrl.control_transfer(slot_id, &setup, None, false) {
            crate::kprintln!("xhci: failed to power hub port {}: {:?}", port, e);
        } else {
            crate::kprintln!("xhci: powered hub port {}", port);
        }
    }

    // Wait for power to stabilize
    let delay_ms = (power_on_time as u64) * 2 + 10;
    crate::drivers::hpet::sleep_ms(delay_ms);

    // Check port status for connected devices
    for port in 1..=num_ports {
        match get_hub_port_status(ctrl, slot_id, port) {
            Ok(status) => {
                if status.connected() {
                    crate::kprintln!("xhci: device connected on hub port {}", port);

                    // Reset the port to enable the device
                    let setup = super::hub::UsbHub::set_port_feature_setup(
                        port,
                        super::hub::hub_feature::PORT_RESET,
                    );
                    if let Err(e) = ctrl.control_transfer(slot_id, &setup, None, false) {
                        crate::kprintln!("xhci: failed to reset hub port {}: {:?}", port, e);
                        continue;
                    }

                    // Wait for reset to complete
                    crate::drivers::hpet::sleep_ms(50);

                    // Check status again
                    if let Ok(new_status) = get_hub_port_status(ctrl, slot_id, port) {
                        if new_status.enabled() {
                            crate::kprintln!(
                                "xhci: hub port {} enabled, speed={:?}",
                                port,
                                new_status.speed()
                            );
                            // TODO: Enumerate device on this hub port
                            // This requires implementing hub-based device enumeration
                            // which involves route strings and hub port routing
                        }
                    }

                    // Clear connection change status
                    let setup = super::hub::UsbHub::clear_port_feature_setup(
                        port,
                        super::hub::hub_feature::C_PORT_CONNECTION,
                    );
                    let _ = ctrl.control_transfer(slot_id, &setup, None, false);
                }
            }
            Err(e) => {
                crate::kprintln!("xhci: failed to get hub port {} status: {:?}", port, e);
            }
        }
    }

    Ok(())
}

/// Get status of a hub port
fn get_hub_port_status(
    ctrl: &mut XhciController,
    slot_id: u8,
    port: u8,
) -> Result<super::hub::PortStatus, KError> {
    let setup = super::hub::UsbHub::get_port_status_setup(port);
    let mut data = [0u8; 4];

    ctrl.control_transfer(slot_id, &setup, Some(&mut data), true)?;

    Ok(super::hub::PortStatus {
        status: (data[0] as u16) | ((data[1] as u16) << 8),
        change: (data[2] as u16) | ((data[3] as u16) << 8),
    })
}
