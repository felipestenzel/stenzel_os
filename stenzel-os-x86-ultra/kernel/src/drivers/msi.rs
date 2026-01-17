//! MSI (Message Signaled Interrupts) and MSI-X Support
//!
//! Implements:
//! - MSI capability detection and configuration
//! - MSI-X capability detection and configuration
//! - Interrupt vector allocation
//! - Message address/data formatting for x86_64

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use spin::Mutex;

use super::pci::{self, PciDevice};

// MSI capability structure offsets (relative to capability base)
const MSI_CAP_CONTROL: u8 = 0x02;    // Message Control
const MSI_CAP_ADDR_LO: u8 = 0x04;    // Message Address Low
const MSI_CAP_ADDR_HI: u8 = 0x08;    // Message Address High (64-bit only)
const MSI_CAP_DATA_32: u8 = 0x08;    // Message Data (32-bit address)
const MSI_CAP_DATA_64: u8 = 0x0C;    // Message Data (64-bit address)
const MSI_CAP_MASK_32: u8 = 0x0C;    // Mask bits (32-bit, if per-vector masking)
const MSI_CAP_MASK_64: u8 = 0x10;    // Mask bits (64-bit, if per-vector masking)
const MSI_CAP_PENDING_32: u8 = 0x10; // Pending bits (32-bit)
const MSI_CAP_PENDING_64: u8 = 0x14; // Pending bits (64-bit)

// MSI Control Register bits
const MSI_CTRL_ENABLE: u16 = 1 << 0;
const MSI_CTRL_MMC_SHIFT: u16 = 1;    // Multiple Message Capable (shift)
const MSI_CTRL_MMC_MASK: u16 = 0x0E;  // MMC bits (1:3)
const MSI_CTRL_MME_SHIFT: u16 = 4;    // Multiple Message Enable (shift)
const MSI_CTRL_MME_MASK: u16 = 0x70;  // MME bits (4:6)
const MSI_CTRL_64BIT: u16 = 1 << 7;   // 64-bit Address Capable
const MSI_CTRL_PER_VECTOR_MASK: u16 = 1 << 8; // Per-Vector Masking Capable

// MSI-X capability structure offsets
const MSIX_CAP_CONTROL: u8 = 0x02;   // Message Control
const MSIX_CAP_TABLE: u8 = 0x04;     // Table Offset/BIR
const MSIX_CAP_PBA: u8 = 0x08;       // PBA Offset/BIR

// MSI-X Control Register bits
const MSIX_CTRL_ENABLE: u16 = 1 << 15;
const MSIX_CTRL_FUNCTION_MASK: u16 = 1 << 14;
const MSIX_CTRL_TABLE_SIZE_MASK: u16 = 0x07FF;

// MSI-X Table Entry offsets
const MSIX_ENTRY_ADDR_LO: u32 = 0x00;
const MSIX_ENTRY_ADDR_HI: u32 = 0x04;
const MSIX_ENTRY_DATA: u32 = 0x08;
const MSIX_ENTRY_CTRL: u32 = 0x0C;
const MSIX_ENTRY_SIZE: u32 = 16;

// MSI-X Table Entry Control bits
const MSIX_ENTRY_CTRL_MASKED: u32 = 1 << 0;

// x86_64 MSI address format
// Bits 31:20 = 0xFEE (fixed prefix)
// Bits 19:12 = Destination APIC ID
// Bit 3 = Redirection hint (0 = no, 1 = yes)
// Bit 2 = Destination mode (0 = physical, 1 = logical)
const MSI_ADDR_BASE: u64 = 0xFEE00000;
const MSI_ADDR_DEST_SHIFT: u64 = 12;
const MSI_ADDR_REDIRECTION: u64 = 1 << 3;
const MSI_ADDR_DEST_LOGICAL: u64 = 1 << 2;

