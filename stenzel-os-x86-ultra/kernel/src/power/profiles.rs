//! Power Profiles
//!
//! Implements power profiles that control CPU frequency, display brightness,
//! and other power-related settings.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use crate::sync::IrqSafeMutex;
use super::{PowerProfile, cpufreq};

/// Profile settings
#[derive(Debug, Clone)]
pub struct ProfileSettings {
    /// Profile name
    pub name: String,
    /// CPU settings
    pub cpu: CpuSettings,
    /// Display settings
    pub display: DisplaySettings,
    /// Disk settings
    pub disk: DiskSettings,
    /// Wireless settings
    pub wireless: WirelessSettings,
}

/// CPU power settings
#[derive(Debug, Clone)]
pub struct CpuSettings {
    /// Minimum frequency percentage (0-100)
    pub min_freq_percent: u8,
    /// Maximum frequency percentage (0-100)
    pub max_freq_percent: u8,
    /// Turbo boost enabled
    pub turbo_enabled: bool,
    /// Core parking enabled
    pub core_parking: bool,
    /// Max cores active (0 = no limit)
    pub max_cores: u8,
}

/// Display power settings
#[derive(Debug, Clone)]
pub struct DisplaySettings {
    /// Screen brightness (0-100)
    pub brightness: u8,
    /// Dim display after N seconds (0 = disabled)
    pub dim_timeout: u32,
    /// Turn off display after N seconds (0 = disabled)
    pub off_timeout: u32,
    /// Adaptive brightness enabled
    pub adaptive_brightness: bool,
}

/// Disk power settings
#[derive(Debug, Clone)]
pub struct DiskSettings {
    /// Spin down after N seconds (0 = never)
    pub spindown_timeout: u32,
    /// APM level (1-255, 255 = disabled)
    pub apm_level: u8,
    /// Write cache enabled
    pub write_cache: bool,
}

/// Wireless power settings
#[derive(Debug, Clone)]
pub struct WirelessSettings {
    /// WiFi power save mode
    pub wifi_power_save: bool,
    /// Bluetooth power save
    pub bluetooth_power_save: bool,
}

impl Default for ProfileSettings {
    fn default() -> Self {
        ProfileSettings {
            name: String::from("Balanced"),
            cpu: CpuSettings {
                min_freq_percent: 10,
                max_freq_percent: 100,
                turbo_enabled: true,
                core_parking: false,
                max_cores: 0,
            },
            display: DisplaySettings {
                brightness: 70,
                dim_timeout: 120,
                off_timeout: 300,
                adaptive_brightness: true,
            },
            disk: DiskSettings {
                spindown_timeout: 0,
                apm_level: 255,
                write_cache: true,
            },
            wireless: WirelessSettings {
                wifi_power_save: true,
                bluetooth_power_save: true,
            },
        }
    }
}

/// Global profile manager
pub static PROFILE_MANAGER: IrqSafeMutex<ProfileManager> = IrqSafeMutex::new(ProfileManager::new());

/// Profile manager
pub struct ProfileManager {
    /// Current profile
    current: PowerProfile,
    /// Profile settings
    profiles: [ProfileSettings; 4],
    /// On AC power profile
    ac_profile: PowerProfile,
    /// On battery profile
    battery_profile: PowerProfile,
    /// Automatic profile switching
    auto_switch: bool,
    /// Initialized
    initialized: bool,
}

impl ProfileManager {
    pub const fn new() -> Self {
        ProfileManager {
            current: PowerProfile::Balanced,
            profiles: [
                // Performance
                ProfileSettings {
                    name: String::new(),
                    cpu: CpuSettings {
                        min_freq_percent: 50,
                        max_freq_percent: 100,
                        turbo_enabled: true,
                        core_parking: false,
                        max_cores: 0,
                    },
                    display: DisplaySettings {
                        brightness: 100,
                        dim_timeout: 0,
                        off_timeout: 0,
                        adaptive_brightness: false,
                    },
                    disk: DiskSettings {
                        spindown_timeout: 0,
                        apm_level: 255,
                        write_cache: true,
                    },
                    wireless: WirelessSettings {
                        wifi_power_save: false,
                        bluetooth_power_save: false,
                    },
                },
                // Balanced
                ProfileSettings {
                    name: String::new(),
                    cpu: CpuSettings {
                        min_freq_percent: 10,
                        max_freq_percent: 100,
                        turbo_enabled: true,
                        core_parking: false,
                        max_cores: 0,
                    },
                    display: DisplaySettings {
                        brightness: 70,
                        dim_timeout: 120,
                        off_timeout: 300,
                        adaptive_brightness: true,
                    },
                    disk: DiskSettings {
                        spindown_timeout: 0,
                        apm_level: 255,
                        write_cache: true,
                    },
                    wireless: WirelessSettings {
                        wifi_power_save: true,
                        bluetooth_power_save: true,
                    },
                },
                // Power Saver
                ProfileSettings {
                    name: String::new(),
                    cpu: CpuSettings {
                        min_freq_percent: 5,
                        max_freq_percent: 70,
                        turbo_enabled: false,
                        core_parking: true,
                        max_cores: 0,
                    },
                    display: DisplaySettings {
                        brightness: 50,
                        dim_timeout: 60,
                        off_timeout: 180,
                        adaptive_brightness: true,
                    },
                    disk: DiskSettings {
                        spindown_timeout: 300,
                        apm_level: 128,
                        write_cache: true,
                    },
                    wireless: WirelessSettings {
                        wifi_power_save: true,
                        bluetooth_power_save: true,
                    },
                },
                // Battery Saver
                ProfileSettings {
                    name: String::new(),
                    cpu: CpuSettings {
                        min_freq_percent: 5,
                        max_freq_percent: 50,
                        turbo_enabled: false,
                        core_parking: true,
                        max_cores: 2,
                    },
                    display: DisplaySettings {
                        brightness: 30,
                        dim_timeout: 30,
                        off_timeout: 60,
                        adaptive_brightness: true,
                    },
                    disk: DiskSettings {
                        spindown_timeout: 120,
                        apm_level: 64,
                        write_cache: true,
                    },
                    wireless: WirelessSettings {
                        wifi_power_save: true,
                        bluetooth_power_save: true,
                    },
                },
            ],
            ac_profile: PowerProfile::Balanced,
            battery_profile: PowerProfile::PowerSaver,
            auto_switch: true,
            initialized: false,
        }
    }

