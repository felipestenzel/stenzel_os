//! ACPI (Advanced Configuration and Power Interface) detection.
//!
//! Localiza e parse tabelas ACPI para descobrir informações de hardware:
//! - MADT: APIC/IOAPIC configuration
//! - FACP: Power management
//! - HPET: High Precision Event Timer
//! - MCFG: PCIe configuration
//! - DSDT/SSDT: Device definitions (AML parsing)

#![allow(dead_code)]

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use spin::Once;

/// Signature do RSDP: "RSD PTR "
const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";

/// RSDP (Root System Description Pointer) v1.0
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RsdpV1 {
    pub signature: [u8; 8],
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8,
    pub rsdt_address: u32,
}

/// RSDP v2.0 (extensão)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RsdpV2 {
    pub v1: RsdpV1,
    pub length: u32,
    pub xsdt_address: u64,
    pub extended_checksum: u8,
    pub reserved: [u8; 3],
}

/// Header comum de todas as tabelas ACPI
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct AcpiTableHeader {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}

/// Informação sobre uma tabela ACPI encontrada
#[derive(Debug, Clone)]
pub struct AcpiTable {
    pub signature: [u8; 4],
    pub address: u64,
    pub length: u32,
}

/// Estado global do ACPI
pub struct AcpiState {
    pub rsdp_address: u64,
    pub revision: u8,
    pub rsdt_address: Option<u32>,
    pub xsdt_address: Option<u64>,
    pub tables: Vec<AcpiTable>,
}

static ACPI_STATE: Once<AcpiState> = Once::new();

/// Inicializa ACPI: procura RSDP e enumera tabelas.
pub fn init() {
    crate::kprintln!("acpi: procurando RSDP...");

    let rsdp = match find_rsdp() {
        Some(r) => r,
        None => {
            crate::kprintln!("acpi: RSDP não encontrado");
            return;
        }
    };

    crate::kprintln!("acpi: RSDP encontrado @ {:#x}, revision={}", rsdp.0, rsdp.1);

    let state = parse_rsdp(rsdp.0, rsdp.1);

    crate::kprintln!("acpi: {} tabelas encontradas:", state.tables.len());
    for table in &state.tables {
        let sig_str = core::str::from_utf8(&table.signature).unwrap_or("????");
        crate::kprintln!("  {} @ {:#x} (len={})", sig_str, table.address, table.length);
    }

    ACPI_STATE.call_once(|| state);
}

/// Retorna o estado do ACPI (se inicializado).
pub fn get_state() -> Option<&'static AcpiState> {
    ACPI_STATE.get()
}

/// Procura uma tabela ACPI por signature.
pub fn find_table(signature: &[u8; 4]) -> Option<&'static AcpiTable> {
    ACPI_STATE.get()?.tables.iter().find(|t| &t.signature == signature)
}

/// Procura o RSDP na memória.
///
/// Retorna (endereço físico, revisão) se encontrado.
fn find_rsdp() -> Option<(u64, u8)> {
    // O RSDP pode estar em:
    // 1. EBDA (Extended BIOS Data Area) - ponteiro em 0x40E
    // 2. Área de memória 0xE0000 - 0xFFFFF

    let phys_offset = crate::mm::physical_memory_offset();

    // Primeiro, tenta a área de memória padrão 0xE0000 - 0xFFFFF
    for addr in (0xE0000..0x100000).step_by(16) {
        let virt = phys_offset + addr;
        let ptr = virt.as_ptr::<[u8; 8]>();

        unsafe {
            if *ptr == *RSDP_SIGNATURE {
                // Verifica checksum
                let rsdp_ptr = virt.as_ptr::<RsdpV1>();
                let rsdp = &*rsdp_ptr;

                if validate_checksum(virt.as_ptr::<u8>(), 20) {
                    return Some((addr, rsdp.revision));
                }
            }
        }
    }

    // Tenta EBDA
    unsafe {
        let ebda_ptr_virt = phys_offset + 0x40E;
        let ebda_segment = *(ebda_ptr_virt.as_ptr::<u16>()) as u64;
        let ebda_base = ebda_segment << 4;

        // Procura nas primeiras 1KB do EBDA
        if ebda_base != 0 && ebda_base < 0xA0000 {
            for offset in (0..1024).step_by(16) {
                let addr = ebda_base + offset;
                let virt = phys_offset + addr;
                let ptr = virt.as_ptr::<[u8; 8]>();

                if *ptr == *RSDP_SIGNATURE {
                    let rsdp_ptr = virt.as_ptr::<RsdpV1>();
                    let rsdp = &*rsdp_ptr;

                    if validate_checksum(virt.as_ptr::<u8>(), 20) {
                        return Some((addr, rsdp.revision));
                    }
                }
            }
        }
    }

    None
}

/// Valida checksum de uma estrutura ACPI.
fn validate_checksum(ptr: *const u8, len: usize) -> bool {
    let mut sum: u8 = 0;
    for i in 0..len {
        sum = sum.wrapping_add(unsafe { *ptr.add(i) });
    }
    sum == 0
}

/// Parse RSDP e enumera tabelas.
fn parse_rsdp(rsdp_phys: u64, revision: u8) -> AcpiState {
    let phys_offset = crate::mm::physical_memory_offset();
    let rsdp_virt = phys_offset + rsdp_phys;

    let mut state = AcpiState {
        rsdp_address: rsdp_phys,
        revision,
        rsdt_address: None,
        xsdt_address: None,
        tables: Vec::new(),
    };

    unsafe {
        if revision >= 2 {
            // ACPI 2.0+: usa XSDT
            let rsdp2 = &*(rsdp_virt.as_ptr::<RsdpV2>());
            state.xsdt_address = Some(rsdp2.xsdt_address);
            state.rsdt_address = Some(rsdp2.v1.rsdt_address);

            // Parse XSDT (entradas de 64 bits)
            parse_xsdt(rsdp2.xsdt_address, &mut state.tables);
        } else {
            // ACPI 1.0: usa RSDT
            let rsdp1 = &*(rsdp_virt.as_ptr::<RsdpV1>());
            state.rsdt_address = Some(rsdp1.rsdt_address);

            // Parse RSDT (entradas de 32 bits)
            parse_rsdt(rsdp1.rsdt_address as u64, &mut state.tables);
        }
    }

    state
}

/// Parse RSDT (32-bit entries).
fn parse_rsdt(rsdt_phys: u64, tables: &mut Vec<AcpiTable>) {
    let phys_offset = crate::mm::physical_memory_offset();
    let rsdt_virt = phys_offset + rsdt_phys;

    unsafe {
        let header = &*(rsdt_virt.as_ptr::<AcpiTableHeader>());
        let entries_start = rsdt_virt + core::mem::size_of::<AcpiTableHeader>() as u64;
        let entry_count = (header.length as usize - core::mem::size_of::<AcpiTableHeader>()) / 4;

        for i in 0..entry_count {
            let entry_ptr = (entries_start + (i * 4) as u64).as_ptr::<u32>();
            let table_phys = *entry_ptr as u64;

            if table_phys == 0 {
                continue;
            }

            let table_virt = phys_offset + table_phys;
            let table_header = &*(table_virt.as_ptr::<AcpiTableHeader>());

            tables.push(AcpiTable {
                signature: table_header.signature,
                address: table_phys,
                length: table_header.length,
            });
        }
    }
}

/// Parse XSDT (64-bit entries).
fn parse_xsdt(xsdt_phys: u64, tables: &mut Vec<AcpiTable>) {
    let phys_offset = crate::mm::physical_memory_offset();
    let xsdt_virt = phys_offset + xsdt_phys;

    unsafe {
        let header = &*(xsdt_virt.as_ptr::<AcpiTableHeader>());
        let entries_start = xsdt_virt + core::mem::size_of::<AcpiTableHeader>() as u64;
        let entry_count = (header.length as usize - core::mem::size_of::<AcpiTableHeader>()) / 8;

        for i in 0..entry_count {
            let entry_ptr = (entries_start + (i * 8) as u64).as_ptr::<u64>();
            let table_phys = *entry_ptr;

            if table_phys == 0 {
                continue;
            }

            let table_virt = phys_offset + table_phys;
            let table_header = &*(table_virt.as_ptr::<AcpiTableHeader>());

            tables.push(AcpiTable {
                signature: table_header.signature,
                address: table_phys,
                length: table_header.length,
            });
        }
    }
}

// ==================== MADT (Multiple APIC Description Table) ====================

/// MADT entry types
pub mod madt_entry_type {
    pub const LOCAL_APIC: u8 = 0;
    pub const IO_APIC: u8 = 1;
    pub const INTERRUPT_SOURCE_OVERRIDE: u8 = 2;
    pub const NMI_SOURCE: u8 = 3;
    pub const LOCAL_APIC_NMI: u8 = 4;
    pub const LOCAL_APIC_ADDRESS_OVERRIDE: u8 = 5;
    pub const IO_SAPIC: u8 = 6;
    pub const LOCAL_SAPIC: u8 = 7;
    pub const PLATFORM_INTERRUPT_SOURCES: u8 = 8;
    pub const LOCAL_X2APIC: u8 = 9;
    pub const LOCAL_X2APIC_NMI: u8 = 10;
    pub const GIC_CPU_INTERFACE: u8 = 11;
}

/// MADT header
#[repr(C, packed)]
pub struct MadtHeader {
    pub header: AcpiTableHeader,
    pub local_apic_address: u32,
    pub flags: u32,
}

/// MADT entry header
#[repr(C, packed)]
pub struct MadtEntryHeader {
    pub entry_type: u8,
    pub length: u8,
}

/// Local APIC entry
#[repr(C, packed)]
pub struct MadtLocalApic {
    pub header: MadtEntryHeader,
    pub acpi_processor_id: u8,
    pub apic_id: u8,
    pub flags: u32,
}

