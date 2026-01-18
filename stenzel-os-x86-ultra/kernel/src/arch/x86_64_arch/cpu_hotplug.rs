//! CPU Hotplug support
//!
//! Allows CPUs to be brought online and taken offline dynamically.
//! This includes:
//! - CPU state management (online/offline/dying)
//! - Notifier chains for drivers
//! - Task migration when taking CPUs offline
//!
//! References:
//! - Linux CPU hotplug documentation
//! - ACPI processor hotplug

use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use spin::{Mutex, RwLock};

use crate::util::{KResult, KError};

/// Maximum number of CPUs for hotplug
pub const MAX_HOTPLUG_CPUS: usize = 256;

/// CPU state during hotplug operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuState {
    /// CPU is not present in the system
    NotPresent,
    /// CPU is present but offline
    Offline,
    /// CPU is coming online
    BringingUp,
    /// CPU is online and running
    Online,
    /// CPU is being taken offline
    GoingDown,
    /// CPU is in a dying state (cleaning up)
    Dying,
    /// CPU is frozen (for suspend/hibernate)
    Frozen,
}

/// CPU hotplug action (for notifiers)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotplugAction {
    /// CPU is coming online (early stage)
    Online,
    /// CPU is now active
    Active,
    /// CPU is going offline (prepare)
    OfflinePrepare,
    /// CPU is now offline
    Offline,
    /// CPU is dying (final cleanup)
    Dying,
    /// CPU frozen for suspend
    Frozen,
    /// CPU thawed after suspend
    Thawed,
}

/// Priority for hotplug notifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotifierPriority {
    /// Highest priority (scheduler)
    Scheduler = 0,
    /// High priority (timers, workqueues)
    High = 10,
    /// Normal priority (drivers)
    Normal = 20,
    /// Low priority (cleanup tasks)
    Low = 30,
}

/// Hotplug notifier callback type
pub type HotplugCallback = fn(cpu: u32, action: HotplugAction) -> KResult<()>;

/// Hotplug notifier entry
struct HotplugNotifier {
    name: String,
    priority: NotifierPriority,
    callback: HotplugCallback,
}

/// Per-CPU hotplug state
struct CpuHotplugState {
    /// Current state
    state: CpuState,
    /// Whether CPU can be taken offline
    can_offline: bool,
    /// Reference count (prevent offline while tasks are bound)
    ref_count: u32,
    /// Boot CPU flag
    is_boot_cpu: bool,
}

impl Default for CpuHotplugState {
    fn default() -> Self {
        Self {
            state: CpuState::NotPresent,
            can_offline: true,
            ref_count: 0,
            is_boot_cpu: false,
        }
    }
}

/// CPU Hotplug manager
pub struct CpuHotplugManager {
    /// Per-CPU states
    cpu_states: RwLock<[CpuHotplugState; MAX_HOTPLUG_CPUS]>,
    /// Notifier chain
    notifiers: RwLock<Vec<HotplugNotifier>>,
    /// Hotplug lock (only one operation at a time)
    hotplug_lock: Mutex<()>,
    /// Number of online CPUs
    online_count: AtomicU32,
    /// Hotplug enabled
    enabled: AtomicBool,
}

impl CpuHotplugManager {
    pub const fn new() -> Self {
        const DEFAULT_STATE: CpuHotplugState = CpuHotplugState {
            state: CpuState::NotPresent,
            can_offline: true,
            ref_count: 0,
            is_boot_cpu: false,
        };

        Self {
            cpu_states: RwLock::new([DEFAULT_STATE; MAX_HOTPLUG_CPUS]),
            notifiers: RwLock::new(Vec::new()),
            hotplug_lock: Mutex::new(()),
            online_count: AtomicU32::new(0),
            enabled: AtomicBool::new(false),
        }
    }

