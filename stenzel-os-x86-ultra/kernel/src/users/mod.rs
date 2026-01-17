//! User Management System
//!
//! Provides UNIX-style user and group management:
//! - /etc/passwd - User database
//! - /etc/group - Group database
//! - /etc/shadow - Password hashes (separate module)

pub mod passwd;
pub mod group;
pub mod shadow;
pub mod auth;
pub mod sudo;
pub mod commands;
pub mod session;

pub use passwd::{User, UserDatabase, Uid, Gid};
pub use group::{Group, GroupDatabase};
pub use shadow::{ShadowEntry, ShadowDatabase};
pub use auth::{authenticate, verify_password, hash_password};
pub use sudo::{su, sudo as do_sudo, can_sudo, grant_sudo, revoke_sudo, SuResult};
pub use commands::{useradd, userdel, usermod, groupadd, groupdel, groupmod, passwd as change_passwd, chsh, chfn};
pub use commands::{UserAddOptions, UserDelOptions, UserModOptions, GroupAddOptions, GroupModOptions};
pub use session::{Session, SessionId, SessionType, SessionState, login, logout, logout_user, is_logged_in, list_sessions};

use alloc::string::String;
use alloc::vec::Vec;
use spin::RwLock;

/// Global user database
static USER_DB: RwLock<Option<UserDatabase>> = RwLock::new(None);

/// Global group database
static GROUP_DB: RwLock<Option<GroupDatabase>> = RwLock::new(None);

/// Global shadow database
static SHADOW_DB: RwLock<Option<ShadowDatabase>> = RwLock::new(None);

/// Initialize the user system
pub fn init() {
    crate::kprintln!("users: initializing...");

    // Initialize user database
    let mut user_db = UserDatabase::new();

    // Add default users
    user_db.add_user(User {
        username: String::from("root"),
        password: String::from("x"), // Password in shadow
        uid: Uid(0),
        gid: Gid(0),
        gecos: String::from("root"),
        home: String::from("/root"),
        shell: String::from("/bin/sh"),
    });

    user_db.add_user(User {
        username: String::from("nobody"),
        password: String::from("x"),
        uid: Uid(65534),
        gid: Gid(65534),
        gecos: String::from("Nobody"),
        home: String::from("/nonexistent"),
        shell: String::from("/usr/sbin/nologin"),
    });

    *USER_DB.write() = Some(user_db);

    // Initialize group database
    let mut group_db = GroupDatabase::new();

    group_db.add_group(Group {
        name: String::from("root"),
        password: String::from("x"),
        gid: Gid(0),
        members: Vec::new(),
    });

    group_db.add_group(Group {
        name: String::from("wheel"),
        password: String::from("x"),
        gid: Gid(10),
        members: Vec::new(),
    });

    group_db.add_group(Group {
        name: String::from("users"),
        password: String::from("x"),
        gid: Gid(100),
        members: Vec::new(),
    });

    group_db.add_group(Group {
        name: String::from("nogroup"),
        password: String::from("x"),
        gid: Gid(65534),
        members: Vec::new(),
    });

    *GROUP_DB.write() = Some(group_db);

    // Initialize shadow database
    let mut shadow_db = ShadowDatabase::new();

    // Default root password is empty (should be set during installation)
    shadow_db.add_entry(ShadowEntry {
        username: String::from("root"),
        password_hash: String::from("!"), // Locked by default
        last_changed: 0,
        min_days: 0,
        max_days: 99999,
        warn_days: 7,
        inactive_days: None,
        expire_date: None,
        reserved: None,
    });

    *SHADOW_DB.write() = Some(shadow_db);

    crate::kprintln!("users: initialized (root, nobody)");
}

/// Get user by UID
pub fn get_user_by_uid(uid: Uid) -> Option<User> {
    let db = USER_DB.read();
    db.as_ref().and_then(|d| d.get_by_uid(uid).cloned())
}

/// Get user by username
pub fn get_user_by_name(name: &str) -> Option<User> {
    let db = USER_DB.read();
    db.as_ref().and_then(|d| d.get_by_name(name).cloned())
}

/// Get group by GID
pub fn get_group_by_gid(gid: Gid) -> Option<Group> {
    let db = GROUP_DB.read();
    db.as_ref().and_then(|d| d.get_by_gid(gid).cloned())
}

/// Get group by name
pub fn get_group_by_name(name: &str) -> Option<Group> {
    let db = GROUP_DB.read();
    db.as_ref().and_then(|d| d.get_by_name(name).cloned())
}

/// Get all users
pub fn get_all_users() -> Vec<User> {
    let db = USER_DB.read();
    db.as_ref().map(|d| d.users().to_vec()).unwrap_or_default()
}

/// Get all groups
pub fn get_all_groups() -> Vec<Group> {
    let db = GROUP_DB.read();
    db.as_ref().map(|d| d.groups().to_vec()).unwrap_or_default()
}

