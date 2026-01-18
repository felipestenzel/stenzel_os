//! PAM Fingerprint Authentication Module (pam_fprintd)
//!
//! Provides fingerprint-based authentication for PAM. Integrates with the
//! fingerprint driver to provide biometric login capabilities.
//!
//! ## Features
//! - Fingerprint verification for login
//! - Multi-user template support
//! - Fallback to password authentication
//! - Timeout handling
//! - Quality feedback
//! - Enrollment management via separate utility
//!
//! ## Usage
//! Add to PAM stack:
//! ```text
//! auth sufficient pam_fprintd.so
//! auth required pam_unix.so
//! ```

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KError, KResult};
use super::pam::{PamHandle, PamModule, PamResult, PamMessageStyle};
use crate::drivers::fingerprint::{self, VerifyResult, TemplateId, FingerPosition};

/// Fingerprint authentication configuration
#[derive(Debug, Clone)]
pub struct FprintdConfig {
    /// Maximum attempts before falling back to password
    pub max_attempts: u8,
    /// Timeout for finger detection (seconds)
    pub timeout_secs: u32,
    /// Show finger placement guidance
    pub show_guidance: bool,
    /// Allow fallback to password
    pub allow_fallback: bool,
    /// Minimum quality score for verification
    pub min_quality: u8,
    /// Require exact user match (vs any enrolled user)
    pub require_user_match: bool,
    /// Enable debug logging
    pub debug: bool,
    /// Finger detection polling interval (ms)
    pub poll_interval_ms: u32,
}

impl Default for FprintdConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            timeout_secs: 30,
            show_guidance: true,
            allow_fallback: true,
            min_quality: 50,
            require_user_match: true,
            debug: false,
            poll_interval_ms: 100,
        }
    }
}

/// Authentication attempt result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthAttemptResult {
    /// Fingerprint matched successfully
    Success(u32, TemplateId),  // (user_id, template_id)
    /// No match found
    NoMatch,
    /// Finger not detected (timeout)
    Timeout,
    /// Poor scan quality
    PoorQuality,
    /// No fingerprint device available
    NoDevice,
    /// No templates enrolled for user
    NoEnrollment,
    /// User cancelled (chose password fallback)
    Cancelled,
    /// Internal error
    Error,
}

/// Fingerprint authentication statistics
#[derive(Debug, Clone, Default)]
pub struct FprintdStats {
    /// Total authentication attempts
    pub total_attempts: u64,
    /// Successful authentications
    pub successful_auths: u64,
    /// Failed authentications
    pub failed_auths: u64,
    /// Timeout occurrences
    pub timeouts: u64,
    /// Poor quality scans
    pub poor_quality_scans: u64,
    /// Fallbacks to password
    pub fallbacks: u64,
    /// Last authentication timestamp
    pub last_auth_time: u64,
}

/// Enrolled user information
#[derive(Debug, Clone)]
pub struct EnrolledUser {
    /// User ID
    pub user_id: u32,
    /// Username
    pub username: String,
    /// Number of enrolled fingers
    pub enrolled_fingers: u8,
    /// Enrolled finger positions
    pub finger_positions: Vec<FingerPosition>,
    /// Template IDs
    pub template_ids: Vec<TemplateId>,
    /// Enrollment timestamp
    pub enrolled_at: u64,
    /// Last successful auth timestamp
    pub last_auth: u64,
    /// Auth success count
    pub auth_count: u32,
}

/// PAM fingerprint module state
pub struct PamFprintd {
    /// Configuration
    config: FprintdConfig,
    /// User ID to username mapping
    user_map: BTreeMap<u32, String>,
    /// Enrolled users
    enrolled_users: BTreeMap<u32, EnrolledUser>,
    /// Statistics
    stats: FprintdStats,
    /// Initialized flag
    initialized: bool,
    /// Authentication in progress
    auth_in_progress: AtomicBool,
}

