//! Webcam Support Module
//!
//! Provides high-level camera API with:
//! - Unified camera device abstraction
//! - Frame capture and streaming
//! - V4L2-compatible interface for Linux applications
//! - Multiple camera support
//! - Camera controls (brightness, contrast, exposure, etc.)

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use core::sync::atomic::{AtomicU32, Ordering};
use crate::sync::TicketSpinlock;

/// Camera pixel format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CameraPixelFormat {
    /// YUYV 4:2:2 packed
    Yuyv = 0x56595559,
    /// UYVY 4:2:2 packed
    Uyvy = 0x59565955,
    /// NV12 (Y + interleaved UV)
    Nv12 = 0x3231564E,
    /// NV21 (Y + interleaved VU)
    Nv21 = 0x3132564E,
    /// RGB24 packed
    Rgb24 = 0x33424752,
    /// BGR24 packed
    Bgr24 = 0x33524742,
    /// RGBA32 packed
    Rgba32 = 0x41424752,
    /// BGRA32 packed
    Bgra32 = 0x41524742,
    /// MJPEG compressed
    Mjpeg = 0x47504A4D,
    /// H.264 compressed
    H264 = 0x34363248,
    /// Grayscale 8-bit
    Grey = 0x59455247,
}

impl CameraPixelFormat {
    /// Get FourCC code
    pub fn fourcc(&self) -> u32 {
        *self as u32
    }

    /// Get bytes per pixel (for uncompressed formats)
    pub fn bytes_per_pixel(&self) -> Option<usize> {
        match self {
            CameraPixelFormat::Yuyv | CameraPixelFormat::Uyvy => Some(2),
            CameraPixelFormat::Nv12 | CameraPixelFormat::Nv21 => None, // Planar
            CameraPixelFormat::Rgb24 | CameraPixelFormat::Bgr24 => Some(3),
            CameraPixelFormat::Rgba32 | CameraPixelFormat::Bgra32 => Some(4),
            CameraPixelFormat::Grey => Some(1),
            CameraPixelFormat::Mjpeg | CameraPixelFormat::H264 => None, // Compressed
        }
    }

    /// Check if format is compressed
    pub fn is_compressed(&self) -> bool {
        matches!(self, CameraPixelFormat::Mjpeg | CameraPixelFormat::H264)
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            CameraPixelFormat::Yuyv => "YUYV",
            CameraPixelFormat::Uyvy => "UYVY",
            CameraPixelFormat::Nv12 => "NV12",
            CameraPixelFormat::Nv21 => "NV21",
            CameraPixelFormat::Rgb24 => "RGB24",
            CameraPixelFormat::Bgr24 => "BGR24",
            CameraPixelFormat::Rgba32 => "RGBA32",
            CameraPixelFormat::Bgra32 => "BGRA32",
            CameraPixelFormat::Mjpeg => "MJPEG",
            CameraPixelFormat::H264 => "H.264",
            CameraPixelFormat::Grey => "GREY",
        }
    }
}

/// Camera resolution preset
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraResolution {
    pub width: u32,
    pub height: u32,
}

impl CameraResolution {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Common resolution presets
    pub const QVGA: Self = Self::new(320, 240);
    pub const VGA: Self = Self::new(640, 480);
    pub const SVGA: Self = Self::new(800, 600);
    pub const XGA: Self = Self::new(1024, 768);
    pub const HD720: Self = Self::new(1280, 720);
    pub const HD1080: Self = Self::new(1920, 1080);
    pub const UHD4K: Self = Self::new(3840, 2160);

    /// Calculate frame size in bytes for given pixel format
    pub fn frame_size(&self, format: CameraPixelFormat) -> usize {
        let pixels = (self.width * self.height) as usize;
        match format {
            CameraPixelFormat::Yuyv | CameraPixelFormat::Uyvy => pixels * 2,
            CameraPixelFormat::Nv12 | CameraPixelFormat::Nv21 => pixels + pixels / 2,
            CameraPixelFormat::Rgb24 | CameraPixelFormat::Bgr24 => pixels * 3,
            CameraPixelFormat::Rgba32 | CameraPixelFormat::Bgra32 => pixels * 4,
            CameraPixelFormat::Grey => pixels,
            CameraPixelFormat::Mjpeg | CameraPixelFormat::H264 => pixels, // Max estimate
        }
    }
}

/// Camera format configuration
#[derive(Debug, Clone)]
pub struct CameraFormat {
    pub resolution: CameraResolution,
    pub pixel_format: CameraPixelFormat,
    pub frame_rate: u32,
}

