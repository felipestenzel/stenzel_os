//! Fingerprint Reader Driver
//!
//! Implements support for various USB fingerprint sensors:
//! - Validity/Synaptics sensors
//! - Elan sensors
//! - Goodix sensors
//! - AuthenTec sensors
//!
//! Features:
//! - Finger enrollment
//! - Finger verification/matching
//! - Biometric template storage
//! - Multi-user support

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// Fingerprint template ID
pub type TemplateId = u64;

/// Global template ID counter
static NEXT_TEMPLATE_ID: AtomicU64 = AtomicU64::new(1);

fn next_template_id() -> TemplateId {
    NEXT_TEMPLATE_ID.fetch_add(1, Ordering::Relaxed)
}

/// USB Vendor IDs
pub mod vendor_ids {
    pub const VALIDITY: u16 = 0x138A;
    pub const SYNAPTICS: u16 = 0x06CB;
    pub const ELAN: u16 = 0x04F3;
    pub const GOODIX: u16 = 0x27C6;
    pub const AUTHENTEC: u16 = 0x08FF;
    pub const UPEK: u16 = 0x147E;
    pub const STM: u16 = 0x0483;
    pub const FOCAL: u16 = 0x2808;
    pub const FPC: u16 = 0x10A5;
}

/// Device IDs for common fingerprint readers
pub mod device_ids {
    // Validity/Synaptics
    pub const VALIDITY_VFS495: u16 = 0x003F;
    pub const VALIDITY_VFS5011: u16 = 0x0011;
    pub const VALIDITY_VFS5111: u16 = 0x0017;
    pub const VALIDITY_VFS7552: u16 = 0x0097;
    pub const SYNAPTICS_PROMETHEUS: u16 = 0x00BD;
    pub const SYNAPTICS_METALLICA: u16 = 0x00C9;

    // Elan
    pub const ELAN_0903: u16 = 0x0903;
    pub const ELAN_0C03: u16 = 0x0C03;
    pub const ELAN_0C4F: u16 = 0x0C4F;
    pub const ELAN_0C58: u16 = 0x0C58;

    // Goodix
    pub const GOODIX_5110: u16 = 0x5110;
    pub const GOODIX_5395: u16 = 0x5395;
    pub const GOODIX_55B4: u16 = 0x55B4;
    pub const GOODIX_5385: u16 = 0x5385;

    // AuthenTec
    pub const AUTHENTEC_2810: u16 = 0x2810;
    pub const AUTHENTEC_1600: u16 = 0x1600;

    // FPC
    pub const FPC_9500: u16 = 0x9500;
    pub const FPC_9501: u16 = 0x9501;
}

/// Sensor type/manufacturer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorType {
    Validity,
    Synaptics,
    Elan,
    Goodix,
    AuthenTec,
    Upek,
    Stm,
    Focal,
    Fpc,
    Unknown,
}

impl SensorType {
    pub fn from_vendor_id(vendor: u16) -> Self {
        match vendor {
            vendor_ids::VALIDITY => SensorType::Validity,
            vendor_ids::SYNAPTICS => SensorType::Synaptics,
            vendor_ids::ELAN => SensorType::Elan,
            vendor_ids::GOODIX => SensorType::Goodix,
            vendor_ids::AUTHENTEC => SensorType::AuthenTec,
            vendor_ids::UPEK => SensorType::Upek,
            vendor_ids::STM => SensorType::Stm,
            vendor_ids::FOCAL => SensorType::Focal,
            vendor_ids::FPC => SensorType::Fpc,
            _ => SensorType::Unknown,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            SensorType::Validity => "Validity",
            SensorType::Synaptics => "Synaptics",
            SensorType::Elan => "Elan",
            SensorType::Goodix => "Goodix",
            SensorType::AuthenTec => "AuthenTec",
            SensorType::Upek => "UPEK",
            SensorType::Stm => "STMicroelectronics",
            SensorType::Focal => "FocalTech",
            SensorType::Fpc => "FPC",
            SensorType::Unknown => "Unknown",
        }
    }
}

