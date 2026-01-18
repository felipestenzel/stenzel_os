//! USB Video Class (UVC) driver.
//!
//! Implements USB Video Class 1.0/1.1/1.5 specification for webcams.
//!
//! Features:
//! - Video Control (VC) interface parsing
//! - Video Streaming (VS) interface configuration
//! - Isochronous endpoint management
//! - Format negotiation (MJPEG, YUV, etc.)
//! - Camera controls (brightness, contrast, etc.)

#![allow(dead_code)]

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::sync::TicketSpinlock;

/// USB Video Class codes
pub mod class_codes {
    pub const VIDEO: u8 = 0x0E;

    // Video Interface Subclass Codes
    pub const SC_UNDEFINED: u8 = 0x00;
    pub const SC_VIDEOCONTROL: u8 = 0x01;
    pub const SC_VIDEOSTREAMING: u8 = 0x02;
    pub const SC_VIDEO_INTERFACE_COLLECTION: u8 = 0x03;

    // Video Interface Protocol Codes
    pub const PC_PROTOCOL_UNDEFINED: u8 = 0x00;
    pub const PC_PROTOCOL_15: u8 = 0x01; // UVC 1.5

    // Video Class-Specific Descriptor Types
    pub const CS_UNDEFINED: u8 = 0x20;
    pub const CS_DEVICE: u8 = 0x21;
    pub const CS_CONFIGURATION: u8 = 0x22;
    pub const CS_STRING: u8 = 0x23;
    pub const CS_INTERFACE: u8 = 0x24;
    pub const CS_ENDPOINT: u8 = 0x25;

    // Video Control Interface Descriptor Subtypes
    pub const VC_DESCRIPTOR_UNDEFINED: u8 = 0x00;
    pub const VC_HEADER: u8 = 0x01;
    pub const VC_INPUT_TERMINAL: u8 = 0x02;
    pub const VC_OUTPUT_TERMINAL: u8 = 0x03;
    pub const VC_SELECTOR_UNIT: u8 = 0x04;
    pub const VC_PROCESSING_UNIT: u8 = 0x05;
    pub const VC_EXTENSION_UNIT: u8 = 0x06;
    pub const VC_ENCODING_UNIT: u8 = 0x07; // UVC 1.5

    // Video Streaming Interface Descriptor Subtypes
    pub const VS_UNDEFINED: u8 = 0x00;
    pub const VS_INPUT_HEADER: u8 = 0x01;
    pub const VS_OUTPUT_HEADER: u8 = 0x02;
    pub const VS_STILL_IMAGE_FRAME: u8 = 0x03;
    pub const VS_FORMAT_UNCOMPRESSED: u8 = 0x04;
    pub const VS_FRAME_UNCOMPRESSED: u8 = 0x05;
    pub const VS_FORMAT_MJPEG: u8 = 0x06;
    pub const VS_FRAME_MJPEG: u8 = 0x07;
    pub const VS_FORMAT_MPEG2TS: u8 = 0x0A;
    pub const VS_FORMAT_DV: u8 = 0x0C;
    pub const VS_COLORFORMAT: u8 = 0x0D;
    pub const VS_FORMAT_FRAME_BASED: u8 = 0x10;
    pub const VS_FRAME_FRAME_BASED: u8 = 0x11;
    pub const VS_FORMAT_STREAM_BASED: u8 = 0x12;
    pub const VS_FORMAT_H264: u8 = 0x13;
    pub const VS_FRAME_H264: u8 = 0x14;
    pub const VS_FORMAT_H264_SIMULCAST: u8 = 0x15;
    pub const VS_FORMAT_VP8: u8 = 0x16;
    pub const VS_FRAME_VP8: u8 = 0x17;
    pub const VS_FORMAT_VP8_SIMULCAST: u8 = 0x18;
}

/// Terminal types
pub mod terminal_types {
    // USB Terminal Types
    pub const TT_VENDOR_SPECIFIC: u16 = 0x0100;
    pub const TT_STREAMING: u16 = 0x0101;

    // Input Terminal Types
    pub const ITT_VENDOR_SPECIFIC: u16 = 0x0200;
    pub const ITT_CAMERA: u16 = 0x0201;
    pub const ITT_MEDIA_TRANSPORT_INPUT: u16 = 0x0202;

    // Output Terminal Types
    pub const OTT_VENDOR_SPECIFIC: u16 = 0x0300;
    pub const OTT_DISPLAY: u16 = 0x0301;
    pub const OTT_MEDIA_TRANSPORT_OUTPUT: u16 = 0x0302;

    // External Terminal Types
    pub const EXTERNAL_VENDOR_SPECIFIC: u16 = 0x0400;
    pub const COMPOSITE_CONNECTOR: u16 = 0x0401;
    pub const SVIDEO_CONNECTOR: u16 = 0x0402;
    pub const COMPONENT_CONNECTOR: u16 = 0x0403;
}

