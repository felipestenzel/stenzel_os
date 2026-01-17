//! User Management Commands
//!
//! Command-line utilities for user and group management:
//! - useradd: Create a new user
//! - userdel: Delete a user
//! - usermod: Modify a user
//! - groupadd: Create a new group
//! - groupdel: Delete a group
//! - groupmod: Modify a group
//! - passwd: Change password
//! - chsh: Change shell
//! - chfn: Change finger info (GECOS)

use alloc::string::String;
use alloc::vec::Vec;

use super::passwd::{User, Uid, Gid};
use super::group::Group;
use super::shadow::ShadowEntry;
use super::auth::hash_password;
use super::{
    USER_DB, GROUP_DB, SHADOW_DB,
    get_user_by_name, get_user_by_uid,
    get_group_by_name, get_group_by_gid,
    next_uid, next_gid,
};

/// Result type for user commands
pub type CmdResult<T> = Result<T, &'static str>;

/// Options for useradd
#[derive(Debug, Clone)]
pub struct UserAddOptions {
    /// Username
    pub username: String,
    /// User ID (None = auto-assign)
    pub uid: Option<Uid>,
    /// Primary group ID (None = create user's private group)
    pub gid: Option<Gid>,
    /// GECOS field (full name)
    pub gecos: Option<String>,
    /// Home directory (None = /home/username)
    pub home: Option<String>,
    /// Login shell (None = /bin/sh)
    pub shell: Option<String>,
    /// Create home directory
    pub create_home: bool,
    /// Initial password (None = locked)
    pub password: Option<String>,
    /// System user (UID < 1000)
    pub system: bool,
    /// Supplementary groups
    pub groups: Vec<String>,
}

impl UserAddOptions {
    /// Create with just username
    pub fn new(username: &str) -> Self {
        Self {
            username: String::from(username),
            uid: None,
            gid: None,
            gecos: None,
            home: None,
            shell: None,
            create_home: true,
            password: None,
            system: false,
            groups: Vec::new(),
        }
    }

    /// Set UID
    pub fn with_uid(mut self, uid: u32) -> Self {
        self.uid = Some(Uid(uid));
        self
    }

    /// Set GID
    pub fn with_gid(mut self, gid: u32) -> Self {
        self.gid = Some(Gid(gid));
        self
    }

    /// Set GECOS
    pub fn with_gecos(mut self, gecos: &str) -> Self {
        self.gecos = Some(String::from(gecos));
        self
    }

    /// Set home directory
    pub fn with_home(mut self, home: &str) -> Self {
        self.home = Some(String::from(home));
        self
    }

    /// Set shell
    pub fn with_shell(mut self, shell: &str) -> Self {
        self.shell = Some(String::from(shell));
        self
    }

    /// Set password
    pub fn with_password(mut self, password: &str) -> Self {
        self.password = Some(String::from(password));
        self
    }

    /// Don't create home directory
    pub fn no_create_home(mut self) -> Self {
        self.create_home = false;
        self
    }

    /// Mark as system user
    pub fn system_user(mut self) -> Self {
        self.system = true;
        self
    }

    /// Add supplementary group
    pub fn with_group(mut self, group: &str) -> Self {
        self.groups.push(String::from(group));
        self
    }
}

