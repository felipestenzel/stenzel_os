//! Device Tree Subsystem
//!
//! Provides hardware description through a hierarchical tree structure.
//! Supports:
//! - Device Tree Blob (DTB) parsing (FDT format)
//! - Dynamic device tree construction
//! - Property query API
//! - Compatible string matching for driver binding
//! - ACPI integration for x86 platforms

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use crate::sync::TicketSpinlock;
use core::fmt;

/// Global device tree instance
static DEVICE_TREE: TicketSpinlock<Option<DeviceTree>> = TicketSpinlock::new(None);

/// FDT Magic number
const FDT_MAGIC: u32 = 0xD00DFEED;

/// FDT Token types
const FDT_BEGIN_NODE: u32 = 0x01;
const FDT_END_NODE: u32 = 0x02;
const FDT_PROP: u32 = 0x03;
const FDT_NOP: u32 = 0x04;
const FDT_END: u32 = 0x09;

/// Device Tree property value types
#[derive(Debug, Clone)]
pub enum PropertyValue {
    Empty,
    U32(u32),
    U64(u64),
    String(String),
    StringList(Vec<String>),
    Bytes(Vec<u8>),
    U32Array(Vec<u32>),
    U64Array(Vec<u64>),
    PHandle(u32),
    Reg(Vec<RegEntry>),
}

/// Register entry (address, size pairs)
#[derive(Debug, Clone, Copy)]
pub struct RegEntry {
    pub address: u64,
    pub size: u64,
}

/// Device tree property
#[derive(Debug, Clone)]
pub struct Property {
    pub name: String,
    pub value: PropertyValue,
    pub raw: Vec<u8>,
}

impl Property {
    pub fn new(name: &str, value: PropertyValue) -> Self {
        let raw = match &value {
            PropertyValue::Empty => Vec::new(),
            PropertyValue::U32(v) => v.to_be_bytes().to_vec(),
            PropertyValue::U64(v) => v.to_be_bytes().to_vec(),
            PropertyValue::String(s) => {
                let mut v = s.as_bytes().to_vec();
                v.push(0);
                v
            }
            PropertyValue::StringList(list) => {
                let mut v = Vec::new();
                for s in list {
                    v.extend_from_slice(s.as_bytes());
                    v.push(0);
                }
                v
            }
            PropertyValue::Bytes(b) => b.clone(),
            PropertyValue::U32Array(arr) => {
                let mut v = Vec::new();
                for val in arr {
                    v.extend_from_slice(&val.to_be_bytes());
                }
                v
            }
            PropertyValue::U64Array(arr) => {
                let mut v = Vec::new();
                for val in arr {
                    v.extend_from_slice(&val.to_be_bytes());
                }
                v
            }
            PropertyValue::PHandle(p) => p.to_be_bytes().to_vec(),
            PropertyValue::Reg(entries) => {
                let mut v = Vec::new();
                for e in entries {
                    v.extend_from_slice(&e.address.to_be_bytes());
                    v.extend_from_slice(&e.size.to_be_bytes());
                }
                v
            }
        };
        Self {
            name: name.to_string(),
            value,
            raw,
        }
    }

    /// Get property as u32
    pub fn as_u32(&self) -> Option<u32> {
        match &self.value {
            PropertyValue::U32(v) => Some(*v),
            PropertyValue::U64(v) => Some(*v as u32),
            _ => {
                if self.raw.len() >= 4 {
                    Some(u32::from_be_bytes([
                        self.raw[0], self.raw[1], self.raw[2], self.raw[3]
                    ]))
                } else {
                    None
                }
            }
        }
    }

    /// Get property as u64
    pub fn as_u64(&self) -> Option<u64> {
        match &self.value {
            PropertyValue::U64(v) => Some(*v),
            PropertyValue::U32(v) => Some(*v as u64),
            _ => {
                if self.raw.len() >= 8 {
                    Some(u64::from_be_bytes([
                        self.raw[0], self.raw[1], self.raw[2], self.raw[3],
                        self.raw[4], self.raw[5], self.raw[6], self.raw[7]
                    ]))
                } else if self.raw.len() >= 4 {
                    Some(u32::from_be_bytes([
                        self.raw[0], self.raw[1], self.raw[2], self.raw[3]
                    ]) as u64)
                } else {
                    None
                }
            }
        }
    }

