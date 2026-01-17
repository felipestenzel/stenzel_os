//! System Setup
//!
//! Provides functionality for initial system configuration:
//! - User account creation
//! - Timezone setup
//! - Keyboard layout
//! - Network configuration

extern crate alloc;

use alloc::string::String;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

/// Setup steps
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStep {
    Welcome,
    Language,
    Keyboard,
    Timezone,
    Network,
    User,
    Password,
    Summary,
    Complete,
}

/// User setup information
#[derive(Debug, Clone)]
pub struct UserSetup {
    /// Username
    pub username: String,
    /// Full name
    pub full_name: String,
    /// Password hash
    pub password_hash: String,
    /// Is administrator (wheel group)
    pub is_admin: bool,
    /// Auto-login
    pub auto_login: bool,
}

impl Default for UserSetup {
    fn default() -> Self {
        Self {
            username: String::from("user"),
            full_name: String::new(),
            password_hash: String::new(),
            is_admin: true,
            auto_login: false,
        }
    }
}

/// Timezone setup information
#[derive(Debug, Clone)]
pub struct TimezoneSetup {
    /// Timezone name (e.g., "America/Sao_Paulo")
    pub timezone: String,
    /// Use NTP
    pub use_ntp: bool,
    /// NTP servers
    pub ntp_servers: Vec<String>,
    /// UTC hardware clock
    pub utc_hardware_clock: bool,
}

impl Default for TimezoneSetup {
    fn default() -> Self {
        Self {
            timezone: String::from("UTC"),
            use_ntp: true,
            ntp_servers: vec![
                String::from("pool.ntp.org"),
                String::from("time.google.com"),
            ],
            utc_hardware_clock: true,
        }
    }
}

/// Keyboard setup information
#[derive(Debug, Clone)]
pub struct KeyboardSetup {
    /// Layout code (e.g., "us", "br")
    pub layout: String,
    /// Variant (e.g., "intl", "nodeadkeys")
    pub variant: String,
    /// Model (e.g., "pc105")
    pub model: String,
}

impl Default for KeyboardSetup {
    fn default() -> Self {
        Self {
            layout: String::from("us"),
            variant: String::new(),
            model: String::from("pc105"),
        }
    }
}

/// Network setup information
#[derive(Debug, Clone)]
pub struct NetworkSetup {
    /// Use DHCP
    pub use_dhcp: bool,
    /// Static IP (if not DHCP)
    pub ip_address: Option<String>,
    /// Netmask
    pub netmask: Option<String>,
    /// Gateway
    pub gateway: Option<String>,
    /// DNS servers
    pub dns_servers: Vec<String>,
    /// Hostname
    pub hostname: String,
}

impl Default for NetworkSetup {
    fn default() -> Self {
        Self {
            use_dhcp: true,
            ip_address: None,
            netmask: None,
            gateway: None,
            dns_servers: vec![
                String::from("8.8.8.8"),
                String::from("8.8.4.4"),
            ],
            hostname: String::from("stenzel"),
        }
    }
}

/// Available keyboard layouts
pub static KEYBOARD_LAYOUTS: &[(&str, &str)] = &[
    ("us", "English (US)"),
    ("us-intl", "English (US, International)"),
    ("gb", "English (UK)"),
    ("br", "Portuguese (Brazil ABNT2)"),
    ("pt", "Portuguese (Portugal)"),
    ("de", "German"),
    ("fr", "French"),
    ("es", "Spanish"),
    ("it", "Italian"),
    ("ru", "Russian"),
    ("jp", "Japanese"),
    ("kr", "Korean"),
    ("cn", "Chinese"),
];

/// Available timezones (partial list)
pub static TIMEZONES: &[(&str, &str)] = &[
    ("UTC", "UTC"),
    ("America/New_York", "Eastern Time (US)"),
    ("America/Chicago", "Central Time (US)"),
    ("America/Denver", "Mountain Time (US)"),
    ("America/Los_Angeles", "Pacific Time (US)"),
    ("America/Sao_Paulo", "Brasilia Time"),
    ("Europe/London", "London"),
    ("Europe/Paris", "Paris"),
    ("Europe/Berlin", "Berlin"),
    ("Europe/Moscow", "Moscow"),
    ("Asia/Tokyo", "Tokyo"),
    ("Asia/Shanghai", "Shanghai"),
    ("Asia/Singapore", "Singapore"),
    ("Australia/Sydney", "Sydney"),
];

/// Setup wizard
pub struct SetupWizard {
    target_root: String,
    current_step: SetupStep,
    user: UserSetup,
    timezone: TimezoneSetup,
    keyboard: KeyboardSetup,
    network: NetworkSetup,
}

