//! Time management
//!
//! Gerencia tempo do sistema, relógios e timers.

#![allow(dead_code)]

use core::sync::atomic::{AtomicU64, Ordering};

/// Ticks desde o boot (incrementado pelo timer IRQ)
static TICKS: AtomicU64 = AtomicU64::new(0);

/// Frequência do timer em Hz (100 = 10ms por tick)
const TIMER_HZ: u64 = 100;

/// Nanosegundos por tick
const NS_PER_TICK: u64 = 1_000_000_000 / TIMER_HZ;

/// Epoch Unix em segundos (boot time - simplificado)
/// Em um OS real, isso viria do RTC
static BOOT_TIME_SECS: AtomicU64 = AtomicU64::new(1704067200); // 2024-01-01 00:00:00 UTC

/// Chamado pelo timer IRQ handler
pub fn tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

/// Retorna ticks desde o boot
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// Retorna tempo desde o boot em nanosegundos
pub fn uptime_ns() -> u64 {
    ticks() * NS_PER_TICK
}

/// Retorna tempo desde o boot em milissegundos
pub fn uptime_ms() -> u64 {
    (ticks() * 1000) / TIMER_HZ
}

/// Retorna tempo desde o boot em segundos
pub fn uptime_secs() -> u64 {
    ticks() / TIMER_HZ
}

/// Estrutura timespec (para clock_gettime)
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Timespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

/// Estrutura timeval (para gettimeofday)
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Timeval {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

/// Estrutura timezone
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Timezone {
    pub tz_minuteswest: i32,
    pub tz_dsttime: i32,
}

/// IDs de clock
pub mod clock {
    pub const CLOCK_REALTIME: i32 = 0;
    pub const CLOCK_MONOTONIC: i32 = 1;
    pub const CLOCK_PROCESS_CPUTIME_ID: i32 = 2;
    pub const CLOCK_THREAD_CPUTIME_ID: i32 = 3;
    pub const CLOCK_MONOTONIC_RAW: i32 = 4;
    pub const CLOCK_REALTIME_COARSE: i32 = 5;
    pub const CLOCK_MONOTONIC_COARSE: i32 = 6;
    pub const CLOCK_BOOTTIME: i32 = 7;
}

/// Obtém o tempo real (wall clock)
pub fn realtime() -> Timespec {
    let uptime = uptime_ns();
    let boot_secs = BOOT_TIME_SECS.load(Ordering::Relaxed);

    let total_ns = boot_secs * 1_000_000_000 + uptime;
    Timespec {
        tv_sec: (total_ns / 1_000_000_000) as i64,
        tv_nsec: (total_ns % 1_000_000_000) as i64,
    }
}

/// Obtém tempo monotônico (desde o boot)
pub fn monotonic() -> Timespec {
    let uptime = uptime_ns();
    Timespec {
        tv_sec: (uptime / 1_000_000_000) as i64,
        tv_nsec: (uptime % 1_000_000_000) as i64,
    }
}

/// clock_gettime syscall
pub fn clock_gettime(clock_id: i32) -> Option<Timespec> {
    match clock_id {
        clock::CLOCK_REALTIME | clock::CLOCK_REALTIME_COARSE => Some(realtime()),
        clock::CLOCK_MONOTONIC
        | clock::CLOCK_MONOTONIC_RAW
        | clock::CLOCK_MONOTONIC_COARSE
        | clock::CLOCK_BOOTTIME => Some(monotonic()),
        clock::CLOCK_PROCESS_CPUTIME_ID | clock::CLOCK_THREAD_CPUTIME_ID => {
            // CPU time = uptime por simplicidade
            Some(monotonic())
        }
        _ => None,
    }
}

/// gettimeofday syscall
pub fn gettimeofday() -> (Timeval, Timezone) {
    let ts = realtime();
    let tv = Timeval {
        tv_sec: ts.tv_sec,
        tv_usec: ts.tv_nsec / 1000,
    };
    let tz = Timezone {
        tz_minuteswest: 0,
        tz_dsttime: 0,
    };
    (tv, tz)
}

/// nanosleep - dorme por um período
pub fn nanosleep(req: &Timespec) -> Timespec {
    let sleep_ns = req.tv_sec as u64 * 1_000_000_000 + req.tv_nsec as u64;
    let sleep_ticks = (sleep_ns + NS_PER_TICK - 1) / NS_PER_TICK;

    let start = ticks();
    let target = start + sleep_ticks;

    while ticks() < target {
        // Yield para outros processos
        crate::task::yield_now();
    }

    // Retorna tempo restante (0 se dormiu tudo)
    Timespec { tv_sec: 0, tv_nsec: 0 }
}

/// Inicializa o RTC e obtém hora real
pub fn init() {
    // Lê o RTC para obter a hora real
    let rtc_time = read_rtc();
    if rtc_time > 0 {
        BOOT_TIME_SECS.store(rtc_time, Ordering::Relaxed);
        crate::kprintln!("time: RTC epoch = {}", rtc_time);
    }
    crate::kprintln!("time: timer hz = {}", TIMER_HZ);
}

/// Lê segundos desde epoch do RTC (CMOS)
fn read_rtc() -> u64 {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut addr = Port::<u8>::new(0x70);
        let mut data = Port::<u8>::new(0x71);

        // Espera RTC não estar em update
        loop {
            addr.write(0x0A);
            if data.read() & 0x80 == 0 {
                break;
            }
        }

        // Lê valores
        addr.write(0x00);
        let sec = bcd_to_bin(data.read());
        addr.write(0x02);
        let min = bcd_to_bin(data.read());
        addr.write(0x04);
        let hour = bcd_to_bin(data.read());
        addr.write(0x07);
        let day = bcd_to_bin(data.read());
        addr.write(0x08);
        let month = bcd_to_bin(data.read());
        addr.write(0x09);
        let year = bcd_to_bin(data.read()) as u64 + 2000;

        // Converte para Unix timestamp (simplificado, sem leap seconds)
        let mut days: u64 = 0;

        // Anos desde 1970
        for y in 1970..year {
            days += if is_leap_year(y) { 366 } else { 365 };
        }

        // Meses no ano atual
        let days_in_month = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        for m in 1..month {
            days += days_in_month[m as usize - 1] as u64;
            if m == 2 && is_leap_year(year) {
                days += 1;
            }
        }

        // Dias no mês
        days += day as u64 - 1;

        // Converte para segundos
        days * 86400 + hour as u64 * 3600 + min as u64 * 60 + sec as u64
    }
}

fn bcd_to_bin(bcd: u8) -> u8 {
    (bcd & 0x0F) + ((bcd >> 4) * 10)
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
