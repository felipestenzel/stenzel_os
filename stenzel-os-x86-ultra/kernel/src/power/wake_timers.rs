//! Wake Timers Support
//!
//! Allows scheduling system wake from suspend/hibernate at specific times:
//! - RTC alarm-based wake
//! - ACPI timer wake
//! - Scheduled task wake
//! - Recurring wake schedules

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// Wake timer type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeTimerType {
    /// One-time wake at specific time
    OneShot,
    /// Daily recurring wake
    Daily,
    /// Weekly recurring wake
    Weekly,
    /// Custom interval
    Interval,
}

/// Wake timer state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeTimerState {
    /// Timer is disabled
    Disabled,
    /// Timer is active and scheduled
    Active,
    /// Timer has expired
    Expired,
    /// Timer is paused
    Paused,
    /// Error occurred
    Error,
}

/// Wake timer source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeSource {
    /// RTC alarm
    Rtc,
    /// ACPI timer
    Acpi,
    /// HPET timer
    Hpet,
}

/// Days of week for recurring timers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DayOfWeek {
    Sunday = 0,
    Monday = 1,
    Tuesday = 2,
    Wednesday = 3,
    Thursday = 4,
    Friday = 5,
    Saturday = 6,
}

/// Wake timer schedule
#[derive(Debug, Clone)]
pub struct WakeSchedule {
    /// Timer type
    pub timer_type: WakeTimerType,
    /// Wake time (Unix timestamp or relative seconds)
    pub wake_time: u64,
    /// For recurring: hour of day (0-23)
    pub hour: u8,
    /// For recurring: minute of hour (0-59)
    pub minute: u8,
    /// For weekly: days of week (bitmask)
    pub days_of_week: u8,
    /// For interval: interval in seconds
    pub interval_secs: u64,
}

impl Default for WakeSchedule {
    fn default() -> Self {
        WakeSchedule {
            timer_type: WakeTimerType::OneShot,
            wake_time: 0,
            hour: 0,
            minute: 0,
            days_of_week: 0x7F, // All days
            interval_secs: 0,
        }
    }
}

/// Wake timer configuration
#[derive(Debug, Clone)]
pub struct WakeTimer {
    /// Timer ID
    pub id: u32,
    /// Timer name/description
    pub name: String,
    /// Current state
    pub state: WakeTimerState,
    /// Schedule
    pub schedule: WakeSchedule,
    /// Wake source to use
    pub source: WakeSource,
    /// Created timestamp
    pub created: u64,
    /// Last triggered timestamp
    pub last_triggered: u64,
    /// Trigger count
    pub trigger_count: u32,
    /// Associated process ID (if any)
    pub owner_pid: Option<u32>,
    /// Run command on wake
    pub command: Option<String>,
}

impl WakeTimer {
    pub fn new(id: u32, name: String, schedule: WakeSchedule) -> Self {
        WakeTimer {
            id,
            name,
            state: WakeTimerState::Disabled,
            schedule,
            source: WakeSource::Rtc,
            created: crate::time::uptime_ms(),
            last_triggered: 0,
            trigger_count: 0,
            owner_pid: None,
            command: None,
        }
    }
}

/// Wake timer statistics
#[derive(Debug, Default)]
pub struct WakeTimerStats {
    /// Total timers created
    pub timers_created: AtomicU64,
    /// Currently active timers
    pub active_timers: AtomicU64,
    /// Total wake events
    pub wake_events: AtomicU64,
    /// Missed wakes (couldn't wake in time)
    pub missed_wakes: AtomicU64,
    /// Last wake timestamp
    pub last_wake: AtomicU64,
}

/// Wake timer manager
pub struct WakeTimerManager {
    /// Registered timers
    timers: Vec<WakeTimer>,
    /// Next timer ID
    next_id: u32,
    /// RTC alarm enabled
    rtc_enabled: bool,
    /// ACPI timer enabled
    acpi_enabled: bool,
    /// Statistics
    stats: WakeTimerStats,
    /// Callbacks for wake events
    callbacks: Vec<fn(u32)>,
    /// Initialized
    initialized: bool,
}

pub static WAKE_TIMER_MANAGER: IrqSafeMutex<WakeTimerManager> = IrqSafeMutex::new(WakeTimerManager::new());

impl WakeTimerManager {
    pub const fn new() -> Self {
        WakeTimerManager {
            timers: Vec::new(),
            next_id: 1,
            rtc_enabled: false,
            acpi_enabled: false,
            stats: WakeTimerStats {
                timers_created: AtomicU64::new(0),
                active_timers: AtomicU64::new(0),
                wake_events: AtomicU64::new(0),
                missed_wakes: AtomicU64::new(0),
                last_wake: AtomicU64::new(0),
            },
            callbacks: Vec::new(),
            initialized: false,
        }
    }