    /// Initialize the hotplug subsystem
    pub fn init(&self) {
        // Mark BSP as online
        let bsp_id = super::smp::bsp_apic_id() as usize;
        if bsp_id < MAX_HOTPLUG_CPUS {
            let mut states = self.cpu_states.write();
            states[bsp_id].state = CpuState::Online;
            states[bsp_id].is_boot_cpu = true;
            states[bsp_id].can_offline = false; // BSP cannot go offline
        }

        // Count present CPUs from SMP info
        let cpus = super::smp::get_all_cpus();
        let mut online = 0u32;

        for cpu in &cpus {
            let id = cpu.apic_id as usize;
            if id < MAX_HOTPLUG_CPUS {
                let mut states = self.cpu_states.write();
                if cpu.online {
                    states[id].state = CpuState::Online;
                    online += 1;
                } else {
                    states[id].state = CpuState::Offline;
                }
                states[id].is_boot_cpu = cpu.is_bsp;
                if cpu.is_bsp {
                    states[id].can_offline = false;
                }
            }
        }

        self.online_count.store(online, Ordering::SeqCst);
        self.enabled.store(true, Ordering::SeqCst);

        crate::kprintln!("cpu_hotplug: initialized, {} CPUs online", online);
    }

    /// Register a hotplug notifier
    pub fn register_notifier(
        &self,
        name: &str,
        priority: NotifierPriority,
        callback: HotplugCallback,
    ) {
        let notifier = HotplugNotifier {
            name: String::from(name),
            priority,
            callback,
        };

        let mut notifiers = self.notifiers.write();
        notifiers.push(notifier);

        // Sort by priority
        notifiers.sort_by_key(|n| n.priority);
    }

    /// Unregister a hotplug notifier
    pub fn unregister_notifier(&self, name: &str) {
        let mut notifiers = self.notifiers.write();
        notifiers.retain(|n| n.name != name);
    }

    /// Call notifiers for a hotplug action
    fn call_notifiers(&self, cpu: u32, action: HotplugAction) -> KResult<()> {
        let notifiers = self.notifiers.read();

        for notifier in notifiers.iter() {
            if let Err(e) = (notifier.callback)(cpu, action) {
                crate::kprintln!(
                    "cpu_hotplug: notifier '{}' failed for CPU {} action {:?}: {:?}",
                    notifier.name, cpu, action, e
                );
                return Err(e);
            }
        }

        Ok(())
    }

    /// Bring a CPU online
    pub fn cpu_up(&self, cpu_id: u32) -> KResult<()> {
        if !self.enabled.load(Ordering::SeqCst) {
            return Err(KError::NotSupported);
        }

        let id = cpu_id as usize;
        if id >= MAX_HOTPLUG_CPUS {
            return Err(KError::Invalid);
        }

        // Take hotplug lock
        let _lock = self.hotplug_lock.lock();

        // Check current state
        {
            let states = self.cpu_states.read();
            match states[id].state {
                CpuState::Online => return Ok(()), // Already online
                CpuState::Offline | CpuState::NotPresent => {}
                _ => return Err(KError::Busy), // In transition
            }
        }

        // Set state to bringing up
        {
            let mut states = self.cpu_states.write();
            states[id].state = CpuState::BringingUp;
        }

        crate::kprintln!("cpu_hotplug: bringing CPU {} online...", cpu_id);

        // Call pre-online notifiers
        if let Err(e) = self.call_notifiers(cpu_id, HotplugAction::Online) {
            let mut states = self.cpu_states.write();
            states[id].state = CpuState::Offline;
            return Err(e);
        }

        // Actually bring CPU up (via SMP)
        if let Err(e) = self.start_cpu(cpu_id) {
            let mut states = self.cpu_states.write();
            states[id].state = CpuState::Offline;
            return Err(e);
        }

        // Set state to online
        {
            let mut states = self.cpu_states.write();
            states[id].state = CpuState::Online;
        }

        self.online_count.fetch_add(1, Ordering::SeqCst);

        // Call post-online notifiers
        self.call_notifiers(cpu_id, HotplugAction::Active)?;

        crate::kprintln!("cpu_hotplug: CPU {} is now online", cpu_id);

        Ok(())
    }