// x86_64 MSI data format
// Bits 7:0 = Vector number
// Bits 10:8 = Delivery mode (000 = Fixed, 001 = Lowest Priority, etc.)
// Bit 14 = Level (0 = Deassert, 1 = Assert)
// Bit 15 = Trigger mode (0 = Edge, 1 = Level)
const MSI_DATA_VECTOR_MASK: u32 = 0xFF;
const MSI_DATA_DELIVERY_FIXED: u32 = 0 << 8;
const MSI_DATA_DELIVERY_LOWEST: u32 = 1 << 8;
const MSI_DATA_LEVEL_ASSERT: u32 = 1 << 14;
const MSI_DATA_TRIGGER_EDGE: u32 = 0 << 15;
const MSI_DATA_TRIGGER_LEVEL: u32 = 1 << 15;

/// MSI capability information
#[derive(Debug, Clone, Copy)]
pub struct MsiCapability {
    /// Offset of MSI capability in PCI config space
    pub offset: u8,
    /// Maximum number of vectors supported (1, 2, 4, 8, 16, or 32)
    pub max_vectors: u8,
    /// 64-bit addressing capable
    pub is_64bit: bool,
    /// Per-vector masking capable
    pub per_vector_mask: bool,
}

/// MSI-X capability information
#[derive(Debug, Clone, Copy)]
pub struct MsixCapability {
    /// Offset of MSI-X capability in PCI config space
    pub offset: u8,
    /// Number of table entries (1 to 2048)
    pub table_size: u16,
    /// Table BAR index
    pub table_bir: u8,
    /// Table offset within BAR
    pub table_offset: u32,
    /// PBA BAR index
    pub pba_bir: u8,
    /// PBA offset within BAR
    pub pba_offset: u32,
}

/// MSI/MSI-X configuration for a device
#[derive(Debug, Clone)]
pub struct MsiConfig {
    /// PCI device
    pub device: PciDevice,
    /// MSI capability (if supported)
    pub msi: Option<MsiCapability>,
    /// MSI-X capability (if supported)
    pub msix: Option<MsixCapability>,
    /// Allocated vector numbers
    pub vectors: Vec<u8>,
    /// Is MSI enabled?
    pub msi_enabled: bool,
    /// Is MSI-X enabled?
    pub msix_enabled: bool,
}

/// MSI subsystem
pub struct MsiSubsystem {
    /// Map of PCI device to MSI configuration
    devices: BTreeMap<u32, MsiConfig>,
    /// Next available MSI vector
    next_vector: AtomicU32,
    /// Vector allocation bitmap
    vector_bitmap: [u64; 4], // 256 vectors
}