impl SetupWizard {
    /// Create a new setup wizard
    pub fn new(target_root: &str) -> Self {
        Self {
            target_root: String::from(target_root),
            current_step: SetupStep::Welcome,
            user: UserSetup::default(),
            timezone: TimezoneSetup::default(),
            keyboard: KeyboardSetup::default(),
            network: NetworkSetup::default(),
        }
    }

    /// Get current step
    pub fn current_step(&self) -> SetupStep {
        self.current_step
    }

    /// Advance to next step
    pub fn next_step(&mut self) {
        self.current_step = match self.current_step {
            SetupStep::Welcome => SetupStep::Language,
            SetupStep::Language => SetupStep::Keyboard,
            SetupStep::Keyboard => SetupStep::Timezone,
            SetupStep::Timezone => SetupStep::Network,
            SetupStep::Network => SetupStep::User,
            SetupStep::User => SetupStep::Password,
            SetupStep::Password => SetupStep::Summary,
            SetupStep::Summary => SetupStep::Complete,
            SetupStep::Complete => SetupStep::Complete,
        };
    }

    /// Go back to previous step
    pub fn prev_step(&mut self) {
        self.current_step = match self.current_step {
            SetupStep::Welcome => SetupStep::Welcome,
            SetupStep::Language => SetupStep::Welcome,
            SetupStep::Keyboard => SetupStep::Language,
            SetupStep::Timezone => SetupStep::Keyboard,
            SetupStep::Network => SetupStep::Timezone,
            SetupStep::User => SetupStep::Network,
            SetupStep::Password => SetupStep::User,
            SetupStep::Summary => SetupStep::Password,
            SetupStep::Complete => SetupStep::Summary,
        };
    }

    /// Set hostname
    pub fn set_hostname(&self, hostname: &str) -> Result<(), String> {
        // Write /etc/hostname
        let hostname_file = format!("{}/etc/hostname", self.target_root);
        write_file(&hostname_file, &format!("{}\n", hostname))?;

        // Update /etc/hosts
        let hosts_file = format!("{}/etc/hosts", self.target_root);
        let hosts_content = format!(
            "127.0.0.1   localhost\n::1         localhost\n127.0.1.1   {}\n",
            hostname
        );
        write_file(&hosts_file, &hosts_content)?;

        crate::kprintln!("Set hostname: {}", hostname);
        Ok(())
    }

    /// Set timezone
    pub fn set_timezone(&self, timezone: &str) -> Result<(), String> {
        // Create symlink /etc/localtime -> /usr/share/zoneinfo/...
        let zoneinfo_path = format!("/usr/share/zoneinfo/{}", timezone);
        let localtime_path = format!("{}/etc/localtime", self.target_root);

        create_symlink(&zoneinfo_path, &localtime_path)?;

        // Write /etc/timezone
        let timezone_file = format!("{}/etc/timezone", self.target_root);
        write_file(&timezone_file, &format!("{}\n", timezone))?;

        crate::kprintln!("Set timezone: {}", timezone);
        Ok(())
    }

    /// Set keyboard layout
    pub fn set_keyboard_layout(&self, layout: &str) -> Result<(), String> {
        // Write /etc/vconsole.conf (for virtual console)
        let vconsole_file = format!("{}/etc/vconsole.conf", self.target_root);
        let vconsole_content = format!("KEYMAP={}\n", layout);
        write_file(&vconsole_file, &vconsole_content)?;

        // Write /etc/X11/xorg.conf.d/00-keyboard.conf (for X11)
        let xorg_dir = format!("{}/etc/X11/xorg.conf.d", self.target_root);
        create_directory(&xorg_dir)?;

        let xorg_keyboard = format!(
            "Section \"InputClass\"\n\
             \tIdentifier \"system-keyboard\"\n\
             \tMatchIsKeyboard \"on\"\n\
             \tOption \"XkbLayout\" \"{}\"\n\
             EndSection\n",
            layout
        );
        let xorg_keyboard_file = format!("{}/00-keyboard.conf", xorg_dir);
        write_file(&xorg_keyboard_file, &xorg_keyboard)?;

        // Write /etc/stenzel/keyboard.conf (for Stenzel GUI)
        let stenzel_dir = format!("{}/etc/stenzel", self.target_root);
        create_directory(&stenzel_dir)?;

        let stenzel_keyboard = format!("layout={}\n", layout);
        let stenzel_keyboard_file = format!("{}/keyboard.conf", stenzel_dir);
        write_file(&stenzel_keyboard_file, &stenzel_keyboard)?;

        crate::kprintln!("Set keyboard layout: {}", layout);
        Ok(())
    }