    /// Take a CPU offline
    pub fn cpu_down(&self, cpu_id: u32) -> KResult<()> {
        if !self.enabled.load(Ordering::SeqCst) {
            return Err(KError::NotSupported);
        }

        let id = cpu_id as usize;
        if id >= MAX_HOTPLUG_CPUS {
            return Err(KError::Invalid);
        }

        // Take hotplug lock
        let _lock = self.hotplug_lock.lock();

        // Check current state
        {
            let states = self.cpu_states.read();
            match states[id].state {
                CpuState::Offline | CpuState::NotPresent => return Ok(()), // Already offline
                CpuState::Online => {}
                _ => return Err(KError::Busy), // In transition
            }

            // Check if CPU can go offline
            if !states[id].can_offline {
                return Err(KError::NotSupported); // Boot CPU
            }

            if states[id].ref_count > 0 {
                return Err(KError::Busy); // Tasks bound to this CPU
            }
        }

        // Must have at least one CPU online
        if self.online_count.load(Ordering::SeqCst) <= 1 {
            return Err(KError::Invalid);
        }

        crate::kprintln!("cpu_hotplug: taking CPU {} offline...", cpu_id);

        // Set state to going down
        {
            let mut states = self.cpu_states.write();
            states[id].state = CpuState::GoingDown;
        }

        // Call prepare notifiers
        if let Err(e) = self.call_notifiers(cpu_id, HotplugAction::OfflinePrepare) {
            let mut states = self.cpu_states.write();
            states[id].state = CpuState::Online;
            return Err(e);
        }

        // Migrate tasks away from this CPU
        self.migrate_tasks_away(cpu_id)?;

        // Set state to dying
        {
            let mut states = self.cpu_states.write();
            states[id].state = CpuState::Dying;
        }

        // Call dying notifiers
        self.call_notifiers(cpu_id, HotplugAction::Dying)?;

        // Actually stop the CPU
        self.stop_cpu(cpu_id)?;

        // Set state to offline
        {
            let mut states = self.cpu_states.write();
            states[id].state = CpuState::Offline;
        }

        self.online_count.fetch_sub(1, Ordering::SeqCst);

        // Call offline notifiers
        self.call_notifiers(cpu_id, HotplugAction::Offline)?;

        crate::kprintln!("cpu_hotplug: CPU {} is now offline", cpu_id);

        Ok(())
    }

    /// Start a CPU (send IPI to start AP)
    fn start_cpu(&self, cpu_id: u32) -> KResult<()> {
        // Call into SMP module to start the AP
        super::smp::start_ap(cpu_id as u8)
    }

    /// Stop a CPU (send IPI to halt)
    fn stop_cpu(&self, cpu_id: u32) -> KResult<()> {
        // Send stop IPI to the CPU
        super::ipi::stop_cpu(cpu_id as u8);

        // Wait for CPU to acknowledge
        for _ in 0..1000 {
            // Simple delay
            for _ in 0..10000 {
                core::hint::spin_loop();
            }
        }

        Ok(())
    }

    /// Migrate all tasks away from a CPU
    fn migrate_tasks_away(&self, cpu_id: u32) -> KResult<()> {
        crate::kprintln!("cpu_hotplug: migrating tasks from CPU {}...", cpu_id);

        // TODO: Actually migrate tasks via scheduler
        // For now, just a placeholder
        // crate::sched::migrate_tasks_from_cpu(cpu_id)?;

        Ok(())
    }

    /// Get CPU state
    pub fn get_state(&self, cpu_id: u32) -> CpuState {
        let id = cpu_id as usize;
        if id >= MAX_HOTPLUG_CPUS {
            return CpuState::NotPresent;
        }

        self.cpu_states.read()[id].state
    }

    /// Check if CPU is online
    pub fn is_online(&self, cpu_id: u32) -> bool {
        self.get_state(cpu_id) == CpuState::Online
    }

    /// Get number of online CPUs
    pub fn online_count(&self) -> u32 {
        self.online_count.load(Ordering::SeqCst)
    }

    /// Increment CPU reference count (prevent offline)
    pub fn cpu_get(&self, cpu_id: u32) -> KResult<()> {
        let id = cpu_id as usize;
        if id >= MAX_HOTPLUG_CPUS {
            return Err(KError::Invalid);
        }

        let mut states = self.cpu_states.write();
        if states[id].state != CpuState::Online {
            return Err(KError::NotFound);
        }

        states[id].ref_count = states[id].ref_count.saturating_add(1);
        Ok(())
    }

    /// Decrement CPU reference count
    pub fn cpu_put(&self, cpu_id: u32) {
        let id = cpu_id as usize;
        if id >= MAX_HOTPLUG_CPUS {
            return;
        }

        let mut states = self.cpu_states.write();
        states[id].ref_count = states[id].ref_count.saturating_sub(1);
    }

