//! Printer Settings Application
//!
//! Configuration and management of printers, print queues, and print jobs.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton};
use crate::gui::surface::Surface;
use crate::drivers::framebuffer::Color;

/// Printer connection type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Usb,
    Network,
    Bluetooth,
    Serial,
    Parallel,
    Virtual,
}

impl ConnectionType {
    pub fn name(&self) -> &'static str {
        match self {
            ConnectionType::Usb => "USB",
            ConnectionType::Network => "Network",
            ConnectionType::Bluetooth => "Bluetooth",
            ConnectionType::Serial => "Serial",
            ConnectionType::Parallel => "Parallel",
            ConnectionType::Virtual => "Virtual",
        }
    }

    pub fn icon(&self) -> char {
        match self {
            ConnectionType::Usb => '🔌',
            ConnectionType::Network => '🌐',
            ConnectionType::Bluetooth => '📶',
            ConnectionType::Serial => '📡',
            ConnectionType::Parallel => '🔗',
            ConnectionType::Virtual => '💻',
        }
    }
}

/// Printer type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrinterType {
    Laser,
    Inkjet,
    DotMatrix,
    Thermal,
    Label,
    ThreeD,
    Virtual,
}

impl PrinterType {
    pub fn name(&self) -> &'static str {
        match self {
            PrinterType::Laser => "Laser",
            PrinterType::Inkjet => "Inkjet",
            PrinterType::DotMatrix => "Dot Matrix",
            PrinterType::Thermal => "Thermal",
            PrinterType::Label => "Label",
            PrinterType::ThreeD => "3D Printer",
            PrinterType::Virtual => "Virtual",
        }
    }
}

/// Printer state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrinterState {
    Idle,
    Printing,
    Paused,
    Error,
    Offline,
    OutOfPaper,
    OutOfInk,
    Jammed,
    Warming,
}

impl PrinterState {
    pub fn name(&self) -> &'static str {
        match self {
            PrinterState::Idle => "Ready",
            PrinterState::Printing => "Printing",
            PrinterState::Paused => "Paused",
            PrinterState::Error => "Error",
            PrinterState::Offline => "Offline",
            PrinterState::OutOfPaper => "Out of Paper",
            PrinterState::OutOfInk => "Low Ink/Toner",
            PrinterState::Jammed => "Paper Jam",
            PrinterState::Warming => "Warming Up",
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, PrinterState::Idle | PrinterState::Warming)
    }

    pub fn color(&self) -> Color {
        match self {
            PrinterState::Idle => Color::new(80, 200, 80),
            PrinterState::Printing => Color::new(100, 150, 255),
            PrinterState::Paused => Color::new(255, 200, 50),
            PrinterState::Warming => Color::new(255, 200, 50),
            PrinterState::Error | PrinterState::Jammed => Color::new(255, 80, 80),
            PrinterState::OutOfPaper | PrinterState::OutOfInk => Color::new(255, 150, 50),
            PrinterState::Offline => Color::new(150, 150, 150),
        }
    }
}

/// Paper size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperSize {
    Letter,
    Legal,
    A4,
    A3,
    A5,
    B5,
    Tabloid,
    Envelope,
    Photo4x6,
    Photo5x7,
    Custom,
}

impl PaperSize {
    pub fn name(&self) -> &'static str {
        match self {
            PaperSize::Letter => "Letter (8.5x11)",
            PaperSize::Legal => "Legal (8.5x14)",
            PaperSize::A4 => "A4 (210x297mm)",
            PaperSize::A3 => "A3 (297x420mm)",
            PaperSize::A5 => "A5 (148x210mm)",
            PaperSize::B5 => "B5 (176x250mm)",
            PaperSize::Tabloid => "Tabloid (11x17)",
            PaperSize::Envelope => "Envelope",
            PaperSize::Photo4x6 => "4x6 Photo",
            PaperSize::Photo5x7 => "5x7 Photo",
            PaperSize::Custom => "Custom",
        }
    }

    pub fn dimensions_mm(&self) -> (f32, f32) {
        match self {
            PaperSize::Letter => (215.9, 279.4),
            PaperSize::Legal => (215.9, 355.6),
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::A3 => (297.0, 420.0),
            PaperSize::A5 => (148.0, 210.0),
            PaperSize::B5 => (176.0, 250.0),
            PaperSize::Tabloid => (279.4, 431.8),
            PaperSize::Envelope => (110.0, 220.0),
            PaperSize::Photo4x6 => (101.6, 152.4),
            PaperSize::Photo5x7 => (127.0, 177.8),
            PaperSize::Custom => (210.0, 297.0),
        }
    }
}