    /// Initialize profile manager
    pub fn init(&mut self) {
        // Set profile names
        self.profiles[0].name = String::from("Performance");
        self.profiles[1].name = String::from("Balanced");
        self.profiles[2].name = String::from("Power Saver");
        self.profiles[3].name = String::from("Battery Saver");

        self.initialized = true;
        crate::kprintln!("profiles: manager initialized");
    }

    /// Get current profile
    pub fn current(&self) -> PowerProfile {
        self.current
    }

    /// Get profile settings
    pub fn get_settings(&self, profile: PowerProfile) -> &ProfileSettings {
        &self.profiles[profile as usize]
    }

    /// Set profile
    pub fn set_profile(&mut self, profile: PowerProfile) {
        self.current = profile;
        let settings = &self.profiles[profile as usize];

        crate::kprintln!("profiles: switching to {}", settings.name);

        // Apply CPU settings
        self.apply_cpu_settings(&settings.cpu);

        // Apply display settings
        self.apply_display_settings(&settings.display);

        // Apply disk settings
        self.apply_disk_settings(&settings.disk);

        // Apply wireless settings
        self.apply_wireless_settings(&settings.wireless);

        // Apply to cpufreq
        cpufreq::apply_profile(profile);
    }

    /// Set AC power profile
    pub fn set_ac_profile(&mut self, profile: PowerProfile) {
        self.ac_profile = profile;
    }

    /// Set battery profile
    pub fn set_battery_profile(&mut self, profile: PowerProfile) {
        self.battery_profile = profile;
    }

    /// Enable/disable automatic profile switching
    pub fn set_auto_switch(&mut self, enabled: bool) {
        self.auto_switch = enabled;
    }

    /// Handle power source change
    pub fn handle_power_source_change(&mut self, on_ac: bool) {
        if !self.auto_switch {
            return;
        }

        let new_profile = if on_ac {
            self.ac_profile
        } else {
            self.battery_profile
        };

        if new_profile != self.current {
            self.set_profile(new_profile);
        }
    }

    /// Apply CPU settings
    fn apply_cpu_settings(&self, settings: &CpuSettings) {
        let mut cpufreq = cpufreq::CPUFREQ.lock();

        cpufreq.set_turbo(settings.turbo_enabled);

        // Set frequency limits for all CPUs
        for i in 0..cpufreq.cpus.len() {
            if let Some(cpu) = cpufreq.cpus.get(i) {
                let range = cpu.max_freq - cpu.min_freq;
                let min = cpu.min_freq + (range * settings.min_freq_percent as u32 / 100);
                let max = cpu.min_freq + (range * settings.max_freq_percent as u32 / 100);
                let _ = cpufreq.set_scaling_limits(i as u32, min, max);
            }
        }
    }

    /// Apply display settings
    fn apply_display_settings(&self, settings: &DisplaySettings) {
        // Set brightness
        let _ = crate::drivers::backlight::set_brightness(settings.brightness as u32);

        // Configure timeouts
        // TODO: Integrate with display timeout system
    }

    /// Apply disk settings
    fn apply_disk_settings(&self, _settings: &DiskSettings) {
        // Set APM level
        // TODO: Integrate with storage subsystem
    }

    /// Apply wireless settings
    fn apply_wireless_settings(&self, _settings: &WirelessSettings) {
        // Configure power save modes
        // TODO: Integrate with WiFi and Bluetooth drivers
    }
}

/// Initialize profiles subsystem
pub fn init() {
    PROFILE_MANAGER.lock().init();
}

/// Get current profile
pub fn current_profile() -> PowerProfile {
    PROFILE_MANAGER.lock().current()
}

/// Set profile
pub fn set_profile(profile: PowerProfile) {
    PROFILE_MANAGER.lock().set_profile(profile);
}

/// Handle power source change
pub fn handle_power_source_change(on_ac: bool) {
    PROFILE_MANAGER.lock().handle_power_source_change(on_ac);
}
