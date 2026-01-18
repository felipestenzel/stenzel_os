//! A/B Partition System for Safe Updates
//!
//! Implements an A/B partition scheme (similar to Android/Chrome OS) that allows
//! seamless updates with automatic rollback on failure.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU32, AtomicBool, Ordering};

/// Maximum number of boot attempts before rollback
pub const MAX_BOOT_ATTEMPTS: u32 = 3;

/// A/B partition error types
#[derive(Debug, Clone)]
pub enum AbError {
    InvalidSlot(String),
    SlotNotBootable(char),
    UpdateInProgress,
    NoActiveSlot,
    MetadataCorrupted,
    WriteError(String),
    ReadError(String),
    VerificationFailed(String),
    RollbackFailed(String),
    PartitionNotFound(String),
    SlotFull,
}

pub type AbResult<T> = Result<T, AbError>;

/// Partition slot identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    A,
    B,
}

impl Slot {
    pub fn as_char(&self) -> char {
        match self {
            Slot::A => 'a',
            Slot::B => 'b',
        }
    }

    pub fn as_suffix(&self) -> &'static str {
        match self {
            Slot::A => "_a",
            Slot::B => "_b",
        }
    }

    pub fn other(&self) -> Slot {
        match self {
            Slot::A => Slot::B,
            Slot::B => Slot::A,
        }
    }

    pub fn from_char(c: char) -> Option<Slot> {
        match c {
            'a' | 'A' => Some(Slot::A),
            'b' | 'B' => Some(Slot::B),
            _ => None,
        }
    }
}

/// Slot state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
    /// Slot is bootable and verified
    Successful,
    /// Slot is bootable but unverified (new update)
    Unverified,
    /// Slot is not bootable (corrupted or empty)
    Unbootable,
}

impl SlotState {
    pub fn as_str(&self) -> &'static str {
        match self {
            SlotState::Successful => "successful",
            SlotState::Unverified => "unverified",
            SlotState::Unbootable => "unbootable",
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => SlotState::Successful,
            1 => SlotState::Unverified,
            _ => SlotState::Unbootable,
        }
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            SlotState::Successful => 0,
            SlotState::Unverified => 1,
            SlotState::Unbootable => 2,
        }
    }
}

/// Metadata for a single slot
#[derive(Debug, Clone)]
pub struct SlotMetadata {
    /// Slot identifier
    pub slot: Slot,
    /// Current state
    pub state: SlotState,
    /// Boot priority (higher = preferred)
    pub priority: u8,
    /// Number of boot attempts remaining
    pub tries_remaining: u8,
    /// Whether this slot booted successfully
    pub successful_boot: bool,
    /// Version string installed in this slot
    pub version: String,
    /// Build timestamp
    pub build_timestamp: u64,
    /// SHA256 hash of the slot contents
    pub content_hash: String,
    /// Size of the slot in bytes
    pub slot_size: u64,
    /// Space used in the slot
    pub used_size: u64,
}

impl SlotMetadata {
    pub fn new(slot: Slot) -> Self {
        Self {
            slot,
            state: SlotState::Unbootable,
            priority: 0,
            tries_remaining: 0,
            successful_boot: false,
            version: String::new(),
            build_timestamp: 0,
            content_hash: String::new(),
            slot_size: 0,
            used_size: 0,
        }
    }

    /// Check if slot is bootable
    pub fn is_bootable(&self) -> bool {
        matches!(self.state, SlotState::Successful | SlotState::Unverified)
            && self.tries_remaining > 0
    }

    /// Mark slot as successful after boot verification
    pub fn mark_successful(&mut self) {
        self.state = SlotState::Successful;
        self.successful_boot = true;
        self.tries_remaining = MAX_BOOT_ATTEMPTS as u8;
    }

    /// Decrement boot tries
    pub fn decrement_tries(&mut self) {
        if self.tries_remaining > 0 {
            self.tries_remaining -= 1;
        }
        if self.tries_remaining == 0 && !self.successful_boot {
            self.state = SlotState::Unbootable;
        }
    }
}