impl MsiSubsystem {
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            next_vector: AtomicU32::new(MSI_VECTOR_BASE as u32),
            vector_bitmap: [0; 4],
        }
    }

    /// Get device key from PCI address
    fn device_key(dev: &PciDevice) -> u32 {
        ((dev.addr.bus as u32) << 16) | ((dev.addr.device as u32) << 8) | (dev.addr.function as u32)
    }

    /// Allocate a single vector
    fn alloc_vector(&mut self) -> Option<u8> {
        for word_idx in 0..4 {
            if self.vector_bitmap[word_idx] != u64::MAX {
                for bit in 0..64 {
                    let vec = (word_idx * 64 + bit) as u8;
                    if vec < MSI_VECTOR_BASE || vec > MSI_VECTOR_MAX {
                        continue;
                    }
                    if self.vector_bitmap[word_idx] & (1 << bit) == 0 {
                        self.vector_bitmap[word_idx] |= 1 << bit;
                        return Some(vec);
                    }
                }
            }
        }
        None
    }

    /// Free a vector
    fn free_vector(&mut self, vec: u8) {
        if vec >= MSI_VECTOR_BASE && vec <= MSI_VECTOR_MAX {
            let word_idx = (vec / 64) as usize;
            let bit = (vec % 64) as usize;
            self.vector_bitmap[word_idx] &= !(1 << bit);
        }
    }

    /// Allocate multiple vectors
    fn alloc_vectors(&mut self, count: u8) -> Vec<u8> {
        let mut vectors = Vec::with_capacity(count as usize);
        for _ in 0..count {
            if let Some(vec) = self.alloc_vector() {
                vectors.push(vec);
            } else {
                // Free already allocated vectors
                for v in &vectors {
                    self.free_vector(*v);
                }
                return Vec::new();
            }
        }
        vectors
    }

    /// Probe device for MSI/MSI-X support
    pub fn probe_device(&mut self, dev: &PciDevice) -> MsiConfig {
        let msi = probe_msi(dev);
        let msix = probe_msix(dev);

        let config = MsiConfig {
            device: *dev,
            msi,
            msix,
            vectors: Vec::new(),
            msi_enabled: false,
            msix_enabled: false,
        };

        let key = Self::device_key(dev);
        self.devices.insert(key, config.clone());

        config
    }

    /// Enable MSI for a device
    pub fn enable_msi(&mut self, dev: &PciDevice, num_vectors: u8) -> Result<Vec<u8>, MsiError> {
        let key = Self::device_key(dev);

        // Get capability info first without holding mutable borrow
        let msi_cap = {
            let config = self.devices.get(&key).ok_or(MsiError::DeviceNotFound)?;
            config.msi.ok_or(MsiError::NotSupported)?
        };

        // Clamp to supported vectors
        let max_vectors = msi_cap.max_vectors;
        let actual_vectors = num_vectors.min(max_vectors).max(1);

        // Allocate vectors (now we can borrow self mutably)
        let vectors = self.alloc_vectors(actual_vectors);
        if vectors.is_empty() {
            return Err(MsiError::NoVectorsAvailable);
        }

        // Configure MSI
        let first_vector = vectors[0];
        let target_cpu = 0u8; // Default to CPU 0

        configure_msi(dev, &msi_cap, first_vector, target_cpu, vectors.len() as u8)?;

        // Update config
        if let Some(config) = self.devices.get_mut(&key) {
            config.vectors = vectors.clone();
            config.msi_enabled = true;
            config.msix_enabled = false;
        }

        Ok(vectors)
    }

    /// Enable MSI-X for a device
    pub fn enable_msix(&mut self, dev: &PciDevice, entries: &[(u16, u8)]) -> Result<Vec<(u16, u8)>, MsiError> {
        let key = Self::device_key(dev);

        // Get capability info first without holding mutable borrow
        let msix_cap = {
            let config = self.devices.get(&key).ok_or(MsiError::DeviceNotFound)?;
            config.msix.ok_or(MsiError::NotSupported)?
        };

        // Validate entry indices
        for (entry, _) in entries {
            if *entry >= msix_cap.table_size {
                return Err(MsiError::InvalidEntry);
            }
        }

        // Allocate vectors for each entry (now we can borrow self mutably)
        let num_entries = entries.len() as u8;
        let vectors = self.alloc_vectors(num_entries);
        if vectors.is_empty() {
            return Err(MsiError::NoVectorsAvailable);
        }

        // Get BAR address for table
        let (bar_base, _is_io) = pci::read_bar(dev, msix_cap.table_bir);
        let table_base = bar_base + msix_cap.table_offset as u64;

        // Enable MSI-X (but keep function masked)
        let mut ctrl = pci::read_u16(dev.addr.bus, dev.addr.device, dev.addr.function, msix_cap.offset + MSIX_CAP_CONTROL);
        ctrl |= MSIX_CTRL_ENABLE | MSIX_CTRL_FUNCTION_MASK;
        pci::write_u16(dev.addr.bus, dev.addr.device, dev.addr.function, msix_cap.offset + MSIX_CAP_CONTROL, ctrl);

        // Configure each entry
        let mut result = Vec::with_capacity(entries.len());
        for (i, (entry, target_cpu)) in entries.iter().enumerate() {
            let vector = vectors[i];
            configure_msix_entry(table_base, *entry, vector, *target_cpu)?;
            result.push((*entry, vector));
        }

        // Clear function mask
        ctrl &= !MSIX_CTRL_FUNCTION_MASK;
        pci::write_u16(dev.addr.bus, dev.addr.device, dev.addr.function, msix_cap.offset + MSIX_CAP_CONTROL, ctrl);

        // Update config
        if let Some(config) = self.devices.get_mut(&key) {
            config.vectors = vectors;
            config.msix_enabled = true;
            config.msi_enabled = false;
        }

        Ok(result)
    }

    /// Disable MSI for a device
    pub fn disable_msi(&mut self, dev: &PciDevice) {
        let key = Self::device_key(dev);

        // Get vectors and capability info first
        let (vectors_to_free, msi_cap, was_enabled) = {
            if let Some(config) = self.devices.get(&key) {
                if config.msi_enabled {
                    (config.vectors.clone(), config.msi, true)
                } else {
                    (Vec::new(), None, false)
                }
            } else {
                (Vec::new(), None, false)
            }
        };

        if was_enabled {
            // Disable MSI in hardware
            if let Some(msi_cap) = msi_cap {
                let ctrl = pci::read_u16(dev.addr.bus, dev.addr.device, dev.addr.function, msi_cap.offset + MSI_CAP_CONTROL);
                pci::write_u16(dev.addr.bus, dev.addr.device, dev.addr.function, msi_cap.offset + MSI_CAP_CONTROL, ctrl & !MSI_CTRL_ENABLE);
            }

            // Free vectors
            for vec in &vectors_to_free {
                self.free_vector(*vec);
            }

            // Update config
            if let Some(config) = self.devices.get_mut(&key) {
                config.vectors.clear();
                config.msi_enabled = false;
            }
        }
    }

    /// Disable MSI-X for a device
    pub fn disable_msix(&mut self, dev: &PciDevice) {
        let key = Self::device_key(dev);

        // Get vectors and capability info first
        let (vectors_to_free, msix_cap, was_enabled) = {
            if let Some(config) = self.devices.get(&key) {
                if config.msix_enabled {
                    (config.vectors.clone(), config.msix, true)
                } else {
                    (Vec::new(), None, false)
                }
            } else {
                (Vec::new(), None, false)
            }
        };

        if was_enabled {
            // Disable MSI-X in hardware
            if let Some(msix_cap) = msix_cap {
                let ctrl = pci::read_u16(dev.addr.bus, dev.addr.device, dev.addr.function, msix_cap.offset + MSIX_CAP_CONTROL);
                pci::write_u16(dev.addr.bus, dev.addr.device, dev.addr.function, msix_cap.offset + MSIX_CAP_CONTROL, ctrl & !MSIX_CTRL_ENABLE);
            }

            // Free vectors
            for vec in &vectors_to_free {
                self.free_vector(*vec);
            }

            // Update config
            if let Some(config) = self.devices.get_mut(&key) {
                config.vectors.clear();
                config.msix_enabled = false;
            }
        }
    }

    /// Get MSI configuration for a device
    pub fn get_config(&self, dev: &PciDevice) -> Option<&MsiConfig> {
        let key = Self::device_key(dev);
        self.devices.get(&key)
    }

    /// Mask a specific MSI-X entry
    pub fn mask_msix_entry(&self, dev: &PciDevice, entry: u16) {
        let key = Self::device_key(dev);
        if let Some(config) = self.devices.get(&key) {
            if let Some(msix_cap) = config.msix {
                let (bar_base, _) = pci::read_bar(&config.device, msix_cap.table_bir);
                let table_base = bar_base + msix_cap.table_offset as u64;
                let entry_addr = table_base + (entry as u64) * MSIX_ENTRY_SIZE as u64;

                unsafe {
                    let virt = crate::mm::phys_to_virt(x86_64::PhysAddr::new(entry_addr + MSIX_ENTRY_CTRL as u64));
                    let ctrl_ptr = virt.as_mut_ptr::<u32>();
                    *ctrl_ptr |= MSIX_ENTRY_CTRL_MASKED;
                }
            }
        }
    }

    /// Unmask a specific MSI-X entry
    pub fn unmask_msix_entry(&self, dev: &PciDevice, entry: u16) {
        let key = Self::device_key(dev);
        if let Some(config) = self.devices.get(&key) {
            if let Some(msix_cap) = config.msix {
                let (bar_base, _) = pci::read_bar(&config.device, msix_cap.table_bir);
                let table_base = bar_base + msix_cap.table_offset as u64;
                let entry_addr = table_base + (entry as u64) * MSIX_ENTRY_SIZE as u64;

                unsafe {
                    let virt = crate::mm::phys_to_virt(x86_64::PhysAddr::new(entry_addr + MSIX_ENTRY_CTRL as u64));
                    let ctrl_ptr = virt.as_mut_ptr::<u32>();
                    *ctrl_ptr &= !MSIX_ENTRY_CTRL_MASKED;
                }
            }
        }
    }
}

