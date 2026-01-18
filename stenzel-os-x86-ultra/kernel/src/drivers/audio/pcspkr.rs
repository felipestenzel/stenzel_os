//! PC Speaker (Beep) Driver
//!
//! Implements the classic PC speaker using the PIT (Programmable Interval Timer)
//! channel 2 to generate square waves at various frequencies.
//!
//! This is the simplest form of audio output on x86 systems, capable of
//! producing monophonic beeps and tones.

#![allow(dead_code)]

extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::Mutex;
use x86_64::instructions::port::Port;

// ============================================================================
// Constants
// ============================================================================

/// PIT frequency (1.193182 MHz)
const PIT_FREQUENCY: u32 = 1193182;

/// PIT channel 2 data port
const PIT_CHANNEL2_PORT: u16 = 0x42;

/// PIT command port
const PIT_COMMAND_PORT: u16 = 0x43;

/// PC speaker control port (port B)
const SPEAKER_PORT: u16 = 0x61;

/// PIT command: channel 2, lobyte/hibyte, mode 3 (square wave)
const PIT_CMD_CHANNEL2_SQUARE: u8 = 0xB6;

// Common beep frequencies
pub const FREQ_BEEP: u32 = 1000;      // Standard beep
pub const FREQ_ERROR: u32 = 440;       // Error tone (A4)
pub const FREQ_SUCCESS: u32 = 2000;    // Success chirp
pub const FREQ_WARNING: u32 = 800;     // Warning tone
pub const FREQ_CLICK: u32 = 4000;      // Click sound

// Musical notes (A4 = 440Hz standard)
pub mod notes {
    pub const C4: u32 = 262;
    pub const CS4: u32 = 277;
    pub const D4: u32 = 294;
    pub const DS4: u32 = 311;
    pub const E4: u32 = 330;
    pub const F4: u32 = 349;
    pub const FS4: u32 = 370;
    pub const G4: u32 = 392;
    pub const GS4: u32 = 415;
    pub const A4: u32 = 440;
    pub const AS4: u32 = 466;
    pub const B4: u32 = 494;
    pub const C5: u32 = 523;
    pub const CS5: u32 = 554;
    pub const D5: u32 = 587;
    pub const DS5: u32 = 622;
    pub const E5: u32 = 659;
    pub const F5: u32 = 698;
    pub const FS5: u32 = 740;
    pub const G5: u32 = 784;
    pub const GS5: u32 = 831;
    pub const A5: u32 = 880;
    pub const AS5: u32 = 932;
    pub const B5: u32 = 988;
    pub const C6: u32 = 1047;

    /// Get note frequency from MIDI note number (60 = C4)
    pub fn from_midi(note: u8) -> u32 {
        // A4 (440Hz) is MIDI note 69
        // Use lookup table instead of floating point pow
        // Each semitone is multiplied by 2^(1/12) ≈ 1.05946
        const FREQ_TABLE: [u32; 128] = [
            8, 9, 9, 10, 10, 11, 12, 12, 13, 14, 15, 15, // C-1 to B-1
            16, 17, 18, 19, 21, 22, 23, 25, 26, 28, 29, 31, // C0 to B0
            33, 35, 37, 39, 41, 44, 46, 49, 52, 55, 58, 62, // C1 to B1
            65, 69, 73, 78, 82, 87, 93, 98, 104, 110, 117, 123, // C2 to B2
            131, 139, 147, 156, 165, 175, 185, 196, 208, 220, 233, 247, // C3 to B3
            262, 277, 294, 311, 330, 349, 370, 392, 415, 440, 466, 494, // C4 to B4
            523, 554, 587, 622, 659, 698, 740, 784, 831, 880, 932, 988, // C5 to B5
            1047, 1109, 1175, 1245, 1319, 1397, 1480, 1568, 1661, 1760, 1865, 1976, // C6 to B6
            2093, 2217, 2349, 2489, 2637, 2794, 2960, 3136, 3322, 3520, 3729, 3951, // C7 to B7
            4186, 4435, 4699, 4978, 5274, 5588, 5920, 6272, 6645, 7040, 7459, 7902, // C8 to B8
            8372, 8870, 9397, 9956, 10548, 11175, 11840, 12544, // C9 to G9
        ];
        FREQ_TABLE.get(note as usize).copied().unwrap_or(440)
    }
}