/// Paper type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperType {
    Plain,
    Photo,
    Glossy,
    Matte,
    Cardstock,
    Labels,
    Transparent,
    Recycled,
}

impl PaperType {
    pub fn name(&self) -> &'static str {
        match self {
            PaperType::Plain => "Plain Paper",
            PaperType::Photo => "Photo Paper",
            PaperType::Glossy => "Glossy",
            PaperType::Matte => "Matte",
            PaperType::Cardstock => "Cardstock",
            PaperType::Labels => "Labels",
            PaperType::Transparent => "Transparency",
            PaperType::Recycled => "Recycled",
        }
    }
}

/// Print quality
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintQuality {
    Draft,
    Normal,
    High,
    Best,
}

impl PrintQuality {
    pub fn name(&self) -> &'static str {
        match self {
            PrintQuality::Draft => "Draft",
            PrintQuality::Normal => "Normal",
            PrintQuality::High => "High",
            PrintQuality::Best => "Best",
        }
    }

    pub fn dpi(&self) -> u32 {
        match self {
            PrintQuality::Draft => 150,
            PrintQuality::Normal => 300,
            PrintQuality::High => 600,
            PrintQuality::Best => 1200,
        }
    }
}

/// Color mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Color,
    Grayscale,
    BlackWhite,
}

impl ColorMode {
    pub fn name(&self) -> &'static str {
        match self {
            ColorMode::Color => "Color",
            ColorMode::Grayscale => "Grayscale",
            ColorMode::BlackWhite => "Black & White",
        }
    }
}

/// Duplex mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplexMode {
    None,
    LongEdge,
    ShortEdge,
}

impl DuplexMode {
    pub fn name(&self) -> &'static str {
        match self {
            DuplexMode::None => "Single-sided",
            DuplexMode::LongEdge => "Double-sided (Long Edge)",
            DuplexMode::ShortEdge => "Double-sided (Short Edge)",
        }
    }
}

/// Paper tray/source
#[derive(Debug, Clone)]
pub struct PaperTray {
    pub id: u8,
    pub name: String,
    pub paper_size: PaperSize,
    pub paper_type: PaperType,
    pub capacity: usize,
    pub level: usize,
}

impl PaperTray {
    pub fn percentage(&self) -> usize {
        if self.capacity == 0 { 0 } else { self.level * 100 / self.capacity }
    }
}

/// Ink/toner cartridge
#[derive(Debug, Clone)]
pub struct InkCartridge {
    pub id: u8,
    pub name: String,
    pub color: CartridgeColor,
    pub level: u8,
    pub is_low: bool,
    pub is_empty: bool,
}

impl InkCartridge {
    pub fn status(&self) -> &'static str {
        if self.is_empty { "Empty" }
        else if self.is_low { "Low" }
        else if self.level > 50 { "Good" }
        else { "OK" }
    }

    pub fn display_color(&self) -> Color {
        match self.color {
            CartridgeColor::Black => Color::new(40, 40, 40),
            CartridgeColor::Cyan => Color::new(0, 200, 255),
            CartridgeColor::Magenta => Color::new(255, 0, 200),
            CartridgeColor::Yellow => Color::new(255, 230, 0),
            CartridgeColor::Photo => Color::new(150, 150, 150),
            CartridgeColor::LightCyan => Color::new(150, 230, 255),
            CartridgeColor::LightMagenta => Color::new(255, 150, 230),
        }
    }
}

/// Cartridge color
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgeColor {
    Black,
    Cyan,
    Magenta,
    Yellow,
    Photo,
    LightCyan,
    LightMagenta,
}

impl CartridgeColor {
    pub fn name(&self) -> &'static str {
        match self {
            CartridgeColor::Black => "Black",
            CartridgeColor::Cyan => "Cyan",
            CartridgeColor::Magenta => "Magenta",
            CartridgeColor::Yellow => "Yellow",
            CartridgeColor::Photo => "Photo Black",
            CartridgeColor::LightCyan => "Light Cyan",
            CartridgeColor::LightMagenta => "Light Magenta",
        }
    }
}

/// Print job status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Processing,
    Printing,
    Paused,
    Completed,
    Cancelled,
    Error,
}

impl JobStatus {
    pub fn name(&self) -> &'static str {
        match self {
            JobStatus::Pending => "Pending",
            JobStatus::Processing => "Processing",
            JobStatus::Printing => "Printing",
            JobStatus::Paused => "Paused",
            JobStatus::Completed => "Completed",
            JobStatus::Cancelled => "Cancelled",
            JobStatus::Error => "Error",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, JobStatus::Pending | JobStatus::Processing | JobStatus::Printing)
    }
}

