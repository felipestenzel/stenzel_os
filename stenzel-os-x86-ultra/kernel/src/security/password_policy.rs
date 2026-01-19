//! Password Policy Management
//!
//! Implements password complexity requirements, expiration,
//! history tracking, and lockout policies.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::kprintln;

/// Password policy manager state
static PASSWORD_POLICY: IrqSafeMutex<Option<PasswordPolicyManager>> = IrqSafeMutex::new(None);

/// Statistics
static STATS: PasswordPolicyStats = PasswordPolicyStats::new();

/// Character classes for password complexity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharClass {
    Lowercase,
    Uppercase,
    Digit,
    Special,
}

/// Password strength level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PasswordStrength {
    VeryWeak = 0,
    Weak = 1,
    Fair = 2,
    Strong = 3,
    VeryStrong = 4,
}

impl PasswordStrength {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VeryWeak => "Very Weak",
            Self::Weak => "Weak",
            Self::Fair => "Fair",
            Self::Strong => "Strong",
            Self::VeryStrong => "Very Strong",
        }
    }

    pub fn score(&self) -> u32 {
        *self as u32
    }
}

/// Password policy configuration
#[derive(Debug, Clone)]
pub struct PasswordPolicy {
    /// Minimum length
    pub min_length: usize,
    /// Maximum length (0 = no limit)
    pub max_length: usize,
    /// Minimum uppercase letters
    pub min_uppercase: usize,
    /// Minimum lowercase letters
    pub min_lowercase: usize,
    /// Minimum digits
    pub min_digits: usize,
    /// Minimum special characters
    pub min_special: usize,
    /// Minimum unique characters
    pub min_unique: usize,
    /// Minimum character classes required
    pub min_char_classes: usize,
    /// Disallow username in password
    pub no_username: bool,
    /// Disallow common passwords
    pub no_common: bool,
    /// Disallow dictionary words
    pub no_dictionary: bool,
    /// Disallow sequential characters (abc, 123)
    pub no_sequential: bool,
    /// Disallow repeated characters (aaa, 111)
    pub no_repeated: bool,
    /// Maximum consecutive same characters
    pub max_consecutive: usize,
    /// Password expiration (days, 0 = never)
    pub max_age_days: u32,
    /// Minimum password age (days)
    pub min_age_days: u32,
    /// Password history count (prevent reuse)
    pub history_count: usize,
    /// Grace period after expiration (days)
    pub grace_period_days: u32,
    /// Warning before expiration (days)
    pub warn_days: u32,
    /// Enable zxcvbn-style strength check
    pub zxcvbn_enabled: bool,
    /// Minimum zxcvbn score (0-4)
    pub min_zxcvbn_score: u32,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            min_length: 8,
            max_length: 128,
            min_uppercase: 1,
            min_lowercase: 1,
            min_digits: 1,
            min_special: 0,
            min_unique: 4,
            min_char_classes: 3,
            no_username: true,
            no_common: true,
            no_dictionary: false,
            no_sequential: true,
            no_repeated: true,
            max_consecutive: 3,
            max_age_days: 90,
            min_age_days: 1,
            history_count: 5,
            grace_period_days: 7,
            warn_days: 14,
            zxcvbn_enabled: true,
            min_zxcvbn_score: 2,
        }
    }
}

impl PasswordPolicy {
    /// Create a minimal policy (for testing/weak systems)
    pub fn minimal() -> Self {
        Self {
            min_length: 4,
            max_length: 0,
            min_uppercase: 0,
            min_lowercase: 0,
            min_digits: 0,
            min_special: 0,
            min_unique: 0,
            min_char_classes: 0,
            no_username: false,
            no_common: false,
            no_dictionary: false,
            no_sequential: false,
            no_repeated: false,
            max_consecutive: 0,
            max_age_days: 0,
            min_age_days: 0,
            history_count: 0,
            grace_period_days: 0,
            warn_days: 0,
            zxcvbn_enabled: false,
            min_zxcvbn_score: 0,
        }
    }