impl PamFprintd {
    /// Create a new PAM fingerprint module
    pub const fn new() -> Self {
        Self {
            config: FprintdConfig {
                max_attempts: 3,
                timeout_secs: 30,
                show_guidance: true,
                allow_fallback: true,
                min_quality: 50,
                require_user_match: true,
                debug: false,
                poll_interval_ms: 100,
            },
            user_map: BTreeMap::new(),
            enrolled_users: BTreeMap::new(),
            stats: FprintdStats {
                total_attempts: 0,
                successful_auths: 0,
                failed_auths: 0,
                timeouts: 0,
                poor_quality_scans: 0,
                fallbacks: 0,
                last_auth_time: 0,
            },
            initialized: false,
            auth_in_progress: AtomicBool::new(false),
        }
    }

    /// Initialize the module
    pub fn init(&mut self) {
        if self.initialized {
            return;
        }

        // Ensure fingerprint subsystem is initialized
        if fingerprint::device_count() == 0 {
            crate::kprintln!("pam_fprintd: no fingerprint devices available");
        }

        // Load enrolled users from storage
        self.load_enrollments();

        self.initialized = true;
        crate::kprintln!("pam_fprintd: initialized ({} enrolled users)", self.enrolled_users.len());
    }

    /// Load enrolled users from persistent storage
    fn load_enrollments(&mut self) {
        // In a real implementation, this would read from /var/lib/fprintd/
        // For now, sync with fingerprint device templates

        let manager = fingerprint::manager();

        for device in &manager.devices {
            for template in device.templates.values() {
                let user_id = template.user_id;

                let entry = self.enrolled_users.entry(user_id).or_insert_with(|| {
                    EnrolledUser {
                        user_id,
                        username: self.user_map.get(&user_id).cloned()
                            .unwrap_or_else(|| alloc::format!("uid{}", user_id)),
                        enrolled_fingers: 0,
                        finger_positions: Vec::new(),
                        template_ids: Vec::new(),
                        enrolled_at: template.created_at,
                        last_auth: 0,
                        auth_count: 0,
                    }
                });

                entry.enrolled_fingers += 1;
                entry.finger_positions.push(template.finger);
                entry.template_ids.push(template.id);
            }
        }
    }

    /// Register a user ID to username mapping
    pub fn register_user(&mut self, user_id: u32, username: &str) {
        self.user_map.insert(user_id, username.to_string());

        // Update enrolled user if exists
        if let Some(enrolled) = self.enrolled_users.get_mut(&user_id) {
            enrolled.username = username.to_string();
        }
    }

    /// Get user ID from username
    pub fn get_user_id(&self, username: &str) -> Option<u32> {
        // First try our map
        for (&uid, name) in &self.user_map {
            if name == username {
                return Some(uid);
            }
        }

        // Try the kernel user database
        if let Some(user) = crate::security::user_db().get(username) {
            return Some(user.uid.0);
        }

        None
    }

    /// Check if user has enrolled fingerprints
    pub fn has_enrollment(&self, user_id: u32) -> bool {
        self.enrolled_users.get(&user_id)
            .map(|u| u.enrolled_fingers > 0)
            .unwrap_or(false)
    }

    /// Check if user has enrolled fingerprints by username
    pub fn has_enrollment_by_name(&self, username: &str) -> bool {
        self.get_user_id(username)
            .map(|uid| self.has_enrollment(uid))
            .unwrap_or(false)
    }

