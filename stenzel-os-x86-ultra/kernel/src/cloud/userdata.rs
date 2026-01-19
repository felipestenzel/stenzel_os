//! Cloud-init User Data Processing
//!
//! Parse and execute user-data (cloud-config, scripts, etc.)

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

use super::{CloudConfig, UserConfig, WriteFile, WriteFileEncoding, MountEntry, SwapConfig,
            PhoneHomeConfig, PowerStateConfig, PowerStateMode};

/// Parse cloud-config YAML
pub fn parse_cloud_config(data: &[u8]) -> Result<CloudConfig, &'static str> {
    let text = core::str::from_utf8(data).map_err(|_| "Invalid UTF-8")?;

    // Check for cloud-config marker
    if !text.trim_start().starts_with("#cloud-config") {
        return Err("Not a cloud-config");
    }

    let mut config = CloudConfig::default();

    // Very simple YAML parsing (real implementation would use proper parser)
    let mut current_section = String::new();
    let mut current_user: Option<UserConfig> = None;
    let mut current_file: Option<WriteFile> = None;
    let mut in_list = false;
    let mut list_items: Vec<String> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Top-level keys
        if !line.starts_with(' ') && !line.starts_with('\t') {
            // Save previous context
            save_section_context(&mut config, &current_section, &list_items, &current_user, &current_file);
            list_items.clear();
            current_user = None;
            current_file = None;
            in_list = false;

            // Parse new section
            if let Some((key, value)) = trimmed.split_once(':') {
                current_section = key.trim().to_string();
                let value = value.trim();

                // Simple values
                match current_section.as_str() {
                    "hostname" => config.hostname = Some(value.to_string()),
                    "fqdn" => config.fqdn = Some(value.to_string()),
                    "manage_etc_hosts" => config.manage_etc_hosts = value == "true",
                    "timezone" => config.timezone = Some(value.to_string()),
                    "locale" => config.locale = Some(value.to_string()),
                    "disable_root" => config.disable_root = value == "true",
                    "final_message" => config.final_message = Some(value.to_string()),
                    _ => {}
                }
            }
            continue;
        }

        // List items
        if trimmed.starts_with("- ") {
            in_list = true;
            let item = trimmed.strip_prefix("- ").unwrap().trim();

            match current_section.as_str() {
                "ssh_authorized_keys" => {
                    config.ssh_authorized_keys.push(item.to_string());
                }
                "packages" => {
                    config.packages.push(item.to_string());
                }
                "bootcmd" => {
                    config.bootcmd.push(item.to_string());
                }
                "runcmd" => {
                    config.runcmd.push(item.to_string());
                }
                "users" => {
                    // Save previous user
                    if let Some(user) = current_user.take() {
                        config.users.push(user);
                    }

                    // Start new user
                    if item.starts_with("name:") {
                        let name = item.strip_prefix("name:").unwrap().trim().to_string();
                        current_user = Some(UserConfig {
                            name,
                            ..Default::default()
                        });
                    } else if item == "default" {
                        // Default user placeholder
                    } else {
                        current_user = Some(UserConfig {
                            name: item.to_string(),
                            ..Default::default()
                        });
                    }
                }
                "write_files" => {
                    // Save previous file
                    if let Some(file) = current_file.take() {
                        config.write_files.push(file);
                    }

                    // Start new file
                    if item.starts_with("path:") {
                        let path = item.strip_prefix("path:").unwrap().trim().to_string();
                        current_file = Some(WriteFile {
                            path,
                            ..Default::default()
                        });
                    }
                }
                _ => {
                    list_items.push(item.to_string());
                }
            }
            continue;
        }

        // Nested properties
        let indent_level = line.len() - line.trim_start().len();

        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            let value = value.trim();

            // User properties
            if let Some(ref mut user) = current_user {
                match key {
                    "name" => user.name = value.to_string(),
                    "gecos" => user.gecos = Some(value.to_string()),
                    "shell" => user.shell = Some(value.to_string()),
                    "home" => user.home = Some(value.to_string()),
                    "passwd" | "password" => user.hashed_password = Some(value.to_string()),
                    "plain_text_passwd" => user.password = Some(value.to_string()),
                    "sudo" => user.sudo = Some(value.to_string()),
                    "lock_passwd" => user.lock_passwd = value == "true" || value.is_empty(),
                    "no_create_home" => user.no_create_home = value == "true",
                    "system" => user.system = value == "true",
                    "groups" => {
                        user.groups = value.split(',').map(|s| s.trim().to_string()).collect();
                    }
                    _ => {}
                }
            }

            // File properties
            if let Some(ref mut file) = current_file {
                match key {
                    "path" => file.path = value.to_string(),
                    "content" => file.content = value.to_string(),
                    "owner" => file.owner = Some(value.to_string()),
                    "permissions" => file.permissions = Some(value.to_string()),
                    "encoding" => {
                        file.encoding = match value {
                            "b64" | "base64" => WriteFileEncoding::Base64,
                            "gz" | "gzip" => WriteFileEncoding::Gzip,
                            "gz+b64" | "gzip+base64" => WriteFileEncoding::GzipBase64,
                            _ => WriteFileEncoding::Text,
                        };
                    }
                    "append" => file.append = value == "true",
                    "defer" => file.defer = value == "true",
                    _ => {}
                }
            }
        }
    }

    // Save final context
    save_section_context(&mut config, &current_section, &list_items, &current_user, &current_file);

    Ok(config)
}

