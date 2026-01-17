//! Driver de mouse PS/2 (i8042 auxiliary device)
//!
//! O mouse PS/2 usa o mesmo controlador i8042 do teclado, mas através
//! do dispositivo auxiliar (porta 2). Gera IRQ12 (vetor 44).
//!
//! Protocolo padrão: 3 bytes por pacote
//! - Byte 1: Bits de status (botões, overflow, sinal)
//! - Byte 2: Movimento X (signed)
//! - Byte 3: Movimento Y (signed)

use alloc::collections::VecDeque;
use spin::Mutex;
use x86_64::instructions::port::{PortReadOnly, PortWriteOnly};

/// Eventos do mouse
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    pub x_delta: i16,
    pub y_delta: i16,
    pub left_button: bool,
    pub right_button: bool,
    pub middle_button: bool,
}

/// Buffer de eventos do mouse
static EVENT_BUFFER: Mutex<VecDeque<MouseEvent>> = Mutex::new(VecDeque::new());

/// Estado do pacote em construção
static PACKET_STATE: Mutex<PacketState> = Mutex::new(PacketState::new());

/// Posição atual do cursor
static CURSOR_POS: Mutex<CursorPosition> = Mutex::new(CursorPosition { x: 0, y: 0 });

/// Capacidade máxima do buffer de eventos
const BUFFER_CAPACITY: usize = 64;

/// Limites da tela (para confinamento do cursor)
const SCREEN_WIDTH: i32 = 800;
const SCREEN_HEIGHT: i32 = 600;

/// Portas do i8042
const I8042_DATA_PORT: u16 = 0x60;
const I8042_STATUS_PORT: u16 = 0x64;
const I8042_COMMAND_PORT: u16 = 0x64;

/// Comandos do i8042 controller
const CMD_READ_CONFIG: u8 = 0x20;
const CMD_WRITE_CONFIG: u8 = 0x60;
const CMD_DISABLE_MOUSE: u8 = 0xA7;
const CMD_ENABLE_MOUSE: u8 = 0xA8;
const CMD_TEST_MOUSE: u8 = 0xA9;
const CMD_WRITE_MOUSE: u8 = 0xD4;

/// Comandos do mouse
const MOUSE_CMD_RESET: u8 = 0xFF;
const MOUSE_CMD_ENABLE_DATA: u8 = 0xF4;
const MOUSE_CMD_DISABLE_DATA: u8 = 0xF5;
const MOUSE_CMD_SET_DEFAULTS: u8 = 0xF6;
const MOUSE_CMD_SET_SAMPLE_RATE: u8 = 0xF3;
const MOUSE_CMD_GET_ID: u8 = 0xF2;

/// Respostas do mouse
const MOUSE_ACK: u8 = 0xFA;
const MOUSE_RESEND: u8 = 0xFE;
const MOUSE_RESET_OK: u8 = 0xAA;

/// Bits do byte de status do pacote
const STATUS_LEFT_BTN: u8 = 0x01;
const STATUS_RIGHT_BTN: u8 = 0x02;
const STATUS_MIDDLE_BTN: u8 = 0x04;
const STATUS_ALWAYS_ONE: u8 = 0x08;
const STATUS_X_SIGN: u8 = 0x10;
const STATUS_Y_SIGN: u8 = 0x20;
const STATUS_X_OVERFLOW: u8 = 0x40;
const STATUS_Y_OVERFLOW: u8 = 0x80;

/// Estado do construtor de pacotes
#[derive(Debug, Clone, Copy)]
struct PacketState {
    byte_index: u8,
    bytes: [u8; 3],
}

impl PacketState {
    const fn new() -> Self {
        Self {
            byte_index: 0,
            bytes: [0; 3],
        }
    }
}

/// Posição do cursor na tela
#[derive(Debug, Clone, Copy)]
pub struct CursorPosition {
    pub x: i32,
    pub y: i32,
}

/// Espera até poder ler do controller
fn wait_read() {
    let mut status: PortReadOnly<u8> = PortReadOnly::new(I8042_STATUS_PORT);
    for _ in 0..100_000 {
        if unsafe { status.read() } & 0x01 != 0 {
            return;
        }
        core::hint::spin_loop();
    }
}

/// Espera até poder escrever no controller
fn wait_write() {
    let mut status: PortReadOnly<u8> = PortReadOnly::new(I8042_STATUS_PORT);
    for _ in 0..100_000 {
        if unsafe { status.read() } & 0x02 == 0 {
            return;
        }
        core::hint::spin_loop();
    }
}