    /// Freeze all non-boot CPUs (for suspend/hibernate)
    pub fn freeze_cpus(&self) -> KResult<()> {
        crate::kprintln!("cpu_hotplug: freezing secondary CPUs...");

        let bsp_id = super::smp::bsp_apic_id() as u32;

        for id in 0..MAX_HOTPLUG_CPUS as u32 {
            if id == bsp_id {
                continue;
            }

            if self.is_online(id) {
                self.call_notifiers(id, HotplugAction::Frozen)?;

                let mut states = self.cpu_states.write();
                states[id as usize].state = CpuState::Frozen;
            }
        }

        crate::kprintln!("cpu_hotplug: secondary CPUs frozen");
        Ok(())
    }

    /// Thaw frozen CPUs (after resume)
    pub fn thaw_cpus(&self) -> KResult<()> {
        crate::kprintln!("cpu_hotplug: thawing secondary CPUs...");

        for id in 0..MAX_HOTPLUG_CPUS as u32 {
            let state = self.get_state(id);
            if state == CpuState::Frozen {
                let mut states = self.cpu_states.write();
                states[id as usize].state = CpuState::Online;
                drop(states);

                self.call_notifiers(id, HotplugAction::Thawed)?;
            }
        }

        crate::kprintln!("cpu_hotplug: secondary CPUs thawed");
        Ok(())
    }

    /// Set whether CPU can go offline
    pub fn set_can_offline(&self, cpu_id: u32, can_offline: bool) {
        let id = cpu_id as usize;
        if id >= MAX_HOTPLUG_CPUS {
            return;
        }

        let mut states = self.cpu_states.write();

        // Never allow boot CPU to go offline
        if states[id].is_boot_cpu {
            return;
        }

        states[id].can_offline = can_offline;
    }
}

// ============================================================================
// Global instance
// ============================================================================

static HOTPLUG_MANAGER: CpuHotplugManager = CpuHotplugManager::new();

/// Initialize CPU hotplug subsystem
pub fn init() {
    HOTPLUG_MANAGER.init();
}

/// Bring a CPU online
pub fn cpu_up(cpu_id: u32) -> KResult<()> {
    HOTPLUG_MANAGER.cpu_up(cpu_id)
}

/// Take a CPU offline
pub fn cpu_down(cpu_id: u32) -> KResult<()> {
    HOTPLUG_MANAGER.cpu_down(cpu_id)
}

/// Check if CPU is online
pub fn is_online(cpu_id: u32) -> bool {
    HOTPLUG_MANAGER.is_online(cpu_id)
}

/// Get CPU state
pub fn get_state(cpu_id: u32) -> CpuState {
    HOTPLUG_MANAGER.get_state(cpu_id)
}

/// Get number of online CPUs
pub fn online_count() -> u32 {
    HOTPLUG_MANAGER.online_count()
}

/// Register a hotplug notifier
pub fn register_notifier(
    name: &str,
    priority: NotifierPriority,
    callback: HotplugCallback,
) {
    HOTPLUG_MANAGER.register_notifier(name, priority, callback);
}

/// Unregister a hotplug notifier
pub fn unregister_notifier(name: &str) {
    HOTPLUG_MANAGER.unregister_notifier(name);
}

/// Increment CPU reference count
pub fn cpu_get(cpu_id: u32) -> KResult<()> {
    HOTPLUG_MANAGER.cpu_get(cpu_id)
}

/// Decrement CPU reference count
pub fn cpu_put(cpu_id: u32) {
    HOTPLUG_MANAGER.cpu_put(cpu_id)
}

/// Freeze CPUs for suspend
pub fn freeze_cpus() -> KResult<()> {
    HOTPLUG_MANAGER.freeze_cpus()
}

/// Thaw CPUs after resume
pub fn thaw_cpus() -> KResult<()> {
    HOTPLUG_MANAGER.thaw_cpus()
}

/// Format CPU hotplug status
pub fn format_status() -> String {
    let manager = &HOTPLUG_MANAGER;
    let states = manager.cpu_states.read();

    let mut online_cpus = Vec::new();
    let mut offline_cpus = Vec::new();

    for (id, state) in states.iter().enumerate() {
        match state.state {
            CpuState::Online => online_cpus.push(id),
            CpuState::Offline => offline_cpus.push(id),
            CpuState::NotPresent => continue,
            _ => {}
        }
    }

    format!(
        "CPU Hotplug:\n\
         - Online CPUs: {} ({:?})\n\
         - Offline CPUs: {} ({:?})\n\
         - Registered notifiers: {}\n",
        online_cpus.len(), online_cpus,
        offline_cpus.len(), offline_cpus,
        manager.notifiers.read().len()
    )
}