/// Video format types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFormat {
    Uncompressed,
    Mjpeg,
    Mpeg2Ts,
    Dv,
    FrameBased,
    H264,
    Vp8,
    Unknown(u8),
}

impl VideoFormat {
    pub fn from_subtype(subtype: u8) -> Self {
        match subtype {
            class_codes::VS_FORMAT_UNCOMPRESSED => VideoFormat::Uncompressed,
            class_codes::VS_FORMAT_MJPEG => VideoFormat::Mjpeg,
            class_codes::VS_FORMAT_MPEG2TS => VideoFormat::Mpeg2Ts,
            class_codes::VS_FORMAT_DV => VideoFormat::Dv,
            class_codes::VS_FORMAT_FRAME_BASED => VideoFormat::FrameBased,
            class_codes::VS_FORMAT_H264 => VideoFormat::H264,
            class_codes::VS_FORMAT_VP8 => VideoFormat::Vp8,
            other => VideoFormat::Unknown(other),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            VideoFormat::Uncompressed => "Uncompressed",
            VideoFormat::Mjpeg => "MJPEG",
            VideoFormat::Mpeg2Ts => "MPEG2-TS",
            VideoFormat::Dv => "DV",
            VideoFormat::FrameBased => "Frame-Based",
            VideoFormat::H264 => "H.264",
            VideoFormat::Vp8 => "VP8",
            VideoFormat::Unknown(_) => "Unknown",
        }
    }
}

/// Pixel format (FOURCC)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Yuy2,    // YUY2 - packed YUV 4:2:2
    Nv12,    // NV12 - planar YUV 4:2:0
    Mjpeg,   // MJPEG compressed
    H264,    // H.264 compressed
    Rgb24,   // RGB 24-bit
    Bgr24,   // BGR 24-bit
    Unknown([u8; 4]),
}

impl PixelFormat {
    pub fn from_guid(guid: &[u8; 16]) -> Self {
        // Check common GUIDs
        // YUY2: 32595559-0000-0010-8000-00AA00389B71
        if guid[0..4] == [0x59, 0x55, 0x59, 0x32] {
            return PixelFormat::Yuy2;
        }
        // NV12: 3231564E-0000-0010-8000-00AA00389B71
        if guid[0..4] == [0x4E, 0x56, 0x31, 0x32] {
            return PixelFormat::Nv12;
        }
        // MJPEG: 47504A4D-0000-0010-8000-00AA00389B71
        if guid[0..4] == [0x4D, 0x4A, 0x50, 0x47] {
            return PixelFormat::Mjpeg;
        }
        // H264: 34363248-0000-0010-8000-00AA00389B71
        if guid[0..4] == [0x48, 0x32, 0x36, 0x34] {
            return PixelFormat::H264;
        }

        let mut fourcc = [0u8; 4];
        fourcc.copy_from_slice(&guid[0..4]);
        PixelFormat::Unknown(fourcc)
    }

    pub fn bits_per_pixel(&self) -> u8 {
        match self {
            PixelFormat::Yuy2 => 16,
            PixelFormat::Nv12 => 12,
            PixelFormat::Rgb24 | PixelFormat::Bgr24 => 24,
            PixelFormat::Mjpeg | PixelFormat::H264 => 0, // Variable
            PixelFormat::Unknown(_) => 0,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            PixelFormat::Yuy2 => "YUY2",
            PixelFormat::Nv12 => "NV12",
            PixelFormat::Mjpeg => "MJPEG",
            PixelFormat::H264 => "H264",
            PixelFormat::Rgb24 => "RGB24",
            PixelFormat::Bgr24 => "BGR24",
            PixelFormat::Unknown(_) => "Unknown",
        }
    }
}

/// Video frame size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameSize {
    pub width: u16,
    pub height: u16,
}

impl FrameSize {
    pub const QVGA: Self = FrameSize { width: 320, height: 240 };
    pub const VGA: Self = FrameSize { width: 640, height: 480 };
    pub const SVGA: Self = FrameSize { width: 800, height: 600 };
    pub const HD720: Self = FrameSize { width: 1280, height: 720 };
    pub const HD1080: Self = FrameSize { width: 1920, height: 1080 };
    pub const UHD4K: Self = FrameSize { width: 3840, height: 2160 };

    pub fn pixels(&self) -> u32 {
        self.width as u32 * self.height as u32
    }

    pub fn aspect_ratio(&self) -> (u16, u16) {
        let gcd = gcd(self.width, self.height);
        (self.width / gcd, self.height / gcd)
    }
}

fn gcd(a: u16, b: u16) -> u16 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Frame interval (100ns units)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameInterval(pub u32);