/// I/O APIC entry
#[repr(C, packed)]
pub struct MadtIoApic {
    pub header: MadtEntryHeader,
    pub io_apic_id: u8,
    pub reserved: u8,
    pub io_apic_address: u32,
    pub global_system_interrupt_base: u32,
}

/// Parse MADT e retorna informações sobre APICs.
pub fn parse_madt() -> Option<MadtInfo> {
    let madt = find_table(b"APIC")?;
    let phys_offset = crate::mm::physical_memory_offset();
    let madt_virt = phys_offset + madt.address;

    let mut info = MadtInfo {
        local_apic_address: 0,
        local_apics: Vec::new(),
        io_apics: Vec::new(),
    };

    unsafe {
        let madt_header = &*(madt_virt.as_ptr::<MadtHeader>());
        info.local_apic_address = madt_header.local_apic_address;

        // Itera pelas entradas
        let mut offset = core::mem::size_of::<MadtHeader>();
        while offset < madt_header.header.length as usize {
            let entry_ptr = (madt_virt + offset as u64).as_ptr::<MadtEntryHeader>();
            let entry = &*entry_ptr;

            match entry.entry_type {
                madt_entry_type::LOCAL_APIC => {
                    let lapic = &*(entry_ptr as *const MadtLocalApic);
                    if lapic.flags & 1 != 0 || lapic.flags & 2 != 0 {
                        info.local_apics.push(LocalApicInfo {
                            processor_id: lapic.acpi_processor_id,
                            apic_id: lapic.apic_id,
                        });
                    }
                }
                madt_entry_type::IO_APIC => {
                    let ioapic = &*(entry_ptr as *const MadtIoApic);
                    info.io_apics.push(IoApicInfo {
                        id: ioapic.io_apic_id,
                        address: ioapic.io_apic_address,
                        gsi_base: ioapic.global_system_interrupt_base,
                    });
                }
                _ => {}
            }

            offset += entry.length as usize;
            if entry.length == 0 {
                break;
            }
        }
    }

    Some(info)
}

/// Informações do MADT
#[derive(Debug)]
pub struct MadtInfo {
    pub local_apic_address: u32,
    pub local_apics: Vec<LocalApicInfo>,
    pub io_apics: Vec<IoApicInfo>,
}

#[derive(Debug)]
pub struct LocalApicInfo {
    pub processor_id: u8,
    pub apic_id: u8,
}

#[derive(Debug)]
pub struct IoApicInfo {
    pub id: u8,
    pub address: u32,
    pub gsi_base: u32,
}

// ==================== FADT (Fixed ACPI Description Table) ====================

/// FADT/FACP structure (partial - just what we need for shutdown)
#[repr(C, packed)]
pub struct Fadt {
    pub header: AcpiTableHeader,
    pub firmware_ctrl: u32,
    pub dsdt: u32,
    pub reserved: u8,
    pub preferred_pm_profile: u8,
    pub sci_int: u16,
    pub smi_cmd: u32,
    pub acpi_enable: u8,
    pub acpi_disable: u8,
    pub s4bios_req: u8,
    pub pstate_cnt: u8,
    pub pm1a_evt_blk: u32,
    pub pm1b_evt_blk: u32,
    pub pm1a_cnt_blk: u32,
    pub pm1b_cnt_blk: u32,
    pub pm2_cnt_blk: u32,
    pub pm_tmr_blk: u32,
    pub gpe0_blk: u32,
    pub gpe1_blk: u32,
    pub pm1_evt_len: u8,
    pub pm1_cnt_len: u8,
    pub pm2_cnt_len: u8,
    pub pm_tmr_len: u8,
    pub gpe0_blk_len: u8,
    pub gpe1_blk_len: u8,
    pub gpe1_base: u8,
    pub cst_cnt: u8,
    pub p_lvl2_lat: u16,
    pub p_lvl3_lat: u16,
    pub flush_size: u16,
    pub flush_stride: u16,
    pub duty_offset: u8,
    pub duty_width: u8,
    pub day_alrm: u8,
    pub mon_alrm: u8,
    pub century: u8,
    pub iapc_boot_arch: u16,
    pub reserved2: u8,
    pub flags: u32,
    pub reset_reg: GenericAddressStructure,
    pub reset_value: u8,
    // ... more fields follow but we don't need them
}

/// Generic Address Structure
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct GenericAddressStructure {
    pub address_space: u8,
    pub bit_width: u8,
    pub bit_offset: u8,
    pub access_size: u8,
    pub address: u64,
}

/// Parse FADT and extract power management info
pub fn parse_fadt() -> Option<FadtInfo> {
    let fadt = find_table(b"FACP")?;
    let phys_offset = crate::mm::physical_memory_offset();
    let fadt_virt = phys_offset + fadt.address;

    unsafe {
        let fadt_data = &*(fadt_virt.as_ptr::<Fadt>());

        Some(FadtInfo {
            pm1a_cnt_blk: fadt_data.pm1a_cnt_blk,
            pm1b_cnt_blk: fadt_data.pm1b_cnt_blk,
            pm1a_evt_blk: fadt_data.pm1a_evt_blk,
            pm1b_evt_blk: fadt_data.pm1b_evt_blk,
            pm1_evt_len: fadt_data.pm1_evt_len,
            pm1_cnt_len: fadt_data.pm1_cnt_len,
            smi_cmd: fadt_data.smi_cmd,
            acpi_enable: fadt_data.acpi_enable,
            acpi_disable: fadt_data.acpi_disable,
            sci_int: fadt_data.sci_int,
            reset_reg: fadt_data.reset_reg,
            reset_value: fadt_data.reset_value,
        })
    }
}

/// FADT information needed for power management
#[derive(Debug, Clone)]
pub struct FadtInfo {
    pub pm1a_cnt_blk: u32,
    pub pm1b_cnt_blk: u32,
    pub pm1a_evt_blk: u32,
    pub pm1b_evt_blk: u32,
    pub pm1_evt_len: u8,
    pub pm1_cnt_len: u8,
    pub smi_cmd: u32,
    pub acpi_enable: u8,
    pub acpi_disable: u8,
    pub sci_int: u16,
    pub reset_reg: GenericAddressStructure,
    pub reset_value: u8,
}

// ==================== ACPI Power States ====================

/// ACPI Sleep State (S-state)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SleepState {
    S0 = 0, // Working state
    S1 = 1, // Power on suspend (CPU stops, RAM preserved)
    S2 = 2, // CPU off (CPU context lost)
    S3 = 3, // Suspend to RAM (STR/Sleep)
    S4 = 4, // Suspend to disk (Hibernate)
    S5 = 5, // Soft off
}

impl SleepState {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::S0),
            1 => Some(Self::S1),
            2 => Some(Self::S2),
            3 => Some(Self::S3),
            4 => Some(Self::S4),
            5 => Some(Self::S5),
            _ => None,
        }
    }
}

/// Sleep state type values for PM1 control register
#[derive(Debug, Clone, Copy, Default)]
pub struct SleepTypeValue {
    pub slp_typ_a: u8, // SLP_TYP value for PM1a_CNT
    pub slp_typ_b: u8, // SLP_TYP value for PM1b_CNT
}

/// Power management state information
#[derive(Debug, Clone)]
pub struct PowerStateInfo {
    pub s0: Option<SleepTypeValue>,
    pub s1: Option<SleepTypeValue>,
    pub s2: Option<SleepTypeValue>,
    pub s3: Option<SleepTypeValue>,
    pub s4: Option<SleepTypeValue>,
    pub s5: Option<SleepTypeValue>,
}

impl PowerStateInfo {
    pub fn new() -> Self {
        Self {
            s0: None,
            s1: None,
            s2: None,
            s3: None,
            s4: None,
            s5: None,
        }
    }

    /// Get SLP_TYP values for a given sleep state
    pub fn get_slp_typ(&self, state: SleepState) -> Option<SleepTypeValue> {
        match state {
            SleepState::S0 => self.s0,
            SleepState::S1 => self.s1,
            SleepState::S2 => self.s2,
            SleepState::S3 => self.s3,
            SleepState::S4 => self.s4,
            SleepState::S5 => self.s5,
        }
    }

    /// Check if a sleep state is supported
    pub fn is_supported(&self, state: SleepState) -> bool {
        self.get_slp_typ(state).is_some()
    }
}

impl Default for PowerStateInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Global power state information (initialized from DSDT)
static POWER_STATE_INFO: crate::sync::IrqSafeMutex<Option<PowerStateInfo>> =
    crate::sync::IrqSafeMutex::new(None);

/// Initialize power state info from DSDT
pub fn init_power_states() {
    // Try to get default values based on common QEMU/Bochs/real hardware values
    let mut info = PowerStateInfo::new();

    // Default S5 (soft off) - most common value is 5
    info.s5 = Some(SleepTypeValue { slp_typ_a: 5, slp_typ_b: 5 });

    // S0 (working) - typically 0
    info.s0 = Some(SleepTypeValue { slp_typ_a: 0, slp_typ_b: 0 });

    // S3 (suspend to RAM) - typically 1 or 5 depending on firmware
    info.s3 = Some(SleepTypeValue { slp_typ_a: 1, slp_typ_b: 1 });

    // S1 (power on suspend) - typically 1
    info.s1 = Some(SleepTypeValue { slp_typ_a: 1, slp_typ_b: 1 });

    // TODO: Parse actual values from DSDT _S0-_S5 methods
    // For now use common defaults that work with QEMU

    *POWER_STATE_INFO.lock() = Some(info);
    crate::kprintln!("acpi: power state info initialized with defaults");
}

/// Get current power state information
pub fn power_state_info() -> Option<PowerStateInfo> {
    POWER_STATE_INFO.lock().clone()
}

/// PM1 Control Register bits
pub mod pm1_cnt {
    pub const SCI_EN: u16 = 1 << 0;      // SCI interrupt enable
    pub const BM_RLD: u16 = 1 << 1;      // Bus master reload
    pub const GBL_RLS: u16 = 1 << 2;     // Global release
    pub const SLP_TYP_SHIFT: u16 = 10;   // Sleep type (bits 10-12)
    pub const SLP_TYP_MASK: u16 = 0x7 << 10;
    pub const SLP_EN: u16 = 1 << 13;     // Sleep enable
}