    /// Create a strict policy (for high security systems)
    pub fn strict() -> Self {
        Self {
            min_length: 12,
            max_length: 128,
            min_uppercase: 2,
            min_lowercase: 2,
            min_digits: 2,
            min_special: 2,
            min_unique: 8,
            min_char_classes: 4,
            no_username: true,
            no_common: true,
            no_dictionary: true,
            no_sequential: true,
            no_repeated: true,
            max_consecutive: 2,
            max_age_days: 60,
            min_age_days: 1,
            history_count: 12,
            grace_period_days: 3,
            warn_days: 14,
            zxcvbn_enabled: true,
            min_zxcvbn_score: 3,
        }
    }
}

/// Lockout policy
#[derive(Debug, Clone)]
pub struct LockoutPolicy {
    /// Maximum failed attempts before lockout
    pub max_attempts: u32,
    /// Lockout duration (seconds)
    pub lockout_duration: u32,
    /// Reset attempt counter after (seconds)
    pub reset_counter_after: u32,
    /// Unlock requires admin
    pub admin_unlock_required: bool,
    /// Notify admin on lockout
    pub notify_admin: bool,
    /// Progressive lockout (increasing duration)
    pub progressive: bool,
    /// Progressive multiplier
    pub progressive_multiplier: u32,
    /// Maximum progressive lockout (seconds)
    pub max_lockout_duration: u32,
}

impl Default for LockoutPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            lockout_duration: 300, // 5 minutes
            reset_counter_after: 900, // 15 minutes
            admin_unlock_required: false,
            notify_admin: true,
            progressive: true,
            progressive_multiplier: 2,
            max_lockout_duration: 86400, // 24 hours
        }
    }
}

/// Password validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
    pub strength: PasswordStrength,
    pub score: u32,
    pub entropy_bits: f32,
    pub crack_time_seconds: u64,
    pub suggestions: Vec<String>,
}

/// Password validation error
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    TooShort { min: usize, actual: usize },
    TooLong { max: usize, actual: usize },
    InsufficientUppercase { min: usize, actual: usize },
    InsufficientLowercase { min: usize, actual: usize },
    InsufficientDigits { min: usize, actual: usize },
    InsufficientSpecial { min: usize, actual: usize },
    InsufficientUnique { min: usize, actual: usize },
    InsufficientCharClasses { min: usize, actual: usize },
    ContainsUsername,
    CommonPassword,
    DictionaryWord,
    SequentialCharacters,
    RepeatedCharacters { count: usize },
    TooWeak { min_score: u32, actual_score: u32 },
    RecentlyUsed,
    TooRecent { days: u32 },
}

impl ValidationError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TooShort { .. } => "Password too short",
            Self::TooLong { .. } => "Password too long",
            Self::InsufficientUppercase { .. } => "Not enough uppercase letters",
            Self::InsufficientLowercase { .. } => "Not enough lowercase letters",
            Self::InsufficientDigits { .. } => "Not enough digits",
            Self::InsufficientSpecial { .. } => "Not enough special characters",
            Self::InsufficientUnique { .. } => "Not enough unique characters",
            Self::InsufficientCharClasses { .. } => "Not enough character types",
            Self::ContainsUsername => "Password contains username",
            Self::CommonPassword => "Password is too common",
            Self::DictionaryWord => "Password is a dictionary word",
            Self::SequentialCharacters => "Password contains sequential characters",
            Self::RepeatedCharacters { .. } => "Password contains repeated characters",
            Self::TooWeak { .. } => "Password is too weak",
            Self::RecentlyUsed => "Password was recently used",
            Self::TooRecent { .. } => "Password changed too recently",
        }
    }
}

/// User password history
#[derive(Debug, Clone)]
pub struct PasswordHistory {
    /// User ID
    pub uid: u32,
    /// Previous password hashes
    pub hashes: Vec<[u8; 32]>,
    /// Password change timestamps
    pub change_times: Vec<u64>,
    /// Current password hash
    pub current_hash: [u8; 32],
    /// Last change time
    pub last_change: u64,
    /// Failed attempt count
    pub failed_attempts: u32,
    /// Last failed attempt time
    pub last_failed: Option<u64>,
    /// Lockout until (timestamp)
    pub locked_until: Option<u64>,
    /// Lockout count (for progressive)
    pub lockout_count: u32,
}