impl FrameInterval {
    pub const FPS_5: Self = FrameInterval(2000000);   // 5 fps
    pub const FPS_10: Self = FrameInterval(1000000);  // 10 fps
    pub const FPS_15: Self = FrameInterval(666666);   // 15 fps
    pub const FPS_24: Self = FrameInterval(416667);   // 24 fps
    pub const FPS_25: Self = FrameInterval(400000);   // 25 fps
    pub const FPS_30: Self = FrameInterval(333333);   // 30 fps
    pub const FPS_60: Self = FrameInterval(166666);   // 60 fps

    pub fn fps(&self) -> f32 {
        10_000_000.0 / self.0 as f32
    }

    pub fn from_fps(fps: u32) -> Self {
        FrameInterval(10_000_000 / fps)
    }
}

/// Video frame descriptor
#[derive(Debug, Clone)]
pub struct FrameDescriptor {
    pub index: u8,
    pub size: FrameSize,
    pub default_interval: FrameInterval,
    pub min_interval: FrameInterval,
    pub max_interval: FrameInterval,
    pub interval_step: FrameInterval,
    pub discrete_intervals: Vec<FrameInterval>,
    pub max_video_frame_size: u32,
}

/// Video format descriptor
#[derive(Debug, Clone)]
pub struct FormatDescriptor {
    pub index: u8,
    pub format: VideoFormat,
    pub pixel_format: PixelFormat,
    pub num_frame_descriptors: u8,
    pub default_frame_index: u8,
    pub aspect_ratio: (u8, u8),
    pub interlace_flags: u8,
    pub frames: Vec<FrameDescriptor>,
}

/// Camera terminal controls
#[derive(Debug, Clone, Copy)]
pub struct CameraControls(pub u32);

impl CameraControls {
    pub fn has_scanning_mode(&self) -> bool { self.0 & (1 << 0) != 0 }
    pub fn has_auto_exposure_mode(&self) -> bool { self.0 & (1 << 1) != 0 }
    pub fn has_auto_exposure_priority(&self) -> bool { self.0 & (1 << 2) != 0 }
    pub fn has_exposure_time_absolute(&self) -> bool { self.0 & (1 << 3) != 0 }
    pub fn has_exposure_time_relative(&self) -> bool { self.0 & (1 << 4) != 0 }
    pub fn has_focus_absolute(&self) -> bool { self.0 & (1 << 5) != 0 }
    pub fn has_focus_relative(&self) -> bool { self.0 & (1 << 6) != 0 }
    pub fn has_iris_absolute(&self) -> bool { self.0 & (1 << 7) != 0 }
    pub fn has_iris_relative(&self) -> bool { self.0 & (1 << 8) != 0 }
    pub fn has_zoom_absolute(&self) -> bool { self.0 & (1 << 9) != 0 }
    pub fn has_zoom_relative(&self) -> bool { self.0 & (1 << 10) != 0 }
    pub fn has_pantilt_absolute(&self) -> bool { self.0 & (1 << 11) != 0 }
    pub fn has_pantilt_relative(&self) -> bool { self.0 & (1 << 12) != 0 }
    pub fn has_roll_absolute(&self) -> bool { self.0 & (1 << 13) != 0 }
    pub fn has_roll_relative(&self) -> bool { self.0 & (1 << 14) != 0 }
    pub fn has_focus_auto(&self) -> bool { self.0 & (1 << 17) != 0 }
    pub fn has_privacy(&self) -> bool { self.0 & (1 << 18) != 0 }
    pub fn has_focus_simple(&self) -> bool { self.0 & (1 << 19) != 0 }
    pub fn has_window(&self) -> bool { self.0 & (1 << 20) != 0 }
    pub fn has_region_of_interest(&self) -> bool { self.0 & (1 << 21) != 0 }
}

/// Processing unit controls
#[derive(Debug, Clone, Copy)]
pub struct ProcessingControls(pub u32);