    /// Get property as string
    pub fn as_string(&self) -> Option<&str> {
        match &self.value {
            PropertyValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Get property as string list
    pub fn as_string_list(&self) -> Option<&[String]> {
        match &self.value {
            PropertyValue::StringList(list) => Some(list.as_slice()),
            PropertyValue::String(s) => None, // Single string, not a list
            _ => None,
        }
    }

    /// Get property as bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.raw
    }
}

/// Device tree node
#[derive(Debug, Clone)]
pub struct DeviceNode {
    pub name: String,
    pub unit_address: Option<String>,
    pub properties: BTreeMap<String, Property>,
    pub children: BTreeMap<String, DeviceNode>,
    pub phandle: Option<u32>,
    pub path: String,
}

impl DeviceNode {
    /// Create a new device node
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            unit_address: None,
            properties: BTreeMap::new(),
            children: BTreeMap::new(),
            phandle: None,
            path: String::new(),
        }
    }

    /// Create node with unit address
    pub fn with_unit_address(name: &str, unit_address: &str) -> Self {
        Self {
            name: name.to_string(),
            unit_address: Some(unit_address.to_string()),
            properties: BTreeMap::new(),
            children: BTreeMap::new(),
            phandle: None,
            path: String::new(),
        }
    }

    /// Get full node name (name@unit_address)
    pub fn full_name(&self) -> String {
        match &self.unit_address {
            Some(addr) => alloc::format!("{}@{}", self.name, addr),
            None => self.name.clone(),
        }
    }

    /// Add a property
    pub fn add_property(&mut self, prop: Property) {
        // Handle special phandle property
        if prop.name == "phandle" || prop.name == "linux,phandle" {
            if let Some(p) = prop.as_u32() {
                self.phandle = Some(p);
            }
        }
        self.properties.insert(prop.name.clone(), prop);
    }

    /// Get a property
    pub fn get_property(&self, name: &str) -> Option<&Property> {
        self.properties.get(name)
    }

    /// Check if node has property
    pub fn has_property(&self, name: &str) -> bool {
        self.properties.contains_key(name)
    }

    /// Add a child node
    pub fn add_child(&mut self, child: DeviceNode) {
        self.children.insert(child.full_name(), child);
    }

    /// Get a child node
    pub fn get_child(&self, name: &str) -> Option<&DeviceNode> {
        self.children.get(name)
    }

    /// Get child by path segment (handles @unit_address)
    pub fn get_child_by_name(&self, name: &str) -> Option<&DeviceNode> {
        // Try exact match first
        if let Some(node) = self.children.get(name) {
            return Some(node);
        }
        // Try matching just the node name (without unit address)
        for (full_name, node) in &self.children {
            if node.name == name || full_name.starts_with(&alloc::format!("{}@", name)) {
                return Some(node);
            }
        }
        None
    }

    /// Get mutable child by name
    pub fn get_child_mut(&mut self, name: &str) -> Option<&mut DeviceNode> {
        self.children.get_mut(name)
    }

    /// Check compatible property
    pub fn is_compatible(&self, compat: &str) -> bool {
        if let Some(prop) = self.get_property("compatible") {
            match &prop.value {
                PropertyValue::String(s) => s == compat,
                PropertyValue::StringList(list) => list.iter().any(|s| s == compat),
                _ => {
                    // Parse raw bytes as null-terminated strings
                    let s = String::from_utf8_lossy(&prop.raw);
                    s.split('\0').any(|part| part == compat)
                }
            }
        } else {
            false
        }
    }

    /// Get all compatible strings
    pub fn get_compatible(&self) -> Vec<String> {
        if let Some(prop) = self.get_property("compatible") {
            match &prop.value {
                PropertyValue::String(s) => vec![s.clone()],
                PropertyValue::StringList(list) => list.clone(),
                _ => {
                    // Parse raw bytes
                    let s = String::from_utf8_lossy(&prop.raw);
                    s.split('\0')
                        .filter(|p| !p.is_empty())
                        .map(|p| p.to_string())
                        .collect()
                }
            }
        } else {
            Vec::new()
        }
    }

    /// Get reg property as RegEntry list
    pub fn get_reg(&self) -> Vec<RegEntry> {
        if let Some(prop) = self.get_property("reg") {
            match &prop.value {
                PropertyValue::Reg(entries) => entries.clone(),
                _ => {
                    // Parse from raw bytes (assuming 64-bit address and size cells)
                    let mut entries = Vec::new();
                    let bytes = &prop.raw;
                    let mut offset = 0;
                    while offset + 16 <= bytes.len() {
                        let address = u64::from_be_bytes([
                            bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3],
                            bytes[offset + 4], bytes[offset + 5], bytes[offset + 6], bytes[offset + 7]
                        ]);
                        let size = u64::from_be_bytes([
                            bytes[offset + 8], bytes[offset + 9], bytes[offset + 10], bytes[offset + 11],
                            bytes[offset + 12], bytes[offset + 13], bytes[offset + 14], bytes[offset + 15]
                        ]);
                        entries.push(RegEntry { address, size });
                        offset += 16;
                    }
                    entries
                }
            }
        } else {
            Vec::new()
        }
    }

    /// Get status property (defaults to "okay")
    pub fn get_status(&self) -> &str {
        if let Some(prop) = self.get_property("status") {
            prop.as_string().unwrap_or("okay")
        } else {
            "okay"
        }
    }

    /// Check if device is enabled
    pub fn is_enabled(&self) -> bool {
        let status = self.get_status();
        status == "okay" || status == "ok"
    }

    /// Iterate over all children
    pub fn iter_children(&self) -> impl Iterator<Item = &DeviceNode> {
        self.children.values()
    }

    /// Find nodes matching a compatible string
    pub fn find_compatible(&self, compat: &str) -> Vec<&DeviceNode> {
        let mut results = Vec::new();
        if self.is_compatible(compat) {
            results.push(self);
        }
        for child in self.children.values() {
            results.extend(child.find_compatible(compat));
        }
        results
    }

    /// Find all nodes with a specific property
    pub fn find_with_property(&self, prop_name: &str) -> Vec<&DeviceNode> {
        let mut results = Vec::new();
        if self.has_property(prop_name) {
            results.push(self);
        }
        for child in self.children.values() {
            results.extend(child.find_with_property(prop_name));
        }
        results
    }

    /// Count total nodes in subtree
    pub fn node_count(&self) -> usize {
        1 + self.children.values().map(|c| c.node_count()).sum::<usize>()
    }
}