impl PasswordHistory {
    fn new(uid: u32, password_hash: [u8; 32]) -> Self {
        Self {
            uid,
            hashes: Vec::new(),
            change_times: Vec::new(),
            current_hash: password_hash,
            last_change: crate::time::uptime_ms() / 1000,
            failed_attempts: 0,
            last_failed: None,
            locked_until: None,
            lockout_count: 0,
        }
    }
}

/// Password policy error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyError {
    NotInitialized,
    UserNotFound,
    Locked,
    ValidationFailed,
    TooRecent,
    InternalError,
}

pub type PolicyResult<T> = Result<T, PolicyError>;

/// Statistics
pub struct PasswordPolicyStats {
    validations: AtomicU64,
    validation_failures: AtomicU64,
    password_changes: AtomicU64,
    lockouts: AtomicU64,
    failed_logins: AtomicU64,
    expired_passwords: AtomicU64,
}

impl PasswordPolicyStats {
    const fn new() -> Self {
        Self {
            validations: AtomicU64::new(0),
            validation_failures: AtomicU64::new(0),
            password_changes: AtomicU64::new(0),
            lockouts: AtomicU64::new(0),
            failed_logins: AtomicU64::new(0),
            expired_passwords: AtomicU64::new(0),
        }
    }
}

/// Common passwords list (abbreviated)
const COMMON_PASSWORDS: &[&str] = &[
    "123456", "password", "12345678", "qwerty", "123456789",
    "12345", "1234", "111111", "1234567", "dragon",
    "123123", "baseball", "iloveyou", "trustno1", "sunshine",
    "master", "welcome", "shadow", "ashley", "football",
    "jesus", "michael", "ninja", "mustang", "password1",
    "admin", "letmein", "monkey", "abc123", "passw0rd",
    "password123", "secret", "login", "qwerty123", "starwars",
];

/// Password Policy Manager
pub struct PasswordPolicyManager {
    /// Default password policy
    policy: PasswordPolicy,
    /// Per-user policies (override default)
    user_policies: BTreeMap<u32, PasswordPolicy>,
    /// Lockout policy
    lockout_policy: LockoutPolicy,
    /// User password histories
    histories: BTreeMap<u32, PasswordHistory>,
    /// Enable password expiration checks
    expiration_enabled: bool,
    /// Enable lockout
    lockout_enabled: bool,
}

impl PasswordPolicyManager {
    fn new() -> Self {
        Self {
            policy: PasswordPolicy::default(),
            user_policies: BTreeMap::new(),
            lockout_policy: LockoutPolicy::default(),
            histories: BTreeMap::new(),
            expiration_enabled: true,
            lockout_enabled: true,
        }
    }

    /// Set default password policy
    pub fn set_policy(&mut self, policy: PasswordPolicy) {
        self.policy = policy;
        kprintln!("password-policy: Updated default policy");
    }

    /// Set policy for specific user
    pub fn set_user_policy(&mut self, uid: u32, policy: PasswordPolicy) {
        self.user_policies.insert(uid, policy);
        kprintln!("password-policy: Set policy for user {}", uid);
    }

    /// Get policy for user (or default)
    pub fn get_policy(&self, uid: u32) -> &PasswordPolicy {
        self.user_policies.get(&uid).unwrap_or(&self.policy)
    }

    /// Set lockout policy
    pub fn set_lockout_policy(&mut self, policy: LockoutPolicy) {
        self.lockout_policy = policy;
    }