    /// Configure network
    pub fn configure_network(&self, config: &NetworkSetup) -> Result<(), String> {
        // Write /etc/stenzel/network.conf
        let network_conf = format!("{}/etc/stenzel/network.conf", self.target_root);

        let mut content = String::new();
        content.push_str(&format!("hostname={}\n", config.hostname));
        content.push_str(&format!("dhcp={}\n", config.use_dhcp));

        if !config.use_dhcp {
            if let Some(ref ip) = config.ip_address {
                content.push_str(&format!("address={}\n", ip));
            }
            if let Some(ref mask) = config.netmask {
                content.push_str(&format!("netmask={}\n", mask));
            }
            if let Some(ref gw) = config.gateway {
                content.push_str(&format!("gateway={}\n", gw));
            }
        }

        for (i, dns) in config.dns_servers.iter().enumerate() {
            content.push_str(&format!("dns{}={}\n", i + 1, dns));
        }

        write_file(&network_conf, &content)?;

        // Write /etc/resolv.conf
        let resolv_conf = format!("{}/etc/resolv.conf", self.target_root);
        let mut resolv_content = String::new();
        for dns in &config.dns_servers {
            resolv_content.push_str(&format!("nameserver {}\n", dns));
        }
        write_file(&resolv_conf, &resolv_content)?;

        crate::kprintln!("Configured network (DHCP: {})", config.use_dhcp);
        Ok(())
    }

    /// Set root password
    pub fn set_root_password(&self, password_hash: &str) -> Result<(), String> {
        // Update /etc/shadow
        let shadow_file = format!("{}/etc/shadow", self.target_root);

        // Read existing shadow or create new
        let shadow_content = format!(
            "root:{}:19000:0:99999:7:::\n",
            password_hash
        );

        write_file(&shadow_file, &shadow_content)?;
        set_permissions(&shadow_file, 0o600)?;

        crate::kprintln!("Set root password");
        Ok(())
    }

    /// Create user account
    pub fn create_user(&self, username: &str, password_hash: &str) -> Result<(), String> {
        // Get next UID (start at 1000)
        let uid = 1000u32;
        let gid = 1000u32;

        // Create home directory
        let home_dir = format!("{}/home/{}", self.target_root, username);
        create_directory(&home_dir)?;

        // Add to /etc/passwd
        let passwd_file = format!("{}/etc/passwd", self.target_root);
        let passwd_entry = format!(
            "{}:x:{}:{}::/home/{}:/bin/sh\n",
            username, uid, gid, username
        );
        append_file(&passwd_file, &passwd_entry)?;

        // Add to /etc/shadow
        let shadow_file = format!("{}/etc/shadow", self.target_root);
        let shadow_entry = format!(
            "{}:{}:19000:0:99999:7:::\n",
            username, password_hash
        );
        append_file(&shadow_file, &shadow_entry)?;

        // Add to /etc/group (create user group)
        let group_file = format!("{}/etc/group", self.target_root);
        let group_entry = format!("{}:x:{}:\n", username, gid);
        append_file(&group_file, &group_entry)?;

        // Copy skeleton files
        copy_skel(&home_dir)?;

        // Set home directory ownership
        set_owner(&home_dir, uid, gid)?;

        crate::kprintln!("Created user: {} (UID: {})", username, uid);
        Ok(())
    }

    /// Add user to group
    pub fn add_user_to_group(&self, username: &str, group: &str) -> Result<(), String> {
        let group_file = format!("{}/etc/group", self.target_root);

        // Read group file and find the group
        let content = read_file(&group_file)?;
        let mut new_content = String::new();

        for line in content.lines() {
            if line.starts_with(&format!("{}:", group)) {
                // Add user to group
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 4 {
                    let members = parts[3];
                    let new_members = if members.is_empty() {
                        String::from(username)
                    } else {
                        format!("{},{}", members, username)
                    };
                    new_content.push_str(&format!("{}:{}:{}:{}\n",
                        parts[0], parts[1], parts[2], new_members));
                } else {
                    new_content.push_str(line);
                    new_content.push('\n');
                }
            } else {
                new_content.push_str(line);
                new_content.push('\n');
            }
        }

        write_file(&group_file, &new_content)?;

        crate::kprintln!("Added {} to group {}", username, group);
        Ok(())
    }

    /// Generate fstab
    pub fn generate_fstab(&self, target: &super::InstallTarget) -> Result<(), String> {
        let fstab_file = format!("{}/etc/fstab", self.target_root);

        let mut fstab = String::new();
        fstab.push_str("# /etc/fstab: static file system information\n");
        fstab.push_str("# <file system>  <mount point>  <type>  <options>  <dump>  <pass>\n\n");

        // Root partition
        fstab.push_str("LABEL=stenzel_root  /         ext4    defaults,errors=remount-ro  0  1\n");

        // EFI partition (if UEFI)
        fstab.push_str("LABEL=EFI           /boot/efi vfat    umask=0077                  0  1\n");

        // Swap (if present)
        fstab.push_str("LABEL=swap          none      swap    sw                          0  0\n");

        // Virtual filesystems
        fstab.push_str("\n# Virtual filesystems\n");
        fstab.push_str("proc                /proc     proc    defaults                    0  0\n");
        fstab.push_str("sysfs               /sys      sysfs   defaults                    0  0\n");
        fstab.push_str("devpts              /dev/pts  devpts  defaults                    0  0\n");
        fstab.push_str("tmpfs               /tmp      tmpfs   defaults                    0  0\n");
        fstab.push_str("tmpfs               /run      tmpfs   defaults                    0  0\n");

        write_file(&fstab_file, &fstab)?;

        crate::kprintln!("Generated /etc/fstab");
        Ok(())
    }

