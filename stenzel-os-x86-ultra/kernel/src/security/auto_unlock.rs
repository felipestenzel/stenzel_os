//! Auto Unlock
//!
//! Automatic disk/device unlocking using TPM, biometrics, FIDO2, etc.
//! Coordinates multiple unlock methods and provides a unified interface.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::kprintln;

/// Auto unlock state
static AUTO_UNLOCK: IrqSafeMutex<Option<AutoUnlockManager>> = IrqSafeMutex::new(None);

/// Statistics
static STATS: AutoUnlockStats = AutoUnlockStats::new();

/// Unlock target type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnlockTarget {
    /// LUKS encrypted disk
    LuksDisk,
    /// Encrypted home directory
    EncryptedHome,
    /// Keyring/secrets
    Keyring,
    /// SSH key
    SshKey,
    /// GPG key
    GpgKey,
    /// System login
    Login,
    /// Screensaver unlock
    Screensaver,
    /// Sudo authentication
    Sudo,
    /// Custom application
    Custom,
}

impl UnlockTarget {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LuksDisk => "luks-disk",
            Self::EncryptedHome => "encrypted-home",
            Self::Keyring => "keyring",
            Self::SshKey => "ssh-key",
            Self::GpgKey => "gpg-key",
            Self::Login => "login",
            Self::Screensaver => "screensaver",
            Self::Sudo => "sudo",
            Self::Custom => "custom",
        }
    }

    pub fn requires_strong_auth(&self) -> bool {
        matches!(self, Self::LuksDisk | Self::EncryptedHome | Self::GpgKey)
    }
}

/// Authentication method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    /// TPM 2.0 sealed key
    Tpm2,
    /// TPM + PIN
    Tpm2Pin,
    /// Fingerprint
    Fingerprint,
    /// Face recognition
    FaceRecognition,
    /// FIDO2 security key
    Fido2,
    /// Smartcard
    Smartcard,
    /// Password
    Password,
    /// Recovery key
    RecoveryKey,
    /// Bluetooth device proximity
    BluetoothProximity,
    /// NFC token
    Nfc,
}

impl AuthMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tpm2 => "tpm2",
            Self::Tpm2Pin => "tpm2+pin",
            Self::Fingerprint => "fingerprint",
            Self::FaceRecognition => "face",
            Self::Fido2 => "fido2",
            Self::Smartcard => "smartcard",
            Self::Password => "password",
            Self::RecoveryKey => "recovery",
            Self::BluetoothProximity => "bluetooth",
            Self::Nfc => "nfc",
        }
    }

    pub fn is_biometric(&self) -> bool {
        matches!(self, Self::Fingerprint | Self::FaceRecognition)
    }

    pub fn is_hardware(&self) -> bool {
        matches!(self, Self::Tpm2 | Self::Tpm2Pin | Self::Fido2 | Self::Smartcard | Self::Nfc)
    }

    pub fn requires_presence(&self) -> bool {
        matches!(self, Self::Fingerprint | Self::FaceRecognition | Self::Fido2 | Self::Smartcard | Self::Nfc)
    }

    pub fn strength(&self) -> AuthStrength {
        match self {
            Self::Tpm2 => AuthStrength::Medium,
            Self::Tpm2Pin => AuthStrength::Strong,
            Self::Fingerprint => AuthStrength::Medium,
            Self::FaceRecognition => AuthStrength::Medium,
            Self::Fido2 => AuthStrength::Strong,
            Self::Smartcard => AuthStrength::Strong,
            Self::Password => AuthStrength::Varies,
            Self::RecoveryKey => AuthStrength::Strong,
            Self::BluetoothProximity => AuthStrength::Weak,
            Self::Nfc => AuthStrength::Medium,
        }
    }
}

/// Authentication strength
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuthStrength {
    Weak,
    Medium,
    Strong,
    Varies,
}