    /// Initialize the wake timer manager
    pub fn init(&mut self) -> KResult<()> {
        if self.initialized {
            return Ok(());
        }

        // Check for RTC alarm support
        self.rtc_enabled = self.probe_rtc_alarm();

        // Check for ACPI timer support
        self.acpi_enabled = self.probe_acpi_timer();

        self.initialized = true;
        crate::kprintln!("wake_timers: initialized (RTC={}, ACPI={})",
            self.rtc_enabled, self.acpi_enabled);
        Ok(())
    }

    /// Probe for RTC alarm support
    fn probe_rtc_alarm(&self) -> bool {
        // Check if RTC supports alarm functionality
        // Most x86 systems have RTC alarm capability
        unsafe {
            use x86_64::instructions::port::Port;

            // Read RTC status register B
            let mut addr_port: Port<u8> = Port::new(0x70);
            let mut data_port: Port<u8> = Port::new(0x71);

            addr_port.write(0x0B);
            let status_b = data_port.read();

            // Check AIE (Alarm Interrupt Enable) capability
            // If we can read the register, assume RTC is present
            let _ = status_b;
            true
        }
    }

    /// Probe for ACPI timer support
    fn probe_acpi_timer(&self) -> bool {
        // Check for ACPI PM timer
        // This would query ACPI tables
        true // Assume available for now
    }

    /// Create a new wake timer
    pub fn create_timer(&mut self, name: String, schedule: WakeSchedule) -> KResult<u32> {
        let id = self.next_id;
        self.next_id += 1;

        let timer = WakeTimer::new(id, name, schedule);
        self.timers.push(timer);

        self.stats.timers_created.fetch_add(1, Ordering::Relaxed);
        crate::kprintln!("wake_timers: created timer {} (id={})",
            self.timers.last().map(|t| t.name.as_str()).unwrap_or(""), id);

        Ok(id)
    }

    /// Create a one-shot wake timer
    pub fn create_oneshot(&mut self, name: String, wake_time: u64) -> KResult<u32> {
        let schedule = WakeSchedule {
            timer_type: WakeTimerType::OneShot,
            wake_time,
            ..Default::default()
        };
        self.create_timer(name, schedule)
    }

    /// Create a daily recurring wake timer
    pub fn create_daily(&mut self, name: String, hour: u8, minute: u8) -> KResult<u32> {
        if hour > 23 || minute > 59 {
            return Err(KError::Invalid);
        }

        let schedule = WakeSchedule {
            timer_type: WakeTimerType::Daily,
            hour,
            minute,
            days_of_week: 0x7F, // All days
            ..Default::default()
        };
        self.create_timer(name, schedule)
    }

    /// Create a weekly recurring wake timer
    pub fn create_weekly(&mut self, name: String, hour: u8, minute: u8, days: u8) -> KResult<u32> {
        if hour > 23 || minute > 59 {
            return Err(KError::Invalid);
        }

        let schedule = WakeSchedule {
            timer_type: WakeTimerType::Weekly,
            hour,
            minute,
            days_of_week: days,
            ..Default::default()
        };
        self.create_timer(name, schedule)
    }

    /// Create an interval-based wake timer
    pub fn create_interval(&mut self, name: String, interval_secs: u64) -> KResult<u32> {
        if interval_secs == 0 {
            return Err(KError::Invalid);
        }

        let schedule = WakeSchedule {
            timer_type: WakeTimerType::Interval,
            interval_secs,
            ..Default::default()
        };
        self.create_timer(name, schedule)
    }

    /// Enable a timer
    pub fn enable_timer(&mut self, id: u32) -> KResult<()> {
        let timer = self.timers.iter_mut().find(|t| t.id == id)
            .ok_or(KError::NotFound)?;

        if timer.state == WakeTimerState::Active {
            return Ok(());
        }

        timer.state = WakeTimerState::Active;
        self.stats.active_timers.fetch_add(1, Ordering::Relaxed);

        // Program the hardware
        self.program_next_wake()?;

        crate::kprintln!("wake_timers: enabled timer {}", id);
        Ok(())
    }

    /// Disable a timer
    pub fn disable_timer(&mut self, id: u32) -> KResult<()> {
        let timer = self.timers.iter_mut().find(|t| t.id == id)
            .ok_or(KError::NotFound)?;

        if timer.state != WakeTimerState::Active {
            return Ok(());
        }

        timer.state = WakeTimerState::Disabled;
        self.stats.active_timers.fetch_sub(1, Ordering::Relaxed);

        // Reprogram hardware if needed
        self.program_next_wake()?;

        crate::kprintln!("wake_timers: disabled timer {}", id);
        Ok(())
    }

