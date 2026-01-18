//! Users & Accounts Settings
//!
//! User account management, login options, and online accounts.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

/// Global users settings state
static USERS_SETTINGS: Mutex<Option<UsersSettings>> = Mutex::new(None);

/// Users settings state
pub struct UsersSettings {
    /// All users
    pub users: Vec<UserAccount>,
    /// Current user ID
    pub current_user: u32,
    /// Login options
    pub login_options: LoginOptions,
    /// Online accounts
    pub online_accounts: Vec<OnlineAccount>,
}

/// User account
#[derive(Debug, Clone)]
pub struct UserAccount {
    /// User ID
    pub uid: u32,
    /// Username
    pub username: String,
    /// Display name
    pub display_name: String,
    /// Account type
    pub account_type: AccountType,
    /// Home directory
    pub home_dir: String,
    /// Shell
    pub shell: String,
    /// Is logged in
    pub logged_in: bool,
    /// Avatar path (or None for default)
    pub avatar: Option<String>,
    /// Password is set
    pub has_password: bool,
    /// Account is locked
    pub locked: bool,
    /// Last login time
    pub last_login: Option<u64>,
}

/// Account type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType {
    /// Standard user
    Standard,
    /// Administrator
    Administrator,
    /// Root user
    Root,
}

impl AccountType {
    pub fn name(&self) -> &'static str {
        match self {
            AccountType::Standard => "Standard",
            AccountType::Administrator => "Administrator",
            AccountType::Root => "Root",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            AccountType::Standard => "Can use most software and change their own settings",
            AccountType::Administrator => "Can install software and change system settings",
            AccountType::Root => "Has full system access",
        }
    }
}

/// Login options
#[derive(Debug, Clone)]
pub struct LoginOptions {
    /// Automatic login user
    pub auto_login_user: Option<u32>,
    /// Show user list
    pub show_user_list: bool,
    /// Allow guest login
    pub allow_guest: bool,
    /// Password required for login
    pub require_password: bool,
    /// Lock screen on suspend
    pub lock_on_suspend: bool,
    /// Lock screen after idle
    pub lock_after_idle: bool,
    /// Idle timeout for lock (seconds)
    pub idle_lock_timeout: u32,
    /// Show notifications on lock screen
    pub lock_screen_notifications: bool,
    /// Fingerprint login enabled
    pub fingerprint_login: bool,
}

/// Online account
#[derive(Debug, Clone)]
pub struct OnlineAccount {
    /// Account ID
    pub id: String,
    /// Provider
    pub provider: OnlineAccountProvider,
    /// Account name/email
    pub account_name: String,
    /// Is connected
    pub connected: bool,
    /// Enabled services
    pub enabled_services: Vec<OnlineService>,
}

/// Online account provider
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlineAccountProvider {
    Google,
    Microsoft,
    Nextcloud,
    IMAP,
    CalDAV,
    CardDAV,
    WebDAV,
}

impl OnlineAccountProvider {
    pub fn name(&self) -> &'static str {
        match self {
            OnlineAccountProvider::Google => "Google",
            OnlineAccountProvider::Microsoft => "Microsoft",
            OnlineAccountProvider::Nextcloud => "Nextcloud",
            OnlineAccountProvider::IMAP => "IMAP Email",
            OnlineAccountProvider::CalDAV => "CalDAV Calendar",
            OnlineAccountProvider::CardDAV => "CardDAV Contacts",
            OnlineAccountProvider::WebDAV => "WebDAV Files",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            OnlineAccountProvider::Google => "google",
            OnlineAccountProvider::Microsoft => "microsoft",
            OnlineAccountProvider::Nextcloud => "nextcloud",
            _ => "mail-generic",
        }
    }

    pub fn supported_services(&self) -> &[OnlineService] {
        match self {
            OnlineAccountProvider::Google => &[
                OnlineService::Email,
                OnlineService::Calendar,
                OnlineService::Contacts,
                OnlineService::Files,
            ],
            OnlineAccountProvider::Microsoft => &[
                OnlineService::Email,
                OnlineService::Calendar,
                OnlineService::Contacts,
                OnlineService::Files,
            ],
            OnlineAccountProvider::Nextcloud => &[
                OnlineService::Calendar,
                OnlineService::Contacts,
                OnlineService::Files,
            ],
            OnlineAccountProvider::IMAP => &[OnlineService::Email],
            OnlineAccountProvider::CalDAV => &[OnlineService::Calendar],
            OnlineAccountProvider::CardDAV => &[OnlineService::Contacts],
            OnlineAccountProvider::WebDAV => &[OnlineService::Files],
        }
    }
}

