//! NTP (Network Time Protocol) Client
//!
//! Implements NTPv4 client for time synchronization.
//!
//! ## Protocol
//! - Uses UDP port 123
//! - NTP timestamp: 64-bit (32 bits seconds + 32 bits fraction since 1900-01-01)
//! - Unix timestamp: seconds since 1970-01-01
//!
//! ## Usage
//! ```ignore
//! // Synchronize with a single server
//! ntp::sync_time("pool.ntp.org")?;
//!
//! // Get current offset from server
//! let offset = ntp::get_offset("time.google.com")?;
//! ```

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, AtomicI64, AtomicU32, Ordering};

use super::Ipv4Addr;
use super::udp;
use crate::sync::IrqSafeMutex;

/// NTP port
pub const NTP_PORT: u16 = 123;

/// NTP timestamp epoch difference: seconds between 1900-01-01 and 1970-01-01
pub const NTP_UNIX_EPOCH_DIFF: u64 = 2208988800;

/// Default timeout for NTP requests in milliseconds
pub const DEFAULT_TIMEOUT_MS: u32 = 5000;

/// Maximum allowed clock offset before warning (in seconds)
pub const MAX_OFFSET_WARN: i64 = 3600; // 1 hour

/// NTP Leap Indicator values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LeapIndicator {
    NoWarning = 0,
    LastMinute61Seconds = 1,
    LastMinute59Seconds = 2,
    AlarmCondition = 3,
}

impl From<u8> for LeapIndicator {
    fn from(val: u8) -> Self {
        match val {
            0 => LeapIndicator::NoWarning,
            1 => LeapIndicator::LastMinute61Seconds,
            2 => LeapIndicator::LastMinute59Seconds,
            _ => LeapIndicator::AlarmCondition,
        }
    }
}

/// NTP Mode values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NtpMode {
    Reserved = 0,
    SymmetricActive = 1,
    SymmetricPassive = 2,
    Client = 3,
    Server = 4,
    Broadcast = 5,
    ControlMessage = 6,
    Private = 7,
}

impl From<u8> for NtpMode {
    fn from(val: u8) -> Self {
        match val {
            1 => NtpMode::SymmetricActive,
            2 => NtpMode::SymmetricPassive,
            3 => NtpMode::Client,
            4 => NtpMode::Server,
            5 => NtpMode::Broadcast,
            6 => NtpMode::ControlMessage,
            7 => NtpMode::Private,
            _ => NtpMode::Reserved,
        }
    }
}

/// NTP Stratum values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stratum(pub u8);

impl Stratum {
    pub const UNSPECIFIED: Self = Self(0);
    pub const PRIMARY: Self = Self(1);
    pub const SECONDARY_MIN: Self = Self(2);
    pub const SECONDARY_MAX: Self = Self(15);
    pub const UNSYNCHRONIZED: Self = Self(16);

    pub fn is_synchronized(&self) -> bool {
        self.0 > 0 && self.0 < 16
    }

    pub fn description(&self) -> &'static str {
        match self.0 {
            0 => "unspecified/invalid",
            1 => "primary reference (GPS, atomic clock)",
            2..=15 => "secondary reference",
            16 => "unsynchronized",
            _ => "reserved",
        }
    }
}

/// NTP timestamp (64 bits: 32 seconds + 32 fraction)
#[derive(Debug, Clone, Copy, Default)]
pub struct NtpTimestamp {
    pub seconds: u32,
    pub fraction: u32,
}

impl NtpTimestamp {
    pub const ZERO: Self = Self { seconds: 0, fraction: 0 };

    /// Create from bytes (big-endian)
    pub fn from_bytes(data: &[u8]) -> Self {
        if data.len() < 8 {
            return Self::ZERO;
        }
        Self {
            seconds: u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
            fraction: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
        }
    }

    /// Convert to bytes (big-endian)
    pub fn to_bytes(&self) -> [u8; 8] {
        let mut buf = [0u8; 8];
        buf[0..4].copy_from_slice(&self.seconds.to_be_bytes());
        buf[4..8].copy_from_slice(&self.fraction.to_be_bytes());
        buf
    }

    /// Convert to Unix timestamp (seconds since 1970-01-01)
    pub fn to_unix_secs(&self) -> i64 {
        (self.seconds as i64).saturating_sub(NTP_UNIX_EPOCH_DIFF as i64)
    }

    /// Convert to Unix timestamp with fractional seconds
    pub fn to_unix_secs_f64(&self) -> f64 {
        self.to_unix_secs() as f64 + (self.fraction as f64 / 4294967296.0)
    }