impl Default for CameraFormat {
    fn default() -> Self {
        Self {
            resolution: CameraResolution::VGA,
            pixel_format: CameraPixelFormat::Yuyv,
            frame_rate: 30,
        }
    }
}

/// Camera control type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraControl {
    Brightness,
    Contrast,
    Saturation,
    Hue,
    Gamma,
    Gain,
    Sharpness,
    BacklightCompensation,
    WhiteBalanceTemperature,
    AutoWhiteBalance,
    Exposure,
    AutoExposure,
    ExposurePriority,
    Focus,
    AutoFocus,
    Zoom,
    Pan,
    Tilt,
    PowerLineFrequency,
}

impl CameraControl {
    /// V4L2 control ID
    pub fn v4l2_id(&self) -> u32 {
        match self {
            CameraControl::Brightness => 0x00980900,
            CameraControl::Contrast => 0x00980901,
            CameraControl::Saturation => 0x00980902,
            CameraControl::Hue => 0x00980903,
            CameraControl::Gamma => 0x00980910,
            CameraControl::Gain => 0x00980913,
            CameraControl::Sharpness => 0x0098091B,
            CameraControl::BacklightCompensation => 0x0098091C,
            CameraControl::WhiteBalanceTemperature => 0x0098091A,
            CameraControl::AutoWhiteBalance => 0x0098090C,
            CameraControl::Exposure => 0x009A0902,
            CameraControl::AutoExposure => 0x009A0901,
            CameraControl::ExposurePriority => 0x009A0903,
            CameraControl::Focus => 0x009A090A,
            CameraControl::AutoFocus => 0x009A090C,
            CameraControl::Zoom => 0x009A090D,
            CameraControl::Pan => 0x009A0908,
            CameraControl::Tilt => 0x009A0909,
            CameraControl::PowerLineFrequency => 0x00980918,
        }
    }
}

/// Camera control value with range
#[derive(Debug, Clone)]
pub struct CameraControlInfo {
    pub control: CameraControl,
    pub name: &'static str,
    pub min: i32,
    pub max: i32,
    pub default: i32,
    pub step: i32,
    pub current: i32,
    pub supported: bool,
}

/// Camera capabilities
#[derive(Debug, Clone)]
pub struct CameraCapabilities {
    pub name: String,
    pub driver: String,
    pub bus_info: String,
    pub supported_formats: Vec<CameraFormat>,
    pub controls: Vec<CameraControlInfo>,
    pub can_capture: bool,
    pub can_stream: bool,
    pub has_autofocus: bool,
    pub has_zoom: bool,
}

/// Camera state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraState {
    Closed,
    Open,
    Streaming,
    Error,
}

/// Frame buffer for captured frames
#[derive(Debug)]
pub struct CameraFrame {
    pub data: Vec<u8>,
    pub format: CameraFormat,
    pub sequence: u32,
    pub timestamp: u64,
    pub bytes_used: usize,
}

impl CameraFrame {
    pub fn new(format: CameraFormat, max_size: usize) -> Self {
        Self {
            data: vec![0u8; max_size],
            format,
            sequence: 0,
            timestamp: 0,
            bytes_used: 0,
        }
    }
}

/// Buffer queue for streaming
pub struct BufferQueue {
    buffers: Vec<CameraFrame>,
    queued: Vec<usize>,
    ready: Vec<usize>,
    dequeued: Vec<usize>,
}

impl BufferQueue {
    pub fn new(count: usize, format: CameraFormat) -> Self {
        let max_size = format.resolution.frame_size(format.pixel_format);
        let buffers = (0..count)
            .map(|_| CameraFrame::new(format.clone(), max_size))
            .collect();

        Self {
            buffers,
            queued: Vec::new(),
            ready: Vec::new(),
            dequeued: (0..count).collect(),
        }
    }

    /// Queue a buffer for capture
    pub fn queue(&mut self, index: usize) -> Result<(), &'static str> {
        if index >= self.buffers.len() {
            return Err("Invalid buffer index");
        }