/// PM1 Status Register bits
pub mod pm1_sts {
    pub const TMR_STS: u16 = 1 << 0;     // Timer status
    pub const BM_STS: u16 = 1 << 4;      // Bus master status
    pub const GBL_STS: u16 = 1 << 5;     // Global status
    pub const PWRBTN_STS: u16 = 1 << 8;  // Power button status
    pub const SLPBTN_STS: u16 = 1 << 9;  // Sleep button status
    pub const RTC_STS: u16 = 1 << 10;    // RTC status
    pub const PCIEXP_WAKE_STS: u16 = 1 << 14; // PCIe wake status
    pub const WAK_STS: u16 = 1 << 15;    // Wake status
}

/// PM1 Enable Register bits
pub mod pm1_en {
    pub const TMR_EN: u16 = 1 << 0;      // Timer enable
    pub const GBL_EN: u16 = 1 << 5;      // Global enable
    pub const PWRBTN_EN: u16 = 1 << 8;   // Power button enable
    pub const SLPBTN_EN: u16 = 1 << 9;   // Sleep button enable
    pub const RTC_EN: u16 = 1 << 10;     // RTC enable
    pub const PCIEXP_WAKE_DIS: u16 = 1 << 14; // PCIe wake disable
}

// ==================== Power Button Handling ====================

/// Power button event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerButtonEvent {
    Pressed,
    Released,
}

/// Power button action to take
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerButtonAction {
    Ignore,
    Shutdown,
    Suspend,
    Hibernate,
    AskUser,
}

/// Power button configuration
static POWER_BUTTON_ACTION: core::sync::atomic::AtomicU8 =
    core::sync::atomic::AtomicU8::new(PowerButtonAction::Shutdown as u8);

/// Set the action to take when power button is pressed
pub fn set_power_button_action(action: PowerButtonAction) {
    use core::sync::atomic::Ordering;
    POWER_BUTTON_ACTION.store(action as u8, Ordering::SeqCst);
}

/// Get the current power button action
pub fn get_power_button_action() -> PowerButtonAction {
    use core::sync::atomic::Ordering;
    match POWER_BUTTON_ACTION.load(Ordering::SeqCst) {
        0 => PowerButtonAction::Ignore,
        1 => PowerButtonAction::Shutdown,
        2 => PowerButtonAction::Suspend,
        3 => PowerButtonAction::Hibernate,
        4 => PowerButtonAction::AskUser,
        _ => PowerButtonAction::Shutdown,
    }
}

/// Enable power button events (SCI)
pub fn enable_power_button_event() {
    use x86_64::instructions::port::Port;

    if let Some(fadt) = parse_fadt() {
        // Enable power button event in PM1 enable register
        // PM1_EN is at PM1a_EVT_BLK + PM1_EVT_LEN/2
        let pm1_en_offset = (fadt.pm1_evt_len / 2) as u32;

        unsafe {
            if fadt.pm1a_evt_blk != 0 {
                let pm1a_en_addr = fadt.pm1a_evt_blk + pm1_en_offset;
                let mut port: Port<u16> = Port::new(pm1a_en_addr as u16);
                let current = port.read();
                port.write(current | pm1_en::PWRBTN_EN);
            }
            if fadt.pm1b_evt_blk != 0 {
                let pm1b_en_addr = fadt.pm1b_evt_blk + pm1_en_offset;
                let mut port: Port<u16> = Port::new(pm1b_en_addr as u16);
                let current = port.read();
                port.write(current | pm1_en::PWRBTN_EN);
            }
        }

        crate::kprintln!("acpi: power button events enabled");
    }
}

/// Check if power button was pressed and clear the status
pub fn check_power_button_pressed() -> bool {
    use x86_64::instructions::port::Port;

    if let Some(fadt) = parse_fadt() {
        unsafe {
            if fadt.pm1a_evt_blk != 0 {
                let mut port: Port<u16> = Port::new(fadt.pm1a_evt_blk as u16);
                let status = port.read();
                if status & pm1_sts::PWRBTN_STS != 0 {
                    // Clear the status by writing 1
                    port.write(pm1_sts::PWRBTN_STS);
                    return true;
                }
            }
            if fadt.pm1b_evt_blk != 0 {
                let mut port: Port<u16> = Port::new(fadt.pm1b_evt_blk as u16);
                let status = port.read();
                if status & pm1_sts::PWRBTN_STS != 0 {
                    port.write(pm1_sts::PWRBTN_STS);
                    return true;
                }
            }
        }
    }
    false
}

/// Handle power button event
pub fn handle_power_button() {
    if !check_power_button_pressed() {
        return;
    }

    crate::kprintln!("acpi: power button pressed!");

    match get_power_button_action() {
        PowerButtonAction::Ignore => {
            crate::kprintln!("acpi: power button ignored (configured)");
        }
        PowerButtonAction::Shutdown => {
            crate::kprintln!("acpi: initiating shutdown due to power button");
            // Sync filesystems first
            // TODO: crate::fs::sync_all();
            shutdown();
        }
        PowerButtonAction::Suspend => {
            crate::kprintln!("acpi: initiating suspend due to power button");
            let _ = suspend_to_ram();
        }
        PowerButtonAction::Hibernate => {
            crate::kprintln!("acpi: hibernate not supported, using suspend");
            let _ = suspend_to_ram();
        }
        PowerButtonAction::AskUser => {
            crate::kprintln!("acpi: power button action = ask user (sending signal)");
            // TODO: Send signal to desktop environment or init
            // For now, just log it
        }
    }
}

/// Check if sleep button was pressed and clear the status
pub fn check_sleep_button_pressed() -> bool {
    use x86_64::instructions::port::Port;

    if let Some(fadt) = parse_fadt() {
        unsafe {
            if fadt.pm1a_evt_blk != 0 {
                let mut port: Port<u16> = Port::new(fadt.pm1a_evt_blk as u16);
                let status = port.read();
                if status & pm1_sts::SLPBTN_STS != 0 {
                    port.write(pm1_sts::SLPBTN_STS);
                    return true;
                }
            }
            if fadt.pm1b_evt_blk != 0 {
                let mut port: Port<u16> = Port::new(fadt.pm1b_evt_blk as u16);
                let status = port.read();
                if status & pm1_sts::SLPBTN_STS != 0 {
                    port.write(pm1_sts::SLPBTN_STS);
                    return true;
                }
            }
        }
    }
    false
}

/// Handle sleep button event
pub fn handle_sleep_button() {
    if !check_sleep_button_pressed() {
        return;
    }

    crate::kprintln!("acpi: sleep button pressed, suspending...");
    let _ = suspend_to_ram();
}

/// Poll for ACPI events (power button, sleep button, etc.)
/// Should be called from a periodic timer or dedicated thread
pub fn poll_acpi_events() {
    handle_power_button();
    handle_sleep_button();
    handle_lid_switch();
}

// ==================== Lid Switch ====================

// Note: spin::Once already imported at top of file

/// Lid switch state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LidState {
    Open,
    Closed,
    Unknown,
}

/// Lid switch configuration and state
pub struct LidSwitch {
    /// Current state
    state: LidState,
    /// Previous state (for detecting changes)
    prev_state: LidState,
    /// Whether lid switch is detected
    present: bool,
    /// Action when lid is closed
    close_action: LidAction,
    /// Action when lid is opened
    open_action: LidAction,
    /// GPE number for lid events (if using GPE)
    gpe_number: Option<u8>,
    /// Whether lid wake is enabled
    wake_enabled: bool,
}

/// Action to take on lid events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LidAction {
    /// Do nothing
    None,
    /// Suspend to RAM
    Suspend,
    /// Lock screen (notify userspace)
    Lock,
    /// Hibernate
    Hibernate,
    /// Shutdown
    Shutdown,
}

impl LidAction {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "none" | "ignore" => Some(Self::None),
            "suspend" | "sleep" => Some(Self::Suspend),
            "lock" => Some(Self::Lock),
            "hibernate" => Some(Self::Hibernate),
            "shutdown" | "poweroff" => Some(Self::Shutdown),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Suspend => "suspend",
            Self::Lock => "lock",
            Self::Hibernate => "hibernate",
            Self::Shutdown => "shutdown",
        }
    }
}

static LID_SWITCH: spin::Mutex<Option<LidSwitch>> = spin::Mutex::new(None);

/// Initialize lid switch detection
pub fn init_lid_switch() {
    // Try to detect lid switch via ACPI
    let mut lid = LidSwitch {
        state: LidState::Unknown,
        prev_state: LidState::Unknown,
        present: false,
        close_action: LidAction::Suspend,
        open_action: LidAction::None,
        gpe_number: None,
        wake_enabled: true,
    };

    // Method 1: Check for _LID device in DSDT
    if let Some(dsdt) = DSDT_INFO.get() {
        for device in &dsdt.devices {
            // Look for PNP0C0D (ACPI lid device)
            if device.hid.as_deref() == Some("PNP0C0D") {
                lid.present = true;
                crate::kprintln!("acpi: lid switch found (PNP0C0D)");
                break;
            }
        }
    }

    // Method 2: Try reading from standard EC port
    if !lid.present {
        // Check for embedded controller which often reports lid state
        if check_ec_lid_support() {
            lid.present = true;
            crate::kprintln!("acpi: lid switch detected via EC");
        }
    }

    // Method 3: Try GPE-based lid (common on Intel laptops)
    if !lid.present {
        // GPE 0x17 is common for lid events
        if check_gpe_lid_support(0x17) {
            lid.present = true;
            lid.gpe_number = Some(0x17);
            crate::kprintln!("acpi: lid switch detected via GPE");
        }
    }

    if lid.present {
        // Get initial state
        lid.state = read_lid_state_internal();
        lid.prev_state = lid.state;
        crate::kprintln!("acpi: lid state: {:?}", lid.state);
    }

    *LID_SWITCH.lock() = Some(lid);
}