/// A/B partition metadata header
#[derive(Debug, Clone)]
#[repr(C)]
pub struct AbMetadataHeader {
    /// Magic number: "ABMT"
    pub magic: [u8; 4],
    /// Version of the metadata format
    pub version: u32,
    /// CRC32 of the metadata
    pub crc32: u32,
    /// Currently active slot
    pub active_slot: u8,
    /// Slot A state
    pub slot_a_state: u8,
    /// Slot A priority
    pub slot_a_priority: u8,
    /// Slot A tries remaining
    pub slot_a_tries: u8,
    /// Slot B state
    pub slot_b_state: u8,
    /// Slot B priority
    pub slot_b_priority: u8,
    /// Slot B tries remaining
    pub slot_b_tries: u8,
    /// Reserved for future use
    pub reserved: [u8; 9],
    /// Slot A version (32 bytes, null-terminated)
    pub slot_a_version: [u8; 32],
    /// Slot B version (32 bytes, null-terminated)
    pub slot_b_version: [u8; 32],
    /// Slot A content hash (64 bytes, hex string)
    pub slot_a_hash: [u8; 64],
    /// Slot B content hash (64 bytes, hex string)
    pub slot_b_hash: [u8; 64],
}

impl Default for AbMetadataHeader {
    fn default() -> Self {
        Self {
            magic: *b"ABMT",
            version: 1,
            crc32: 0,
            active_slot: 0, // Slot A
            slot_a_state: SlotState::Unbootable.to_u8(),
            slot_a_priority: 15,
            slot_a_tries: MAX_BOOT_ATTEMPTS as u8,
            slot_b_state: SlotState::Unbootable.to_u8(),
            slot_b_priority: 14,
            slot_b_tries: MAX_BOOT_ATTEMPTS as u8,
            reserved: [0; 9],
            slot_a_version: [0; 32],
            slot_b_version: [0; 32],
            slot_a_hash: [0; 64],
            slot_b_hash: [0; 64],
        }
    }
}

impl AbMetadataHeader {
    /// Validate the header
    pub fn validate(&self) -> bool {
        self.magic == *b"ABMT" && self.version == 1
    }

    /// Calculate CRC32 of the header (excluding crc32 field)
    pub fn calculate_crc(&self) -> u32 {
        let mut crc: u32 = 0xFFFFFFFF;
        let bytes = self.to_bytes();

        // Skip the crc32 field (bytes 8-11)
        for (i, &byte) in bytes.iter().enumerate() {
            if i >= 8 && i < 12 {
                continue;
            }
            crc ^= byte as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB88320;
                } else {
                    crc >>= 1;
                }
            }
        }
        !crc
    }

    /// Serialize header to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(228);
        bytes.extend_from_slice(&self.magic);
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.extend_from_slice(&self.crc32.to_le_bytes());
        bytes.push(self.active_slot);
        bytes.push(self.slot_a_state);
        bytes.push(self.slot_a_priority);
        bytes.push(self.slot_a_tries);
        bytes.push(self.slot_b_state);
        bytes.push(self.slot_b_priority);
        bytes.push(self.slot_b_tries);
        bytes.extend_from_slice(&self.reserved);
        bytes.extend_from_slice(&self.slot_a_version);
        bytes.extend_from_slice(&self.slot_b_version);
        bytes.extend_from_slice(&self.slot_a_hash);
        bytes.extend_from_slice(&self.slot_b_hash);
        bytes
    }

    /// Deserialize header from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 228 {
            return None;
        }

        let mut header = Self::default();
        header.magic.copy_from_slice(&bytes[0..4]);
        header.version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        header.crc32 = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        header.active_slot = bytes[12];
        header.slot_a_state = bytes[13];
        header.slot_a_priority = bytes[14];
        header.slot_a_tries = bytes[15];
        header.slot_b_state = bytes[16];
        header.slot_b_priority = bytes[17];
        header.slot_b_tries = bytes[18];
        header.reserved.copy_from_slice(&bytes[19..28]);
        header.slot_a_version.copy_from_slice(&bytes[28..60]);
        header.slot_b_version.copy_from_slice(&bytes[60..92]);
        header.slot_a_hash.copy_from_slice(&bytes[92..156]);
        header.slot_b_hash.copy_from_slice(&bytes[156..220]);

        if header.validate() {
            Some(header)
        } else {
            None
        }
    }
}