/// Print job
#[derive(Debug, Clone)]
pub struct PrintJob {
    pub id: u64,
    pub printer_id: u64,
    pub document_name: String,
    pub owner: String,
    pub status: JobStatus,
    pub pages_total: usize,
    pub pages_printed: usize,
    pub copies: usize,
    pub submitted: u64,
    pub started: Option<u64>,
    pub completed: Option<u64>,
    pub size_bytes: u64,
    pub priority: u8,
    pub error_message: Option<String>,
}

impl PrintJob {
    pub fn new(id: u64, printer_id: u64, document_name: &str, owner: &str) -> Self {
        Self {
            id,
            printer_id,
            document_name: document_name.to_string(),
            owner: owner.to_string(),
            status: JobStatus::Pending,
            pages_total: 1,
            pages_printed: 0,
            copies: 1,
            submitted: 0,
            started: None,
            completed: None,
            size_bytes: 0,
            priority: 50,
            error_message: None,
        }
    }

    pub fn progress(&self) -> usize {
        if self.pages_total == 0 { 0 }
        else { self.pages_printed * 100 / self.pages_total }
    }

    pub fn format_size(&self) -> String {
        if self.size_bytes < 1024 {
            format!("{} B", self.size_bytes)
        } else if self.size_bytes < 1024 * 1024 {
            format!("{} KB", self.size_bytes / 1024)
        } else {
            format!("{:.1} MB", self.size_bytes as f32 / (1024.0 * 1024.0))
        }
    }
}

/// Printer capabilities
#[derive(Debug, Clone)]
pub struct PrinterCapabilities {
    pub supports_color: bool,
    pub supports_duplex: bool,
    pub max_dpi: u32,
    pub paper_sizes: Vec<PaperSize>,
    pub paper_types: Vec<PaperType>,
    pub max_paper_width_mm: f32,
    pub max_paper_height_mm: f32,
    pub pages_per_minute: u32,
    pub supports_borderless: bool,
    pub supports_stapling: bool,
    pub supports_hole_punch: bool,
}

impl Default for PrinterCapabilities {
    fn default() -> Self {
        Self {
            supports_color: true,
            supports_duplex: true,
            max_dpi: 1200,
            paper_sizes: vec![PaperSize::Letter, PaperSize::A4, PaperSize::Legal],
            paper_types: vec![PaperType::Plain, PaperType::Photo],
            max_paper_width_mm: 216.0,
            max_paper_height_mm: 356.0,
            pages_per_minute: 20,
            supports_borderless: false,
            supports_stapling: false,
            supports_hole_punch: false,
        }
    }
}

/// Printer settings
#[derive(Debug, Clone)]
pub struct PrintSettings {
    pub paper_size: PaperSize,
    pub paper_type: PaperType,
    pub quality: PrintQuality,
    pub color_mode: ColorMode,
    pub duplex: DuplexMode,
    pub copies: usize,
    pub collate: bool,
    pub reverse_order: bool,
    pub borderless: bool,
    pub scale_to_fit: bool,
    pub scale_percentage: u32,
    pub orientation_landscape: bool,
    pub pages_per_sheet: u8,
    pub selected_tray: Option<u8>,
}

impl Default for PrintSettings {
    fn default() -> Self {
        Self {
            paper_size: PaperSize::Letter,
            paper_type: PaperType::Plain,
            quality: PrintQuality::Normal,
            color_mode: ColorMode::Color,
            duplex: DuplexMode::None,
            copies: 1,
            collate: true,
            reverse_order: false,
            borderless: false,
            scale_to_fit: true,
            scale_percentage: 100,
            orientation_landscape: false,
            pages_per_sheet: 1,
            selected_tray: None,
        }
    }
}

/// Printer
#[derive(Debug, Clone)]
pub struct Printer {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub location: String,
    pub manufacturer: String,
    pub model: String,
    pub driver_name: String,
    pub connection_type: ConnectionType,
    pub printer_type: PrinterType,
    pub state: PrinterState,
    pub is_default: bool,
    pub is_shared: bool,
    pub capabilities: PrinterCapabilities,
    pub settings: PrintSettings,
    pub trays: Vec<PaperTray>,
    pub cartridges: Vec<InkCartridge>,
    pub uri: String,
    pub serial_number: Option<String>,
    pub pages_printed_total: u64,
}