        if let Some(pos) = self.dequeued.iter().position(|&i| i == index) {
            self.dequeued.remove(pos);
            self.queued.push(index);
            Ok(())
        } else {
            Err("Buffer not dequeued")
        }
    }

    /// Get next queued buffer for filling
    pub fn get_next_queued(&mut self) -> Option<&mut CameraFrame> {
        if let Some(&index) = self.queued.first() {
            Some(&mut self.buffers[index])
        } else {
            None
        }
    }

    /// Mark queued buffer as ready
    pub fn mark_ready(&mut self, sequence: u32, timestamp: u64, bytes_used: usize) {
        if let Some(index) = self.queued.first().copied() {
            self.queued.remove(0);
            self.buffers[index].sequence = sequence;
            self.buffers[index].timestamp = timestamp;
            self.buffers[index].bytes_used = bytes_used;
            self.ready.push(index);
        }
    }

    /// Dequeue a ready buffer
    pub fn dequeue(&mut self) -> Option<&CameraFrame> {
        if let Some(index) = self.ready.first().copied() {
            self.ready.remove(0);
            self.dequeued.push(index);
            Some(&self.buffers[index])
        } else {
            None
        }
    }

    /// Get number of ready buffers
    pub fn ready_count(&self) -> usize {
        self.ready.len()
    }

    /// Get number of queued buffers
    pub fn queued_count(&self) -> usize {
        self.queued.len()
    }
}

/// Camera device handle
pub type CameraHandle = u32;

/// Camera device abstraction
pub struct Camera {
    pub handle: CameraHandle,
    pub name: String,
    pub state: CameraState,
    pub format: CameraFormat,
    pub capabilities: CameraCapabilities,
    pub buffer_queue: Option<BufferQueue>,
    usb_device_id: Option<u32>,
    frame_sequence: AtomicU32,
}

impl Camera {
    pub fn new(handle: CameraHandle, name: String, caps: CameraCapabilities) -> Self {
        Self {
            handle,
            name,
            state: CameraState::Closed,
            format: CameraFormat::default(),
            capabilities: caps,
            buffer_queue: None,
            usb_device_id: None,
            frame_sequence: AtomicU32::new(0),
        }
    }

    /// Open the camera
    pub fn open(&mut self) -> Result<(), &'static str> {
        if self.state != CameraState::Closed {
            return Err("Camera already open");
        }
        self.state = CameraState::Open;
        Ok(())
    }

    /// Close the camera
    pub fn close(&mut self) -> Result<(), &'static str> {
        if self.state == CameraState::Streaming {
            self.stop_streaming()?;
        }
        self.state = CameraState::Closed;
        self.buffer_queue = None;
        Ok(())
    }

    /// Set capture format
    pub fn set_format(&mut self, format: CameraFormat) -> Result<(), &'static str> {
        if self.state == CameraState::Streaming {
            return Err("Cannot change format while streaming");
        }

        // Validate format is supported
        let supported = self.capabilities.supported_formats.iter()
            .any(|f| f.resolution.width == format.resolution.width &&
                     f.resolution.height == format.resolution.height &&
                     f.pixel_format == format.pixel_format);

        if !supported {
            return Err("Format not supported");
        }

        self.format = format;
        Ok(())
    }

    /// Request buffers for streaming
    pub fn request_buffers(&mut self, count: usize) -> Result<usize, &'static str> {
        if self.state != CameraState::Open {
            return Err("Camera not open");
        }

        let count = count.min(32).max(2);
        self.buffer_queue = Some(BufferQueue::new(count, self.format.clone()));
        Ok(count)
    }

    /// Queue a buffer for capture
    pub fn queue_buffer(&mut self, index: usize) -> Result<(), &'static str> {
        if let Some(ref mut queue) = self.buffer_queue {
            queue.queue(index)
        } else {
            Err("No buffers allocated")
        }
    }

    /// Start streaming
    pub fn start_streaming(&mut self) -> Result<(), &'static str> {
        if self.state != CameraState::Open {
            return Err("Camera not open");
        }

        if self.buffer_queue.is_none() {
            return Err("No buffers allocated");
        }

        if let Some(ref queue) = self.buffer_queue {
            if queue.queued_count() == 0 {
                return Err("No buffers queued");
            }
        }

        self.state = CameraState::Streaming;
        self.frame_sequence.store(0, Ordering::SeqCst);

        // Start USB streaming if USB camera
        if let Some(usb_id) = self.usb_device_id {
            crate::drivers::usb::video::start_streaming(usb_id)?;
        }

        Ok(())
    }

    /// Stop streaming
    pub fn stop_streaming(&mut self) -> Result<(), &'static str> {
        if self.state != CameraState::Streaming {
            return Err("Camera not streaming");
        }

        // Stop USB streaming if USB camera
        if let Some(usb_id) = self.usb_device_id {
            crate::drivers::usb::video::stop_streaming(usb_id)?;
        }

        self.state = CameraState::Open;
        Ok(())
    }

    /// Dequeue a ready frame
    pub fn dequeue_frame(&mut self) -> Option<&CameraFrame> {
        if self.state != CameraState::Streaming {
            return None;
        }

        if let Some(ref mut queue) = self.buffer_queue {
            queue.dequeue()
        } else {
            None
        }
    }

    /// Get a control value
    pub fn get_control(&self, control: CameraControl) -> Result<i32, &'static str> {
        for ctrl in &self.capabilities.controls {
            if ctrl.control == control && ctrl.supported {
                return Ok(ctrl.current);
            }
        }
        Err("Control not supported")
    }

    /// Set a control value
    pub fn set_control(&mut self, control: CameraControl, value: i32) -> Result<(), &'static str> {
        for ctrl in &mut self.capabilities.controls {
            if ctrl.control == control && ctrl.supported {
                if value < ctrl.min || value > ctrl.max {
                    return Err("Value out of range");
                }
                ctrl.current = value;

                // Apply to hardware if USB camera
                if let Some(usb_id) = self.usb_device_id {
                    crate::drivers::usb::video::set_control(usb_id, control as u32, value)?;
                }

                return Ok(());
            }
        }
        Err("Control not supported")
    }

    /// Check if frame is available
    pub fn poll(&self) -> bool {
        if self.state != CameraState::Streaming {
            return false;
        }

        if let Some(ref queue) = self.buffer_queue {
            queue.ready_count() > 0
        } else {
            false
        }
    }

    /// Internal: receive frame from driver
    pub fn receive_frame(&mut self, data: &[u8], timestamp: u64) {
        if let Some(ref mut queue) = self.buffer_queue {
            if let Some(frame) = queue.get_next_queued() {
                let len = data.len().min(frame.data.len());
                frame.data[..len].copy_from_slice(&data[..len]);
                let seq = self.frame_sequence.fetch_add(1, Ordering::SeqCst);
                queue.mark_ready(seq, timestamp, len);
            }
        }
    }
}