/// Partition definition for A/B scheme
#[derive(Debug, Clone)]
pub struct AbPartition {
    /// Base name (e.g., "system", "boot", "vendor")
    pub name: String,
    /// Size per slot in bytes
    pub size: u64,
    /// Whether this partition is updatable
    pub updatable: bool,
    /// Block device path template (e.g., "/dev/nvme0n1p{}")
    pub device_template: String,
    /// Partition number for slot A
    pub part_num_a: u32,
    /// Partition number for slot B
    pub part_num_b: u32,
}

impl AbPartition {
    pub fn new(name: &str, size: u64) -> Self {
        Self {
            name: String::from(name),
            size,
            updatable: true,
            device_template: String::from("/dev/nvme0n1p{}"),
            part_num_a: 0,
            part_num_b: 0,
        }
    }

    /// Get device path for a specific slot
    pub fn device_path(&self, slot: Slot) -> String {
        let part_num = match slot {
            Slot::A => self.part_num_a,
            Slot::B => self.part_num_b,
        };
        self.device_template.replace("{}", &format!("{}", part_num))
    }
}

/// A/B partition layout
#[derive(Debug, Clone)]
pub struct AbLayout {
    /// Boot partition (kernel, initramfs)
    pub boot: AbPartition,
    /// System partition (root filesystem)
    pub system: AbPartition,
    /// Vendor partition (optional)
    pub vendor: Option<AbPartition>,
    /// Metadata partition (shared, not duplicated)
    pub metadata_device: String,
    /// Metadata offset within the device
    pub metadata_offset: u64,
    /// User data partition (shared, not duplicated)
    pub userdata_device: Option<String>,
}

impl Default for AbLayout {
    fn default() -> Self {
        Self {
            boot: AbPartition {
                name: String::from("boot"),
                size: 512 * 1024 * 1024, // 512 MB
                updatable: true,
                device_template: String::from("/dev/nvme0n1p{}"),
                part_num_a: 2,
                part_num_b: 3,
            },
            system: AbPartition {
                name: String::from("system"),
                size: 16 * 1024 * 1024 * 1024, // 16 GB
                updatable: true,
                device_template: String::from("/dev/nvme0n1p{}"),
                part_num_a: 4,
                part_num_b: 5,
            },
            vendor: None,
            metadata_device: String::from("/dev/nvme0n1p6"),
            metadata_offset: 0,
            userdata_device: Some(String::from("/dev/nvme0n1p7")),
        }
    }
}

/// Update state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateState {
    Idle,
    Downloading,
    Verifying,
    Applying,
    PendingReboot,
    Failed,
}

impl UpdateState {
    pub fn as_str(&self) -> &'static str {
        match self {
            UpdateState::Idle => "idle",
            UpdateState::Downloading => "downloading",
            UpdateState::Verifying => "verifying",
            UpdateState::Applying => "applying",
            UpdateState::PendingReboot => "pending_reboot",
            UpdateState::Failed => "failed",
        }
    }
}

/// A/B partition manager
pub struct AbManager {
    /// Layout configuration
    layout: AbLayout,
    /// Slot A metadata
    slot_a: SlotMetadata,
    /// Slot B metadata
    slot_b: SlotMetadata,
    /// Current active slot
    active_slot: Slot,
    /// Current update state
    update_state: UpdateState,
    /// Update target slot
    update_target: Option<Slot>,
    /// Boot count for current slot
    boot_count: AtomicU32,
    /// Whether metadata needs to be written
    metadata_dirty: AtomicBool,
}

impl AbManager {
    /// Create a new A/B manager with default layout
    pub fn new() -> Self {
        Self {
            layout: AbLayout::default(),
            slot_a: SlotMetadata::new(Slot::A),
            slot_b: SlotMetadata::new(Slot::B),
            active_slot: Slot::A,
            update_state: UpdateState::Idle,
            update_target: None,
            boot_count: AtomicU32::new(0),
            metadata_dirty: AtomicBool::new(false),
        }
    }

