//! Keyboard Accessibility Features
//!
//! Provides keyboard accessibility features:
//! - Sticky Keys: Modifier keys stay active after being pressed
//! - Slow Keys: Keys require prolonged press to register
//! - Bounce Keys: Ignore rapid repeated key presses
//! - Mouse Keys: Control mouse pointer with keyboard

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

use crate::sync::IrqSafeMutex;

/// Modifier key type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModifierKey {
    /// Shift key
    Shift,
    /// Control key
    Ctrl,
    /// Alt key
    Alt,
    /// Super/Windows/Meta key
    Super,
    /// AltGr key
    AltGr,
}

impl ModifierKey {
    /// Get modifier key name
    pub fn name(&self) -> &'static str {
        match self {
            ModifierKey::Shift => "Shift",
            ModifierKey::Ctrl => "Control",
            ModifierKey::Alt => "Alt",
            ModifierKey::Super => "Super",
            ModifierKey::AltGr => "AltGr",
        }
    }

    /// All modifier keys
    pub fn all() -> Vec<ModifierKey> {
        alloc::vec![
            ModifierKey::Shift,
            ModifierKey::Ctrl,
            ModifierKey::Alt,
            ModifierKey::Super,
            ModifierKey::AltGr,
        ]
    }
}

/// Sticky key state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StickyState {
    /// Modifier not active
    Off,
    /// Modifier latched (active for next key)
    Latched,
    /// Modifier locked (active until pressed again)
    Locked,
}

impl StickyState {
    /// Get state name
    pub fn name(&self) -> &'static str {
        match self {
            StickyState::Off => "Off",
            StickyState::Latched => "Latched",
            StickyState::Locked => "Locked",
        }
    }
}

/// Sticky keys configuration
#[derive(Debug, Clone)]
pub struct StickyKeysConfig {
    /// Whether sticky keys is enabled
    pub enabled: bool,
    /// Lock modifiers on double-press
    pub lock_on_double_press: bool,
    /// Double-press timeout (ms)
    pub double_press_timeout_ms: u32,
    /// Turn off when two modifiers are pressed together
    pub off_on_two_modifiers: bool,
    /// Play sound on modifier change
    pub sound_on_modifier: bool,
    /// Show on-screen indicator
    pub show_indicator: bool,
}

impl Default for StickyKeysConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            lock_on_double_press: true,
            double_press_timeout_ms: 300,
            off_on_two_modifiers: true,
            sound_on_modifier: true,
            show_indicator: true,
        }
    }
}

/// Per-modifier state tracking
#[derive(Debug, Clone)]
struct ModifierState {
    /// Current sticky state
    state: StickyState,
    /// Last press timestamp
    last_press_ms: u64,
    /// Number of times used while latched
    use_count: u64,
}

impl Default for ModifierState {
    fn default() -> Self {
        Self {
            state: StickyState::Off,
            last_press_ms: 0,
            use_count: 0,
        }
    }
}

/// Slow keys configuration
#[derive(Debug, Clone)]
pub struct SlowKeysConfig {
    /// Whether slow keys is enabled
    pub enabled: bool,
    /// Acceptance delay (ms) - how long key must be held
    pub acceptance_delay_ms: u32,
    /// Play sound when key is accepted
    pub sound_on_accept: bool,
    /// Play sound when key is rejected
    pub sound_on_reject: bool,
}

impl Default for SlowKeysConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            acceptance_delay_ms: 300,
            sound_on_accept: true,
            sound_on_reject: true,
        }
    }
}

/// Per-key slow keys tracking
#[derive(Debug, Clone, Default)]
struct SlowKeyState {
    /// Key code being tracked
    key_code: u8,
    /// Press start timestamp
    press_start_ms: u64,
    /// Whether key has been accepted
    accepted: bool,
}

/// Bounce keys configuration
#[derive(Debug, Clone)]
pub struct BounceKeysConfig {
    /// Whether bounce keys is enabled
    pub enabled: bool,
    /// Debounce delay (ms)
    pub debounce_delay_ms: u32,
}

impl Default for BounceKeysConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            debounce_delay_ms: 100,
        }
    }
}