/// Camera manager
pub struct CameraManager {
    cameras: Vec<Camera>,
    next_handle: CameraHandle,
}

impl CameraManager {
    pub const fn new() -> Self {
        Self {
            cameras: Vec::new(),
            next_handle: 1,
        }
    }

    /// Register a new camera
    pub fn register(&mut self, name: String, caps: CameraCapabilities) -> CameraHandle {
        let handle = self.next_handle;
        self.next_handle += 1;

        let camera = Camera::new(handle, name, caps);
        self.cameras.push(camera);

        handle
    }

    /// Register a USB camera
    pub fn register_usb_camera(&mut self, usb_device_id: u32, name: String, caps: CameraCapabilities) -> CameraHandle {
        let handle = self.register(name, caps);

        if let Some(camera) = self.cameras.iter_mut().find(|c| c.handle == handle) {
            camera.usb_device_id = Some(usb_device_id);
        }

        handle
    }

    /// Unregister a camera
    pub fn unregister(&mut self, handle: CameraHandle) {
        if let Some(pos) = self.cameras.iter().position(|c| c.handle == handle) {
            let mut camera = self.cameras.remove(pos);
            let _ = camera.close();
        }
    }

    /// Get camera by handle
    pub fn get(&mut self, handle: CameraHandle) -> Option<&mut Camera> {
        self.cameras.iter_mut().find(|c| c.handle == handle)
    }

    /// List all cameras
    pub fn list(&self) -> Vec<(CameraHandle, &str)> {
        self.cameras.iter()
            .map(|c| (c.handle, c.name.as_str()))
            .collect()
    }

    /// Get camera count
    pub fn count(&self) -> usize {
        self.cameras.len()
    }

    /// Get camera by index
    pub fn get_by_index(&mut self, index: usize) -> Option<&mut Camera> {
        self.cameras.get_mut(index)
    }
}

/// Global camera manager
pub static CAMERA_MANAGER: TicketSpinlock<CameraManager> = TicketSpinlock::new(CameraManager::new());

// ============================================================================
// V4L2 Compatibility Layer
// ============================================================================

/// V4L2 capability flags
pub mod v4l2_cap {
    pub const VIDEO_CAPTURE: u32 = 0x00000001;
    pub const VIDEO_OUTPUT: u32 = 0x00000002;
    pub const STREAMING: u32 = 0x04000000;
    pub const READWRITE: u32 = 0x01000000;
}