/// Sensor technology type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorTechnology {
    /// Capacitive sensor (most common)
    Capacitive,
    /// Optical sensor
    Optical,
    /// Ultrasonic sensor
    Ultrasonic,
    /// Thermal sensor
    Thermal,
    /// Unknown technology
    Unknown,
}

impl SensorTechnology {
    pub fn name(&self) -> &'static str {
        match self {
            SensorTechnology::Capacitive => "Capacitive",
            SensorTechnology::Optical => "Optical",
            SensorTechnology::Ultrasonic => "Ultrasonic",
            SensorTechnology::Thermal => "Thermal",
            SensorTechnology::Unknown => "Unknown",
        }
    }
}

/// Scan quality level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanQuality {
    /// Excellent quality
    Excellent,
    /// Good quality
    Good,
    /// Acceptable quality
    Acceptable,
    /// Poor quality - retry recommended
    Poor,
    /// Very poor - must retry
    Failed,
}

impl ScanQuality {
    pub fn from_score(score: u8) -> Self {
        match score {
            90..=100 => ScanQuality::Excellent,
            70..=89 => ScanQuality::Good,
            50..=69 => ScanQuality::Acceptable,
            20..=49 => ScanQuality::Poor,
            _ => ScanQuality::Failed,
        }
    }

    pub fn is_acceptable(&self) -> bool {
        matches!(
            self,
            ScanQuality::Excellent | ScanQuality::Good | ScanQuality::Acceptable
        )
    }
}

/// Finger position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FingerPosition {
    LeftThumb,
    LeftIndex,
    LeftMiddle,
    LeftRing,
    LeftPinky,
    RightThumb,
    RightIndex,
    RightMiddle,
    RightRing,
    RightPinky,
    Unknown,
}

impl FingerPosition {
    pub fn name(&self) -> &'static str {
        match self {
            FingerPosition::LeftThumb => "Left Thumb",
            FingerPosition::LeftIndex => "Left Index",
            FingerPosition::LeftMiddle => "Left Middle",
            FingerPosition::LeftRing => "Left Ring",
            FingerPosition::LeftPinky => "Left Pinky",
            FingerPosition::RightThumb => "Right Thumb",
            FingerPosition::RightIndex => "Right Index",
            FingerPosition::RightMiddle => "Right Middle",
            FingerPosition::RightRing => "Right Ring",
            FingerPosition::RightPinky => "Right Pinky",
            FingerPosition::Unknown => "Unknown",
        }
    }
}

/// Enrollment state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollmentState {
    /// Not enrolling
    Idle,
    /// Waiting for finger
    WaitingForFinger,
    /// Capturing image
    Capturing,
    /// Processing captured image
    Processing,
    /// Need more captures
    NeedMoreCaptures,
    /// Enrollment complete
    Complete,
    /// Enrollment failed
    Failed,
}

/// Verification result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyResult {
    /// Match found
    Match(TemplateId),
    /// No match found
    NoMatch,
    /// Finger not detected
    NoFinger,
    /// Scan quality too low
    PoorQuality,
    /// Sensor error
    Error,
    /// Verification in progress
    InProgress,
}

/// Device state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    /// Device disconnected
    Disconnected,
    /// Device connected but not initialized
    Connected,
    /// Device initializing
    Initializing,
    /// Device idle and ready
    Ready,
    /// Device busy (scanning/enrolling/verifying)
    Busy,
    /// Device in error state
    Error,
    /// Device suspended (low power)
    Suspended,
}

/// A fingerprint template (biometric data)
#[derive(Clone)]
pub struct FingerprintTemplate {
    /// Template ID
    pub id: TemplateId,
    /// User ID this template belongs to
    pub user_id: u32,
    /// Finger position
    pub finger: FingerPosition,
    /// Template data (encrypted/hashed)
    pub data: Vec<u8>,
    /// Template format version
    pub format_version: u8,
    /// Creation timestamp
    pub created_at: u64,
    /// Last used timestamp
    pub last_used: u64,
    /// Number of times used
    pub use_count: u32,
    /// Quality score
    pub quality_score: u8,
    /// Label/name
    pub label: String,
}