/// Create a new user
pub fn create_user(username: &str, uid: Uid, gid: Gid, home: &str, shell: &str, password: &str) -> Result<(), &'static str> {
    // Check if user already exists
    if get_user_by_name(username).is_some() {
        return Err("User already exists");
    }

    if get_user_by_uid(uid).is_some() {
        return Err("UID already in use");
    }

    // Add to passwd
    {
        let mut db = USER_DB.write();
        if let Some(ref mut d) = *db {
            d.add_user(User {
                username: String::from(username),
                password: String::from("x"),
                uid,
                gid,
                gecos: String::from(username),
                home: String::from(home),
                shell: String::from(shell),
            });
        }
    }

    // Add to shadow with hashed password
    {
        let mut db = SHADOW_DB.write();
        if let Some(ref mut d) = *db {
            let hash = if password.is_empty() {
                String::from("!")
            } else {
                auth::hash_password(password)
            };

            d.add_entry(ShadowEntry {
                username: String::from(username),
                password_hash: hash,
                last_changed: 0, // TODO: get current day
                min_days: 0,
                max_days: 99999,
                warn_days: 7,
                inactive_days: None,
                expire_date: None,
                reserved: None,
            });
        }
    }

    Ok(())
}

/// Delete a user
pub fn delete_user(username: &str) -> Result<(), &'static str> {
    // Don't allow deleting root
    if username == "root" {
        return Err("Cannot delete root user");
    }

    // Check if user exists
    if get_user_by_name(username).is_none() {
        return Err("User does not exist");
    }

    // Remove from passwd
    {
        let mut db = USER_DB.write();
        if let Some(ref mut d) = *db {
            d.remove_user(username);
        }
    }

    // Remove from shadow
    {
        let mut db = SHADOW_DB.write();
        if let Some(ref mut d) = *db {
            d.remove_entry(username);
        }
    }

    // Remove from groups
    {
        let mut db = GROUP_DB.write();
        if let Some(ref mut d) = *db {
            d.remove_user_from_all_groups(username);
        }
    }

    Ok(())
}

/// Change user password
pub fn change_password(username: &str, new_password: &str) -> Result<(), &'static str> {
    let mut db = SHADOW_DB.write();
    if let Some(ref mut d) = *db {
        if let Some(entry) = d.get_mut(username) {
            entry.password_hash = if new_password.is_empty() {
                String::from("!")
            } else {
                auth::hash_password(new_password)
            };
            entry.last_changed = 0; // TODO: get current day
            return Ok(());
        }
    }
    Err("User not found")
}

/// Add user to group
pub fn add_user_to_group(username: &str, groupname: &str) -> Result<(), &'static str> {
    // Check user exists
    if get_user_by_name(username).is_none() {
        return Err("User does not exist");
    }

    let mut db = GROUP_DB.write();
    if let Some(ref mut d) = *db {
        if let Some(group) = d.get_mut(groupname) {
            if !group.members.contains(&String::from(username)) {
                group.members.push(String::from(username));
            }
            return Ok(());
        }
    }
    Err("Group does not exist")
}

/// Remove user from group
pub fn remove_user_from_group(username: &str, groupname: &str) -> Result<(), &'static str> {
    let mut db = GROUP_DB.write();
    if let Some(ref mut d) = *db {
        if let Some(group) = d.get_mut(groupname) {
            group.members.retain(|m| m != username);
            return Ok(());
        }
    }
    Err("Group does not exist")
}

/// Check if user is in group
pub fn user_in_group(username: &str, groupname: &str) -> bool {
    let db = GROUP_DB.read();
    if let Some(ref d) = *db {
        if let Some(group) = d.get_by_name(groupname) {
            return group.members.contains(&String::from(username));
        }
    }
    false
}

/// Get groups for user
pub fn get_user_groups(username: &str) -> Vec<Group> {
    let db = GROUP_DB.read();
    if let Some(ref d) = *db {
        return d.groups()
            .iter()
            .filter(|g| g.members.contains(&String::from(username)))
            .cloned()
            .collect();
    }
    Vec::new()
}

/// Get next available UID (for user creation)
pub fn next_uid() -> Uid {
    let db = USER_DB.read();
    if let Some(ref d) = *db {
        let max_uid = d.users().iter().map(|u| u.uid.0).max().unwrap_or(999);
        return Uid(max_uid + 1);
    }
    Uid(1000) // Default first user UID
}

/// Get next available GID (for group creation)
pub fn next_gid() -> Gid {
    let db = GROUP_DB.read();
    if let Some(ref d) = *db {
        let max_gid = d.groups().iter().map(|g| g.gid.0).max().unwrap_or(999);
        return Gid(max_gid + 1);
    }
    Gid(1000) // Default first group GID
}