/// Online service type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlineService {
    Email,
    Calendar,
    Contacts,
    Files,
}

impl OnlineService {
    pub fn name(&self) -> &'static str {
        match self {
            OnlineService::Email => "Email",
            OnlineService::Calendar => "Calendar",
            OnlineService::Contacts => "Contacts",
            OnlineService::Files => "Files",
        }
    }
}

/// Initialize users settings
pub fn init() {
    let mut state = USERS_SETTINGS.lock();
    if state.is_some() {
        return;
    }

    *state = Some(UsersSettings {
        users: vec![
            UserAccount {
                uid: 0,
                username: "root".to_string(),
                display_name: "Root".to_string(),
                account_type: AccountType::Root,
                home_dir: "/root".to_string(),
                shell: "/bin/sh".to_string(),
                logged_in: false,
                avatar: None,
                has_password: true,
                locked: false,
                last_login: None,
            },
            UserAccount {
                uid: 1000,
                username: "user".to_string(),
                display_name: "User".to_string(),
                account_type: AccountType::Administrator,
                home_dir: "/home/user".to_string(),
                shell: "/bin/sh".to_string(),
                logged_in: true,
                avatar: None,
                has_password: true,
                locked: false,
                last_login: Some(0),
            },
        ],
        current_user: 1000,
        login_options: LoginOptions {
            auto_login_user: None,
            show_user_list: true,
            allow_guest: false,
            require_password: true,
            lock_on_suspend: true,
            lock_after_idle: true,
            idle_lock_timeout: 300,
            lock_screen_notifications: true,
            fingerprint_login: false,
        },
        online_accounts: Vec::new(),
    });

    crate::kprintln!("users settings: initialized");
}

/// Get current user
pub fn get_current_user() -> Option<UserAccount> {
    let state = USERS_SETTINGS.lock();
    state.as_ref().and_then(|s| {
        s.users.iter().find(|u| u.uid == s.current_user).cloned()
    })
}

/// Get all users
pub fn get_users() -> Vec<UserAccount> {
    let state = USERS_SETTINGS.lock();
    state.as_ref().map(|s| s.users.clone()).unwrap_or_default()
}

/// Get user by UID
pub fn get_user(uid: u32) -> Option<UserAccount> {
    let state = USERS_SETTINGS.lock();
    state.as_ref().and_then(|s| {
        s.users.iter().find(|u| u.uid == uid).cloned()
    })
}

/// Create user
pub fn create_user(username: &str, display_name: &str, account_type: AccountType) -> Result<u32, UsersError> {
    let mut state = USERS_SETTINGS.lock();
    let state = state.as_mut().ok_or(UsersError::NotInitialized)?;

    // Check for duplicate username
    if state.users.iter().any(|u| u.username == username) {
        return Err(UsersError::UserExists);
    }

    // Find next available UID
    let uid = state.users.iter()
        .filter(|u| u.uid >= 1000)
        .map(|u| u.uid)
        .max()
        .unwrap_or(999) + 1;

    let user = UserAccount {
        uid,
        username: username.to_string(),
        display_name: display_name.to_string(),
        account_type,
        home_dir: alloc::format!("/home/{}", username),
        shell: "/bin/sh".to_string(),
        logged_in: false,
        avatar: None,
        has_password: false,
        locked: false,
        last_login: None,
    };

    state.users.push(user);

    Ok(uid)
}

/// Delete user
pub fn delete_user(uid: u32) -> Result<(), UsersError> {
    let mut state = USERS_SETTINGS.lock();
    let state = state.as_mut().ok_or(UsersError::NotInitialized)?;

    if uid == 0 {
        return Err(UsersError::CannotDeleteRoot);
    }

    if uid == state.current_user {
        return Err(UsersError::CannotDeleteCurrentUser);
    }

    let idx = state.users.iter()
        .position(|u| u.uid == uid)
        .ok_or(UsersError::UserNotFound)?;

    state.users.remove(idx);

    Ok(())
}

/// Set user display name
pub fn set_display_name(uid: u32, name: &str) -> Result<(), UsersError> {
    let mut state = USERS_SETTINGS.lock();
    let state = state.as_mut().ok_or(UsersError::NotInitialized)?;

    let user = state.users.iter_mut()
        .find(|u| u.uid == uid)
        .ok_or(UsersError::UserNotFound)?;

    user.display_name = name.to_string();

    Ok(())
}