/// V4L2 buffer type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum V4l2BufType {
    VideoCapture = 1,
    VideoOutput = 2,
    VideoOverlay = 3,
}

/// V4L2 memory type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum V4l2Memory {
    Mmap = 1,
    UserPtr = 2,
    Overlay = 3,
    DmaBuf = 4,
}

/// V4L2 format description
#[repr(C)]
#[derive(Debug, Clone)]
pub struct V4l2Format {
    pub buf_type: V4l2BufType,
    pub width: u32,
    pub height: u32,
    pub pixel_format: u32,
    pub field: u32,
    pub bytes_per_line: u32,
    pub size_image: u32,
    pub colorspace: u32,
}

/// V4L2 request buffers
#[repr(C)]
#[derive(Debug, Clone)]
pub struct V4l2RequestBuffers {
    pub count: u32,
    pub buf_type: V4l2BufType,
    pub memory: V4l2Memory,
}

/// V4L2 buffer
#[repr(C)]
#[derive(Debug, Clone)]
pub struct V4l2Buffer {
    pub index: u32,
    pub buf_type: V4l2BufType,
    pub bytes_used: u32,
    pub flags: u32,
    pub field: u32,
    pub timestamp_sec: u64,
    pub timestamp_usec: u64,
    pub sequence: u32,
    pub memory: V4l2Memory,
    pub offset: u32,
    pub length: u32,
}

/// V4L2 control
#[repr(C)]
#[derive(Debug, Clone)]
pub struct V4l2Control {
    pub id: u32,
    pub value: i32,
}

/// V4L2 ioctl numbers
pub mod v4l2_ioctl {
    pub const QUERYCAP: u32 = 0x80685600;
    pub const ENUM_FMT: u32 = 0xC0405602;
    pub const G_FMT: u32 = 0xC0CC5604;
    pub const S_FMT: u32 = 0xC0CC5605;
    pub const REQBUFS: u32 = 0xC0145608;
    pub const QUERYBUF: u32 = 0xC0445609;
    pub const QBUF: u32 = 0xC044560F;
    pub const DQBUF: u32 = 0xC0445611;
    pub const STREAMON: u32 = 0x40045612;
    pub const STREAMOFF: u32 = 0x40045613;
    pub const G_CTRL: u32 = 0xC008561B;
    pub const S_CTRL: u32 = 0xC008561C;
}

/// V4L2 device handle (for compatibility)
pub struct V4l2Device {
    camera_handle: CameraHandle,
    memory_type: V4l2Memory,
}

impl V4l2Device {
    pub fn new(camera_handle: CameraHandle) -> Self {
        Self {
            camera_handle,
            memory_type: V4l2Memory::Mmap,
        }
    }

    /// Handle V4L2 ioctl
    pub fn ioctl(&mut self, cmd: u32, arg: *mut u8) -> Result<(), i32> {
        let mut manager = CAMERA_MANAGER.lock();
        let camera = manager.get(self.camera_handle).ok_or(-1)?;

        match cmd {
            v4l2_ioctl::QUERYCAP => {
                // Query capabilities
                Ok(())
            }
            v4l2_ioctl::S_FMT => {
                // Set format
                let fmt = unsafe { &*(arg as *const V4l2Format) };
                let format = CameraFormat {
                    resolution: CameraResolution::new(fmt.width, fmt.height),
                    pixel_format: fourcc_to_format(fmt.pixel_format),
                    frame_rate: 30,
                };
                camera.set_format(format).map_err(|_| -1)?;
                Ok(())
            }
            v4l2_ioctl::G_FMT => {
                // Get format
                let fmt = unsafe { &mut *(arg as *mut V4l2Format) };
                fmt.width = camera.format.resolution.width;
                fmt.height = camera.format.resolution.height;
                fmt.pixel_format = camera.format.pixel_format.fourcc();
                Ok(())
            }
            v4l2_ioctl::REQBUFS => {
                // Request buffers
                let req = unsafe { &mut *(arg as *mut V4l2RequestBuffers) };
                self.memory_type = req.memory;
                let count = camera.request_buffers(req.count as usize).map_err(|_| -1)?;
                req.count = count as u32;
                Ok(())
            }
            v4l2_ioctl::QBUF => {
                // Queue buffer
                let buf = unsafe { &*(arg as *const V4l2Buffer) };
                camera.queue_buffer(buf.index as usize).map_err(|_| -1)?;
                Ok(())
            }
            v4l2_ioctl::DQBUF => {
                // Dequeue buffer
                let buf = unsafe { &mut *(arg as *mut V4l2Buffer) };
                let frame = camera.dequeue_frame().ok_or(-11)?; // EAGAIN
                buf.bytes_used = frame.bytes_used as u32;
                buf.sequence = frame.sequence;
                buf.timestamp_sec = frame.timestamp / 1_000_000;
                buf.timestamp_usec = frame.timestamp % 1_000_000;
                Ok(())
            }
            v4l2_ioctl::STREAMON => {
                camera.start_streaming().map_err(|_| -1)?;
                Ok(())
            }
            v4l2_ioctl::STREAMOFF => {
                camera.stop_streaming().map_err(|_| -1)?;
                Ok(())
            }
            v4l2_ioctl::G_CTRL => {
                let ctrl = unsafe { &mut *(arg as *mut V4l2Control) };
                let control = id_to_control(ctrl.id).ok_or(-1)?;
                ctrl.value = camera.get_control(control).map_err(|_| -1)?;
                Ok(())
            }
            v4l2_ioctl::S_CTRL => {
                let ctrl = unsafe { &*(arg as *const V4l2Control) };
                let control = id_to_control(ctrl.id).ok_or(-1)?;
                camera.set_control(control, ctrl.value).map_err(|_| -1)?;
                Ok(())
            }
            _ => Err(-1), // EINVAL
        }
    }
}