/// Create a new user (useradd)
pub fn useradd(options: UserAddOptions) -> CmdResult<Uid> {
    // Validate username
    if options.username.is_empty() {
        return Err("Username cannot be empty");
    }

    if options.username.len() > 32 {
        return Err("Username too long (max 32 characters)");
    }

    // Check for invalid characters
    for c in options.username.chars() {
        if !c.is_ascii_alphanumeric() && c != '_' && c != '-' && c != '.' {
            return Err("Username contains invalid characters");
        }
    }

    // First character must be lowercase letter or underscore
    if let Some(first) = options.username.chars().next() {
        if !first.is_ascii_lowercase() && first != '_' {
            return Err("Username must start with lowercase letter or underscore");
        }
    }

    // Check if user already exists
    if get_user_by_name(&options.username).is_some() {
        return Err("User already exists");
    }

    // Determine UID
    let uid = if let Some(uid) = options.uid {
        if get_user_by_uid(uid).is_some() {
            return Err("UID already in use");
        }
        uid
    } else if options.system {
        // System users get UIDs 100-999
        let mut uid = Uid(100);
        loop {
            if get_user_by_uid(uid).is_none() {
                break;
            }
            uid = Uid(uid.0 + 1);
            if uid.0 >= 1000 {
                return Err("No available system UID");
            }
        }
        uid
    } else {
        next_uid()
    };

    // Determine GID (create private group or use specified)
    let gid = if let Some(gid) = options.gid {
        gid
    } else {
        // Create user's private group
        let group_gid = Gid(uid.0);
        let group = Group::new(&options.username, group_gid);

        let mut db = GROUP_DB.write();
        if let Some(ref mut d) = *db {
            d.add_group(group);
        }
        drop(db);

        group_gid
    };

    // Determine home directory
    let home = options.home.unwrap_or_else(|| {
        let mut h = String::from("/home/");
        h.push_str(&options.username);
        h
    });

    // Determine shell
    let shell = options.shell.unwrap_or_else(|| String::from("/bin/sh"));

    // GECOS
    let gecos = options.gecos.unwrap_or_else(|| options.username.clone());

    // Create user
    let user = User {
        username: options.username.clone(),
        password: String::from("x"),
        uid,
        gid,
        gecos,
        home: home.clone(),
        shell,
    };

    // Add to passwd database
    {
        let mut db = USER_DB.write();
        if let Some(ref mut d) = *db {
            d.add_user(user);
        }
    }

    // Create shadow entry
    {
        let mut db = SHADOW_DB.write();
        if let Some(ref mut d) = *db {
            let password_hash = if let Some(ref pw) = options.password {
                hash_password(pw)
            } else {
                String::from("!") // Locked
            };

            d.add_entry(ShadowEntry {
                username: options.username.clone(),
                password_hash,
                last_changed: 0,
                min_days: 0,
                max_days: 99999,
                warn_days: 7,
                inactive_days: None,
                expire_date: None,
                reserved: None,
            });
        }
    }

    // Add to supplementary groups
    for groupname in &options.groups {
        let mut db = GROUP_DB.write();
        if let Some(ref mut d) = *db {
            if let Some(group) = d.get_mut(groupname) {
                group.add_member(&options.username);
            }
        }
    }

    // Create home directory (if requested)
    if options.create_home {
        // This would call the filesystem to create the directory
        // For now, we just log it
        crate::kprintln!("useradd: would create home directory {}", home);
    }

    crate::kprintln!("useradd: created user '{}' (uid={})", options.username, uid.0);

    Ok(uid)
}

/// Options for userdel
#[derive(Debug, Clone)]
pub struct UserDelOptions {
    /// Username to delete
    pub username: String,
    /// Remove home directory
    pub remove_home: bool,
    /// Force removal even if user is logged in
    pub force: bool,
}

impl UserDelOptions {
    pub fn new(username: &str) -> Self {
        Self {
            username: String::from(username),
            remove_home: false,
            force: false,
        }
    }

    pub fn remove_home(mut self) -> Self {
        self.remove_home = true;
        self
    }

    pub fn force(mut self) -> Self {
        self.force = true;
        self
    }
}

/// Delete a user (userdel)
pub fn userdel(options: UserDelOptions) -> CmdResult<()> {
    // Can't delete root
    if options.username == "root" {
        return Err("Cannot delete root user");
    }

    // Check if user exists
    let user = get_user_by_name(&options.username)
        .ok_or("User does not exist")?;

    let home = user.home.clone();

    // Remove from passwd
    {
        let mut db = USER_DB.write();
        if let Some(ref mut d) = *db {
            d.remove_user(&options.username);
        }
    }

    // Remove from shadow
    {
        let mut db = SHADOW_DB.write();
        if let Some(ref mut d) = *db {
            d.remove_entry(&options.username);
        }
    }

    // Remove from all groups
    {
        let mut db = GROUP_DB.write();
        if let Some(ref mut d) = *db {
            d.remove_user_from_all_groups(&options.username);
        }
    }

    // Remove user's private group if it exists and has same name
    {
        let mut db = GROUP_DB.write();
        if let Some(ref mut d) = *db {
            if let Some(group) = d.get_by_name(&options.username) {
                if group.members.is_empty() {
                    d.remove_group(&options.username);
                }
            }
        }
    }

    // Remove home directory if requested
    if options.remove_home {
        crate::kprintln!("userdel: would remove home directory {}", home);
        // Would call filesystem to remove directory
    }

    crate::kprintln!("userdel: deleted user '{}'", options.username);

    Ok(())
}

/// Options for usermod
#[derive(Debug, Clone)]
pub struct UserModOptions {
    /// Username to modify
    pub username: String,
    /// New username
    pub new_username: Option<String>,
    /// New UID
    pub new_uid: Option<Uid>,
    /// New GID
    pub new_gid: Option<Gid>,
    /// New GECOS
    pub new_gecos: Option<String>,
    /// New home directory
    pub new_home: Option<String>,
    /// Move home directory contents
    pub move_home: bool,
    /// New shell
    pub new_shell: Option<String>,
    /// Lock account
    pub lock: bool,
    /// Unlock account
    pub unlock: bool,
    /// Groups to add
    pub add_groups: Vec<String>,
    /// Groups to remove
    pub remove_groups: Vec<String>,
}

