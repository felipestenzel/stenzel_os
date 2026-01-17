//! APIC (Advanced Programmable Interrupt Controller)
//!
//! Implementa Local APIC e I/O APIC para hardware moderno.
//! Usa informações do MADT (ACPI) quando disponível.

#![allow(dead_code)]

use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicU32, Ordering};

use x86_64::instructions::port::Port;

/// Base padrão do Local APIC (pode ser diferente, checar MSR)
const LAPIC_BASE_DEFAULT: u64 = 0xFEE0_0000;

/// Base padrão do I/O APIC
const IOAPIC_BASE_DEFAULT: u64 = 0xFEC0_0000;

/// MSR para APIC base
const IA32_APIC_BASE_MSR: u32 = 0x1B;

// Registradores do Local APIC (offsets)
const LAPIC_ID: u32 = 0x020;
const LAPIC_VERSION: u32 = 0x030;
const LAPIC_TPR: u32 = 0x080;       // Task Priority Register
const LAPIC_APR: u32 = 0x090;       // Arbitration Priority Register
const LAPIC_PPR: u32 = 0x0A0;       // Processor Priority Register
const LAPIC_EOI: u32 = 0x0B0;       // End of Interrupt
const LAPIC_RRD: u32 = 0x0C0;       // Remote Read Register
const LAPIC_LDR: u32 = 0x0D0;       // Logical Destination Register
const LAPIC_DFR: u32 = 0x0E0;       // Destination Format Register
const LAPIC_SVR: u32 = 0x0F0;       // Spurious Interrupt Vector Register
const LAPIC_ISR: u32 = 0x100;       // In-Service Register (8 regs)
const LAPIC_TMR: u32 = 0x180;       // Trigger Mode Register (8 regs)
const LAPIC_IRR: u32 = 0x200;       // Interrupt Request Register (8 regs)
const LAPIC_ESR: u32 = 0x280;       // Error Status Register
const LAPIC_ICR_LO: u32 = 0x300;    // Interrupt Command Register (low)
const LAPIC_ICR_HI: u32 = 0x310;    // Interrupt Command Register (high)
const LAPIC_LVT_TIMER: u32 = 0x320; // LVT Timer
const LAPIC_LVT_THERMAL: u32 = 0x330; // LVT Thermal Sensor
const LAPIC_LVT_PERF: u32 = 0x340;  // LVT Performance Counter
const LAPIC_LVT_LINT0: u32 = 0x350; // LVT LINT0
const LAPIC_LVT_LINT1: u32 = 0x360; // LVT LINT1
const LAPIC_LVT_ERROR: u32 = 0x370; // LVT Error
const LAPIC_TIMER_ICR: u32 = 0x380; // Timer Initial Count
const LAPIC_TIMER_CCR: u32 = 0x390; // Timer Current Count
const LAPIC_TIMER_DCR: u32 = 0x3E0; // Timer Divide Configuration

// I/O APIC registers
const IOAPIC_REGSEL: u32 = 0x00;
const IOAPIC_WIN: u32 = 0x10;

// I/O APIC register indices
const IOAPIC_REG_ID: u32 = 0x00;
const IOAPIC_REG_VER: u32 = 0x01;
const IOAPIC_REG_ARB: u32 = 0x02;
const IOAPIC_REDTBL_BASE: u32 = 0x10;

/// Vetores de interrupção para APIC
pub const APIC_TIMER_VECTOR: u8 = 32;
pub const APIC_KEYBOARD_VECTOR: u8 = 33;
pub const APIC_CASCADE_VECTOR: u8 = 34;  // Not used but reserved
pub const APIC_COM2_VECTOR: u8 = 35;
pub const APIC_COM1_VECTOR: u8 = 36;
pub const APIC_LPT2_VECTOR: u8 = 37;
pub const APIC_FLOPPY_VECTOR: u8 = 38;
pub const APIC_LPT1_VECTOR: u8 = 39;
pub const APIC_RTC_VECTOR: u8 = 40;
pub const APIC_FREE1_VECTOR: u8 = 41;
pub const APIC_FREE2_VECTOR: u8 = 42;
pub const APIC_FREE3_VECTOR: u8 = 43;
pub const APIC_MOUSE_VECTOR: u8 = 44;
pub const APIC_FPU_VECTOR: u8 = 45;
pub const APIC_ATA1_VECTOR: u8 = 46;
pub const APIC_ATA2_VECTOR: u8 = 47;
pub const APIC_SPURIOUS_VECTOR: u8 = 0xFF;
pub const APIC_ERROR_VECTOR: u8 = 0xFE;