/// Check if EC reports lid state
fn check_ec_lid_support() -> bool {
    // EC (Embedded Controller) is at ports 0x62/0x66
    // This is simplified - real implementation would query EC properly
    false
}

/// Check if a GPE is available for lid events
fn check_gpe_lid_support(_gpe: u8) -> bool {
    // Check GPE enable registers
    // This is simplified
    false
}

/// Read current lid state from hardware
fn read_lid_state_internal() -> LidState {
    // Method 1: Try to evaluate _LID method
    // In a full ACPI implementation, we would evaluate the _LID method
    // For now, use simplified detection

    // Method 2: Read from EC
    if let Some(state) = read_ec_lid_state() {
        return state;
    }

    // Method 3: Check GPIO (platform-specific)
    // Many laptops have a GPIO pin for lid state

    LidState::Unknown
}

/// Read lid state from Embedded Controller
fn read_ec_lid_state() -> Option<LidState> {
    use x86_64::instructions::port::Port;

    // EC command port is 0x66, data port is 0x62
    const EC_SC: u16 = 0x66;  // Status/Command
    const EC_DATA: u16 = 0x62;

    // Check if EC is present and ready
    let status = unsafe {
        let mut port: Port<u8> = Port::new(EC_SC);
        port.read()
    };

    if status == 0xFF {
        return None; // EC not present
    }

    // Wait for input buffer empty
    for _ in 0..10000 {
        let s = unsafe {
            let mut port: Port<u8> = Port::new(EC_SC);
            port.read()
        };
        if s & 0x02 == 0 {
            break;
        }
    }

    // Send read command (0x80 = read)
    // Lid state is often at offset 0x03 or 0x10 depending on OEM
    // This is highly system-specific
    unsafe {
        let mut cmd_port: Port<u8> = Port::new(EC_SC);
        cmd_port.write(0x80); // RD_EC command

        // Wait for input buffer empty
        for _ in 0..10000 {
            let s: u8 = {
                let mut port: Port<u8> = Port::new(EC_SC);
                port.read()
            };
            if s & 0x02 == 0 {
                break;
            }
        }

        // Send address
        let mut data_port: Port<u8> = Port::new(EC_DATA);
        data_port.write(0x03); // Lid state offset (varies by system)

        // Wait for output buffer full
        for _ in 0..10000 {
            let s: u8 = {
                let mut port: Port<u8> = Port::new(EC_SC);
                port.read()
            };
            if s & 0x01 != 0 {
                break;
            }
        }

        // Read result
        let result = data_port.read();

        // Bit 0 is often lid state (0 = closed, 1 = open)
        if result & 0x01 != 0 {
            Some(LidState::Open)
        } else {
            Some(LidState::Closed)
        }
    }
}

/// Handle lid switch events
pub fn handle_lid_switch() {
    let mut lid_guard = LID_SWITCH.lock();
    let lid = match lid_guard.as_mut() {
        Some(l) if l.present => l,
        _ => return,
    };

    // Read current state
    let current = read_lid_state_internal();
    if current == LidState::Unknown {
        return;
    }

    // Check for state change
    if current != lid.prev_state {
        crate::kprintln!("acpi: lid state changed: {:?} -> {:?}", lid.prev_state, current);

        // Update state
        lid.prev_state = lid.state;
        lid.state = current;

        // Copy action before dropping the lock
        let action = if current == LidState::Closed {
            lid.close_action
        } else {
            lid.open_action
        };

        // Drop lock before performing action
        drop(lid_guard);

        // Perform action
        match action {
            LidAction::None => {}
            LidAction::Suspend => {
                crate::kprintln!("acpi: lid closed, suspending...");
                let _ = suspend_to_ram();
            }
            LidAction::Lock => {
                crate::kprintln!("acpi: lid closed, locking screen");
                // Notify userspace to lock screen
            }
            LidAction::Hibernate => {
                crate::kprintln!("acpi: lid closed, hibernating...");
                // Hibernate not implemented
            }
            LidAction::Shutdown => {
                crate::kprintln!("acpi: lid closed, shutting down...");
                let _ = shutdown();
            }
        }
    }
}

/// Get current lid state
pub fn get_lid_state() -> LidState {
    LID_SWITCH.lock()
        .as_ref()
        .map(|l| l.state)
        .unwrap_or(LidState::Unknown)
}

/// Check if lid switch is present
pub fn has_lid_switch() -> bool {
    LID_SWITCH.lock()
        .as_ref()
        .map(|l| l.present)
        .unwrap_or(false)
}

/// Set action for lid close
pub fn set_lid_close_action(action: LidAction) {
    if let Some(ref mut lid) = *LID_SWITCH.lock() {
        lid.close_action = action;
    }
}

/// Set action for lid open
pub fn set_lid_open_action(action: LidAction) {
    if let Some(ref mut lid) = *LID_SWITCH.lock() {
        lid.open_action = action;
    }
}

/// Enable/disable lid as wake source
pub fn set_lid_wake_enabled(enabled: bool) {
    if let Some(ref mut lid) = *LID_SWITCH.lock() {
        lid.wake_enabled = enabled;

        // Enable/disable GPE for lid if using GPE method
        if let Some(gpe) = lid.gpe_number {
            if enabled {
                enable_gpe_wake(gpe);
            } else {
                disable_gpe_wake(gpe);
            }
        }
    }
}

/// Enable GPE as wake source
fn enable_gpe_wake(_gpe: u8) {
    // Enable GPE in GPE enable register
    // This is platform-specific
}

/// Disable GPE as wake source
fn disable_gpe_wake(_gpe: u8) {
    // Disable GPE in GPE enable register
}

/// Get lid switch info for sysfs
pub fn sysfs_lid_state() -> String {
    match get_lid_state() {
        LidState::Open => String::from("open\n"),
        LidState::Closed => String::from("closed\n"),
        LidState::Unknown => String::from("unknown\n"),
    }
}

/// Get lid close action for sysfs
pub fn sysfs_lid_close_action() -> String {
    let action = LID_SWITCH.lock()
        .as_ref()
        .map(|l| l.close_action)
        .unwrap_or(LidAction::None);
    format!("{}\n", action.as_str())
}

/// Set lid close action from sysfs
pub fn sysfs_set_lid_close_action(s: &str) -> Result<(), &'static str> {
    let action = LidAction::from_str(s).ok_or("Invalid action")?;
    set_lid_close_action(action);
    Ok(())
}

/// Enter a specific ACPI sleep state
/// Returns Ok(()) for S1-S3 (wake-able states), or doesn't return for S4-S5
pub fn enter_sleep_state(state: SleepState) -> Result<(), &'static str> {
    use x86_64::instructions::port::Port;

    crate::kprintln!("acpi: entering sleep state S{}", state as u8);

    // Get FADT info
    let fadt = parse_fadt().ok_or("FADT not found")?;

    // Get sleep type values
    let psi = power_state_info().ok_or("Power state info not initialized")?;
    let slp_typ = psi.get_slp_typ(state).ok_or("Sleep state not supported")?;

    // Disable interrupts for the transition
    x86_64::instructions::interrupts::disable();

    // Clear wake status bits before sleeping
    if fadt.pm1a_evt_blk != 0 {
        unsafe {
            let mut port: Port<u16> = Port::new(fadt.pm1a_evt_blk as u16);
            port.write(pm1_sts::WAK_STS); // Write 1 to clear
        }
    }
    if fadt.pm1b_evt_blk != 0 {
        unsafe {
            let mut port: Port<u16> = Port::new(fadt.pm1b_evt_blk as u16);
            port.write(pm1_sts::WAK_STS);
        }
    }

    // Build PM1 control value: SLP_TYP | SLP_EN
    let pm1a_cnt_value = ((slp_typ.slp_typ_a as u16) << pm1_cnt::SLP_TYP_SHIFT) | pm1_cnt::SLP_EN;
    let pm1b_cnt_value = ((slp_typ.slp_typ_b as u16) << pm1_cnt::SLP_TYP_SHIFT) | pm1_cnt::SLP_EN;

    // Write to PM1 control registers
    unsafe {
        if fadt.pm1a_cnt_blk != 0 {
            let mut port: Port<u16> = Port::new(fadt.pm1a_cnt_blk as u16);
            port.write(pm1a_cnt_value);
        }

        if fadt.pm1b_cnt_blk != 0 {
            let mut port: Port<u16> = Port::new(fadt.pm1b_cnt_blk as u16);
            port.write(pm1b_cnt_value);
        }
    }

    // For S1-S3, we should wake up here after an interrupt
    // Wait for wake status
    for _ in 0..10000000 {
        if fadt.pm1a_evt_blk != 0 {
            unsafe {
                let mut port: Port<u16> = Port::new(fadt.pm1a_evt_blk as u16);
                let status = port.read();
                if status & pm1_sts::WAK_STS != 0 {
                    // Woke up!
                    x86_64::instructions::interrupts::enable();
                    crate::kprintln!("acpi: woke up from S{}", state as u8);
                    return Ok(());
                }
            }
        }
        core::hint::spin_loop();
    }

    // For S4/S5, we shouldn't reach here
    if matches!(state, SleepState::S4 | SleepState::S5) {
        Err("Sleep state entry failed")
    } else {
        // Re-enable interrupts
        x86_64::instructions::interrupts::enable();
        Ok(())
    }
}

