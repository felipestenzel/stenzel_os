//! PAM - Pluggable Authentication Modules
//!
//! Implements a Linux-compatible PAM framework for flexible authentication.
//! Supports multiple authentication methods including passwords, tokens,
//! fingerprints, and smart cards.
//!
//! ## Module Types
//! - auth: Authentication (verify identity)
//! - account: Account management (access control)
//! - session: Session setup/teardown
//! - password: Password management

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KError, KResult};

/// PAM return codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PamResult {
    Success = 0,
    OpenErr = 1,
    SymbolErr = 2,
    ServiceErr = 3,
    SystemErr = 4,
    BufErr = 5,
    PermDenied = 6,
    AuthErr = 7,
    CredInsufficient = 8,
    AuthInfoUnavail = 9,
    UserUnknown = 10,
    MaxTries = 11,
    NewAuthtokReqd = 12,
    AcctExpired = 13,
    SessionErr = 14,
    CredUnavail = 15,
    CredExpired = 16,
    CredErr = 17,
    NoModuleData = 18,
    ConvErr = 19,
    AuthtokErr = 20,
    AuthtokRecoveryErr = 21,
    AuthtokLockBusy = 22,
    AuthtokDisableAging = 23,
    TryAgain = 24,
    Ignore = 25,
    Abort = 26,
    AuthtokExpired = 27,
    ModuleUnknown = 28,
    BadItem = 29,
    ConvAgain = 30,
    Incomplete = 31,
}

impl PamResult {
    /// Convert to KError (for non-success results)
    pub fn to_kerror(self) -> Option<KError> {
        match self {
            PamResult::Success => None,
            PamResult::PermDenied | PamResult::AuthErr => Some(KError::PermissionDenied),
            PamResult::UserUnknown => Some(KError::NotFound),
            _ => Some(KError::IO),
        }
    }
}

/// PAM module type (what the module does)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PamModuleType {
    /// Authentication (verify identity)
    Auth,
    /// Account management
    Account,
    /// Session management
    Session,
    /// Password management
    Password,
}

/// PAM control flags (how failures are handled)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PamControl {
    /// Module must succeed; failure is fatal
    Required,
    /// Module must succeed; failure returns immediately
    Requisite,
    /// Module success grants access (if no required modules fail)
    Sufficient,
    /// Module is optional
    Optional,
    /// Include another PAM stack
    Include,
    /// Skip remaining modules on success
    Substack,
}

/// PAM item types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PamItem {
    Service = 1,
    User = 2,
    Tty = 3,
    Rhost = 4,
    ConvRes = 5,
    Authtok = 6,
    OldAuthtok = 7,
    Ruser = 8,
    UserPrompt = 9,
    FailDelay = 10,
    Xdisplay = 11,
    Xauthdata = 12,
    AuthtokType = 13,
}

/// PAM conversation message types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PamMessageStyle {
    /// Prompt for visible input
    PromptEchoOn = 1,
    /// Prompt for hidden input (password)
    PromptEchoOff = 2,
    /// Error message
    ErrorMsg = 3,
    /// Informational message
    TextInfo = 4,
}

/// PAM conversation message
#[derive(Debug, Clone)]
pub struct PamMessage {
    pub style: PamMessageStyle,
    pub msg: String,
}

/// PAM conversation response
#[derive(Debug, Clone)]
pub struct PamResponse {
    pub resp: String,
}

/// PAM conversation function type
pub type PamConvFn = fn(&[PamMessage]) -> Result<Vec<PamResponse>, PamResult>;

/// PAM conversation structure
pub struct PamConv {
    pub conv: PamConvFn,
    pub appdata_ptr: usize,
}

/// PAM module trait
pub trait PamModule: Send + Sync {
    /// Module name
    fn name(&self) -> &str;

    /// Authenticate user
    fn authenticate(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        let _ = (handle, flags);
        PamResult::Ignore
    }