impl FingerprintTemplate {
    pub fn new(user_id: u32, finger: FingerPosition, data: Vec<u8>, quality: u8) -> Self {
        Self {
            id: next_template_id(),
            user_id,
            finger,
            data,
            format_version: 1,
            created_at: crate::time::uptime_secs(),
            last_used: 0,
            use_count: 0,
            quality_score: quality,
            label: String::new(),
        }
    }

    pub fn touch(&mut self) {
        self.last_used = crate::time::uptime_secs();
        self.use_count += 1;
    }
}

/// A scan result (raw capture)
#[derive(Clone)]
pub struct ScanResult {
    /// Raw image data
    pub image: Vec<u8>,
    /// Image width
    pub width: u16,
    /// Image height
    pub height: u16,
    /// Bits per pixel
    pub bpp: u8,
    /// Quality score (0-100)
    pub quality: u8,
    /// Timestamp
    pub timestamp: u64,
}

impl ScanResult {
    pub fn quality_level(&self) -> ScanQuality {
        ScanQuality::from_score(self.quality)
    }
}

/// Enrollment progress
#[derive(Clone)]
pub struct EnrollmentProgress {
    /// Current state
    pub state: EnrollmentState,
    /// Number of captures completed
    pub captures_done: u8,
    /// Number of captures needed
    pub captures_needed: u8,
    /// Last scan quality
    pub last_quality: ScanQuality,
    /// Partial template being built
    pub partial_data: Vec<Vec<u8>>,
    /// Target user ID
    pub user_id: u32,
    /// Target finger
    pub finger: FingerPosition,
}

impl EnrollmentProgress {
    pub fn new(user_id: u32, finger: FingerPosition, captures_needed: u8) -> Self {
        Self {
            state: EnrollmentState::Idle,
            captures_done: 0,
            captures_needed,
            last_quality: ScanQuality::Failed,
            partial_data: Vec::new(),
            user_id,
            finger,
        }
    }

    pub fn progress_percent(&self) -> u8 {
        if self.captures_needed == 0 {
            return 100;
        }
        ((self.captures_done as u32 * 100) / self.captures_needed as u32) as u8
    }
}

/// Fingerprint sensor device
pub struct FingerprintDevice {
    /// Vendor ID
    pub vendor_id: u16,
    /// Device ID
    pub device_id: u16,
    /// Sensor type
    pub sensor_type: SensorType,
    /// Technology type
    pub technology: SensorTechnology,
    /// Device state
    pub state: DeviceState,
    /// Sensor resolution (DPI)
    pub resolution_dpi: u16,
    /// Image width
    pub image_width: u16,
    /// Image height
    pub image_height: u16,
    /// Device name
    pub name: String,
    /// Firmware version
    pub firmware_version: String,
    /// Serial number
    pub serial_number: String,
    /// USB endpoint for commands
    pub cmd_endpoint: u8,
    /// USB endpoint for data
    pub data_endpoint: u8,
    /// USB slot ID (for xHCI)
    pub usb_slot_id: u8,
    /// Current enrollment progress
    pub enrollment: Option<EnrollmentProgress>,
    /// Stored templates
    pub templates: BTreeMap<TemplateId, FingerprintTemplate>,
    /// Scan callback
    pub on_scan: Option<fn(ScanResult)>,
    /// Match callback
    pub on_match: Option<fn(VerifyResult)>,
}