impl Printer {
    pub fn new(id: u64, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            description: String::new(),
            location: String::new(),
            manufacturer: String::new(),
            model: String::new(),
            driver_name: String::new(),
            connection_type: ConnectionType::Usb,
            printer_type: PrinterType::Laser,
            state: PrinterState::Idle,
            is_default: false,
            is_shared: false,
            capabilities: PrinterCapabilities::default(),
            settings: PrintSettings::default(),
            trays: Vec::new(),
            cartridges: Vec::new(),
            uri: String::new(),
            serial_number: None,
            pages_printed_total: 0,
        }
    }

    pub fn display_name(&self) -> String {
        if self.is_default {
            format!("{} (Default)", self.name)
        } else {
            self.name.clone()
        }
    }

    pub fn status_summary(&self) -> String {
        match self.state {
            PrinterState::Idle => String::from("Ready to print"),
            PrinterState::Printing => String::from("Printing in progress"),
            PrinterState::Paused => String::from("Print queue paused"),
            PrinterState::Error => String::from("Error - check printer"),
            PrinterState::Offline => String::from("Printer is offline"),
            PrinterState::OutOfPaper => String::from("Add paper to continue"),
            PrinterState::OutOfInk => String::from("Replace ink/toner"),
            PrinterState::Jammed => String::from("Clear paper jam"),
            PrinterState::Warming => String::from("Warming up..."),
        }
    }
}

/// View mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Printers,
    PrintQueue,
    Settings,
    AddPrinter,
}

// Helper functions for rendering
fn draw_char_at(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;
    if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
        for row in 0..DEFAULT_FONT.height {
            let byte = glyph[row];
            for col in 0..DEFAULT_FONT.width {
                if (byte >> (7 - col)) & 1 != 0 {
                    surface.set_pixel(x + col, y + row, color);
                }
            }
        }
    }
}

fn draw_char(surface: &mut Surface, x: isize, y: isize, c: char, color: Color) {
    if x >= 0 && y >= 0 {
        draw_char_at(surface, x as usize, y as usize, c, color);
    }
}

fn draw_string(surface: &mut Surface, x: isize, y: isize, s: &str, color: Color) {
    if x < 0 || y < 0 { return; }
    let mut px = x as usize;
    for c in s.chars() {
        draw_char_at(surface, px, y as usize, c, color);
        px += 8;
    }
}

/// Printer settings widget
pub struct PrinterSettingsApp {
    id: WidgetId,
    bounds: Bounds,
    enabled: bool,
    visible: bool,

    // Data
    printers: Vec<Printer>,
    print_queue: Vec<PrintJob>,
    next_printer_id: u64,
    next_job_id: u64,

    // View state
    view_mode: ViewMode,
    selected_printer_id: Option<u64>,
    selected_job_id: Option<u64>,
    scroll_offset: usize,
    hovered_index: Option<usize>,

    // UI state
    sidebar_width: usize,
    show_supplies: bool,
    show_advanced: bool,
}

impl PrinterSettingsApp {
    pub fn new(id: WidgetId) -> Self {
        let mut app = Self {
            id,
            bounds: Bounds { x: 0, y: 0, width: 850, height: 550 },
            enabled: true,
            visible: true,
            printers: Vec::new(),
            print_queue: Vec::new(),
            next_printer_id: 1,
            next_job_id: 1,
            view_mode: ViewMode::Printers,
            selected_printer_id: None,
            selected_job_id: None,
            scroll_offset: 0,
            hovered_index: None,
            sidebar_width: 200,
            show_supplies: false,
            show_advanced: false,
        };

        app.add_sample_data();
        app
    }

