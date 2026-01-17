//! Console unificado - combina teclado PS/2 e serial para input/output.
//!
//! Permite que o sistema funcione tanto em QEMU (via serial) quanto
//! em hardware real (via teclado PS/2).

#![allow(dead_code)]

use core::fmt;

/// Lê um byte de qualquer fonte de input disponível (teclado PS/2 ou serial).
/// Retorna None se nenhum input estiver disponível.
pub fn read_byte() -> Option<u8> {
    // Prioridade 1: Teclado PS/2
    if let Some(b) = crate::drivers::keyboard::read_char() {
        return Some(b);
    }

    // Prioridade 2: Serial (útil para QEMU)
    if let Some(b) = crate::serial::read_byte() {
        return Some(b);
    }

    None
}

/// Verifica se há input disponível (SEM consumir dados).
pub fn has_input() -> bool {
    crate::drivers::keyboard::has_input() || crate::serial::has_data()
}

/// Alias para has_input() - usado por poll/select.
pub fn has_data() -> bool {
    has_input()
}

/// Escreve um byte no console (serial).
pub fn write_byte(b: u8) {
    crate::serial::write_byte(b);
}

/// Escreve uma string no console.
pub fn write_str(s: &str) {
    for &b in s.as_bytes() {
        if b == b'\n' {
            write_byte(b'\r');
        }
        write_byte(b);
    }
}

/// Imprime formatado no console.
pub fn print(args: fmt::Arguments) {
    crate::serial::print(args);
}

/// Lê um byte bloqueando até haver input.
pub fn read_byte_blocking() -> u8 {
    loop {
        if let Some(b) = read_byte() {
            return b;
        }
        // Yield para outras tasks
        crate::task::yield_now();
    }
}

/// Lê uma linha de input com echo e edição básica.
/// Retorna quando Enter é pressionado.
pub fn read_line(buf: &mut alloc::string::String) {
    buf.clear();

    loop {
        let b = read_byte_blocking();

        match b {
            b'\r' | b'\n' => {
                crate::kprintln!("");
                break;
            }
            8 | 127 => {
                // Backspace
                if !buf.is_empty() {
                    buf.pop();
                    // Apaga caractere na tela: backspace, espaço, backspace
                    crate::kprint!("\x08 \x08");
                }
            }
            3 => {
                // Ctrl+C - cancela linha
                crate::kprintln!("^C");
                buf.clear();
                break;
            }
            4 => {
                // Ctrl+D - EOF (para shells seria sair)
                crate::kprintln!("^D");
                break;
            }
            b if b.is_ascii_graphic() || b == b' ' => {
                buf.push(b as char);
                crate::kprint!("{}", b as char);
            }
            _ => {
                // Ignora outros caracteres de controle
            }
        }
    }
}