impl FingerprintDevice {
    pub fn new(vendor_id: u16, device_id: u16) -> Self {
        let sensor_type = SensorType::from_vendor_id(vendor_id);

        // Determine technology based on vendor
        let technology = match sensor_type {
            SensorType::Goodix => SensorTechnology::Optical,
            SensorType::Validity | SensorType::Synaptics | SensorType::Elan => {
                SensorTechnology::Capacitive
            }
            _ => SensorTechnology::Unknown,
        };

        // Default resolution and image size
        let (resolution, width, height) = match sensor_type {
            SensorType::Validity | SensorType::Synaptics => (500, 144, 144),
            SensorType::Elan => (508, 96, 96),
            SensorType::Goodix => (508, 88, 108),
            _ => (500, 128, 128),
        };

        Self {
            vendor_id,
            device_id,
            sensor_type,
            technology,
            state: DeviceState::Disconnected,
            resolution_dpi: resolution,
            image_width: width,
            image_height: height,
            name: alloc::format!("{} Fingerprint Sensor", sensor_type.name()),
            firmware_version: String::new(),
            serial_number: String::new(),
            cmd_endpoint: 0,
            data_endpoint: 0,
            usb_slot_id: 0,
            enrollment: None,
            templates: BTreeMap::new(),
            on_scan: None,
            on_match: None,
        }
    }

    /// Initialize the device
    pub fn init(&mut self) -> Result<(), &'static str> {
        self.state = DeviceState::Initializing;

        // Send initialization commands based on sensor type
        match self.sensor_type {
            SensorType::Validity | SensorType::Synaptics => {
                self.init_validity()?;
            }
            SensorType::Elan => {
                self.init_elan()?;
            }
            SensorType::Goodix => {
                self.init_goodix()?;
            }
            _ => {
                self.init_generic()?;
            }
        }