/// Shutdown the system using ACPI (S5 state)
pub fn shutdown() -> ! {
    use x86_64::instructions::port::Port;

    crate::kprintln!("acpi: initiating shutdown (S5)...");

    // Disable interrupts
    x86_64::instructions::interrupts::disable();

    // Sync filesystems before shutdown
    crate::kprintln!("acpi: syncing filesystems...");
    // TODO: Call filesystem sync

    // Method 1: Try QEMU/Bochs specific shutdown first (fast path)
    unsafe {
        let mut port: Port<u16> = Port::new(0x604);
        port.write(0x2000);
    }

    // Wait a bit
    for _ in 0..1000000 {
        core::hint::spin_loop();
    }

    // Method 2: Try standard ACPI S5 shutdown
    if let Some(fadt) = parse_fadt() {
        // Get SLP_TYP from power state info, or use default
        let slp_typ = power_state_info()
            .and_then(|psi| psi.s5)
            .unwrap_or(SleepTypeValue { slp_typ_a: 5, slp_typ_b: 5 });

        // Clear wake status before entering S5
        if fadt.pm1a_evt_blk != 0 {
            unsafe {
                let mut port: Port<u16> = Port::new(fadt.pm1a_evt_blk as u16);
                port.write(pm1_sts::WAK_STS);
            }
        }

        let pm1a_cnt_value = ((slp_typ.slp_typ_a as u16) << pm1_cnt::SLP_TYP_SHIFT) | pm1_cnt::SLP_EN;
        let pm1b_cnt_value = ((slp_typ.slp_typ_b as u16) << pm1_cnt::SLP_TYP_SHIFT) | pm1_cnt::SLP_EN;

        unsafe {
            if fadt.pm1a_cnt_blk != 0 {
                let mut port: Port<u16> = Port::new(fadt.pm1a_cnt_blk as u16);
                port.write(pm1a_cnt_value);
            }

            if fadt.pm1b_cnt_blk != 0 {
                let mut port: Port<u16> = Port::new(fadt.pm1b_cnt_blk as u16);
                port.write(pm1b_cnt_value);
            }
        }
    }

    // Wait for shutdown
    for _ in 0..10000000 {
        core::hint::spin_loop();
    }

    // Method 3: Try I/O port 0xB004 (Bochs/QEMU alternative)
    unsafe {
        let mut port: Port<u16> = Port::new(0xB004);
        port.write(0x2000);
    }

    // If we're still running, halt
    crate::kprintln!("acpi: shutdown failed, halting CPU");
    loop {
        x86_64::instructions::hlt();
    }
}

/// Suspend to RAM (S3 state)
pub fn suspend_to_ram() -> Result<(), &'static str> {
    crate::kprintln!("acpi: suspending to RAM (S3)...");

    // TODO: Save CPU state, device states, etc.
    // TODO: Notify drivers to suspend

    enter_sleep_state(SleepState::S3)?;

    // TODO: Restore device states after wake
    crate::kprintln!("acpi: resumed from S3 suspend");
    Ok(())
}

/// Light sleep (S1 state)
pub fn light_sleep() -> Result<(), &'static str> {
    crate::kprintln!("acpi: entering light sleep (S1)...");
    enter_sleep_state(SleepState::S1)?;
    crate::kprintln!("acpi: resumed from S1 sleep");
    Ok(())
}

/// Hibernate (S4 state) - requires swap/disk support
pub fn hibernate() -> Result<(), &'static str> {
    crate::kprintln!("acpi: hibernate (S4) not fully supported");
    // TODO: Save memory to disk before entering S4
    Err("Hibernate not implemented - requires swap support")
}

/// Get current ACPI system state (always S0 when running)
pub fn current_state() -> SleepState {
    SleepState::S0
}

/// Check if a sleep state is supported by the hardware
pub fn is_sleep_state_supported(state: SleepState) -> bool {
    power_state_info()
        .map(|psi| psi.is_supported(state))
        .unwrap_or(false)
}

/// Reboot the system
pub fn reboot() -> ! {
    use x86_64::instructions::port::Port;

    crate::kprintln!("acpi: initiating reboot...");

    // Disable interrupts
    x86_64::instructions::interrupts::disable();

    // Method 1: Try keyboard controller reset (most compatible)
    unsafe {
        // Wait for keyboard controller to be ready
        let mut status_port: Port<u8> = Port::new(0x64);
        let mut cmd_port: Port<u8> = Port::new(0x64);

        for _ in 0..10000 {
            if (status_port.read() & 0x02) == 0 {
                break;
            }
            core::hint::spin_loop();
        }

        // Send reset command to keyboard controller
        cmd_port.write(0xFE);
    }

    // Wait a bit
    for _ in 0..1000000 {
        core::hint::spin_loop();
    }

    // Method 2: Try ACPI reset if available
    if let Some(fadt) = parse_fadt() {
        if fadt.reset_reg.address != 0 {
            match fadt.reset_reg.address_space {
                1 => {
                    // System I/O
                    unsafe {
                        let mut port: Port<u8> = Port::new(fadt.reset_reg.address as u16);
                        port.write(fadt.reset_value);
                    }
                }
                0 => {
                    // System Memory
                    let phys_offset = crate::mm::physical_memory_offset();
                    let addr = phys_offset + fadt.reset_reg.address;
                    unsafe {
                        let ptr = addr.as_mut_ptr::<u8>();
                        *ptr = fadt.reset_value;
                    }
                }
                _ => {}
            }
        }
    }

    // Wait for reboot
    for _ in 0..10000000 {
        core::hint::spin_loop();
    }

    // Method 3: Triple fault (last resort)
    crate::kprintln!("acpi: reboot failed, attempting triple fault");

    // Load a null IDT to cause a triple fault
    unsafe {
        let null_idt = x86_64::structures::idt::InterruptDescriptorTable::new();
        let idt_ptr = &null_idt as *const _;
        core::arch::asm!(
            "lidt [{}]",
            "int 3",
            in(reg) idt_ptr,
            options(noreturn)
        );
    }
}

// ==================== HPET (High Precision Event Timer) ====================

/// HPET ACPI table structure
#[repr(C, packed)]
pub struct HpetTable {
    pub header: AcpiTableHeader,
    pub event_timer_block_id: u32,
    pub base_address: GenericAddressStructure,
    pub hpet_number: u8,
    pub min_clock_tick: u16,
    pub page_protection: u8,
}

/// HPET information from ACPI
#[derive(Debug, Clone)]
pub struct HpetInfo {
    pub base_address: u64,
    pub hpet_number: u8,
    pub num_comparators: u8,
    pub counter_size_64: bool,
    pub legacy_replacement: bool,
    pub vendor_id: u16,
    pub min_clock_tick: u16,
}

/// Parse HPET ACPI table
pub fn parse_hpet() -> Option<HpetInfo> {
    let hpet = find_table(b"HPET")?;
    let phys_offset = crate::mm::physical_memory_offset();
    let hpet_virt = phys_offset + hpet.address;

    unsafe {
        let hpet_data = &*(hpet_virt.as_ptr::<HpetTable>());

        // Extract fields from event_timer_block_id
        let block_id = { hpet_data.event_timer_block_id };
        let num_comparators = ((block_id >> 8) & 0x1F) as u8 + 1;
        let counter_size_64 = (block_id & (1 << 13)) != 0;
        let legacy_replacement = (block_id & (1 << 15)) != 0;
        let vendor_id = ((block_id >> 16) & 0xFFFF) as u16;

        // Get base address - handle packed struct
        let base_address = core::ptr::read_unaligned(
            core::ptr::addr_of!(hpet_data.base_address.address)
        );
        let hpet_number = { hpet_data.hpet_number };
        let min_clock_tick = core::ptr::read_unaligned(
            core::ptr::addr_of!(hpet_data.min_clock_tick)
        );

        Some(HpetInfo {
            base_address,
            hpet_number,
            num_comparators,
            counter_size_64,
            legacy_replacement,
            vendor_id,
            min_clock_tick,
        })
    }
}

// ==================== DSDT/SSDT (Differentiated System Description Table) ====================

/// AML Opcodes
pub mod aml_opcode {
    pub const ZERO_OP: u8 = 0x00;
    pub const ONE_OP: u8 = 0x01;
    pub const ALIAS_OP: u8 = 0x06;
    pub const NAME_OP: u8 = 0x08;
    pub const BYTE_PREFIX: u8 = 0x0A;
    pub const WORD_PREFIX: u8 = 0x0B;
    pub const DWORD_PREFIX: u8 = 0x0C;
    pub const STRING_PREFIX: u8 = 0x0D;
    pub const QWORD_PREFIX: u8 = 0x0E;
    pub const SCOPE_OP: u8 = 0x10;
    pub const BUFFER_OP: u8 = 0x11;
    pub const PACKAGE_OP: u8 = 0x12;
    pub const VAR_PACKAGE_OP: u8 = 0x13;
    pub const METHOD_OP: u8 = 0x14;
    pub const EXTERNAL_OP: u8 = 0x15;
    pub const DUAL_NAME_PREFIX: u8 = 0x2E;
    pub const MULTI_NAME_PREFIX: u8 = 0x2F;
    pub const EXT_OP_PREFIX: u8 = 0x5B;
    pub const ROOT_CHAR: u8 = 0x5C;
    pub const PARENT_PREFIX: u8 = 0x5E;
    pub const LOCAL0_OP: u8 = 0x60;
    pub const LOCAL7_OP: u8 = 0x67;
    pub const ARG0_OP: u8 = 0x68;
    pub const ARG6_OP: u8 = 0x6E;