/// Estado do APIC
static APIC_ENABLED: AtomicBool = AtomicBool::new(false);
static LAPIC_BASE: AtomicU64 = AtomicU64::new(0);
static IOAPIC_BASE: AtomicU64 = AtomicU64::new(0);
static LAPIC_TICKS_PER_MS: AtomicU32 = AtomicU32::new(0);

/// Interrupt Source Override entry (from MADT)
#[derive(Debug, Clone, Copy)]
pub struct IsoEntry {
    pub bus_source: u8,
    pub irq_source: u8,
    pub gsi: u32,
    pub flags: u16,
}

/// Global interrupt source overrides
static mut ISO_ENTRIES: [Option<IsoEntry>; 16] = [None; 16];
static ISO_COUNT: AtomicU32 = AtomicU32::new(0);

/// Verifica se APIC está disponível via CPUID
pub fn is_apic_available() -> bool {
    let cpuid = unsafe { core::arch::x86_64::__cpuid(1) };
    // Bit 9 do EDX indica APIC
    (cpuid.edx & (1 << 9)) != 0
}

/// Verifica se x2APIC está disponível
pub fn is_x2apic_available() -> bool {
    let cpuid = unsafe { core::arch::x86_64::__cpuid(1) };
    // Bit 21 do ECX indica x2APIC
    (cpuid.ecx & (1 << 21)) != 0
}

/// Converte endereço físico do APIC para virtual
fn apic_phys_to_virt(phys: u64) -> u64 {
    crate::mm::phys_to_virt(x86_64::PhysAddr::new(phys)).as_u64()
}

/// Lê um registro do Local APIC
unsafe fn lapic_read(offset: u32) -> u32 {
    let phys_base = LAPIC_BASE.load(Ordering::Relaxed);
    if phys_base == 0 {
        return 0;
    }
    let virt = apic_phys_to_virt(phys_base + offset as u64);
    read_volatile(virt as *const u32)
}

/// Escreve em um registro do Local APIC
unsafe fn lapic_write(offset: u32, value: u32) {
    let phys_base = LAPIC_BASE.load(Ordering::Relaxed);
    if phys_base == 0 {
        return;
    }
    let virt = apic_phys_to_virt(phys_base + offset as u64);
    write_volatile(virt as *mut u32, value);
}

/// Lê um registro do I/O APIC
unsafe fn ioapic_read(reg: u32) -> u32 {
    let phys_base = IOAPIC_BASE.load(Ordering::Relaxed);
    if phys_base == 0 {
        return 0;
    }
    let regsel = apic_phys_to_virt(phys_base) as *mut u32;
    let win = apic_phys_to_virt(phys_base + IOAPIC_WIN as u64) as *mut u32;

    write_volatile(regsel, reg);
    read_volatile(win)
}

/// Escreve em um registro do I/O APIC
unsafe fn ioapic_write(reg: u32, value: u32) {
    let phys_base = IOAPIC_BASE.load(Ordering::Relaxed);
    if phys_base == 0 {
        return;
    }
    let regsel = apic_phys_to_virt(phys_base) as *mut u32;
    let win = apic_phys_to_virt(phys_base + IOAPIC_WIN as u64) as *mut u32;

    write_volatile(regsel, reg);
    write_volatile(win, value);
}

/// Configura uma entrada da tabela de redireção do I/O APIC
unsafe fn ioapic_set_irq_full(gsi: u8, vector: u8, destination: u8, mask: bool,
                              level_triggered: bool, low_active: bool) {
    let reg_lo = IOAPIC_REDTBL_BASE + (gsi as u32 * 2);
    let reg_hi = reg_lo + 1;

    // Bits 63:56 = destination APIC ID
    let hi = (destination as u32) << 24;

    // Low 32 bits:
    // Bit 16 = mask (1 = masked)
    // Bit 15 = trigger mode (0 = edge, 1 = level)
    // Bit 13 = interrupt pin polarity (0 = high active, 1 = low active)
    // Bit 11 = destination mode (0 = physical)
    // Bits 10:8 = delivery mode (000 = fixed)
    // Bits 7:0 = vector
    let mut lo = vector as u32;
    if mask {
        lo |= 1 << 16;
    }
    if level_triggered {
        lo |= 1 << 15;
    }
    if low_active {
        lo |= 1 << 13;
    }

    ioapic_write(reg_hi, hi);
    ioapic_write(reg_lo, lo);
}

/// Configura uma entrada da tabela de redireção do I/O APIC (edge-triggered, active high)
unsafe fn ioapic_set_irq(irq: u8, vector: u8, destination: u8, mask: bool) {
    // Check for interrupt source override
    let (gsi, level, low) = get_irq_mapping(irq);
    ioapic_set_irq_full(gsi as u8, vector, destination, mask, level, low);
}