    fn add_sample_data(&mut self) {
        // Add sample printers
        let mut printer1 = Printer::new(self.next_printer_id, "HP LaserJet Pro");
        printer1.description = String::from("Network laser printer");
        printer1.location = String::from("Office");
        printer1.manufacturer = String::from("HP");
        printer1.model = String::from("LaserJet Pro M404dn");
        printer1.driver_name = String::from("hp-laserjet-pro-m404");
        printer1.connection_type = ConnectionType::Network;
        printer1.printer_type = PrinterType::Laser;
        printer1.is_default = true;
        printer1.uri = String::from("ipp://192.168.1.50:631/ipp/print");
        printer1.trays = vec![
            PaperTray { id: 1, name: String::from("Tray 1"), paper_size: PaperSize::Letter, paper_type: PaperType::Plain, capacity: 250, level: 180 },
            PaperTray { id: 2, name: String::from("Tray 2"), paper_size: PaperSize::Legal, paper_type: PaperType::Plain, capacity: 500, level: 350 },
        ];
        printer1.cartridges = vec![
            InkCartridge { id: 1, name: String::from("Black Toner"), color: CartridgeColor::Black, level: 65, is_low: false, is_empty: false },
        ];
        printer1.pages_printed_total = 15420;
        self.printers.push(printer1);
        self.next_printer_id += 1;

        let mut printer2 = Printer::new(self.next_printer_id, "Epson WorkForce");
        printer2.description = String::from("Color inkjet printer");
        printer2.location = String::from("Home Office");
        printer2.manufacturer = String::from("Epson");
        printer2.model = String::from("WorkForce WF-7720");
        printer2.driver_name = String::from("epson-inkjet-wf7720");
        printer2.connection_type = ConnectionType::Usb;
        printer2.printer_type = PrinterType::Inkjet;
        printer2.capabilities.supports_color = true;
        printer2.capabilities.supports_borderless = true;
        printer2.cartridges = vec![
            InkCartridge { id: 1, name: String::from("Black"), color: CartridgeColor::Black, level: 45, is_low: false, is_empty: false },
            InkCartridge { id: 2, name: String::from("Cyan"), color: CartridgeColor::Cyan, level: 78, is_low: false, is_empty: false },
            InkCartridge { id: 3, name: String::from("Magenta"), color: CartridgeColor::Magenta, level: 30, is_low: true, is_empty: false },
            InkCartridge { id: 4, name: String::from("Yellow"), color: CartridgeColor::Yellow, level: 55, is_low: false, is_empty: false },
        ];
        printer2.trays = vec![
            PaperTray { id: 1, name: String::from("Main Tray"), paper_size: PaperSize::A4, paper_type: PaperType::Plain, capacity: 200, level: 120 },
        ];
        printer2.pages_printed_total = 3250;
        self.printers.push(printer2);
        self.next_printer_id += 1;

        let mut printer3 = Printer::new(self.next_printer_id, "Print to PDF");
        printer3.description = String::from("Virtual PDF printer");
        printer3.connection_type = ConnectionType::Virtual;
        printer3.printer_type = PrinterType::Virtual;
        printer3.driver_name = String::from("cups-pdf");
        printer3.uri = String::from("cups-pdf:/");
        self.printers.push(printer3);
        self.next_printer_id += 1;

        // Select first printer
        self.selected_printer_id = Some(1);

        // Add sample print jobs
        let mut job1 = PrintJob::new(self.next_job_id, 1, "Annual Report.pdf", "John");
        job1.pages_total = 25;
        job1.pages_printed = 12;
        job1.status = JobStatus::Printing;
        job1.size_bytes = 2_500_000;
        self.print_queue.push(job1);
        self.next_job_id += 1;

        let mut job2 = PrintJob::new(self.next_job_id, 1, "Meeting Notes.docx", "Sarah");
        job2.pages_total = 3;
        job2.status = JobStatus::Pending;
        job2.size_bytes = 150_000;
        self.print_queue.push(job2);
        self.next_job_id += 1;

        let mut job3 = PrintJob::new(self.next_job_id, 2, "Photo Album.jpg", "Mike");
        job3.pages_total = 10;
        job3.copies = 2;
        job3.status = JobStatus::Pending;
        job3.size_bytes = 15_000_000;
        self.print_queue.push(job3);
        self.next_job_id += 1;
    }

    // Printer management
    pub fn add_printer(&mut self, name: &str) -> u64 {
        let printer = Printer::new(self.next_printer_id, name);
        let id = printer.id;
        self.printers.push(printer);
        self.next_printer_id += 1;
        id
    }

    pub fn remove_printer(&mut self, printer_id: u64) {
        // Cancel all jobs for this printer
        for job in &mut self.print_queue {
            if job.printer_id == printer_id && job.status.is_active() {
                job.status = JobStatus::Cancelled;
            }
        }
        self.printers.retain(|p| p.id != printer_id);

        if self.selected_printer_id == Some(printer_id) {
            self.selected_printer_id = self.printers.first().map(|p| p.id);
        }
    }

    pub fn set_default_printer(&mut self, printer_id: u64) {
        for printer in &mut self.printers {
            printer.is_default = printer.id == printer_id;
        }
    }

    pub fn get_printer(&self, printer_id: u64) -> Option<&Printer> {
        self.printers.iter().find(|p| p.id == printer_id)
    }

    pub fn get_printer_mut(&mut self, printer_id: u64) -> Option<&mut Printer> {
        self.printers.iter_mut().find(|p| p.id == printer_id)
    }

    // Print queue management
    pub fn cancel_job(&mut self, job_id: u64) {
        if let Some(job) = self.print_queue.iter_mut().find(|j| j.id == job_id) {
            if job.status.is_active() {
                job.status = JobStatus::Cancelled;
            }
        }
    }

    pub fn pause_job(&mut self, job_id: u64) {
        if let Some(job) = self.print_queue.iter_mut().find(|j| j.id == job_id) {
            if job.status == JobStatus::Printing || job.status == JobStatus::Pending {
                job.status = JobStatus::Paused;
            }
        }
    }