    // Extended opcodes (after 0x5B prefix)
    pub const EXT_MUTEX_OP: u8 = 0x01;
    pub const EXT_EVENT_OP: u8 = 0x02;
    pub const EXT_COND_REF_OF_OP: u8 = 0x12;
    pub const EXT_CREATE_FIELD_OP: u8 = 0x13;
    pub const EXT_LOAD_TABLE_OP: u8 = 0x1F;
    pub const EXT_LOAD_OP: u8 = 0x20;
    pub const EXT_STALL_OP: u8 = 0x21;
    pub const EXT_SLEEP_OP: u8 = 0x22;
    pub const EXT_ACQUIRE_OP: u8 = 0x23;
    pub const EXT_SIGNAL_OP: u8 = 0x24;
    pub const EXT_WAIT_OP: u8 = 0x25;
    pub const EXT_RESET_OP: u8 = 0x26;
    pub const EXT_RELEASE_OP: u8 = 0x27;
    pub const EXT_FROM_BCD_OP: u8 = 0x28;
    pub const EXT_TO_BCD_OP: u8 = 0x29;
    pub const EXT_REVISION_OP: u8 = 0x30;
    pub const EXT_DEBUG_OP: u8 = 0x31;
    pub const EXT_FATAL_OP: u8 = 0x32;
    pub const EXT_TIMER_OP: u8 = 0x33;
    pub const EXT_OP_REGION_OP: u8 = 0x80;
    pub const EXT_FIELD_OP: u8 = 0x81;
    pub const EXT_DEVICE_OP: u8 = 0x82;
    pub const EXT_PROCESSOR_OP: u8 = 0x83;
    pub const EXT_POWER_RES_OP: u8 = 0x84;
    pub const EXT_THERMAL_ZONE_OP: u8 = 0x85;
    pub const EXT_INDEX_FIELD_OP: u8 = 0x86;
    pub const EXT_BANK_FIELD_OP: u8 = 0x87;
    pub const EXT_DATA_REGION_OP: u8 = 0x88;
}

/// DSDT/SSDT information
#[derive(Debug, Clone)]
pub struct DsdtInfo {
    pub address: u64,
    pub length: u32,
    pub revision: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub devices: Vec<AcpiDevice>,
}

/// An ACPI device found in DSDT/SSDT
#[derive(Debug, Clone)]
pub struct AcpiDevice {
    /// Full namespace path (e.g., "\_SB.PCI0.LPCB")
    pub path: String,
    /// _HID (Hardware ID) - EISA ID or PnP ID
    pub hid: Option<String>,
    /// _CID (Compatible ID) - may have multiple
    pub cid: Vec<String>,
    /// _UID (Unique ID)
    pub uid: Option<u64>,
    /// _ADR (Address) - for PCI devices
    pub adr: Option<u64>,
    /// _STA (Status) value if available
    pub sta: Option<u32>,
    /// Device type classification
    pub device_type: AcpiDeviceType,
}

/// Device type classification based on _HID
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpiDeviceType {
    Unknown,
    Processor,
    SystemBus,
    PciBus,
    PciDevice,
    IsaBus,
    Keyboard,
    Mouse,
    RealTimeClock,
    Timer,
    Speaker,
    PowerButton,
    SleepButton,
    LidSwitch,
    AcAdapter,
    Battery,
    ThermalZone,
    EmbeddedController,
    Uart,
    I2cBus,
    SpiBus,
    Gpio,
    Other,
}

/// Static storage for DSDT info
static DSDT_INFO: Once<DsdtInfo> = Once::new();

/// Get DSDT pointer from FADT
fn get_dsdt_address() -> Option<u64> {
    let fadt = find_table(b"FACP")?;
    let phys_offset = crate::mm::physical_memory_offset();
    let fadt_virt = phys_offset + fadt.address;

    unsafe {
        // Check if we have extended FADT with X_DSDT
        let fadt_header = &*(fadt_virt.as_ptr::<AcpiTableHeader>());
        if fadt_header.length >= 148 {
            // ACPI 2.0+ FADT has X_DSDT at offset 140
            let x_dsdt_ptr = (fadt_virt + 140).as_ptr::<u64>();
            let x_dsdt = *x_dsdt_ptr;
            if x_dsdt != 0 {
                return Some(x_dsdt);
            }
        }

        // Fall back to 32-bit DSDT pointer
        let fadt_data = &*(fadt_virt.as_ptr::<Fadt>());
        if fadt_data.dsdt != 0 {
            return Some(fadt_data.dsdt as u64);
        }
    }

    None
}

/// Initialize DSDT/SSDT parsing
pub fn init_dsdt() {
    crate::kprintln!("acpi: parsing DSDT/SSDT...");

    let dsdt_addr = match get_dsdt_address() {
        Some(addr) => addr,
        None => {
            crate::kprintln!("acpi: DSDT not found in FADT");
            return;
        }
    };

    let phys_offset = crate::mm::physical_memory_offset();
    let dsdt_virt = phys_offset + dsdt_addr;

    let mut info = unsafe {
        let header_ptr = dsdt_virt.as_ptr::<AcpiTableHeader>();
        let length = core::ptr::read_unaligned(core::ptr::addr_of!((*header_ptr).length));
        let revision = core::ptr::read_unaligned(core::ptr::addr_of!((*header_ptr).revision));
        let oem_id = core::ptr::read_unaligned(core::ptr::addr_of!((*header_ptr).oem_id));
        let oem_table_id = core::ptr::read_unaligned(core::ptr::addr_of!((*header_ptr).oem_table_id));

        crate::kprintln!(
            "acpi: DSDT @ {:#x} (len={}, rev={})",
            dsdt_addr,
            length,
            revision
        );

        DsdtInfo {
            address: dsdt_addr,
            length,
            revision,
            oem_id,
            oem_table_id,
            devices: Vec::new(),
        }
    };

    // Parse DSDT AML
    let aml_start = dsdt_virt + core::mem::size_of::<AcpiTableHeader>() as u64;
    let aml_len = info.length as usize - core::mem::size_of::<AcpiTableHeader>();

    parse_aml_namespace(aml_start, aml_len, &mut info.devices, String::from("\\"));

    // Also parse any SSDTs
    if let Some(state) = get_state() {
        for table in &state.tables {
            if &table.signature == b"SSDT" {
                let ssdt_virt = phys_offset + table.address;
                unsafe {
                    let header_ptr = ssdt_virt.as_ptr::<AcpiTableHeader>();
                    let ssdt_length = core::ptr::read_unaligned(core::ptr::addr_of!((*header_ptr).length));
                    let ssdt_aml_start = ssdt_virt + core::mem::size_of::<AcpiTableHeader>() as u64;
                    let ssdt_aml_len = ssdt_length as usize - core::mem::size_of::<AcpiTableHeader>();

                    crate::kprintln!(
                        "acpi: SSDT @ {:#x} (len={})",
                        table.address,
                        ssdt_length
                    );

                    parse_aml_namespace(ssdt_aml_start, ssdt_aml_len, &mut info.devices, String::from("\\"));
                }
            }
        }
    }

    crate::kprintln!("acpi: found {} devices in DSDT/SSDT", info.devices.len());

    // Print device summary
    for dev in &info.devices {
        if dev.hid.is_some() || dev.adr.is_some() {
            let hid_str = dev.hid.as_ref().map(|s| s.as_str()).unwrap_or("-");
            let adr_str = if let Some(adr) = dev.adr {
                alloc::format!("ADR={:#x}", adr)
            } else {
                String::from("")
            };
            crate::kprintln!(
                "  {:?}: {} HID={} {}",
                dev.device_type,
                dev.path,
                hid_str,
                adr_str
            );
        }
    }

    DSDT_INFO.call_once(|| info);
}

/// Get parsed DSDT info
pub fn get_dsdt_info() -> Option<&'static DsdtInfo> {
    DSDT_INFO.get()
}

/// Find devices by type
pub fn find_devices_by_type(device_type: AcpiDeviceType) -> Vec<&'static AcpiDevice> {
    DSDT_INFO
        .get()
        .map(|info| {
            info.devices
                .iter()
                .filter(|d| d.device_type == device_type)
                .collect()
        })
        .unwrap_or_default()
}

/// Find device by HID
pub fn find_device_by_hid(hid: &str) -> Option<&'static AcpiDevice> {
    DSDT_INFO.get()?.devices.iter().find(|d| {
        d.hid.as_ref().map(|h| h == hid).unwrap_or(false)
            || d.cid.iter().any(|c| c == hid)
    })
}