    /// Create from Unix timestamp
    pub fn from_unix_secs(unix_secs: u64) -> Self {
        Self {
            seconds: (unix_secs + NTP_UNIX_EPOCH_DIFF) as u32,
            fraction: 0,
        }
    }

    /// Create from Unix timestamp with nanoseconds
    pub fn from_unix_secs_ns(unix_secs: u64, ns: u32) -> Self {
        // fraction = ns * 2^32 / 10^9
        let fraction = ((ns as u64) << 32) / 1_000_000_000;
        Self {
            seconds: (unix_secs + NTP_UNIX_EPOCH_DIFF) as u32,
            fraction: fraction as u32,
        }
    }

    /// Get current NTP timestamp
    pub fn now() -> Self {
        let ts = crate::time::realtime();
        Self::from_unix_secs_ns(ts.tv_sec as u64, ts.tv_nsec as u32)
    }
}

/// NTP packet structure (48 bytes minimum)
#[derive(Debug, Clone)]
pub struct NtpPacket {
    /// LI (2 bits) + VN (3 bits) + Mode (3 bits)
    pub li_vn_mode: u8,
    /// Stratum level
    pub stratum: Stratum,
    /// Poll interval (log2 seconds)
    pub poll: i8,
    /// Precision (log2 seconds)
    pub precision: i8,
    /// Root delay (32 bits, fixed-point)
    pub root_delay: u32,
    /// Root dispersion (32 bits, fixed-point)
    pub root_dispersion: u32,
    /// Reference identifier (4 bytes)
    pub reference_id: [u8; 4],
    /// Reference timestamp
    pub reference_ts: NtpTimestamp,
    /// Origin timestamp (T1 in client, copied from transmit in server response)
    pub origin_ts: NtpTimestamp,
    /// Receive timestamp (T2 - when server received request)
    pub receive_ts: NtpTimestamp,
    /// Transmit timestamp (T3 - when server sent response)
    pub transmit_ts: NtpTimestamp,
}

impl NtpPacket {
    /// Create a client request packet
    pub fn client_request() -> Self {
        Self {
            li_vn_mode: (0 << 6) | (4 << 3) | 3, // LI=0, VN=4, Mode=Client
            stratum: Stratum::UNSPECIFIED,
            poll: 0,
            precision: 0,
            root_delay: 0,
            root_dispersion: 0,
            reference_id: [0; 4],
            reference_ts: NtpTimestamp::ZERO,
            origin_ts: NtpTimestamp::ZERO,
            receive_ts: NtpTimestamp::ZERO,
            transmit_ts: NtpTimestamp::now(),
        }
    }

    /// Parse packet from bytes
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 48 {
            return None;
        }

        Some(Self {
            li_vn_mode: data[0],
            stratum: Stratum(data[1]),
            poll: data[2] as i8,
            precision: data[3] as i8,
            root_delay: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
            root_dispersion: u32::from_be_bytes([data[8], data[9], data[10], data[11]]),
            reference_id: [data[12], data[13], data[14], data[15]],
            reference_ts: NtpTimestamp::from_bytes(&data[16..24]),
            origin_ts: NtpTimestamp::from_bytes(&data[24..32]),
            receive_ts: NtpTimestamp::from_bytes(&data[32..40]),
            transmit_ts: NtpTimestamp::from_bytes(&data[40..48]),
        })
    }

    /// Serialize packet to bytes
    pub fn to_bytes(&self) -> [u8; 48] {
        let mut buf = [0u8; 48];
        buf[0] = self.li_vn_mode;
        buf[1] = self.stratum.0;
        buf[2] = self.poll as u8;
        buf[3] = self.precision as u8;
        buf[4..8].copy_from_slice(&self.root_delay.to_be_bytes());
        buf[8..12].copy_from_slice(&self.root_dispersion.to_be_bytes());
        buf[12..16].copy_from_slice(&self.reference_id);
        buf[16..24].copy_from_slice(&self.reference_ts.to_bytes());
        buf[24..32].copy_from_slice(&self.origin_ts.to_bytes());
        buf[32..40].copy_from_slice(&self.receive_ts.to_bytes());
        buf[40..48].copy_from_slice(&self.transmit_ts.to_bytes());
        buf
    }

    /// Get leap indicator
    pub fn leap_indicator(&self) -> LeapIndicator {
        LeapIndicator::from((self.li_vn_mode >> 6) & 0x03)
    }

    /// Get version number
    pub fn version(&self) -> u8 {
        (self.li_vn_mode >> 3) & 0x07
    }

    /// Get mode
    pub fn mode(&self) -> NtpMode {
        NtpMode::from(self.li_vn_mode & 0x07)
    }

    /// Get reference ID as string (for stratum 1)
    pub fn reference_id_string(&self) -> String {
        if self.stratum.0 == 1 {
            // Stratum 1: reference ID is ASCII string
            let s: String = self.reference_id
                .iter()
                .filter(|&&b| b >= 0x20 && b < 0x7F)
                .map(|&b| b as char)
                .collect();
            s
        } else {
            // Stratum 2+: reference ID is IP address of upstream server
            alloc::format!("{}.{}.{}.{}",
                self.reference_id[0],
                self.reference_id[1],
                self.reference_id[2],
                self.reference_id[3])
        }
    }
}