impl fmt::Display for DeviceNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.full_name())
    }
}

/// Device Tree structure
pub struct DeviceTree {
    pub root: DeviceNode,
    pub phandle_map: BTreeMap<u32, String>,
    pub aliases: BTreeMap<String, String>,
    pub boot_args: Option<String>,
    pub model: Option<String>,
    pub compatible: Vec<String>,
}

impl DeviceTree {
    /// Create an empty device tree
    pub fn new() -> Self {
        let mut root = DeviceNode::new("");
        root.path = "/".to_string();
        Self {
            root,
            phandle_map: BTreeMap::new(),
            aliases: BTreeMap::new(),
            boot_args: None,
            model: None,
            compatible: Vec::new(),
        }
    }

    /// Parse DTB (Flattened Device Tree) from memory
    pub fn from_dtb(data: &[u8]) -> Result<Self, DtError> {
        if data.len() < 40 {
            return Err(DtError::InvalidHeader);
        }

        // Parse FDT header
        let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        if magic != FDT_MAGIC {
            return Err(DtError::BadMagic(magic));
        }

        let totalsize = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let off_dt_struct = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;
        let off_dt_strings = u32::from_be_bytes([data[12], data[13], data[14], data[15]]) as usize;
        let _off_mem_rsvmap = u32::from_be_bytes([data[16], data[17], data[18], data[19]]) as usize;
        let version = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        let _last_comp_version = u32::from_be_bytes([data[24], data[25], data[26], data[27]]);
        let _boot_cpuid_phys = u32::from_be_bytes([data[28], data[29], data[30], data[31]]);
        let size_dt_strings = u32::from_be_bytes([data[32], data[33], data[34], data[35]]) as usize;
        let _size_dt_struct = u32::from_be_bytes([data[36], data[37], data[38], data[39]]) as usize;

        if totalsize > data.len() {
            return Err(DtError::TruncatedData);
        }

        if version < 16 {
            return Err(DtError::UnsupportedVersion(version));
        }

        let strings = &data[off_dt_strings..off_dt_strings + size_dt_strings];
        let structure = &data[off_dt_struct..];

        let mut dt = DeviceTree::new();
        let mut parser = DtbParser::new(structure, strings);
        parser.parse_node(&mut dt.root, "/")?;

        // Build phandle map and extract special properties
        dt.build_phandle_map(&dt.root.clone(), "/");
        dt.extract_aliases();
        dt.extract_root_props();

        Ok(dt)
    }