// ============================================================================
// PC Speaker State
// ============================================================================

/// PC Speaker state
struct PcSpeakerState {
    /// Is the speaker currently making sound?
    playing: bool,
    /// Current frequency
    current_freq: u32,
    /// Is the speaker enabled?
    enabled: bool,
    /// Volume (0-100, affects duty cycle approximation)
    volume: u8,
}

impl PcSpeakerState {
    const fn new() -> Self {
        Self {
            playing: false,
            current_freq: 0,
            enabled: true,
            volume: 100,
        }
    }
}

static SPEAKER_STATE: Mutex<PcSpeakerState> = Mutex::new(PcSpeakerState::new());
static SPEAKER_PLAYING: AtomicBool = AtomicBool::new(false);
static CURRENT_FREQUENCY: AtomicU32 = AtomicU32::new(0);

// ============================================================================
// Low-level Hardware Control
// ============================================================================

/// Set the PIT channel 2 frequency
unsafe fn set_pit_frequency(frequency: u32) {
    if frequency == 0 {
        return;
    }

    let divisor = PIT_FREQUENCY / frequency;

    // Clamp divisor to valid range
    let divisor = divisor.clamp(1, 65535) as u16;

    let mut cmd_port = Port::<u8>::new(PIT_COMMAND_PORT);
    let mut data_port = Port::<u8>::new(PIT_CHANNEL2_PORT);

    // Set PIT to square wave mode on channel 2
    cmd_port.write(PIT_CMD_CHANNEL2_SQUARE);

    // Write divisor (low byte first, then high byte)
    data_port.write((divisor & 0xFF) as u8);
    data_port.write((divisor >> 8) as u8);
}

/// Enable the PC speaker
unsafe fn enable_speaker() {
    let mut port = Port::<u8>::new(SPEAKER_PORT);
    let value = port.read();
    // Set bits 0 and 1 to enable speaker and connect to PIT channel 2
    port.write(value | 0x03);
}

/// Disable the PC speaker
unsafe fn disable_speaker() {
    let mut port = Port::<u8>::new(SPEAKER_PORT);
    let value = port.read();
    // Clear bits 0 and 1 to disable speaker
    port.write(value & 0xFC);
}