/// NTP synchronization result
#[derive(Debug, Clone)]
pub struct NtpResult {
    /// Server IP address
    pub server: Ipv4Addr,
    /// Clock offset (local - server) in milliseconds
    pub offset_ms: i64,
    /// Round-trip delay in milliseconds
    pub delay_ms: u64,
    /// Server stratum
    pub stratum: Stratum,
    /// Reference ID string
    pub reference_id: String,
    /// Leap indicator
    pub leap: LeapIndicator,
}

/// NTP client state
pub struct NtpClient {
    /// Configured NTP servers
    servers: Vec<Ipv4Addr>,
    /// Last synchronization timestamp
    last_sync: AtomicU64,
    /// Last calculated offset in milliseconds
    last_offset_ms: AtomicI64,
    /// Number of successful syncs
    sync_count: AtomicU32,
    /// Number of failed syncs
    fail_count: AtomicU32,
}

impl NtpClient {
    pub const fn new() -> Self {
        Self {
            servers: Vec::new(),
            last_sync: AtomicU64::new(0),
            last_offset_ms: AtomicI64::new(0),
            sync_count: AtomicU32::new(0),
            fail_count: AtomicU32::new(0),
        }
    }
}

/// Global NTP client
static NTP_CLIENT: IrqSafeMutex<Option<NtpClient>> = IrqSafeMutex::new(None);

/// Default NTP servers
static DEFAULT_SERVERS: &[Ipv4Addr] = &[
    Ipv4Addr::new(216, 239, 35, 0),   // time.google.com
    Ipv4Addr::new(216, 239, 35, 4),   // time2.google.com
    Ipv4Addr::new(129, 6, 15, 28),    // time-a-g.nist.gov
    Ipv4Addr::new(129, 6, 15, 29),    // time-b-g.nist.gov
];

/// Initialize the NTP client
pub fn init() {
    let mut client = NtpClient::new();
    client.servers = DEFAULT_SERVERS.to_vec();

    *NTP_CLIENT.lock() = Some(client);
    crate::kprintln!("ntp: client initialized with {} servers", DEFAULT_SERVERS.len());
}

/// Add an NTP server
pub fn add_server(ip: Ipv4Addr) {
    let mut guard = NTP_CLIENT.lock();
    if let Some(client) = guard.as_mut() {
        if !client.servers.contains(&ip) {
            client.servers.push(ip);
        }
    }
}

/// Remove an NTP server
pub fn remove_server(ip: Ipv4Addr) {
    let mut guard = NTP_CLIENT.lock();
    if let Some(client) = guard.as_mut() {
        client.servers.retain(|&s| s != ip);
    }
}