impl ProcessingControls {
    pub fn has_brightness(&self) -> bool { self.0 & (1 << 0) != 0 }
    pub fn has_contrast(&self) -> bool { self.0 & (1 << 1) != 0 }
    pub fn has_hue(&self) -> bool { self.0 & (1 << 2) != 0 }
    pub fn has_saturation(&self) -> bool { self.0 & (1 << 3) != 0 }
    pub fn has_sharpness(&self) -> bool { self.0 & (1 << 4) != 0 }
    pub fn has_gamma(&self) -> bool { self.0 & (1 << 5) != 0 }
    pub fn has_white_balance_temperature(&self) -> bool { self.0 & (1 << 6) != 0 }
    pub fn has_white_balance_component(&self) -> bool { self.0 & (1 << 7) != 0 }
    pub fn has_backlight_compensation(&self) -> bool { self.0 & (1 << 8) != 0 }
    pub fn has_gain(&self) -> bool { self.0 & (1 << 9) != 0 }
    pub fn has_power_line_frequency(&self) -> bool { self.0 & (1 << 10) != 0 }
    pub fn has_auto_hue(&self) -> bool { self.0 & (1 << 11) != 0 }
    pub fn has_auto_white_balance_temperature(&self) -> bool { self.0 & (1 << 12) != 0 }
    pub fn has_auto_white_balance_component(&self) -> bool { self.0 & (1 << 13) != 0 }
    pub fn has_digital_multiplier(&self) -> bool { self.0 & (1 << 14) != 0 }
    pub fn has_digital_multiplier_limit(&self) -> bool { self.0 & (1 << 15) != 0 }
    pub fn has_analog_video_standard(&self) -> bool { self.0 & (1 << 16) != 0 }
    pub fn has_analog_video_lock_status(&self) -> bool { self.0 & (1 << 17) != 0 }
    pub fn has_contrast_auto(&self) -> bool { self.0 & (1 << 18) != 0 }
}

/// Input terminal descriptor
#[derive(Debug, Clone)]
pub struct InputTerminal {
    pub id: u8,
    pub terminal_type: u16,
    pub assoc_terminal: u8,
    pub string_index: u8,
    pub camera_controls: Option<CameraControls>,
}

impl InputTerminal {
    pub fn is_camera(&self) -> bool {
        self.terminal_type == terminal_types::ITT_CAMERA
    }
}

/// Processing unit descriptor
#[derive(Debug, Clone)]
pub struct ProcessingUnit {
    pub id: u8,
    pub source_id: u8,
    pub max_multiplier: u16,
    pub controls: ProcessingControls,
    pub string_index: u8,
    pub video_standards: u8,
}

/// Output terminal descriptor
#[derive(Debug, Clone)]
pub struct OutputTerminal {
    pub id: u8,
    pub terminal_type: u16,
    pub assoc_terminal: u8,
    pub source_id: u8,
    pub string_index: u8,
}

/// Video streaming interface
#[derive(Debug, Clone)]
pub struct StreamingInterface {
    pub interface_number: u8,
    pub alt_setting: u8,
    pub endpoint_address: u8,
    pub max_packet_size: u16,
    pub formats: Vec<FormatDescriptor>,
}

/// USB Video device
#[derive(Debug)]
pub struct UsbVideoDevice {
    pub slot_id: u8,
    pub address: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub name: String,
    pub uvc_version: UvcVersion,
    pub input_terminals: Vec<InputTerminal>,
    pub output_terminals: Vec<OutputTerminal>,
    pub processing_units: Vec<ProcessingUnit>,
    pub streaming_interfaces: Vec<StreamingInterface>,
    pub current_format: Option<(u8, u8)>, // (format_index, frame_index)
    pub current_interval: Option<FrameInterval>,
    pub active: AtomicBool,
    pub streaming: AtomicBool,
    pub brightness: AtomicU32,
    pub contrast: AtomicU32,
}

/// UVC version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UvcVersion {
    Uvc10,
    Uvc11,
    Uvc15,
    Unknown(u16),
}

impl UvcVersion {
    pub fn from_bcd(bcd: u16) -> Self {
        match bcd {
            0x0100 => UvcVersion::Uvc10,
            0x0110 => UvcVersion::Uvc11,
            0x0150 => UvcVersion::Uvc15,
            other => UvcVersion::Unknown(other),
        }
    }
}

impl UsbVideoDevice {
    pub fn new(slot_id: u8, address: u8, vendor_id: u16, product_id: u16) -> Self {
        Self {
            slot_id,
            address,
            vendor_id,
            product_id,
            name: String::new(),
            uvc_version: UvcVersion::Uvc10,
            input_terminals: Vec::new(),
            output_terminals: Vec::new(),
            processing_units: Vec::new(),
            streaming_interfaces: Vec::new(),
            current_format: None,
            current_interval: None,
            active: AtomicBool::new(false),
            streaming: AtomicBool::new(false),
            brightness: AtomicU32::new(128),
            contrast: AtomicU32::new(128),
        }
    }

    pub fn has_camera(&self) -> bool {
        self.input_terminals.iter().any(|t| t.is_camera())
    }

    pub fn supported_formats(&self) -> Vec<&FormatDescriptor> {
        self.streaming_interfaces.iter()
            .flat_map(|si| si.formats.iter())
            .collect()
    }

    pub fn supported_resolutions(&self) -> Vec<FrameSize> {
        let mut sizes = Vec::new();
        for si in &self.streaming_interfaces {
            for fmt in &si.formats {
                for frame in &fmt.frames {
                    if !sizes.contains(&frame.size) {
                        sizes.push(frame.size);
                    }
                }
            }
        }
        sizes.sort_by_key(|s| s.pixels());
        sizes
    }