/// Unlock policy
#[derive(Debug, Clone)]
pub struct UnlockPolicy {
    /// Target to unlock
    pub target: UnlockTarget,
    /// Device/path identifier
    pub identifier: String,
    /// Allowed methods (in priority order)
    pub allowed_methods: Vec<AuthMethod>,
    /// Require multiple factors
    pub require_mfa: bool,
    /// Minimum authentication strength
    pub min_strength: AuthStrength,
    /// Timeout for auto-unlock (seconds, 0 = no timeout)
    pub timeout_secs: u32,
    /// Require user presence (e.g., keypress)
    pub require_presence: bool,
    /// Allow fallback to password
    pub allow_password_fallback: bool,
    /// Auto-lock timeout (seconds, 0 = never)
    pub auto_lock_secs: u32,
    /// Allow while locked
    pub allow_while_locked: bool,
}

impl Default for UnlockPolicy {
    fn default() -> Self {
        Self {
            target: UnlockTarget::Custom,
            identifier: String::new(),
            allowed_methods: vec![AuthMethod::Password],
            require_mfa: false,
            min_strength: AuthStrength::Medium,
            timeout_secs: 30,
            require_presence: false,
            allow_password_fallback: true,
            auto_lock_secs: 0,
            allow_while_locked: false,
        }
    }
}

/// Auto unlock configuration for a target
#[derive(Debug, Clone)]
pub struct AutoUnlockConfig {
    /// Policy
    pub policy: UnlockPolicy,
    /// Enabled
    pub enabled: bool,
    /// Methods configured
    pub configured_methods: Vec<ConfiguredMethod>,
    /// Last unlock time
    pub last_unlock: Option<u64>,
    /// Unlock count
    pub unlock_count: u64,
    /// Failed attempts
    pub failed_attempts: u32,
    /// Locked out until (timestamp)
    pub locked_out_until: Option<u64>,
}

/// Configured authentication method
#[derive(Debug, Clone)]
pub struct ConfiguredMethod {
    /// Method type
    pub method: AuthMethod,
    /// Enrolled (has data)
    pub enrolled: bool,
    /// Last used
    pub last_used: Option<u64>,
    /// Use count
    pub use_count: u64,
    /// Method-specific data
    pub data: MethodData,
}

/// Method-specific data
#[derive(Debug, Clone)]
pub enum MethodData {
    /// TPM data
    Tpm {
        nv_index: u32,
        pcr_mask: u32,
    },
    /// Fingerprint
    Fingerprint {
        enrolled_count: u8,
        finger_ids: Vec<u8>,
    },
    /// Face recognition
    FaceRecognition {
        model_version: u32,
    },
    /// FIDO2 key
    Fido2 {
        credential_id: Vec<u8>,
        user_handle: Vec<u8>,
    },
    /// Smartcard
    Smartcard {
        certificate_hash: [u8; 32],
    },
    /// Bluetooth device
    Bluetooth {
        device_address: [u8; 6],
        device_name: String,
    },
    /// NFC
    Nfc {
        token_id: Vec<u8>,
    },
    /// No data
    None,
}

impl Default for MethodData {
    fn default() -> Self {
        Self::None
    }
}

/// Authentication attempt
#[derive(Debug)]
pub struct AuthAttempt {
    /// Target
    pub target: UnlockTarget,
    /// Identifier
    pub identifier: String,
    /// Methods tried
    pub methods_tried: Vec<AuthMethod>,
    /// Current method
    pub current_method: Option<AuthMethod>,
    /// Start time
    pub start_time: u64,
    /// User input (password, PIN)
    pub user_input: Option<String>,
    /// Challenge response
    pub challenge: Option<Vec<u8>>,
}

/// Authentication result
#[derive(Debug)]
pub enum AuthResult {
    /// Success
    Success {
        method: AuthMethod,
        duration_ms: u64,
    },
    /// Failed - wrong credentials
    Failed {
        method: AuthMethod,
        reason: AuthFailReason,
    },
    /// Cancelled by user
    Cancelled,
    /// Timeout
    Timeout,
    /// Locked out
    LockedOut {
        until: u64,
    },
    /// No methods available
    NoMethodsAvailable,
    /// Need user input
    NeedInput {
        method: AuthMethod,
        prompt: String,
    },
    /// Need hardware interaction
    NeedPresence {
        method: AuthMethod,
        instruction: String,
    },
}