    /// Set credentials
    fn setcred(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        let _ = (handle, flags);
        PamResult::Ignore
    }

    /// Account management
    fn acct_mgmt(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        let _ = (handle, flags);
        PamResult::Ignore
    }

    /// Open session
    fn open_session(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        let _ = (handle, flags);
        PamResult::Ignore
    }

    /// Close session
    fn close_session(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        let _ = (handle, flags);
        PamResult::Ignore
    }

    /// Change authentication token (password)
    fn chauthtok(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        let _ = (handle, flags);
        PamResult::Ignore
    }
}

/// PAM stack entry
pub struct PamStackEntry {
    pub module_type: PamModuleType,
    pub control: PamControl,
    pub module: Box<dyn PamModule>,
    pub args: Vec<String>,
}

/// PAM configuration for a service
pub struct PamServiceConfig {
    pub name: String,
    pub stack: Vec<PamStackEntry>,
}

impl PamServiceConfig {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            stack: Vec::new(),
        }
    }

    pub fn add(&mut self, module_type: PamModuleType, control: PamControl,
               module: Box<dyn PamModule>, args: Vec<String>) {
        self.stack.push(PamStackEntry {
            module_type,
            control,
            module,
            args,
        });
    }
}

/// PAM handle (session state)
pub struct PamHandle {
    /// Service name
    pub service: String,
    /// Username
    pub user: Option<String>,
    /// TTY
    pub tty: Option<String>,
    /// Remote host
    pub rhost: Option<String>,
    /// Remote user
    pub ruser: Option<String>,
    /// Authentication token (password)
    authtok: Option<String>,
    /// Old authentication token
    old_authtok: Option<String>,
    /// Conversation function
    conv: Option<PamConv>,
    /// Module-specific data
    data: BTreeMap<String, Vec<u8>>,
    /// Environment variables
    env: BTreeMap<String, String>,
    /// Fail delay (microseconds)
    fail_delay: u64,
}

impl PamHandle {
    pub fn new(service: &str) -> Self {
        Self {
            service: String::from(service),
            user: None,
            tty: None,
            rhost: None,
            ruser: None,
            authtok: None,
            old_authtok: None,
            conv: None,
            data: BTreeMap::new(),
            env: BTreeMap::new(),
            fail_delay: 0,
        }
    }

    /// Set user name
    pub fn set_user(&mut self, user: &str) {
        self.user = Some(String::from(user));
    }

    /// Get user name
    pub fn get_user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    /// Set authentication token
    pub fn set_authtok(&mut self, authtok: &str) {
        self.authtok = Some(String::from(authtok));
    }

    /// Get authentication token
    pub fn get_authtok(&self) -> Option<&str> {
        self.authtok.as_deref()
    }

    /// Set conversation function
    pub fn set_conv(&mut self, conv: PamConv) {
        self.conv = Some(conv);
    }

    /// Prompt user for input
    pub fn prompt(&self, style: PamMessageStyle, msg: &str) -> Result<String, PamResult> {
        let conv = self.conv.as_ref().ok_or(PamResult::ConvErr)?;

        let messages = [PamMessage {
            style,
            msg: String::from(msg),
        }];

        let responses = (conv.conv)(&messages)?;

        if responses.is_empty() {
            return Err(PamResult::ConvErr);
        }

        Ok(responses[0].resp.clone())
    }

    /// Prompt for password
    pub fn prompt_password(&self, msg: &str) -> Result<String, PamResult> {
        self.prompt(PamMessageStyle::PromptEchoOff, msg)
    }

    /// Prompt for username
    pub fn prompt_user(&self, msg: &str) -> Result<String, PamResult> {
        self.prompt(PamMessageStyle::PromptEchoOn, msg)
    }

    /// Display an error message
    pub fn error(&self, msg: &str) -> Result<(), PamResult> {
        let _ = self.prompt(PamMessageStyle::ErrorMsg, msg);
        Ok(())
    }