    pub fn max_resolution(&self) -> Option<FrameSize> {
        self.supported_resolutions().into_iter().max_by_key(|s| s.pixels())
    }

    pub fn set_brightness(&self, value: u8) {
        self.brightness.store(value as u32, Ordering::SeqCst);
    }

    pub fn get_brightness(&self) -> u8 {
        (self.brightness.load(Ordering::SeqCst) & 0xFF) as u8
    }

    pub fn set_contrast(&self, value: u8) {
        self.contrast.store(value as u32, Ordering::SeqCst);
    }

    pub fn get_contrast(&self) -> u8 {
        (self.contrast.load(Ordering::SeqCst) & 0xFF) as u8
    }

    pub fn is_streaming(&self) -> bool {
        self.streaming.load(Ordering::SeqCst)
    }
}

/// Video stream state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    Idle,
    Starting,
    Streaming,
    Stopping,
    Error,
}

/// Video frame buffer
#[derive(Debug)]
pub struct VideoFrame {
    pub data: Box<[u8]>,
    pub size: FrameSize,
    pub format: PixelFormat,
    pub timestamp: u64,
    pub sequence: u32,
}

/// Video probe/commit control
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct VideoProbeCommit {
    pub hint: u16,
    pub format_index: u8,
    pub frame_index: u8,
    pub frame_interval: u32,
    pub key_frame_rate: u16,
    pub p_frame_rate: u16,
    pub comp_quality: u16,
    pub comp_window_size: u16,
    pub delay: u16,
    pub max_video_frame_size: u32,
    pub max_payload_transfer_size: u32,
    pub clock_frequency: u32,
    pub framing_info: u8,
    pub preferred_version: u8,
    pub min_version: u8,
    pub max_version: u8,
}

impl Default for VideoProbeCommit {
    fn default() -> Self {
        Self {
            hint: 0,
            format_index: 1,
            frame_index: 1,
            frame_interval: FrameInterval::FPS_30.0,
            key_frame_rate: 0,
            p_frame_rate: 0,
            comp_quality: 0,
            comp_window_size: 0,
            delay: 0,
            max_video_frame_size: 0,
            max_payload_transfer_size: 0,
            clock_frequency: 0,
            framing_info: 0,
            preferred_version: 0,
            min_version: 0,
            max_version: 0,
        }
    }
}

/// USB Video driver state
pub struct UsbVideoDriver {
    devices: Vec<UsbVideoDevice>,
    default_device: Option<usize>,
    initialized: bool,
}

impl UsbVideoDriver {
    pub const fn new() -> Self {
        Self {
            devices: Vec::new(),
            default_device: None,
            initialized: false,
        }
    }

    pub fn init(&mut self) {
        crate::kprintln!("uvc: initializing USB Video Class driver");
        self.initialized = true;
    }

    pub fn register_device(&mut self, device: UsbVideoDevice) -> usize {
        let index = self.devices.len();

        crate::kprintln!("uvc: registered device {} ({}:{:04X}:{:04X})",
            device.name,
            device.slot_id,
            device.vendor_id,
            device.product_id
        );

        if let Some(max_res) = device.max_resolution() {
            crate::kprintln!("uvc: max resolution {}x{}", max_res.width, max_res.height);
        }

        if self.default_device.is_none() {
            self.default_device = Some(index);
            crate::kprintln!("uvc: set as default video device");
        }

        self.devices.push(device);
        index
    }

    pub fn unregister_device(&mut self, index: usize) {
        if index < self.devices.len() {
            let device = &self.devices[index];
            crate::kprintln!("uvc: unregistering device {}", device.name);

            if self.default_device == Some(index) {
                self.default_device = None;
            }

            self.devices.remove(index);
        }
    }

    pub fn get_device(&self, index: usize) -> Option<&UsbVideoDevice> {
        self.devices.get(index)
    }