    /// Create manager with custom layout
    pub fn with_layout(layout: AbLayout) -> Self {
        Self {
            layout,
            slot_a: SlotMetadata::new(Slot::A),
            slot_b: SlotMetadata::new(Slot::B),
            active_slot: Slot::A,
            update_state: UpdateState::Idle,
            update_target: None,
            boot_count: AtomicU32::new(0),
            metadata_dirty: AtomicBool::new(false),
        }
    }

    /// Initialize the A/B system
    pub fn init(&mut self) -> AbResult<()> {
        // Load metadata from disk
        self.load_metadata()?;

        // Determine active slot
        self.active_slot = self.select_active_slot()?;

        // Decrement tries for current slot if unverified
        let slot_meta = self.get_slot_metadata_mut(self.active_slot);
        if slot_meta.state == SlotState::Unverified {
            slot_meta.decrement_tries();
            self.metadata_dirty.store(true, Ordering::SeqCst);
        }

        // Increment boot count
        self.boot_count.fetch_add(1, Ordering::SeqCst);

        crate::kprintln!("ab_partitions: Initialized, active slot: {}",
            self.active_slot.as_char());

        Ok(())
    }

    /// Load metadata from disk
    fn load_metadata(&mut self) -> AbResult<()> {
        // In real implementation, read from metadata_device at metadata_offset
        // For now, use defaults

        // Try to read existing metadata
        // If not found or corrupted, initialize fresh
        self.slot_a = SlotMetadata::new(Slot::A);
        self.slot_a.state = SlotState::Successful;
        self.slot_a.priority = 15;
        self.slot_a.tries_remaining = MAX_BOOT_ATTEMPTS as u8;
        self.slot_a.version = String::from("1.0.0");

        self.slot_b = SlotMetadata::new(Slot::B);
        self.slot_b.priority = 14;
        self.slot_b.tries_remaining = MAX_BOOT_ATTEMPTS as u8;

        Ok(())
    }

    /// Save metadata to disk
    fn save_metadata(&self) -> AbResult<()> {
        let header = self.build_metadata_header();
        let bytes = header.to_bytes();

        // In real implementation, write to metadata_device
        let _ = bytes;

        Ok(())
    }

    /// Build metadata header from current state
    fn build_metadata_header(&self) -> AbMetadataHeader {
        let mut header = AbMetadataHeader::default();

        header.active_slot = match self.active_slot {
            Slot::A => 0,
            Slot::B => 1,
        };

        header.slot_a_state = self.slot_a.state.to_u8();
        header.slot_a_priority = self.slot_a.priority;
        header.slot_a_tries = self.slot_a.tries_remaining;

        header.slot_b_state = self.slot_b.state.to_u8();
        header.slot_b_priority = self.slot_b.priority;
        header.slot_b_tries = self.slot_b.tries_remaining;

        // Copy version strings
        let ver_a = self.slot_a.version.as_bytes();
        let len_a = core::cmp::min(ver_a.len(), 31);
        header.slot_a_version[..len_a].copy_from_slice(&ver_a[..len_a]);

        let ver_b = self.slot_b.version.as_bytes();
        let len_b = core::cmp::min(ver_b.len(), 31);
        header.slot_b_version[..len_b].copy_from_slice(&ver_b[..len_b]);

        // Copy hashes
        let hash_a = self.slot_a.content_hash.as_bytes();
        let len_ha = core::cmp::min(hash_a.len(), 64);
        header.slot_a_hash[..len_ha].copy_from_slice(&hash_a[..len_ha]);

        let hash_b = self.slot_b.content_hash.as_bytes();
        let len_hb = core::cmp::min(hash_b.len(), 64);
        header.slot_b_hash[..len_hb].copy_from_slice(&hash_b[..len_hb]);

        // Calculate and set CRC
        header.crc32 = header.calculate_crc();

        header
    }

    /// Select which slot to boot from
    fn select_active_slot(&self) -> AbResult<Slot> {
        // Compare priorities and bootability
        let a_bootable = self.slot_a.is_bootable();
        let b_bootable = self.slot_b.is_bootable();

        match (a_bootable, b_bootable) {
            (true, true) => {
                // Both bootable, use priority
                if self.slot_a.priority >= self.slot_b.priority {
                    Ok(Slot::A)
                } else {
                    Ok(Slot::B)
                }
            }
            (true, false) => Ok(Slot::A),
            (false, true) => Ok(Slot::B),
            (false, false) => Err(AbError::NoActiveSlot),
        }
    }

