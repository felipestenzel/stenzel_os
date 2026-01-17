//! su/sudo - Privilege Elevation
//!
//! Provides mechanisms for users to execute commands as other users:
//! - su: Switch user (requires target user's password)
//! - sudo: Execute as another user (requires own password if in sudoers)
//!
//! Configuration:
//! - /etc/sudoers - Who can sudo and what they can do
//! - wheel group - Traditional Unix group for sudo access

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use spin::RwLock;

use super::passwd::{Uid, Gid};
use super::auth::{authenticate, verify_password, AuthError};
use super::{get_user_by_name, get_user_by_uid, user_in_group, GROUP_DB, SHADOW_DB};

/// Sudoers database
static SUDOERS: RwLock<SudoersConfig> = RwLock::new(SudoersConfig::new_const());

/// Sudo entry - who can do what
#[derive(Debug, Clone)]
pub struct SudoEntry {
    /// Username or %groupname
    pub user_spec: String,
    /// List of hosts (usually "ALL")
    pub hosts: Vec<String>,
    /// Run as users (usually "(ALL)" meaning any)
    pub run_as: Vec<String>,
    /// Commands allowed (usually "ALL")
    pub commands: Vec<String>,
    /// Whether password is required
    pub nopasswd: bool,
}

impl SudoEntry {
    /// Create a new sudo entry for all commands
    pub fn all(user_spec: &str) -> Self {
        Self {
            user_spec: String::from(user_spec),
            hosts: vec![String::from("ALL")],
            run_as: vec![String::from("ALL")],
            commands: vec![String::from("ALL")],
            nopasswd: false,
        }
    }

    /// Create without password requirement
    pub fn all_nopasswd(user_spec: &str) -> Self {
        Self {
            user_spec: String::from(user_spec),
            hosts: vec![String::from("ALL")],
            run_as: vec![String::from("ALL")],
            commands: vec![String::from("ALL")],
            nopasswd: true,
        }
    }

    /// Check if this entry matches a user
    pub fn matches_user(&self, username: &str) -> bool {
        if self.user_spec == username {
            return true;
        }

        // Check group match (%groupname)
        if self.user_spec.starts_with('%') {
            let groupname = &self.user_spec[1..];
            return user_in_group(username, groupname);
        }

        false
    }

