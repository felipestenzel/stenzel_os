//! Driver de teclado PS/2 (i8042)
//!
//! Converte scancodes Set 1 para caracteres ASCII e mantém
//! estado de modificadores (Shift, Ctrl, Alt, Caps Lock).

use alloc::collections::VecDeque;
use spin::Mutex;

/// Buffer de entrada do teclado (caracteres prontos para leitura)
static INPUT_BUFFER: Mutex<VecDeque<u8>> = Mutex::new(VecDeque::new());

/// Estado dos modificadores
static MODIFIERS: Mutex<Modifiers> = Mutex::new(Modifiers::new());

/// Capacidade máxima do buffer
const BUFFER_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy)]
struct Modifiers {
    left_shift: bool,
    right_shift: bool,
    ctrl: bool,
    alt: bool,
    caps_lock: bool,
}

impl Modifiers {
    const fn new() -> Self {
        Self {
            left_shift: false,
            right_shift: false,
            ctrl: false,
            alt: false,
            caps_lock: false,
        }
    }

    fn shift(&self) -> bool {
        self.left_shift || self.right_shift
    }
}

/// Converte scancode Set 1 para ASCII (sem shift)
fn scancode_to_ascii(sc: u8) -> u8 {
    match sc {
        0x01 => 27,   // ESC
        0x02 => b'1', 0x03 => b'2', 0x04 => b'3', 0x05 => b'4', 0x06 => b'5',
        0x07 => b'6', 0x08 => b'7', 0x09 => b'8', 0x0A => b'9', 0x0B => b'0',
        0x0C => b'-', 0x0D => b'=', 0x0E => 8,    // Backspace
        0x0F => b'\t',
        0x10 => b'q', 0x11 => b'w', 0x12 => b'e', 0x13 => b'r', 0x14 => b't',
        0x15 => b'y', 0x16 => b'u', 0x17 => b'i', 0x18 => b'o', 0x19 => b'p',
        0x1A => b'[', 0x1B => b']', 0x1C => b'\n', // Enter
        0x1E => b'a', 0x1F => b's', 0x20 => b'd', 0x21 => b'f', 0x22 => b'g',
        0x23 => b'h', 0x24 => b'j', 0x25 => b'k', 0x26 => b'l',
        0x27 => b';', 0x28 => b'\'', 0x29 => b'`',
        0x2B => b'\\',
        0x2C => b'z', 0x2D => b'x', 0x2E => b'c', 0x2F => b'v', 0x30 => b'b',
        0x31 => b'n', 0x32 => b'm',
        0x33 => b',', 0x34 => b'.', 0x35 => b'/',
        0x37 => b'*', // Keypad *
        0x39 => b' ', // Space
        // Keypad
        0x47 => b'7', 0x48 => b'8', 0x49 => b'9', 0x4A => b'-',
        0x4B => b'4', 0x4C => b'5', 0x4D => b'6', 0x4E => b'+',
        0x4F => b'1', 0x50 => b'2', 0x51 => b'3',
        0x52 => b'0', 0x53 => b'.',
        _ => 0,
    }
}

/// Converte scancode Set 1 para ASCII (com shift)
fn scancode_to_ascii_shift(sc: u8) -> u8 {
    match sc {
        0x01 => 27,   // ESC
        0x02 => b'!', 0x03 => b'@', 0x04 => b'#', 0x05 => b'$', 0x06 => b'%',
        0x07 => b'^', 0x08 => b'&', 0x09 => b'*', 0x0A => b'(', 0x0B => b')',
        0x0C => b'_', 0x0D => b'+', 0x0E => 8,    // Backspace
        0x0F => b'\t',
        0x10 => b'Q', 0x11 => b'W', 0x12 => b'E', 0x13 => b'R', 0x14 => b'T',
        0x15 => b'Y', 0x16 => b'U', 0x17 => b'I', 0x18 => b'O', 0x19 => b'P',
        0x1A => b'{', 0x1B => b'}', 0x1C => b'\n', // Enter
        0x1E => b'A', 0x1F => b'S', 0x20 => b'D', 0x21 => b'F', 0x22 => b'G',
        0x23 => b'H', 0x24 => b'J', 0x25 => b'K', 0x26 => b'L',
        0x27 => b':', 0x28 => b'"', 0x29 => b'~',
        0x2B => b'|',
        0x2C => b'Z', 0x2D => b'X', 0x2E => b'C', 0x2F => b'V', 0x30 => b'B',
        0x31 => b'N', 0x32 => b'M',
        0x33 => b'<', 0x34 => b'>', 0x35 => b'?',
        0x37 => b'*', // Keypad *
        0x39 => b' ', // Space
        // Keypad (same as without shift)
        0x47 => b'7', 0x48 => b'8', 0x49 => b'9', 0x4A => b'-',
        0x4B => b'4', 0x4C => b'5', 0x4D => b'6', 0x4E => b'+',
        0x4F => b'1', 0x50 => b'2', 0x51 => b'3',
        0x52 => b'0', 0x53 => b'.',
        _ => 0,
    }
}

/// Scancodes especiais
const SC_LEFT_SHIFT: u8 = 0x2A;
const SC_RIGHT_SHIFT: u8 = 0x36;
const SC_LEFT_CTRL: u8 = 0x1D;
const SC_LEFT_ALT: u8 = 0x38;
const SC_CAPS_LOCK: u8 = 0x3A;
const SC_RELEASE_BIT: u8 = 0x80;

/// Processa um scancode recebido da IRQ do teclado.
/// Chamado pelo interrupt handler.
pub fn process_scancode(scancode: u8) {
    let mut mods = MODIFIERS.lock();

    let is_release = (scancode & SC_RELEASE_BIT) != 0;
    let key = scancode & !SC_RELEASE_BIT;

    // Atualiza estado dos modificadores
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
            mods.ctrl = !is_release;
            return;
        }
        SC_LEFT_ALT => {
            mods.alt = !is_release;
            return;
        }
        SC_CAPS_LOCK if !is_release => {
            mods.caps_lock = !mods.caps_lock;
            return;
        }
        _ => {}
    }

    // Ignora key release para teclas normais
    if is_release {
        return;
    }

    // Converte para ASCII
    let base = if mods.shift() {
        scancode_to_ascii_shift(key)
    } else {
        scancode_to_ascii(key)
    };

    // Aplica Caps Lock apenas para letras
    let ascii = if mods.caps_lock && !mods.shift() && base >= b'a' && base <= b'z' {
        base - 32 // Converte para maiúscula
    } else if mods.caps_lock && mods.shift() && base >= b'A' && base <= b'Z' {
        base + 32 // Converte para minúscula (shift + caps = minúscula)
    } else {
        base
    };

    // Ctrl+C = 0x03, Ctrl+D = 0x04, etc.
    let final_char = if mods.ctrl && ascii >= b'a' && ascii <= b'z' {
        ascii - b'a' + 1
    } else if mods.ctrl && ascii >= b'A' && ascii <= b'Z' {
        ascii - b'A' + 1
    } else {
        ascii
    };

    // Adiciona ao buffer se é um caractere válido
    if final_char != 0 {
        let mut buf = INPUT_BUFFER.lock();
        if buf.len() < BUFFER_CAPACITY {
            buf.push_back(final_char);
        }
    }
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