    /// Get metadata for a slot
    pub fn get_slot_metadata(&self, slot: Slot) -> &SlotMetadata {
        match slot {
            Slot::A => &self.slot_a,
            Slot::B => &self.slot_b,
        }
    }

    /// Get mutable metadata for a slot
    fn get_slot_metadata_mut(&mut self, slot: Slot) -> &mut SlotMetadata {
        match slot {
            Slot::A => &mut self.slot_a,
            Slot::B => &mut self.slot_b,
        }
    }

    /// Get currently active slot
    pub fn active_slot(&self) -> Slot {
        self.active_slot
    }

    /// Get the inactive (update target) slot
    pub fn inactive_slot(&self) -> Slot {
        self.active_slot.other()
    }

    /// Check if update is in progress
    pub fn is_update_in_progress(&self) -> bool {
        !matches!(self.update_state, UpdateState::Idle | UpdateState::Failed)
    }

    /// Get current update state
    pub fn update_state(&self) -> UpdateState {
        self.update_state
    }

    /// Mark current slot as successfully booted
    pub fn mark_boot_successful(&mut self) -> AbResult<()> {
        let slot_meta = self.get_slot_metadata_mut(self.active_slot);
        slot_meta.mark_successful();
        self.metadata_dirty.store(true, Ordering::SeqCst);

        if self.metadata_dirty.load(Ordering::SeqCst) {
            self.save_metadata()?;
            self.metadata_dirty.store(false, Ordering::SeqCst);
        }

        crate::kprintln!("ab_partitions: Slot {} marked successful",
            self.active_slot.as_char());

        Ok(())
    }

    /// Begin an update to the inactive slot
    pub fn begin_update(&mut self, version: &str) -> AbResult<Slot> {
        if self.is_update_in_progress() {
            return Err(AbError::UpdateInProgress);
        }

        let target = self.inactive_slot();
        self.update_target = Some(target);
        self.update_state = UpdateState::Downloading;

        // Prepare target slot
        let target_meta = self.get_slot_metadata_mut(target);
        target_meta.state = SlotState::Unbootable;
        target_meta.version = String::from(version);
        target_meta.tries_remaining = 0;
        target_meta.successful_boot = false;
        target_meta.content_hash.clear();

        self.metadata_dirty.store(true, Ordering::SeqCst);

        crate::kprintln!("ab_partitions: Beginning update to slot {}, version {}",
            target.as_char(), version);

        Ok(target)
    }

    /// Set update state
    pub fn set_update_state(&mut self, state: UpdateState) {
        self.update_state = state;
    }

    /// Finalize update and make it bootable
    pub fn finalize_update(&mut self, content_hash: &str) -> AbResult<()> {
        let target = self.update_target.ok_or(AbError::UpdateInProgress)?;

        self.update_state = UpdateState::Verifying;

        // Update target slot metadata
        let target_meta = self.get_slot_metadata_mut(target);
        target_meta.state = SlotState::Unverified;
        target_meta.tries_remaining = MAX_BOOT_ATTEMPTS as u8;
        target_meta.content_hash = String::from(content_hash);

        // Make target slot higher priority
        let current_meta = self.get_slot_metadata(self.active_slot);
        let current_priority = current_meta.priority;

        let target_meta = self.get_slot_metadata_mut(target);
        target_meta.priority = current_priority + 1;

        self.update_state = UpdateState::PendingReboot;
        self.metadata_dirty.store(true, Ordering::SeqCst);
        self.save_metadata()?;

        crate::kprintln!("ab_partitions: Update finalized for slot {}, pending reboot",
            target.as_char());

        Ok(())
    }

    /// Cancel current update
    pub fn cancel_update(&mut self) -> AbResult<()> {
        if let Some(target) = self.update_target {
            let target_meta = self.get_slot_metadata_mut(target);
            target_meta.state = SlotState::Unbootable;
            target_meta.tries_remaining = 0;
        }

        self.update_state = UpdateState::Idle;
        self.update_target = None;
        self.metadata_dirty.store(true, Ordering::SeqCst);
        self.save_metadata()?;

        Ok(())
    }