/// Set user account type
pub fn set_account_type(uid: u32, account_type: AccountType) -> Result<(), UsersError> {
    let mut state = USERS_SETTINGS.lock();
    let state = state.as_mut().ok_or(UsersError::NotInitialized)?;

    if uid == 0 {
        return Err(UsersError::CannotModifyRoot);
    }

    let user = state.users.iter_mut()
        .find(|u| u.uid == uid)
        .ok_or(UsersError::UserNotFound)?;

    user.account_type = account_type;

    Ok(())
}

/// Set user avatar
pub fn set_avatar(uid: u32, avatar_path: Option<&str>) -> Result<(), UsersError> {
    let mut state = USERS_SETTINGS.lock();
    let state = state.as_mut().ok_or(UsersError::NotInitialized)?;

    let user = state.users.iter_mut()
        .find(|u| u.uid == uid)
        .ok_or(UsersError::UserNotFound)?;

    user.avatar = avatar_path.map(|s| s.to_string());

    Ok(())
}

/// Lock user account
pub fn lock_user(uid: u32) -> Result<(), UsersError> {
    let mut state = USERS_SETTINGS.lock();
    let state = state.as_mut().ok_or(UsersError::NotInitialized)?;

    if uid == 0 {
        return Err(UsersError::CannotModifyRoot);
    }

    if uid == state.current_user {
        return Err(UsersError::CannotLockCurrentUser);
    }

    let user = state.users.iter_mut()
        .find(|u| u.uid == uid)
        .ok_or(UsersError::UserNotFound)?;

    user.locked = true;

    Ok(())
}

/// Unlock user account
pub fn unlock_user(uid: u32) -> Result<(), UsersError> {
    let mut state = USERS_SETTINGS.lock();
    let state = state.as_mut().ok_or(UsersError::NotInitialized)?;

    let user = state.users.iter_mut()
        .find(|u| u.uid == uid)
        .ok_or(UsersError::UserNotFound)?;

    user.locked = false;

    Ok(())
}

/// Get login options
pub fn get_login_options() -> Option<LoginOptions> {
    let state = USERS_SETTINGS.lock();
    state.as_ref().map(|s| s.login_options.clone())
}

/// Set auto login user
pub fn set_auto_login(uid: Option<u32>) -> Result<(), UsersError> {
    let mut state = USERS_SETTINGS.lock();
    let state = state.as_mut().ok_or(UsersError::NotInitialized)?;

    if let Some(uid) = uid {
        if !state.users.iter().any(|u| u.uid == uid) {
            return Err(UsersError::UserNotFound);
        }
    }

    state.login_options.auto_login_user = uid;

    Ok(())
}

/// Set lock on suspend
pub fn set_lock_on_suspend(enabled: bool) {
    let mut state = USERS_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.login_options.lock_on_suspend = enabled;
    }
}

/// Set lock after idle
pub fn set_lock_after_idle(enabled: bool, timeout: Option<u32>) {
    let mut state = USERS_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.login_options.lock_after_idle = enabled;
        if let Some(t) = timeout {
            s.login_options.idle_lock_timeout = t;
        }
    }
}

/// Get online accounts
pub fn get_online_accounts() -> Vec<OnlineAccount> {
    let state = USERS_SETTINGS.lock();
    state.as_ref().map(|s| s.online_accounts.clone()).unwrap_or_default()
}

/// Add online account
pub fn add_online_account(provider: OnlineAccountProvider, account_name: &str) -> Result<String, UsersError> {
    let mut state = USERS_SETTINGS.lock();
    let state = state.as_mut().ok_or(UsersError::NotInitialized)?;

    let id = alloc::format!("{:?}-{}", provider, account_name);

    let account = OnlineAccount {
        id: id.clone(),
        provider,
        account_name: account_name.to_string(),
        connected: false,
        enabled_services: provider.supported_services().to_vec(),
    };

    state.online_accounts.push(account);

    Ok(id)
}

/// Remove online account
pub fn remove_online_account(account_id: &str) -> Result<(), UsersError> {
    let mut state = USERS_SETTINGS.lock();
    let state = state.as_mut().ok_or(UsersError::NotInitialized)?;

    let idx = state.online_accounts.iter()
        .position(|a| a.id == account_id)
        .ok_or(UsersError::AccountNotFound)?;

    state.online_accounts.remove(idx);

    Ok(())
}

/// Users error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsersError {
    NotInitialized,
    UserNotFound,
    UserExists,
    CannotDeleteRoot,
    CannotModifyRoot,
    CannotDeleteCurrentUser,
    CannotLockCurrentUser,
    AccountNotFound,
    AuthFailed,
}