/// Mouse keys configuration
#[derive(Debug, Clone)]
pub struct MouseKeysConfig {
    /// Whether mouse keys is enabled
    pub enabled: bool,
    /// Initial movement speed (pixels per keypress)
    pub initial_speed: u16,
    /// Maximum movement speed
    pub max_speed: u16,
    /// Acceleration factor
    pub acceleration: f32,
    /// Acceleration delay (ms before speeding up)
    pub acceleration_delay_ms: u32,
    /// Use numpad for mouse keys
    pub use_numpad: bool,
}

impl Default for MouseKeysConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            initial_speed: 5,
            max_speed: 20,
            acceleration: 1.5,
            acceleration_delay_ms: 500,
            use_numpad: true,
        }
    }
}

/// Mouse keys state
#[derive(Debug, Clone, Default)]
struct MouseKeysState {
    /// Current movement direction
    direction: (i32, i32),
    /// Movement start timestamp
    movement_start_ms: u64,
    /// Current speed
    current_speed: f32,
    /// Button being held
    held_button: Option<MouseButton>,
    /// Whether moving continuously
    moving: bool,
}

/// Mouse button for mouse keys
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

impl MouseButton {
    /// Get button name
    pub fn name(&self) -> &'static str {
        match self {
            MouseButton::Left => "Left",
            MouseButton::Middle => "Middle",
            MouseButton::Right => "Right",
        }
    }
}

/// Key event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventType {
    /// Key pressed
    Press,
    /// Key released
    Release,
    /// Key repeat (held down)
    Repeat,
}

/// Processed key event result
#[derive(Debug, Clone)]
pub enum KeyEventResult {
    /// Pass through normally
    PassThrough,
    /// Suppress the key event
    Suppress,
    /// Modified key event (with different modifiers)
    Modified {
        /// Active modifiers
        modifiers: u8, // Bitmask: Shift=1, Ctrl=2, Alt=4, Super=8, AltGr=16
    },
    /// Mouse movement
    MouseMove {
        dx: i32,
        dy: i32,
    },
    /// Mouse button event
    MouseButton {
        button: MouseButton,
        pressed: bool,
    },
}

/// Keyboard accessibility statistics
#[derive(Debug, Clone, Default)]
pub struct KeyboardAccessibilityStats {
    /// Times sticky keys used
    pub sticky_keys_activations: u64,
    /// Keys pressed with sticky modifiers
    pub sticky_modified_keys: u64,
    /// Keys rejected by slow keys
    pub slow_keys_rejections: u64,
    /// Keys rejected by bounce keys
    pub bounce_keys_rejections: u64,
    /// Mouse movements via mouse keys
    pub mouse_keys_movements: u64,
    /// Session start timestamp
    pub session_start_ms: u64,
}

/// Keyboard accessibility manager
pub struct KeyboardAccessibility {
    /// Sticky keys config
    sticky_config: StickyKeysConfig,
    /// Slow keys config
    slow_config: SlowKeysConfig,
    /// Bounce keys config
    bounce_config: BounceKeysConfig,
    /// Mouse keys config
    mouse_config: MouseKeysConfig,
    /// Sticky modifier states
    modifier_states: [ModifierState; 5], // Shift, Ctrl, Alt, Super, AltGr
    /// Slow key tracking
    slow_key_state: Option<SlowKeyState>,
    /// Last key press times for bounce keys
    last_key_press: [u64; 256],
    /// Mouse keys state
    mouse_state: MouseKeysState,
    /// Statistics
    stats: KeyboardAccessibilityStats,
    /// Callback for sticky state changes
    on_sticky_change: Option<fn(ModifierKey, StickyState)>,
    /// Callback for mouse movement
    on_mouse_move: Option<fn(i32, i32)>,
    /// Callback for mouse button
    on_mouse_button: Option<fn(MouseButton, bool)>,
}