    /// Manually trigger rollback to previous slot
    pub fn rollback(&mut self) -> AbResult<()> {
        let previous = self.active_slot.other();
        let prev_meta = self.get_slot_metadata(previous);

        if !prev_meta.is_bootable() {
            return Err(AbError::SlotNotBootable(previous.as_char()));
        }

        // Mark current slot as unbootable
        let current_meta = self.get_slot_metadata_mut(self.active_slot);
        current_meta.state = SlotState::Unbootable;
        current_meta.tries_remaining = 0;

        // Increase priority of previous slot
        let prev_meta = self.get_slot_metadata_mut(previous);
        prev_meta.priority = 15;

        self.metadata_dirty.store(true, Ordering::SeqCst);
        self.save_metadata()?;

        crate::kprintln!("ab_partitions: Rollback requested to slot {}",
            previous.as_char());

        Ok(())
    }

    /// Get partition device path for a slot
    pub fn get_partition_device(&self, partition: &str, slot: Slot) -> Option<String> {
        match partition {
            "boot" => Some(self.layout.boot.device_path(slot)),
            "system" => Some(self.layout.system.device_path(slot)),
            "vendor" => self.layout.vendor.as_ref().map(|v| v.device_path(slot)),
            _ => None,
        }
    }

    /// Get current root filesystem device
    pub fn current_root_device(&self) -> String {
        self.layout.system.device_path(self.active_slot)
    }

    /// Get current boot device
    pub fn current_boot_device(&self) -> String {
        self.layout.boot.device_path(self.active_slot)
    }

    /// Get status summary
    pub fn status(&self) -> AbStatus {
        AbStatus {
            active_slot: self.active_slot,
            slot_a: self.slot_a.clone(),
            slot_b: self.slot_b.clone(),
            update_state: self.update_state,
            update_target: self.update_target,
            boot_count: self.boot_count.load(Ordering::SeqCst),
        }
    }

    /// Format status as string
    pub fn format_status(&self) -> String {
        let status = self.status();
        let mut output = String::new();

        output.push_str("A/B Partition Status:\n");
        output.push_str(&format!("  Active Slot: {}\n", status.active_slot.as_char()));
        output.push_str(&format!("  Boot Count: {}\n\n", status.boot_count));

        output.push_str("  Slot A:\n");
        output.push_str(&format!("    State: {}\n", status.slot_a.state.as_str()));
        output.push_str(&format!("    Version: {}\n", status.slot_a.version));
        output.push_str(&format!("    Priority: {}\n", status.slot_a.priority));
        output.push_str(&format!("    Tries: {}\n", status.slot_a.tries_remaining));
        output.push_str(&format!("    Bootable: {}\n\n", status.slot_a.is_bootable()));

        output.push_str("  Slot B:\n");
        output.push_str(&format!("    State: {}\n", status.slot_b.state.as_str()));
        output.push_str(&format!("    Version: {}\n", status.slot_b.version));
        output.push_str(&format!("    Priority: {}\n", status.slot_b.priority));
        output.push_str(&format!("    Tries: {}\n", status.slot_b.tries_remaining));
        output.push_str(&format!("    Bootable: {}\n\n", status.slot_b.is_bootable()));

        if status.update_state != UpdateState::Idle {
            output.push_str(&format!("  Update State: {}\n", status.update_state.as_str()));
            if let Some(target) = status.update_target {
                output.push_str(&format!("  Update Target: {}\n", target.as_char()));
            }
        }

        output
    }
}

/// A/B status summary
#[derive(Debug, Clone)]
pub struct AbStatus {
    pub active_slot: Slot,
    pub slot_a: SlotMetadata,
    pub slot_b: SlotMetadata,
    pub update_state: UpdateState,
    pub update_target: Option<Slot>,
    pub boot_count: u32,
}

/// Bootloader interface for A/B
pub struct AbBootloader;

impl AbBootloader {
    /// Generate kernel command line for A/B boot
    pub fn generate_cmdline(manager: &AbManager) -> String {
        let slot = manager.active_slot();
        let root_device = manager.current_root_device();

        format!(
            "root={} androidboot.slot_suffix={} ro quiet",
            root_device,
            slot.as_suffix()
        )
    }

