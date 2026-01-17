//! Driver de teclado PS/2 (i8042)
//!
//! Converte scancodes Set 1 para caracteres usando layout configurável.
//! Suporta layouts: US, ABNT2

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

use super::keyboard_layout::{self, KeyResult, Layout};

/// Buffer de entrada do teclado (caracteres prontos para leitura)
static INPUT_BUFFER: Mutex<VecDeque<u8>> = Mutex::new(VecDeque::new());

/// Buffer para caracteres UTF-8 multibyte
static UTF8_BUFFER: Mutex<VecDeque<u8>> = Mutex::new(VecDeque::new());

/// Estado dos modificadores
static MODIFIERS: Mutex<Modifiers> = Mutex::new(Modifiers::new());

/// Capacidade máxima do buffer
const BUFFER_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy)]
struct Modifiers {
    left_shift: bool,
    right_shift: bool,
    left_ctrl: bool,
    right_ctrl: bool,
    left_alt: bool,
    right_alt: bool,  // AltGr on international keyboards
    caps_lock: bool,
    num_lock: bool,
    scroll_lock: bool,
}

impl Modifiers {
    const fn new() -> Self {
        Self {
            left_shift: false,
            right_shift: false,
            left_ctrl: false,
            right_ctrl: false,
            left_alt: false,
            right_alt: false,
            caps_lock: false,
            num_lock: true,  // Num Lock usually starts on
            scroll_lock: false,
        }
    }

    fn shift(&self) -> bool {
        self.left_shift || self.right_shift
    }

    fn ctrl(&self) -> bool {
        self.left_ctrl || self.right_ctrl
    }

    fn alt(&self) -> bool {
        self.left_alt
    }

    fn alt_gr(&self) -> bool {
        self.right_alt
    }
}

/// Scancodes especiais
const SC_LEFT_SHIFT: u8 = 0x2A;
const SC_RIGHT_SHIFT: u8 = 0x36;
const SC_LEFT_CTRL: u8 = 0x1D;
const SC_LEFT_ALT: u8 = 0x38;
const SC_CAPS_LOCK: u8 = 0x3A;
const SC_NUM_LOCK: u8 = 0x45;
const SC_SCROLL_LOCK: u8 = 0x46;
const SC_RELEASE_BIT: u8 = 0x80;

// Extended scancode prefix
const SC_EXTENDED: u8 = 0xE0;

// Extended key scancodes (after 0xE0)
const SC_EXT_RIGHT_CTRL: u8 = 0x1D;
const SC_EXT_RIGHT_ALT: u8 = 0x38;  // AltGr

/// State machine for extended scancodes
static EXTENDED_PENDING: Mutex<bool> = Mutex::new(false);

/// Processa um scancode recebido da IRQ do teclado.
/// Chamado pelo interrupt handler.
pub fn process_scancode(scancode: u8) {
    // Check for extended scancode prefix
    if scancode == SC_EXTENDED {
        *EXTENDED_PENDING.lock() = true;
        return;
    }

    let is_extended = {
        let mut ext = EXTENDED_PENDING.lock();
        let was_extended = *ext;
        *ext = false;
        was_extended
    };

    let mut mods = MODIFIERS.lock();
    let is_release = (scancode & SC_RELEASE_BIT) != 0;
    let key = scancode & !SC_RELEASE_BIT;

    // Report key event to the input subsystem
    super::input::report_key(key, !is_release);

    // Handle extended keys (after E0 prefix)
    if is_extended {
        match key {
            SC_EXT_RIGHT_CTRL => {
                mods.right_ctrl = !is_release;
                return;
            }
            SC_EXT_RIGHT_ALT => {
                mods.right_alt = !is_release;
                return;
            }
            // Add more extended keys as needed (arrows, home, end, etc.)
            _ => {}
        }
    }

    // Atualiza estado dos modificadores normais
    match key {
        SC_LEFT_SHIFT => {
            mods.left_shift = !is_release;
            return;
        }
        SC_RIGHT_SHIFT => {
            mods.right_shift = !is_release;
            return;
        }
        SC_LEFT_CTRL => {
            mods.left_ctrl = !is_release;
            return;
        }
        SC_LEFT_ALT => {
            mods.left_alt = !is_release;
            return;
        }
        SC_CAPS_LOCK if !is_release => {
            mods.caps_lock = !mods.caps_lock;
            update_leds(&mods);
            return;
        }
        SC_NUM_LOCK if !is_release => {
            mods.num_lock = !mods.num_lock;
            update_leds(&mods);
            return;
        }
        SC_SCROLL_LOCK if !is_release => {
            mods.scroll_lock = !mods.scroll_lock;
            update_leds(&mods);
            return;
        }
        _ => {}
    }

    // Ignora key release para teclas normais
    if is_release {
        return;
    }

    // Get shift state considering caps lock
    let effective_shift = mods.shift() ^ mods.caps_lock;

    // Convert using layout
    let result = keyboard_layout::scancode_to_char(key, effective_shift, mods.alt_gr());

    match result {
        KeyResult::Char(ch) => {
            process_char(ch, &mods);
        }
        KeyResult::ExtChar(s) => {
            // Add UTF-8 bytes to buffer
            for b in s.bytes() {
                add_to_buffer(b);
            }
        }
        KeyResult::Dead(_) => {
            // Dead key state is handled in the layout module
            // Nothing to add to buffer yet
        }
        KeyResult::None => {
            // Unknown or modifier key
        }
    }
}