    /// Attempt fingerprint authentication
    pub fn authenticate_user(&mut self, user_id: Option<u32>) -> AuthAttemptResult {
        if self.auth_in_progress.swap(true, Ordering::SeqCst) {
            // Authentication already in progress
            return AuthAttemptResult::Error;
        }

        self.stats.total_attempts += 1;

        // Check for fingerprint device
        if fingerprint::device_count() == 0 {
            self.auth_in_progress.store(false, Ordering::SeqCst);
            return AuthAttemptResult::NoDevice;
        }

        // If specific user required, check enrollment
        if let Some(uid) = user_id {
            if self.config.require_user_match && !self.has_enrollment(uid) {
                self.auth_in_progress.store(false, Ordering::SeqCst);
                return AuthAttemptResult::NoEnrollment;
            }
        }

        // Attempt verification
        let result = if let Some(uid) = user_id {
            fingerprint::verify_user(uid)
        } else {
            fingerprint::verify()
        };

        let auth_result = match result {
            VerifyResult::Match(template_id) => {
                // Find the user ID for this template
                let matched_user_id = self.find_user_for_template(template_id);

                if let Some(uid) = matched_user_id {
                    // Update stats
                    self.stats.successful_auths += 1;
                    self.stats.last_auth_time = crate::time::uptime_secs();

                    // Update enrolled user stats
                    if let Some(enrolled) = self.enrolled_users.get_mut(&uid) {
                        enrolled.last_auth = crate::time::uptime_secs();
                        enrolled.auth_count += 1;
                    }

                    AuthAttemptResult::Success(uid, template_id)
                } else {
                    self.stats.failed_auths += 1;
                    AuthAttemptResult::NoMatch
                }
            }
            VerifyResult::NoMatch => {
                self.stats.failed_auths += 1;
                AuthAttemptResult::NoMatch
            }
            VerifyResult::NoFinger => {
                self.stats.timeouts += 1;
                AuthAttemptResult::Timeout
            }
            VerifyResult::PoorQuality => {
                self.stats.poor_quality_scans += 1;
                AuthAttemptResult::PoorQuality
            }
            VerifyResult::Error => {
                AuthAttemptResult::Error
            }
            VerifyResult::InProgress => {
                AuthAttemptResult::Error
            }
        };

        self.auth_in_progress.store(false, Ordering::SeqCst);
        auth_result
    }

    /// Find user ID for a template
    fn find_user_for_template(&self, template_id: TemplateId) -> Option<u32> {
        for (uid, enrolled) in &self.enrolled_users {
            if enrolled.template_ids.contains(&template_id) {
                return Some(*uid);
            }
        }

        // Also check fingerprint manager directly
        let manager = fingerprint::manager();
        for device in &manager.devices {
            if let Some(template) = device.templates.get(&template_id) {
                return Some(template.user_id);
            }
        }

        None
    }

    /// Enroll a new fingerprint
    pub fn enroll(&mut self, user_id: u32, username: &str, finger: FingerPosition) -> KResult<TemplateId> {
        // Register user mapping
        self.register_user(user_id, username);

        // Start enrollment
        fingerprint::enroll(user_id, finger)
            .map_err(|_| KError::IO)?;

        // Run enrollment process
        let manager = fingerprint::manager();
        let device = manager.get_default_device()
            .ok_or(KError::NotFound)?;

        // Capture multiple samples
        loop {
            match device.enrollment_capture() {
                Ok(fingerprint::EnrollmentState::Complete) => break,
                Ok(fingerprint::EnrollmentState::NeedMoreCaptures) => continue,
                Ok(fingerprint::EnrollmentState::Failed) => {
                    device.cancel_enrollment();
                    return Err(KError::IO);
                }
                Err(_) => {
                    device.cancel_enrollment();
                    return Err(KError::IO);
                }
                _ => continue,
            }
        }

        // Finish enrollment
        let template_id = device.finish_enrollment()
            .map_err(|_| KError::IO)?;

        // Update enrolled users
        let entry = self.enrolled_users.entry(user_id).or_insert_with(|| {
            EnrolledUser {
                user_id,
                username: username.to_string(),
                enrolled_fingers: 0,
                finger_positions: Vec::new(),
                template_ids: Vec::new(),
                enrolled_at: crate::time::uptime_secs(),
                last_auth: 0,
                auth_count: 0,
            }
        });

        entry.enrolled_fingers += 1;
        entry.finger_positions.push(finger);
        entry.template_ids.push(template_id);

        crate::kprintln!("pam_fprintd: enrolled {} for user {} ({})",
            finger.name(), username, user_id);

        Ok(template_id)
    }

