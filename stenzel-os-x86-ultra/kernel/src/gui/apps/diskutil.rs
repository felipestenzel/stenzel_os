//! Disk Utility Application
//!
//! A graphical application for managing disks and partitions.
//! Supports viewing disk info, partitioning, formatting, and disk health.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::format;

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton, theme};

/// Disk type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskType {
    Hdd,
    Ssd,
    Nvme,
    Usb,
    Cdrom,
    Floppy,
    Virtual,
    Unknown,
}

impl DiskType {
    pub fn name(&self) -> &'static str {
        match self {
            DiskType::Hdd => "HDD",
            DiskType::Ssd => "SSD",
            DiskType::Nvme => "NVMe",
            DiskType::Usb => "USB",
            DiskType::Cdrom => "CD/DVD",
            DiskType::Floppy => "Floppy",
            DiskType::Virtual => "Virtual",
            DiskType::Unknown => "Unknown",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            DiskType::Hdd => "[HDD]",
            DiskType::Ssd => "[SSD]",
            DiskType::Nvme => "[NVM]",
            DiskType::Usb => "[USB]",
            DiskType::Cdrom => "[CD] ",
            DiskType::Floppy => "[FLP]",
            DiskType::Virtual => "[VRT]",
            DiskType::Unknown => "[???]",
        }
    }
}

/// Partition table type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionTable {
    Mbr,
    Gpt,
    None,
    Unknown,
}

impl PartitionTable {
    pub fn name(&self) -> &'static str {
        match self {
            PartitionTable::Mbr => "MBR",
            PartitionTable::Gpt => "GPT",
            PartitionTable::None => "None",
            PartitionTable::Unknown => "Unknown",
        }
    }
}

/// Filesystem type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemType {
    Ext2,
    Ext3,
    Ext4,
    Btrfs,
    Xfs,
    Zfs,
    Fat12,
    Fat16,
    Fat32,
    ExFat,
    Ntfs,
    Hfs,
    HfsPlus,
    Apfs,
    Iso9660,
    Udf,
    Swap,
    Raw,
    Unknown,
}

impl FilesystemType {
    pub fn name(&self) -> &'static str {
        match self {
            FilesystemType::Ext2 => "ext2",
            FilesystemType::Ext3 => "ext3",
            FilesystemType::Ext4 => "ext4",
            FilesystemType::Btrfs => "Btrfs",
            FilesystemType::Xfs => "XFS",
            FilesystemType::Zfs => "ZFS",
            FilesystemType::Fat12 => "FAT12",
            FilesystemType::Fat16 => "FAT16",
            FilesystemType::Fat32 => "FAT32",
            FilesystemType::ExFat => "exFAT",
            FilesystemType::Ntfs => "NTFS",
            FilesystemType::Hfs => "HFS",
            FilesystemType::HfsPlus => "HFS+",
            FilesystemType::Apfs => "APFS",
            FilesystemType::Iso9660 => "ISO 9660",
            FilesystemType::Udf => "UDF",
            FilesystemType::Swap => "Swap",
            FilesystemType::Raw => "Raw",
            FilesystemType::Unknown => "Unknown",
        }
    }

    pub fn supports_permissions(&self) -> bool {
        matches!(self,
            FilesystemType::Ext2 | FilesystemType::Ext3 | FilesystemType::Ext4 |
            FilesystemType::Btrfs | FilesystemType::Xfs | FilesystemType::Zfs |
            FilesystemType::HfsPlus | FilesystemType::Apfs | FilesystemType::Ntfs)
    }

    pub fn supports_journaling(&self) -> bool {
        matches!(self,
            FilesystemType::Ext3 | FilesystemType::Ext4 | FilesystemType::Btrfs |
            FilesystemType::Xfs | FilesystemType::Zfs | FilesystemType::HfsPlus |
            FilesystemType::Apfs | FilesystemType::Ntfs)
    }

    pub fn max_file_size(&self) -> Option<u64> {
        match self {
            FilesystemType::Fat12 => Some(32 * 1024 * 1024),
            FilesystemType::Fat16 => Some(2 * 1024 * 1024 * 1024),
            FilesystemType::Fat32 => Some(4 * 1024 * 1024 * 1024 - 1),
            FilesystemType::Ext2 | FilesystemType::Ext3 => Some(2 * 1024 * 1024 * 1024 * 1024),
            _ => None, // Essentially unlimited for modern filesystems
        }
    }
}