/// Get the GSI and trigger mode for an IRQ (applying ISOs)
fn get_irq_mapping(irq: u8) -> (u32, bool, bool) {
    let count = ISO_COUNT.load(Ordering::Relaxed) as usize;

    unsafe {
        for i in 0..count {
            if let Some(iso) = ISO_ENTRIES[i] {
                if iso.irq_source == irq {
                    // flags: bits 0-1 = polarity, bits 2-3 = trigger mode
                    let polarity = iso.flags & 0x3;
                    let trigger = (iso.flags >> 2) & 0x3;

                    // Polarity: 0=bus default, 1=active high, 3=active low
                    let low_active = polarity == 3;
                    // Trigger: 0=bus default, 1=edge, 3=level
                    let level_triggered = trigger == 3;

                    return (iso.gsi, level_triggered, low_active);
                }
            }
        }
    }

    // No override, use IRQ as GSI, edge-triggered, active high
    (irq as u32, false, false)
}

/// Desabilita o PIC 8259 (máscaras todas as IRQs)
unsafe fn disable_pic() {
    // Remap PIC to avoid spurious interrupts during transition
    let mut pic1_cmd = Port::<u8>::new(0x20);
    let mut pic1_data = Port::<u8>::new(0x21);
    let mut pic2_cmd = Port::<u8>::new(0xA0);
    let mut pic2_data = Port::<u8>::new(0xA1);

    // ICW1: init + ICW4 needed
    pic1_cmd.write(0x11);
    pic2_cmd.write(0x11);

    // ICW2: vector offsets (remap to 0x20-0x27 and 0x28-0x2F)
    pic1_data.write(0x20);
    pic2_data.write(0x28);

    // ICW3: master/slave wiring
    pic1_data.write(4);
    pic2_data.write(2);

    // ICW4: 8086 mode
    pic1_data.write(0x01);
    pic2_data.write(0x01);

    // Mask all interrupts
    pic1_data.write(0xFF);
    pic2_data.write(0xFF);

    crate::kprintln!("apic: PIC 8259 desabilitado");
}

/// Obtém o APIC base do MSR
unsafe fn get_apic_base() -> u64 {
    let lo: u32;
    let hi: u32;

    core::arch::asm!(
        "rdmsr",
        in("ecx") IA32_APIC_BASE_MSR,
        out("eax") lo,
        out("edx") hi,
    );

    // Base address está nos bits 12:35 (página física)
    ((hi as u64) << 32 | lo as u64) & 0xFFFF_F000
}

/// Habilita o Local APIC via MSR
unsafe fn enable_apic_msr() {
    let lo: u32;
    let hi: u32;

    core::arch::asm!(
        "rdmsr",
        in("ecx") IA32_APIC_BASE_MSR,
        out("eax") lo,
        out("edx") hi,
    );

    // Set bit 11 (APIC Global Enable)
    let new_lo = lo | (1 << 11);

    core::arch::asm!(
        "wrmsr",
        in("ecx") IA32_APIC_BASE_MSR,
        in("eax") new_lo,
        in("edx") hi,
    );
}

/// Inicializa o Local APIC
unsafe fn init_lapic(lapic_addr: Option<u32>) {
    // Use MADT address if available, otherwise get from MSR
    let base = if let Some(addr) = lapic_addr {
        addr as u64
    } else {
        get_apic_base()
    };

    LAPIC_BASE.store(base, Ordering::Relaxed);
    crate::kprintln!("apic: Local APIC base = {:#x}", base);

    // Habilita o APIC via MSR
    enable_apic_msr();

    // Lê a versão
    let version = lapic_read(LAPIC_VERSION);
    let max_lvt = ((version >> 16) & 0xFF) + 1;
    crate::kprintln!("apic: version = {:#x}, max LVT entries = {}", version & 0xFF, max_lvt);

    // Configura o Spurious Interrupt Vector Register
    // Bit 8 = APIC Software Enable
    // Bits 7:0 = Spurious Vector (deve ser 0xXF)
    lapic_write(LAPIC_SVR, (1 << 8) | APIC_SPURIOUS_VECTOR as u32);

    // Limpa o Task Priority Register (permite todas as interrupções)
    lapic_write(LAPIC_TPR, 0);

    // Configura DFR para flat model
    lapic_write(LAPIC_DFR, 0xFFFFFFFF);

    // Configura LDR
    lapic_write(LAPIC_LDR, (lapic_read(LAPIC_LDR) & 0x00FFFFFF) | 1);

    // Configura LVT Error
    lapic_write(LAPIC_LVT_ERROR, APIC_ERROR_VECTOR as u32);

    // Configura LINT0 e LINT1 (mascarados por padrão)
    lapic_write(LAPIC_LVT_LINT0, 1 << 16);
    lapic_write(LAPIC_LVT_LINT1, 1 << 16);

    // Clear error status by writing 0
    lapic_write(LAPIC_ESR, 0);
    lapic_write(LAPIC_ESR, 0);

    // Clear any pending interrupts
    lapic_write(LAPIC_EOI, 0);

    crate::kprintln!("apic: Local APIC inicializado (ID={})", lapic_id());
}