    /// Validate password against policy
    pub fn validate(
        &self,
        password: &str,
        uid: u32,
        username: Option<&str>,
    ) -> ValidationResult {
        STATS.validations.fetch_add(1, Ordering::Relaxed);

        let policy = self.get_policy(uid);
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut suggestions = Vec::new();

        // Length checks
        if password.len() < policy.min_length {
            errors.push(ValidationError::TooShort {
                min: policy.min_length,
                actual: password.len(),
            });
            suggestions.push(alloc::format!(
                "Use at least {} characters",
                policy.min_length
            ));
        }

        if policy.max_length > 0 && password.len() > policy.max_length {
            errors.push(ValidationError::TooLong {
                max: policy.max_length,
                actual: password.len(),
            });
        }

        // Character class counts
        let uppercase_count = password.chars().filter(|c| c.is_uppercase()).count();
        let lowercase_count = password.chars().filter(|c| c.is_lowercase()).count();
        let digit_count = password.chars().filter(|c| c.is_ascii_digit()).count();
        let special_count = password
            .chars()
            .filter(|c| !c.is_alphanumeric() && !c.is_whitespace())
            .count();

        if uppercase_count < policy.min_uppercase {
            errors.push(ValidationError::InsufficientUppercase {
                min: policy.min_uppercase,
                actual: uppercase_count,
            });
            suggestions.push("Add more uppercase letters".to_string());
        }

        if lowercase_count < policy.min_lowercase {
            errors.push(ValidationError::InsufficientLowercase {
                min: policy.min_lowercase,
                actual: lowercase_count,
            });
            suggestions.push("Add more lowercase letters".to_string());
        }

        if digit_count < policy.min_digits {
            errors.push(ValidationError::InsufficientDigits {
                min: policy.min_digits,
                actual: digit_count,
            });
            suggestions.push("Add more numbers".to_string());
        }

        if special_count < policy.min_special {
            errors.push(ValidationError::InsufficientSpecial {
                min: policy.min_special,
                actual: special_count,
            });
            suggestions.push("Add special characters (!@#$%...)".to_string());
        }

        // Unique characters
        let unique_chars = self.count_unique_chars(password);
        if unique_chars < policy.min_unique {
            errors.push(ValidationError::InsufficientUnique {
                min: policy.min_unique,
                actual: unique_chars,
            });
        }

        // Character classes
        let mut char_classes = 0;
        if uppercase_count > 0 { char_classes += 1; }
        if lowercase_count > 0 { char_classes += 1; }
        if digit_count > 0 { char_classes += 1; }
        if special_count > 0 { char_classes += 1; }

        if char_classes < policy.min_char_classes {
            errors.push(ValidationError::InsufficientCharClasses {
                min: policy.min_char_classes,
                actual: char_classes,
            });
        }

        // Username check
        if policy.no_username {
            if let Some(uname) = username {
                if !uname.is_empty() && password.to_lowercase().contains(&uname.to_lowercase()) {
                    errors.push(ValidationError::ContainsUsername);
                    suggestions.push("Don't include your username in the password".to_string());
                }
            }
        }

        // Common passwords
        if policy.no_common {
            let lower = password.to_lowercase();
            if COMMON_PASSWORDS.iter().any(|&p| p == lower) {
                errors.push(ValidationError::CommonPassword);
                suggestions.push("Choose a less common password".to_string());
            }
        }

        // Sequential characters
        if policy.no_sequential && self.has_sequential(password) {
            errors.push(ValidationError::SequentialCharacters);
            suggestions.push("Avoid sequential characters (abc, 123)".to_string());
        }

        // Repeated characters
        if policy.no_repeated {
            let max_repeat = self.max_consecutive_chars(password);
            if policy.max_consecutive > 0 && max_repeat > policy.max_consecutive {
                errors.push(ValidationError::RepeatedCharacters { count: max_repeat });
                suggestions.push("Avoid repeating characters".to_string());
            }
        }

        // Check password history
        if let Some(history) = self.histories.get(&uid) {
            if policy.history_count > 0 {
                let pw_hash = self.hash_password(password);
                // Check current password
                if pw_hash == history.current_hash {
                    errors.push(ValidationError::RecentlyUsed);
                }
                // Check history
                for hash in history.hashes.iter().rev().take(policy.history_count) {
                    if *hash == pw_hash {
                        errors.push(ValidationError::RecentlyUsed);
                        break;
                    }
                }
            }

            // Check minimum age
            if policy.min_age_days > 0 {
                let now = crate::time::uptime_ms() / 1000;
                let days_since_change = (now - history.last_change) / 86400;
                if days_since_change < policy.min_age_days as u64 {
                    errors.push(ValidationError::TooRecent {
                        days: (policy.min_age_days as u64 - days_since_change) as u32,
                    });
                }
            }
        }

        // Calculate strength
        let (strength, score, entropy) = self.calculate_strength(password);

        // zxcvbn check
        if policy.zxcvbn_enabled && score < policy.min_zxcvbn_score {
            errors.push(ValidationError::TooWeak {
                min_score: policy.min_zxcvbn_score,
                actual_score: score,
            });
        }

        // Crack time estimation
        let crack_time = self.estimate_crack_time(entropy);

        // Add warnings
        if crack_time < 3600 {
            warnings.push("Password could be cracked in under an hour".to_string());
        } else if crack_time < 86400 {
            warnings.push("Password could be cracked in under a day".to_string());
        }

        let valid = errors.is_empty();
        if !valid {
            STATS.validation_failures.fetch_add(1, Ordering::Relaxed);
        }

        ValidationResult {
            valid,
            errors,
            warnings,
            strength,
            score,
            entropy_bits: entropy,
            crack_time_seconds: crack_time,
            suggestions,
        }
    }