/// Check if speaker is currently enabled
unsafe fn is_speaker_enabled() -> bool {
    let mut port = Port::<u8>::new(SPEAKER_PORT);
    (port.read() & 0x03) == 0x03
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize the PC speaker
pub fn init() {
    crate::kprintln!("pcspkr: PC speaker initialized");
}

/// Play a tone at the specified frequency
pub fn play_tone(frequency: u32) {
    if frequency == 0 {
        stop();
        return;
    }

    let mut state = SPEAKER_STATE.lock();
    if !state.enabled {
        return;
    }

    unsafe {
        set_pit_frequency(frequency);
        enable_speaker();
    }

    state.playing = true;
    state.current_freq = frequency;
    SPEAKER_PLAYING.store(true, Ordering::Release);
    CURRENT_FREQUENCY.store(frequency, Ordering::Release);
}

/// Stop any current sound
pub fn stop() {
    unsafe {
        disable_speaker();
    }

    let mut state = SPEAKER_STATE.lock();
    state.playing = false;
    state.current_freq = 0;
    SPEAKER_PLAYING.store(false, Ordering::Release);
    CURRENT_FREQUENCY.store(0, Ordering::Release);
}

/// Play a beep for a specified duration (in milliseconds)
/// Note: This is a blocking call
pub fn beep(frequency: u32, duration_ms: u32) {
    play_tone(frequency);
    delay_ms(duration_ms);
    stop();
}

/// Play the standard system beep
pub fn system_beep() {
    beep(FREQ_BEEP, 100);
}

/// Play an error beep
pub fn error_beep() {
    beep(FREQ_ERROR, 200);
    delay_ms(100);
    beep(FREQ_ERROR, 200);
}

/// Play a success chirp
pub fn success_beep() {
    beep(FREQ_SUCCESS, 100);
}

/// Play a warning beep
pub fn warning_beep() {
    for _ in 0..3 {
        beep(FREQ_WARNING, 100);
        delay_ms(100);
    }
}

/// Play a click sound
pub fn click() {
    beep(FREQ_CLICK, 10);
}

/// Play a sequence of notes
/// Each note is a tuple of (frequency, duration_ms)
/// A frequency of 0 means a rest/pause
pub fn play_melody(notes: &[(u32, u32)]) {
    for &(freq, duration) in notes {
        if freq == 0 {
            stop();
            delay_ms(duration);
        } else {
            beep(freq, duration);
        }
        delay_ms(20); // Small gap between notes
    }
}

/// Play a startup jingle
pub fn startup_sound() {
    let melody = [
        (notes::C5, 100),
        (notes::E5, 100),
        (notes::G5, 100),
        (notes::C6, 200),
    ];
    play_melody(&melody);
}

/// Play a shutdown sound
pub fn shutdown_sound() {
    let melody = [
        (notes::G5, 150),
        (notes::E5, 150),
        (notes::C5, 200),
    ];
    play_melody(&melody);
}

/// Play a notification sound
pub fn notification_sound() {
    let melody = [
        (notes::E5, 80),
        (notes::G5, 120),
    ];
    play_melody(&melody);
}

/// Is the speaker currently playing?
pub fn is_playing() -> bool {
    SPEAKER_PLAYING.load(Ordering::Acquire)
}

/// Get the current frequency being played
pub fn current_frequency() -> u32 {
    CURRENT_FREQUENCY.load(Ordering::Acquire)
}

/// Enable or disable the PC speaker
pub fn set_enabled(enabled: bool) {
    let mut state = SPEAKER_STATE.lock();
    state.enabled = enabled;
    if !enabled {
        drop(state);
        stop();
    }
}

/// Check if the speaker is enabled
pub fn is_enabled() -> bool {
    SPEAKER_STATE.lock().enabled
}

/// Set volume (0-100)
/// Note: PC speaker doesn't have real volume control, this is for API compatibility
pub fn set_volume(volume: u8) {
    SPEAKER_STATE.lock().volume = volume.min(100);
}

/// Get current volume
pub fn get_volume() -> u8 {
    SPEAKER_STATE.lock().volume
}

// ============================================================================
// Delay Helper (using PIT or busy loop)
// ============================================================================

/// Simple delay in milliseconds
fn delay_ms(ms: u32) {
    // Use a simple busy-wait loop
    // This assumes roughly 1GHz CPU, will be inaccurate but sufficient for beeps
    // In a real implementation, this would use the kernel timer
    for _ in 0..ms {
        for _ in 0..1_000_000 {
            core::hint::spin_loop();
        }
    }
}

// ============================================================================
// Simple Music Notation Parser
// ============================================================================

/// Parse and play a simple music string
/// Format: "NOTE[OCTAVE][DURATION] ..." (e.g., "C4Q E4Q G4Q C5H")
/// Notes: C, D, E, F, G, A, B (# for sharp, b for flat)
/// Octaves: 0-8 (default 4)
/// Durations: W=whole, H=half, Q=quarter, E=eighth, S=sixteenth (default Q)
pub fn play_music(music: &str) {
    let base_duration = 400u32; // Quarter note duration in ms

    for token in music.split_whitespace() {
        if token.is_empty() {
            continue;
        }

        let chars: Vec<char> = token.chars().collect();
        if chars.is_empty() {
            continue;
        }

        // Parse note
        let (note_base, sharp, flat, idx) = parse_note_name(&chars);

        // Parse octave
        let (octave, idx) = parse_octave(&chars, idx);

        // Parse duration
        let duration = parse_duration(&chars, idx, base_duration);

        // Calculate frequency
        if let Some(freq) = note_to_frequency(note_base, sharp, flat, octave) {
            beep(freq, duration);
        } else {
            // Rest
            delay_ms(duration);
        }
    }
}

fn parse_note_name(chars: &[char]) -> (char, bool, bool, usize) {
    let note = chars[0].to_ascii_uppercase();
    let mut idx = 1;
    let mut sharp = false;
    let mut flat = false;

    if idx < chars.len() {
        if chars[idx] == '#' {
            sharp = true;
            idx += 1;
        } else if chars[idx] == 'b' {
            flat = true;
            idx += 1;
        }
    }

    (note, sharp, flat, idx)
}

fn parse_octave(chars: &[char], start: usize) -> (u8, usize) {
    if start < chars.len() && chars[start].is_ascii_digit() {
        let octave = (chars[start] as u8) - b'0';
        (octave, start + 1)
    } else {
        (4, start) // Default octave
    }
}

fn parse_duration(chars: &[char], start: usize, base: u32) -> u32 {
    if start < chars.len() {
        match chars[start].to_ascii_uppercase() {
            'W' => base * 4,
            'H' => base * 2,
            'Q' => base,
            'E' => base / 2,
            'S' => base / 4,
            _ => base,
        }
    } else {
        base
    }
}

fn note_to_frequency(note: char, sharp: bool, flat: bool, octave: u8) -> Option<u32> {
    let base_note = match note {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        'R' | 'P' => return None, // Rest/Pause
        _ => return None,
    };

    let mut semitone: i32 = base_note;
    if sharp {
        semitone += 1;
    }
    if flat {
        semitone = semitone.saturating_sub(1);
    }

    // MIDI note number (C4 = 60)
    let midi = (octave as i32 + 1) * 12 + semitone;

    Some(notes::from_midi(midi as u8))
}

// ============================================================================
// BIOS Beep Codes (for diagnostic purposes)
// ============================================================================

/// BIOS-style beep code patterns
pub mod bios_codes {
    use super::*;

    /// POST success (1 short beep)
    pub fn post_success() {
        beep(FREQ_BEEP, 200);
    }

    /// Memory error (3 short beeps)
    pub fn memory_error() {
        for _ in 0..3 {
            beep(FREQ_ERROR, 200);
            delay_ms(100);
        }
    }

    /// Video error (1 long, 2 short)
    pub fn video_error() {
        beep(FREQ_ERROR, 500);
        delay_ms(200);
        beep(FREQ_ERROR, 200);
        delay_ms(100);
        beep(FREQ_ERROR, 200);
    }

    /// Keyboard error (1 long, 3 short)
    pub fn keyboard_error() {
        beep(FREQ_ERROR, 500);
        delay_ms(200);
        for _ in 0..3 {
            beep(FREQ_ERROR, 200);
            delay_ms(100);
        }
    }

    /// Generic fatal error (continuous beep)
    pub fn fatal_error() {
        for _ in 0..5 {
            beep(FREQ_ERROR, 500);
            delay_ms(500);
        }
    }
}

// ============================================================================
// Sysfs Interface Helpers
// ============================================================================

/// Format speaker status for sysfs
pub fn format_status() -> &'static str {
    if is_playing() {
        "playing"
    } else if is_enabled() {
        "ready"
    } else {
        "disabled"
    }
}

/// Format frequency info for sysfs
pub fn format_frequency_info() -> alloc::string::String {
    extern crate alloc;
    let freq = current_frequency();
    if freq > 0 {
        alloc::format!("{} Hz", freq)
    } else {
        alloc::string::String::from("0 Hz (silent)")
    }
}