/// Calibra o timer do LAPIC usando o PIT
unsafe fn calibrate_lapic_timer() -> u32 {
    // Configura o PIT channel 2 para one-shot mode
    let mut pit_cmd = Port::<u8>::new(0x43);
    let mut pit_ch2 = Port::<u8>::new(0x42);
    let mut port_61 = Port::<u8>::new(0x61);

    // Save and enable speaker gate for channel 2
    let old_61 = port_61.read();
    port_61.write((old_61 & 0xFD) | 0x01);

    // Configure PIT channel 2 for mode 0 (one-shot)
    pit_cmd.write(0xB0); // Channel 2, lo/hi, mode 0, binary

    // Count for ~10ms (1193182 Hz / 100 = 11932)
    let pit_count: u16 = 11932;
    pit_ch2.write((pit_count & 0xFF) as u8);
    pit_ch2.write((pit_count >> 8) as u8);

    // Set LAPIC timer divider to 16
    lapic_write(LAPIC_TIMER_DCR, 0x03);

    // Start LAPIC counter with max value
    lapic_write(LAPIC_TIMER_ICR, 0xFFFFFFFF);

    // Wait for PIT to finish (bit 5 of port 0x61 goes high)
    while (port_61.read() & 0x20) == 0 {
        core::hint::spin_loop();
    }

    // Stop the LAPIC timer
    lapic_write(LAPIC_LVT_TIMER, 1 << 16); // Mask

    // Calculate how many ticks elapsed in ~10ms
    let elapsed = 0xFFFFFFFF - lapic_read(LAPIC_TIMER_CCR);

    // Restore port 61
    port_61.write(old_61);

    // elapsed is ticks in ~10ms, multiply by 100 to get ticks per second
    // Then divide by 1000 to get ticks per ms
    let ticks_per_sec = elapsed.saturating_mul(100);
    let ticks_per_ms = ticks_per_sec / 1000;

    crate::kprintln!("apic: timer calibration: {} ticks/10ms, {} ticks/sec",
                     elapsed, ticks_per_sec);

    LAPIC_TICKS_PER_MS.store(ticks_per_ms, Ordering::Relaxed);

    ticks_per_sec
}

/// Inicializa o timer do Local APIC
unsafe fn init_lapic_timer(frequency_hz: u32) {
    // Calibrate using PIT
    let ticks_per_sec = calibrate_lapic_timer();

    if ticks_per_sec == 0 {
        crate::kprintln!("apic: timer calibration failed!");
        return;
    }

    // Set divider to 16
    lapic_write(LAPIC_TIMER_DCR, 0x03);

    // Configure LVT Timer for periodic mode
    // Bits 18:17 = Timer Mode (01 = periodic)
    // Bits 7:0 = Vector
    lapic_write(LAPIC_LVT_TIMER, (1 << 17) | APIC_TIMER_VECTOR as u32);

    // Calculate initial count for desired frequency
    let initial_count = ticks_per_sec / frequency_hz;
    lapic_write(LAPIC_TIMER_ICR, initial_count);

    crate::kprintln!("apic: timer configurado para {}Hz (initial count = {})",
                     frequency_hz, initial_count);
}