/// Vector range for MSI
pub const MSI_VECTOR_BASE: u8 = 32;  // Start after exceptions
pub const MSI_VECTOR_MAX: u8 = 223;  // End before system vectors

/// Global MSI subsystem
static MSI_SUBSYSTEM: Mutex<MsiSubsystem> = Mutex::new(MsiSubsystem::new());

/// Initialize MSI subsystem
pub fn init() {
    crate::kprintln!("msi: initializing MSI/MSI-X subsystem");
    crate::kprintln!("msi: vector range {}-{}", MSI_VECTOR_BASE, MSI_VECTOR_MAX);
}

/// Probe device for MSI support
pub fn probe_msi(dev: &PciDevice) -> Option<MsiCapability> {
    let offset = pci::find_capability(dev, pci::PCI_CAP_ID_MSI)?;

    let ctrl = pci::read_u16(dev.addr.bus, dev.addr.device, dev.addr.function, offset + MSI_CAP_CONTROL);

    let mmc = ((ctrl & MSI_CTRL_MMC_MASK) >> MSI_CTRL_MMC_SHIFT) as u8;
    let max_vectors = 1u8 << mmc; // 2^mmc

    Some(MsiCapability {
        offset,
        max_vectors,
        is_64bit: (ctrl & MSI_CTRL_64BIT) != 0,
        per_vector_mask: (ctrl & MSI_CTRL_PER_VECTOR_MASK) != 0,
    })
}