/// SMART health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Good,
    Warning,
    Critical,
    Unknown,
}

impl HealthStatus {
    pub fn name(&self) -> &'static str {
        match self {
            HealthStatus::Good => "Good",
            HealthStatus::Warning => "Warning",
            HealthStatus::Critical => "Critical",
            HealthStatus::Unknown => "Unknown",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            HealthStatus::Good => Color::new(40, 167, 69),
            HealthStatus::Warning => Color::new(255, 193, 7),
            HealthStatus::Critical => Color::new(220, 53, 69),
            HealthStatus::Unknown => Color::new(108, 117, 125),
        }
    }
}

/// SMART attribute
#[derive(Debug, Clone)]
pub struct SmartAttribute {
    pub id: u8,
    pub name: String,
    pub current: u8,
    pub worst: u8,
    pub threshold: u8,
    pub raw_value: u64,
    pub status: HealthStatus,
}

/// Disk information
#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub id: String,
    pub name: String,
    pub disk_type: DiskType,
    pub model: String,
    pub serial: String,
    pub firmware: String,
    pub capacity: u64,
    pub sector_size: u32,
    pub rotation_rate: u16,  // 0 for SSD
    pub partition_table: PartitionTable,
    pub health: HealthStatus,
    pub temperature: Option<i16>,
    pub smart_attributes: Vec<SmartAttribute>,
    pub partitions: Vec<PartitionInfo>,
    pub is_removable: bool,
    pub is_read_only: bool,
}

impl DiskInfo {
    pub fn new(id: &str, name: &str, disk_type: DiskType) -> Self {
        Self {
            id: String::from(id),
            name: String::from(name),
            disk_type,
            model: String::new(),
            serial: String::new(),
            firmware: String::new(),
            capacity: 0,
            sector_size: 512,
            rotation_rate: 0,
            partition_table: PartitionTable::Unknown,
            health: HealthStatus::Unknown,
            temperature: None,
            smart_attributes: Vec::new(),
            partitions: Vec::new(),
            is_removable: false,
            is_read_only: false,
        }
    }
}

/// Partition information
#[derive(Debug, Clone)]
pub struct PartitionInfo {
    pub id: String,
    pub number: u32,
    pub name: String,
    pub label: Option<String>,
    pub filesystem: FilesystemType,
    pub start_sector: u64,
    pub end_sector: u64,
    pub size: u64,
    pub used: u64,
    pub available: u64,
    pub mount_point: Option<String>,
    pub uuid: Option<String>,
    pub flags: Vec<String>,
    pub is_bootable: bool,
    pub is_mounted: bool,
}

impl PartitionInfo {
    pub fn new(id: &str, number: u32) -> Self {
        Self {
            id: String::from(id),
            number,
            name: format!("Partition {}", number),
            label: None,
            filesystem: FilesystemType::Unknown,
            start_sector: 0,
            end_sector: 0,
            size: 0,
            used: 0,
            available: 0,
            mount_point: None,
            uuid: None,
            flags: Vec::new(),
            is_bootable: false,
            is_mounted: false,
        }
    }

    pub fn usage_percent(&self) -> f32 {
        if self.size == 0 {
            return 0.0;
        }
        (self.used as f32 / self.size as f32) * 100.0
    }
}

/// Disk operation result
#[derive(Debug)]
pub enum DiskError {
    DiskNotFound,
    PartitionNotFound,
    PermissionDenied,
    DiskBusy,
    InvalidPartitionTable,
    FilesystemError,
    IoError,
    UnsupportedOperation,
    InsufficientSpace,
}

/// Format options
#[derive(Debug, Clone)]
pub struct FormatOptions {
    pub filesystem: FilesystemType,
    pub label: Option<String>,
    pub quick_format: bool,
    pub enable_journaling: bool,
    pub block_size: u32,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            filesystem: FilesystemType::Ext4,
            label: None,
            quick_format: true,
            enable_journaling: true,
            block_size: 4096,
        }
    }
}