    /// Delete a timer
    pub fn delete_timer(&mut self, id: u32) -> KResult<()> {
        let pos = self.timers.iter().position(|t| t.id == id)
            .ok_or(KError::NotFound)?;

        let timer = self.timers.remove(pos);
        if timer.state == WakeTimerState::Active {
            self.stats.active_timers.fetch_sub(1, Ordering::Relaxed);
            self.program_next_wake()?;
        }

        crate::kprintln!("wake_timers: deleted timer {}", id);
        Ok(())
    }

    /// Get timer info
    pub fn get_timer(&self, id: u32) -> Option<&WakeTimer> {
        self.timers.iter().find(|t| t.id == id)
    }

    /// List all timers
    pub fn list_timers(&self) -> &[WakeTimer] {
        &self.timers
    }

    /// Get next scheduled wake time
    pub fn next_wake_time(&self) -> Option<u64> {
        let now = crate::time::realtime().tv_sec as u64;

        self.timers.iter()
            .filter(|t| t.state == WakeTimerState::Active)
            .filter_map(|t| self.calculate_next_wake(t, now))
            .min()
    }

    /// Calculate next wake time for a timer
    fn calculate_next_wake(&self, timer: &WakeTimer, now: u64) -> Option<u64> {
        match timer.schedule.timer_type {
            WakeTimerType::OneShot => {
                if timer.schedule.wake_time > now {
                    Some(timer.schedule.wake_time)
                } else {
                    None
                }
            }
            WakeTimerType::Daily => {
                // Calculate next occurrence
                let secs_in_day = 24 * 60 * 60;
                let target_secs = timer.schedule.hour as u64 * 3600 + timer.schedule.minute as u64 * 60;
                let day_start = (now / secs_in_day) * secs_in_day;
                let mut next = day_start + target_secs;
                if next <= now {
                    next += secs_in_day;
                }
                Some(next)
            }
            WakeTimerType::Weekly => {
                // Find next matching day
                let secs_in_day = 24 * 60 * 60;
                let target_secs = timer.schedule.hour as u64 * 3600 + timer.schedule.minute as u64 * 60;

                // Calculate current day of week (0 = Thursday for Unix epoch)
                let days_since_epoch = now / secs_in_day;
                let current_dow = ((days_since_epoch + 4) % 7) as u8; // Adjust to Sunday = 0

                for offset in 0..7 {
                    let check_dow = (current_dow + offset) % 7;
                    if timer.schedule.days_of_week & (1 << check_dow) != 0 {
                        let day_start = (now / secs_in_day + offset as u64) * secs_in_day;
                        let next = day_start + target_secs;
                        if next > now {
                            return Some(next);
                        }
                    }
                }
                None
            }
            WakeTimerType::Interval => {
                let base = if timer.last_triggered > 0 {
                    timer.last_triggered
                } else {
                    timer.created / 1000 // Convert from ms
                };
                Some(base + timer.schedule.interval_secs)
            }
        }
    }

    /// Program the next wake time into hardware
    fn program_next_wake(&mut self) -> KResult<()> {
        let next_wake = match self.next_wake_time() {
            Some(t) => t,
            None => {
                // No active timers, disable hardware alarm
                self.disable_rtc_alarm()?;
                return Ok(());
            }
        };

        // Program RTC alarm
        if self.rtc_enabled {
            self.set_rtc_alarm(next_wake)?;
        }

        Ok(())
    }

    /// Set RTC alarm time
    fn set_rtc_alarm(&self, wake_time: u64) -> KResult<()> {
        // Convert Unix timestamp to RTC format
        // RTC uses BCD format for time
        let secs_in_day = 24 * 60 * 60;
        let time_of_day = wake_time % secs_in_day;
        let hour = (time_of_day / 3600) as u8;
        let minute = ((time_of_day % 3600) / 60) as u8;
        let second = (time_of_day % 60) as u8;

        unsafe {
            use x86_64::instructions::port::Port;

            let mut addr_port: Port<u8> = Port::new(0x70);
            let mut data_port: Port<u8> = Port::new(0x71);

            // Disable NMI while programming
            // Set alarm seconds
            addr_port.write(0x01);
            data_port.write(self.to_bcd(second));

            // Set alarm minutes
            addr_port.write(0x03);
            data_port.write(self.to_bcd(minute));

            // Set alarm hours
            addr_port.write(0x05);
            data_port.write(self.to_bcd(hour));

            // Enable alarm interrupt in Status Register B
            addr_port.write(0x0B);
            let status_b = data_port.read();
            addr_port.write(0x0B);
            data_port.write(status_b | 0x20); // Set AIE bit
        }

        crate::kprintln!("wake_timers: RTC alarm set for {:02}:{:02}:{:02}", hour, minute, second);
        Ok(())
    }