    /// Change password for user
    pub fn change_password(
        &mut self,
        uid: u32,
        old_password: &str,
        new_password: &str,
        username: Option<&str>,
    ) -> PolicyResult<ValidationResult> {
        kprintln!("password-policy: Changing password for uid {}", uid);

        // Verify old password (if exists)
        if let Some(history) = self.histories.get(&uid) {
            let old_hash = self.hash_password(old_password);
            if old_hash != history.current_hash {
                return Err(PolicyError::ValidationFailed);
            }
        }

        // Validate new password
        let result = self.validate(new_password, uid, username);
        if !result.valid {
            return Ok(result);
        }

        // Update history
        let new_hash = self.hash_password(new_password);
        let policy = self.get_policy(uid);
        let history_count = policy.history_count;

        if let Some(history) = self.histories.get_mut(&uid) {
            // Save current to history
            history.hashes.push(history.current_hash);
            history.change_times.push(history.last_change);

            // Trim history
            while history.hashes.len() > history_count {
                history.hashes.remove(0);
                history.change_times.remove(0);
            }

            // Update current
            history.current_hash = new_hash;
            history.last_change = crate::time::uptime_ms() / 1000;
        } else {
            // New user
            self.histories.insert(uid, PasswordHistory::new(uid, new_hash));
        }

        STATS.password_changes.fetch_add(1, Ordering::Relaxed);
        kprintln!("password-policy: Password changed for uid {}", uid);

        Ok(result)
    }

    /// Set initial password (no old password required)
    pub fn set_password(
        &mut self,
        uid: u32,
        password: &str,
        username: Option<&str>,
    ) -> PolicyResult<ValidationResult> {
        let result = self.validate(password, uid, username);
        if !result.valid {
            return Ok(result);
        }

        let hash = self.hash_password(password);
        self.histories.insert(uid, PasswordHistory::new(uid, hash));

        STATS.password_changes.fetch_add(1, Ordering::Relaxed);
        Ok(result)
    }

    /// Check password expiration
    pub fn check_expiration(&self, uid: u32) -> Option<PasswordExpiration> {
        if !self.expiration_enabled {
            return None;
        }

        let policy = self.get_policy(uid);
        if policy.max_age_days == 0 {
            return None;
        }

        let history = self.histories.get(&uid)?;
        let now = crate::time::uptime_ms() / 1000;
        let age_days = ((now - history.last_change) / 86400) as u32;

        let status = if age_days >= policy.max_age_days + policy.grace_period_days {
            PasswordExpirationStatus::Locked
        } else if age_days >= policy.max_age_days {
            PasswordExpirationStatus::Grace
        } else if age_days >= policy.max_age_days - policy.warn_days {
            PasswordExpirationStatus::Warning
        } else {
            PasswordExpirationStatus::Valid
        };

        Some(PasswordExpiration {
            uid,
            status,
            days_remaining: policy.max_age_days.saturating_sub(age_days),
            last_change: history.last_change,
            expires_at: history.last_change + (policy.max_age_days as u64 * 86400),
            grace_until: if status == PasswordExpirationStatus::Grace {
                Some(history.last_change + ((policy.max_age_days + policy.grace_period_days) as u64 * 86400))
            } else {
                None
            },
        })
    }

