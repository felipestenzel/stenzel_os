//! NUMA (Non-Uniform Memory Access) Support
//!
//! This module provides NUMA topology detection and memory allocation policies.
//! It parses the ACPI SRAT (System Resource Affinity Table) to discover:
//! - Which CPUs belong to which NUMA nodes
//! - Memory regions and their proximity domains
//!
//! References:
//! - ACPI Specification Section 5.2.16 (SRAT)
//! - Linux kernel: mm/numa.c, arch/x86/mm/numa.c

#![allow(dead_code)]

extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::drivers::acpi;
use crate::sync::IrqSafeMutex;

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of NUMA nodes supported
pub const MAX_NUMA_NODES: usize = 64;

/// Maximum number of CPUs per NUMA node
pub const MAX_CPUS_PER_NODE: usize = 64;

/// Maximum number of memory ranges per NUMA node
pub const MAX_MEM_RANGES_PER_NODE: usize = 16;

// SRAT structure types
const SRAT_TYPE_CPU_AFFINITY: u8 = 0;
const SRAT_TYPE_MEM_AFFINITY: u8 = 1;
const SRAT_TYPE_X2APIC_AFFINITY: u8 = 2;

// ============================================================================
// SRAT Table Structures
// ============================================================================

/// SRAT header (after standard ACPI table header)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct SratHeader {
    /// Reserved (must be 1)
    reserved1: u32,
    /// Reserved
    reserved2: [u8; 8],
}

/// SRAT Processor Local APIC Affinity Structure
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct SratCpuAffinity {
    /// Structure type (0)
    structure_type: u8,
    /// Length (16)
    length: u8,
    /// Proximity domain bits [7:0]
    proximity_lo: u8,
    /// APIC ID
    apic_id: u8,
    /// Flags (bit 0 = enabled)
    flags: u32,
    /// Local SAPIC EID
    local_sapic_eid: u8,
    /// Proximity domain bits [31:8]
    proximity_hi: [u8; 3],
    /// Clock domain
    clock_domain: u32,
}

/// SRAT Memory Affinity Structure
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct SratMemAffinity {
    /// Structure type (1)
    structure_type: u8,
    /// Length (40)
    length: u8,
    /// Proximity domain
    proximity_domain: u32,
    /// Reserved
    reserved1: u16,
    /// Base address low
    base_lo: u32,
    /// Base address high
    base_hi: u32,
    /// Length low
    length_lo: u32,
    /// Length high
    length_hi: u32,
    /// Reserved
    reserved2: u32,
    /// Flags (bit 0 = enabled, bit 1 = hot-pluggable, bit 2 = non-volatile)
    flags: u32,
    /// Reserved
    reserved3: [u8; 8],
}

/// SRAT x2APIC Affinity Structure
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct SratX2ApicAffinity {
    /// Structure type (2)
    structure_type: u8,
    /// Length (24)
    length: u8,
    /// Reserved
    reserved1: [u8; 2],
    /// Proximity domain
    proximity_domain: u32,
    /// x2APIC ID
    x2apic_id: u32,
    /// Flags
    flags: u32,
    /// Clock domain
    clock_domain: u32,
    /// Reserved
    reserved2: [u8; 4],
}

// ============================================================================
// NUMA Node Information
// ============================================================================

/// Memory range within a NUMA node
#[derive(Debug, Clone, Copy, Default)]
pub struct NumaMemRange {
    /// Start address
    pub base: u64,
    /// Length in bytes
    pub length: u64,
    /// Is hot-pluggable
    pub hot_pluggable: bool,
    /// Is non-volatile memory
    pub non_volatile: bool,
}

/// Information about a single NUMA node
#[derive(Debug)]
pub struct NumaNode {
    /// Node ID (proximity domain)
    pub id: u32,
    /// Is this node present/enabled
    pub present: bool,
    /// CPUs (APIC IDs) on this node
    pub cpus: Vec<u32>,
    /// Memory ranges on this node
    pub mem_ranges: Vec<NumaMemRange>,
    /// Total memory on this node
    pub total_memory: u64,
    /// Free memory on this node (tracked)
    pub free_memory: AtomicU64,
}