impl UserModOptions {
    pub fn new(username: &str) -> Self {
        Self {
            username: String::from(username),
            new_username: None,
            new_uid: None,
            new_gid: None,
            new_gecos: None,
            new_home: None,
            move_home: false,
            new_shell: None,
            lock: false,
            unlock: false,
            add_groups: Vec::new(),
            remove_groups: Vec::new(),
        }
    }
}

/// Modify a user (usermod)
pub fn usermod(options: UserModOptions) -> CmdResult<()> {
    // Check if user exists
    if get_user_by_name(&options.username).is_none() {
        return Err("User does not exist");
    }

    // Modify in passwd database
    {
        let mut db = USER_DB.write();
        if let Some(ref mut d) = *db {
            if let Some(user) = d.get_by_name_mut(&options.username) {
                if let Some(ref gecos) = options.new_gecos {
                    user.gecos = gecos.clone();
                }
                if let Some(ref home) = options.new_home {
                    user.home = home.clone();
                }
                if let Some(ref shell) = options.new_shell {
                    user.shell = shell.clone();
                }
                if let Some(gid) = options.new_gid {
                    user.gid = gid;
                }
                if let Some(uid) = options.new_uid {
                    user.uid = uid;
                }
                if let Some(ref new_name) = options.new_username {
                    user.username = new_name.clone();
                }
            }
        }
    }

    // Handle lock/unlock
    if options.lock {
        let mut db = SHADOW_DB.write();
        if let Some(ref mut d) = *db {
            d.lock_account(&options.username);
        }
    }
    if options.unlock {
        let mut db = SHADOW_DB.write();
        if let Some(ref mut d) = *db {
            d.unlock_account(&options.username);
        }
    }

    // Add to groups
    for groupname in &options.add_groups {
        let mut db = GROUP_DB.write();
        if let Some(ref mut d) = *db {
            if let Some(group) = d.get_mut(groupname) {
                group.add_member(&options.username);
            }
        }
    }

    // Remove from groups
    for groupname in &options.remove_groups {
        let mut db = GROUP_DB.write();
        if let Some(ref mut d) = *db {
            if let Some(group) = d.get_mut(groupname) {
                group.remove_member(&options.username);
            }
        }
    }

    // Update shadow username if changed
    if let Some(ref new_name) = options.new_username {
        let mut db = SHADOW_DB.write();
        if let Some(ref mut d) = *db {
            if let Some(entry) = d.get_mut(&options.username) {
                entry.username = new_name.clone();
            }
        }
    }

    crate::kprintln!("usermod: modified user '{}'", options.username);

    Ok(())
}

/// Options for groupadd
#[derive(Debug, Clone)]
pub struct GroupAddOptions {
    /// Group name
    pub name: String,
    /// Group ID (None = auto-assign)
    pub gid: Option<Gid>,
    /// System group (GID < 1000)
    pub system: bool,
}

impl GroupAddOptions {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            gid: None,
            system: false,
        }
    }

    pub fn with_gid(mut self, gid: u32) -> Self {
        self.gid = Some(Gid(gid));
        self
    }

    pub fn system_group(mut self) -> Self {
        self.system = true;
        self
    }
}

/// Create a new group (groupadd)
pub fn groupadd(options: GroupAddOptions) -> CmdResult<Gid> {
    // Validate group name
    if options.name.is_empty() {
        return Err("Group name cannot be empty");
    }

    if options.name.len() > 32 {
        return Err("Group name too long (max 32 characters)");
    }

    // Check if group already exists
    if get_group_by_name(&options.name).is_some() {
        return Err("Group already exists");
    }

    // Determine GID
    let gid = if let Some(gid) = options.gid {
        if get_group_by_gid(gid).is_some() {
            return Err("GID already in use");
        }
        gid
    } else if options.system {
        // System groups get GIDs 100-999
        let mut gid = Gid(100);
        loop {
            if get_group_by_gid(gid).is_none() {
                break;
            }
            gid = Gid(gid.0 + 1);
            if gid.0 >= 1000 {
                return Err("No available system GID");
            }
        }
        gid
    } else {
        next_gid()
    };

    // Create group
    let group = Group::new(&options.name, gid);

    // Add to database
    {
        let mut db = GROUP_DB.write();
        if let Some(ref mut d) = *db {
            d.add_group(group);
        }
    }

    crate::kprintln!("groupadd: created group '{}' (gid={})", options.name, gid.0);

    Ok(gid)
}