    /// Check if command is allowed
    pub fn allows_command(&self, command: &str) -> bool {
        for cmd in &self.commands {
            if cmd == "ALL" {
                return true;
            }
            if cmd == command {
                return true;
            }
            // Check prefix match (e.g., "/usr/bin/*")
            if cmd.ends_with('*') {
                let prefix = &cmd[..cmd.len() - 1];
                if command.starts_with(prefix) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if can run as target user
    pub fn allows_run_as(&self, target_user: &str) -> bool {
        for user in &self.run_as {
            if user == "ALL" || user == target_user {
                return true;
            }
        }
        false
    }

    /// Parse from sudoers line
    pub fn from_sudoers_line(line: &str) -> Option<Self> {
        // Format: user_spec host=(run_as) [NOPASSWD:] commands
        // Example: alice ALL=(ALL) ALL
        // Example: %wheel ALL=(ALL) NOPASSWD: ALL
        // Example: bob ALL=(root) /usr/bin/apt, /usr/bin/snap

        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }

        let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
        if parts.len() < 2 {
            return None;
        }

        let user_spec = parts[0];
        let rest = parts[1].trim();

        // Parse host=(run_as) [NOPASSWD:] commands
        let parts: Vec<&str> = rest.splitn(2, '=').collect();
        if parts.len() < 2 {
            return None;
        }

        let hosts = vec![String::from(parts[0].trim())];

        let rest = parts[1].trim();

        // Parse (run_as) and commands
        let (run_as, commands_part) = if rest.starts_with('(') {
            if let Some(end) = rest.find(')') {
                let run_as_str = &rest[1..end];
                let run_as: Vec<String> = run_as_str
                    .split(',')
                    .map(|s| String::from(s.trim()))
                    .collect();
                (run_as, rest[end + 1..].trim())
            } else {
                (vec![String::from("ALL")], rest)
            }
        } else {
            (vec![String::from("ALL")], rest)
        };

        // Check for NOPASSWD:
        let (nopasswd, commands_str) = if commands_part.starts_with("NOPASSWD:") {
            (true, commands_part[9..].trim())
        } else {
            (false, commands_part)
        };

        let commands: Vec<String> = commands_str
            .split(',')
            .map(|s| String::from(s.trim()))
            .collect();

        Some(Self {
            user_spec: String::from(user_spec),
            hosts,
            run_as,
            commands,
            nopasswd,
        })
    }

    /// Format as sudoers line
    pub fn to_sudoers_line(&self) -> String {
        let mut s = self.user_spec.clone();
        s.push(' ');

        // Hosts
        s.push_str(&self.hosts.join(", "));

        // Run as
        s.push_str("=(");
        s.push_str(&self.run_as.join(", "));
        s.push_str(") ");

        // NOPASSWD
        if self.nopasswd {
            s.push_str("NOPASSWD: ");
        }

        // Commands
        s.push_str(&self.commands.join(", "));

        s
    }
}

/// Sudoers configuration
pub struct SudoersConfig {
    entries: Vec<SudoEntry>,
    /// Default behavior
    defaults: SudoDefaults,
}

/// Sudo defaults
#[derive(Debug, Clone)]
pub struct SudoDefaults {
    /// Require tty for sudo
    pub require_tty: bool,
    /// Environment variables to preserve
    pub env_keep: Vec<String>,
    /// Timestamp timeout in minutes (0 = always ask)
    pub timestamp_timeout: u32,
    /// Number of password attempts
    pub passwd_tries: u32,
    /// Insults on wrong password (fun feature)
    pub insults: bool,
}

impl Default for SudoDefaults {
    fn default() -> Self {
        Self {
            require_tty: true,
            env_keep: vec![
                String::from("PATH"),
                String::from("HOME"),
                String::from("SHELL"),
                String::from("TERM"),
                String::from("USER"),
                String::from("LOGNAME"),
            ],
            timestamp_timeout: 15,
            passwd_tries: 3,
            insults: false,
        }
    }
}

impl SudoersConfig {
    /// Create empty config
    pub const fn new_const() -> Self {
        Self {
            entries: Vec::new(),
            defaults: SudoDefaults {
                require_tty: true,
                env_keep: Vec::new(),
                timestamp_timeout: 15,
                passwd_tries: 3,
                insults: false,
            },
        }
    }

    /// Create with default entries
    pub fn new() -> Self {
        let mut config = Self {
            entries: Vec::new(),
            defaults: SudoDefaults::default(),
        };

        // Default: root can do anything
        config.entries.push(SudoEntry::all_nopasswd("root"));

        // Default: wheel group can sudo
        config.entries.push(SudoEntry::all("%wheel"));

        config
    }

    /// Load from /etc/sudoers content
    pub fn from_sudoers(content: &str) -> Self {
        let mut config = Self {
            entries: Vec::new(),
            defaults: SudoDefaults::default(),
        };

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse Defaults
            if line.starts_with("Defaults") {
                // TODO: Parse defaults
                continue;
            }

            // Parse sudo entry
            if let Some(entry) = SudoEntry::from_sudoers_line(line) {
                config.entries.push(entry);
            }
        }

        config
    }

    /// Export to /etc/sudoers format
    pub fn to_sudoers(&self) -> String {
        let mut content = String::from("# Stenzel OS sudoers file\n");
        content.push_str("# This file controls who can run what commands as what users\n\n");

        content.push_str("# Defaults\n");
        content.push_str("Defaults    env_reset\n");
        if self.defaults.require_tty {
            content.push_str("Defaults    requiretty\n");
        }
        content.push_str(&format!("Defaults    timestamp_timeout={}\n", self.defaults.timestamp_timeout));
        content.push_str(&format!("Defaults    passwd_tries={}\n", self.defaults.passwd_tries));
        content.push('\n');

        content.push_str("# User privilege specification\n");
        for entry in &self.entries {
            content.push_str(&entry.to_sudoers_line());
            content.push('\n');
        }

        content
    }

    /// Add an entry
    pub fn add_entry(&mut self, entry: SudoEntry) {
        self.entries.push(entry);
    }

    /// Remove entries for a user
    pub fn remove_user(&mut self, user_spec: &str) {
        self.entries.retain(|e| e.user_spec != user_spec);
    }

    /// Check if user can sudo a command as target user
    pub fn can_sudo(&self, username: &str, target_user: &str, command: &str) -> Option<&SudoEntry> {
        for entry in &self.entries {
            if entry.matches_user(username) &&
               entry.allows_run_as(target_user) &&
               entry.allows_command(command) {
                return Some(entry);
            }
        }
        None
    }
}

impl Default for SudoersConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a privilege elevation attempt
#[derive(Debug, Clone)]
pub enum SuResult {
    /// Success - new UID and GID to use
    Success { uid: Uid, gid: Gid },
    /// Failed authentication
    AuthFailed,
    /// User not found
    UserNotFound,
    /// Permission denied (not in sudoers)
    PermissionDenied,
    /// Account locked
    AccountLocked,
}

/// Switch user (su)
///
/// Requires the target user's password (unless already root)
pub fn su(current_uid: Uid, target_username: &str, password: &str) -> SuResult {
    // Get target user
    let target_user = match get_user_by_name(target_username) {
        Some(u) => u,
        None => return SuResult::UserNotFound,
    };

    // Root can su to anyone without password
    if current_uid.is_root() {
        return SuResult::Success {
            uid: target_user.uid,
            gid: target_user.gid,
        };
    }

    // Verify target user's password
    match authenticate(target_username, password) {
        Ok(()) => SuResult::Success {
            uid: target_user.uid,
            gid: target_user.gid,
        },
        Err(AuthError::InvalidPassword) => SuResult::AuthFailed,
        Err(AuthError::AccountLocked) => SuResult::AccountLocked,
        Err(AuthError::UserNotFound) => SuResult::UserNotFound,
        Err(_) => SuResult::AuthFailed,
    }
}

/// Execute command as another user (sudo)
///
/// Checks sudoers configuration and may require the caller's password
pub fn sudo(
    current_username: &str,
    current_uid: Uid,
    target_username: &str,
    command: &str,
    password: Option<&str>,
) -> SuResult {
    // Get current user
    let current_user = match get_user_by_name(current_username) {
        Some(u) => u,
        None => return SuResult::UserNotFound,
    };

    // Get target user
    let target_user = match get_user_by_name(target_username) {
        Some(u) => u,
        None => return SuResult::UserNotFound,
    };

    // Root can sudo to anyone without checks
    if current_uid.is_root() {
        return SuResult::Success {
            uid: target_user.uid,
            gid: target_user.gid,
        };
    }

    // Check sudoers
    let sudoers = SUDOERS.read();
    let entry = match sudoers.can_sudo(current_username, target_username, command) {
        Some(e) => e.clone(),
        None => return SuResult::PermissionDenied,
    };
    drop(sudoers);

    // If NOPASSWD, allow without password
    if entry.nopasswd {
        return SuResult::Success {
            uid: target_user.uid,
            gid: target_user.gid,
        };
    }

    // Require password
    let password = match password {
        Some(p) => p,
        None => return SuResult::AuthFailed,
    };

    // Verify the CALLER's password (not target user)
    match authenticate(current_username, password) {
        Ok(()) => SuResult::Success {
            uid: target_user.uid,
            gid: target_user.gid,
        },
        Err(AuthError::InvalidPassword) => SuResult::AuthFailed,
        Err(AuthError::AccountLocked) => SuResult::AccountLocked,
        Err(_) => SuResult::AuthFailed,
    }
}

/// Check if user can use sudo
pub fn can_sudo(username: &str) -> bool {
    // Root always can
    if username == "root" {
        return true;
    }

    let sudoers = SUDOERS.read();

    // Check for any matching entry
    for entry in &sudoers.entries {
        if entry.matches_user(username) {
            return true;
        }
    }

    false
}

/// Add a user to sudoers (all commands)
pub fn grant_sudo(username: &str, nopasswd: bool) {
    let mut sudoers = SUDOERS.write();
    let entry = if nopasswd {
        SudoEntry::all_nopasswd(username)
    } else {
        SudoEntry::all(username)
    };
    sudoers.add_entry(entry);
}

/// Remove user from sudoers
pub fn revoke_sudo(username: &str) {
    let mut sudoers = SUDOERS.write();
    sudoers.remove_user(username);
}

/// Grant sudo to a group
pub fn grant_sudo_group(groupname: &str, nopasswd: bool) {
    let spec = {
        let mut s = String::from("%");
        s.push_str(groupname);
        s
    };

    let mut sudoers = SUDOERS.write();
    let entry = if nopasswd {
        SudoEntry::all_nopasswd(&spec)
    } else {
        SudoEntry::all(&spec)
    };
    sudoers.add_entry(entry);
}

/// Initialize the sudoers configuration
pub fn init() {
    let mut sudoers = SUDOERS.write();
    *sudoers = SudoersConfig::new();
    crate::kprintln!("sudo: initialized (root, %wheel)");
}

/// Reload sudoers from file content
pub fn reload(content: &str) {
    let mut sudoers = SUDOERS.write();
    *sudoers = SudoersConfig::from_sudoers(content);
}

/// Get current sudoers configuration
pub fn get_config() -> String {
    let sudoers = SUDOERS.read();
    sudoers.to_sudoers()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_sudoers_parse() {
        let line = "alice ALL=(ALL) ALL";
        let entry = SudoEntry::from_sudoers_line(line).unwrap();
        assert_eq!(entry.user_spec, "alice");
        assert!(entry.allows_command("/bin/ls"));
        assert!(entry.allows_run_as("root"));
        assert!(!entry.nopasswd);
    }

    fn test_group_match() {
        let entry = SudoEntry::all("%wheel");
        // Would need actual group membership check
        assert!(!entry.matches_user("alice")); // Only matches if alice is in wheel
    }

    fn test_nopasswd() {
        let line = "%wheel ALL=(ALL) NOPASSWD: ALL";
        let entry = SudoEntry::from_sudoers_line(line).unwrap();
        assert!(entry.nopasswd);
    }
}