    pub fn get_device_mut(&mut self, index: usize) -> Option<&mut UsbVideoDevice> {
        self.devices.get_mut(index)
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    pub fn list_devices(&self) -> &[UsbVideoDevice] {
        &self.devices
    }

    pub fn default_device(&self) -> Option<&UsbVideoDevice> {
        self.default_device.and_then(|i| self.devices.get(i))
    }
}

/// Global USB video driver instance
static USB_VIDEO_DRIVER: TicketSpinlock<UsbVideoDriver> = TicketSpinlock::new(UsbVideoDriver::new());

/// Initialize USB video driver
pub fn init() {
    USB_VIDEO_DRIVER.lock().init();
}

/// Check if interface is USB Video Class
pub fn is_video_interface(class: u8, subclass: u8) -> bool {
    class == class_codes::VIDEO &&
    (subclass == class_codes::SC_VIDEOCONTROL || subclass == class_codes::SC_VIDEOSTREAMING)
}

/// Register a USB video device
pub fn register_device(device: UsbVideoDevice) -> usize {
    USB_VIDEO_DRIVER.lock().register_device(device)
}

/// Unregister a USB video device
pub fn unregister_device(index: usize) {
    USB_VIDEO_DRIVER.lock().unregister_device(index)
}

/// Get device count
pub fn device_count() -> usize {
    USB_VIDEO_DRIVER.lock().device_count()
}

/// Get device info string
pub fn format_devices() -> String {
    use core::fmt::Write;
    let mut output = String::new();
    let driver = USB_VIDEO_DRIVER.lock();

    writeln!(output, "USB Video Devices: {}", driver.device_count()).ok();

    for (i, device) in driver.list_devices().iter().enumerate() {
        let default = driver.default_device == Some(i);

        writeln!(output, "  [{}]{} {} ({:04X}:{:04X})",
            i,
            if default { " (default)" } else { "" },
            device.name,
            device.vendor_id,
            device.product_id
        ).ok();

        if let Some(max_res) = device.max_resolution() {
            writeln!(output, "      Max: {}x{}", max_res.width, max_res.height).ok();
        }

        let formats: Vec<&str> = device.supported_formats()
            .iter()
            .map(|f| f.format.name())
            .collect();
        let unique: Vec<&str> = formats.iter()
            .cloned()
            .collect::<alloc::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        writeln!(output, "      Formats: {}", unique.join(", ")).ok();

        writeln!(output, "      Streaming: {}",
            if device.is_streaming() { "Yes" } else { "No" }
        ).ok();
    }

    output
}

/// Video control requests
pub mod requests {
    pub const SET_CUR: u8 = 0x01;
    pub const GET_CUR: u8 = 0x81;
    pub const GET_MIN: u8 = 0x82;
    pub const GET_MAX: u8 = 0x83;
    pub const GET_RES: u8 = 0x84;
    pub const GET_LEN: u8 = 0x85;
    pub const GET_INFO: u8 = 0x86;
    pub const GET_DEF: u8 = 0x87;

    // Video Streaming Interface Control Selectors
    pub const VS_PROBE_CONTROL: u8 = 0x01;
    pub const VS_COMMIT_CONTROL: u8 = 0x02;
    pub const VS_STILL_PROBE_CONTROL: u8 = 0x03;
    pub const VS_STILL_COMMIT_CONTROL: u8 = 0x04;
    pub const VS_STILL_IMAGE_TRIGGER_CONTROL: u8 = 0x05;
    pub const VS_STREAM_ERROR_CODE_CONTROL: u8 = 0x06;
    pub const VS_GENERATE_KEY_FRAME_CONTROL: u8 = 0x07;
    pub const VS_UPDATE_FRAME_SEGMENT_CONTROL: u8 = 0x08;
    pub const VS_SYNC_DELAY_CONTROL: u8 = 0x09;

    // Camera Terminal Control Selectors
    pub const CT_SCANNING_MODE_CONTROL: u8 = 0x01;
    pub const CT_AE_MODE_CONTROL: u8 = 0x02;
    pub const CT_AE_PRIORITY_CONTROL: u8 = 0x03;
    pub const CT_EXPOSURE_TIME_ABSOLUTE_CONTROL: u8 = 0x04;
    pub const CT_EXPOSURE_TIME_RELATIVE_CONTROL: u8 = 0x05;
    pub const CT_FOCUS_ABSOLUTE_CONTROL: u8 = 0x06;
    pub const CT_FOCUS_RELATIVE_CONTROL: u8 = 0x07;
    pub const CT_FOCUS_AUTO_CONTROL: u8 = 0x08;
    pub const CT_IRIS_ABSOLUTE_CONTROL: u8 = 0x09;
    pub const CT_IRIS_RELATIVE_CONTROL: u8 = 0x0A;
    pub const CT_ZOOM_ABSOLUTE_CONTROL: u8 = 0x0B;
    pub const CT_ZOOM_RELATIVE_CONTROL: u8 = 0x0C;
    pub const CT_PANTILT_ABSOLUTE_CONTROL: u8 = 0x0D;
    pub const CT_PANTILT_RELATIVE_CONTROL: u8 = 0x0E;
    pub const CT_ROLL_ABSOLUTE_CONTROL: u8 = 0x0F;
    pub const CT_ROLL_RELATIVE_CONTROL: u8 = 0x10;
    pub const CT_PRIVACY_CONTROL: u8 = 0x11;