/// Auth failure reason
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthFailReason {
    WrongPassword,
    WrongPin,
    BiometricNoMatch,
    HardwareError,
    TpmPcrMismatch,
    KeyNotFound,
    CertificateExpired,
    DeviceNotPresent,
    Timeout,
    PolicyViolation,
    Unknown,
}

impl AuthFailReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WrongPassword => "Wrong password",
            Self::WrongPin => "Wrong PIN",
            Self::BiometricNoMatch => "Biometric mismatch",
            Self::HardwareError => "Hardware error",
            Self::TpmPcrMismatch => "TPM PCR mismatch",
            Self::KeyNotFound => "Key not found",
            Self::CertificateExpired => "Certificate expired",
            Self::DeviceNotPresent => "Device not present",
            Self::Timeout => "Timeout",
            Self::PolicyViolation => "Policy violation",
            Self::Unknown => "Unknown error",
        }
    }
}

/// Biometric sensor state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiometricState {
    NotPresent,
    Initializing,
    Ready,
    Scanning,
    Processing,
    Error,
}

/// FIDO2 state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fido2State {
    NotPresent,
    WaitingForKey,
    KeyDetected,
    WaitingForTouch,
    Processing,
    Success,
    Error,
}

/// Auto unlock manager
pub struct AutoUnlockManager {
    /// Configured targets
    configs: Vec<AutoUnlockConfig>,
    /// Available methods (hardware detected)
    available_methods: Vec<AuthMethod>,
    /// TPM available
    tpm_available: bool,
    /// Fingerprint scanner present
    fingerprint_present: bool,
    /// Face recognition available
    face_recognition_available: bool,
    /// FIDO2 key connected
    fido2_connected: bool,
    /// Smartcard reader present
    smartcard_present: bool,
    /// Current authentication in progress
    current_auth: Option<AuthAttempt>,
    /// Max failed attempts before lockout
    max_failed_attempts: u32,
    /// Lockout duration (seconds)
    lockout_duration_secs: u32,
}

/// Statistics
pub struct AutoUnlockStats {
    unlock_attempts: AtomicU64,
    successful_unlocks: AtomicU64,
    failed_unlocks: AtomicU64,
    tpm_unlocks: AtomicU64,
    biometric_unlocks: AtomicU64,
    fido2_unlocks: AtomicU64,
    password_unlocks: AtomicU64,
    lockouts: AtomicU64,
}

impl AutoUnlockStats {
    const fn new() -> Self {
        Self {
            unlock_attempts: AtomicU64::new(0),
            successful_unlocks: AtomicU64::new(0),
            failed_unlocks: AtomicU64::new(0),
            tpm_unlocks: AtomicU64::new(0),
            biometric_unlocks: AtomicU64::new(0),
            fido2_unlocks: AtomicU64::new(0),
            password_unlocks: AtomicU64::new(0),
            lockouts: AtomicU64::new(0),
        }
    }
}

impl AutoUnlockManager {
    /// Create new manager
    pub fn new() -> Self {
        Self {
            configs: Vec::new(),
            available_methods: Vec::new(),
            tpm_available: false,
            fingerprint_present: false,
            face_recognition_available: false,
            fido2_connected: false,
            smartcard_present: false,
            current_auth: None,
            max_failed_attempts: 5,
            lockout_duration_secs: 300,
        }
    }

    /// Initialize
    pub fn init(&mut self) {
        kprintln!("auto-unlock: Initializing...");

        // Detect available hardware
        self.detect_hardware();

        // Build available methods list
        self.build_available_methods();

        // Load saved configurations
        self.load_configs();

        kprintln!("auto-unlock: Available methods: {:?}", self.available_methods);
    }

    /// Detect available hardware
    fn detect_hardware(&mut self) {
        // Check TPM
        self.tpm_available = self.detect_tpm();

        // Check fingerprint sensor
        self.fingerprint_present = self.detect_fingerprint();

        // Check face recognition
        self.face_recognition_available = self.detect_face_recognition();

        // Check FIDO2 keys
        self.fido2_connected = self.detect_fido2();

        // Check smartcard reader
        self.smartcard_present = self.detect_smartcard();

        kprintln!("auto-unlock: TPM={}, Fingerprint={}, Face={}, FIDO2={}, Smartcard={}",
            self.tpm_available, self.fingerprint_present, self.face_recognition_available,
            self.fido2_connected, self.smartcard_present);
    }