    /// Disable RTC alarm
    fn disable_rtc_alarm(&self) -> KResult<()> {
        unsafe {
            use x86_64::instructions::port::Port;

            let mut addr_port: Port<u8> = Port::new(0x70);
            let mut data_port: Port<u8> = Port::new(0x71);

            // Disable alarm interrupt in Status Register B
            addr_port.write(0x0B);
            let status_b = data_port.read();
            addr_port.write(0x0B);
            data_port.write(status_b & !0x20); // Clear AIE bit
        }

        Ok(())
    }

    /// Convert to BCD format
    fn to_bcd(&self, val: u8) -> u8 {
        ((val / 10) << 4) | (val % 10)
    }

    /// Handle RTC alarm interrupt (wake occurred)
    pub fn handle_rtc_alarm(&mut self) {
        let now = crate::time::realtime().tv_sec as u64;
        self.stats.last_wake.store(now, Ordering::Relaxed);
        self.stats.wake_events.fetch_add(1, Ordering::Relaxed);

        // Find and update triggered timers
        let triggered_ids: Vec<u32> = self.timers.iter()
            .filter(|t| t.state == WakeTimerState::Active)
            .filter(|t| {
                if let Some(next) = self.calculate_next_wake(t, now.saturating_sub(60)) {
                    next <= now
                } else {
                    false
                }
            })
            .map(|t| t.id)
            .collect();

        for id in triggered_ids {
            if let Some(timer) = self.timers.iter_mut().find(|t| t.id == id) {
                timer.last_triggered = now;
                timer.trigger_count += 1;

                // Disable one-shot timers
                if timer.schedule.timer_type == WakeTimerType::OneShot {
                    timer.state = WakeTimerState::Expired;
                    self.stats.active_timers.fetch_sub(1, Ordering::Relaxed);
                }

                // Fire callbacks
                for cb in &self.callbacks {
                    cb(id);
                }
            }
        }

        // Program next wake
        let _ = self.program_next_wake();

        crate::kprintln!("wake_timers: wake event processed");
    }

    /// Register callback for wake events
    pub fn register_callback(&mut self, cb: fn(u32)) {
        self.callbacks.push(cb);
    }

    /// Prepare for suspend
    pub fn prepare_suspend(&mut self) -> KResult<()> {
        // Ensure next wake is programmed
        self.program_next_wake()
    }

    /// Get statistics
    pub fn stats(&self) -> &WakeTimerStats {
        &self.stats
    }

    /// Check if any wake timers are active
    pub fn has_active_timers(&self) -> bool {
        self.timers.iter().any(|t| t.state == WakeTimerState::Active)
    }
}

/// Initialize wake timer subsystem
pub fn init() -> KResult<()> {
    WAKE_TIMER_MANAGER.lock().init()
}

/// Create a one-shot wake timer
pub fn create_oneshot(name: String, wake_time: u64) -> KResult<u32> {
    WAKE_TIMER_MANAGER.lock().create_oneshot(name, wake_time)
}

/// Create a daily recurring wake timer
pub fn create_daily(name: String, hour: u8, minute: u8) -> KResult<u32> {
    WAKE_TIMER_MANAGER.lock().create_daily(name, hour, minute)
}

/// Create a weekly recurring wake timer
pub fn create_weekly(name: String, hour: u8, minute: u8, days: u8) -> KResult<u32> {
    WAKE_TIMER_MANAGER.lock().create_weekly(name, hour, minute, days)
}

/// Create an interval-based wake timer
pub fn create_interval(name: String, interval_secs: u64) -> KResult<u32> {
    WAKE_TIMER_MANAGER.lock().create_interval(name, interval_secs)
}

/// Enable a timer
pub fn enable_timer(id: u32) -> KResult<()> {
    WAKE_TIMER_MANAGER.lock().enable_timer(id)
}

/// Disable a timer
pub fn disable_timer(id: u32) -> KResult<()> {
    WAKE_TIMER_MANAGER.lock().disable_timer(id)
}

/// Delete a timer
pub fn delete_timer(id: u32) -> KResult<()> {
    WAKE_TIMER_MANAGER.lock().delete_timer(id)
}

/// Get next scheduled wake time
pub fn next_wake_time() -> Option<u64> {
    WAKE_TIMER_MANAGER.lock().next_wake_time()
}

/// Prepare for suspend
pub fn prepare_suspend() -> KResult<()> {
    WAKE_TIMER_MANAGER.lock().prepare_suspend()
}

/// Handle RTC alarm interrupt
pub fn handle_rtc_alarm() {
    WAKE_TIMER_MANAGER.lock().handle_rtc_alarm();
}

/// Check if any wake timers are active
pub fn has_active_timers() -> bool {
    WAKE_TIMER_MANAGER.lock().has_active_timers()
}