    /// Record failed login attempt
    pub fn record_failed_login(&mut self, uid: u32) -> LockoutStatus {
        STATS.failed_logins.fetch_add(1, Ordering::Relaxed);

        let history = self.histories.entry(uid).or_insert_with(|| {
            PasswordHistory::new(uid, [0u8; 32])
        });

        let now = crate::time::uptime_ms() / 1000;

        // Check if counter should reset
        if let Some(last) = history.last_failed {
            if now - last > self.lockout_policy.reset_counter_after as u64 {
                history.failed_attempts = 0;
            }
        }

        history.failed_attempts += 1;
        history.last_failed = Some(now);

        // Check lockout
        if self.lockout_enabled && history.failed_attempts >= self.lockout_policy.max_attempts {
            let duration = if self.lockout_policy.progressive {
                let mult = self.lockout_policy.progressive_multiplier.pow(history.lockout_count);
                let dur = self.lockout_policy.lockout_duration * mult;
                dur.min(self.lockout_policy.max_lockout_duration)
            } else {
                self.lockout_policy.lockout_duration
            };

            history.locked_until = Some(now + duration as u64);
            history.lockout_count += 1;
            STATS.lockouts.fetch_add(1, Ordering::Relaxed);

            kprintln!(
                "password-policy: User {} locked out for {} seconds",
                uid,
                duration
            );

            return LockoutStatus::Locked {
                until: now + duration as u64,
                remaining: duration as u64,
                attempts: history.failed_attempts,
            };
        }

        LockoutStatus::Warning {
            attempts: history.failed_attempts,
            max_attempts: self.lockout_policy.max_attempts,
        }
    }

    /// Record successful login
    pub fn record_successful_login(&mut self, uid: u32) {
        if let Some(history) = self.histories.get_mut(&uid) {
            history.failed_attempts = 0;
            history.last_failed = None;
            // Don't reset lockout_count - that's for progressive lockout
        }
    }

    /// Check if user is locked out
    pub fn is_locked(&self, uid: u32) -> Option<LockoutStatus> {
        let history = self.histories.get(&uid)?;
        let locked_until = history.locked_until?;
        let now = crate::time::uptime_ms() / 1000;

        if now < locked_until {
            Some(LockoutStatus::Locked {
                until: locked_until,
                remaining: locked_until - now,
                attempts: history.failed_attempts,
            })
        } else {
            None
        }
    }

    /// Unlock user
    pub fn unlock(&mut self, uid: u32) -> PolicyResult<()> {
        let history = self.histories.get_mut(&uid)
            .ok_or(PolicyError::UserNotFound)?;

        history.locked_until = None;
        history.failed_attempts = 0;
        history.lockout_count = 0;

        kprintln!("password-policy: User {} unlocked", uid);
        Ok(())
    }

    /// Force password change on next login
    pub fn force_change(&mut self, uid: u32) -> PolicyResult<()> {
        let history = self.histories.get_mut(&uid)
            .ok_or(PolicyError::UserNotFound)?;

        // Set last change to very old time to trigger expiration
        history.last_change = 0;

        kprintln!("password-policy: Forced password change for uid {}", uid);
        Ok(())
    }

    /// Get password strength
    pub fn get_strength(&self, password: &str) -> PasswordStrength {
        let (strength, _, _) = self.calculate_strength(password);
        strength
    }

    /// Get statistics
    pub fn get_stats(&self) -> (u64, u64, u64, u64, u64) {
        (
            STATS.validations.load(Ordering::Relaxed),
            STATS.validation_failures.load(Ordering::Relaxed),
            STATS.password_changes.load(Ordering::Relaxed),
            STATS.lockouts.load(Ordering::Relaxed),
            STATS.failed_logins.load(Ordering::Relaxed),
        )
    }

    // Internal helpers