fn save_section_context(
    config: &mut CloudConfig,
    section: &str,
    list_items: &[String],
    user: &Option<UserConfig>,
    file: &Option<WriteFile>,
) {
    if let Some(user) = user.clone() {
        config.users.push(user);
    }

    if let Some(file) = file.clone() {
        config.write_files.push(file);
    }

    // Handle list sections that weren't already processed
    match section {
        "mounts" => {
            // Parse mount entries
        }
        _ => {}
    }
}

/// Execute cloud-config
pub fn execute_cloud_config(config: &CloudConfig) -> Result<(), &'static str> {
    // 1. Set hostname
    if let Some(ref hostname) = config.hostname {
        set_hostname(hostname)?;
    }

    // 2. Set timezone
    if let Some(ref timezone) = config.timezone {
        set_timezone(timezone)?;
    }

    // 3. Write files (non-deferred)
    for file in &config.write_files {
        if !file.defer {
            write_file(file)?;
        }
    }

    // 4. Run bootcmd
    for cmd in &config.bootcmd {
        run_command(cmd)?;
    }

    // 5. Create users
    for user in &config.users {
        create_user(user)?;
    }

    // 6. Install packages
    if !config.packages.is_empty() {
        install_packages(&config.packages)?;
    }

    // 7. Configure swap
    if let Some(ref swap) = config.swap {
        configure_swap(swap)?;
    }

    // 8. Configure mounts
    for mount in &config.mounts {
        configure_mount(mount)?;
    }

    // 9. Run runcmd
    for cmd in &config.runcmd {
        run_command(cmd)?;
    }

    // 10. Write deferred files
    for file in &config.write_files {
        if file.defer {
            write_file(file)?;
        }
    }

    // 11. Phone home
    if let Some(ref phone_home) = config.phone_home {
        do_phone_home(phone_home)?;
    }

    // 12. Final message
    if let Some(ref msg) = config.final_message {
        crate::kprintln!("cloud-init: {}", msg);
    }

    // 13. Power state change
    if let Some(ref power_state) = config.power_state {
        schedule_power_state(power_state)?;
    }

    Ok(())
}

/// Set hostname
fn set_hostname(hostname: &str) -> Result<(), &'static str> {
    crate::kprintln!("cloud-init: Setting hostname to {}", hostname);
    // Would write to /etc/hostname and call sethostname()
    Ok(())
}

/// Set timezone
fn set_timezone(timezone: &str) -> Result<(), &'static str> {
    crate::kprintln!("cloud-init: Setting timezone to {}", timezone);
    // Would symlink /etc/localtime -> /usr/share/zoneinfo/{timezone}
    Ok(())
}

/// Write file
fn write_file(file: &WriteFile) -> Result<(), &'static str> {
    crate::kprintln!("cloud-init: Writing file {}", file.path);

    let content = match file.encoding {
        WriteFileEncoding::Text => file.content.as_bytes().to_vec(),
        WriteFileEncoding::Base64 => decode_base64(&file.content)?,
        WriteFileEncoding::Gzip => {
            // Would decompress gzip
            file.content.as_bytes().to_vec()
        }
        WriteFileEncoding::GzipBase64 => {
            let decoded = decode_base64(&file.content)?;
            // Would decompress gzip
            decoded
        }
    };

    // Would write content to file.path
    // Set owner if specified
    // Set permissions if specified

    let _ = content;
    Ok(())
}

/// Run command
fn run_command(cmd: &str) -> Result<(), &'static str> {
    crate::kprintln!("cloud-init: Running: {}", cmd);
    // Would execute command via shell
    Ok(())
}

/// Create user
fn create_user(user: &UserConfig) -> Result<(), &'static str> {
    crate::kprintln!("cloud-init: Creating user {}", user.name);

    // Would:
    // 1. useradd with options
    // 2. Set password if provided
    // 3. Add to groups
    // 4. Write authorized_keys
    // 5. Configure sudo

    Ok(())
}

/// Install packages
fn install_packages(packages: &[String]) -> Result<(), &'static str> {
    crate::kprintln!("cloud-init: Installing packages: {}", packages.join(", "));
    // Would call package manager
    Ok(())
}

/// Configure swap
fn configure_swap(swap: &SwapConfig) -> Result<(), &'static str> {
    if let Some(ref filename) = swap.filename {
        crate::kprintln!("cloud-init: Configuring swap at {}", filename);
    }
    Ok(())
}

/// Configure mount
fn configure_mount(mount: &MountEntry) -> Result<(), &'static str> {
    crate::kprintln!("cloud-init: Mounting {} at {}", mount.device, mount.mountpoint);
    Ok(())
}