/// Parse AML namespace and extract devices
fn parse_aml_namespace(
    aml_start: x86_64::VirtAddr,
    aml_len: usize,
    devices: &mut Vec<AcpiDevice>,
    current_scope: String,
) {
    // Limit recursion depth to avoid stack overflow
    static mut RECURSION_DEPTH: u32 = 0;
    const MAX_RECURSION: u32 = 16;

    unsafe {
        if RECURSION_DEPTH >= MAX_RECURSION {
            return;
        }
        RECURSION_DEPTH += 1;
    }

    let mut offset = 0usize;
    let mut iterations = 0u32;
    const MAX_ITERATIONS: u32 = 10000;

    while offset < aml_len && iterations < MAX_ITERATIONS {
        iterations += 1;
        let opcode = unsafe { *((aml_start + offset as u64).as_ptr::<u8>()) };
        offset += 1;

        match opcode {
            aml_opcode::SCOPE_OP => {
                // Scope(name) { ... }
                if let Some((pkg_len, name, inner_offset)) = parse_scope_or_device(aml_start, offset - 1, aml_len) {
                    let full_path = resolve_path(&current_scope, &name);
                    let inner_len = pkg_len.saturating_sub(inner_offset);

                    // Recursively parse inner scope
                    parse_aml_namespace(
                        aml_start + (offset - 1 + inner_offset) as u64,
                        inner_len,
                        devices,
                        full_path,
                    );

                    offset = offset - 1 + pkg_len;
                }
            }

            aml_opcode::EXT_OP_PREFIX => {
                if offset >= aml_len {
                    break;
                }
                let ext_opcode = unsafe { *((aml_start + offset as u64).as_ptr::<u8>()) };
                offset += 1;

                match ext_opcode {
                    aml_opcode::EXT_DEVICE_OP => {
                        // Device(name) { ... }
                        if let Some((pkg_len, name, inner_offset)) = parse_scope_or_device(aml_start, offset - 2, aml_len) {
                            let full_path = resolve_path(&current_scope, &name);

                            // Create device entry
                            let mut device = AcpiDevice {
                                path: full_path.clone(),
                                hid: None,
                                cid: Vec::new(),
                                uid: None,
                                adr: None,
                                sta: None,
                                device_type: AcpiDeviceType::Unknown,
                            };

                            // Parse device internals for _HID, _CID, _UID, _ADR
                            let inner_start = aml_start + (offset - 2 + inner_offset) as u64;
                            let inner_len = pkg_len.saturating_sub(inner_offset);
                            parse_device_properties(inner_start, inner_len, &mut device);

                            // Classify device type
                            device.device_type = classify_device(&device);

                            devices.push(device.clone());

                            // Also recurse to find nested devices
                            parse_aml_namespace(inner_start, inner_len, devices, full_path);

                            offset = offset - 2 + pkg_len;
                        }
                    }

                    aml_opcode::EXT_PROCESSOR_OP => {
                        // Processor declaration
                        if let Some((pkg_len, name, _)) = parse_processor(aml_start, offset - 2, aml_len) {
                            let full_path = resolve_path(&current_scope, &name);
                            devices.push(AcpiDevice {
                                path: full_path,
                                hid: Some(String::from("ACPI0007")),
                                cid: Vec::new(),
                                uid: None,
                                adr: None,
                                sta: Some(0x0F), // Present and functioning
                                device_type: AcpiDeviceType::Processor,
                            });
                            offset = offset - 2 + pkg_len;
                        }
                    }

                    aml_opcode::EXT_THERMAL_ZONE_OP => {
                        // ThermalZone
                        if let Some((pkg_len, name, _)) = parse_scope_or_device(aml_start, offset - 2, aml_len) {
                            let full_path = resolve_path(&current_scope, &name);
                            devices.push(AcpiDevice {
                                path: full_path,
                                hid: None,
                                cid: Vec::new(),
                                uid: None,
                                adr: None,
                                sta: None,
                                device_type: AcpiDeviceType::ThermalZone,
                            });
                            offset = offset - 2 + pkg_len;
                        }
                    }

                    aml_opcode::EXT_POWER_RES_OP => {
                        // PowerResource - skip
                        if let Some((pkg_len, _, _)) = parse_scope_or_device(aml_start, offset - 2, aml_len) {
                            offset = offset - 2 + pkg_len;
                        }
                    }

                    aml_opcode::EXT_OP_REGION_OP | aml_opcode::EXT_FIELD_OP
                    | aml_opcode::EXT_INDEX_FIELD_OP | aml_opcode::EXT_BANK_FIELD_OP => {
                        // Skip these for now
                    }

                    _ => {}
                }
            }

            aml_opcode::NAME_OP | aml_opcode::METHOD_OP => {
                // Skip Name and Method definitions at this level
            }

            _ => {
                // Skip other opcodes
            }
        }
    }

    unsafe {
        RECURSION_DEPTH = RECURSION_DEPTH.saturating_sub(1);
    }
}

/// Parse a Scope or Device package
/// Returns (total_length, name_string, offset_to_inner_content)
fn parse_scope_or_device(
    aml_start: x86_64::VirtAddr,
    offset: usize,
    max_len: usize,
) -> Option<(usize, String, usize)> {
    let mut pos = offset;

    // Skip opcode (1 or 2 bytes)
    let opcode = unsafe { *((aml_start + pos as u64).as_ptr::<u8>()) };
    pos += 1;

    if opcode == aml_opcode::EXT_OP_PREFIX {
        pos += 1; // Skip extended opcode
    }

    if pos >= max_len {
        return None;
    }

    // Parse PkgLength
    let (pkg_len, pkg_len_bytes) = parse_pkg_length(aml_start, pos, max_len)?;
    pos += pkg_len_bytes;

    // Parse NameString
    let (name, name_bytes) = parse_name_string(aml_start, pos, max_len)?;
    pos += name_bytes;

    let total_len = 1 + (if opcode == aml_opcode::EXT_OP_PREFIX { 1 } else { 0 }) + pkg_len;
    let inner_offset = pos - offset;

    Some((total_len, name, inner_offset))
}

/// Parse Processor definition
fn parse_processor(
    aml_start: x86_64::VirtAddr,
    offset: usize,
    max_len: usize,
) -> Option<(usize, String, usize)> {
    let mut pos = offset + 2; // Skip 0x5B 0x83

    if pos >= max_len {
        return None;
    }

    // Parse PkgLength
    let (pkg_len, pkg_len_bytes) = parse_pkg_length(aml_start, pos, max_len)?;
    pos += pkg_len_bytes;

    // Parse NameString
    let (name, name_bytes) = parse_name_string(aml_start, pos, max_len)?;
    pos += name_bytes;

    // Skip ProcID (1 byte), PblkAddr (4 bytes), PblkLen (1 byte)
    pos += 6;

    let total_len = 2 + pkg_len;
    let inner_offset = pos - offset;

    Some((total_len, name, inner_offset))
}

/// Parse PkgLength
fn parse_pkg_length(aml_start: x86_64::VirtAddr, offset: usize, max_len: usize) -> Option<(usize, usize)> {
    if offset >= max_len {
        return None;
    }

    let lead_byte = unsafe { *((aml_start + offset as u64).as_ptr::<u8>()) };

    // PkgLength encoding:
    // If bit 6-7 are 0: bits 0-5 are the length (1 byte encoding)
    // If bit 6-7 are 1: bits 0-3 are low nibble, next byte is bits 4-11 (2 byte encoding)
    // If bit 6-7 are 2: bits 0-3 are low nibble, next 2 bytes (3 byte encoding)
    // If bit 6-7 are 3: bits 0-3 are low nibble, next 3 bytes (4 byte encoding)

    let byte_count = ((lead_byte >> 6) & 0x03) as usize;

    if byte_count == 0 {
        // Single byte encoding
        let length = (lead_byte & 0x3F) as usize;
        Some((length, 1))
    } else {
        // Multi-byte encoding
        let num_bytes = byte_count + 1;
        if offset + num_bytes > max_len {
            return None;
        }

        let mut length = (lead_byte & 0x0F) as usize;
        for i in 1..num_bytes {
            let b = unsafe { *((aml_start + (offset + i) as u64).as_ptr::<u8>()) };
            length |= (b as usize) << (4 + (i - 1) * 8);
        }

        Some((length, num_bytes))
    }
}

/// Parse a NameString from AML
fn parse_name_string(aml_start: x86_64::VirtAddr, offset: usize, max_len: usize) -> Option<(String, usize)> {
    if offset >= max_len {
        return None;
    }

    let mut pos = offset;
    let mut name = String::new();

    // Check for root or parent prefix
    loop {
        let b = unsafe { *((aml_start + pos as u64).as_ptr::<u8>()) };

        match b {
            aml_opcode::ROOT_CHAR => {
                name.push('\\');
                pos += 1;
            }
            aml_opcode::PARENT_PREFIX => {
                name.push('^');
                pos += 1;
            }
            _ => break,
        }

        if pos >= max_len {
            return None;
        }
    }

    // Parse name segments
    let lead = unsafe { *((aml_start + pos as u64).as_ptr::<u8>()) };

    match lead {
        0x00 => {
            // NullName
            pos += 1;
        }
        aml_opcode::DUAL_NAME_PREFIX => {
            pos += 1;
            // Two 4-character names
            for _ in 0..2 {
                if pos + 4 > max_len {
                    return None;
                }
                if !name.is_empty() && !name.ends_with('\\') && !name.ends_with('^') {
                    name.push('.');
                }
                for i in 0..4 {
                    let c = unsafe { *((aml_start + (pos + i) as u64).as_ptr::<u8>()) };
                    if c != b'_' || i < 4 {
                        // Keep trailing underscores for now
                        name.push(c as char);
                    }
                }
                pos += 4;
            }
        }
        aml_opcode::MULTI_NAME_PREFIX => {
            pos += 1;
            if pos >= max_len {
                return None;
            }
            let seg_count = unsafe { *((aml_start + pos as u64).as_ptr::<u8>()) } as usize;
            pos += 1;

            for _ in 0..seg_count {
                if pos + 4 > max_len {
                    return None;
                }
                if !name.is_empty() && !name.ends_with('\\') && !name.ends_with('^') {
                    name.push('.');
                }
                for i in 0..4 {
                    let c = unsafe { *((aml_start + (pos + i) as u64).as_ptr::<u8>()) };
                    name.push(c as char);
                }
                pos += 4;
            }
        }
        b if is_lead_name_char(b) => {
            // Single 4-character NameSeg
            if pos + 4 > max_len {
                return None;
            }
            for i in 0..4 {
                let c = unsafe { *((aml_start + (pos + i) as u64).as_ptr::<u8>()) };
                name.push(c as char);
            }
            pos += 4;
        }
        _ => {
            // Not a valid name - might be empty
        }
    }

    // Trim trailing underscores from each segment
    let trimmed: String = name
        .split('.')
        .map(|s| s.trim_end_matches('_'))
        .collect::<Vec<_>>()
        .join(".");

    Some((trimmed, pos - offset))
}

/// Check if byte is a valid lead name character (A-Z or _)
fn is_lead_name_char(b: u8) -> bool {
    matches!(b, b'A'..=b'Z' | b'_')
}

/// Resolve a relative path against current scope
fn resolve_path(current: &str, name: &str) -> String {
    if name.starts_with('\\') {
        // Absolute path
        name.to_string()
    } else if name.starts_with('^') {
        // Parent reference - go up
        let mut parts: Vec<&str> = current.split('.').collect();
        let mut remaining = name;

        while remaining.starts_with('^') {
            if parts.len() > 1 {
                parts.pop();
            }
            remaining = &remaining[1..];
        }

        if remaining.is_empty() {
            parts.join(".")
        } else {
            format!("{}.{}", parts.join("."), remaining)
        }
    } else if name.is_empty() {
        current.to_string()
    } else {
        // Relative path
        if current == "\\" {
            format!("\\{}", name)
        } else {
            format!("{}.{}", current, name)
        }
    }
}