/// Query a single NTP server
pub fn query_server(server_ip: Ipv4Addr, timeout_ms: u32) -> crate::util::KResult<NtpResult> {
    // Allocate a port for our request
    let local_port = udp::allocate_port();

    // Record T1 (client transmit time)
    let t1 = NtpTimestamp::now();

    // Build and send request
    let mut request = NtpPacket::client_request();
    request.transmit_ts = t1;
    let packet = request.to_bytes();

    udp::send(local_port, server_ip, NTP_PORT, &packet)?;

    // Wait for response
    let response = match udp::recv_timeout(local_port, timeout_ms) {
        Some(dgram) => dgram,
        None => {
            udp::unbind(local_port);
            return Err(crate::util::KError::Timeout);
        }
    };

    // Record T4 (client receive time)
    let t4 = NtpTimestamp::now();

    // Clean up
    udp::unbind(local_port);

    // Parse response
    let reply = NtpPacket::parse(&response.data)
        .ok_or(crate::util::KError::Invalid)?;

    // Verify response
    if reply.mode() != NtpMode::Server {
        return Err(crate::util::KError::Invalid);
    }

    // Extract timestamps
    // T1 = origin timestamp (our transmit time, echoed back)
    // T2 = receive timestamp (server received our request)
    // T3 = transmit timestamp (server sent response)
    // T4 = our receive time

    let t2 = reply.receive_ts;
    let t3 = reply.transmit_ts;

    // Calculate offset and delay using NTP algorithm
    // offset = ((T2 - T1) + (T3 - T4)) / 2
    // delay = (T4 - T1) - (T3 - T2)

    // Convert to milliseconds for calculation
    let t1_ms = t1.to_unix_secs() * 1000 + (t1.fraction as i64 * 1000 / 4294967296);
    let t2_ms = t2.to_unix_secs() * 1000 + (t2.fraction as i64 * 1000 / 4294967296);
    let t3_ms = t3.to_unix_secs() * 1000 + (t3.fraction as i64 * 1000 / 4294967296);
    let t4_ms = t4.to_unix_secs() * 1000 + (t4.fraction as i64 * 1000 / 4294967296);

    let offset_ms = ((t2_ms - t1_ms) + (t3_ms - t4_ms)) / 2;
    let delay_ms = ((t4_ms - t1_ms) - (t3_ms - t2_ms)).unsigned_abs();

    Ok(NtpResult {
        server: server_ip,
        offset_ms,
        delay_ms,
        stratum: reply.stratum,
        reference_id: reply.reference_id_string(),
        leap: reply.leap_indicator(),
    })
}