fn fourcc_to_format(fourcc: u32) -> CameraPixelFormat {
    match fourcc {
        0x56595559 => CameraPixelFormat::Yuyv,
        0x59565955 => CameraPixelFormat::Uyvy,
        0x3231564E => CameraPixelFormat::Nv12,
        0x3132564E => CameraPixelFormat::Nv21,
        0x33424752 => CameraPixelFormat::Rgb24,
        0x33524742 => CameraPixelFormat::Bgr24,
        0x41424752 => CameraPixelFormat::Rgba32,
        0x41524742 => CameraPixelFormat::Bgra32,
        0x47504A4D => CameraPixelFormat::Mjpeg,
        0x34363248 => CameraPixelFormat::H264,
        0x59455247 => CameraPixelFormat::Grey,
        _ => CameraPixelFormat::Yuyv,
    }
}

fn id_to_control(id: u32) -> Option<CameraControl> {
    match id {
        0x00980900 => Some(CameraControl::Brightness),
        0x00980901 => Some(CameraControl::Contrast),
        0x00980902 => Some(CameraControl::Saturation),
        0x00980903 => Some(CameraControl::Hue),
        0x00980910 => Some(CameraControl::Gamma),
        0x00980913 => Some(CameraControl::Gain),
        0x0098091B => Some(CameraControl::Sharpness),
        0x0098091C => Some(CameraControl::BacklightCompensation),
        0x0098091A => Some(CameraControl::WhiteBalanceTemperature),
        0x0098090C => Some(CameraControl::AutoWhiteBalance),
        0x009A0902 => Some(CameraControl::Exposure),
        0x009A0901 => Some(CameraControl::AutoExposure),
        0x009A0903 => Some(CameraControl::ExposurePriority),
        0x009A090A => Some(CameraControl::Focus),
        0x009A090C => Some(CameraControl::AutoFocus),
        0x009A090D => Some(CameraControl::Zoom),
        0x009A0908 => Some(CameraControl::Pan),
        0x009A0909 => Some(CameraControl::Tilt),
        0x00980918 => Some(CameraControl::PowerLineFrequency),
        _ => None,
    }
}

// ============================================================================
// Image Processing Utilities
// ============================================================================