impl KeyboardAccessibility {
    /// Create a new keyboard accessibility manager
    pub fn new() -> Self {
        Self {
            sticky_config: StickyKeysConfig::default(),
            slow_config: SlowKeysConfig::default(),
            bounce_config: BounceKeysConfig::default(),
            mouse_config: MouseKeysConfig::default(),
            modifier_states: Default::default(),
            slow_key_state: None,
            last_key_press: [0; 256],
            mouse_state: MouseKeysState::default(),
            stats: KeyboardAccessibilityStats::default(),
            on_sticky_change: None,
            on_mouse_move: None,
            on_mouse_button: None,
        }
    }

    /// Initialize
    pub fn init(&mut self) {
        self.stats.session_start_ms = crate::time::uptime_ms();
        crate::kprintln!("[keyboard_a11y] Keyboard accessibility initialized");
    }

    // ==================== Sticky Keys ====================

    /// Enable sticky keys
    pub fn enable_sticky_keys(&mut self) {
        self.sticky_config.enabled = true;
        crate::kprintln!("[keyboard_a11y] Sticky keys enabled");
    }

    /// Disable sticky keys
    pub fn disable_sticky_keys(&mut self) {
        self.sticky_config.enabled = false;
        // Clear all modifier states
        for state in &mut self.modifier_states {
            state.state = StickyState::Off;
        }
        crate::kprintln!("[keyboard_a11y] Sticky keys disabled");
    }

    /// Check if sticky keys is enabled
    pub fn is_sticky_keys_enabled(&self) -> bool {
        self.sticky_config.enabled
    }

    /// Process modifier key press for sticky keys
    pub fn process_modifier(&mut self, modifier: ModifierKey, pressed: bool) -> KeyEventResult {
        if !self.sticky_config.enabled {
            return KeyEventResult::PassThrough;
        }

        let now = crate::time::uptime_ms();
        let idx = modifier as usize;
        let state = &mut self.modifier_states[idx];

        if pressed {
            match state.state {
                StickyState::Off => {
                    // Check for double-press to lock
                    if self.sticky_config.lock_on_double_press
                        && (now - state.last_press_ms) < self.sticky_config.double_press_timeout_ms as u64
                    {
                        state.state = StickyState::Locked;
                    } else {
                        state.state = StickyState::Latched;
                    }
                    state.last_press_ms = now;
                    self.stats.sticky_keys_activations += 1;
                }
                StickyState::Latched => {
                    // Double-press: latch -> lock
                    if self.sticky_config.lock_on_double_press
                        && (now - state.last_press_ms) < self.sticky_config.double_press_timeout_ms as u64
                    {
                        state.state = StickyState::Locked;
                    }
                    state.last_press_ms = now;
                }
                StickyState::Locked => {
                    // Press again to unlock
                    state.state = StickyState::Off;
                    state.use_count = 0;
                }
            }

            if let Some(callback) = self.on_sticky_change {
                callback(modifier, state.state);
            }

            // Suppress the modifier key itself
            KeyEventResult::Suppress
        } else {
            // Release: don't do anything special
            KeyEventResult::Suppress
        }
    }

    /// Process regular key with sticky modifiers
    pub fn process_key_with_sticky(&mut self, key_code: u8, pressed: bool) -> KeyEventResult {
        if !self.sticky_config.enabled || !pressed {
            return KeyEventResult::PassThrough;
        }

        // Build modifier bitmask from sticky states
        let mut modifiers: u8 = 0;
        let mut had_latched = false;

        for (idx, modifier) in ModifierKey::all().iter().enumerate() {
            let state = &self.modifier_states[idx];
            if state.state == StickyState::Latched || state.state == StickyState::Locked {
                modifiers |= 1 << idx;
                if state.state == StickyState::Latched {
                    had_latched = true;
                }
            }
        }

        // If we have active modifiers, return modified result
        if modifiers != 0 {
            self.stats.sticky_modified_keys += 1;

            // Clear latched modifiers after use
            if had_latched {
                for state in &mut self.modifier_states {
                    if state.state == StickyState::Latched {
                        state.state = StickyState::Off;
                        state.use_count += 1;
                    }
                }
            }

            KeyEventResult::Modified { modifiers }
        } else {
            KeyEventResult::PassThrough
        }
    }

    /// Get sticky state for a modifier
    pub fn get_sticky_state(&self, modifier: ModifierKey) -> StickyState {
        self.modifier_states[modifier as usize].state
    }