/// Partition create options
#[derive(Debug, Clone)]
pub struct PartitionCreateOptions {
    pub size: u64,
    pub filesystem: FilesystemType,
    pub label: Option<String>,
    pub bootable: bool,
    pub alignment: u64,
}

impl Default for PartitionCreateOptions {
    fn default() -> Self {
        Self {
            size: 0,
            filesystem: FilesystemType::Ext4,
            label: None,
            bootable: false,
            alignment: 1024 * 1024, // 1MB alignment
        }
    }
}

/// View mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    DiskList,
    PartitionMap,
    SmartInfo,
    Operations,
}

/// Selected item
#[derive(Debug, Clone)]
pub enum SelectedItem {
    None,
    Disk(usize),
    Partition(usize, usize),
}

/// Disk Utility widget
pub struct DiskUtility {
    id: WidgetId,
    bounds: Bounds,
    visible: bool,
    enabled: bool,
    focused: bool,
    disks: Vec<DiskInfo>,
    selected_item: SelectedItem,
    view_mode: ViewMode,
    scroll_offset: usize,
    visible_rows: usize,
    hover_index: Option<usize>,
    status_message: String,
    operation_in_progress: bool,
    operation_progress: f32,
    show_confirmation_dialog: bool,
    confirmation_action: Option<String>,
}