    /// Build from ACPI tables (for x86)
    pub fn from_acpi() -> Self {
        let mut dt = DeviceTree::new();

        // Set model and compatible for x86
        dt.root.add_property(Property::new("model", PropertyValue::String("x86_64 PC".to_string())));
        dt.root.add_property(Property::new("compatible", PropertyValue::StringList(vec![
            "x86_64".to_string(),
            "pc".to_string(),
        ])));
        dt.model = Some("x86_64 PC".to_string());
        dt.compatible = vec!["x86_64".to_string(), "pc".to_string()];

        // Add CPU nodes
        let mut cpus = DeviceNode::new("cpus");
        cpus.add_property(Property::new("#address-cells", PropertyValue::U32(1)));
        cpus.add_property(Property::new("#size-cells", PropertyValue::U32(0)));

        // Detect CPU count from APIC or assume 1
        let cpu_count = crate::arch::cpu_count();
        for i in 0..cpu_count {
            let mut cpu = DeviceNode::with_unit_address("cpu", &alloc::format!("{}", i));
            cpu.add_property(Property::new("device_type", PropertyValue::String("cpu".to_string())));
            cpu.add_property(Property::new("compatible", PropertyValue::String("x86_64".to_string())));
            cpu.add_property(Property::new("reg", PropertyValue::U32(i as u32)));
            cpus.add_child(cpu);
        }
        dt.root.add_child(cpus);

        // Add memory node
        if let Some((base, size)) = get_memory_region() {
            let mut memory = DeviceNode::with_unit_address("memory", &alloc::format!("{:x}", base));
            memory.add_property(Property::new("device_type", PropertyValue::String("memory".to_string())));
            memory.add_property(Property::new("reg", PropertyValue::Reg(vec![
                RegEntry { address: base, size }
            ])));
            dt.root.add_child(memory);
        }

        // Add PCI node
        let mut pci = DeviceNode::with_unit_address("pci", "0");
        pci.add_property(Property::new("compatible", PropertyValue::StringList(vec![
            "pci-host-ecam-generic".to_string(),
            "pci".to_string(),
        ])));
        pci.add_property(Property::new("device_type", PropertyValue::String("pci".to_string())));
        pci.add_property(Property::new("#address-cells", PropertyValue::U32(3)));
        pci.add_property(Property::new("#size-cells", PropertyValue::U32(2)));
        pci.add_property(Property::new("bus-range", PropertyValue::U32Array(vec![0, 255])));

        // Add discovered PCI devices
        let devices = crate::drivers::pci::scan();
        for dev in devices {
            let mut pci_dev = DeviceNode::with_unit_address(
                &get_pci_device_name(dev.class.class_code, dev.class.subclass),
                &alloc::format!("{:04x},{:04x}", dev.id.vendor_id, dev.id.device_id)
            );
            pci_dev.add_property(Property::new("vendor-id", PropertyValue::U32(dev.id.vendor_id as u32)));
            pci_dev.add_property(Property::new("device-id", PropertyValue::U32(dev.id.device_id as u32)));
            pci_dev.add_property(Property::new("class", PropertyValue::U32(dev.class.class_code as u32)));
            pci_dev.add_property(Property::new("subclass", PropertyValue::U32(dev.class.subclass as u32)));
            pci_dev.add_property(Property::new("bus", PropertyValue::U32(dev.addr.bus as u32)));
            pci_dev.add_property(Property::new("device", PropertyValue::U32(dev.addr.device as u32)));
            pci_dev.add_property(Property::new("function", PropertyValue::U32(dev.addr.function as u32)));
            pci.add_child(pci_dev);
        }
        dt.root.add_child(pci);

        // Add chosen node with boot args
        let mut chosen = DeviceNode::new("chosen");
        chosen.add_property(Property::new("bootargs", PropertyValue::String(String::new())));
        chosen.add_property(Property::new("stdout-path", PropertyValue::String("/serial@3f8".to_string())));
        dt.root.add_child(chosen);

        // Add serial port
        let mut serial = DeviceNode::with_unit_address("serial", "3f8");
        serial.add_property(Property::new("compatible", PropertyValue::String("ns16550a".to_string())));
        serial.add_property(Property::new("reg", PropertyValue::Reg(vec![
            RegEntry { address: 0x3f8, size: 8 }
        ])));
        serial.add_property(Property::new("clock-frequency", PropertyValue::U32(1843200)));
        dt.root.add_child(serial);

        // Add PS/2 controller
        let mut ps2 = DeviceNode::with_unit_address("ps2", "60");
        ps2.add_property(Property::new("compatible", PropertyValue::StringList(vec![
            "i8042".to_string(),
            "ps2-controller".to_string(),
        ])));
        ps2.add_property(Property::new("reg", PropertyValue::Reg(vec![
            RegEntry { address: 0x60, size: 1 },
            RegEntry { address: 0x64, size: 1 },
        ])));
        dt.root.add_child(ps2);

        // Add RTC
        let mut rtc = DeviceNode::with_unit_address("rtc", "70");
        rtc.add_property(Property::new("compatible", PropertyValue::String("motorola,mc146818".to_string())));
        rtc.add_property(Property::new("reg", PropertyValue::Reg(vec![
            RegEntry { address: 0x70, size: 2 }
        ])));
        dt.root.add_child(rtc);

        // Add aliases
        dt.aliases.insert("serial0".to_string(), "/serial@3f8".to_string());
        dt.aliases.insert("console".to_string(), "/serial@3f8".to_string());

        dt
    }