/// Convert YUYV to RGB24
pub fn yuyv_to_rgb24(yuyv: &[u8], width: u32, height: u32) -> Vec<u8> {
    let pixels = (width * height) as usize;
    let mut rgb = vec![0u8; pixels * 3];

    for i in 0..pixels / 2 {
        let y0 = yuyv[i * 4] as i32;
        let u = yuyv[i * 4 + 1] as i32 - 128;
        let y1 = yuyv[i * 4 + 2] as i32;
        let v = yuyv[i * 4 + 3] as i32 - 128;

        // First pixel
        let r0 = (y0 + ((v * 359) >> 8)).clamp(0, 255) as u8;
        let g0 = (y0 - ((u * 88 + v * 183) >> 8)).clamp(0, 255) as u8;
        let b0 = (y0 + ((u * 454) >> 8)).clamp(0, 255) as u8;

        // Second pixel
        let r1 = (y1 + ((v * 359) >> 8)).clamp(0, 255) as u8;
        let g1 = (y1 - ((u * 88 + v * 183) >> 8)).clamp(0, 255) as u8;
        let b1 = (y1 + ((u * 454) >> 8)).clamp(0, 255) as u8;

        rgb[i * 6] = r0;
        rgb[i * 6 + 1] = g0;
        rgb[i * 6 + 2] = b0;
        rgb[i * 6 + 3] = r1;
        rgb[i * 6 + 4] = g1;
        rgb[i * 6 + 5] = b1;
    }

    rgb
}

/// Convert NV12 to RGB24
pub fn nv12_to_rgb24(nv12: &[u8], width: u32, height: u32) -> Vec<u8> {
    let pixels = (width * height) as usize;
    let mut rgb = vec![0u8; pixels * 3];

    let y_plane = &nv12[..pixels];
    let uv_plane = &nv12[pixels..];

    for y_pos in 0..height {
        for x_pos in 0..width {
            let idx = (y_pos * width + x_pos) as usize;
            let uv_idx = ((y_pos / 2) * width + (x_pos / 2 * 2)) as usize;

            let y = y_plane[idx] as i32;
            let u = uv_plane.get(uv_idx).copied().unwrap_or(128) as i32 - 128;
            let v = uv_plane.get(uv_idx + 1).copied().unwrap_or(128) as i32 - 128;

            let r = (y + ((v * 359) >> 8)).clamp(0, 255) as u8;
            let g = (y - ((u * 88 + v * 183) >> 8)).clamp(0, 255) as u8;
            let b = (y + ((u * 454) >> 8)).clamp(0, 255) as u8;

            rgb[idx * 3] = r;
            rgb[idx * 3 + 1] = g;
            rgb[idx * 3 + 2] = b;
        }
    }

    rgb
}

/// Simple MJPEG frame extraction (finds JPEG start/end markers)
pub fn extract_mjpeg_frame(data: &[u8]) -> Option<&[u8]> {
    // Find SOI marker (0xFFD8)
    let mut start = None;
    for i in 0..data.len().saturating_sub(1) {
        if data[i] == 0xFF && data[i + 1] == 0xD8 {
            start = Some(i);
            break;
        }
    }

    let start = start?;

    // Find EOI marker (0xFFD9)
    for i in start..data.len().saturating_sub(1) {
        if data[i] == 0xFF && data[i + 1] == 0xD9 {
            return Some(&data[start..=i + 1]);
        }
    }

    None
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize webcam subsystem
pub fn init() {
    crate::kprintln!("webcam: initializing camera subsystem");

    // Register any detected USB cameras from the video driver
    scan_for_cameras();

    let manager = CAMERA_MANAGER.lock();
    let count = manager.count();
    crate::kprintln!("webcam: {} camera(s) detected", count);
}

/// Scan for available cameras
fn scan_for_cameras() {
    // Check USB Video driver for detected cameras
    let usb_cameras = crate::drivers::usb::video::list_cameras();

    let mut manager = CAMERA_MANAGER.lock();
    for (id, name, caps) in usb_cameras {
        let camera_caps = CameraCapabilities {
            name: name.clone(),
            driver: String::from("uvc"),
            bus_info: format!("usb:{}", id),
            supported_formats: caps.formats.iter().map(|f| CameraFormat {
                resolution: CameraResolution::new(f.width, f.height),
                pixel_format: match f.format {
                    crate::drivers::usb::video::PixelFormat::Yuy2 => CameraPixelFormat::Yuyv,
                    crate::drivers::usb::video::PixelFormat::Nv12 => CameraPixelFormat::Nv12,
                    crate::drivers::usb::video::PixelFormat::Mjpeg => CameraPixelFormat::Mjpeg,
                    crate::drivers::usb::video::PixelFormat::H264 => CameraPixelFormat::H264,
                    _ => CameraPixelFormat::Yuyv,
                },
                frame_rate: f.frame_rate.unwrap_or(30),
            }).collect(),
            controls: vec![
                CameraControlInfo {
                    control: CameraControl::Brightness,
                    name: "Brightness",
                    min: 0,
                    max: 255,
                    default: 128,
                    step: 1,
                    current: 128,
                    supported: caps.has_brightness,
                },
                CameraControlInfo {
                    control: CameraControl::Contrast,
                    name: "Contrast",
                    min: 0,
                    max: 255,
                    default: 128,
                    step: 1,
                    current: 128,
                    supported: caps.has_contrast,
                },
                CameraControlInfo {
                    control: CameraControl::Saturation,
                    name: "Saturation",
                    min: 0,
                    max: 255,
                    default: 128,
                    step: 1,
                    current: 128,
                    supported: caps.has_saturation,
                },
                CameraControlInfo {
                    control: CameraControl::AutoFocus,
                    name: "Auto Focus",
                    min: 0,
                    max: 1,
                    default: 1,
                    step: 1,
                    current: 1,
                    supported: caps.has_autofocus,
                },
                CameraControlInfo {
                    control: CameraControl::Zoom,
                    name: "Zoom",
                    min: 100,
                    max: 500,
                    default: 100,
                    step: 10,
                    current: 100,
                    supported: caps.has_zoom,
                },
            ],
            can_capture: true,
            can_stream: true,
            has_autofocus: caps.has_autofocus,
            has_zoom: caps.has_zoom,
        };

        manager.register_usb_camera(id, name, camera_caps);
    }
}

/// Open a camera by index
pub fn open(index: usize) -> Result<CameraHandle, &'static str> {
    let mut manager = CAMERA_MANAGER.lock();
    let camera = manager.get_by_index(index).ok_or("Camera not found")?;
    camera.open()?;
    Ok(camera.handle)
}