impl DiskUtility {
    const HEADER_HEIGHT: usize = 40;
    const SIDEBAR_WIDTH: usize = 220;
    const ROW_HEIGHT: usize = 24;
    const STATUS_HEIGHT: usize = 24;

    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        let content_height = height.saturating_sub(Self::HEADER_HEIGHT + Self::STATUS_HEIGHT);
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            visible: true,
            enabled: true,
            focused: false,
            disks: Vec::new(),
            selected_item: SelectedItem::None,
            view_mode: ViewMode::DiskList,
            scroll_offset: 0,
            visible_rows: content_height / Self::ROW_HEIGHT,
            hover_index: None,
            status_message: String::from("Ready"),
            operation_in_progress: false,
            operation_progress: 0.0,
            show_confirmation_dialog: false,
            confirmation_action: None,
        }
    }

    /// Refresh disk list
    pub fn refresh(&mut self) {
        self.status_message = String::from("Scanning disks...");
        self.disks.clear();

        // Create sample disks for demonstration
        let mut disk1 = DiskInfo::new("sda", "/dev/sda", DiskType::Ssd);
        disk1.model = String::from("Samsung 970 EVO Plus");
        disk1.serial = String::from("S4EVNX0R123456");
        disk1.firmware = String::from("2B2QEXM7");
        disk1.capacity = 500 * 1024 * 1024 * 1024; // 500GB
        disk1.sector_size = 512;
        disk1.partition_table = PartitionTable::Gpt;
        disk1.health = HealthStatus::Good;
        disk1.temperature = Some(35);

        let mut part1 = PartitionInfo::new("sda1", 1);
        part1.name = String::from("EFI System");
        part1.filesystem = FilesystemType::Fat32;
        part1.size = 512 * 1024 * 1024;
        part1.used = 32 * 1024 * 1024;
        part1.available = 480 * 1024 * 1024;
        part1.mount_point = Some(String::from("/boot/efi"));
        part1.is_bootable = true;
        part1.is_mounted = true;
        part1.flags.push(String::from("esp"));
        disk1.partitions.push(part1);

        let mut part2 = PartitionInfo::new("sda2", 2);
        part2.name = String::from("Linux Root");
        part2.label = Some(String::from("StenzelOS"));
        part2.filesystem = FilesystemType::Ext4;
        part2.size = 100 * 1024 * 1024 * 1024;
        part2.used = 25 * 1024 * 1024 * 1024;
        part2.available = 75 * 1024 * 1024 * 1024;
        part2.mount_point = Some(String::from("/"));
        part2.is_mounted = true;
        disk1.partitions.push(part2);

        let mut part3 = PartitionInfo::new("sda3", 3);
        part3.name = String::from("Linux Swap");
        part3.filesystem = FilesystemType::Swap;
        part3.size = 8 * 1024 * 1024 * 1024;
        part3.is_mounted = true;
        part3.flags.push(String::from("swap"));
        disk1.partitions.push(part3);

        self.disks.push(disk1);

        // USB drive
        let mut disk2 = DiskInfo::new("sdb", "/dev/sdb", DiskType::Usb);
        disk2.model = String::from("SanDisk Ultra");
        disk2.capacity = 64 * 1024 * 1024 * 1024;
        disk2.partition_table = PartitionTable::Mbr;
        disk2.health = HealthStatus::Good;
        disk2.is_removable = true;

        let mut usb_part = PartitionInfo::new("sdb1", 1);
        usb_part.name = String::from("USB Storage");
        usb_part.label = Some(String::from("BACKUP"));
        usb_part.filesystem = FilesystemType::ExFat;
        usb_part.size = 64 * 1024 * 1024 * 1024;
        usb_part.used = 20 * 1024 * 1024 * 1024;
        usb_part.available = 44 * 1024 * 1024 * 1024;
        disk2.partitions.push(usb_part);

        self.disks.push(disk2);

        self.status_message = format!("Found {} disks", self.disks.len());
    }

    /// Get selected disk
    pub fn get_selected_disk(&self) -> Option<&DiskInfo> {
        match &self.selected_item {
            SelectedItem::Disk(idx) | SelectedItem::Partition(idx, _) => {
                self.disks.get(*idx)
            }
            SelectedItem::None => None,
        }
    }

    /// Get selected partition
    pub fn get_selected_partition(&self) -> Option<&PartitionInfo> {
        if let SelectedItem::Partition(disk_idx, part_idx) = &self.selected_item {
            self.disks.get(*disk_idx)
                .and_then(|d| d.partitions.get(*part_idx))
        } else {
            None
        }
    }

    /// Mount partition
    pub fn mount(&mut self, mount_point: &str) -> Result<(), DiskError> {
        let _point = mount_point;
        self.status_message = String::from("Mounted successfully");
        Ok(())
    }

    /// Unmount partition
    pub fn unmount(&mut self) -> Result<(), DiskError> {
        self.status_message = String::from("Unmounted successfully");
        Ok(())
    }

    /// Format partition
    pub fn format(&mut self, options: FormatOptions) -> Result<(), DiskError> {
        self.operation_in_progress = true;
        self.operation_progress = 0.0;
        self.status_message = format!("Formatting as {}...", options.filesystem.name());

        // Simulate formatting
        for i in 0..=100 {
            self.operation_progress = i as f32;
        }

        self.operation_in_progress = false;
        self.status_message = String::from("Format complete");
        Ok(())
    }

    /// Create new partition
    pub fn create_partition(&mut self, _options: PartitionCreateOptions) -> Result<(), DiskError> {
        self.status_message = String::from("Partition created");
        Ok(())
    }

    /// Delete partition
    pub fn delete_partition(&mut self) -> Result<(), DiskError> {
        self.status_message = String::from("Partition deleted");
        Ok(())
    }

    /// Resize partition
    pub fn resize_partition(&mut self, _new_size: u64) -> Result<(), DiskError> {
        self.status_message = String::from("Partition resized");
        Ok(())
    }

    /// Create partition table
    pub fn create_partition_table(&mut self, table_type: PartitionTable) -> Result<(), DiskError> {
        self.status_message = format!("Created {} partition table", table_type.name());
        Ok(())
    }

    /// Erase disk
    pub fn erase_disk(&mut self) -> Result<(), DiskError> {
        self.operation_in_progress = true;
        self.operation_progress = 0.0;
        self.status_message = String::from("Erasing disk...");

        for i in 0..=100 {
            self.operation_progress = i as f32;
        }

        self.operation_in_progress = false;
        self.status_message = String::from("Disk erased");
        Ok(())
    }

    /// Get disk at point
    fn item_at_point(&self, x: isize, y: isize) -> Option<SelectedItem> {
        let base_x = self.bounds.x.max(0) as usize;
        let base_y = self.bounds.y.max(0) as usize;
        let sidebar_x = base_x;
        let list_y = base_y + Self::HEADER_HEIGHT;

        let ux = x.max(0) as usize;
        let uy = y.max(0) as usize;

        if ux < sidebar_x || ux >= sidebar_x + Self::SIDEBAR_WIDTH {
            return None;
        }
        if uy < list_y || uy >= base_y + self.bounds.height.saturating_sub(Self::STATUS_HEIGHT) {
            return None;
        }

        let row = (uy - list_y) / Self::ROW_HEIGHT;
        let mut current_row = 0;

        for (disk_idx, disk) in self.disks.iter().enumerate() {
            if current_row == row {
                return Some(SelectedItem::Disk(disk_idx));
            }
            current_row += 1;

            for (part_idx, _) in disk.partitions.iter().enumerate() {
                if current_row == row {
                    return Some(SelectedItem::Partition(disk_idx, part_idx));
                }
                current_row += 1;
            }
        }

        None
    }

    /// Format file size for display
    fn format_size(size: u64) -> String {
        if size < 1024 {
            return format!("{} B", size);
        }
        let kb = size as f64 / 1024.0;
        if kb < 1024.0 {
            return format!("{:.1} KB", kb);
        }
        let mb = kb / 1024.0;
        if mb < 1024.0 {
            return format!("{:.1} MB", mb);
        }
        let gb = mb / 1024.0;
        if gb < 1024.0 {
            return format!("{:.1} GB", gb);
        }
        let tb = gb / 1024.0;
        format!("{:.2} TB", tb)
    }
}