    /// Delete enrollment for a user
    pub fn delete_enrollment(&mut self, user_id: u32) -> KResult<()> {
        if let Some(enrolled) = self.enrolled_users.remove(&user_id) {
            let manager = fingerprint::manager();

            if let Some(device) = manager.get_default_device() {
                for template_id in &enrolled.template_ids {
                    device.delete_template(*template_id);
                }
            }
        }

        Ok(())
    }

    /// Get enrollment info for a user
    pub fn get_enrollment(&self, user_id: u32) -> Option<&EnrolledUser> {
        self.enrolled_users.get(&user_id)
    }

    /// List all enrolled users
    pub fn list_enrolled_users(&self) -> Vec<&EnrolledUser> {
        self.enrolled_users.values().collect()
    }

    /// Get configuration
    pub fn config(&self) -> &FprintdConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: FprintdConfig) {
        self.config = config;
    }

    /// Get statistics
    pub fn stats(&self) -> &FprintdStats {
        &self.stats
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        let mut s = String::new();
        use core::fmt::Write;

        let _ = writeln!(s, "PAM Fingerprint Authentication (pam_fprintd):");
        let _ = writeln!(s, "  Initialized: {}", self.initialized);
        let _ = writeln!(s, "  Devices: {}", fingerprint::device_count());
        let _ = writeln!(s, "  Enrolled users: {}", self.enrolled_users.len());
        let _ = writeln!(s, "  Total attempts: {}", self.stats.total_attempts);
        let _ = writeln!(s, "  Successful: {}", self.stats.successful_auths);
        let _ = writeln!(s, "  Failed: {}", self.stats.failed_auths);
        let _ = writeln!(s, "  Timeouts: {}", self.stats.timeouts);
        let _ = writeln!(s, "  Poor quality: {}", self.stats.poor_quality_scans);
        let _ = writeln!(s, "  Fallbacks: {}", self.stats.fallbacks);

        if !self.enrolled_users.is_empty() {
            let _ = writeln!(s, "\nEnrolled Users:");
            for enrolled in self.enrolled_users.values() {
                let _ = writeln!(s, "  {}: {} fingers",
                    enrolled.username, enrolled.enrolled_fingers);
            }
        }

        s
    }
}

/// PAM module implementation
impl PamModule for PamFprintd {
    fn name(&self) -> &str {
        "pam_fprintd"
    }

    fn authenticate(&self, handle: &mut PamHandle, _flags: u32) -> PamResult {
        // Get username
        let username = match handle.get_user() {
            Some(u) => u.to_string(),
            None => {
                // Try to prompt for username
                match handle.prompt_user("login: ") {
                    Ok(u) => {
                        handle.set_user(&u);
                        u
                    }
                    Err(e) => return e,
                }
            }
        };

        // Check if fingerprint devices available
        if fingerprint::device_count() == 0 {
            if self.config.debug {
                crate::kprintln!("pam_fprintd: no fingerprint devices");
            }
            return PamResult::AuthInfoUnavail;
        }

        // Get user ID
        let user_id = match self.get_user_id(&username) {
            Some(uid) => uid,
            None => {
                if self.config.debug {
                    crate::kprintln!("pam_fprintd: unknown user {}", username);
                }
                return PamResult::UserUnknown;
            }
        };

        // Check if user has enrolled fingerprints
        if !self.has_enrollment(user_id) {
            if self.config.debug {
                crate::kprintln!("pam_fprintd: no enrollment for user {}", username);
            }
            return PamResult::AuthInfoUnavail;
        }

        // Show guidance
        if self.config.show_guidance {
            let _ = handle.info("Place your finger on the fingerprint sensor...");
        }

        // Attempt authentication with retries
        for attempt in 1..=self.config.max_attempts {
            // We need to use the global instance to call authenticate_user
            let result = PAM_FPRINTD.lock().authenticate_user(Some(user_id));

            match result {
                AuthAttemptResult::Success(matched_uid, _template_id) => {
                    if matched_uid == user_id {
                        let _ = handle.info("Fingerprint matched.");
                        return PamResult::Success;
                    } else {
                        // Matched different user
                        let _ = handle.error("Fingerprint does not match this user.");
                    }
                }
                AuthAttemptResult::NoMatch => {
                    let _ = handle.error(&alloc::format!(
                        "Fingerprint not recognized (attempt {}/{})",
                        attempt, self.config.max_attempts
                    ));
                }
                AuthAttemptResult::Timeout => {
                    let _ = handle.error("Finger not detected.");
                }
                AuthAttemptResult::PoorQuality => {
                    let _ = handle.error("Please try again with better finger placement.");
                }
                AuthAttemptResult::NoDevice => {
                    return PamResult::AuthInfoUnavail;
                }
                AuthAttemptResult::NoEnrollment => {
                    return PamResult::AuthInfoUnavail;
                }
                AuthAttemptResult::Cancelled => {
                    PAM_FPRINTD.lock().stats.fallbacks += 1;
                    return PamResult::Ignore;
                }
                AuthAttemptResult::Error => {
                    return PamResult::AuthErr;
                }
            }
        }

        // All attempts failed
        PamResult::AuthErr
    }