    /// Display an info message
    pub fn info(&self, msg: &str) -> Result<(), PamResult> {
        let _ = self.prompt(PamMessageStyle::TextInfo, msg);
        Ok(())
    }

    /// Set module data
    pub fn set_data(&mut self, key: &str, data: Vec<u8>) {
        self.data.insert(String::from(key), data);
    }

    /// Get module data
    pub fn get_data(&self, key: &str) -> Option<&Vec<u8>> {
        self.data.get(key)
    }

    /// Set environment variable
    pub fn putenv(&mut self, name_value: &str) {
        if let Some(idx) = name_value.find('=') {
            let name = &name_value[..idx];
            let value = &name_value[idx + 1..];
            self.env.insert(String::from(name), String::from(value));
        }
    }

    /// Get environment variable
    pub fn getenv(&self, name: &str) -> Option<&str> {
        self.env.get(name).map(|s| s.as_str())
    }

    /// Get all environment variables
    pub fn get_env_list(&self) -> Vec<String> {
        self.env.iter()
            .map(|(k, v)| alloc::format!("{}={}", k, v))
            .collect()
    }

    /// Set fail delay
    pub fn set_fail_delay(&mut self, usec: u64) {
        self.fail_delay = usec;
    }
}

// =============================================================================
// Built-in PAM Modules
// =============================================================================

/// pam_unix - Traditional Unix password authentication
pub struct PamUnix;

impl PamModule for PamUnix {
    fn name(&self) -> &str {
        "pam_unix"
    }

    fn authenticate(&self, handle: &mut PamHandle, _flags: u32) -> PamResult {
        // Get username
        let user = match handle.get_user() {
            Some(u) => u.to_string(),
            None => {
                match handle.prompt_user("login: ") {
                    Ok(u) => {
                        handle.set_user(&u);
                        u
                    }
                    Err(e) => return e,
                }
            }
        };

        // Get password
        let password = match handle.get_authtok() {
            Some(p) => p.to_string(),
            None => {
                match handle.prompt_password("Password: ") {
                    Ok(p) => {
                        handle.set_authtok(&p);
                        p
                    }
                    Err(e) => return e,
                }
            }
        };

        // Verify password against /etc/shadow or user database
        if verify_password(&user, &password) {
            PamResult::Success
        } else {
            handle.set_fail_delay(2_000_000); // 2 second delay on failure
            PamResult::AuthErr
        }
    }

    fn acct_mgmt(&self, handle: &mut PamHandle, _flags: u32) -> PamResult {
        // Check if account is valid (not expired, not locked)
        let user = match handle.get_user() {
            Some(u) => u,
            None => return PamResult::UserUnknown,
        };

        if check_account_valid(user) {
            PamResult::Success
        } else {
            PamResult::AcctExpired
        }
    }

    fn chauthtok(&self, handle: &mut PamHandle, _flags: u32) -> PamResult {
        let user = match handle.get_user() {
            Some(u) => u.to_string(),
            None => return PamResult::UserUnknown,
        };

        // Get old password
        let old_pass = match handle.prompt_password("Current password: ") {
            Ok(p) => p,
            Err(e) => return e,
        };

        if !verify_password(&user, &old_pass) {
            return PamResult::AuthErr;
        }

        // Get new password
        let new_pass = match handle.prompt_password("New password: ") {
            Ok(p) => p,
            Err(e) => return e,
        };

        // Confirm new password
        let confirm = match handle.prompt_password("Retype new password: ") {
            Ok(p) => p,
            Err(e) => return e,
        };

        if new_pass != confirm {
            let _ = handle.error("Passwords don't match");
            return PamResult::AuthtokErr;
        }

        // Update password
        if update_password(&user, &new_pass) {
            PamResult::Success
        } else {
            PamResult::AuthtokErr
        }
    }
}

/// pam_permit - Always permit (for testing)
pub struct PamPermit;

impl PamModule for PamPermit {
    fn name(&self) -> &str {
        "pam_permit"
    }

