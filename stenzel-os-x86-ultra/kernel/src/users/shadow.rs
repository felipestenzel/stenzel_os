//! Shadow Password Database (/etc/shadow)
//!
//! Format: username:password_hash:lastchanged:min:max:warn:inactive:expire:reserved
//!
//! Example:
//! root:$6$rounds=5000$salt$hash:18000:0:99999:7:::
//! alice:$6$xyz$abcdef:18500:0:99999:7:::
//!
//! Password hash format: $id$salt$hash
//! - $1$ = MD5 (deprecated)
//! - $5$ = SHA-256
//! - $6$ = SHA-512
//! - ! or * = locked account
//! - empty = no password

use alloc::string::String;
use alloc::vec::Vec;

/// Shadow password entry
#[derive(Debug, Clone)]
pub struct ShadowEntry {
    /// Username
    pub username: String,
    /// Password hash (or ! for locked, * for disabled, empty for no password)
    pub password_hash: String,
    /// Days since Jan 1, 1970 that password was last changed
    pub last_changed: u32,
    /// Minimum number of days before password can be changed
    pub min_days: u32,
    /// Maximum number of days password is valid
    pub max_days: u32,
    /// Days before expiration to warn user
    pub warn_days: u32,
    /// Days after expiration until account is disabled
    pub inactive_days: Option<u32>,
    /// Days since Jan 1, 1970 that account is disabled
    pub expire_date: Option<u32>,
    /// Reserved field
    pub reserved: Option<String>,
}

impl ShadowEntry {
    /// Create a new shadow entry with default values
    pub fn new(username: &str, password_hash: &str) -> Self {
        Self {
            username: String::from(username),
            password_hash: String::from(password_hash),
            last_changed: 0,
            min_days: 0,
            max_days: 99999,
            warn_days: 7,
            inactive_days: None,
            expire_date: None,
            reserved: None,
        }
    }

    /// Create a locked account
    pub fn locked(username: &str) -> Self {
        Self::new(username, "!")
    }

    /// Create an account with no password
    pub fn no_password(username: &str) -> Self {
        Self::new(username, "")
    }

    /// Check if account is locked
    pub fn is_locked(&self) -> bool {
        self.password_hash.starts_with('!') || self.password_hash.starts_with('*')
    }

    /// Check if account has no password
    pub fn has_no_password(&self) -> bool {
        self.password_hash.is_empty()
    }

    /// Check if password is expired
    pub fn is_expired(&self, current_days: u32) -> bool {
        if self.max_days == 99999 || self.max_days == 0 {
            return false; // Never expires
        }

        current_days > self.last_changed + self.max_days
    }

    /// Check if account is disabled
    pub fn is_disabled(&self, current_days: u32) -> bool {
        if let Some(expire) = self.expire_date {
            if current_days > expire {
                return true;
            }
        }
        false
    }

    /// Check if password needs to be changed
    pub fn needs_change(&self, current_days: u32) -> bool {
        if self.max_days == 99999 || self.max_days == 0 {
            return false;
        }

        current_days > self.last_changed + self.max_days - self.warn_days
    }

    /// Lock the account
    pub fn lock(&mut self) {
        if !self.password_hash.starts_with('!') {
            let mut new_hash = String::from("!");
            new_hash.push_str(&self.password_hash);
            self.password_hash = new_hash;
        }
    }

    /// Unlock the account
    pub fn unlock(&mut self) {
        if self.password_hash.starts_with('!') {
            self.password_hash = String::from(&self.password_hash[1..]);
        }
    }

    /// Parse from /etc/shadow line
    pub fn from_shadow_line(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 9 {
            return None;
        }

        let parse_u32 = |s: &str| -> u32 {
            if s.is_empty() {
                0
            } else {
                s.parse().unwrap_or(0)
            }
        };

        let parse_opt_u32 = |s: &str| -> Option<u32> {
            if s.is_empty() {
                None
            } else {
                s.parse().ok()
            }
        };

        Some(Self {
            username: String::from(parts[0]),
            password_hash: String::from(parts[1]),
            last_changed: parse_u32(parts[2]),
            min_days: parse_u32(parts[3]),
            max_days: parse_u32(parts[4]),
            warn_days: parse_u32(parts[5]),
            inactive_days: parse_opt_u32(parts[6]),
            expire_date: parse_opt_u32(parts[7]),
            reserved: if parts[8].is_empty() {
                None
            } else {
                Some(String::from(parts[8]))
            },
        })
    }