/// Phone home
fn do_phone_home(config: &PhoneHomeConfig) -> Result<(), &'static str> {
    crate::kprintln!("cloud-init: Phoning home to {}", config.url);
    // Would HTTP POST instance data
    Ok(())
}

/// Schedule power state change
fn schedule_power_state(config: &PowerStateConfig) -> Result<(), &'static str> {
    let action = match config.mode {
        PowerStateMode::Poweroff => "poweroff",
        PowerStateMode::Reboot => "reboot",
        PowerStateMode::Halt => "halt",
    };
    crate::kprintln!("cloud-init: Scheduling {} in {}", action, config.delay);
    Ok(())
}

/// Decode base64
fn decode_base64(encoded: &str) -> Result<Vec<u8>, &'static str> {
    fn decode_char(c: char) -> Option<u8> {
        match c {
            'A'..='Z' => Some(c as u8 - b'A'),
            'a'..='z' => Some(c as u8 - b'a' + 26),
            '0'..='9' => Some(c as u8 - b'0' + 52),
            '+' => Some(62),
            '/' => Some(63),
            '=' => Some(0),
            _ => None,
        }
    }

    let mut result = Vec::new();
    let chars: Vec<char> = encoded.chars().filter(|c| !c.is_whitespace()).collect();

    for chunk in chars.chunks(4) {
        if chunk.len() < 4 {
            break;
        }

        let b0 = decode_char(chunk[0]).ok_or("Invalid base64")? as u32;
        let b1 = decode_char(chunk[1]).ok_or("Invalid base64")? as u32;
        let b2 = decode_char(chunk[2]).ok_or("Invalid base64")? as u32;
        let b3 = decode_char(chunk[3]).ok_or("Invalid base64")? as u32;

        let combined = (b0 << 18) | (b1 << 12) | (b2 << 6) | b3;

        result.push((combined >> 16) as u8);

        if chunk[2] != '=' {
            result.push((combined >> 8) as u8);
        }

        if chunk[3] != '=' {
            result.push(combined as u8);
        }
    }

    Ok(result)
}

/// Parse MIME multipart user-data
pub fn parse_multipart(data: &[u8]) -> Result<Vec<UserDataPart>, &'static str> {
    let text = core::str::from_utf8(data).map_err(|_| "Invalid UTF-8")?;

    let mut parts = Vec::new();
    let mut boundary = String::new();

    // Find boundary
    for line in text.lines() {
        if line.starts_with("Content-Type:") && line.contains("boundary=") {
            if let Some(b) = line.split("boundary=").nth(1) {
                boundary = b.trim_matches('"').to_string();
                break;
            }
        }
    }

    if boundary.is_empty() {
        return Err("No boundary found");
    }

    let boundary_marker = alloc::format!("--{}", boundary);
    let end_marker = alloc::format!("--{}--", boundary);

    let mut current_part: Option<UserDataPart> = None;
    let mut in_body = false;
    let mut body = String::new();

    for line in text.lines() {
        if line.starts_with(&end_marker) {
            if let Some(mut part) = current_part.take() {
                part.content = body.clone();
                parts.push(part);
            }
            break;
        }

        if line.starts_with(&boundary_marker) {
            if let Some(mut part) = current_part.take() {
                part.content = body.clone();
                parts.push(part);
            }

            current_part = Some(UserDataPart::default());
            in_body = false;
            body.clear();
            continue;
        }

        if let Some(ref mut part) = current_part {
            if !in_body {
                if line.is_empty() {
                    in_body = true;
                } else if line.starts_with("Content-Type:") {
                    part.content_type = line.strip_prefix("Content-Type:").unwrap().trim().to_string();
                } else if line.starts_with("Content-Disposition:") {
                    // Parse filename, etc.
                }
            } else {
                body.push_str(line);
                body.push('\n');
            }
        }
    }

    Ok(parts)
}

/// User data part from multipart
#[derive(Debug, Clone, Default)]
pub struct UserDataPart {
    pub content_type: String,
    pub content: String,
    pub filename: Option<String>,
}

impl UserDataPart {
    /// Get part type
    pub fn part_type(&self) -> UserDataPartType {
        match self.content_type.as_str() {
            "text/cloud-config" | "text/cloud-config; charset=utf-8" => UserDataPartType::CloudConfig,
            "text/x-shellscript" | "text/x-shellscript; charset=utf-8" => UserDataPartType::Script,
            "text/x-include-url" | "text/x-include-url; charset=utf-8" => UserDataPartType::IncludeUrl,
            "text/cloud-boothook" | "text/cloud-boothook; charset=utf-8" => UserDataPartType::BootHook,
            "text/jinja2" | "text/jinja2; charset=utf-8" => UserDataPartType::Jinja2,
            _ => UserDataPartType::Unknown,
        }
    }
}

/// User data part type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserDataPartType {
    CloudConfig,
    Script,
    IncludeUrl,
    BootHook,
    Jinja2,
    Unknown,
}

/// Process include URL list
pub fn process_include_urls(content: &str) -> Vec<String> {
    content
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
        .map(|line| line.trim().to_string())
        .collect()
}