    fn count_unique_chars(&self, password: &str) -> usize {
        let mut chars: Vec<char> = password.chars().collect();
        chars.sort();
        chars.dedup();
        chars.len()
    }

    fn has_sequential(&self, password: &str) -> bool {
        let chars: Vec<char> = password.chars().collect();
        if chars.len() < 3 {
            return false;
        }

        for window in chars.windows(3) {
            let a = window[0] as i32;
            let b = window[1] as i32;
            let c = window[2] as i32;

            // Check ascending (abc, 123)
            if b - a == 1 && c - b == 1 {
                return true;
            }
            // Check descending (cba, 321)
            if a - b == 1 && b - c == 1 {
                return true;
            }
        }
        false
    }

    fn max_consecutive_chars(&self, password: &str) -> usize {
        let mut max = 1;
        let mut current = 1;
        let mut prev: Option<char> = None;

        for c in password.chars() {
            if Some(c) == prev {
                current += 1;
                max = max.max(current);
            } else {
                current = 1;
            }
            prev = Some(c);
        }
        max
    }

    fn hash_password(&self, password: &str) -> [u8; 32] {
        // Placeholder - would use proper password hashing (Argon2, bcrypt, etc.)
        let mut hash = [0u8; 32];
        for (i, byte) in password.bytes().enumerate() {
            hash[i % 32] ^= byte;
        }
        hash
    }

    fn calculate_strength(&self, password: &str) -> (PasswordStrength, u32, f32) {
        // Simple entropy calculation using integer approximation
        let len = password.len();
        let charset_size = self.estimate_charset_size(password);
        // log2 approximation: log2(n) ≈ number of bits needed
        let log2_charset = self.approx_log2(charset_size as u32);
        let entropy = (len as f32) * log2_charset;

        let strength = match entropy as u32 {
            0..=28 => PasswordStrength::VeryWeak,
            29..=35 => PasswordStrength::Weak,
            36..=59 => PasswordStrength::Fair,
            60..=127 => PasswordStrength::Strong,
            _ => PasswordStrength::VeryStrong,
        };

        let score = strength.score();
        (strength, score, entropy)
    }

    fn estimate_charset_size(&self, password: &str) -> usize {
        let mut size = 0;

        if password.chars().any(|c| c.is_lowercase()) {
            size += 26;
        }
        if password.chars().any(|c| c.is_uppercase()) {
            size += 26;
        }
        if password.chars().any(|c| c.is_ascii_digit()) {
            size += 10;
        }
        if password.chars().any(|c| !c.is_alphanumeric() && !c.is_whitespace()) {
            size += 32;
        }

        size.max(10)
    }

    fn approx_log2(&self, n: u32) -> f32 {
        // Simple log2 approximation using leading zeros
        if n == 0 {
            return 0.0;
        }
        let bits = 32 - n.leading_zeros();
        bits as f32
    }

    fn estimate_crack_time(&self, entropy: f32) -> u64 {
        // Assume 10 billion guesses per second (modern GPU)
        // Use integer math: 2^entropy / 10^10
        // For large entropy, this will overflow, so cap it
        if entropy > 60.0 {
            return u64::MAX;
        }
        let entropy_int = entropy as u32;
        if entropy_int > 33 {
            // 2^33 / 10^10 ≈ 0.86, so anything > 33 bits takes > 1 second
            // Approximate: 2^entropy / 10^10 ≈ 2^(entropy-33) seconds
            let shift = entropy_int.saturating_sub(33);
            1u64.checked_shl(shift).unwrap_or(u64::MAX)
        } else {
            // Less than 33 bits is cracked in < 1 second
            0
        }
    }
}

/// Password expiration info
#[derive(Debug, Clone)]
pub struct PasswordExpiration {
    pub uid: u32,
    pub status: PasswordExpirationStatus,
    pub days_remaining: u32,
    pub last_change: u64,
    pub expires_at: u64,
    pub grace_until: Option<u64>,
}

/// Password expiration status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordExpirationStatus {
    Valid,
    Warning,
    Grace,
    Locked,
}

/// Lockout status
#[derive(Debug, Clone)]
pub enum LockoutStatus {
    Warning {
        attempts: u32,
        max_attempts: u32,
    },
    Locked {
        until: u64,
        remaining: u64,
        attempts: u32,
    },
}

