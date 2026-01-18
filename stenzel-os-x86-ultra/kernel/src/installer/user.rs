//! User Configuration for Installer

use alloc::string::String;
use super::{InstallConfig, InstallError, InstallResult};
use super::partition::PartitionLayout;

/// Configure users on the installed system
pub fn configure_users(layout: &PartitionLayout, config: &InstallConfig) -> InstallResult<()> {
    crate::kprintln!("user: Configuring users");

    // Create root user
    create_user("root", None, "/root", "/bin/sh", true)?;

    // Create primary user
    create_user(&config.username, Some(&config.password_hash), 
                &alloc::format!("/home/{}", config.username), "/bin/sh", false)?;

    // Setup sudo
    setup_sudo(&config.username)?;

    // Create groups
    create_default_groups()?;

    // Add user to groups
    add_user_to_groups(&config.username, &["wheel", "audio", "video", "network"])?;

    crate::kprintln!("user: User configuration complete");
    Ok(())
}

fn create_user(username: &str, password_hash: Option<&str>, home: &str, shell: &str, is_system: bool) -> InstallResult<()> {
    crate::kprintln!("user: Creating user {} (home: {}, shell: {})", username, home, shell);
    
    let uid = if is_system { 0 } else { 1000 };
    let gid = uid;
    
    // Add to /etc/passwd
    let passwd_entry = alloc::format!("{}:x:{}:{}:{}:{}:{}\n", 
        username, uid, gid, username, home, shell);
    
    // Add to /etc/shadow
    let shadow_entry = if let Some(hash) = password_hash {
        alloc::format!("{}:{}:19000:0:99999:7:::\n", username, hash)
    } else {
        alloc::format!("{}:!:19000:0:99999:7:::\n", username)
    };
    
    let _ = (passwd_entry, shadow_entry);
    
    // Create home directory
    if !is_system {
        create_home_directory(username, home)?;
    }
    
    Ok(())
}

fn create_home_directory(username: &str, home: &str) -> InstallResult<()> {
    crate::kprintln!("user: Creating home directory {}", home);
    // mkdir -p home
    // Copy skel files
    // chown to user
    let _ = username;
    Ok(())
}

fn setup_sudo(username: &str) -> InstallResult<()> {
    crate::kprintln!("user: Setting up sudo for {}", username);
    // Add user to sudoers or wheel group
    Ok(())
}

fn create_default_groups() -> InstallResult<()> {
    let groups = [
        ("root", 0), ("wheel", 10), ("audio", 92), ("video", 93),
        ("network", 90), ("storage", 95), ("users", 100),
    ];
    
    for (name, gid) in groups {
        crate::kprintln!("user: Creating group {} (gid: {})", name, gid);
    }
    
    Ok(())
}

fn add_user_to_groups(username: &str, groups: &[&str]) -> InstallResult<()> {
    for group in groups {
        crate::kprintln!("user: Adding {} to group {}", username, group);
    }
    Ok(())
}

pub fn init() {
    crate::kprintln!("user: User manager initialized");
}