        self.state = DeviceState::Ready;
        Ok(())
    }

    /// Initialize Validity/Synaptics sensor
    fn init_validity(&mut self) -> Result<(), &'static str> {
        // In real implementation, this would send USB control transfers
        // to initialize the sensor
        crate::kprintln!("fingerprint: Initializing Validity sensor");

        // Get device info
        // Set operating mode
        // Configure capture parameters

        Ok(())
    }

    /// Initialize Elan sensor
    fn init_elan(&mut self) -> Result<(), &'static str> {
        crate::kprintln!("fingerprint: Initializing Elan sensor");

        // Elan sensors use HID protocol
        // Send GET_DESCRIPTOR for sensor info
        // Configure sensor mode

        Ok(())
    }

    /// Initialize Goodix sensor
    fn init_goodix(&mut self) -> Result<(), &'static str> {
        crate::kprintln!("fingerprint: Initializing Goodix sensor");

        // Goodix uses custom protocol
        // Initialize communication
        // Configure capture settings

        Ok(())
    }

    /// Initialize generic sensor
    fn init_generic(&mut self) -> Result<(), &'static str> {
        crate::kprintln!("fingerprint: Initializing generic sensor");
        Ok(())
    }

    /// Capture a fingerprint image
    pub fn capture(&mut self) -> Result<ScanResult, &'static str> {
        if self.state != DeviceState::Ready {
            return Err("Device not ready");
        }

        self.state = DeviceState::Busy;

        // In real implementation, this would:
        // 1. Wait for finger detection
        // 2. Capture image from sensor
        // 3. Calculate quality score

        // Simulated capture result
        let result = ScanResult {
            image: Vec::new(),
            width: self.image_width,
            height: self.image_height,
            bpp: 8,
            quality: 75,
            timestamp: crate::time::uptime_secs(),
        };

        self.state = DeviceState::Ready;

        if let Some(callback) = self.on_scan {
            callback(result.clone());
        }

        Ok(result)
    }

    /// Start enrollment process
    pub fn start_enrollment(
        &mut self,
        user_id: u32,
        finger: FingerPosition,
    ) -> Result<(), &'static str> {
        if self.state != DeviceState::Ready {
            return Err("Device not ready");
        }

        // Typically need 3-5 captures for enrollment
        let captures_needed = match self.sensor_type {
            SensorType::Validity | SensorType::Synaptics => 5,
            SensorType::Elan => 3,
            SensorType::Goodix => 4,
            _ => 4,
        };

        self.enrollment = Some(EnrollmentProgress::new(user_id, finger, captures_needed));

        if let Some(ref mut enrollment) = self.enrollment {
            enrollment.state = EnrollmentState::WaitingForFinger;
        }

        Ok(())
    }

    /// Process enrollment capture
    pub fn enrollment_capture(&mut self) -> Result<EnrollmentState, &'static str> {
        // First check enrollment state without holding mutable borrow
        {
            let enrollment = self.enrollment.as_ref().ok_or("No enrollment in progress")?;
            if enrollment.state == EnrollmentState::Complete
                || enrollment.state == EnrollmentState::Failed
            {
                return Ok(enrollment.state);
            }
        }

        // Set state to capturing
        if let Some(ref mut enrollment) = self.enrollment {
            enrollment.state = EnrollmentState::Capturing;
        }

        // Capture fingerprint (requires mutable borrow of self)
        let scan = self.capture()?;

        // Now update enrollment with capture results
        let enrollment = self.enrollment.as_mut().ok_or("No enrollment in progress")?;

        enrollment.state = EnrollmentState::Processing;

        // Check quality
        let quality = scan.quality_level();
        enrollment.last_quality = quality;

        if !quality.is_acceptable() {
            enrollment.state = EnrollmentState::WaitingForFinger;
            return Ok(EnrollmentState::NeedMoreCaptures);
        }

        // Store partial data
        enrollment.partial_data.push(scan.image);
        enrollment.captures_done += 1;

        if enrollment.captures_done >= enrollment.captures_needed {
            // Create template from captures
            enrollment.state = EnrollmentState::Complete;
        } else {
            enrollment.state = EnrollmentState::NeedMoreCaptures;
        }

        Ok(enrollment.state)
    }

    /// Finish enrollment and create template
    pub fn finish_enrollment(&mut self) -> Result<TemplateId, &'static str> {
        let enrollment = self.enrollment.take().ok_or("No enrollment in progress")?;

        if enrollment.state != EnrollmentState::Complete {
            return Err("Enrollment not complete");
        }

        // In real implementation, this would:
        // 1. Process all captured images
        // 2. Extract minutiae
        // 3. Create template

        // For now, just concatenate the data
        let template_data: Vec<u8> = enrollment
            .partial_data
            .iter()
            .flat_map(|d| d.iter().cloned())
            .collect();

        let template =
            FingerprintTemplate::new(enrollment.user_id, enrollment.finger, template_data, 80);

        let id = template.id;
        self.templates.insert(id, template);

        Ok(id)
    }

    /// Cancel enrollment
    pub fn cancel_enrollment(&mut self) {
        self.enrollment = None;
    }

    /// Verify fingerprint against stored templates
    pub fn verify(&mut self) -> VerifyResult {
        if self.state != DeviceState::Ready {
            return VerifyResult::Error;
        }

        // Capture fingerprint
        let scan = match self.capture() {
            Ok(s) => s,
            Err(_) => return VerifyResult::Error,
        };

        // Check quality
        if !scan.quality_level().is_acceptable() {
            return VerifyResult::PoorQuality;
        }

        // Match against templates
        // In real implementation, this would use proper minutiae matching
        for (id, template) in &mut self.templates {
            // Simulated matching - in reality would compare minutiae
            if !template.data.is_empty() {
                template.touch();

                let result = VerifyResult::Match(*id);
                if let Some(callback) = self.on_match {
                    callback(result);
                }
                return result;
            }
        }

        let result = VerifyResult::NoMatch;
        if let Some(callback) = self.on_match {
            callback(result);
        }
        result
    }

    /// Verify against a specific user's templates
    pub fn verify_user(&mut self, user_id: u32) -> VerifyResult {
        if self.state != DeviceState::Ready {
            return VerifyResult::Error;
        }

        // Capture fingerprint
        let scan = match self.capture() {
            Ok(s) => s,
            Err(_) => return VerifyResult::Error,
        };

        if !scan.quality_level().is_acceptable() {
            return VerifyResult::PoorQuality;
        }

        // Match against user's templates only
        for (id, template) in &mut self.templates {
            if template.user_id == user_id && !template.data.is_empty() {
                template.touch();

                let result = VerifyResult::Match(*id);
                if let Some(callback) = self.on_match {
                    callback(result);
                }
                return result;
            }
        }

        VerifyResult::NoMatch
    }

    /// Get template by ID
    pub fn get_template(&self, id: TemplateId) -> Option<&FingerprintTemplate> {
        self.templates.get(&id)
    }

    /// Get templates for a user
    pub fn get_user_templates(&self, user_id: u32) -> Vec<&FingerprintTemplate> {
        self.templates
            .values()
            .filter(|t| t.user_id == user_id)
            .collect()
    }

    /// Delete a template
    pub fn delete_template(&mut self, id: TemplateId) -> bool {
        self.templates.remove(&id).is_some()
    }

    /// Delete all templates for a user
    pub fn delete_user_templates(&mut self, user_id: u32) -> usize {
        let ids: Vec<TemplateId> = self
            .templates
            .iter()
            .filter(|(_, t)| t.user_id == user_id)
            .map(|(id, _)| *id)
            .collect();

        let count = ids.len();
        for id in ids {
            self.templates.remove(&id);
        }
        count
    }

    /// Get device info string
    pub fn info_string(&self) -> String {
        alloc::format!(
            "{} ({:04X}:{:04X})\n  Type: {} ({})\n  Resolution: {} DPI\n  Image: {}x{}\n  State: {:?}\n  Templates: {}",
            self.name,
            self.vendor_id,
            self.device_id,
            self.sensor_type.name(),
            self.technology.name(),
            self.resolution_dpi,
            self.image_width,
            self.image_height,
            self.state,
            self.templates.len(),
        )
    }
}