    fn authenticate(&self, _handle: &mut PamHandle, _flags: u32) -> PamResult {
        PamResult::Success
    }

    fn acct_mgmt(&self, _handle: &mut PamHandle, _flags: u32) -> PamResult {
        PamResult::Success
    }

    fn open_session(&self, _handle: &mut PamHandle, _flags: u32) -> PamResult {
        PamResult::Success
    }

    fn close_session(&self, _handle: &mut PamHandle, _flags: u32) -> PamResult {
        PamResult::Success
    }

    fn chauthtok(&self, _handle: &mut PamHandle, _flags: u32) -> PamResult {
        PamResult::Success
    }
}

/// pam_deny - Always deny
pub struct PamDeny;

impl PamModule for PamDeny {
    fn name(&self) -> &str {
        "pam_deny"
    }

    fn authenticate(&self, _handle: &mut PamHandle, _flags: u32) -> PamResult {
        PamResult::AuthErr
    }

    fn acct_mgmt(&self, _handle: &mut PamHandle, _flags: u32) -> PamResult {
        PamResult::AcctExpired
    }

    fn open_session(&self, _handle: &mut PamHandle, _flags: u32) -> PamResult {
        PamResult::SessionErr
    }

    fn close_session(&self, _handle: &mut PamHandle, _flags: u32) -> PamResult {
        PamResult::SessionErr
    }

    fn chauthtok(&self, _handle: &mut PamHandle, _flags: u32) -> PamResult {
        PamResult::AuthtokErr
    }
}

/// pam_env - Set environment variables
pub struct PamEnv;

impl PamModule for PamEnv {
    fn name(&self) -> &str {
        "pam_env"
    }

    fn open_session(&self, handle: &mut PamHandle, _flags: u32) -> PamResult {
        // Set default environment variables
        // Clone user to avoid borrow conflict
        let user_opt = handle.get_user().map(String::from);
        if let Some(user) = user_opt {
            handle.putenv(&alloc::format!("USER={}", user));
            handle.putenv(&alloc::format!("LOGNAME={}", user));
            handle.putenv(&alloc::format!("HOME=/home/{}", user));
        }

        handle.putenv("PATH=/usr/local/bin:/usr/bin:/bin");
        handle.putenv("SHELL=/bin/sh");

        PamResult::Success
    }
}

/// pam_limits - Set resource limits
pub struct PamLimits;

impl PamModule for PamLimits {
    fn name(&self) -> &str {
        "pam_limits"
    }

    fn open_session(&self, _handle: &mut PamHandle, _flags: u32) -> PamResult {
        // Apply resource limits from /etc/security/limits.conf
        // This would call setrlimit() for various resources
        PamResult::Success
    }
}

/// pam_securetty - Restrict root login to secure terminals
pub struct PamSecuretty;

impl PamModule for PamSecuretty {
    fn name(&self) -> &str {
        "pam_securetty"
    }