    /// Format as /etc/shadow line
    pub fn to_shadow_line(&self) -> String {
        use alloc::string::ToString;
        let mut s = self.username.clone();
        s.push(':');
        s.push_str(&self.password_hash);
        s.push(':');
        s.push_str(&self.last_changed.to_string());
        s.push(':');
        s.push_str(&self.min_days.to_string());
        s.push(':');
        s.push_str(&self.max_days.to_string());
        s.push(':');
        s.push_str(&self.warn_days.to_string());
        s.push(':');
        if let Some(inactive) = self.inactive_days {
            s.push_str(&inactive.to_string());
        }
        s.push(':');
        if let Some(expire) = self.expire_date {
            s.push_str(&expire.to_string());
        }
        s.push(':');
        if let Some(ref reserved) = self.reserved {
            s.push_str(reserved);
        }
        s
    }
}

/// Shadow password database
pub struct ShadowDatabase {
    entries: Vec<ShadowEntry>,
}

impl ShadowDatabase {
    /// Create a new empty database
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Load from /etc/shadow content
    pub fn from_shadow(content: &str) -> Self {
        let mut db = Self::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(entry) = ShadowEntry::from_shadow_line(line) {
                db.entries.push(entry);
            }
        }

        db
    }

    /// Export to /etc/shadow format
    pub fn to_shadow(&self) -> String {
        let mut content = String::new();
        for entry in &self.entries {
            content.push_str(&entry.to_shadow_line());
            content.push('\n');
        }
        content
    }

    /// Get all entries
    pub fn entries(&self) -> &[ShadowEntry] {
        &self.entries
    }

    /// Get entry by username
    pub fn get(&self, username: &str) -> Option<&ShadowEntry> {
        self.entries.iter().find(|e| e.username == username)
    }

    /// Get mutable entry by username
    pub fn get_mut(&mut self, username: &str) -> Option<&mut ShadowEntry> {
        self.entries.iter_mut().find(|e| e.username == username)
    }

    /// Add an entry
    pub fn add_entry(&mut self, entry: ShadowEntry) {
        // Remove existing entry with same username if any
        self.entries.retain(|e| e.username != entry.username);
        self.entries.push(entry);
    }

    /// Remove an entry by username
    pub fn remove_entry(&mut self, username: &str) {
        self.entries.retain(|e| e.username != username);
    }

    /// Count entries
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Check if user exists
    pub fn exists(&self, username: &str) -> bool {
        self.entries.iter().any(|e| e.username == username)
    }

    /// Get password hash for user
    pub fn get_password_hash(&self, username: &str) -> Option<&str> {
        self.get(username).map(|e| e.password_hash.as_str())
    }

    /// Set password hash for user
    pub fn set_password_hash(&mut self, username: &str, hash: &str) -> bool {
        if let Some(entry) = self.get_mut(username) {
            entry.password_hash = String::from(hash);
            true
        } else {
            false
        }
    }

    /// Lock user account
    pub fn lock_account(&mut self, username: &str) -> bool {
        if let Some(entry) = self.get_mut(username) {
            entry.lock();
            true
        } else {
            false
        }
    }

    /// Unlock user account
    pub fn unlock_account(&mut self, username: &str) -> bool {
        if let Some(entry) = self.get_mut(username) {
            entry.unlock();
            true
        } else {
            false
        }
    }
}

impl Default for ShadowDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_parse_shadow_line() {
        let line = "root:$6$salt$hash:18000:0:99999:7:::";
        let entry = ShadowEntry::from_shadow_line(line).unwrap();
        assert_eq!(entry.username, "root");
        assert_eq!(entry.password_hash, "$6$salt$hash");
        assert_eq!(entry.last_changed, 18000);
    }

    fn test_locked_account() {
        let entry = ShadowEntry::locked("testuser");
        assert!(entry.is_locked());
    }

    fn test_lock_unlock() {
        let mut entry = ShadowEntry::new("testuser", "$6$salt$hash");
        assert!(!entry.is_locked());

        entry.lock();
        assert!(entry.is_locked());
        assert_eq!(entry.password_hash, "!$6$salt$hash");

        entry.unlock();
        assert!(!entry.is_locked());
        assert_eq!(entry.password_hash, "$6$salt$hash");
    }
}