/// Synchronize time with configured servers
/// Returns the best result (lowest delay)
pub fn sync() -> crate::util::KResult<NtpResult> {
    let servers = {
        let guard = NTP_CLIENT.lock();
        match guard.as_ref() {
            Some(client) => client.servers.clone(),
            None => return Err(crate::util::KError::NotSupported),
        }
    };

    if servers.is_empty() {
        return Err(crate::util::KError::NotSupported);
    }

    let mut best_result: Option<NtpResult> = None;

    for server in &servers {
        match query_server(*server, DEFAULT_TIMEOUT_MS) {
            Ok(result) => {
                // Check if this is better (lower delay)
                let is_better = match &best_result {
                    None => true,
                    Some(prev) => result.delay_ms < prev.delay_ms,
                };

                if is_better {
                    best_result = Some(result);
                }
            }
            Err(_) => {
                // Try next server
                continue;
            }
        }
    }

    let result = best_result.ok_or(crate::util::KError::Timeout)?;

    // Update client state
    {
        let mut guard = NTP_CLIENT.lock();
        if let Some(client) = guard.as_mut() {
            client.last_sync.store(crate::time::uptime_secs(), Ordering::Relaxed);
            client.last_offset_ms.store(result.offset_ms, Ordering::Relaxed);
            client.sync_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    // Apply the offset to system time
    apply_offset(result.offset_ms);

    crate::kprintln!(
        "ntp: synced with {} (stratum {}, offset {}ms, delay {}ms)",
        result.server,
        result.stratum.0,
        result.offset_ms,
        result.delay_ms
    );

    Ok(result)
}

/// Apply a time offset to the system clock
fn apply_offset(offset_ms: i64) {
    // Get current boot time and adjust it
    use core::sync::atomic::Ordering;

    // Access BOOT_TIME_SECS through time module
    // The offset tells us: local_time - server_time = offset
    // So server_time = local_time - offset
    // To correct, we need to adjust BOOT_TIME_SECS by -offset

    // Convert offset from ms to seconds
    let offset_secs = offset_ms / 1000;

    if offset_secs.abs() > MAX_OFFSET_WARN {
        crate::kprintln!(
            "ntp: WARNING: large clock offset detected: {} seconds",
            offset_secs
        );
    }

    // For now, we just log the offset
    // In a full implementation, we would:
    // 1. For small offsets: use adjtime() for gradual adjustment
    // 2. For large offsets: step the clock immediately

    // Update the system time by adjusting the boot time epoch
    // This is done through the time module's set_time function
    let current = crate::time::realtime();
    let corrected_secs = (current.tv_sec - offset_secs) as u64;

    // Set the new time
    set_system_time(corrected_secs);
}

/// Set the system time (Unix timestamp in seconds)
fn set_system_time(unix_secs: u64) {
    // Calculate what BOOT_TIME_SECS should be so that:
    // realtime() = BOOT_TIME_SECS + uptime = unix_secs
    // BOOT_TIME_SECS = unix_secs - uptime_secs

    let uptime = crate::time::uptime_secs();
    let new_boot_time = unix_secs.saturating_sub(uptime);

    // We need to access the BOOT_TIME_SECS in time.rs
    // For now, we store it in our own static
    CORRECTED_BOOT_TIME.store(new_boot_time, Ordering::Relaxed);
    TIME_CORRECTED.store(1, Ordering::Relaxed);
}

/// Corrected boot time (set by NTP)
static CORRECTED_BOOT_TIME: AtomicU64 = AtomicU64::new(0);
static TIME_CORRECTED: AtomicU64 = AtomicU64::new(0);

/// Check if time has been corrected by NTP
pub fn is_time_corrected() -> bool {
    TIME_CORRECTED.load(Ordering::Relaxed) != 0
}

/// Get the time correction that was applied (in seconds)
pub fn get_correction() -> i64 {
    let guard = NTP_CLIENT.lock();
    match guard.as_ref() {
        Some(client) => client.last_offset_ms.load(Ordering::Relaxed) / 1000,
        None => 0,
    }
}

/// Get NTP statistics
pub fn get_stats() -> Option<NtpStats> {
    let guard = NTP_CLIENT.lock();
    guard.as_ref().map(|client| NtpStats {
        servers_configured: client.servers.len(),
        last_sync_uptime: client.last_sync.load(Ordering::Relaxed),
        last_offset_ms: client.last_offset_ms.load(Ordering::Relaxed),
        sync_count: client.sync_count.load(Ordering::Relaxed),
        fail_count: client.fail_count.load(Ordering::Relaxed),
    })
}

/// NTP statistics
#[derive(Debug, Clone)]
pub struct NtpStats {
    pub servers_configured: usize,
    pub last_sync_uptime: u64,
    pub last_offset_ms: i64,
    pub sync_count: u32,
    pub fail_count: u32,
}

/// Format NTP info for display
pub fn format_info() -> String {
    use alloc::format;

    let stats = match get_stats() {
        Some(s) => s,
        None => return String::from("NTP client not initialized\n"),
    };

    let servers = {
        let guard = NTP_CLIENT.lock();
        match guard.as_ref() {
            Some(client) => client.servers.clone(),
            None => Vec::new(),
        }
    };

    let mut info = String::new();
    info.push_str(&format!("NTP Client Status\n"));
    info.push_str(&format!("=================\n"));
    info.push_str(&format!("Servers configured: {}\n", stats.servers_configured));
    info.push_str(&format!("Successful syncs:   {}\n", stats.sync_count));
    info.push_str(&format!("Failed syncs:       {}\n", stats.fail_count));
    info.push_str(&format!("Last sync uptime:   {} seconds\n", stats.last_sync_uptime));
    info.push_str(&format!("Last offset:        {} ms\n", stats.last_offset_ms));
    info.push_str(&format!("Time corrected:     {}\n", if is_time_corrected() { "yes" } else { "no" }));
    info.push_str(&format!("\nConfigured servers:\n"));
    for server in &servers {
        info.push_str(&format!("  {}\n", server));
    }

    info
}

/// Resolve hostname to IP and query that server
pub fn query_hostname(hostname: &str, timeout_ms: u32) -> crate::util::KResult<NtpResult> {
    // Use DNS to resolve hostname
    let ip = super::dns::resolve(hostname)?;
    query_server(ip, timeout_ms)
}

/// Synchronize time using a hostname
pub fn sync_with_hostname(hostname: &str) -> crate::util::KResult<NtpResult> {
    let result = query_hostname(hostname, DEFAULT_TIMEOUT_MS)?;

    // Update client state
    {
        let mut guard = NTP_CLIENT.lock();
        if let Some(client) = guard.as_mut() {
            client.last_sync.store(crate::time::uptime_secs(), Ordering::Relaxed);
            client.last_offset_ms.store(result.offset_ms, Ordering::Relaxed);
            client.sync_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    // Apply the offset to system time
    apply_offset(result.offset_ms);

    crate::kprintln!(
        "ntp: synced with {} ({}) stratum {}, offset {}ms",
        hostname,
        result.server,
        result.stratum.0,
        result.offset_ms
    );

    Ok(result)
}

/// Background NTP synchronization task
/// This should be called periodically (e.g., every 64-1024 seconds)
pub fn periodic_sync() {
    match sync() {
        Ok(_) => {},
        Err(e) => {
            // Record failure
            let mut guard = NTP_CLIENT.lock();
            if let Some(client) = guard.as_mut() {
                client.fail_count.fetch_add(1, Ordering::Relaxed);
            }
            crate::kprintln!("ntp: sync failed: {:?}", e);
        }
    }
}