    /// Get active modifiers bitmask
    pub fn get_active_modifiers(&self) -> u8 {
        let mut modifiers: u8 = 0;
        for (idx, state) in self.modifier_states.iter().enumerate() {
            if state.state != StickyState::Off {
                modifiers |= 1 << idx;
            }
        }
        modifiers
    }

    // ==================== Slow Keys ====================

    /// Enable slow keys
    pub fn enable_slow_keys(&mut self) {
        self.slow_config.enabled = true;
        crate::kprintln!("[keyboard_a11y] Slow keys enabled ({}ms delay)", self.slow_config.acceptance_delay_ms);
    }

    /// Disable slow keys
    pub fn disable_slow_keys(&mut self) {
        self.slow_config.enabled = false;
        self.slow_key_state = None;
        crate::kprintln!("[keyboard_a11y] Slow keys disabled");
    }

    /// Check if slow keys is enabled
    pub fn is_slow_keys_enabled(&self) -> bool {
        self.slow_config.enabled
    }

    /// Set slow keys delay
    pub fn set_slow_keys_delay(&mut self, delay_ms: u32) {
        self.slow_config.acceptance_delay_ms = delay_ms.max(100).min(5000);
    }

    /// Process key for slow keys
    pub fn process_slow_key(&mut self, key_code: u8, event_type: KeyEventType) -> KeyEventResult {
        if !self.slow_config.enabled {
            return KeyEventResult::PassThrough;
        }

        let now = crate::time::uptime_ms();

        match event_type {
            KeyEventType::Press => {
                // Start tracking this key
                self.slow_key_state = Some(SlowKeyState {
                    key_code,
                    press_start_ms: now,
                    accepted: false,
                });
                // Suppress initial press
                KeyEventResult::Suppress
            }
            KeyEventType::Repeat => {
                // Check if key should be accepted
                if let Some(ref mut state) = self.slow_key_state {
                    if state.key_code == key_code {
                        let held_time = now - state.press_start_ms;
                        if held_time >= self.slow_config.acceptance_delay_ms as u64 {
                            if !state.accepted {
                                state.accepted = true;
                                // Key accepted - pass through
                                return KeyEventResult::PassThrough;
                            }
                        }
                    }
                }
                // Still waiting or different key
                self.stats.slow_keys_rejections += 1;
                KeyEventResult::Suppress
            }
            KeyEventType::Release => {
                if let Some(state) = self.slow_key_state.take() {
                    if state.key_code == key_code && state.accepted {
                        // Key was accepted, pass through release
                        return KeyEventResult::PassThrough;
                    }
                }
                // Key was not accepted, suppress release too
                self.stats.slow_keys_rejections += 1;
                KeyEventResult::Suppress
            }
        }
    }

    // ==================== Bounce Keys ====================

    /// Enable bounce keys
    pub fn enable_bounce_keys(&mut self) {
        self.bounce_config.enabled = true;
        crate::kprintln!("[keyboard_a11y] Bounce keys enabled ({}ms debounce)", self.bounce_config.debounce_delay_ms);
    }

    /// Disable bounce keys
    pub fn disable_bounce_keys(&mut self) {
        self.bounce_config.enabled = false;
        crate::kprintln!("[keyboard_a11y] Bounce keys disabled");
    }

    /// Check if bounce keys is enabled
    pub fn is_bounce_keys_enabled(&self) -> bool {
        self.bounce_config.enabled
    }

    /// Set bounce keys delay
    pub fn set_bounce_keys_delay(&mut self, delay_ms: u32) {
        self.bounce_config.debounce_delay_ms = delay_ms.max(50).min(2000);
    }

    /// Process key for bounce keys
    pub fn process_bounce_key(&mut self, key_code: u8, pressed: bool) -> KeyEventResult {
        if !self.bounce_config.enabled || !pressed {
            return KeyEventResult::PassThrough;
        }

        let now = crate::time::uptime_ms();
        let last_press = self.last_key_press[key_code as usize];

        if (now - last_press) < self.bounce_config.debounce_delay_ms as u64 {
            // Too fast - bounce rejected
            self.stats.bounce_keys_rejections += 1;
            KeyEventResult::Suppress
        } else {
            // Accept and record time
            self.last_key_press[key_code as usize] = now;
            KeyEventResult::PassThrough
        }
    }