/// Inicializa o I/O APIC
unsafe fn init_ioapic(ioapic_addr: Option<u32>) {
    // Use MADT address if available, otherwise use default
    let base = if let Some(addr) = ioapic_addr {
        addr as u64
    } else {
        IOAPIC_BASE_DEFAULT
    };

    IOAPIC_BASE.store(base, Ordering::Relaxed);

    let id = ioapic_read(IOAPIC_REG_ID);
    let version = ioapic_read(IOAPIC_REG_VER);
    let max_redir = ((version >> 16) & 0xFF) + 1;

    crate::kprintln!("apic: I/O APIC @ {:#x}, id = {}, version = {:#x}, max redirections = {}",
                     base, (id >> 24) & 0xF, version & 0xFF, max_redir);

    // Mask all entries first
    for i in 0..max_redir as u8 {
        ioapic_set_irq_full(i, 0, 0, true, false, false);
    }

    // Configure standard ISA IRQs
    // IRQ0: Timer -> vector 32
    ioapic_set_irq(0, APIC_TIMER_VECTOR, 0, false);

    // IRQ1: Keyboard -> vector 33
    ioapic_set_irq(1, APIC_KEYBOARD_VECTOR, 0, false);

    // IRQ8: RTC -> vector 40
    ioapic_set_irq(8, APIC_RTC_VECTOR, 0, false);

    // IRQ12: PS/2 Mouse -> vector 44
    ioapic_set_irq(12, APIC_MOUSE_VECTOR, 0, false);

    // IRQ14: Primary ATA -> vector 46
    ioapic_set_irq(14, APIC_ATA1_VECTOR, 0, false);

    // IRQ15: Secondary ATA -> vector 47
    ioapic_set_irq(15, APIC_ATA2_VECTOR, 0, false);

    crate::kprintln!("apic: I/O APIC inicializado com IRQ routing");
}

/// Inicializa o APIC usando informações do MADT (ACPI)
pub fn init_with_madt() -> bool {
    if !is_apic_available() {
        crate::kprintln!("apic: não disponível neste hardware");
        return false;
    }

    // Try to get MADT info from ACPI
    let madt_info = crate::drivers::acpi::parse_madt();

    let (lapic_addr, ioapic_addr) = if let Some(ref info) = madt_info {
        crate::kprintln!("apic: MADT encontrado - {} CPUs, {} I/O APICs",
                         info.local_apics.len(), info.io_apics.len());

        let ioapic = info.io_apics.first().map(|io| io.address);
        (Some(info.local_apic_address), ioapic)
    } else {
        crate::kprintln!("apic: MADT não encontrado, usando endereços padrão");
        (None, None)
    };

    // Parse interrupt source overrides from MADT
    if let Some(ref _info) = madt_info {
        parse_interrupt_source_overrides();
    }

    unsafe {
        // Disable PIC 8259
        disable_pic();

        // Initialize Local APIC
        init_lapic(lapic_addr);

        // Initialize LAPIC timer (1000Hz like PIT)
        init_lapic_timer(1000);

        // Initialize I/O APIC
        init_ioapic(ioapic_addr);
    }

    APIC_ENABLED.store(true, Ordering::Relaxed);
    crate::kprintln!("apic: sistema APIC inicializado com sucesso");
    true
}

/// Parse interrupt source overrides from MADT
fn parse_interrupt_source_overrides() {
    use crate::drivers::acpi::{find_table, AcpiTableHeader};

    let madt = match find_table(b"APIC") {
        Some(t) => t,
        None => return,
    };

    let phys_offset = crate::mm::physical_memory_offset();
    let madt_virt = phys_offset + madt.address;

    unsafe {
        #[repr(C, packed)]
        struct MadtHeader {
            header: AcpiTableHeader,
            local_apic_address: u32,
            flags: u32,
        }

        #[repr(C, packed)]
        struct EntryHeader {
            entry_type: u8,
            length: u8,
        }

        #[repr(C, packed)]
        struct IsoEntryRaw {
            header: EntryHeader,
            bus_source: u8,
            irq_source: u8,
            gsi: u32,
            flags: u16,
        }

        let madt_header = &*(madt_virt.as_ptr::<MadtHeader>());
        let mut offset = core::mem::size_of::<MadtHeader>();
        let mut iso_index = 0usize;

        while offset < madt_header.header.length as usize && iso_index < 16 {
            let entry_ptr = (madt_virt + offset as u64).as_ptr::<EntryHeader>();
            let entry = &*entry_ptr;

            // Type 2 = Interrupt Source Override
            if entry.entry_type == 2 {
                let iso_ptr = entry_ptr as *const IsoEntryRaw;
                // Read values using ptr::read_unaligned to avoid unaligned reference issues
                let bus_source = core::ptr::read_unaligned(core::ptr::addr_of!((*iso_ptr).bus_source));
                let irq_source = core::ptr::read_unaligned(core::ptr::addr_of!((*iso_ptr).irq_source));
                let gsi = core::ptr::read_unaligned(core::ptr::addr_of!((*iso_ptr).gsi));
                let flags = core::ptr::read_unaligned(core::ptr::addr_of!((*iso_ptr).flags));
                ISO_ENTRIES[iso_index] = Some(IsoEntry {
                    bus_source,
                    irq_source,
                    gsi,
                    flags,
                });
                crate::kprintln!("apic: ISO: IRQ{} -> GSI{} (flags={:#x})",
                                 irq_source, gsi, flags);
                iso_index += 1;
            }

            offset += entry.length as usize;
            if entry.length == 0 {
                break;
            }
        }

        ISO_COUNT.store(iso_index as u32, Ordering::Relaxed);
    }
}

