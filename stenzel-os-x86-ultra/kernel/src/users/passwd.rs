//! User Database (/etc/passwd)
//!
//! Format: username:password:uid:gid:gecos:home:shell
//!
//! Example:
//! root:x:0:0:root:/root:/bin/sh
//! nobody:x:65534:65534:Nobody:/nonexistent:/usr/sbin/nologin

use alloc::string::String;
use alloc::vec::Vec;

/// User ID type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uid(pub u32);

impl Uid {
    /// Root UID
    pub const ROOT: Uid = Uid(0);

    /// Nobody UID
    pub const NOBODY: Uid = Uid(65534);

    /// Check if this is root
    pub fn is_root(&self) -> bool {
        self.0 == 0
    }

    /// Get the raw value
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl From<u32> for Uid {
    fn from(val: u32) -> Self {
        Uid(val)
    }
}

/// Group ID type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Gid(pub u32);

impl Gid {
    /// Root GID
    pub const ROOT: Gid = Gid(0);

    /// Wheel group GID
    pub const WHEEL: Gid = Gid(10);

    /// Users group GID
    pub const USERS: Gid = Gid(100);

    /// Nogroup GID
    pub const NOGROUP: Gid = Gid(65534);

    /// Get the raw value
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl From<u32> for Gid {
    fn from(val: u32) -> Self {
        Gid(val)
    }
}

/// User entry (from /etc/passwd)
#[derive(Debug, Clone)]
pub struct User {
    /// Username (login name)
    pub username: String,
    /// Password placeholder (usually 'x' meaning check shadow)
    pub password: String,
    /// User ID
    pub uid: Uid,
    /// Primary group ID
    pub gid: Gid,
    /// GECOS field (full name, contact info)
    pub gecos: String,
    /// Home directory
    pub home: String,
    /// Login shell
    pub shell: String,
}

impl User {
    /// Create a new user
    pub fn new(
        username: &str,
        uid: Uid,
        gid: Gid,
        gecos: &str,
        home: &str,
        shell: &str,
    ) -> Self {
        Self {
            username: String::from(username),
            password: String::from("x"),
            uid,
            gid,
            gecos: String::from(gecos),
            home: String::from(home),
            shell: String::from(shell),
        }
    }

    /// Check if this user can login (has a valid shell)
    pub fn can_login(&self) -> bool {
        !self.shell.ends_with("nologin") && !self.shell.ends_with("false")
    }

    /// Check if this is root
    pub fn is_root(&self) -> bool {
        self.uid.is_root()
    }

    /// Parse from /etc/passwd line
    pub fn from_passwd_line(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 7 {
            return None;
        }

        let uid: u32 = parts[2].parse().ok()?;
        let gid: u32 = parts[3].parse().ok()?;

        Some(Self {
            username: String::from(parts[0]),
            password: String::from(parts[1]),
            uid: Uid(uid),
            gid: Gid(gid),
            gecos: String::from(parts[4]),
            home: String::from(parts[5]),
            shell: String::from(parts[6]),
        })
    }

    /// Format as /etc/passwd line
    pub fn to_passwd_line(&self) -> String {
        use alloc::string::ToString;
        let mut s = self.username.clone();
        s.push(':');
        s.push_str(&self.password);
        s.push(':');
        s.push_str(&self.uid.0.to_string());
        s.push(':');
        s.push_str(&self.gid.0.to_string());
        s.push(':');
        s.push_str(&self.gecos);
        s.push(':');
        s.push_str(&self.home);
        s.push(':');
        s.push_str(&self.shell);
        s
    }
}

/// User database
pub struct UserDatabase {
    users: Vec<User>,
}

impl UserDatabase {
    /// Create a new empty database
    pub fn new() -> Self {
        Self { users: Vec::new() }
    }

    /// Load from /etc/passwd content
    pub fn from_passwd(content: &str) -> Self {
        let mut db = Self::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(user) = User::from_passwd_line(line) {
                db.users.push(user);
            }
        }

        db
    }