    fn setcred(&self, _handle: &mut PamHandle, _flags: u32) -> PamResult {
        PamResult::Success
    }

    fn acct_mgmt(&self, handle: &mut PamHandle, _flags: u32) -> PamResult {
        // Check if user account is valid
        let username = match handle.get_user() {
            Some(u) => u,
            None => return PamResult::UserUnknown,
        };

        // Verify user exists and has valid enrollment
        if let Some(_uid) = self.get_user_id(username) {
            PamResult::Success
        } else {
            PamResult::UserUnknown
        }
    }

    fn open_session(&self, _handle: &mut PamHandle, _flags: u32) -> PamResult {
        PamResult::Success
    }

    fn close_session(&self, _handle: &mut PamHandle, _flags: u32) -> PamResult {
        PamResult::Success
    }
}

// =============================================================================
// Fingerprint Enrollment Utility
// =============================================================================

/// Enrollment session state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollSessionState {
    /// Not started
    Idle,
    /// Waiting for finger
    WaitingForFinger,
    /// Capturing
    Capturing,
    /// Processing
    Processing,
    /// Need more samples
    NeedMore(u8, u8),  // (current, total)
    /// Complete
    Complete,
    /// Failed
    Failed,
    /// Cancelled
    Cancelled,
}

/// Enrollment session
pub struct EnrollSession {
    /// Target user ID
    pub user_id: u32,
    /// Target username
    pub username: String,
    /// Finger being enrolled
    pub finger: FingerPosition,
    /// Current state
    pub state: EnrollSessionState,
    /// Resulting template ID
    pub template_id: Option<TemplateId>,
    /// Error message if failed
    pub error: Option<String>,
}

impl EnrollSession {
    /// Create a new enrollment session
    pub fn new(user_id: u32, username: &str, finger: FingerPosition) -> Self {
        Self {
            user_id,
            username: username.to_string(),
            finger,
            state: EnrollSessionState::Idle,
            template_id: None,
            error: None,
        }
    }

    /// Start the enrollment
    pub fn start(&mut self) -> KResult<()> {
        self.state = EnrollSessionState::WaitingForFinger;

        fingerprint::enroll(self.user_id, self.finger)
            .map_err(|_| KError::IO)?;

        Ok(())
    }