    /// Configure auto-login
    pub fn configure_autologin(&self, username: &str) -> Result<(), String> {
        let autologin_file = format!("{}/etc/stenzel/autologin.conf", self.target_root);
        write_file(&autologin_file, &format!("user={}\n", username))?;

        crate::kprintln!("Configured auto-login for {}", username);
        Ok(())
    }

    /// Set locale
    pub fn set_locale(&self, locale: &str) -> Result<(), String> {
        // Write /etc/locale.conf
        let locale_file = format!("{}/etc/locale.conf", self.target_root);
        let locale_content = format!("LANG={}\n", locale);
        write_file(&locale_file, &locale_content)?;

        crate::kprintln!("Set locale: {}", locale);
        Ok(())
    }

    /// Apply all configuration
    pub fn apply(&self) -> Result<(), String> {
        self.set_hostname(&self.network.hostname)?;
        self.set_timezone(&self.timezone.timezone)?;
        self.set_keyboard_layout(&self.keyboard.layout)?;
        self.configure_network(&self.network)?;
        self.create_user(&self.user.username, &self.user.password_hash)?;

        if self.user.is_admin {
            self.add_user_to_group(&self.user.username, "wheel")?;
        }

        if self.user.auto_login {
            self.configure_autologin(&self.user.username)?;
        }

        Ok(())
    }

    /// Validate username
    pub fn validate_username(username: &str) -> Result<(), String> {
        if username.is_empty() {
            return Err(String::from("Username cannot be empty"));
        }

        if username.len() > 32 {
            return Err(String::from("Username too long (max 32 characters)"));
        }

        // Must start with lowercase letter
        if !username.chars().next().unwrap().is_ascii_lowercase() {
            return Err(String::from("Username must start with a lowercase letter"));
        }

        // Only lowercase, digits, underscore, hyphen
        for c in username.chars() {
            if !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '_' && c != '-' {
                return Err(String::from("Username contains invalid characters"));
            }
        }

        // Reserved names
        let reserved = ["root", "admin", "daemon", "bin", "sys", "sync", "games",
                        "man", "mail", "news", "uucp", "proxy", "www-data", "backup",
                        "list", "irc", "gnats", "nobody", "systemd-network", "systemd-resolve"];

        if reserved.contains(&username) {
            return Err(String::from("Username is reserved"));
        }

        Ok(())
    }

    /// Validate password strength
    pub fn validate_password(password: &str) -> Result<(), String> {
        if password.len() < 8 {
            return Err(String::from("Password must be at least 8 characters"));
        }

        let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
        let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
        let has_digit = password.chars().any(|c| c.is_ascii_digit());

        if !has_upper || !has_lower || !has_digit {
            return Err(String::from(
                "Password must contain uppercase, lowercase, and digits"
            ));
        }

        Ok(())
    }

    /// Hash password
    pub fn hash_password(password: &str) -> String {
        // Would use SHA-512 crypt or similar
        // For now, use simple marker
        format!("$6$rounds=5000${}", password)
    }
}

// ============================================================================
// Helper functions
// ============================================================================

fn write_file(path: &str, content: &str) -> Result<(), String> {
    crate::kprintln!("Writing: {}", path);
    Ok(())
}

fn read_file(_path: &str) -> Result<String, String> {
    Ok(String::new())
}

fn append_file(path: &str, content: &str) -> Result<(), String> {
    crate::kprintln!("Appending to: {}", path);
    Ok(())
}

fn create_directory(path: &str) -> Result<(), String> {
    crate::kprintln!("Creating directory: {}", path);
    Ok(())
}

fn create_symlink(target: &str, link: &str) -> Result<(), String> {
    crate::kprintln!("Creating symlink: {} -> {}", link, target);
    Ok(())
}

fn set_permissions(path: &str, mode: u32) -> Result<(), String> {
    Ok(())
}

fn set_owner(path: &str, uid: u32, gid: u32) -> Result<(), String> {
    crate::kprintln!("Setting owner of {} to {}:{}", path, uid, gid);
    Ok(())
}

fn copy_skel(home_dir: &str) -> Result<(), String> {
    // Copy /etc/skel/* to home directory
    crate::kprintln!("Copying skeleton to {}", home_dir);
    Ok(())
}