    pub fn resume_job(&mut self, job_id: u64) {
        if let Some(job) = self.print_queue.iter_mut().find(|j| j.id == job_id) {
            if job.status == JobStatus::Paused {
                job.status = JobStatus::Pending;
            }
        }
    }

    pub fn clear_completed_jobs(&mut self) {
        self.print_queue.retain(|j| j.status.is_active());
    }

    pub fn pause_printer(&mut self, printer_id: u64) {
        if let Some(printer) = self.printers.iter_mut().find(|p| p.id == printer_id) {
            if printer.state == PrinterState::Printing || printer.state == PrinterState::Idle {
                printer.state = PrinterState::Paused;
            }
        }
    }

    pub fn resume_printer(&mut self, printer_id: u64) {
        if let Some(printer) = self.printers.iter_mut().find(|p| p.id == printer_id) {
            if printer.state == PrinterState::Paused {
                printer.state = PrinterState::Idle;
            }
        }
    }

    // UI helpers
    fn get_visible_count(&self) -> usize {
        let content_height = self.bounds.height.saturating_sub(100);
        content_height / 50
    }

    fn jobs_for_printer(&self, printer_id: u64) -> Vec<&PrintJob> {
        self.print_queue.iter().filter(|j| j.printer_id == printer_id).collect()
    }
}