/// Parse device properties (_HID, _CID, _UID, _ADR)
fn parse_device_properties(
    aml_start: x86_64::VirtAddr,
    aml_len: usize,
    device: &mut AcpiDevice,
) {
    let mut offset = 0usize;

    while offset < aml_len {
        let opcode = unsafe { *((aml_start + offset as u64).as_ptr::<u8>()) };

        if opcode == aml_opcode::NAME_OP {
            offset += 1;
            if offset + 4 > aml_len {
                break;
            }

            // Get the 4-character name
            let name_bytes: [u8; 4] = unsafe {
                [
                    *((aml_start + offset as u64).as_ptr::<u8>()),
                    *((aml_start + (offset + 1) as u64).as_ptr::<u8>()),
                    *((aml_start + (offset + 2) as u64).as_ptr::<u8>()),
                    *((aml_start + (offset + 3) as u64).as_ptr::<u8>()),
                ]
            };
            offset += 4;

            match &name_bytes {
                b"_HID" => {
                    if let Some((val, len)) = parse_name_value(aml_start, offset, aml_len) {
                        device.hid = Some(eisaid_to_string(val));
                        offset += len;
                    }
                }
                b"_CID" => {
                    if let Some((val, len)) = parse_name_value(aml_start, offset, aml_len) {
                        device.cid.push(eisaid_to_string(val));
                        offset += len;
                    }
                }
                b"_UID" => {
                    if let Some((val, len)) = parse_integer_value(aml_start, offset, aml_len) {
                        device.uid = Some(val);
                        offset += len;
                    }
                }
                b"_ADR" => {
                    if let Some((val, len)) = parse_integer_value(aml_start, offset, aml_len) {
                        device.adr = Some(val);
                        offset += len;
                    }
                }
                b"_STA" => {
                    // _STA could be a method or integer
                    if let Some((val, len)) = parse_integer_value(aml_start, offset, aml_len) {
                        device.sta = Some(val as u32);
                        offset += len;
                    }
                }
                _ => {}
            }
        } else {
            offset += 1;
        }
    }
}

/// Parse a name value (can be integer or string)
fn parse_name_value(
    aml_start: x86_64::VirtAddr,
    offset: usize,
    max_len: usize,
) -> Option<(u64, usize)> {
    if offset >= max_len {
        return None;
    }

    let opcode = unsafe { *((aml_start + offset as u64).as_ptr::<u8>()) };

    match opcode {
        aml_opcode::ZERO_OP => Some((0, 1)),
        aml_opcode::ONE_OP => Some((1, 1)),
        aml_opcode::BYTE_PREFIX => {
            if offset + 2 > max_len {
                return None;
            }
            let val = unsafe { *((aml_start + (offset + 1) as u64).as_ptr::<u8>()) } as u64;
            Some((val, 2))
        }
        aml_opcode::WORD_PREFIX => {
            if offset + 3 > max_len {
                return None;
            }
            let val = unsafe { *((aml_start + (offset + 1) as u64).as_ptr::<u16>()) } as u64;
            Some((val, 3))
        }
        aml_opcode::DWORD_PREFIX => {
            if offset + 5 > max_len {
                return None;
            }
            let val = unsafe { *((aml_start + (offset + 1) as u64).as_ptr::<u32>()) } as u64;
            Some((val, 5))
        }
        aml_opcode::QWORD_PREFIX => {
            if offset + 9 > max_len {
                return None;
            }
            let val = unsafe { *((aml_start + (offset + 1) as u64).as_ptr::<u64>()) };
            Some((val, 9))
        }
        aml_opcode::STRING_PREFIX => {
            // String value - parse as EISAID-like string
            let mut end = offset + 1;
            while end < max_len {
                let b = unsafe { *((aml_start + end as u64).as_ptr::<u8>()) };
                if b == 0 {
                    break;
                }
                end += 1;
            }
            // For now, just return 0 for strings (we'll handle them specially)
            Some((0, end - offset + 1))
        }
        _ => None,
    }
}

/// Parse an integer value
fn parse_integer_value(
    aml_start: x86_64::VirtAddr,
    offset: usize,
    max_len: usize,
) -> Option<(u64, usize)> {
    parse_name_value(aml_start, offset, max_len)
}

/// Convert EISAID to string
/// EISAID is a 32-bit compressed EISA ID or a string
fn eisaid_to_string(val: u64) -> String {
    if val == 0 {
        return String::from("UNKNOWN");
    }

    // EISAID encoding: bits 16-31 are 3-letter vendor ID (compressed)
    // bits 0-15 are 4-digit product ID (BCD)
    let val = val as u32;

    // Extract vendor ID (3 letters, 5 bits each)
    let c1 = ((val >> 26) & 0x1F) as u8 + b'@';
    let c2 = ((val >> 21) & 0x1F) as u8 + b'@';
    let c3 = ((val >> 16) & 0x1F) as u8 + b'@';

    // Extract product ID (4 hex digits)
    let prod = val & 0xFFFF;

    alloc::format!(
        "{}{}{}{}",
        c1 as char,
        c2 as char,
        c3 as char,
        alloc::format!("{:04X}", prod)
    )
}

/// Classify device based on _HID
fn classify_device(device: &AcpiDevice) -> AcpiDeviceType {
    let check_id = |id: &str| -> AcpiDeviceType {
        match id {
            // Processors
            "ACPI0007" => AcpiDeviceType::Processor,

            // System/Bus
            "PNP0A03" | "PNP0A08" => AcpiDeviceType::PciBus,  // PCI, PCIe
            "PNP0A05" | "PNP0A06" => AcpiDeviceType::SystemBus, // Generic Container
            "ACPI0004" => AcpiDeviceType::SystemBus, // Module Device

            // Input
            "PNP0303" | "PNP030B" => AcpiDeviceType::Keyboard, // IBM Enhanced, MS Natural
            "PNP0F03" | "PNP0F13" => AcpiDeviceType::Mouse, // PS/2 Mouse

            // RTC/Timer
            "PNP0B00" => AcpiDeviceType::RealTimeClock, // AT RTC
            "PNP0100" => AcpiDeviceType::Timer, // AT Timer
            "PNP0103" => AcpiDeviceType::Timer, // HPET

            // Speaker
            "PNP0800" => AcpiDeviceType::Speaker,

            // Power/Thermal
            "PNP0C0A" => AcpiDeviceType::Battery,
            "ACPI0003" => AcpiDeviceType::AcAdapter,
            "PNP0C0D" => AcpiDeviceType::LidSwitch,
            "PNP0C0C" => AcpiDeviceType::PowerButton,
            "PNP0C0E" => AcpiDeviceType::SleepButton,

            // Embedded Controller
            "PNP0C09" => AcpiDeviceType::EmbeddedController,

            // Communication
            "PNP0501" => AcpiDeviceType::Uart, // 16550 UART

            // Buses
            "PNP0A00" => AcpiDeviceType::IsaBus, // ISA
            "ACPI0005" => AcpiDeviceType::SpiBus, // SPI
            "INT33C2" | "INT33C3" => AcpiDeviceType::I2cBus, // Intel I2C

            // GPIO
            "INT0002" | "INT3450" | "INT3452" | "INT345D" => AcpiDeviceType::Gpio,

            _ => AcpiDeviceType::Other,
        }
    };

    // Check _HID first
    if let Some(ref hid) = device.hid {
        let t = check_id(hid);
        if t != AcpiDeviceType::Other {
            return t;
        }
    }

    // Check _CID
    for cid in &device.cid {
        let t = check_id(cid);
        if t != AcpiDeviceType::Other {
            return t;
        }
    }

    // Check path for common patterns
    if device.path.contains("CPU") || device.path.contains("PR") {
        return AcpiDeviceType::Processor;
    }
    if device.path.contains("_SB") && device.adr.is_none() && device.hid.is_none() {
        return AcpiDeviceType::SystemBus;
    }

    // If it has an _ADR, it's likely a PCI device
    if device.adr.is_some() {
        return AcpiDeviceType::PciDevice;
    }

    AcpiDeviceType::Unknown
}

// ==================== MCFG (PCI Express Configuration) ====================

/// MCFG table structure
#[repr(C, packed)]
pub struct McfgTable {
    pub header: AcpiTableHeader,
    pub reserved: u64,
    // Followed by McfgEntry array
}

/// MCFG entry for a PCIe segment
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct McfgEntry {
    pub base_address: u64,
    pub segment_group: u16,
    pub start_bus: u8,
    pub end_bus: u8,
    pub reserved: u32,
}

/// PCIe configuration information from MCFG
#[derive(Debug, Clone)]
pub struct McfgInfo {
    pub entries: Vec<McfgEntry>,
}

/// Parse MCFG table for PCIe configuration space
pub fn parse_mcfg() -> Option<McfgInfo> {
    let mcfg = find_table(b"MCFG")?;
    let phys_offset = crate::mm::physical_memory_offset();
    let mcfg_virt = phys_offset + mcfg.address;

    let mut info = McfgInfo {
        entries: Vec::new(),
    };

    unsafe {
        let header = &*(mcfg_virt.as_ptr::<McfgTable>());
        let entries_start = mcfg_virt + core::mem::size_of::<McfgTable>() as u64;
        let entry_count = (header.header.length as usize - core::mem::size_of::<McfgTable>())
            / core::mem::size_of::<McfgEntry>();

        for i in 0..entry_count {
            let entry_ptr = (entries_start + (i * core::mem::size_of::<McfgEntry>()) as u64)
                .as_ptr::<McfgEntry>();
            info.entries.push(*entry_ptr);
        }
    }

    Some(info)
}

/// Get PCIe ECAM base address for a given segment/bus
pub fn get_pcie_ecam_base(segment: u16, bus: u8) -> Option<u64> {
    let mcfg = parse_mcfg()?;

    for entry in &mcfg.entries {
        if entry.segment_group == segment && bus >= entry.start_bus && bus <= entry.end_bus {
            return Some(entry.base_address);
        }
    }

    None
}