/// Delete a group (groupdel)
pub fn groupdel(name: &str) -> CmdResult<()> {
    // Can't delete root group
    if name == "root" {
        return Err("Cannot delete root group");
    }

    // Check if group exists
    if get_group_by_name(name).is_none() {
        return Err("Group does not exist");
    }

    // Check if any user has this as primary group
    {
        let db = USER_DB.read();
        if let Some(ref d) = *db {
            let group = get_group_by_name(name).unwrap();
            let users = d.users().iter().filter(|u| u.gid == group.gid).count();
            if users > 0 {
                return Err("Cannot delete group: users still have it as primary group");
            }
        }
    }

    // Remove from database
    {
        let mut db = GROUP_DB.write();
        if let Some(ref mut d) = *db {
            d.remove_group(name);
        }
    }

    crate::kprintln!("groupdel: deleted group '{}'", name);

    Ok(())
}

/// Options for groupmod
#[derive(Debug, Clone)]
pub struct GroupModOptions {
    /// Group name
    pub name: String,
    /// New name
    pub new_name: Option<String>,
    /// New GID
    pub new_gid: Option<Gid>,
}

impl GroupModOptions {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            new_name: None,
            new_gid: None,
        }
    }
}

/// Modify a group (groupmod)
pub fn groupmod(options: GroupModOptions) -> CmdResult<()> {
    // Check if group exists
    if get_group_by_name(&options.name).is_none() {
        return Err("Group does not exist");
    }

    // Check new name doesn't conflict
    if let Some(ref new_name) = options.new_name {
        if new_name != &options.name && get_group_by_name(new_name).is_some() {
            return Err("Group name already in use");
        }
    }

    // Check new GID doesn't conflict
    if let Some(gid) = options.new_gid {
        let current_gid = get_group_by_name(&options.name).unwrap().gid;
        if gid != current_gid && get_group_by_gid(gid).is_some() {
            return Err("GID already in use");
        }
    }

    // Modify
    {
        let mut db = GROUP_DB.write();
        if let Some(ref mut d) = *db {
            if let Some(group) = d.get_mut(&options.name) {
                if let Some(ref new_name) = options.new_name {
                    group.name = new_name.clone();
                }
                if let Some(gid) = options.new_gid {
                    group.gid = gid;
                }
            }
        }
    }

    crate::kprintln!("groupmod: modified group '{}'", options.name);

    Ok(())
}

/// Change user password (passwd)
pub fn passwd(username: &str, new_password: &str) -> CmdResult<()> {
    // Check if user exists
    if get_user_by_name(username).is_none() {
        return Err("User does not exist");
    }

    // Update shadow
    {
        let mut db = SHADOW_DB.write();
        if let Some(ref mut d) = *db {
            if let Some(entry) = d.get_mut(username) {
                entry.password_hash = if new_password.is_empty() {
                    String::from("!")
                } else {
                    hash_password(new_password)
                };
                entry.last_changed = 0; // TODO: current day
                return Ok(());
            }
        }
    }

    Err("Failed to update password")
}

/// Change user shell (chsh)
pub fn chsh(username: &str, new_shell: &str) -> CmdResult<()> {
    // Check if user exists
    if get_user_by_name(username).is_none() {
        return Err("User does not exist");
    }

    // Validate shell (should check /etc/shells in a full implementation)
    if !new_shell.starts_with('/') {
        return Err("Shell must be an absolute path");
    }

    // Update
    {
        let mut db = USER_DB.write();
        if let Some(ref mut d) = *db {
            if let Some(user) = d.get_by_name_mut(username) {
                user.shell = String::from(new_shell);
                return Ok(());
            }
        }
    }

    Err("Failed to change shell")
}

/// Change user GECOS/finger info (chfn)
pub fn chfn(username: &str, new_gecos: &str) -> CmdResult<()> {
    // Check if user exists
    if get_user_by_name(username).is_none() {
        return Err("User does not exist");
    }

    // Update
    {
        let mut db = USER_DB.write();
        if let Some(ref mut d) = *db {
            if let Some(user) = d.get_by_name_mut(username) {
                user.gecos = String::from(new_gecos);
                return Ok(());
            }
        }
    }

    Err("Failed to change GECOS")
}

/// Get list of all users
pub fn list_users() -> Vec<(String, Uid, Gid, String)> {
    let db = USER_DB.read();
    if let Some(ref d) = *db {
        d.users()
            .iter()
            .map(|u| (u.username.clone(), u.uid, u.gid, u.home.clone()))
            .collect()
    } else {
        Vec::new()
    }
}

/// Get list of all groups
pub fn list_groups() -> Vec<(String, Gid, usize)> {
    let db = GROUP_DB.read();
    if let Some(ref d) = *db {
        d.groups()
            .iter()
            .map(|g| (g.name.clone(), g.gid, g.members.len()))
            .collect()
    } else {
        Vec::new()
    }
}