    /// Generate GRUB configuration for A/B
    pub fn generate_grub_config(manager: &AbManager) -> String {
        let mut config = String::new();

        config.push_str("# A/B Partition GRUB Configuration\n\n");
        config.push_str("set default=0\n");
        config.push_str("set timeout=5\n\n");

        // Generate entry for slot A
        config.push_str("menuentry \"Stenzel OS (Slot A)\" {\n");
        config.push_str(&format!("    linux {} root={} androidboot.slot_suffix=_a ro quiet\n",
            manager.layout.boot.device_path(Slot::A),
            manager.layout.system.device_path(Slot::A)
        ));
        config.push_str(&format!("    initrd {}/initramfs.img\n",
            manager.layout.boot.device_path(Slot::A)));
        config.push_str("}\n\n");

        // Generate entry for slot B
        config.push_str("menuentry \"Stenzel OS (Slot B)\" {\n");
        config.push_str(&format!("    linux {} root={} androidboot.slot_suffix=_b ro quiet\n",
            manager.layout.boot.device_path(Slot::B),
            manager.layout.system.device_path(Slot::B)
        ));
        config.push_str(&format!("    initrd {}/initramfs.img\n",
            manager.layout.boot.device_path(Slot::B)));
        config.push_str("}\n\n");

        config.push_str("menuentry \"Recovery Mode\" {\n");
        config.push_str("    linux /boot/vmlinuz root=/dev/disk/by-label/recovery ro single\n");
        config.push_str("    initrd /boot/initramfs-recovery.img\n");
        config.push_str("}\n");

        config
    }

    /// Generate systemd-boot configuration for A/B
    pub fn generate_systemd_boot_config(manager: &AbManager) -> Vec<(String, String)> {
        let mut entries = Vec::new();

        // Loader config
        let loader_conf = format!(
            "default stenzel-{}.conf\ntimeout 5\n",
            manager.active_slot().as_char()
        );
        entries.push((String::from("loader/loader.conf"), loader_conf));

        // Slot A entry
        let entry_a = format!(
            "title Stenzel OS (Slot A)\n\
             linux /vmlinuz_a\n\
             initrd /initramfs_a.img\n\
             options root={} androidboot.slot_suffix=_a ro quiet\n",
            manager.layout.system.device_path(Slot::A)
        );
        entries.push((String::from("loader/entries/stenzel-a.conf"), entry_a));

        // Slot B entry
        let entry_b = format!(
            "title Stenzel OS (Slot B)\n\
             linux /vmlinuz_b\n\
             initrd /initramfs_b.img\n\
             options root={} androidboot.slot_suffix=_b ro quiet\n",
            manager.layout.system.device_path(Slot::B)
        );
        entries.push((String::from("loader/entries/stenzel-b.conf"), entry_b));

        // Recovery entry
        let recovery = String::from(
            "title Recovery Mode\n\
             linux /vmlinuz-recovery\n\
             initrd /initramfs-recovery.img\n\
             options root=/dev/disk/by-label/recovery ro single\n"
        );
        entries.push((String::from("loader/entries/recovery.conf"), recovery));

        entries
    }
}

/// Global A/B manager instance
static mut AB_MANAGER: Option<AbManager> = None;

/// Get global A/B manager
pub fn ab_manager() -> &'static mut AbManager {
    unsafe {
        if AB_MANAGER.is_none() {
            AB_MANAGER = Some(AbManager::new());
        }
        AB_MANAGER.as_mut().unwrap()
    }
}

/// Initialize A/B partition system
pub fn init() -> AbResult<()> {
    let manager = ab_manager();
    manager.init()
}

/// Mark boot as successful (call after successful system startup)
pub fn mark_successful() -> AbResult<()> {
    ab_manager().mark_boot_successful()
}

/// Get current slot
pub fn current_slot() -> Slot {
    ab_manager().active_slot()
}

/// Get status
pub fn get_status() -> AbStatus {
    ab_manager().status()
}

/// Format status as string
pub fn format_status() -> String {
    ab_manager().format_status()
}