/// Envia um comando para o controller i8042
fn send_controller_cmd(cmd: u8) {
    wait_write();
    let mut port: PortWriteOnly<u8> = PortWriteOnly::new(I8042_COMMAND_PORT);
    unsafe { port.write(cmd) };
}

/// Envia um comando para o mouse (via controller)
fn send_mouse_cmd(cmd: u8) {
    send_controller_cmd(CMD_WRITE_MOUSE);
    wait_write();
    let mut port: PortWriteOnly<u8> = PortWriteOnly::new(I8042_DATA_PORT);
    unsafe { port.write(cmd) };
}

/// Lê um byte do data port
fn read_data() -> u8 {
    wait_read();
    let mut port: PortReadOnly<u8> = PortReadOnly::new(I8042_DATA_PORT);
    unsafe { port.read() }
}

/// Lê um byte do data port sem esperar
fn read_data_nowait() -> Option<u8> {
    let mut status: PortReadOnly<u8> = PortReadOnly::new(I8042_STATUS_PORT);
    if unsafe { status.read() } & 0x01 != 0 {
        let mut port: PortReadOnly<u8> = PortReadOnly::new(I8042_DATA_PORT);
        Some(unsafe { port.read() })
    } else {
        None
    }
}

/// Escreve um byte no data port
fn write_data(data: u8) {
    wait_write();
    let mut port: PortWriteOnly<u8> = PortWriteOnly::new(I8042_DATA_PORT);
    unsafe { port.write(data) };
}

/// Espera ACK do mouse
fn wait_ack() -> bool {
    for _ in 0..10 {
        let data = read_data();
        if data == MOUSE_ACK {
            return true;
        }
        if data == MOUSE_RESEND {
            return false;
        }
    }
    false
}

/// Inicializa o mouse PS/2
pub fn init() {
    // Habilita o dispositivo auxiliar (mouse)
    send_controller_cmd(CMD_ENABLE_MOUSE);

    // Lê configuração atual do controller
    send_controller_cmd(CMD_READ_CONFIG);
    let mut config = read_data();

    // Habilita IRQ12 (bit 1) e mantém clock do mouse habilitado (bit 5 = 0)
    config |= 0x02;   // Habilita IRQ do mouse
    config &= !0x20;  // Habilita clock do mouse

    // Escreve configuração de volta
    send_controller_cmd(CMD_WRITE_CONFIG);
    write_data(config);

    // Reseta o mouse
    send_mouse_cmd(MOUSE_CMD_RESET);
    let _ack = read_data(); // ACK
    let reset_result = read_data(); // Deve ser 0xAA (self-test passed)
    let _device_id = read_data(); // Device ID (0x00 para mouse padrão)

    if reset_result != MOUSE_RESET_OK {
        crate::kprintln!("mouse: reset falhou ({:#x})", reset_result);
        return;
    }

    // Define valores padrão
    send_mouse_cmd(MOUSE_CMD_SET_DEFAULTS);
    if !wait_ack() {
        crate::kprintln!("mouse: set_defaults falhou");
        return;
    }

    // Define taxa de amostragem (100 amostras/seg)
    send_mouse_cmd(MOUSE_CMD_SET_SAMPLE_RATE);
    wait_ack();
    send_mouse_cmd(100);
    wait_ack();

    // Habilita envio de dados
    send_mouse_cmd(MOUSE_CMD_ENABLE_DATA);
    if !wait_ack() {
        crate::kprintln!("mouse: enable_data falhou");
        return;
    }

    crate::kprintln!("mouse: PS/2 mouse inicializado");
}