/// Fingerprint subsystem manager
pub struct FingerprintManager {
    /// Connected devices
    pub devices: Vec<FingerprintDevice>,
    /// Default device index
    pub default_device: Option<usize>,
    /// Initialized flag
    pub initialized: bool,
}

impl FingerprintManager {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            default_device: None,
            initialized: false,
        }
    }

    /// Probe for fingerprint devices
    pub fn probe(&mut self) {
        // In real implementation, this would scan USB bus for fingerprint devices
        // For now, we just mark as initialized

        crate::kprintln!("fingerprint: Scanning for fingerprint readers...");

        // The real implementation would:
        // 1. Scan USB devices for known vendor IDs
        // 2. Check device class (HID or vendor-specific)
        // 3. Initialize each found device

        self.initialized = true;
    }

    /// Register a device
    pub fn register_device(&mut self, device: FingerprintDevice) -> usize {
        let index = self.devices.len();
        self.devices.push(device);

        if self.default_device.is_none() {
            self.default_device = Some(index);
        }

        crate::kprintln!("fingerprint: Registered device at index {}", index);
        index
    }

    /// Get device count
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Get device by index
    pub fn get_device(&mut self, index: usize) -> Option<&mut FingerprintDevice> {
        self.devices.get_mut(index)
    }

    /// Get default device
    pub fn get_default_device(&mut self) -> Option<&mut FingerprintDevice> {
        self.default_device
            .and_then(move |i| self.devices.get_mut(i))
    }

    /// Set default device
    pub fn set_default_device(&mut self, index: usize) -> bool {
        if index < self.devices.len() {
            self.default_device = Some(index);
            true
        } else {
            false
        }
    }

    /// Start enrollment on default device
    pub fn enroll(&mut self, user_id: u32, finger: FingerPosition) -> Result<(), &'static str> {
        let device = self
            .get_default_device()
            .ok_or("No fingerprint device available")?;
        device.start_enrollment(user_id, finger)
    }

    /// Verify on default device
    pub fn verify(&mut self) -> VerifyResult {
        match self.default_device {
            Some(i) => {
                if let Some(device) = self.devices.get_mut(i) {
                    device.verify()
                } else {
                    VerifyResult::Error
                }
            }
            None => VerifyResult::Error,
        }
    }

    /// Verify user on default device
    pub fn verify_user(&mut self, user_id: u32) -> VerifyResult {
        match self.default_device {
            Some(i) => {
                if let Some(device) = self.devices.get_mut(i) {
                    device.verify_user(user_id)
                } else {
                    VerifyResult::Error
                }
            }
            None => VerifyResult::Error,
        }
    }

    /// Get all templates across all devices
    pub fn all_templates(&self) -> Vec<&FingerprintTemplate> {
        self.devices
            .iter()
            .flat_map(|d| d.templates.values())
            .collect()
    }

    /// Get template count across all devices
    pub fn template_count(&self) -> usize {
        self.devices.iter().map(|d| d.templates.len()).sum()
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        let mut s = String::from("Fingerprint Subsystem:\n");

        if self.devices.is_empty() {
            s.push_str("  No fingerprint devices found\n");
            return s;
        }

        for (i, device) in self.devices.iter().enumerate() {
            let default_marker = if Some(i) == self.default_device {
                " [default]"
            } else {
                ""
            };
            s.push_str(&alloc::format!("\nDevice {}{}:\n  {}\n", i, default_marker, device.info_string()));
        }

        s
    }
}