    /// Process next capture step
    pub fn step(&mut self) -> EnrollSessionState {
        let manager = fingerprint::manager();
        let device = match manager.get_default_device() {
            Some(d) => d,
            None => {
                self.state = EnrollSessionState::Failed;
                self.error = Some(String::from("No fingerprint device"));
                return self.state;
            }
        };

        match device.enrollment_capture() {
            Ok(fingerprint::EnrollmentState::Complete) => {
                match device.finish_enrollment() {
                    Ok(template_id) => {
                        self.template_id = Some(template_id);
                        self.state = EnrollSessionState::Complete;

                        // Update PAM module
                        let mut pam = PAM_FPRINTD.lock();
                        pam.load_enrollments();
                    }
                    Err(e) => {
                        self.state = EnrollSessionState::Failed;
                        self.error = Some(String::from(e));
                    }
                }
            }
            Ok(fingerprint::EnrollmentState::NeedMoreCaptures) => {
                if let Some(ref enrollment) = device.enrollment {
                    self.state = EnrollSessionState::NeedMore(
                        enrollment.captures_done,
                        enrollment.captures_needed
                    );
                }
            }
            Ok(fingerprint::EnrollmentState::WaitingForFinger) => {
                self.state = EnrollSessionState::WaitingForFinger;
            }
            Ok(fingerprint::EnrollmentState::Capturing) => {
                self.state = EnrollSessionState::Capturing;
            }
            Ok(fingerprint::EnrollmentState::Processing) => {
                self.state = EnrollSessionState::Processing;
            }
            Ok(fingerprint::EnrollmentState::Failed) => {
                self.state = EnrollSessionState::Failed;
                self.error = Some(String::from("Enrollment failed"));
            }
            Err(e) => {
                self.state = EnrollSessionState::Failed;
                self.error = Some(String::from(e));
            }
            _ => {}
        }

        self.state
    }

    /// Cancel enrollment
    pub fn cancel(&mut self) {
        let manager = fingerprint::manager();
        if let Some(device) = manager.get_default_device() {
            device.cancel_enrollment();
        }
        self.state = EnrollSessionState::Cancelled;
    }
}

// =============================================================================
// Global Instance
// =============================================================================

pub static PAM_FPRINTD: IrqSafeMutex<PamFprintd> = IrqSafeMutex::new(PamFprintd::new());

/// Initialize PAM fingerprint module
pub fn init() {
    PAM_FPRINTD.lock().init();
}

/// Check if user has fingerprint enrollment
pub fn has_enrollment(username: &str) -> bool {
    PAM_FPRINTD.lock().has_enrollment_by_name(username)
}

/// Start enrollment for a user
pub fn start_enrollment(user_id: u32, username: &str, finger: FingerPosition) -> KResult<EnrollSession> {
    let mut session = EnrollSession::new(user_id, username, finger);
    session.start()?;
    Ok(session)
}

/// Delete enrollment for a user
pub fn delete_enrollment(user_id: u32) -> KResult<()> {
    PAM_FPRINTD.lock().delete_enrollment(user_id)
}

/// Get enrollment info
pub fn get_enrollment(user_id: u32) -> Option<EnrolledUser> {
    PAM_FPRINTD.lock().get_enrollment(user_id).cloned()
}

/// List all enrolled users
pub fn list_enrolled() -> Vec<EnrolledUser> {
    PAM_FPRINTD.lock().list_enrolled_users().into_iter().cloned().collect()
}

/// Get statistics
pub fn stats() -> FprintdStats {
    PAM_FPRINTD.lock().stats().clone()
}

/// Format status
pub fn status() -> String {
    PAM_FPRINTD.lock().format_status()
}

/// Get the PAM module for integration
pub fn pam_module() -> Box<dyn PamModule> {
    Box::new(PamFprintdWrapper)
}

/// Wrapper for using PamFprintd as a trait object
struct PamFprintdWrapper;

impl PamModule for PamFprintdWrapper {
    fn name(&self) -> &str {
        "pam_fprintd"
    }

    fn authenticate(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        PAM_FPRINTD.lock().authenticate(handle, flags)
    }

    fn setcred(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        PAM_FPRINTD.lock().setcred(handle, flags)
    }

    fn acct_mgmt(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        PAM_FPRINTD.lock().acct_mgmt(handle, flags)
    }

    fn open_session(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        PAM_FPRINTD.lock().open_session(handle, flags)
    }

    fn close_session(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        PAM_FPRINTD.lock().close_session(handle, flags)
    }
}