/// Process a single ASCII character with modifiers
fn process_char(mut ascii: u8, mods: &Modifiers) {
    // Ctrl+letter combinations
    if mods.ctrl() && ascii >= b'a' && ascii <= b'z' {
        ascii = ascii - b'a' + 1;
    } else if mods.ctrl() && ascii >= b'A' && ascii <= b'Z' {
        ascii = ascii - b'A' + 1;
    }

    // Tratamento especial para Ctrl+C (SIGINT)
    if ascii == 3 {
        // Envia SIGINT para o processo foreground
        crate::sched::send_sigint_to_foreground();
    }

    add_to_buffer(ascii);
}

/// Add a byte to the input buffer
fn add_to_buffer(byte: u8) {
    if byte != 0 {
        let mut buf = INPUT_BUFFER.lock();
        if buf.len() < BUFFER_CAPACITY {
            buf.push_back(byte);
        }
    }
}

/// Update keyboard LEDs (Caps Lock, Num Lock, Scroll Lock)
fn update_leds(mods: &Modifiers) {
    let mut led_state = 0u8;
    if mods.scroll_lock { led_state |= 1; }
    if mods.num_lock { led_state |= 2; }
    if mods.caps_lock { led_state |= 4; }

    // Send LED update command to keyboard
    // This requires writing to the keyboard controller
    // For simplicity, we'll skip the actual hardware write
    // In a full implementation:
    // 1. Write 0xED to port 0x60
    // 2. Wait for ACK (0xFA)
    // 3. Write led_state to port 0x60
    let _ = led_state; // Suppress unused warning
}

/// Lê um caractere do buffer de entrada (não-bloqueante).
/// Retorna None se o buffer estiver vazio.
pub fn read_char() -> Option<u8> {
    INPUT_BUFFER.lock().pop_front()
}

/// Verifica se há caracteres disponíveis para leitura.
pub fn has_input() -> bool {
    !INPUT_BUFFER.lock().is_empty()
}

/// Get current keyboard layout
pub fn current_layout() -> Layout {
    keyboard_layout::current_layout()
}

/// Set keyboard layout
pub fn set_layout(layout: Layout) {
    keyboard_layout::set_layout(layout);
}

/// Set keyboard layout by name
pub fn set_layout_by_name(name: &str) -> bool {
    if let Some(layout) = keyboard_layout::parse_layout(name) {
        keyboard_layout::set_layout(layout);
        true
    } else {
        false
    }
}

/// List available layouts
pub fn available_layouts() -> &'static [Layout] {
    keyboard_layout::available_layouts()
}

/// Initialize keyboard driver
pub fn init() {
    crate::kprintln!("keyboard: initialized with {} layout", current_layout().name());
}