    /// Build phandle map recursively
    fn build_phandle_map(&mut self, node: &DeviceNode, path: &str) {
        if let Some(phandle) = node.phandle {
            self.phandle_map.insert(phandle, path.to_string());
        }
        for child in node.children.values() {
            let child_path = if path == "/" {
                alloc::format!("/{}", child.full_name())
            } else {
                alloc::format!("{}/{}", path, child.full_name())
            };
            self.build_phandle_map(child, &child_path);
        }
    }

    /// Extract aliases from /aliases node
    fn extract_aliases(&mut self) {
        if let Some(aliases_node) = self.root.get_child("aliases") {
            for (name, prop) in &aliases_node.properties {
                if let Some(path) = prop.as_string() {
                    self.aliases.insert(name.clone(), path.to_string());
                }
            }
        }
    }

    /// Extract root properties
    fn extract_root_props(&mut self) {
        if let Some(prop) = self.root.get_property("model") {
            self.model = prop.as_string().map(|s| s.to_string());
        }
        self.compatible = self.root.get_compatible();

        // Extract boot args from /chosen
        if let Some(chosen) = self.root.get_child("chosen") {
            if let Some(prop) = chosen.get_property("bootargs") {
                self.boot_args = prop.as_string().map(|s| s.to_string());
            }
        }
    }

    /// Get node by path
    pub fn get_node(&self, path: &str) -> Option<&DeviceNode> {
        if path == "/" || path.is_empty() {
            return Some(&self.root);
        }

        // Handle aliases
        let resolved_path = if let Some(alias_path) = self.aliases.get(path.trim_start_matches('/')) {
            alias_path.as_str()
        } else {
            path
        };

        let mut current = &self.root;
        for segment in resolved_path.trim_start_matches('/').split('/') {
            if segment.is_empty() {
                continue;
            }
            current = current.get_child_by_name(segment)?;
        }
        Some(current)
    }

    /// Get node by phandle
    pub fn get_node_by_phandle(&self, phandle: u32) -> Option<&DeviceNode> {
        let path = self.phandle_map.get(&phandle)?;
        self.get_node(path)
    }

    /// Find all nodes with compatible string
    pub fn find_compatible(&self, compat: &str) -> Vec<&DeviceNode> {
        self.root.find_compatible(compat)
    }