/// Inicializa o APIC (Local + I/O) - versão simplificada sem MADT
pub fn init() -> bool {
    init_with_madt()
}

/// Verifica se o APIC está habilitado
pub fn is_enabled() -> bool {
    APIC_ENABLED.load(Ordering::Relaxed)
}

/// Envia EOI para o Local APIC
pub fn eoi() {
    if APIC_ENABLED.load(Ordering::Relaxed) {
        unsafe {
            lapic_write(LAPIC_EOI, 0);
        }
    }
}

/// Obtém o ID do APIC local
pub fn lapic_id() -> u32 {
    unsafe {
        (lapic_read(LAPIC_ID) >> 24) & 0xFF
    }
}

/// Envia IPI (Inter-Processor Interrupt) para outro CPU
pub fn send_ipi(destination: u8, vector: u8) {
    unsafe {
        // Wait for ICR to be ready
        while (lapic_read(LAPIC_ICR_LO) & (1 << 12)) != 0 {
            core::hint::spin_loop();
        }

        // Write destination
        lapic_write(LAPIC_ICR_HI, (destination as u32) << 24);

        // Write vector and send (delivery mode = fixed)
        lapic_write(LAPIC_ICR_LO, vector as u32);
    }
}

/// Envia IPI broadcast para todos os CPUs exceto o atual
pub fn send_ipi_all_excluding_self(vector: u8) {
    unsafe {
        while (lapic_read(LAPIC_ICR_LO) & (1 << 12)) != 0 {
            core::hint::spin_loop();
        }

        // Shorthand = 11 (all excluding self), delivery mode = fixed
        lapic_write(LAPIC_ICR_LO, (0b11 << 18) | vector as u32);
    }
}

/// Envia INIT IPI para um CPU
pub fn send_init_ipi(destination: u8) {
    unsafe {
        while (lapic_read(LAPIC_ICR_LO) & (1 << 12)) != 0 {
            core::hint::spin_loop();
        }

        lapic_write(LAPIC_ICR_HI, (destination as u32) << 24);
        // Level = 1, trigger = edge, delivery mode = INIT (101)
        lapic_write(LAPIC_ICR_LO, (1 << 14) | (0b101 << 8));
    }
}

/// Envia SIPI (Startup IPI) para um CPU
pub fn send_sipi(destination: u8, vector: u8) {
    unsafe {
        while (lapic_read(LAPIC_ICR_LO) & (1 << 12)) != 0 {
            core::hint::spin_loop();
        }

        lapic_write(LAPIC_ICR_HI, (destination as u32) << 24);
        // Delivery mode = SIPI (110), vector = page number of startup code
        lapic_write(LAPIC_ICR_LO, (0b110 << 8) | vector as u32);
    }
}

/// Mask/unmask an IRQ in the I/O APIC
pub fn set_irq_mask(irq: u8, masked: bool) {
    let (gsi, level, low) = get_irq_mapping(irq);
    let vector = 32 + irq;  // Simple mapping

    unsafe {
        ioapic_set_irq_full(gsi as u8, vector, 0, masked, level, low);
    }
}

/// Get the number of ticks per millisecond for the LAPIC timer
pub fn get_ticks_per_ms() -> u32 {
    LAPIC_TICKS_PER_MS.load(Ordering::Relaxed)
}

/// Send NMI to all CPUs except self (used for panic notification)
pub fn send_nmi_all_excluding_self() {
    unsafe {
        // Wait for ICR to be ready
        while (lapic_read(LAPIC_ICR_LO) & (1 << 12)) != 0 {
            core::hint::spin_loop();
        }

        // ICR format for NMI:
        // - Delivery Mode: 4 (NMI) - bits 10:8
        // - Level: 1 (Assert) - bit 14
        // - Destination Shorthand: 3 (All Excluding Self) - bits 19:18
        let icr_low = (4 << 8)    // Delivery mode = NMI
                    | (1 << 14)   // Level = Assert
                    | (0b11 << 18);  // Shorthand = All excluding self

        lapic_write(LAPIC_ICR_LO, icr_low);
    }
}