impl Widget for PrinterSettingsApp {
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
        match event {
            WidgetEvent::MouseDown { x, y, button } => {
                if *button != MouseButton::Left {
                    return false;
                }

                // Sidebar clicks
                if *x >= self.bounds.x && *x < self.bounds.x + self.sidebar_width as isize {
                    let rel_y = *y - self.bounds.y - 60;
                    if rel_y >= 0 {
                        let index = (rel_y / 40) as usize;
                        if index < self.printers.len() {
                            self.selected_printer_id = Some(self.printers[index].id);
                            self.view_mode = ViewMode::Printers;
                            return true;
                        }
                    }
                }

                // Toolbar buttons
                let toolbar_y = self.bounds.y + 15;
                let content_x = self.bounds.x + self.sidebar_width as isize + 10;

                if *y >= toolbar_y && *y < toolbar_y + 30 {
                    if *x >= content_x && *x < content_x + 60 {
                        self.view_mode = ViewMode::AddPrinter;
                        return true;
                    }
                    if *x >= content_x + 70 && *x < content_x + 130 {
                        self.view_mode = ViewMode::PrintQueue;
                        return true;
                    }
                    if *x >= content_x + 140 && *x < content_x + 200 {
                        self.view_mode = ViewMode::Settings;
                        return true;
                    }
                }

                false
            }

            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x1B => { // Escape
                        if self.view_mode != ViewMode::Printers {
                            self.view_mode = ViewMode::Printers;
                            return true;
                        }
                        false
                    }
                    _ => false,
                }
            }

            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        let bg = Color::new(30, 30, 35);
        let sidebar_bg = Color::new(25, 25, 30);
        let text_color = Color::new(230, 230, 230);
        let dim_text = Color::new(150, 150, 155);
        let accent_color = Color::new(100, 180, 255);
        let selected_bg = Color::new(60, 60, 70);
        let border_color = Color::new(60, 60, 65);
        let ready_color = Color::new(80, 200, 80);

        // Background
        for y in 0..self.bounds.height {
            for x in 0..self.bounds.width {
                surface.set_pixel(
                    (self.bounds.x as usize) + x,
                    (self.bounds.y as usize) + y,
                    bg
                );
            }
        }

        // Sidebar
        for y in 0..self.bounds.height {
            for x in 0..self.sidebar_width {
                surface.set_pixel(
                    (self.bounds.x as usize) + x,
                    (self.bounds.y as usize) + y,
                    sidebar_bg
                );
            }
        }

        // Sidebar title
        draw_string(surface, self.bounds.x + 15, self.bounds.y + 20, "Printers", accent_color);

        // Printer list in sidebar
        let mut sidebar_y = self.bounds.y + 60;
        for printer in &self.printers {
            let is_selected = self.selected_printer_id == Some(printer.id);

            if is_selected {
                for y in 0..38 {
                    for x in 0..self.sidebar_width {
                        surface.set_pixel(
                            (self.bounds.x as usize) + x,
                            (sidebar_y as usize) + y,
                            selected_bg
                        );
                    }
                }
            }

            // Status dot
            draw_char(surface, self.bounds.x + 10, sidebar_y + 8, '●', printer.state.color());

            // Printer name
            let name = if printer.name.len() > 20 {
                let mut n: String = printer.name.chars().take(17).collect();
                n.push_str("...");
                n
            } else {
                printer.name.clone()
            };
            draw_string(surface, self.bounds.x + 25, sidebar_y + 8, &name,
                if is_selected { accent_color } else { text_color });

            // Status
            draw_string(surface, self.bounds.x + 25, sidebar_y + 22, printer.state.name(), dim_text);

            // Default indicator
            if printer.is_default {
                draw_char(surface, self.bounds.x + self.sidebar_width as isize - 20, sidebar_y + 12, '★', accent_color);
            }

            sidebar_y += 40;
        }

        // Add printer button
        draw_string(surface, self.bounds.x + 15, sidebar_y + 15, "+ Add Printer", dim_text);

        // Sidebar border
        for y in 0..self.bounds.height {
            surface.set_pixel(
                (self.bounds.x as usize) + self.sidebar_width,
                (self.bounds.y as usize) + y,
                border_color
            );
        }

        // Content area
        let content_x = self.bounds.x + self.sidebar_width as isize + 15;
        let content_y = self.bounds.y + 60;

        // Toolbar
        draw_string(surface, content_x, self.bounds.y + 20, "[Add]", dim_text);
        draw_string(surface, content_x + 70, self.bounds.y + 20, "[Queue]", dim_text);
        draw_string(surface, content_x + 140, self.bounds.y + 20, "[Settings]", dim_text);

        match self.view_mode {
            ViewMode::Printers => {
                if let Some(printer_id) = self.selected_printer_id {
                    if let Some(printer) = self.get_printer(printer_id) {
                        // Printer details
                        draw_string(surface, content_x, content_y, &printer.name, accent_color);
                        draw_string(surface, content_x, content_y + 25, &printer.status_summary(), text_color);

                        // Info section
                        let info_y = content_y + 60;
                        draw_string(surface, content_x, info_y, "INFORMATION", dim_text);

                        draw_string(surface, content_x, info_y + 25, "Type:", dim_text);
                        draw_string(surface, content_x + 120, info_y + 25, printer.printer_type.name(), text_color);

                        draw_string(surface, content_x, info_y + 45, "Connection:", dim_text);
                        draw_string(surface, content_x + 120, info_y + 45, printer.connection_type.name(), text_color);

                        draw_string(surface, content_x, info_y + 65, "Location:", dim_text);
                        let location = if printer.location.is_empty() { "-" } else { &printer.location };
                        draw_string(surface, content_x + 120, info_y + 65, location, text_color);

                        draw_string(surface, content_x, info_y + 85, "Driver:", dim_text);
                        let driver = if printer.driver_name.is_empty() { "-" } else { &printer.driver_name };
                        draw_string(surface, content_x + 120, info_y + 85, driver, text_color);

                        draw_string(surface, content_x, info_y + 105, "Pages printed:", dim_text);
                        let pages_str = printer.pages_printed_total.to_string();
                        draw_string(surface, content_x + 120, info_y + 105, &pages_str, text_color);

                        // Supplies section
                        if !printer.cartridges.is_empty() {
                            let supplies_y = info_y + 150;
                            draw_string(surface, content_x, supplies_y, "SUPPLIES", dim_text);

                            let mut cart_y = supplies_y + 25;
                            for cartridge in &printer.cartridges {
                                // Color bar
                                let bar_width = 100usize;
                                let filled_width = (cartridge.level as usize * bar_width) / 100;

                                // Background
                                for x in 0..bar_width {
                                    for y in 0..12 {
                                        surface.set_pixel(
                                            (content_x + 120) as usize + x,
                                            cart_y as usize + y,
                                            Color::new(60, 60, 65)
                                        );
                                    }
                                }

                                // Filled portion
                                for x in 0..filled_width {
                                    for y in 0..12 {
                                        surface.set_pixel(
                                            (content_x + 120) as usize + x,
                                            cart_y as usize + y,
                                            cartridge.display_color()
                                        );
                                    }
                                }

                                draw_string(surface, content_x, cart_y + 2, cartridge.color.name(), text_color);

                                let level_str = format!("{}%", cartridge.level);
                                draw_string(surface, content_x + 230, cart_y + 2, &level_str,
                                    if cartridge.is_low { Color::new(255, 150, 50) } else { dim_text });

                                cart_y += 22;
                            }
                        }

                        // Paper trays
                        if !printer.trays.is_empty() {
                            let trays_y = if printer.cartridges.is_empty() { info_y + 150 } else { info_y + 150 + (printer.cartridges.len() as isize * 22) + 40 };
                            draw_string(surface, content_x, trays_y, "PAPER TRAYS", dim_text);

                            let mut tray_y = trays_y + 25;
                            for tray in &printer.trays {
                                draw_string(surface, content_x, tray_y, &tray.name, text_color);
                                draw_string(surface, content_x + 120, tray_y, tray.paper_size.name(), dim_text);

                                let level_str = format!("{}/{}", tray.level, tray.capacity);
                                draw_string(surface, content_x + 280, tray_y, &level_str, dim_text);

                                tray_y += 20;
                            }
                        }
                    }
                } else {
                    draw_string(surface, content_x, content_y, "Select a printer", dim_text);
                }
            }

            ViewMode::PrintQueue => {
                draw_string(surface, content_x, content_y, "Print Queue", accent_color);

                let queue_y = content_y + 40;

                // Header
                draw_string(surface, content_x, queue_y, "Document", dim_text);
                draw_string(surface, content_x + 200, queue_y, "Status", dim_text);
                draw_string(surface, content_x + 300, queue_y, "Pages", dim_text);
                draw_string(surface, content_x + 380, queue_y, "Size", dim_text);

                let mut job_y = queue_y + 25;
                for job in &self.print_queue {
                    // Document name
                    let doc_name = if job.document_name.len() > 25 {
                        let mut n: String = job.document_name.chars().take(22).collect();
                        n.push_str("...");
                        n
                    } else {
                        job.document_name.clone()
                    };
                    draw_string(surface, content_x, job_y, &doc_name, text_color);

                    // Status
                    let status_color = match job.status {
                        JobStatus::Printing => accent_color,
                        JobStatus::Completed => ready_color,
                        JobStatus::Error | JobStatus::Cancelled => Color::new(255, 100, 100),
                        _ => dim_text,
                    };
                    draw_string(surface, content_x + 200, job_y, job.status.name(), status_color);

                    // Pages
                    let pages_str = format!("{}/{}", job.pages_printed, job.pages_total);
                    draw_string(surface, content_x + 300, job_y, &pages_str, dim_text);

                    // Size
                    draw_string(surface, content_x + 380, job_y, &job.format_size(), dim_text);

                    job_y += 25;
                }

                if self.print_queue.is_empty() {
                    draw_string(surface, content_x, queue_y + 30, "No print jobs", dim_text);
                }
            }

            ViewMode::Settings => {
                if let Some(printer_id) = self.selected_printer_id {
                    if let Some(printer) = self.get_printer(printer_id) {
                        draw_string(surface, content_x, content_y, &format!("Settings: {}", printer.name), accent_color);

                        let settings_y = content_y + 40;

                        draw_string(surface, content_x, settings_y, "Paper Size:", text_color);
                        draw_string(surface, content_x + 150, settings_y, printer.settings.paper_size.name(), dim_text);

                        draw_string(surface, content_x, settings_y + 25, "Quality:", text_color);
                        draw_string(surface, content_x + 150, settings_y + 25, printer.settings.quality.name(), dim_text);

                        draw_string(surface, content_x, settings_y + 50, "Color Mode:", text_color);
                        draw_string(surface, content_x + 150, settings_y + 50, printer.settings.color_mode.name(), dim_text);

                        draw_string(surface, content_x, settings_y + 75, "Duplex:", text_color);
                        draw_string(surface, content_x + 150, settings_y + 75, printer.settings.duplex.name(), dim_text);

                        draw_string(surface, content_x, settings_y + 100, "Copies:", text_color);
                        let copies_str = printer.settings.copies.to_string();
                        draw_string(surface, content_x + 150, settings_y + 100, &copies_str, dim_text);

                        draw_string(surface, content_x, settings_y + 150, "Press [ESC] to go back", dim_text);
                    }
                }
            }

            ViewMode::AddPrinter => {
                draw_string(surface, content_x, content_y, "Add Printer", accent_color);
                draw_string(surface, content_x, content_y + 40, "Searching for printers...", dim_text);
                draw_string(surface, content_x, content_y + 80, "Or enter printer address manually:", text_color);
                draw_string(surface, content_x, content_y + 120, "Press [ESC] to cancel", dim_text);
            }
        }

        // Status bar
        let status_y = self.bounds.y + self.bounds.height as isize - 25;
        for x in self.sidebar_width..(self.bounds.width) {
            surface.set_pixel(
                (self.bounds.x as usize) + x,
                status_y as usize,
                border_color
            );
        }

        let active_jobs = self.print_queue.iter().filter(|j| j.status.is_active()).count();
        let status_str = format!("{} printers, {} active jobs", self.printers.len(), active_jobs);
        draw_string(surface, content_x, status_y + 8, &status_str, dim_text);
    }
}

/// Initialize the printer settings module
pub fn init() {
    crate::kprintln!("[PrinterSettings] Printer settings application initialized");
}