impl NumaNode {
    fn new(id: u32) -> Self {
        Self {
            id,
            present: false,
            cpus: Vec::new(),
            mem_ranges: Vec::new(),
            total_memory: 0,
            free_memory: AtomicU64::new(0),
        }
    }
}

// ============================================================================
// NUMA Topology
// ============================================================================

/// Global NUMA topology information
pub struct NumaTopology {
    /// Number of NUMA nodes
    pub nr_nodes: u32,
    /// NUMA nodes
    nodes: Vec<NumaNode>,
    /// CPU to node mapping (indexed by APIC ID)
    cpu_to_node: [u32; 256],
    /// Is NUMA available
    numa_available: bool,
}

impl NumaTopology {
    const fn new() -> Self {
        Self {
            nr_nodes: 0,
            nodes: Vec::new(),
            cpu_to_node: [0; 256],
            numa_available: false,
        }
    }
}

static NUMA_TOPOLOGY: IrqSafeMutex<NumaTopology> = IrqSafeMutex::new(NumaTopology::new());

// ============================================================================
// SRAT Parsing
// ============================================================================

/// Parse SRAT table and populate NUMA topology
fn parse_srat(srat_addr: u64) -> Option<()> {
    let mut topology = NUMA_TOPOLOGY.lock();

    // Read SRAT header
    let header = unsafe {
        let ptr = srat_addr as *const acpi::AcpiTableHeader;
        &*ptr
    };

    let srat_length = header.length as usize;
    if srat_length < core::mem::size_of::<acpi::AcpiTableHeader>() + core::mem::size_of::<SratHeader>() {
        return None;
    }

    // Skip table header and SRAT header
    let mut offset = core::mem::size_of::<acpi::AcpiTableHeader>() + core::mem::size_of::<SratHeader>();

    // Initialize nodes (we'll discover them as we parse)
    for i in 0..MAX_NUMA_NODES {
        if topology.nodes.len() <= i {
            topology.nodes.push(NumaNode::new(i as u32));
        }
    }

    // Parse SRAT entries
    while offset + 2 <= srat_length {
        let entry_addr = srat_addr + offset as u64;
        let entry_type = unsafe { *(entry_addr as *const u8) };
        let entry_len = unsafe { *((entry_addr + 1) as *const u8) } as usize;

        if entry_len == 0 || offset + entry_len > srat_length {
            break;
        }

        match entry_type {
            SRAT_TYPE_CPU_AFFINITY => {
                if entry_len >= core::mem::size_of::<SratCpuAffinity>() {
                    let entry = unsafe { &*(entry_addr as *const SratCpuAffinity) };
                    let flags = entry.flags;

                    // Check if enabled
                    if flags & 1 != 0 {
                        // Calculate full proximity domain
                        let proximity = entry.proximity_lo as u32
                            | ((entry.proximity_hi[0] as u32) << 8)
                            | ((entry.proximity_hi[1] as u32) << 16)
                            | ((entry.proximity_hi[2] as u32) << 24);

                        let apic_id = entry.apic_id as u32;

                        if proximity < MAX_NUMA_NODES as u32 {
                            topology.nodes[proximity as usize].cpus.push(apic_id);
                            topology.nodes[proximity as usize].present = true;
                            if apic_id < 256 {
                                topology.cpu_to_node[apic_id as usize] = proximity;
                            }
                        }
                    }
                }
            }

            SRAT_TYPE_MEM_AFFINITY => {
                if entry_len >= core::mem::size_of::<SratMemAffinity>() {
                    let entry = unsafe { &*(entry_addr as *const SratMemAffinity) };
                    let flags = entry.flags;

                    // Check if enabled
                    if flags & 1 != 0 {
                        let proximity = entry.proximity_domain;
                        let base = (entry.base_lo as u64) | ((entry.base_hi as u64) << 32);
                        let length = (entry.length_lo as u64) | ((entry.length_hi as u64) << 32);

                        if proximity < MAX_NUMA_NODES as u32 && length > 0 {
                            let range = NumaMemRange {
                                base,
                                length,
                                hot_pluggable: flags & 2 != 0,
                                non_volatile: flags & 4 != 0,
                            };

                            topology.nodes[proximity as usize].mem_ranges.push(range);
                            topology.nodes[proximity as usize].total_memory += length;
                            topology.nodes[proximity as usize].free_memory.fetch_add(length, Ordering::Relaxed);
                            topology.nodes[proximity as usize].present = true;
                        }
                    }
                }
            }

            SRAT_TYPE_X2APIC_AFFINITY => {
                if entry_len >= core::mem::size_of::<SratX2ApicAffinity>() {
                    let entry = unsafe { &*(entry_addr as *const SratX2ApicAffinity) };
                    let flags = entry.flags;

                    // Check if enabled
                    if flags & 1 != 0 {
                        let proximity = entry.proximity_domain;
                        let x2apic_id = entry.x2apic_id;

                        if proximity < MAX_NUMA_NODES as u32 {
                            topology.nodes[proximity as usize].cpus.push(x2apic_id);
                            topology.nodes[proximity as usize].present = true;
                            if x2apic_id < 256 {
                                topology.cpu_to_node[x2apic_id as usize] = proximity;
                            }
                        }
                    }
                }
            }

            _ => {
                // Unknown entry type, skip
            }
        }

        offset += entry_len;
    }

    // Count number of nodes
    topology.nr_nodes = topology.nodes.iter().filter(|n| n.present).count() as u32;
    topology.numa_available = topology.nr_nodes > 0;

    Some(())
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize NUMA subsystem
pub fn init() {
    // Look for SRAT table
    if let Some(srat) = acpi::find_table(b"SRAT") {
        if parse_srat(srat.address).is_some() {
            let topology = NUMA_TOPOLOGY.lock();
            crate::kprintln!("numa: {} NUMA node(s) detected", topology.nr_nodes);

            for node in topology.nodes.iter().filter(|n| n.present) {
                let mem_mb = node.total_memory / (1024 * 1024);
                crate::kprintln!(
                    "numa: node {} - {} CPUs, {} MB memory",
                    node.id,
                    node.cpus.len(),
                    mem_mb
                );
            }
            return;
        }
    }

    // No NUMA information found, treat as single node
    crate::kprintln!("numa: no SRAT table found, assuming flat memory model");

    let mut topology = NUMA_TOPOLOGY.lock();
    topology.nr_nodes = 1;
    topology.numa_available = false;
    if topology.nodes.is_empty() {
        topology.nodes.push(NumaNode::new(0));
    }
    topology.nodes[0].present = true;
}

/// Check if NUMA is available
pub fn is_numa_available() -> bool {
    NUMA_TOPOLOGY.lock().numa_available
}

/// Get number of NUMA nodes
pub fn num_nodes() -> u32 {
    NUMA_TOPOLOGY.lock().nr_nodes
}

/// Get the NUMA node for a given CPU (by APIC ID)
pub fn cpu_to_node(cpu: u32) -> u32 {
    if cpu < 256 {
        NUMA_TOPOLOGY.lock().cpu_to_node[cpu as usize]
    } else {
        0
    }
}

/// Get total memory on a NUMA node
pub fn node_memory(node: u32) -> u64 {
    let topology = NUMA_TOPOLOGY.lock();
    if (node as usize) < topology.nodes.len() && topology.nodes[node as usize].present {
        topology.nodes[node as usize].total_memory
    } else {
        0
    }
}

/// Get free memory on a NUMA node
pub fn node_free_memory(node: u32) -> u64 {
    let topology = NUMA_TOPOLOGY.lock();
    if (node as usize) < topology.nodes.len() && topology.nodes[node as usize].present {
        topology.nodes[node as usize].free_memory.load(Ordering::Relaxed)
    } else {
        0
    }
}

/// Get number of CPUs on a NUMA node
pub fn node_cpus(node: u32) -> Vec<u32> {
    let topology = NUMA_TOPOLOGY.lock();
    if (node as usize) < topology.nodes.len() && topology.nodes[node as usize].present {
        topology.nodes[node as usize].cpus.clone()
    } else {
        Vec::new()
    }
}

/// Get memory ranges for a NUMA node
pub fn node_mem_ranges(node: u32) -> Vec<NumaMemRange> {
    let topology = NUMA_TOPOLOGY.lock();
    if (node as usize) < topology.nodes.len() && topology.nodes[node as usize].present {
        topology.nodes[node as usize].mem_ranges.clone()
    } else {
        Vec::new()
    }
}

/// Record memory allocation on a NUMA node
pub fn record_alloc(node: u32, size: u64) {
    let topology = NUMA_TOPOLOGY.lock();
    if (node as usize) < topology.nodes.len() && topology.nodes[node as usize].present {
        let current = topology.nodes[node as usize].free_memory.load(Ordering::Relaxed);
        if current >= size {
            topology.nodes[node as usize].free_memory.fetch_sub(size, Ordering::Relaxed);
        }
    }
}

/// Record memory deallocation on a NUMA node
pub fn record_free(node: u32, size: u64) {
    let topology = NUMA_TOPOLOGY.lock();
    if (node as usize) < topology.nodes.len() && topology.nodes[node as usize].present {
        topology.nodes[node as usize].free_memory.fetch_add(size, Ordering::Relaxed);
    }
}

/// Find the NUMA node that contains a given physical address
pub fn addr_to_node(addr: u64) -> Option<u32> {
    let topology = NUMA_TOPOLOGY.lock();
    for node in topology.nodes.iter().filter(|n| n.present) {
        for range in &node.mem_ranges {
            if addr >= range.base && addr < range.base + range.length {
                return Some(node.id);
            }
        }
    }
    None
}

/// Get node with most free memory
pub fn find_freest_node() -> u32 {
    let topology = NUMA_TOPOLOGY.lock();
    let mut best_node = 0;
    let mut best_free = 0u64;

    for node in topology.nodes.iter().filter(|n| n.present) {
        let free = node.free_memory.load(Ordering::Relaxed);
        if free > best_free {
            best_free = free;
            best_node = node.id;
        }
    }

    best_node
}

/// Get NUMA topology summary for /proc/numa
pub fn get_numa_summary() -> Vec<(u32, usize, u64, u64)> {
    let topology = NUMA_TOPOLOGY.lock();
    topology.nodes
        .iter()
        .filter(|n| n.present)
        .map(|n| (n.id, n.cpus.len(), n.total_memory, n.free_memory.load(Ordering::Relaxed)))
        .collect()
}

// ============================================================================
// Memory Allocation Policy
// ============================================================================

/// NUMA memory allocation policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumaPolicy {
    /// Allocate from the local node (CPU's node)
    Local,
    /// Prefer a specific node, fallback to others
    Preferred(u32),
    /// Interleave across all nodes (for large allocations)
    Interleave,
    /// Bind to specific nodes (for specialized use)
    Bind,
}

impl Default for NumaPolicy {
    fn default() -> Self {
        NumaPolicy::Local
    }
}

/// Get the preferred NUMA node for the current CPU
pub fn preferred_node() -> u32 {
    // In a real implementation, this would read the current CPU's APIC ID
    // and return the corresponding node. For now, return 0.
    0
}

/// Select a NUMA node based on policy
pub fn select_node(policy: NumaPolicy) -> u32 {
    match policy {
        NumaPolicy::Local => preferred_node(),
        NumaPolicy::Preferred(node) => {
            // If preferred node has memory, use it; otherwise find freest
            if node_free_memory(node) > 0 {
                node
            } else {
                find_freest_node()
            }
        }
        NumaPolicy::Interleave => {
            // Round-robin across nodes
            static INTERLEAVE_COUNTER: AtomicU32 = AtomicU32::new(0);
            let nr = num_nodes();
            if nr > 0 {
                INTERLEAVE_COUNTER.fetch_add(1, Ordering::Relaxed) % nr
            } else {
                0
            }
        }
        NumaPolicy::Bind => preferred_node(),
    }
}