impl Default for FingerprintManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global fingerprint manager
static mut FINGERPRINT_MANAGER: Option<FingerprintManager> = None;

/// Initialize the fingerprint subsystem
pub fn init() {
    let mut manager = FingerprintManager::new();
    manager.probe();

    let device_count = manager.device_count();

    unsafe {
        FINGERPRINT_MANAGER = Some(manager);
    }

    crate::kprintln!("fingerprint: initialized ({} devices)", device_count);
}

/// Get the fingerprint manager
pub fn manager() -> &'static mut FingerprintManager {
    unsafe {
        FINGERPRINT_MANAGER
            .as_mut()
            .expect("Fingerprint subsystem not initialized")
    }
}

/// Get device count
pub fn device_count() -> usize {
    manager().device_count()
}

/// Verify fingerprint
pub fn verify() -> VerifyResult {
    manager().verify()
}

/// Verify fingerprint for a specific user
pub fn verify_user(user_id: u32) -> VerifyResult {
    manager().verify_user(user_id)
}

/// Start enrollment
pub fn enroll(user_id: u32, finger: FingerPosition) -> Result<(), &'static str> {
    manager().enroll(user_id, finger)
}

/// Get template count
pub fn template_count() -> usize {
    manager().template_count()
}

/// Format status
pub fn format_status() -> String {
    manager().format_status()
}

/// Check if a USB device is a fingerprint reader
pub fn is_fingerprint_device(vendor_id: u16, device_id: u16) -> bool {
    match vendor_id {
        vendor_ids::VALIDITY => true,
        vendor_ids::SYNAPTICS => matches!(
            device_id,
            device_ids::SYNAPTICS_PROMETHEUS | device_ids::SYNAPTICS_METALLICA
        ),
        vendor_ids::ELAN => matches!(
            device_id,
            device_ids::ELAN_0903
                | device_ids::ELAN_0C03
                | device_ids::ELAN_0C4F
                | device_ids::ELAN_0C58
        ),
        vendor_ids::GOODIX => true,
        vendor_ids::AUTHENTEC => true,
        vendor_ids::UPEK => true,
        vendor_ids::FPC => true,
        _ => false,
    }
}

/// Register a USB fingerprint device
pub fn register_usb_device(vendor_id: u16, device_id: u16, slot_id: u8) -> Option<usize> {
    if !is_fingerprint_device(vendor_id, device_id) {
        return None;
    }

    let mut device = FingerprintDevice::new(vendor_id, device_id);
    device.usb_slot_id = slot_id;
    device.state = DeviceState::Connected;

    // Try to initialize
    if let Err(e) = device.init() {
        crate::kprintln!("fingerprint: Failed to initialize device: {}", e);
        device.state = DeviceState::Error;
    }

    Some(manager().register_device(device))
}