/// Processa um byte recebido da IRQ do mouse.
/// Chamado pelo interrupt handler (IRQ12).
pub fn process_byte(byte: u8) {
    let mut state = PACKET_STATE.lock();

    // Verifica sincronização no primeiro byte
    if state.byte_index == 0 {
        // O primeiro byte deve ter bit 3 sempre setado
        if (byte & STATUS_ALWAYS_ONE) == 0 {
            // Byte dessincronizado, descarta
            return;
        }
    }

    let idx = state.byte_index as usize;
    state.bytes[idx] = byte;
    state.byte_index += 1;

    // Pacote completo (3 bytes)
    if state.byte_index >= 3 {
        let b1 = state.bytes[0];
        let b2 = state.bytes[1];
        let b3 = state.bytes[2];

        state.byte_index = 0;

        // Descarta pacotes com overflow
        if (b1 & (STATUS_X_OVERFLOW | STATUS_Y_OVERFLOW)) != 0 {
            return;
        }

        // Calcula movimento X (signed 9-bit, estendido para i16)
        let x_delta = if (b1 & STATUS_X_SIGN) != 0 {
            // Negativo: extensão de sinal
            ((b2 as u16) | 0xFF00) as i16
        } else {
            b2 as i16
        };

        // Calcula movimento Y (signed 9-bit, estendido para i16)
        // Nota: Y é invertido no protocolo PS/2 (positivo = para cima)
        let y_delta = if (b1 & STATUS_Y_SIGN) != 0 {
            // Negativo: extensão de sinal
            -(((b3 as u16) | 0xFF00) as i16)
        } else {
            -(b3 as i16)
        };

        // Extrai estado dos botões
        let left_button = (b1 & STATUS_LEFT_BTN) != 0;
        let right_button = (b1 & STATUS_RIGHT_BTN) != 0;
        let middle_button = (b1 & STATUS_MIDDLE_BTN) != 0;

        // Atualiza posição do cursor
        {
            let mut pos = CURSOR_POS.lock();
            pos.x = (pos.x + x_delta as i32).clamp(0, SCREEN_WIDTH - 1);
            pos.y = (pos.y + y_delta as i32).clamp(0, SCREEN_HEIGHT - 1);
        }

        // Cria evento
        let event = MouseEvent {
            x_delta,
            y_delta,
            left_button,
            right_button,
            middle_button,
        };

        // Adiciona ao buffer de eventos
        let mut buf = EVENT_BUFFER.lock();
        if buf.len() < BUFFER_CAPACITY {
            buf.push_back(event);
        }

        // Drop the lock before calling input functions to avoid deadlock
        drop(buf);

        // Report to input event system
        super::input::report_mouse_move(x_delta as i32, y_delta as i32);
        // Track button state and report changes
        report_button_changes(left_button, right_button, middle_button);
    }
}

/// Previous button state for change detection
static PREV_BUTTONS: Mutex<(bool, bool, bool)> = Mutex::new((false, false, false));

/// Report button state changes to input system
fn report_button_changes(left: bool, right: bool, middle: bool) {
    let mut prev = PREV_BUTTONS.lock();
    let (prev_left, prev_right, prev_middle) = *prev;

    if left != prev_left {
        super::input::report_mouse_button(super::input::KeyCode::BtnLeft as u16, left);
    }
    if right != prev_right {
        super::input::report_mouse_button(super::input::KeyCode::BtnRight as u16, right);
    }
    if middle != prev_middle {
        super::input::report_mouse_button(super::input::KeyCode::BtnMiddle as u16, middle);
    }

    *prev = (left, right, middle);
}

/// Lê um evento do mouse (não-bloqueante).
/// Retorna None se não houver eventos.
pub fn read_event() -> Option<MouseEvent> {
    EVENT_BUFFER.lock().pop_front()
}

/// Verifica se há eventos disponíveis.
pub fn has_events() -> bool {
    !EVENT_BUFFER.lock().is_empty()
}

/// Queue a mouse event from USB HID or other sources.
/// This allows unified mouse handling regardless of input source.
pub fn queue_event(x_delta: i16, y_delta: i16, left: bool, right: bool, middle: bool) {
    // Update cursor position
    {
        let mut pos = CURSOR_POS.lock();
        pos.x = (pos.x + x_delta as i32).clamp(0, SCREEN_WIDTH - 1);
        pos.y = (pos.y + y_delta as i32).clamp(0, SCREEN_HEIGHT - 1);
    }

    // Create event
    let event = MouseEvent {
        x_delta,
        y_delta,
        left_button: left,
        right_button: right,
        middle_button: middle,
    };

    // Add to event buffer
    let mut buf = EVENT_BUFFER.lock();
    if buf.len() < BUFFER_CAPACITY {
        buf.push_back(event);
    }
    drop(buf);

    // Report to input event system
    super::input::report_mouse_move(x_delta as i32, y_delta as i32);
    report_button_changes(left, right, middle);
}

/// Obtém a posição atual do cursor.
pub fn cursor_position() -> CursorPosition {
    *CURSOR_POS.lock()
}

/// Define a posição do cursor.
pub fn set_cursor_position(x: i32, y: i32) {
    let mut pos = CURSOR_POS.lock();
    pos.x = x.clamp(0, SCREEN_WIDTH - 1);
    pos.y = y.clamp(0, SCREEN_HEIGHT - 1);
}

/// Define os limites da tela (para confinamento do cursor).
pub fn set_screen_bounds(width: i32, height: i32) {
    // Para simplificar, usamos constantes. Em um sistema real,
    // isso deveria ser configurável.
    let _ = (width, height);
}