    /// Find nodes by device type
    pub fn find_by_type(&self, device_type: &str) -> Vec<&DeviceNode> {
        self.root.find_with_property("device_type")
            .into_iter()
            .filter(|n| {
                n.get_property("device_type")
                    .and_then(|p| p.as_string())
                    .map(|t| t == device_type)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Get total number of nodes
    pub fn node_count(&self) -> usize {
        self.root.node_count()
    }

    /// Print tree structure
    pub fn print(&self) {
        crate::kprintln!("Device Tree:");
        if let Some(model) = &self.model {
            crate::kprintln!("  Model: {}", model);
        }
        if !self.compatible.is_empty() {
            crate::kprintln!("  Compatible: {:?}", self.compatible);
        }
        crate::kprintln!("  Nodes: {}", self.node_count());
        self.print_node(&self.root, 0);
    }

    fn print_node(&self, node: &DeviceNode, depth: usize) {
        let indent = "  ".repeat(depth + 1);
        crate::kprintln!("{}{}", indent, node.full_name());
        for child in node.children.values() {
            self.print_node(child, depth + 1);
        }
    }
}

/// DTB Parser state
struct DtbParser<'a> {
    structure: &'a [u8],
    strings: &'a [u8],
    offset: usize,
}

impl<'a> DtbParser<'a> {
    fn new(structure: &'a [u8], strings: &'a [u8]) -> Self {
        Self {
            structure,
            strings,
            offset: 0,
        }
    }

    fn read_u32(&mut self) -> Result<u32, DtError> {
        if self.offset + 4 > self.structure.len() {
            return Err(DtError::TruncatedData);
        }
        let val = u32::from_be_bytes([
            self.structure[self.offset],
            self.structure[self.offset + 1],
            self.structure[self.offset + 2],
            self.structure[self.offset + 3],
        ]);
        self.offset += 4;
        Ok(val)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], DtError> {
        if self.offset + len > self.structure.len() {
            return Err(DtError::TruncatedData);
        }
        let data = &self.structure[self.offset..self.offset + len];
        self.offset += len;
        // Align to 4 bytes
        self.offset = (self.offset + 3) & !3;
        Ok(data)
    }

    fn get_string(&self, offset: usize) -> Result<&'a str, DtError> {
        let start = offset;
        let mut end = start;
        while end < self.strings.len() && self.strings[end] != 0 {
            end += 1;
        }
        core::str::from_utf8(&self.strings[start..end])
            .map_err(|_| DtError::InvalidString)
    }

    fn read_node_name(&mut self) -> Result<&'a str, DtError> {
        let start = self.offset;
        while self.offset < self.structure.len() && self.structure[self.offset] != 0 {
            self.offset += 1;
        }
        let name = core::str::from_utf8(&self.structure[start..self.offset])
            .map_err(|_| DtError::InvalidString)?;
        self.offset += 1; // Skip null terminator
        // Align to 4 bytes
        self.offset = (self.offset + 3) & !3;
        Ok(name)
    }

    fn parse_node(&mut self, node: &mut DeviceNode, path: &str) -> Result<(), DtError> {
        node.path = path.to_string();

        loop {
            let token = self.read_u32()?;
            match token {
                FDT_BEGIN_NODE => {
                    let name = self.read_node_name()?;
                    let (node_name, unit_addr) = if let Some(pos) = name.find('@') {
                        (&name[..pos], Some(&name[pos + 1..]))
                    } else {
                        (name, None)
                    };

                    let mut child = if let Some(addr) = unit_addr {
                        DeviceNode::with_unit_address(node_name, addr)
                    } else {
                        DeviceNode::new(node_name)
                    };

                    let child_path = if path == "/" {
                        alloc::format!("/{}", name)
                    } else {
                        alloc::format!("{}/{}", path, name)
                    };

                    self.parse_node(&mut child, &child_path)?;
                    node.add_child(child);
                }
                FDT_END_NODE => {
                    return Ok(());
                }
                FDT_PROP => {
                    let len = self.read_u32()? as usize;
                    let nameoff = self.read_u32()? as usize;
                    let data = self.read_bytes(len)?;
                    let name = self.get_string(nameoff)?;

                    let value = Self::parse_property_value(name, data);
                    node.add_property(Property {
                        name: name.to_string(),
                        value,
                        raw: data.to_vec(),
                    });
                }
                FDT_NOP => continue,
                FDT_END => return Ok(()),
                _ => return Err(DtError::InvalidToken(token)),
            }
        }
    }