/// Probe device for MSI-X support
pub fn probe_msix(dev: &PciDevice) -> Option<MsixCapability> {
    let offset = pci::find_capability(dev, pci::PCI_CAP_ID_MSIX)?;

    let ctrl = pci::read_u16(dev.addr.bus, dev.addr.device, dev.addr.function, offset + MSIX_CAP_CONTROL);
    let table_size = (ctrl & MSIX_CTRL_TABLE_SIZE_MASK) + 1;

    let table_reg = pci::read_u32(dev.addr.bus, dev.addr.device, dev.addr.function, offset + MSIX_CAP_TABLE);
    let table_bir = (table_reg & 0x7) as u8;
    let table_offset = table_reg & !0x7;

    let pba_reg = pci::read_u32(dev.addr.bus, dev.addr.device, dev.addr.function, offset + MSIX_CAP_PBA);
    let pba_bir = (pba_reg & 0x7) as u8;
    let pba_offset = pba_reg & !0x7;

    Some(MsixCapability {
        offset,
        table_size,
        table_bir,
        table_offset,
        pba_bir,
        pba_offset,
    })
}

/// Configure MSI for a device
fn configure_msi(
    dev: &PciDevice,
    cap: &MsiCapability,
    vector: u8,
    target_cpu: u8,
    num_vectors: u8,
) -> Result<(), MsiError> {
    // Calculate message address and data
    let msg_addr = msi_message_address(target_cpu, false);
    let msg_data = msi_message_data(vector);

    // Write message address
    pci::write_u32(
        dev.addr.bus,
        dev.addr.device,
        dev.addr.function,
        cap.offset + MSI_CAP_ADDR_LO,
        msg_addr as u32,
    );

    let data_offset = if cap.is_64bit {
        // Write high address
        pci::write_u32(
            dev.addr.bus,
            dev.addr.device,
            dev.addr.function,
            cap.offset + MSI_CAP_ADDR_HI,
            (msg_addr >> 32) as u32,
        );
        MSI_CAP_DATA_64
    } else {
        MSI_CAP_DATA_32
    };

    // Write message data
    pci::write_u16(
        dev.addr.bus,
        dev.addr.device,
        dev.addr.function,
        cap.offset + data_offset,
        msg_data as u16,
    );

    // Calculate MME (Multiple Message Enable) value
    let mme = match num_vectors {
        1 => 0,
        2 => 1,
        4 => 2,
        8 => 3,
        16 => 4,
        32 => 5,
        _ => 0,
    };

    // Enable MSI with MME
    let mut ctrl = pci::read_u16(dev.addr.bus, dev.addr.device, dev.addr.function, cap.offset + MSI_CAP_CONTROL);
    ctrl &= !MSI_CTRL_MME_MASK;
    ctrl |= (mme << MSI_CTRL_MME_SHIFT) & MSI_CTRL_MME_MASK;
    ctrl |= MSI_CTRL_ENABLE;
    pci::write_u16(dev.addr.bus, dev.addr.device, dev.addr.function, cap.offset + MSI_CAP_CONTROL, ctrl);

    Ok(())
}