/// Send raw IPI with custom ICR value
pub fn send_ipi_raw(destination: u8, icr_low: u32) {
    unsafe {
        // Wait for ICR to be ready
        while (lapic_read(LAPIC_ICR_LO) & (1 << 12)) != 0 {
            core::hint::spin_loop();
        }

        // Write destination if needed
        if destination != 0 {
            lapic_write(LAPIC_ICR_HI, (destination as u32) << 24);
        }

        // Write ICR low and send
        lapic_write(LAPIC_ICR_LO, icr_low);
    }
}

/// Sleep for a given number of microseconds using LAPIC timer
pub fn delay_us(us: u32) {
    let ticks_per_ms = LAPIC_TICKS_PER_MS.load(Ordering::Relaxed);
    if ticks_per_ms == 0 {
        // Fallback to busy loop if timer not calibrated
        for _ in 0..us * 10 {
            core::hint::spin_loop();
        }
        return;
    }

    let ticks = (ticks_per_ms as u64 * us as u64) / 1000;

    unsafe {
        // Save current timer config
        let old_lvt = lapic_read(LAPIC_LVT_TIMER);

        // Mask timer and set one-shot mode
        lapic_write(LAPIC_LVT_TIMER, 1 << 16);
        lapic_write(LAPIC_TIMER_DCR, 0x03); // Divide by 16

        // Start countdown
        lapic_write(LAPIC_TIMER_ICR, ticks as u32);

        // Wait for countdown to finish
        while lapic_read(LAPIC_TIMER_CCR) > 0 {
            core::hint::spin_loop();
        }

        // Restore timer config
        lapic_write(LAPIC_LVT_TIMER, old_lvt);
    }
}

// ============================================================================
// Interrupt Affinity
// ============================================================================

/// Maximum number of IRQs we track
pub const MAX_IRQS: usize = 24;

/// Maximum number of CPUs for affinity
pub const MAX_AFFINITY_CPUS: usize = 256;

/// IRQ affinity entry
#[derive(Debug, Clone, Copy)]
pub struct IrqAffinity {
    /// IRQ number
    pub irq: u8,
    /// Target CPU (APIC ID)
    pub cpu: u8,
    /// Is the IRQ active/enabled
    pub enabled: bool,
    /// Total number of interrupts handled
    pub count: u64,
}

impl IrqAffinity {
    const fn new() -> Self {
        Self {
            irq: 0,
            cpu: 0,
            enabled: false,
            count: 0,
        }
    }
}

/// IRQ affinity table
static mut IRQ_AFFINITY: [IrqAffinity; MAX_IRQS] = [IrqAffinity::new(); MAX_IRQS];

/// Track IRQ counts per CPU for balancing
static mut IRQ_COUNT_PER_CPU: [AtomicU64; MAX_AFFINITY_CPUS] = {
    const ZERO: AtomicU64 = AtomicU64::new(0);
    [ZERO; MAX_AFFINITY_CPUS]
};

/// Number of online CPUs
static NR_CPUS: AtomicU32 = AtomicU32::new(1);

/// Initialize interrupt affinity tracking
pub fn init_irq_affinity(nr_cpus: u32) {
    NR_CPUS.store(nr_cpus, Ordering::Relaxed);

    unsafe {
        // Initialize IRQ affinity table - all IRQs to CPU 0 (BSP)
        for i in 0..MAX_IRQS {
            IRQ_AFFINITY[i] = IrqAffinity {
                irq: i as u8,
                cpu: 0,
                enabled: false,
                count: 0,
            };
        }
    }

    crate::kprintln!("apic: interrupt affinity initialized for {} CPUs", nr_cpus);
}

/// Set the CPU affinity for an IRQ
///
/// This routes the IRQ to the specified CPU.
/// Returns true if successful, false if IRQ is out of range.
pub fn set_irq_affinity(irq: u8, cpu: u8) -> bool {
    if irq as usize >= MAX_IRQS {
        return false;
    }

    let nr_cpus = NR_CPUS.load(Ordering::Relaxed);
    if cpu as u32 >= nr_cpus {
        return false;
    }

    unsafe {
        // Update tracking
        IRQ_AFFINITY[irq as usize].cpu = cpu;

        // Update I/O APIC redirection entry
        let entry = &IRQ_AFFINITY[irq as usize];
        if entry.enabled {
            // Reroute the IRQ to the new CPU
            let vector = APIC_TIMER_VECTOR + irq;
            let (gsi, level, low) = get_irq_mapping(irq);
            ioapic_set_irq_full(gsi as u8, vector, cpu, false, level, low);
        }
    }

    true
}

/// Get the CPU affinity for an IRQ
pub fn get_irq_affinity(irq: u8) -> Option<u8> {
    if irq as usize >= MAX_IRQS {
        return None;
    }

    unsafe { Some(IRQ_AFFINITY[irq as usize].cpu) }
}