    fn parse_property_value(name: &str, data: &[u8]) -> PropertyValue {
        // Infer type from property name and data
        match name {
            "compatible" | "model" | "status" | "device_type" | "bootargs" | "stdout-path" => {
                // String or string list
                let s = String::from_utf8_lossy(data);
                let strings: Vec<String> = s.split('\0')
                    .filter(|p| !p.is_empty())
                    .map(|p| p.to_string())
                    .collect();
                if strings.len() == 1 {
                    PropertyValue::String(strings.into_iter().next().unwrap())
                } else {
                    PropertyValue::StringList(strings)
                }
            }
            "phandle" | "linux,phandle" => {
                if data.len() >= 4 {
                    PropertyValue::PHandle(u32::from_be_bytes([
                        data[0], data[1], data[2], data[3]
                    ]))
                } else {
                    PropertyValue::Bytes(data.to_vec())
                }
            }
            "#address-cells" | "#size-cells" | "#interrupt-cells" | "interrupt-parent" => {
                if data.len() >= 4 {
                    PropertyValue::U32(u32::from_be_bytes([
                        data[0], data[1], data[2], data[3]
                    ]))
                } else {
                    PropertyValue::Bytes(data.to_vec())
                }
            }
            _ if data.is_empty() => PropertyValue::Empty,
            _ if data.len() == 4 => {
                PropertyValue::U32(u32::from_be_bytes([
                    data[0], data[1], data[2], data[3]
                ]))
            }
            _ if data.len() == 8 => {
                PropertyValue::U64(u64::from_be_bytes([
                    data[0], data[1], data[2], data[3],
                    data[4], data[5], data[6], data[7]
                ]))
            }
            _ => PropertyValue::Bytes(data.to_vec()),
        }
    }
}

/// Device Tree errors
#[derive(Debug)]
pub enum DtError {
    InvalidHeader,
    BadMagic(u32),
    TruncatedData,
    UnsupportedVersion(u32),
    InvalidToken(u32),
    InvalidString,
    NodeNotFound,
    PropertyNotFound,
}

impl fmt::Display for DtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DtError::InvalidHeader => write!(f, "Invalid DTB header"),
            DtError::BadMagic(m) => write!(f, "Bad DTB magic: {:08x}", m),
            DtError::TruncatedData => write!(f, "Truncated DTB data"),
            DtError::UnsupportedVersion(v) => write!(f, "Unsupported DTB version: {}", v),
            DtError::InvalidToken(t) => write!(f, "Invalid DTB token: {:08x}", t),
            DtError::InvalidString => write!(f, "Invalid UTF-8 string in DTB"),
            DtError::NodeNotFound => write!(f, "Node not found"),
            DtError::PropertyNotFound => write!(f, "Property not found"),
        }
    }
}

/// Get memory region from mm module
fn get_memory_region() -> Option<(u64, u64)> {
    // Get total memory from frame allocator stats
    let stats = crate::mm::frame_allocator_stats();
    let total_bytes = (stats.total as u64) * 4096;
    if total_bytes > 0 {
        Some((0, total_bytes))
    } else {
        None
    }
}