    /// Export to /etc/passwd format
    pub fn to_passwd(&self) -> String {
        let mut content = String::new();
        for user in &self.users {
            content.push_str(&user.to_passwd_line());
            content.push('\n');
        }
        content
    }

    /// Get all users
    pub fn users(&self) -> &[User] {
        &self.users
    }

    /// Get user by UID
    pub fn get_by_uid(&self, uid: Uid) -> Option<&User> {
        self.users.iter().find(|u| u.uid == uid)
    }

    /// Get mutable user by UID
    pub fn get_by_uid_mut(&mut self, uid: Uid) -> Option<&mut User> {
        self.users.iter_mut().find(|u| u.uid == uid)
    }

    /// Get user by username
    pub fn get_by_name(&self, name: &str) -> Option<&User> {
        self.users.iter().find(|u| u.username == name)
    }

    /// Get mutable user by username
    pub fn get_by_name_mut(&mut self, name: &str) -> Option<&mut User> {
        self.users.iter_mut().find(|u| u.username == name)
    }

    /// Add a user
    pub fn add_user(&mut self, user: User) {
        // Remove existing user with same username or UID if any
        self.users.retain(|u| u.username != user.username && u.uid != user.uid);
        self.users.push(user);
    }

    /// Remove a user by username
    pub fn remove_user(&mut self, username: &str) {
        self.users.retain(|u| u.username != username);
    }

    /// Remove a user by UID
    pub fn remove_user_by_uid(&mut self, uid: Uid) {
        self.users.retain(|u| u.uid != uid);
    }

    /// Count users
    pub fn count(&self) -> usize {
        self.users.len()
    }

    /// Check if user exists
    pub fn exists(&self, username: &str) -> bool {
        self.users.iter().any(|u| u.username == username)
    }

    /// Check if UID is in use
    pub fn uid_exists(&self, uid: Uid) -> bool {
        self.users.iter().any(|u| u.uid == uid)
    }

    /// Get users with given primary GID
    pub fn users_with_gid(&self, gid: Gid) -> Vec<&User> {
        self.users.iter().filter(|u| u.gid == gid).collect()
    }

    /// Get all usernames
    pub fn usernames(&self) -> Vec<&str> {
        self.users.iter().map(|u| u.username.as_str()).collect()
    }

    /// Validate database (check for duplicates, etc.)
    pub fn validate(&self) -> Result<(), &'static str> {
        // Check for duplicate usernames
        let mut names: Vec<&str> = self.users.iter().map(|u| u.username.as_str()).collect();
        names.sort();
        for i in 1..names.len() {
            if names[i] == names[i - 1] {
                return Err("Duplicate username");
            }
        }

        // Check for duplicate UIDs
        let mut uids: Vec<u32> = self.users.iter().map(|u| u.uid.0).collect();
        uids.sort();
        for i in 1..uids.len() {
            if uids[i] == uids[i - 1] {
                return Err("Duplicate UID");
            }
        }

        // Check that root exists
        if !self.exists("root") {
            return Err("No root user");
        }

        // Check that root has UID 0
        if let Some(root) = self.get_by_name("root") {
            if root.uid.0 != 0 {
                return Err("root user must have UID 0");
            }
        }

        Ok(())
    }
}

impl Default for UserDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_parse_passwd_line() {
        let line = "root:x:0:0:root:/root:/bin/sh";
        let user = User::from_passwd_line(line).unwrap();
        assert_eq!(user.username, "root");
        assert_eq!(user.uid.0, 0);
        assert_eq!(user.gid.0, 0);
        assert_eq!(user.home, "/root");
        assert_eq!(user.shell, "/bin/sh");
    }

    fn test_to_passwd_line() {
        let user = User::new(
            "testuser",
            Uid(1000),
            Gid(1000),
            "Test User",
            "/home/testuser",
            "/bin/bash",
        );
        let line = user.to_passwd_line();
        assert_eq!(line, "testuser:x:1000:1000:Test User:/home/testuser:/bin/bash");
    }
}