/// Enable an IRQ and set its affinity
pub fn enable_irq_with_affinity(irq: u8, cpu: u8) {
    if irq as usize >= MAX_IRQS {
        return;
    }

    unsafe {
        IRQ_AFFINITY[irq as usize].cpu = cpu;
        IRQ_AFFINITY[irq as usize].enabled = true;

        // Enable in I/O APIC
        let vector = APIC_TIMER_VECTOR + irq;
        ioapic_set_irq(irq, vector, cpu, false); // false = unmask
    }
}

/// Disable an IRQ
pub fn disable_irq(irq: u8) {
    if irq as usize >= MAX_IRQS {
        return;
    }

    unsafe {
        IRQ_AFFINITY[irq as usize].enabled = false;

        // Mask in I/O APIC
        let vector = APIC_TIMER_VECTOR + irq;
        ioapic_set_irq(irq, vector, 0, true); // true = mask
    }
}

/// Record an interrupt was handled (for balancing)
pub fn record_irq_handled(irq: u8) {
    if irq as usize >= MAX_IRQS {
        return;
    }

    unsafe {
        let entry = &mut IRQ_AFFINITY[irq as usize];
        entry.count += 1;

        let cpu = entry.cpu as usize;
        if cpu < MAX_AFFINITY_CPUS {
            IRQ_COUNT_PER_CPU[cpu].fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Get IRQ count for a specific CPU
pub fn get_cpu_irq_count(cpu: u8) -> u64 {
    if cpu as usize >= MAX_AFFINITY_CPUS {
        return 0;
    }

    unsafe { IRQ_COUNT_PER_CPU[cpu as usize].load(Ordering::Relaxed) }
}

/// Get total IRQ count for a specific IRQ
pub fn get_irq_count(irq: u8) -> u64 {
    if irq as usize >= MAX_IRQS {
        return 0;
    }

    unsafe { IRQ_AFFINITY[irq as usize].count }
}

/// Balance IRQs across CPUs
///
/// Simple round-robin balancing of enabled IRQs.
pub fn balance_irqs() {
    let nr_cpus = NR_CPUS.load(Ordering::Relaxed);
    if nr_cpus <= 1 {
        return;
    }

    let mut next_cpu: u8 = 0;

    unsafe {
        for irq in 0..MAX_IRQS {
            let entry = &mut IRQ_AFFINITY[irq];
            if entry.enabled {
                set_irq_affinity(irq as u8, next_cpu);
                next_cpu = ((next_cpu as u32 + 1) % nr_cpus) as u8;
            }
        }
    }

    crate::kprintln!("apic: IRQs balanced across {} CPUs", nr_cpus);
}

/// Balance IRQs based on load
///
/// Moves IRQs from heavily loaded CPUs to less loaded ones.
pub fn balance_irqs_by_load() {
    let nr_cpus = NR_CPUS.load(Ordering::Relaxed) as usize;
    if nr_cpus <= 1 {
        return;
    }

    unsafe {
        // Find CPU with most IRQ counts
        let mut max_cpu = 0;
        let mut max_count = 0u64;
        let mut min_cpu = 0;
        let mut min_count = u64::MAX;

        for cpu in 0..nr_cpus {
            let count = IRQ_COUNT_PER_CPU[cpu].load(Ordering::Relaxed);
            if count > max_count {
                max_count = count;
                max_cpu = cpu;
            }
            if count < min_count {
                min_count = count;
                min_cpu = cpu;
            }
        }

        // If there's significant imbalance, move some IRQs
        if max_count > min_count * 2 && max_cpu != min_cpu {
            // Find an IRQ on the max CPU and move it to min CPU
            for irq in 0..MAX_IRQS {
                let entry = &IRQ_AFFINITY[irq];
                if entry.enabled && entry.cpu as usize == max_cpu {
                    set_irq_affinity(irq as u8, min_cpu as u8);
                    crate::kprintln!("apic: moved IRQ {} from CPU {} to CPU {}", irq, max_cpu, min_cpu);
                    break;
                }
            }
        }
    }
}

/// Get IRQ affinity summary for /proc/interrupts
pub fn get_irq_summary() -> [(u8, u8, bool, u64); MAX_IRQS] {
    let mut result = [(0u8, 0u8, false, 0u64); MAX_IRQS];

    unsafe {
        for (i, entry) in IRQ_AFFINITY.iter().enumerate() {
            result[i] = (entry.irq, entry.cpu, entry.enabled, entry.count);
        }
    }

    result
}