    // ==================== Mouse Keys ====================

    /// Enable mouse keys
    pub fn enable_mouse_keys(&mut self) {
        self.mouse_config.enabled = true;
        crate::kprintln!("[keyboard_a11y] Mouse keys enabled");
    }

    /// Disable mouse keys
    pub fn disable_mouse_keys(&mut self) {
        self.mouse_config.enabled = false;
        self.mouse_state = MouseKeysState::default();
        crate::kprintln!("[keyboard_a11y] Mouse keys disabled");
    }

    /// Check if mouse keys is enabled
    pub fn is_mouse_keys_enabled(&self) -> bool {
        self.mouse_config.enabled
    }

    /// Process numpad key for mouse keys
    /// Returns MouseMove or MouseButton if the key is a mouse key, PassThrough otherwise
    pub fn process_mouse_key(&mut self, key_code: u8, pressed: bool) -> KeyEventResult {
        if !self.mouse_config.enabled || !self.mouse_config.use_numpad {
            return KeyEventResult::PassThrough;
        }

        // Numpad key codes (typical):
        // 7 8 9 (home, up, pgup) -> move: (-1,-1), (0,-1), (1,-1)
        // 4 5 6 (left, center, right) -> move: (-1,0), click, (1,0)
        // 1 2 3 (end, down, pgdn) -> move: (-1,1), (0,1), (1,1)
        // 0 (ins) -> drag/hold button
        // / * - -> left/middle/right button select

        // Numpad key codes (scan codes may vary by keyboard)
        const NUMPAD_7: u8 = 0x47; // Home
        const NUMPAD_8: u8 = 0x48; // Up
        const NUMPAD_9: u8 = 0x49; // PgUp
        const NUMPAD_4: u8 = 0x4B; // Left
        const NUMPAD_5: u8 = 0x4C; // Center
        const NUMPAD_6: u8 = 0x4D; // Right
        const NUMPAD_1: u8 = 0x4F; // End
        const NUMPAD_2: u8 = 0x50; // Down
        const NUMPAD_3: u8 = 0x51; // PgDn
        const NUMPAD_0: u8 = 0x52; // Ins
        const NUMPAD_SLASH: u8 = 0x35;
        const NUMPAD_ASTERISK: u8 = 0x37;
        const NUMPAD_MINUS: u8 = 0x4A;

        let now = crate::time::uptime_ms();

        // Direction keys
        let direction = match key_code {
            NUMPAD_7 => Some((-1, -1)),
            NUMPAD_8 => Some((0, -1)),
            NUMPAD_9 => Some((1, -1)),
            NUMPAD_4 => Some((-1, 0)),
            NUMPAD_6 => Some((1, 0)),
            NUMPAD_1 => Some((-1, 1)),
            NUMPAD_2 => Some((0, 1)),
            NUMPAD_3 => Some((1, 1)),
            _ => None,
        };

        if let Some(dir) = direction {
            if pressed {
                // Start or continue movement
                if !self.mouse_state.moving {
                    self.mouse_state.movement_start_ms = now;
                    self.mouse_state.current_speed = self.mouse_config.initial_speed as f32;
                }
                self.mouse_state.direction = dir;
                self.mouse_state.moving = true;

                // Calculate speed with acceleration
                let elapsed = now - self.mouse_state.movement_start_ms;
                if elapsed > self.mouse_config.acceleration_delay_ms as u64 {
                    self.mouse_state.current_speed = (self.mouse_state.current_speed
                        * self.mouse_config.acceleration)
                        .min(self.mouse_config.max_speed as f32);
                }

                let speed = self.mouse_state.current_speed as i32;
                let dx = dir.0 * speed;
                let dy = dir.1 * speed;

                self.stats.mouse_keys_movements += 1;

                if let Some(callback) = self.on_mouse_move {
                    callback(dx, dy);
                }

                return KeyEventResult::MouseMove { dx, dy };
            } else {
                // Stop movement in this direction
                if self.mouse_state.direction == dir {
                    self.mouse_state.moving = false;
                    self.mouse_state.direction = (0, 0);
                }
                return KeyEventResult::Suppress;
            }
        }

        // Click key (numpad 5)
        if key_code == NUMPAD_5 {
            let button = self.mouse_state.held_button.unwrap_or(MouseButton::Left);

            if let Some(callback) = self.on_mouse_button {
                callback(button, pressed);
            }

            return KeyEventResult::MouseButton { button, pressed };
        }

        // Button select keys
        match key_code {
            NUMPAD_SLASH if pressed => {
                self.mouse_state.held_button = Some(MouseButton::Left);
                return KeyEventResult::Suppress;
            }
            NUMPAD_ASTERISK if pressed => {
                self.mouse_state.held_button = Some(MouseButton::Middle);
                return KeyEventResult::Suppress;
            }
            NUMPAD_MINUS if pressed => {
                self.mouse_state.held_button = Some(MouseButton::Right);
                return KeyEventResult::Suppress;
            }
            _ => {}
        }

        // Drag key (numpad 0)
        if key_code == NUMPAD_0 {
            let button = self.mouse_state.held_button.unwrap_or(MouseButton::Left);

            if let Some(callback) = self.on_mouse_button {
                callback(button, pressed);
            }

            return KeyEventResult::MouseButton { button, pressed };
        }

        KeyEventResult::PassThrough
    }