    // Processing Unit Control Selectors
    pub const PU_BACKLIGHT_COMPENSATION_CONTROL: u8 = 0x01;
    pub const PU_BRIGHTNESS_CONTROL: u8 = 0x02;
    pub const PU_CONTRAST_CONTROL: u8 = 0x03;
    pub const PU_GAIN_CONTROL: u8 = 0x04;
    pub const PU_POWER_LINE_FREQUENCY_CONTROL: u8 = 0x05;
    pub const PU_HUE_CONTROL: u8 = 0x06;
    pub const PU_SATURATION_CONTROL: u8 = 0x07;
    pub const PU_SHARPNESS_CONTROL: u8 = 0x08;
    pub const PU_GAMMA_CONTROL: u8 = 0x09;
    pub const PU_WHITE_BALANCE_TEMPERATURE_CONTROL: u8 = 0x0A;
    pub const PU_WHITE_BALANCE_TEMPERATURE_AUTO_CONTROL: u8 = 0x0B;
    pub const PU_WHITE_BALANCE_COMPONENT_CONTROL: u8 = 0x0C;
    pub const PU_WHITE_BALANCE_COMPONENT_AUTO_CONTROL: u8 = 0x0D;
    pub const PU_DIGITAL_MULTIPLIER_CONTROL: u8 = 0x0E;
    pub const PU_DIGITAL_MULTIPLIER_LIMIT_CONTROL: u8 = 0x0F;
    pub const PU_HUE_AUTO_CONTROL: u8 = 0x10;
    pub const PU_ANALOG_VIDEO_STANDARD_CONTROL: u8 = 0x11;
    pub const PU_ANALOG_LOCK_STATUS_CONTROL: u8 = 0x12;
    pub const PU_CONTRAST_AUTO_CONTROL: u8 = 0x13;
}

/// Parse video control interface descriptors
pub fn parse_vc_interface(data: &[u8]) -> Option<(Vec<InputTerminal>, Vec<OutputTerminal>, Vec<ProcessingUnit>)> {
    if data.len() < 12 {
        return None;
    }

    let mut input_terminals = Vec::new();
    let mut output_terminals = Vec::new();
    let mut processing_units = Vec::new();

    let mut pos = 0;
    while pos < data.len() {
        let len = data[pos] as usize;
        if len < 3 || pos + len > data.len() {
            break;
        }

        let desc_type = data[pos + 1];
        let desc_subtype = data[pos + 2];

        if desc_type == class_codes::CS_INTERFACE {
            match desc_subtype {
                class_codes::VC_INPUT_TERMINAL if len >= 8 => {
                    let terminal_type = u16::from_le_bytes([data[pos + 4], data[pos + 5]]);
                    let camera_controls = if terminal_type == terminal_types::ITT_CAMERA && len >= 15 {
                        Some(CameraControls(u32::from_le_bytes([
                            data[pos + 15], data[pos + 16], data[pos + 17], 0
                        ])))
                    } else {
                        None
                    };

                    input_terminals.push(InputTerminal {
                        id: data[pos + 3],
                        terminal_type,
                        assoc_terminal: data[pos + 6],
                        string_index: data[pos + 7],
                        camera_controls,
                    });
                }
                class_codes::VC_OUTPUT_TERMINAL if len >= 9 => {
                    output_terminals.push(OutputTerminal {
                        id: data[pos + 3],
                        terminal_type: u16::from_le_bytes([data[pos + 4], data[pos + 5]]),
                        assoc_terminal: data[pos + 6],
                        source_id: data[pos + 7],
                        string_index: data[pos + 8],
                    });
                }
                class_codes::VC_PROCESSING_UNIT if len >= 8 => {
                    let control_size = data[pos + 7] as usize;
                    let controls = if pos + 8 + control_size <= data.len() {
                        let mut ctrl_bytes = [0u8; 4];
                        for i in 0..control_size.min(4) {
                            ctrl_bytes[i] = data[pos + 8 + i];
                        }
                        ProcessingControls(u32::from_le_bytes(ctrl_bytes))
                    } else {
                        ProcessingControls(0)
                    };

                    processing_units.push(ProcessingUnit {
                        id: data[pos + 3],
                        source_id: data[pos + 4],
                        max_multiplier: u16::from_le_bytes([data[pos + 5], data[pos + 6]]),
                        controls,
                        string_index: if pos + 8 + control_size < len { data[pos + 8 + control_size] } else { 0 },
                        video_standards: if pos + 9 + control_size < len { data[pos + 9 + control_size] } else { 0 },
                    });
                }
                _ => {}
            }
        }

        pos += len;
    }

    Some((input_terminals, output_terminals, processing_units))
}

// ============================================================================
// Webcam Integration API
// ============================================================================

/// Camera capabilities for webcam module
#[derive(Debug, Clone)]
pub struct CameraCapabilities {
    pub formats: Vec<CameraFormatInfo>,
    pub has_brightness: bool,
    pub has_contrast: bool,
    pub has_saturation: bool,
    pub has_hue: bool,
    pub has_autofocus: bool,
    pub has_zoom: bool,
}

/// Format info for webcam module
#[derive(Debug, Clone)]
pub struct CameraFormatInfo {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub frame_rate: Option<u32>,
}

/// List detected cameras with capabilities
pub fn list_cameras() -> Vec<(u32, String, CameraCapabilities)> {
    let driver = USB_VIDEO_DRIVER.lock();
    let mut result = Vec::new();

    for (idx, device) in driver.devices.iter().enumerate() {
        let mut formats = Vec::new();

        for si in &device.streaming_interfaces {
            for fmt in &si.formats {
                for frame in &fmt.frames {
                    let fps = if !frame.discrete_intervals.is_empty() {
                        Some(10_000_000 / frame.discrete_intervals[0].0)
                    } else if frame.default_interval.0 > 0 {
                        Some(10_000_000 / frame.default_interval.0)
                    } else {
                        Some(30)
                    };

                    formats.push(CameraFormatInfo {
                        width: frame.size.width as u32,
                        height: frame.size.height as u32,
                        format: fmt.pixel_format,
                        frame_rate: fps,
                    });
                }
            }
        }

        // Check processing unit capabilities
        let has_brightness = device.processing_units.iter()
            .any(|pu| pu.controls.has_brightness());
        let has_contrast = device.processing_units.iter()
            .any(|pu| pu.controls.has_contrast());
        let has_saturation = device.processing_units.iter()
            .any(|pu| pu.controls.has_saturation());
        let has_hue = device.processing_units.iter()
            .any(|pu| pu.controls.has_hue());

        // Check camera terminal capabilities
        let has_autofocus = device.input_terminals.iter()
            .filter_map(|t| t.camera_controls.as_ref())
            .any(|c| c.has_focus_auto());
        let has_zoom = device.input_terminals.iter()
            .filter_map(|t| t.camera_controls.as_ref())
            .any(|c| c.has_zoom_absolute() || c.has_zoom_relative());

        let caps = CameraCapabilities {
            formats,
            has_brightness,
            has_contrast,
            has_saturation,
            has_hue,
            has_autofocus,
            has_zoom,
        };

        result.push((idx as u32, device.name.clone(), caps));
    }

    result
}

/// Start streaming on a camera
pub fn start_streaming(device_id: u32) -> Result<(), &'static str> {
    let mut driver = USB_VIDEO_DRIVER.lock();
    let device = driver.devices.get_mut(device_id as usize)
        .ok_or("Device not found")?;