/// Close a camera
pub fn close(handle: CameraHandle) -> Result<(), &'static str> {
    let mut manager = CAMERA_MANAGER.lock();
    let camera = manager.get(handle).ok_or("Camera not found")?;
    camera.close()
}

/// List available cameras
pub fn list() -> Vec<(CameraHandle, String)> {
    let manager = CAMERA_MANAGER.lock();
    manager.list().into_iter().map(|(h, s)| (h, String::from(s))).collect()
}

/// Get camera count
pub fn count() -> usize {
    let manager = CAMERA_MANAGER.lock();
    manager.count()
}

/// Capture a single frame (blocking)
pub fn capture_frame(handle: CameraHandle) -> Result<Vec<u8>, &'static str> {
    let mut manager = CAMERA_MANAGER.lock();
    let camera = manager.get(handle).ok_or("Camera not found")?;

    // Setup for single frame capture
    let was_streaming = camera.state == CameraState::Streaming;

    if !was_streaming {
        if camera.state != CameraState::Open {
            camera.open()?;
        }
        camera.request_buffers(4)?;
        for i in 0..4 {
            camera.queue_buffer(i)?;
        }
        camera.start_streaming()?;
    }

    // Wait for frame (simple polling)
    for _ in 0..1000 {
        if let Some(frame) = camera.dequeue_frame() {
            let data = frame.data[..frame.bytes_used].to_vec();

            if !was_streaming {
                let _ = camera.stop_streaming();
            }

            return Ok(data);
        }
        // Small delay
        for _ in 0..10000 {
            core::hint::spin_loop();
        }
    }

    if !was_streaming {
        let _ = camera.stop_streaming();
    }

    Err("Frame capture timeout")
}

/// Format camera info for display
pub fn format_camera_info(handle: CameraHandle) -> Option<String> {
    let manager = CAMERA_MANAGER.lock();

    // Find camera (immutable access)
    let camera = manager.cameras.iter().find(|c| c.handle == handle)?;

    let mut info = format!("Camera: {}\n", camera.name);
    info.push_str(&format!("  State: {:?}\n", camera.state));
    info.push_str(&format!("  Format: {}x{} {} @{}fps\n",
        camera.format.resolution.width,
        camera.format.resolution.height,
        camera.format.pixel_format.name(),
        camera.format.frame_rate));

    info.push_str("  Supported formats:\n");
    for fmt in &camera.capabilities.supported_formats {
        info.push_str(&format!("    - {}x{} {} @{}fps\n",
            fmt.resolution.width,
            fmt.resolution.height,
            fmt.pixel_format.name(),
            fmt.frame_rate));
    }

    info.push_str("  Controls:\n");
    for ctrl in &camera.capabilities.controls {
        if ctrl.supported {
            info.push_str(&format!("    - {}: {} ({}..{})\n",
                ctrl.name, ctrl.current, ctrl.min, ctrl.max));
        }
    }

    Some(info)
}