/// Get PCI device name from class/subclass
fn get_pci_device_name(class: u8, subclass: u8) -> String {
    match (class, subclass) {
        (0x01, 0x01) => "ide".to_string(),
        (0x01, 0x06) => "sata".to_string(),
        (0x01, 0x08) => "nvme".to_string(),
        (0x02, 0x00) => "ethernet".to_string(),
        (0x02, 0x80) => "network".to_string(),
        (0x03, 0x00) => "vga".to_string(),
        (0x03, 0x02) => "3d".to_string(),
        (0x04, 0x01) => "audio".to_string(),
        (0x04, 0x03) => "hda".to_string(),
        (0x06, 0x00) => "host".to_string(),
        (0x06, 0x01) => "isa".to_string(),
        (0x06, 0x04) => "pci-bridge".to_string(),
        (0x0c, 0x03) => "usb".to_string(),
        (0x0c, 0x05) => "smbus".to_string(),
        _ => alloc::format!("device-{:02x}{:02x}", class, subclass),
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize device tree subsystem
pub fn init() {
    crate::kprintln!("devicetree: initializing device tree subsystem");

    // Build device tree from ACPI/hardware probing
    let dt = DeviceTree::from_acpi();
    let node_count = dt.node_count();

    let mut global_dt = DEVICE_TREE.lock();
    *global_dt = Some(dt);

    crate::kprintln!("devicetree: initialized with {} nodes", node_count);
}

/// Initialize from DTB blob (for systems with firmware-provided DTB)
pub fn init_from_dtb(data: &[u8]) -> Result<(), DtError> {
    crate::kprintln!("devicetree: parsing DTB ({} bytes)", data.len());

    let dt = DeviceTree::from_dtb(data)?;
    let node_count = dt.node_count();

    let mut global_dt = DEVICE_TREE.lock();
    *global_dt = Some(dt);

    crate::kprintln!("devicetree: loaded {} nodes from DTB", node_count);
    Ok(())
}

/// Get reference to global device tree
pub fn get() -> Option<&'static DeviceTree> {
    // This is safe because once initialized, the tree is never modified
    unsafe {
        let dt = DEVICE_TREE.lock();
        dt.as_ref().map(|d| &*(d as *const DeviceTree))
    }
}

/// Get node by path
pub fn get_node(path: &str) -> Option<&'static DeviceNode> {
    get().and_then(|dt| dt.get_node(path))
        .map(|n| unsafe { &*(n as *const DeviceNode) })
}

/// Find all nodes compatible with a string
pub fn find_compatible(compat: &str) -> Vec<&'static DeviceNode> {
    get().map(|dt| {
        dt.find_compatible(compat)
            .into_iter()
            .map(|n| unsafe { &*(n as *const DeviceNode) })
            .collect()
    }).unwrap_or_default()
}

/// Find nodes by device type
pub fn find_by_type(device_type: &str) -> Vec<&'static DeviceNode> {
    get().map(|dt| {
        dt.find_by_type(device_type)
            .into_iter()
            .map(|n| unsafe { &*(n as *const DeviceNode) })
            .collect()
    }).unwrap_or_default()
}

/// Print the device tree
pub fn print() {
    if let Some(dt) = get() {
        dt.print();
    } else {
        crate::kprintln!("devicetree: not initialized");
    }
}

/// Get boot arguments from device tree
pub fn get_bootargs() -> Option<String> {
    get().and_then(|dt| dt.boot_args.clone())
}

/// Get model string
pub fn get_model() -> Option<String> {
    get().and_then(|dt| dt.model.clone())
}

/// Get compatible strings
pub fn get_compatible() -> Vec<String> {
    get().map(|dt| dt.compatible.clone()).unwrap_or_default()
}

/// Check if a driver should be loaded for a compatible string
pub fn should_load_driver(compat: &str) -> bool {
    !find_compatible(compat).is_empty()
}

// ============================================================================
// Driver binding helpers
// ============================================================================

/// Trait for device tree compatible drivers
pub trait DtDriver: Send + Sync {
    /// Compatible strings this driver handles
    fn compatible(&self) -> &[&str];

    /// Probe function - called when matching node found
    fn probe(&self, node: &DeviceNode) -> Result<(), &'static str>;

    /// Remove function - called on driver unload
    fn remove(&self, node: &DeviceNode);
}

/// Driver registry
static DRIVER_REGISTRY: TicketSpinlock<Vec<Box<dyn DtDriver>>> = TicketSpinlock::new(Vec::new());

/// Register a device tree driver
pub fn register_driver(driver: Box<dyn DtDriver>) {
    let mut registry = DRIVER_REGISTRY.lock();
    registry.push(driver);
}

/// Probe all registered drivers against the device tree
pub fn probe_drivers() {
    let registry = DRIVER_REGISTRY.lock();
    let dt = match get() {
        Some(dt) => dt,
        None => return,
    };

    for driver in registry.iter() {
        for compat in driver.compatible() {
            for node in dt.find_compatible(compat) {
                if node.is_enabled() {
                    if let Err(e) = driver.probe(node) {
                        crate::kprintln!("devicetree: failed to probe {}: {}", node.full_name(), e);
                    }
                }
            }
        }
    }
}