    fn detect_tpm(&self) -> bool {
        // Would check TPM presence via ACPI/MMIO
        true
    }

    fn detect_fingerprint(&self) -> bool {
        // Would check USB devices for fingerprint scanners
        false
    }

    fn detect_face_recognition(&self) -> bool {
        // Would check for IR camera
        false
    }

    fn detect_fido2(&self) -> bool {
        // Would check USB HID devices for FIDO2 keys
        false
    }

    fn detect_smartcard(&self) -> bool {
        // Would check CCID readers
        false
    }

    /// Build list of available methods
    fn build_available_methods(&mut self) {
        self.available_methods.clear();

        if self.tpm_available {
            self.available_methods.push(AuthMethod::Tpm2);
            self.available_methods.push(AuthMethod::Tpm2Pin);
        }

        if self.fingerprint_present {
            self.available_methods.push(AuthMethod::Fingerprint);
        }

        if self.face_recognition_available {
            self.available_methods.push(AuthMethod::FaceRecognition);
        }

        if self.fido2_connected {
            self.available_methods.push(AuthMethod::Fido2);
        }

        if self.smartcard_present {
            self.available_methods.push(AuthMethod::Smartcard);
        }

        // Password always available
        self.available_methods.push(AuthMethod::Password);
        self.available_methods.push(AuthMethod::RecoveryKey);
    }

    /// Load saved configurations
    fn load_configs(&mut self) {
        // Would load from /etc/auto-unlock.conf or similar
        kprintln!("auto-unlock: Loading configurations...");
    }

    /// Save configurations
    fn save_configs(&self) {
        // Would save to persistent storage
    }

    /// Configure auto-unlock for a target
    pub fn configure(
        &mut self,
        policy: UnlockPolicy,
        methods: Vec<ConfiguredMethod>,
    ) -> Result<(), &'static str> {
        kprintln!("auto-unlock: Configuring {} for {}",
            policy.target.as_str(), policy.identifier);

        // Validate policy
        self.validate_policy(&policy)?;

        // Create or update config
        if let Some(config) = self.configs.iter_mut()
            .find(|c| c.policy.target == policy.target && c.policy.identifier == policy.identifier)
        {
            config.policy = policy;
            config.configured_methods = methods;
        } else {
            self.configs.push(AutoUnlockConfig {
                policy,
                enabled: true,
                configured_methods: methods,
                last_unlock: None,
                unlock_count: 0,
                failed_attempts: 0,
                locked_out_until: None,
            });
        }