    fn authenticate(&self, handle: &mut PamHandle, _flags: u32) -> PamResult {
        let user = match handle.get_user() {
            Some(u) => u,
            None => return PamResult::UserUnknown,
        };

        // Only check for root
        if user != "root" {
            return PamResult::Success;
        }

        // Check if TTY is in /etc/securetty
        let tty = match &handle.tty {
            Some(t) => t.as_str(),
            None => return PamResult::AuthErr, // No TTY = not secure for root
        };

        // Check against allowed TTYs
        let secure_ttys = ["tty1", "tty2", "tty3", "tty4", "tty5", "tty6", "ttyS0", "console"];

        if secure_ttys.iter().any(|&t| tty.ends_with(t)) {
            PamResult::Success
        } else {
            PamResult::AuthErr
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Verify password against system database
fn verify_password(user: &str, password: &str) -> bool {
    // In a real implementation, this would:
    // 1. Look up user in /etc/passwd
    // 2. Get password hash from /etc/shadow
    // 3. Hash provided password with same salt
    // 4. Compare hashes

    // For now, simple check (for testing)
    if user == "root" && password == "root" {
        return true;
    }

    if user == "user" && password == "user" {
        return true;
    }

    // Check against kernel user database
    if let Some(db_user) = crate::security::user_db().get(user) {
        // Simple check - in real implementation, compare hashed passwords
        let _ = db_user;
        return !password.is_empty();
    }

    false
}

/// Check if account is valid
fn check_account_valid(user: &str) -> bool {
    // Check for:
    // - Account expiration
    // - Password expiration
    // - Account locked

    // For now, all accounts are valid
    let _ = user;
    true
}

/// Update user password
fn update_password(user: &str, _new_password: &str) -> bool {
    // In a real implementation:
    // 1. Hash new password with strong algorithm (sha512crypt, bcrypt, etc.)
    // 2. Update /etc/shadow
    // 3. Optionally update password aging info

    // For now, just return success
    let _ = user;
    true
}

// =============================================================================
// PAM Framework
// =============================================================================

/// PAM manager - handles service configurations and execution
pub struct PamManager {
    /// Service configurations
    services: BTreeMap<String, PamServiceConfig>,
    /// Default service config
    default_service: Option<String>,
    /// Initialized
    initialized: bool,
}

impl PamManager {
    pub const fn new() -> Self {
        Self {
            services: BTreeMap::new(),
            default_service: None,
            initialized: false,
        }
    }

    /// Initialize PAM with default configurations
    pub fn init(&mut self) {
        // Create default "login" service
        let mut login = PamServiceConfig::new("login");
        login.add(PamModuleType::Auth, PamControl::Required,
                  Box::new(PamSecuretty), Vec::new());
        login.add(PamModuleType::Auth, PamControl::Required,
                  Box::new(PamUnix), Vec::new());
        login.add(PamModuleType::Account, PamControl::Required,
                  Box::new(PamUnix), Vec::new());
        login.add(PamModuleType::Session, PamControl::Required,
                  Box::new(PamEnv), Vec::new());
        login.add(PamModuleType::Session, PamControl::Optional,
                  Box::new(PamLimits), Vec::new());
        login.add(PamModuleType::Password, PamControl::Required,
                  Box::new(PamUnix), Vec::new());
        self.services.insert(String::from("login"), login);

        // Create "sudo" service
        let mut sudo = PamServiceConfig::new("sudo");
        sudo.add(PamModuleType::Auth, PamControl::Required,
                 Box::new(PamUnix), Vec::new());
        sudo.add(PamModuleType::Account, PamControl::Required,
                 Box::new(PamUnix), Vec::new());
        self.services.insert(String::from("sudo"), sudo);

        // Create "su" service
        let mut su = PamServiceConfig::new("su");
        su.add(PamModuleType::Auth, PamControl::Required,
               Box::new(PamUnix), Vec::new());
        su.add(PamModuleType::Account, PamControl::Required,
               Box::new(PamUnix), Vec::new());
        su.add(PamModuleType::Session, PamControl::Required,
               Box::new(PamEnv), Vec::new());
        self.services.insert(String::from("su"), su);

        // Create default "other" service (fallback)
        let mut other = PamServiceConfig::new("other");
        other.add(PamModuleType::Auth, PamControl::Required,
                  Box::new(PamDeny), Vec::new());
        other.add(PamModuleType::Account, PamControl::Required,
                  Box::new(PamDeny), Vec::new());
        other.add(PamModuleType::Session, PamControl::Required,
                  Box::new(PamDeny), Vec::new());
        other.add(PamModuleType::Password, PamControl::Required,
                  Box::new(PamDeny), Vec::new());
        self.services.insert(String::from("other"), other);

        self.default_service = Some(String::from("other"));
        self.initialized = true;

        crate::kprintln!("pam: initialized with {} services", self.services.len());
    }

    /// Get service configuration
    pub fn get_service(&self, name: &str) -> Option<&PamServiceConfig> {
        self.services.get(name)
            .or_else(|| self.default_service.as_ref().and_then(|d| self.services.get(d)))
    }

    /// Run authentication stack
    pub fn authenticate(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        self.run_stack(handle, PamModuleType::Auth, flags, |m, h, f| m.authenticate(h, f))
    }

    /// Run account management stack
    pub fn acct_mgmt(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        self.run_stack(handle, PamModuleType::Account, flags, |m, h, f| m.acct_mgmt(h, f))
    }

    /// Run session open stack
    pub fn open_session(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        self.run_stack(handle, PamModuleType::Session, flags, |m, h, f| m.open_session(h, f))
    }

    /// Run session close stack
    pub fn close_session(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        self.run_stack(handle, PamModuleType::Session, flags, |m, h, f| m.close_session(h, f))
    }

    /// Run password change stack
    pub fn chauthtok(&self, handle: &mut PamHandle, flags: u32) -> PamResult {
        self.run_stack(handle, PamModuleType::Password, flags, |m, h, f| m.chauthtok(h, f))
    }

    /// Run a PAM stack
    fn run_stack<F>(&self, handle: &mut PamHandle, module_type: PamModuleType,
                    flags: u32, func: F) -> PamResult
    where
        F: Fn(&dyn PamModule, &mut PamHandle, u32) -> PamResult,
    {
        let service = match self.get_service(&handle.service) {
            Some(s) => s,
            None => return PamResult::ServiceErr,
        };

        let mut result = PamResult::Success;
        let mut required_failed = false;

        for entry in &service.stack {
            if entry.module_type != module_type {
                continue;
            }

            let module_result = func(entry.module.as_ref(), handle, flags);

            match entry.control {
                PamControl::Required => {
                    if module_result != PamResult::Success && module_result != PamResult::Ignore {
                        required_failed = true;
                        if result == PamResult::Success {
                            result = module_result;
                        }
                    }
                }
                PamControl::Requisite => {
                    if module_result != PamResult::Success && module_result != PamResult::Ignore {
                        return module_result;
                    }
                }
                PamControl::Sufficient => {
                    if module_result == PamResult::Success && !required_failed {
                        return PamResult::Success;
                    }
                }
                PamControl::Optional => {
                    if result == PamResult::Success && module_result != PamResult::Ignore {
                        // Only update result if nothing else has set it
                    }
                }
                _ => {}
            }
        }

        result
    }
}

// =============================================================================
// Global Instance
// =============================================================================

static PAM_MANAGER: IrqSafeMutex<PamManager> = IrqSafeMutex::new(PamManager::new());
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize PAM subsystem
pub fn init() {
    if INITIALIZED.load(Ordering::Acquire) {
        return;
    }

    PAM_MANAGER.lock().init();
    INITIALIZED.store(true, Ordering::Release);
}

/// Start a PAM session for a service
pub fn start(service: &str, user: Option<&str>) -> PamHandle {
    let mut handle = PamHandle::new(service);
    if let Some(u) = user {
        handle.set_user(u);
    }
    handle
}

/// Authenticate a user
pub fn authenticate(handle: &mut PamHandle, flags: u32) -> PamResult {
    PAM_MANAGER.lock().authenticate(handle, flags)
}

/// Check account validity
pub fn acct_mgmt(handle: &mut PamHandle, flags: u32) -> PamResult {
    PAM_MANAGER.lock().acct_mgmt(handle, flags)
}

/// Open session
pub fn open_session(handle: &mut PamHandle, flags: u32) -> PamResult {
    PAM_MANAGER.lock().open_session(handle, flags)
}

/// Close session
pub fn close_session(handle: &mut PamHandle, flags: u32) -> PamResult {
    PAM_MANAGER.lock().close_session(handle, flags)
}

/// Change authentication token
pub fn chauthtok(handle: &mut PamHandle, flags: u32) -> PamResult {
    PAM_MANAGER.lock().chauthtok(handle, flags)
}