/// Configure a single MSI-X table entry
fn configure_msix_entry(
    table_base: u64,
    entry: u16,
    vector: u8,
    target_cpu: u8,
) -> Result<(), MsiError> {
    let entry_addr = table_base + (entry as u64) * MSIX_ENTRY_SIZE as u64;

    let msg_addr = msi_message_address(target_cpu, false);
    let msg_data = msi_message_data(vector);

    unsafe {
        let virt = crate::mm::phys_to_virt(x86_64::PhysAddr::new(entry_addr));
        let base_ptr = virt.as_mut_ptr::<u32>();

        // Write address low
        *base_ptr.add(0) = msg_addr as u32;
        // Write address high
        *base_ptr.add(1) = (msg_addr >> 32) as u32;
        // Write data
        *base_ptr.add(2) = msg_data;
        // Clear mask bit
        *base_ptr.add(3) = 0;
    }

    Ok(())
}

/// Generate MSI message address for x86_64
pub fn msi_message_address(target_cpu: u8, logical_dest: bool) -> u64 {
    let mut addr = MSI_ADDR_BASE;
    addr |= (target_cpu as u64) << MSI_ADDR_DEST_SHIFT;
    if logical_dest {
        addr |= MSI_ADDR_DEST_LOGICAL;
    }
    addr
}

/// Generate MSI message data for x86_64
pub fn msi_message_data(vector: u8) -> u32 {
    (vector as u32) | MSI_DATA_DELIVERY_FIXED | MSI_DATA_LEVEL_ASSERT | MSI_DATA_TRIGGER_EDGE
}

/// MSI error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsiError {
    /// Device not found in MSI subsystem
    DeviceNotFound,
    /// MSI/MSI-X not supported by device
    NotSupported,
    /// No vectors available
    NoVectorsAvailable,
    /// Invalid entry index
    InvalidEntry,
    /// Configuration error
    ConfigError,
}

// ============================================================================
// Public API
// ============================================================================