impl Widget for DiskUtility {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn bounds(&self) -> Bounds {
        self.bounds
    }

    fn set_position(&mut self, x: isize, y: isize) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn set_size(&mut self, width: usize, height: usize) {
        self.bounds.width = width;
        self.bounds.height = height;
        let content_height = height.saturating_sub(Self::HEADER_HEIGHT + Self::STATUS_HEIGHT);
        self.visible_rows = content_height / Self::ROW_HEIGHT;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        if !self.enabled {
            return false;
        }

        match event {
            WidgetEvent::Focus => {
                self.focused = true;
                true
            }
            WidgetEvent::Blur => {
                self.focused = false;
                true
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                if let Some(item) = self.item_at_point(*x, *y) {
                    self.selected_item = item;
                    return true;
                }
                false
            }
            WidgetEvent::MouseMove { x, y } => {
                // Track hover for sidebar items
                let _ = (x, y);
                true
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                if *delta_y < 0 {
                    self.scroll_offset = self.scroll_offset.saturating_add(1);
                } else if *delta_y > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                }
                true
            }
            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x72 | 0x52 => { // 'r' or 'R' - refresh
                        self.refresh();
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let _theme = theme();
        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        let bg_color = Color::new(248, 249, 250);
        let sidebar_bg = Color::new(52, 58, 64);
        let header_color = Color::new(233, 236, 239);
        let text_color = Color::BLACK;
        let text_light = Color::WHITE;
        let selected_bg = Color::new(0, 123, 255);
        let disk_color = Color::new(73, 80, 87);
        let partition_color = Color::new(108, 117, 125);

        // Background
        surface.fill_rect(x, y, w, h, bg_color);

        // Header
        surface.fill_rect(x, y, w, Self::HEADER_HEIGHT, header_color);
        draw_string(surface, x + 10, y + 12, "Disk Utility", text_color);

        // Toolbar buttons
        draw_string(surface, x + 150, y + 12, "[Refresh]", text_color);
        draw_string(surface, x + 230, y + 12, "[Mount]", text_color);
        draw_string(surface, x + 300, y + 12, "[Unmount]", text_color);
        draw_string(surface, x + 390, y + 12, "[Format]", text_color);
        draw_string(surface, x + 460, y + 12, "[Erase]", text_color);

        // Sidebar
        let sidebar_y = y + Self::HEADER_HEIGHT;
        let sidebar_h = h.saturating_sub(Self::HEADER_HEIGHT + Self::STATUS_HEIGHT);
        surface.fill_rect(x, sidebar_y, Self::SIDEBAR_WIDTH, sidebar_h, sidebar_bg);

        // Disk list in sidebar
        let mut current_y = sidebar_y + 4;
        for (disk_idx, disk) in self.disks.iter().enumerate() {
            let is_disk_selected = matches!(&self.selected_item,
                SelectedItem::Disk(idx) if *idx == disk_idx);
            let is_partition_parent = matches!(&self.selected_item,
                SelectedItem::Partition(idx, _) if *idx == disk_idx);

            // Disk entry
            if is_disk_selected {
                surface.fill_rect(x, current_y, Self::SIDEBAR_WIDTH, Self::ROW_HEIGHT, selected_bg);
            } else {
                surface.fill_rect(x, current_y, Self::SIDEBAR_WIDTH, Self::ROW_HEIGHT, disk_color);
            }

            draw_string(surface, x + 4, current_y + 5, disk.disk_type.icon(), text_light);
            let disk_label = if disk.name.len() > 18 {
                format!("{}...", &disk.name[..15])
            } else {
                disk.name.clone()
            };
            draw_string(surface, x + 44, current_y + 5, &disk_label, text_light);
            current_y += Self::ROW_HEIGHT;

            // Partitions
            for (part_idx, partition) in disk.partitions.iter().enumerate() {
                let is_part_selected = matches!(&self.selected_item,
                    SelectedItem::Partition(d, p) if *d == disk_idx && *p == part_idx);

                if is_part_selected {
                    surface.fill_rect(x, current_y, Self::SIDEBAR_WIDTH, Self::ROW_HEIGHT, selected_bg);
                } else {
                    surface.fill_rect(x, current_y, Self::SIDEBAR_WIDTH, Self::ROW_HEIGHT, partition_color);
                }

                let part_label = partition.label.as_ref()
                    .map(|l| l.as_str())
                    .unwrap_or(&partition.name);
                let part_display = if part_label.len() > 20 {
                    format!("  {}...", &part_label[..17])
                } else {
                    format!("  {}", part_label)
                };
                draw_string(surface, x + 16, current_y + 5, &part_display, text_light);
                current_y += Self::ROW_HEIGHT;
            }
        }

        // Main content area
        let content_x = x + Self::SIDEBAR_WIDTH;
        let content_w = w.saturating_sub(Self::SIDEBAR_WIDTH);
        let content_y = y + Self::HEADER_HEIGHT;
        let content_h = h.saturating_sub(Self::HEADER_HEIGHT + Self::STATUS_HEIGHT);

        surface.fill_rect(content_x, content_y, content_w, content_h, bg_color);

        // Draw selected item details
        match &self.selected_item {
            SelectedItem::Disk(idx) => {
                if let Some(disk) = self.disks.get(*idx) {
                    let info_x = content_x + 20;
                    let mut info_y = content_y + 20;

                    draw_string(surface, info_x, info_y, "Disk Information", text_color);
                    info_y += 30;

                    draw_string(surface, info_x, info_y, &format!("Model: {}", disk.model), text_color);
                    info_y += 20;

                    draw_string(surface, info_x, info_y, &format!("Type: {}", disk.disk_type.name()), text_color);
                    info_y += 20;

                    draw_string(surface, info_x, info_y, &format!("Capacity: {}", Self::format_size(disk.capacity)), text_color);
                    info_y += 20;

                    draw_string(surface, info_x, info_y, &format!("Partition Table: {}", disk.partition_table.name()), text_color);
                    info_y += 20;

                    draw_string(surface, info_x, info_y, &format!("Serial: {}", disk.serial), text_color);
                    info_y += 20;

                    let health_text = format!("Health: {}", disk.health.name());
                    draw_string(surface, info_x, info_y, &health_text, disk.health.color());
                    info_y += 20;

                    if let Some(temp) = disk.temperature {
                        draw_string(surface, info_x, info_y, &format!("Temperature: {}°C", temp), text_color);
                        info_y += 20;
                    }

                    // Partition overview
                    info_y += 20;
                    draw_string(surface, info_x, info_y, &format!("Partitions: {}", disk.partitions.len()), text_color);
                }
            }
            SelectedItem::Partition(disk_idx, part_idx) => {
                if let Some(disk) = self.disks.get(*disk_idx) {
                    if let Some(part) = disk.partitions.get(*part_idx) {
                        let info_x = content_x + 20;
                        let mut info_y = content_y + 20;

                        draw_string(surface, info_x, info_y, "Partition Information", text_color);
                        info_y += 30;

                        if let Some(ref label) = part.label {
                            draw_string(surface, info_x, info_y, &format!("Label: {}", label), text_color);
                            info_y += 20;
                        }

                        draw_string(surface, info_x, info_y, &format!("Filesystem: {}", part.filesystem.name()), text_color);
                        info_y += 20;

                        draw_string(surface, info_x, info_y, &format!("Size: {}", Self::format_size(part.size)), text_color);
                        info_y += 20;

                        draw_string(surface, info_x, info_y, &format!("Used: {} ({:.1}%)", Self::format_size(part.used), part.usage_percent()), text_color);
                        info_y += 20;

                        draw_string(surface, info_x, info_y, &format!("Available: {}", Self::format_size(part.available)), text_color);
                        info_y += 20;

                        if let Some(ref mount) = part.mount_point {
                            draw_string(surface, info_x, info_y, &format!("Mount Point: {}", mount), text_color);
                            info_y += 20;
                        }

                        let status = if part.is_mounted { "Mounted" } else { "Not Mounted" };
                        draw_string(surface, info_x, info_y, &format!("Status: {}", status), text_color);
                        info_y += 20;

                        // Usage bar
                        info_y += 10;
                        let bar_width = 300;
                        let bar_height = 20;
                        surface.fill_rect(info_x, info_y, bar_width, bar_height, Color::new(200, 200, 200));
                        let used_width = ((bar_width as f32 * part.usage_percent() / 100.0) as usize).min(bar_width);
                        let bar_color = if part.usage_percent() > 90.0 {
                            Color::new(220, 53, 69)
                        } else if part.usage_percent() > 70.0 {
                            Color::new(255, 193, 7)
                        } else {
                            Color::new(40, 167, 69)
                        };
                        surface.fill_rect(info_x, info_y, used_width, bar_height, bar_color);
                    }
                }
            }
            SelectedItem::None => {
                draw_string(surface, content_x + 20, content_y + 20, "Select a disk or partition from the sidebar", text_color);
                draw_string(surface, content_x + 20, content_y + 50, "Press 'R' to refresh disk list", text_color);
            }
        }

        // Status bar
        let status_y = y + h.saturating_sub(Self::STATUS_HEIGHT);
        surface.fill_rect(x, status_y, w, Self::STATUS_HEIGHT, header_color);
        draw_string(surface, x + 5, status_y + 5, &self.status_message, text_color);

        // Progress bar if operation in progress
        if self.operation_in_progress {
            let progress_w = 200;
            let progress_x = w.saturating_sub(progress_w + 10);
            surface.fill_rect(x + progress_x, status_y + 4, progress_w, 16, Color::new(200, 200, 200));
            let filled = ((progress_w as f32 * self.operation_progress / 100.0) as usize).min(progress_w);
            surface.fill_rect(x + progress_x, status_y + 4, filled, 16, selected_bg);
        }
    }
}

/// Draw a single character using the system font
fn draw_char(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;
    if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
        for row in 0..DEFAULT_FONT.height {
            let byte = glyph[row];
            for col in 0..DEFAULT_FONT.width {
                if (byte >> (DEFAULT_FONT.width - 1 - col)) & 1 != 0 {
                    surface.set_pixel(x + col, y + row, color);
                }
            }
        }
    }
}

/// Draw a string using the system font
fn draw_string(surface: &mut Surface, x: usize, y: usize, s: &str, color: Color) {
    for (i, c) in s.chars().enumerate() {
        draw_char(surface, x + i * 8, y, c, color);
    }
}

/// Initialize disk utility
pub fn init() {
    crate::kprintln!("diskutil: Disk Utility initialized");
}