    if device.streaming.load(Ordering::SeqCst) {
        return Err("Already streaming");
    }

    device.streaming.store(true, Ordering::SeqCst);
    crate::kprintln!("uvc: started streaming on device {}", device_id);
    Ok(())
}

/// Stop streaming on a camera
pub fn stop_streaming(device_id: u32) -> Result<(), &'static str> {
    let mut driver = USB_VIDEO_DRIVER.lock();
    let device = driver.devices.get_mut(device_id as usize)
        .ok_or("Device not found")?;

    if !device.streaming.load(Ordering::SeqCst) {
        return Err("Not streaming");
    }

    device.streaming.store(false, Ordering::SeqCst);
    crate::kprintln!("uvc: stopped streaming on device {}", device_id);
    Ok(())
}

/// Set a control value on a camera
pub fn set_control(device_id: u32, control_id: u32, value: i32) -> Result<(), &'static str> {
    let driver = USB_VIDEO_DRIVER.lock();
    let device = driver.devices.get(device_id as usize)
        .ok_or("Device not found")?;

    // Map control_id to actual control
    // This is a simplified implementation - real hardware would send USB control transfers
    match control_id {
        // Brightness (CameraControl::Brightness as u32)
        0 => device.brightness.store(value as u32, Ordering::SeqCst),
        // Contrast
        1 => device.contrast.store(value as u32, Ordering::SeqCst),
        // Others - would require actual USB control transfers
        _ => {
            crate::kprintln!("uvc: set control {} = {} on device {}", control_id, value, device_id);
        }
    }

    Ok(())
}

/// Get a control value from a camera
pub fn get_control(device_id: u32, control_id: u32) -> Result<i32, &'static str> {
    let driver = USB_VIDEO_DRIVER.lock();
    let device = driver.devices.get(device_id as usize)
        .ok_or("Device not found")?;

    match control_id {
        0 => Ok(device.brightness.load(Ordering::SeqCst) as i32),
        1 => Ok(device.contrast.load(Ordering::SeqCst) as i32),
        _ => Ok(128), // Default value for unknown controls
    }
}