/// Probe and register a device for MSI
pub fn register_device(dev: &PciDevice) -> MsiConfig {
    let mut msi = MSI_SUBSYSTEM.lock();
    msi.probe_device(dev)
}

/// Enable MSI for a device
pub fn enable_msi(dev: &PciDevice, num_vectors: u8) -> Result<Vec<u8>, MsiError> {
    let mut msi = MSI_SUBSYSTEM.lock();
    msi.enable_msi(dev, num_vectors)
}

/// Enable MSI-X for a device
/// entries: Vec of (table_entry, target_cpu)
pub fn enable_msix(dev: &PciDevice, entries: &[(u16, u8)]) -> Result<Vec<(u16, u8)>, MsiError> {
    let mut msi = MSI_SUBSYSTEM.lock();
    msi.enable_msix(dev, entries)
}

/// Disable MSI for a device
pub fn disable_msi(dev: &PciDevice) {
    let mut msi = MSI_SUBSYSTEM.lock();
    msi.disable_msi(dev);
}

/// Disable MSI-X for a device
pub fn disable_msix(dev: &PciDevice) {
    let mut msi = MSI_SUBSYSTEM.lock();
    msi.disable_msix(dev);
}

/// Get MSI configuration for a device
pub fn get_msi_config(dev: &PciDevice) -> Option<MsiConfig> {
    let msi = MSI_SUBSYSTEM.lock();
    msi.get_config(dev).cloned()
}

/// Check if MSI is supported
pub fn supports_msi(dev: &PciDevice) -> bool {
    probe_msi(dev).is_some()
}

/// Check if MSI-X is supported
pub fn supports_msix(dev: &PciDevice) -> bool {
    probe_msix(dev).is_some()
}

/// Mask MSI-X entry
pub fn mask_msix_entry(dev: &PciDevice, entry: u16) {
    let msi = MSI_SUBSYSTEM.lock();
    msi.mask_msix_entry(dev, entry);
}

/// Unmask MSI-X entry
pub fn unmask_msix_entry(dev: &PciDevice, entry: u16) {
    let msi = MSI_SUBSYSTEM.lock();
    msi.unmask_msix_entry(dev, entry);
}

/// Print MSI capability info
pub fn print_msi_info(dev: &PciDevice) {
    if let Some(msi) = probe_msi(dev) {
        crate::kprintln!(
            "  MSI: {} vectors, {}64-bit, {}per-vector mask",
            msi.max_vectors,
            if msi.is_64bit { "" } else { "no " },
            if msi.per_vector_mask { "" } else { "no " },
        );
    }

    if let Some(msix) = probe_msix(dev) {
        crate::kprintln!(
            "  MSI-X: {} entries, table @ BAR{}+{:#x}, PBA @ BAR{}+{:#x}",
            msix.table_size,
            msix.table_bir,
            msix.table_offset,
            msix.pba_bir,
            msix.pba_offset,
        );
    }
}

/// Allocate and configure MSI-X vectors for a device
/// Returns Vec of (entry_index, vector_number)
pub fn setup_msix_vectors(dev: &PciDevice, count: u16) -> Result<Vec<(u16, u8)>, MsiError> {
    // Create entries for CPU 0
    let entries: Vec<(u16, u8)> = (0..count).map(|i| (i, 0u8)).collect();
    enable_msix(dev, &entries)
}

/// Simple setup: enable MSI or MSI-X with specified vector count
pub fn setup_interrupts(dev: &PciDevice, preferred_count: u8) -> Result<Vec<u8>, MsiError> {
    // Prefer MSI-X over MSI
    if let Some(msix) = probe_msix(dev) {
        let count = (preferred_count as u16).min(msix.table_size);
        let entries = setup_msix_vectors(dev, count)?;
        Ok(entries.into_iter().map(|(_, v)| v).collect())
    } else if let Some(_msi) = probe_msi(dev) {
        enable_msi(dev, preferred_count)
    } else {
        Err(MsiError::NotSupported)
    }
}