        self.save_configs();
        Ok(())
    }

    /// Validate policy
    fn validate_policy(&self, policy: &UnlockPolicy) -> Result<(), &'static str> {
        if policy.allowed_methods.is_empty() {
            return Err("No methods allowed");
        }

        // Check if at least one method is available
        let has_available = policy.allowed_methods.iter()
            .any(|m| self.available_methods.contains(m));

        if !has_available && !policy.allow_password_fallback {
            return Err("No available methods match policy");
        }

        Ok(())
    }

    /// Remove configuration
    pub fn unconfigure(&mut self, target: UnlockTarget, identifier: &str) -> bool {
        let before = self.configs.len();
        self.configs.retain(|c| !(c.policy.target == target && c.policy.identifier == identifier));
        let removed = before != self.configs.len();

        if removed {
            self.save_configs();
        }

        removed
    }

    /// Check if target is configured
    pub fn is_configured(&self, target: UnlockTarget, identifier: &str) -> bool {
        self.configs.iter()
            .any(|c| c.policy.target == target && c.policy.identifier == identifier)
    }

    /// Get configuration
    pub fn get_config(&self, target: UnlockTarget, identifier: &str) -> Option<&AutoUnlockConfig> {
        self.configs.iter()
            .find(|c| c.policy.target == target && c.policy.identifier == identifier)
    }

    /// Start authentication attempt
    pub fn start_auth(
        &mut self,
        target: UnlockTarget,
        identifier: &str,
    ) -> AuthResult {
        kprintln!("auto-unlock: Starting auth for {} {}", target.as_str(), identifier);
        STATS.unlock_attempts.fetch_add(1, Ordering::Relaxed);

        // Find config
        let config = match self.configs.iter().find(|c|
            c.policy.target == target && c.policy.identifier == identifier
        ) {
            Some(c) => c,
            None => return AuthResult::NoMethodsAvailable,
        };

        // Check if locked out
        let now = crate::time::uptime_ms();
        if let Some(until) = config.locked_out_until {
            if now < until {
                return AuthResult::LockedOut { until };
            }
        }

        // Check enabled
        if !config.enabled {
            return AuthResult::NoMethodsAvailable;
        }

        // Create attempt
        self.current_auth = Some(AuthAttempt {
            target,
            identifier: identifier.to_string(),
            methods_tried: Vec::new(),
            current_method: None,
            start_time: now,
            user_input: None,
            challenge: None,
        });

        // Try methods in order
        self.try_next_method()
    }

    /// Try next available method
    fn try_next_method(&mut self) -> AuthResult {
        // First gather info without mutable borrow
        let (target, identifier, methods_tried) = {
            let attempt = match &self.current_auth {
                Some(a) => a,
                None => return AuthResult::NoMethodsAvailable,
            };
            (attempt.target, attempt.identifier.clone(), attempt.methods_tried.clone())
        };

        let (next_method, allow_password_fallback) = {
            let config = match self.configs.iter().find(|c|
                c.policy.target == target && c.policy.identifier == identifier
            ) {
                Some(c) => c,
                None => return AuthResult::NoMethodsAvailable,
            };

            let mut found_method = None;

            // Find next untried method
            for method in &config.policy.allowed_methods {
                if methods_tried.contains(method) {
                    continue;
                }

                // Check if method is available
                if !self.available_methods.contains(method) {
                    continue;
                }

                // Check if method is configured
                let method_config = config.configured_methods.iter()
                    .find(|m| m.method == *method && m.enrolled);

                if method_config.is_none() && method.is_hardware() {
                    continue;
                }

                found_method = Some(*method);
                break;
            }

            (found_method, config.policy.allow_password_fallback)
        };

        // Now we can mutate
        if let Some(method) = next_method {
            if let Some(attempt) = &mut self.current_auth {
                attempt.current_method = Some(method);
            }
            return self.attempt_method_inner(method);
        }

        // No more methods - try password fallback
        if allow_password_fallback {
            return AuthResult::NeedInput {
                method: AuthMethod::Password,
                prompt: "Enter password:".to_string(),
            };
        }

        AuthResult::NoMethodsAvailable
    }

    /// Inner attempt method without policy reference
    fn attempt_method_inner(&mut self, method: AuthMethod) -> AuthResult {
        self.attempt_method(method, &UnlockPolicy::default())
    }

    /// Attempt authentication with specific method
    fn attempt_method(&mut self, method: AuthMethod, _policy: &UnlockPolicy) -> AuthResult {
        match method {
            AuthMethod::Tpm2 => self.attempt_tpm2(false),
            AuthMethod::Tpm2Pin => {
                AuthResult::NeedInput {
                    method: AuthMethod::Tpm2Pin,
                    prompt: "Enter TPM PIN:".to_string(),
                }
            }
            AuthMethod::Fingerprint => self.attempt_fingerprint(),
            AuthMethod::FaceRecognition => self.attempt_face(),
            AuthMethod::Fido2 => self.attempt_fido2(),
            AuthMethod::Smartcard => self.attempt_smartcard(),
            AuthMethod::Password => {
                AuthResult::NeedInput {
                    method: AuthMethod::Password,
                    prompt: "Enter password:".to_string(),
                }
            }
            AuthMethod::RecoveryKey => {
                AuthResult::NeedInput {
                    method: AuthMethod::RecoveryKey,
                    prompt: "Enter recovery key:".to_string(),
                }
            }
            AuthMethod::BluetoothProximity => self.attempt_bluetooth(),
            AuthMethod::Nfc => self.attempt_nfc(),
        }
    }

    /// Attempt TPM2 unlock
    fn attempt_tpm2(&mut self, _with_pin: bool) -> AuthResult {
        let start = crate::time::uptime_ms();

        // Would call TPM disk unlock
        // For now, simulate success
        let success = true;

        if success {
            STATS.tpm_unlocks.fetch_add(1, Ordering::Relaxed);
            AuthResult::Success {
                method: AuthMethod::Tpm2,
                duration_ms: crate::time::uptime_ms() - start,
            }
        } else {
            AuthResult::Failed {
                method: AuthMethod::Tpm2,
                reason: AuthFailReason::TpmPcrMismatch,
            }
        }
    }

    /// Attempt fingerprint
    fn attempt_fingerprint(&self) -> AuthResult {
        if !self.fingerprint_present {
            return AuthResult::Failed {
                method: AuthMethod::Fingerprint,
                reason: AuthFailReason::DeviceNotPresent,
            };
        }

        AuthResult::NeedPresence {
            method: AuthMethod::Fingerprint,
            instruction: "Place finger on sensor".to_string(),
        }
    }

    /// Attempt face recognition
    fn attempt_face(&self) -> AuthResult {
        if !self.face_recognition_available {
            return AuthResult::Failed {
                method: AuthMethod::FaceRecognition,
                reason: AuthFailReason::DeviceNotPresent,
            };
        }

        AuthResult::NeedPresence {
            method: AuthMethod::FaceRecognition,
            instruction: "Look at camera".to_string(),
        }
    }

    /// Attempt FIDO2
    fn attempt_fido2(&self) -> AuthResult {
        if !self.fido2_connected {
            return AuthResult::Failed {
                method: AuthMethod::Fido2,
                reason: AuthFailReason::DeviceNotPresent,
            };
        }

        AuthResult::NeedPresence {
            method: AuthMethod::Fido2,
            instruction: "Touch your security key".to_string(),
        }
    }

    /// Attempt smartcard
    fn attempt_smartcard(&self) -> AuthResult {
        if !self.smartcard_present {
            return AuthResult::Failed {
                method: AuthMethod::Smartcard,
                reason: AuthFailReason::DeviceNotPresent,
            };
        }

        AuthResult::NeedInput {
            method: AuthMethod::Smartcard,
            prompt: "Enter smartcard PIN:".to_string(),
        }
    }

    /// Attempt bluetooth proximity
    fn attempt_bluetooth(&self) -> AuthResult {
        // Would check for paired device in range
        AuthResult::Failed {
            method: AuthMethod::BluetoothProximity,
            reason: AuthFailReason::DeviceNotPresent,
        }
    }

    /// Attempt NFC
    fn attempt_nfc(&self) -> AuthResult {
        AuthResult::NeedPresence {
            method: AuthMethod::Nfc,
            instruction: "Tap NFC token".to_string(),
        }
    }

    /// Provide user input for current authentication
    pub fn provide_input(&mut self, input: &str) -> AuthResult {
        let attempt = match &mut self.current_auth {
            Some(a) => a,
            None => return AuthResult::NoMethodsAvailable,
        };

        let method = match attempt.current_method {
            Some(m) => m,
            None => return AuthResult::NoMethodsAvailable,
        };

        attempt.user_input = Some(input.to_string());
        attempt.methods_tried.push(method);

        let start = crate::time::uptime_ms();

        // Verify based on method
        let result = match method {
            AuthMethod::Password => self.verify_password(input),
            AuthMethod::Tpm2Pin => self.verify_tpm_pin(input),
            AuthMethod::RecoveryKey => self.verify_recovery_key(input),
            AuthMethod::Smartcard => self.verify_smartcard_pin(input),
            _ => false,
        };

        if result {
            self.on_auth_success(method, crate::time::uptime_ms() - start)
        } else {
            self.on_auth_failure(method, AuthFailReason::WrongPassword)
        }
    }

    /// Handle presence confirmation (fingerprint scanned, key touched, etc.)
    pub fn confirm_presence(&mut self, success: bool, data: Option<Vec<u8>>) -> AuthResult {
        let attempt = match &mut self.current_auth {
            Some(a) => a,
            None => return AuthResult::NoMethodsAvailable,
        };

        let method = match attempt.current_method {
            Some(m) => m,
            None => return AuthResult::NoMethodsAvailable,
        };

        attempt.methods_tried.push(method);

        if success {
            let start = attempt.start_time;
            // Would verify biometric/FIDO2 data
            let verified = data.is_some();

            if verified {
                self.on_auth_success(method, crate::time::uptime_ms() - start)
            } else {
                self.on_auth_failure(method, AuthFailReason::BiometricNoMatch)
            }
        } else {
            self.on_auth_failure(method, AuthFailReason::DeviceNotPresent)
        }
    }

    /// Handle successful authentication
    fn on_auth_success(&mut self, method: AuthMethod, duration_ms: u64) -> AuthResult {
        STATS.successful_unlocks.fetch_add(1, Ordering::Relaxed);

        // Update method-specific stats
        match method {
            AuthMethod::Tpm2 | AuthMethod::Tpm2Pin => {
                STATS.tpm_unlocks.fetch_add(1, Ordering::Relaxed);
            }
            AuthMethod::Fingerprint | AuthMethod::FaceRecognition => {
                STATS.biometric_unlocks.fetch_add(1, Ordering::Relaxed);
            }
            AuthMethod::Fido2 => {
                STATS.fido2_unlocks.fetch_add(1, Ordering::Relaxed);
            }
            AuthMethod::Password => {
                STATS.password_unlocks.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        // Update config
        if let Some(attempt) = &self.current_auth {
            if let Some(config) = self.configs.iter_mut().find(|c|
                c.policy.target == attempt.target && c.policy.identifier == attempt.identifier
            ) {
                config.last_unlock = Some(crate::time::uptime_ms());
                config.unlock_count += 1;
                config.failed_attempts = 0;
                config.locked_out_until = None;

                // Update method stats
                if let Some(m) = config.configured_methods.iter_mut()
                    .find(|m| m.method == method)
                {
                    m.last_used = Some(crate::time::uptime_ms());
                    m.use_count += 1;
                }
            }
        }

        self.current_auth = None;

        AuthResult::Success { method, duration_ms }
    }

    /// Handle failed authentication
    fn on_auth_failure(&mut self, method: AuthMethod, reason: AuthFailReason) -> AuthResult {
        STATS.failed_unlocks.fetch_add(1, Ordering::Relaxed);

        // Check for lockout
        if let Some(attempt) = &self.current_auth {
            if let Some(config) = self.configs.iter_mut().find(|c|
                c.policy.target == attempt.target && c.policy.identifier == attempt.identifier
            ) {
                config.failed_attempts += 1;

                if config.failed_attempts >= self.max_failed_attempts {
                    STATS.lockouts.fetch_add(1, Ordering::Relaxed);
                    let until = crate::time::uptime_ms() + (self.lockout_duration_secs as u64 * 1000);
                    config.locked_out_until = Some(until);
                    self.current_auth = None;
                    return AuthResult::LockedOut { until };
                }
            }
        }

        // Try next method
        if let Some(attempt) = &mut self.current_auth {
            attempt.current_method = None;
        }

        // If this was the only failure, try next
        let next = self.try_next_method();
        match next {
            AuthResult::NoMethodsAvailable => {
                self.current_auth = None;
                AuthResult::Failed { method, reason }
            }
            other => other,
        }
    }

    /// Cancel current authentication
    pub fn cancel_auth(&mut self) {
        self.current_auth = None;
    }

    /// Verify password
    fn verify_password(&self, _password: &str) -> bool {
        // Would verify against PAM or similar
        true
    }

    /// Verify TPM PIN
    fn verify_tpm_pin(&self, _pin: &str) -> bool {
        // Would verify with TPM
        true
    }

    /// Verify recovery key
    fn verify_recovery_key(&self, _key: &str) -> bool {
        // Would verify recovery key
        true
    }

    /// Verify smartcard PIN
    fn verify_smartcard_pin(&self, _pin: &str) -> bool {
        // Would verify with smartcard
        true
    }

    /// Auto-unlock all configured targets
    pub fn auto_unlock_all(&mut self) -> Vec<(UnlockTarget, String, AuthResult)> {
        let targets: Vec<(UnlockTarget, String)> = self.configs.iter()
            .filter(|c| c.enabled)
            .map(|c| (c.policy.target, c.policy.identifier.clone()))
            .collect();

        let mut results = Vec::new();

        for (target, identifier) in targets {
            let result = self.start_auth(target, &identifier);
            results.push((target, identifier, result));
        }

        results
    }

    /// Get available methods
    pub fn available_methods(&self) -> &[AuthMethod] {
        &self.available_methods
    }

    /// Check if method is available
    pub fn is_method_available(&self, method: AuthMethod) -> bool {
        self.available_methods.contains(&method)
    }

    /// Refresh hardware detection
    pub fn refresh_hardware(&mut self) {
        self.detect_hardware();
        self.build_available_methods();
    }

    /// Format status
    pub fn format_status(&self) -> String {
        use alloc::fmt::Write;
        let mut s = String::new();

        let _ = writeln!(s, "Auto Unlock Status:");
        let _ = writeln!(s, "  Available Methods:");
        for method in &self.available_methods {
            let _ = writeln!(s, "    - {}", method.as_str());
        }

        let _ = writeln!(s, "  Configured Targets: {}", self.configs.len());
        for config in &self.configs {
            let _ = writeln!(s, "    {} {} (enabled={}, unlocks={})",
                config.policy.target.as_str(),
                config.policy.identifier,
                config.enabled,
                config.unlock_count);
        }

        s
    }
}

impl Default for AutoUnlockManager {
    fn default() -> Self {
        Self::new()
    }
}

// === Public API ===

/// Initialize auto unlock
pub fn init() {
    let mut guard = AUTO_UNLOCK.lock();
    if guard.is_none() {
        let mut manager = AutoUnlockManager::new();
        manager.init();
        *guard = Some(manager);
    }
}

/// Configure auto-unlock
pub fn configure(
    policy: UnlockPolicy,
    methods: Vec<ConfiguredMethod>,
) -> Result<(), &'static str> {
    AUTO_UNLOCK.lock().as_mut()
        .expect("Not initialized")
        .configure(policy, methods)
}