// Public API

/// Initialize password policy subsystem
pub fn init() {
    let mut guard = PASSWORD_POLICY.lock();
    if guard.is_none() {
        *guard = Some(PasswordPolicyManager::new());
        kprintln!("password-policy: Initialized");
    }
}

/// Set default policy
pub fn set_policy(policy: PasswordPolicy) {
    let mut guard = PASSWORD_POLICY.lock();
    if let Some(manager) = guard.as_mut() {
        manager.set_policy(policy);
    }
}

/// Set user-specific policy
pub fn set_user_policy(uid: u32, policy: PasswordPolicy) {
    let mut guard = PASSWORD_POLICY.lock();
    if let Some(manager) = guard.as_mut() {
        manager.set_user_policy(uid, policy);
    }
}

/// Set lockout policy
pub fn set_lockout_policy(policy: LockoutPolicy) {
    let mut guard = PASSWORD_POLICY.lock();
    if let Some(manager) = guard.as_mut() {
        manager.set_lockout_policy(policy);
    }
}

/// Validate password
pub fn validate(password: &str, uid: u32, username: Option<&str>) -> Option<ValidationResult> {
    let guard = PASSWORD_POLICY.lock();
    guard.as_ref().map(|m| m.validate(password, uid, username))
}

/// Change password
pub fn change_password(
    uid: u32,
    old_password: &str,
    new_password: &str,
    username: Option<&str>,
) -> PolicyResult<ValidationResult> {
    let mut guard = PASSWORD_POLICY.lock();
    let manager = guard.as_mut().ok_or(PolicyError::NotInitialized)?;
    manager.change_password(uid, old_password, new_password, username)
}

/// Set initial password
pub fn set_password(
    uid: u32,
    password: &str,
    username: Option<&str>,
) -> PolicyResult<ValidationResult> {
    let mut guard = PASSWORD_POLICY.lock();
    let manager = guard.as_mut().ok_or(PolicyError::NotInitialized)?;
    manager.set_password(uid, password, username)
}

/// Check expiration
pub fn check_expiration(uid: u32) -> Option<PasswordExpiration> {
    let guard = PASSWORD_POLICY.lock();
    guard.as_ref().and_then(|m| m.check_expiration(uid))
}

/// Record failed login
pub fn record_failed_login(uid: u32) -> Option<LockoutStatus> {
    let mut guard = PASSWORD_POLICY.lock();
    guard.as_mut().map(|m| m.record_failed_login(uid))
}

/// Record successful login
pub fn record_successful_login(uid: u32) {
    let mut guard = PASSWORD_POLICY.lock();
    if let Some(manager) = guard.as_mut() {
        manager.record_successful_login(uid);
    }
}

/// Check if locked
pub fn is_locked(uid: u32) -> Option<LockoutStatus> {
    let guard = PASSWORD_POLICY.lock();
    guard.as_ref().and_then(|m| m.is_locked(uid))
}

/// Unlock user
pub fn unlock(uid: u32) -> PolicyResult<()> {
    let mut guard = PASSWORD_POLICY.lock();
    let manager = guard.as_mut().ok_or(PolicyError::NotInitialized)?;
    manager.unlock(uid)
}

/// Force password change
pub fn force_change(uid: u32) -> PolicyResult<()> {
    let mut guard = PASSWORD_POLICY.lock();
    let manager = guard.as_mut().ok_or(PolicyError::NotInitialized)?;
    manager.force_change(uid)
}

/// Get password strength
pub fn get_strength(password: &str) -> PasswordStrength {
    let guard = PASSWORD_POLICY.lock();
    guard.as_ref()
        .map(|m| m.get_strength(password))
        .unwrap_or(PasswordStrength::VeryWeak)
}

/// Get statistics
pub fn get_stats() -> (u64, u64, u64, u64, u64) {
    let guard = PASSWORD_POLICY.lock();
    guard.as_ref()
        .map(|m| m.get_stats())
        .unwrap_or((0, 0, 0, 0, 0))
}