    // ==================== Configuration ====================

    /// Get sticky keys config
    pub fn sticky_config(&self) -> &StickyKeysConfig {
        &self.sticky_config
    }

    /// Set sticky keys config
    pub fn set_sticky_config(&mut self, config: StickyKeysConfig) {
        self.sticky_config = config;
    }

    /// Get slow keys config
    pub fn slow_config(&self) -> &SlowKeysConfig {
        &self.slow_config
    }

    /// Set slow keys config
    pub fn set_slow_config(&mut self, config: SlowKeysConfig) {
        self.slow_config = config;
    }

    /// Get bounce keys config
    pub fn bounce_config(&self) -> &BounceKeysConfig {
        &self.bounce_config
    }

    /// Set bounce keys config
    pub fn set_bounce_config(&mut self, config: BounceKeysConfig) {
        self.bounce_config = config;
    }

    /// Get mouse keys config
    pub fn mouse_config(&self) -> &MouseKeysConfig {
        &self.mouse_config
    }

    /// Set mouse keys config
    pub fn set_mouse_config(&mut self, config: MouseKeysConfig) {
        self.mouse_config = config;
    }

    /// Set sticky change callback
    pub fn set_sticky_change_callback(&mut self, callback: fn(ModifierKey, StickyState)) {
        self.on_sticky_change = Some(callback);
    }

    /// Set mouse move callback
    pub fn set_mouse_move_callback(&mut self, callback: fn(i32, i32)) {
        self.on_mouse_move = Some(callback);
    }

    /// Set mouse button callback
    pub fn set_mouse_button_callback(&mut self, callback: fn(MouseButton, bool)) {
        self.on_mouse_button = Some(callback);
    }

    /// Get statistics
    pub fn stats(&self) -> &KeyboardAccessibilityStats {
        &self.stats
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        format!(
            "Keyboard Accessibility:\n\
             Sticky Keys: {} (activations: {}, modified keys: {})\n\
             Slow Keys: {} (delay: {}ms, rejections: {})\n\
             Bounce Keys: {} (debounce: {}ms, rejections: {})\n\
             Mouse Keys: {} (movements: {})",
            if self.sticky_config.enabled { "Enabled" } else { "Disabled" },
            self.stats.sticky_keys_activations,
            self.stats.sticky_modified_keys,
            if self.slow_config.enabled { "Enabled" } else { "Disabled" },
            self.slow_config.acceptance_delay_ms,
            self.stats.slow_keys_rejections,
            if self.bounce_config.enabled { "Enabled" } else { "Disabled" },
            self.bounce_config.debounce_delay_ms,
            self.stats.bounce_keys_rejections,
            if self.mouse_config.enabled { "Enabled" } else { "Disabled" },
            self.stats.mouse_keys_movements
        )
    }
}