/// Start authentication
pub fn start_auth(target: UnlockTarget, identifier: &str) -> AuthResult {
    AUTO_UNLOCK.lock().as_mut()
        .expect("Not initialized")
        .start_auth(target, identifier)
}

/// Provide user input
pub fn provide_input(input: &str) -> AuthResult {
    AUTO_UNLOCK.lock().as_mut()
        .expect("Not initialized")
        .provide_input(input)
}

/// Confirm presence
pub fn confirm_presence(success: bool, data: Option<Vec<u8>>) -> AuthResult {
    AUTO_UNLOCK.lock().as_mut()
        .expect("Not initialized")
        .confirm_presence(success, data)
}

/// Cancel authentication
pub fn cancel_auth() {
    AUTO_UNLOCK.lock().as_mut()
        .expect("Not initialized")
        .cancel_auth()
}

/// Auto-unlock all
pub fn auto_unlock_all() -> Vec<(UnlockTarget, String, AuthResult)> {
    AUTO_UNLOCK.lock().as_mut()
        .expect("Not initialized")
        .auto_unlock_all()
}

/// Get available methods
pub fn available_methods() -> Vec<AuthMethod> {
    AUTO_UNLOCK.lock().as_ref()
        .expect("Not initialized")
        .available_methods()
        .to_vec()
}

/// Check if configured
pub fn is_configured(target: UnlockTarget, identifier: &str) -> bool {
    AUTO_UNLOCK.lock().as_ref()
        .expect("Not initialized")
        .is_configured(target, identifier)
}

/// Get statistics
pub fn stats() -> (u64, u64, u64) {
    (
        STATS.unlock_attempts.load(Ordering::Relaxed),
        STATS.successful_unlocks.load(Ordering::Relaxed),
        STATS.failed_unlocks.load(Ordering::Relaxed),
    )
}

/// Format status
pub fn format_status() -> String {
    AUTO_UNLOCK.lock().as_ref()
        .map(|m| m.format_status())
        .unwrap_or_else(|| "Auto Unlock: Not initialized".to_string())
}