/// Global keyboard accessibility instance
static KEYBOARD_ACCESSIBILITY: IrqSafeMutex<Option<KeyboardAccessibility>> = IrqSafeMutex::new(None);

/// Initialize keyboard accessibility
pub fn init() {
    let mut ka = KeyboardAccessibility::new();
    ka.init();
    *KEYBOARD_ACCESSIBILITY.lock() = Some(ka);
}

// ==================== Sticky Keys API ====================

/// Enable sticky keys
pub fn enable_sticky_keys() {
    if let Some(ref mut ka) = *KEYBOARD_ACCESSIBILITY.lock() {
        ka.enable_sticky_keys();
    }
}

/// Disable sticky keys
pub fn disable_sticky_keys() {
    if let Some(ref mut ka) = *KEYBOARD_ACCESSIBILITY.lock() {
        ka.disable_sticky_keys();
    }
}

/// Check if sticky keys is enabled
pub fn is_sticky_keys_enabled() -> bool {
    KEYBOARD_ACCESSIBILITY.lock().as_ref().map(|ka| ka.is_sticky_keys_enabled()).unwrap_or(false)
}

/// Get sticky state for a modifier
pub fn get_sticky_state(modifier: ModifierKey) -> StickyState {
    KEYBOARD_ACCESSIBILITY.lock().as_ref().map(|ka| ka.get_sticky_state(modifier)).unwrap_or(StickyState::Off)
}

// ==================== Slow Keys API ====================

/// Enable slow keys
pub fn enable_slow_keys() {
    if let Some(ref mut ka) = *KEYBOARD_ACCESSIBILITY.lock() {
        ka.enable_slow_keys();
    }
}

/// Disable slow keys
pub fn disable_slow_keys() {
    if let Some(ref mut ka) = *KEYBOARD_ACCESSIBILITY.lock() {
        ka.disable_slow_keys();
    }
}

/// Check if slow keys is enabled
pub fn is_slow_keys_enabled() -> bool {
    KEYBOARD_ACCESSIBILITY.lock().as_ref().map(|ka| ka.is_slow_keys_enabled()).unwrap_or(false)
}

// ==================== Bounce Keys API ====================

/// Enable bounce keys
pub fn enable_bounce_keys() {
    if let Some(ref mut ka) = *KEYBOARD_ACCESSIBILITY.lock() {
        ka.enable_bounce_keys();
    }
}

/// Disable bounce keys
pub fn disable_bounce_keys() {
    if let Some(ref mut ka) = *KEYBOARD_ACCESSIBILITY.lock() {
        ka.disable_bounce_keys();
    }
}

/// Check if bounce keys is enabled
pub fn is_bounce_keys_enabled() -> bool {
    KEYBOARD_ACCESSIBILITY.lock().as_ref().map(|ka| ka.is_bounce_keys_enabled()).unwrap_or(false)
}

// ==================== Mouse Keys API ====================

/// Enable mouse keys
pub fn enable_mouse_keys() {
    if let Some(ref mut ka) = *KEYBOARD_ACCESSIBILITY.lock() {
        ka.enable_mouse_keys();
    }
}

/// Disable mouse keys
pub fn disable_mouse_keys() {
    if let Some(ref mut ka) = *KEYBOARD_ACCESSIBILITY.lock() {
        ka.disable_mouse_keys();
    }
}

/// Check if mouse keys is enabled
pub fn is_mouse_keys_enabled() -> bool {
    KEYBOARD_ACCESSIBILITY.lock().as_ref().map(|ka| ka.is_mouse_keys_enabled()).unwrap_or(false)
}

// ==================== General API ====================

/// Get status string
pub fn status() -> String {
    KEYBOARD_ACCESSIBILITY.lock().as_ref()
        .map(|ka| ka.format_status())
        .unwrap_or_else(|| String::from("Keyboard Accessibility: Not initialized"))
}

/// Get statistics
pub fn stats() -> Option<KeyboardAccessibilityStats> {
    KEYBOARD_ACCESSIBILITY.lock().as_ref().map(|ka| ka.stats().clone())
}
