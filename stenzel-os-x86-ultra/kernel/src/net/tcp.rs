//! TCP (Transmission Control Protocol)

#![allow(dead_code)]

use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;

use super::ipv4::{self, Ipv4Packet};
use super::Ipv4Addr;
use crate::sync::IrqSafeMutex;

/// Flags TCP
pub const TCP_FIN: u8 = 0x01;
pub const TCP_SYN: u8 = 0x02;
pub const TCP_RST: u8 = 0x04;
pub const TCP_PSH: u8 = 0x08;
pub const TCP_ACK: u8 = 0x10;
pub const TCP_URG: u8 = 0x20;

/// Estados TCP
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

/// Segmento TCP parseado
#[derive(Debug)]
pub struct TcpSegment {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset: u8,
    pub flags: u8,
    pub window: u16,
    pub checksum: u16,
    pub urgent_ptr: u16,
    pub header_len: usize,
}

impl TcpSegment {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 20 {
            return None;
        }

        let data_offset = (data[12] >> 4) as usize;
        let header_len = data_offset * 4;

        if data.len() < header_len {
            return None;
        }

        Some(Self {
            src_port: u16::from_be_bytes([data[0], data[1]]),
            dst_port: u16::from_be_bytes([data[2], data[3]]),
            seq_num: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
            ack_num: u32::from_be_bytes([data[8], data[9], data[10], data[11]]),
            data_offset: data_offset as u8,
            flags: data[13],
            window: u16::from_be_bytes([data[14], data[15]]),
            checksum: u16::from_be_bytes([data[16], data[17]]),
            urgent_ptr: u16::from_be_bytes([data[18], data[19]]),
            header_len,
        })
    }

    pub fn has_syn(&self) -> bool { self.flags & TCP_SYN != 0 }
    pub fn has_ack(&self) -> bool { self.flags & TCP_ACK != 0 }
    pub fn has_fin(&self) -> bool { self.flags & TCP_FIN != 0 }
    pub fn has_rst(&self) -> bool { self.flags & TCP_RST != 0 }
    pub fn has_psh(&self) -> bool { self.flags & TCP_PSH != 0 }
}

/// Chave de conexão TCP
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TcpConnKey {
    pub local_addr: Ipv4Addr,
    pub local_port: u16,
    pub remote_addr: Ipv4Addr,
    pub remote_port: u16,
}

// ============================================================================
// TCP Congestion Control
// ============================================================================

/// Approximate cube root using Newton's method (for no_std)
fn cbrt_approx(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }

    // Initial guess using bit manipulation
    let mut guess = x / 3.0;
    if guess == 0.0 {
        guess = 0.1;
    }

    // Newton's method iterations: guess = (2*guess + x/(guess*guess)) / 3
    for _ in 0..10 {
        guess = (2.0 * guess + x / (guess * guess)) / 3.0;
    }

    guess
}

/// TCP Congestion Control Algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CongestionAlgorithm {
    /// TCP Reno (default) - AIMD with fast retransmit/recovery
    Reno,
    /// TCP NewReno - improved fast recovery
    NewReno,
    /// TCP Cubic - Linux default, better for high-bandwidth
    Cubic,
}

/// Congestion control state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CongestionState {
    /// Initial slow start phase
    SlowStart,
    /// Congestion avoidance phase
    CongestionAvoidance,
    /// Fast recovery phase
    FastRecovery,
}

/// TCP Congestion Control state
#[derive(Debug, Clone)]
pub struct CongestionControl {
    /// Current congestion control algorithm
    pub algorithm: CongestionAlgorithm,
    /// Current state
    pub state: CongestionState,
    /// Congestion window (in bytes)
    pub cwnd: u32,
    /// Slow start threshold
    pub ssthresh: u32,
    /// Maximum segment size
    pub mss: u16,
    /// Receiver's advertised window
    pub rwnd: u32,
    /// Number of duplicate ACKs received
    pub dup_ack_count: u8,
    /// Smoothed RTT (in microseconds)
    pub srtt: u32,
    /// RTT variation
    pub rttvar: u32,
    /// Retransmission timeout (in microseconds)
    pub rto: u32,
    /// Last time a segment was sent (for RTT calculation)
    pub last_sent_time: u64,
    /// Sequence number of timed segment
    pub rtt_seq: u32,
    /// Whether we're measuring RTT for a segment
    pub rtt_measuring: bool,
    /// Bytes acknowledged since cwnd last increased
    pub bytes_acked: u32,
    /// Recovery point for NewReno
    pub recovery_point: u32,
    /// For CUBIC: last congestion time
    pub cubic_last_congestion: u64,
    /// For CUBIC: W_max at last congestion
    pub cubic_w_max: u32,
    /// For CUBIC: K value
    pub cubic_k: f32,
}

impl Default for CongestionControl {
    fn default() -> Self {
        Self::new()
    }
}

impl CongestionControl {
    /// Initial congestion window (RFC 6928: 10*MSS)
    const INITIAL_CWND_SEGMENTS: u32 = 10;
    /// Initial ssthresh (large value)
    const INITIAL_SSTHRESH: u32 = 65535;
    /// Default MSS
    const DEFAULT_MSS: u16 = 1460;
    /// Minimum RTO (1 second)
    const MIN_RTO: u32 = 1_000_000;
    /// Maximum RTO (60 seconds)
    const MAX_RTO: u32 = 60_000_000;
    /// Initial RTO (3 seconds, RFC 6298)
    const INITIAL_RTO: u32 = 3_000_000;
    /// CUBIC beta
    const CUBIC_BETA: f32 = 0.7;
    /// CUBIC C constant
    const CUBIC_C: f32 = 0.4;

    pub fn new() -> Self {
        let mss = Self::DEFAULT_MSS;
        Self {
            algorithm: CongestionAlgorithm::Reno,
            state: CongestionState::SlowStart,
            cwnd: Self::INITIAL_CWND_SEGMENTS * mss as u32,
            ssthresh: Self::INITIAL_SSTHRESH,
            mss,
            rwnd: 65535,
            dup_ack_count: 0,
            srtt: 0,
            rttvar: 0,
            rto: Self::INITIAL_RTO,
            last_sent_time: 0,
            rtt_seq: 0,
            rtt_measuring: false,
            bytes_acked: 0,
            recovery_point: 0,
            cubic_last_congestion: 0,
            cubic_w_max: 0,
            cubic_k: 0.0,
        }
    }

    /// Set the congestion control algorithm
    pub fn set_algorithm(&mut self, algo: CongestionAlgorithm) {
        self.algorithm = algo;
    }

    /// Get the effective window size (min of cwnd and rwnd)
    pub fn effective_window(&self) -> u32 {
        self.cwnd.min(self.rwnd)
    }

    /// Get flight size (bytes in flight)
    pub fn flight_size(&self, snd_una: u32, snd_nxt: u32) -> u32 {
        snd_nxt.wrapping_sub(snd_una)
    }

    /// Check if we can send more data
    pub fn can_send(&self, snd_una: u32, snd_nxt: u32) -> bool {
        self.flight_size(snd_una, snd_nxt) < self.effective_window()
    }

    /// Update RTT estimate (RFC 6298)
    pub fn update_rtt(&mut self, measured_rtt: u32) {
        if self.srtt == 0 {
            // First measurement
            self.srtt = measured_rtt;
            self.rttvar = measured_rtt / 2;
        } else {
            // Subsequent measurements
            let alpha = 8; // 1/8 as shift
            let beta = 4;  // 1/4 as shift

            let diff = if measured_rtt > self.srtt {
                measured_rtt - self.srtt
            } else {
                self.srtt - measured_rtt
            };

            self.rttvar = (3 * self.rttvar + diff) / 4;
            self.srtt = (7 * self.srtt + measured_rtt) / 8;
        }

        // Calculate RTO
        self.rto = self.srtt + 4 * self.rttvar;
        self.rto = self.rto.max(Self::MIN_RTO).min(Self::MAX_RTO);
    }

    /// Handle new ACK received
    pub fn on_ack(&mut self, bytes_acked: u32, current_time: u64) {
        self.dup_ack_count = 0;

        match self.state {
            CongestionState::SlowStart => {
                // Increase cwnd exponentially
                self.cwnd += bytes_acked.min(self.mss as u32);

                // Check if we should transition to congestion avoidance
                if self.cwnd >= self.ssthresh {
                    self.state = CongestionState::CongestionAvoidance;
                }
            }
            CongestionState::CongestionAvoidance => {
                match self.algorithm {
                    CongestionAlgorithm::Reno | CongestionAlgorithm::NewReno => {
                        // AIMD: increase cwnd by ~MSS per RTT
                        self.bytes_acked += bytes_acked;
                        if self.bytes_acked >= self.cwnd {
                            self.cwnd += self.mss as u32;
                            self.bytes_acked = 0;
                        }
                    }
                    CongestionAlgorithm::Cubic => {
                        self.cubic_update(current_time);
                    }
                }
            }
            CongestionState::FastRecovery => {
                match self.algorithm {
                    CongestionAlgorithm::Reno => {
                        // Exit fast recovery
                        self.cwnd = self.ssthresh;
                        self.state = CongestionState::CongestionAvoidance;
                    }
                    CongestionAlgorithm::NewReno => {
                        // Partial ACK handling
                        // If ACK advances past recovery_point, exit
                        // Otherwise, retransmit and stay in recovery
                        self.cwnd = self.ssthresh;
                        self.state = CongestionState::CongestionAvoidance;
                    }
                    CongestionAlgorithm::Cubic => {
                        self.cwnd = self.ssthresh;
                        self.state = CongestionState::CongestionAvoidance;
                    }
                }
            }
        }
    }

    /// Handle duplicate ACK
    pub fn on_dup_ack(&mut self) {
        self.dup_ack_count += 1;

        if self.dup_ack_count == 3 {
            // Triple duplicate ACK - enter fast recovery
            self.enter_fast_recovery();
        } else if self.state == CongestionState::FastRecovery {
            // Inflate cwnd for each additional dup ack
            self.cwnd += self.mss as u32;
        }
    }

    /// Enter fast recovery
    fn enter_fast_recovery(&mut self) {
        self.ssthresh = (self.cwnd / 2).max(2 * self.mss as u32);
        self.cwnd = self.ssthresh + 3 * self.mss as u32;
        self.state = CongestionState::FastRecovery;

        if self.algorithm == CongestionAlgorithm::Cubic {
            self.cubic_w_max = self.cwnd;
            self.cubic_last_congestion = crate::time::ticks();
            // Calculate K = cbrt(W_max * (1-beta) / C)
            let w_max_f = self.cubic_w_max as f32 / self.mss as f32;
            let val = (w_max_f * (1.0 - Self::CUBIC_BETA)) / Self::CUBIC_C;
            self.cubic_k = cbrt_approx(val);
        }
    }

    /// Handle timeout
    pub fn on_timeout(&mut self) {
        self.ssthresh = (self.cwnd / 2).max(2 * self.mss as u32);
        self.cwnd = self.mss as u32; // Reset to 1 MSS
        self.state = CongestionState::SlowStart;
        self.dup_ack_count = 0;

        // Back off RTO
        self.rto = (self.rto * 2).min(Self::MAX_RTO);
    }

    /// CUBIC window update
    fn cubic_update(&mut self, current_time: u64) {
        let elapsed = current_time.saturating_sub(self.cubic_last_congestion) as f32 / 1000.0; // seconds

        // W_cubic(t) = C * (t - K)^3 + W_max
        let t_minus_k = elapsed - self.cubic_k;
        let w_cubic = Self::CUBIC_C * t_minus_k * t_minus_k * t_minus_k
            + (self.cubic_w_max as f32 / self.mss as f32);

        // TCP-friendly window
        let w_tcp = (self.cubic_w_max as f32 / self.mss as f32) * Self::CUBIC_BETA
            + 3.0 * (1.0 - Self::CUBIC_BETA) / (1.0 + Self::CUBIC_BETA) * elapsed;

        // Use the larger of CUBIC and TCP-friendly
        let target_cwnd = w_cubic.max(w_tcp) * self.mss as f32;

        // Slowly adjust cwnd towards target
        if target_cwnd > self.cwnd as f32 {
            let increase = ((target_cwnd - self.cwnd as f32) / self.cwnd as f32 * self.mss as f32)
                .max(1.0);
            self.cwnd += increase as u32;
        }
    }

    /// Get current statistics for debugging/monitoring
    pub fn stats(&self) -> CongestionStats {
        CongestionStats {
            cwnd: self.cwnd,
            ssthresh: self.ssthresh,
            srtt: self.srtt,
            rto: self.rto,
            state: self.state,
            algorithm: self.algorithm,
        }
    }
}

/// Congestion control statistics
#[derive(Debug, Clone, Copy)]
pub struct CongestionStats {
    pub cwnd: u32,
    pub ssthresh: u32,
    pub srtt: u32,
    pub rto: u32,
    pub state: CongestionState,
    pub algorithm: CongestionAlgorithm,
}

/// Conexão TCP
pub struct TcpConnection {
    pub key: TcpConnKey,
    pub state: TcpState,
    // Sequence numbers
    pub snd_una: u32,    // Oldest unacked seq
    pub snd_nxt: u32,    // Next seq to send
    pub rcv_nxt: u32,    // Next expected seq
    pub rcv_wnd: u16,    // Receive window
    // Buffers
    pub recv_buffer: VecDeque<u8>,
    pub send_buffer: VecDeque<u8>,
    // ISN inicial
    pub iss: u32,
    pub irs: u32,
    // Congestion control
    pub congestion: CongestionControl,
}

impl TcpConnection {
    fn new(key: TcpConnKey) -> Self {
        let iss = generate_isn();
        Self {
            key,
            state: TcpState::Closed,
            snd_una: iss,
            snd_nxt: iss,
            rcv_nxt: 0,
            rcv_wnd: 8192,
            recv_buffer: VecDeque::new(),
            send_buffer: VecDeque::new(),
            iss,
            irs: 0,
            congestion: CongestionControl::new(),
        }
    }

    /// Check if we can send more data (considering congestion window)
    pub fn can_send(&self) -> bool {
        self.congestion.can_send(self.snd_una, self.snd_nxt)
    }

    /// Get max bytes we can send
    pub fn sendable_bytes(&self) -> u32 {
        let flight = self.congestion.flight_size(self.snd_una, self.snd_nxt);
        let window = self.congestion.effective_window();
        window.saturating_sub(flight)
    }
}

/// Conexões TCP ativas
static TCP_CONNECTIONS: IrqSafeMutex<BTreeMap<TcpConnKey, TcpConnection>> = IrqSafeMutex::new(BTreeMap::new());

/// Listening sockets
static TCP_LISTENERS: IrqSafeMutex<BTreeMap<u16, VecDeque<TcpConnKey>>> = IrqSafeMutex::new(BTreeMap::new());

/// Próxima porta efêmera TCP (atômica para thread safety)
static NEXT_EPHEMERAL_PORT: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(49152);

/// Lê TSC (Time Stamp Counter) para entropia
#[inline]
fn read_tsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!(
            "rdtsc",
            out("eax") lo,
            out("edx") hi,
            options(nomem, nostack)
        );
    }
    ((hi as u64) << 32) | (lo as u64)
}

/// Simple hash mix function (FNV-1a inspired)
#[inline]
fn hash_mix(mut h: u64, data: u64) -> u64 {
    h ^= data;
    h = h.wrapping_mul(0x100000001b3);
    h
}

/// Gera ISN (Initial Sequence Number) com entropia
/// Usa TSC + ticks + contador para evitar ISN previsível
fn generate_isn() -> u32 {
    use core::sync::atomic::{AtomicU32, Ordering};
    static ISN_COUNTER: AtomicU32 = AtomicU32::new(0x12345678);

    // Componentes de entropia
    let tsc = read_tsc();
    let ticks = crate::time::ticks();
    let counter = ISN_COUNTER.fetch_add(64000, Ordering::Relaxed);

    // Mix entropy sources
    let mut h: u64 = 0xcbf29ce484222325; // FNV offset basis
    h = hash_mix(h, tsc);
    h = hash_mix(h, ticks);
    h = hash_mix(h, counter as u64);

    // Combina high e low bits para 32-bit result
    ((h >> 32) ^ h) as u32
}

/// Calcula checksum TCP
fn compute_checksum(src_ip: Ipv4Addr, dst_ip: Ipv4Addr, tcp_data: &[u8]) -> u16 {
    let mut sum: u32 = 0;

    // Pseudo-header
    let src = src_ip.0;
    let dst = dst_ip.0;
    sum += u16::from_be_bytes([src[0], src[1]]) as u32;
    sum += u16::from_be_bytes([src[2], src[3]]) as u32;
    sum += u16::from_be_bytes([dst[0], dst[1]]) as u32;
    sum += u16::from_be_bytes([dst[2], dst[3]]) as u32;
    sum += 6u32; // Protocol TCP
    sum += tcp_data.len() as u32;

    // TCP header + data
    for i in (0..tcp_data.len()).step_by(2) {
        let word = if i + 1 < tcp_data.len() {
            u16::from_be_bytes([tcp_data[i], tcp_data[i + 1]])
        } else {
            u16::from_be_bytes([tcp_data[i], 0])
        };
        sum += word as u32;
    }

    while sum > 0xFFFF {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !(sum as u16)
}

/// Processa um pacote TCP recebido
pub fn handle_packet(data: &[u8], ip: &Ipv4Packet) {
    let Some(tcp) = TcpSegment::parse(data) else {
        return;
    };

    // Verifica checksum
    let stored = tcp.checksum;
    let mut check_data = data.to_vec();
    check_data[16] = 0;
    check_data[17] = 0;
    if compute_checksum(ip.src, ip.dst, &check_data) != stored {
        return;
    }

    let payload = &data[tcp.header_len..];
    let config = match super::config() {
        Some(c) => c,
        None => return,
    };

    let key = TcpConnKey {
        local_addr: ip.dst,
        local_port: tcp.dst_port,
        remote_addr: ip.src,
        remote_port: tcp.src_port,
    };

    let mut conns = TCP_CONNECTIONS.lock();

    if let Some(conn) = conns.get_mut(&key) {
        // Conexão existente
        process_segment(conn, &tcp, payload, &config);
    } else {
        // Verifica se há listener
        drop(conns);
        let mut listeners = TCP_LISTENERS.lock();
        if let Some(accept_queue) = listeners.get_mut(&tcp.dst_port) {
            if tcp.has_syn() && !tcp.has_ack() {
                // Novo SYN - cria conexão
                let mut conn = TcpConnection::new(key);
                conn.state = TcpState::SynReceived;
                conn.irs = tcp.seq_num;
                conn.rcv_nxt = tcp.seq_num.wrapping_add(1);

                // Envia SYN-ACK
                send_segment(
                    &conn,
                    TCP_SYN | TCP_ACK,
                    conn.iss,
                    conn.rcv_nxt,
                    &[],
                    Ipv4Addr(config.ip.0),
                );

                conn.snd_nxt = conn.iss.wrapping_add(1);

                // Adiciona à fila de accept
                if accept_queue.len() < 128 {
                    accept_queue.push_back(key);
                }

                drop(listeners);
                let mut conns = TCP_CONNECTIONS.lock();
                conns.insert(key, conn);
            }
        } else if tcp.has_syn() {
            // RST para conexão não solicitada
            send_rst(ip.src, tcp.src_port, ip.dst, tcp.dst_port, tcp.seq_num.wrapping_add(1));
        }
    }
}

/// Processa segmento em conexão existente
fn process_segment(conn: &mut TcpConnection, tcp: &TcpSegment, payload: &[u8], config: &super::NetConfig) {
    if tcp.has_rst() {
        conn.state = TcpState::Closed;
        return;
    }

    match conn.state {
        TcpState::SynReceived => {
            if tcp.has_ack() && tcp.ack_num == conn.snd_nxt {
                conn.state = TcpState::Established;
                conn.snd_una = tcp.ack_num;
            }
        }
        TcpState::SynSent => {
            if tcp.has_syn() && tcp.has_ack() {
                conn.irs = tcp.seq_num;
                conn.rcv_nxt = tcp.seq_num.wrapping_add(1);
                conn.snd_una = tcp.ack_num;

                // Envia ACK
                send_segment(conn, TCP_ACK, conn.snd_nxt, conn.rcv_nxt, &[], Ipv4Addr(config.ip.0));
                conn.state = TcpState::Established;
            }
        }
        TcpState::Established => {
            // Processa dados
            if tcp.seq_num == conn.rcv_nxt && !payload.is_empty() {
                for byte in payload {
                    if conn.recv_buffer.len() < 65536 {
                        conn.recv_buffer.push_back(*byte);
                    }
                }
                conn.rcv_nxt = conn.rcv_nxt.wrapping_add(payload.len() as u32);

                // Envia ACK
                send_segment(conn, TCP_ACK, conn.snd_nxt, conn.rcv_nxt, &[], Ipv4Addr(config.ip.0));
            }

            // Atualiza ACK
            if tcp.has_ack() {
                conn.snd_una = tcp.ack_num;
            }

            // FIN recebido
            if tcp.has_fin() {
                conn.rcv_nxt = conn.rcv_nxt.wrapping_add(1);
                send_segment(conn, TCP_ACK, conn.snd_nxt, conn.rcv_nxt, &[], Ipv4Addr(config.ip.0));
                conn.state = TcpState::CloseWait;
            }
        }
        TcpState::FinWait1 => {
            if tcp.has_ack() {
                conn.state = TcpState::FinWait2;
            }
            if tcp.has_fin() {
                conn.rcv_nxt = conn.rcv_nxt.wrapping_add(1);
                send_segment(conn, TCP_ACK, conn.snd_nxt, conn.rcv_nxt, &[], Ipv4Addr(config.ip.0));
                conn.state = TcpState::TimeWait;
            }
        }
        TcpState::FinWait2 => {
            if tcp.has_fin() {
                conn.rcv_nxt = conn.rcv_nxt.wrapping_add(1);
                send_segment(conn, TCP_ACK, conn.snd_nxt, conn.rcv_nxt, &[], Ipv4Addr(config.ip.0));
                conn.state = TcpState::TimeWait;
            }
        }
        TcpState::LastAck => {
            if tcp.has_ack() {
                conn.state = TcpState::Closed;
            }
        }
        TcpState::CloseWait | TcpState::Closing | TcpState::TimeWait => {
            // Handled elsewhere
        }
        _ => {}
    }
}

/// Envia um segmento TCP
fn send_segment(conn: &TcpConnection, flags: u8, seq: u32, ack: u32, payload: &[u8], src_ip: Ipv4Addr) {
    let header_len = 20;
    let total_len = header_len + payload.len();
    let mut packet = Vec::with_capacity(total_len);

    // Source port
    packet.extend_from_slice(&conn.key.local_port.to_be_bytes());
    // Dest port
    packet.extend_from_slice(&conn.key.remote_port.to_be_bytes());
    // Sequence number
    packet.extend_from_slice(&seq.to_be_bytes());
    // ACK number
    packet.extend_from_slice(&ack.to_be_bytes());
    // Data offset (5 words = 20 bytes) + reserved
    packet.push((5 << 4) as u8);
    // Flags
    packet.push(flags);
    // Window
    packet.extend_from_slice(&conn.rcv_wnd.to_be_bytes());
    // Checksum placeholder
    packet.push(0);
    packet.push(0);
    // Urgent pointer
    packet.push(0);
    packet.push(0);
    // Payload
    packet.extend_from_slice(payload);

    // Compute checksum
    let checksum = compute_checksum(src_ip, conn.key.remote_addr, &packet);
    packet[16] = (checksum >> 8) as u8;
    packet[17] = (checksum & 0xFF) as u8;

    let _ = ipv4::send(conn.key.remote_addr, ipv4::PROTO_TCP, &packet);
}

/// Envia RST
fn send_rst(dst_addr: Ipv4Addr, dst_port: u16, src_addr: Ipv4Addr, src_port: u16, ack: u32) {
    let mut packet = Vec::with_capacity(20);

    packet.extend_from_slice(&src_port.to_be_bytes());
    packet.extend_from_slice(&dst_port.to_be_bytes());
    packet.extend_from_slice(&0u32.to_be_bytes()); // seq
    packet.extend_from_slice(&ack.to_be_bytes());
    packet.push(0x50); // data offset
    packet.push(TCP_RST | TCP_ACK);
    packet.extend_from_slice(&0u16.to_be_bytes()); // window
    packet.push(0);
    packet.push(0);
    packet.push(0);
    packet.push(0);

    let checksum = compute_checksum(src_addr, dst_addr, &packet);
    packet[16] = (checksum >> 8) as u8;
    packet[17] = (checksum & 0xFF) as u8;

    let _ = ipv4::send(dst_addr, ipv4::PROTO_TCP, &packet);
}

// === API pública ===

/// Listen em uma porta
pub fn listen(port: u16) -> crate::util::KResult<()> {
    let mut listeners = TCP_LISTENERS.lock();
    if listeners.contains_key(&port) {
        return Err(crate::util::KError::AlreadyExists);
    }
    listeners.insert(port, VecDeque::new());
    Ok(())
}

/// Accept uma conexão
pub fn accept(port: u16) -> Option<TcpConnKey> {
    let mut listeners = TCP_LISTENERS.lock();
    if let Some(queue) = listeners.get_mut(&port) {
        // Procura conexão estabelecida
        let conns = TCP_CONNECTIONS.lock();
        for i in 0..queue.len() {
            if let Some(key) = queue.get(i) {
                if let Some(conn) = conns.get(key) {
                    if conn.state == TcpState::Established {
                        return queue.remove(i);
                    }
                }
            }
        }
    }
    None
}

/// Connect a um servidor remoto
pub fn connect(remote_addr: Ipv4Addr, remote_port: u16) -> crate::util::KResult<TcpConnKey> {
    let config = super::config().ok_or(crate::util::KError::NotSupported)?;

    let local_port = loop {
        let p = NEXT_EPHEMERAL_PORT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        let port = if p >= 65534 {
            NEXT_EPHEMERAL_PORT.store(49152, core::sync::atomic::Ordering::Relaxed);
            49152
        } else {
            p
        };
        // Verifica se porta não está em uso
        let conns = TCP_CONNECTIONS.lock();
        let in_use = conns.keys().any(|k| k.local_port == port);
        drop(conns);
        if !in_use {
            break port;
        }
    };

    let key = TcpConnKey {
        local_addr: config.ip,
        local_port,
        remote_addr,
        remote_port,
    };

    let mut conn = TcpConnection::new(key);
    conn.state = TcpState::SynSent;

    // Envia SYN
    send_segment(&conn, TCP_SYN, conn.iss, 0, &[], config.ip);
    conn.snd_nxt = conn.iss.wrapping_add(1);

    let mut conns = TCP_CONNECTIONS.lock();
    conns.insert(key, conn);
    drop(conns);

    // Espera conexão estabelecer
    for _ in 0..5000 {
        super::poll();

        let conns = TCP_CONNECTIONS.lock();
        if let Some(c) = conns.get(&key) {
            if c.state == TcpState::Established {
                return Ok(key);
            }
            if c.state == TcpState::Closed {
                drop(conns);
                let mut conns = TCP_CONNECTIONS.lock();
                conns.remove(&key);
                return Err(crate::util::KError::NotSupported);
            }
        }
        drop(conns);

        for _ in 0..10000 {
            core::hint::spin_loop();
        }
    }

    // Timeout
    let mut conns = TCP_CONNECTIONS.lock();
    conns.remove(&key);
    Err(crate::util::KError::NotSupported)
}

/// Envia dados em uma conexão
pub fn send(key: &TcpConnKey, data: &[u8]) -> crate::util::KResult<usize> {
    let config = super::config().ok_or(crate::util::KError::NotSupported)?;
    let mut conns = TCP_CONNECTIONS.lock();

    let conn = conns.get_mut(key).ok_or(crate::util::KError::Invalid)?;

    if conn.state != TcpState::Established {
        return Err(crate::util::KError::NotSupported);
    }

    // Envia em chunks de MSS (simplificado para 1460)
    let mss = 1460;
    let mut sent = 0;

    for chunk in data.chunks(mss) {
        send_segment(conn, TCP_PSH | TCP_ACK, conn.snd_nxt, conn.rcv_nxt, chunk, config.ip);
        conn.snd_nxt = conn.snd_nxt.wrapping_add(chunk.len() as u32);
        sent += chunk.len();
    }

    Ok(sent)
}

/// Recebe dados de uma conexão
pub fn recv(key: &TcpConnKey, buf: &mut [u8]) -> crate::util::KResult<usize> {
    let mut conns = TCP_CONNECTIONS.lock();
    let conn = conns.get_mut(key).ok_or(crate::util::KError::Invalid)?;

    let mut read = 0;
    while read < buf.len() {
        if let Some(byte) = conn.recv_buffer.pop_front() {
            buf[read] = byte;
            read += 1;
        } else {
            break;
        }
    }

    Ok(read)
}

/// Fecha uma conexão
pub fn close(key: &TcpConnKey) -> crate::util::KResult<()> {
    let config = super::config().ok_or(crate::util::KError::NotSupported)?;
    let mut conns = TCP_CONNECTIONS.lock();

    let conn = conns.get_mut(key).ok_or(crate::util::KError::Invalid)?;

    match conn.state {
        TcpState::Established => {
            send_segment(conn, TCP_FIN | TCP_ACK, conn.snd_nxt, conn.rcv_nxt, &[], config.ip);
            conn.snd_nxt = conn.snd_nxt.wrapping_add(1);
            conn.state = TcpState::FinWait1;
        }
        TcpState::CloseWait => {
            send_segment(conn, TCP_FIN | TCP_ACK, conn.snd_nxt, conn.rcv_nxt, &[], config.ip);
            conn.snd_nxt = conn.snd_nxt.wrapping_add(1);
            conn.state = TcpState::LastAck;
        }
        _ => {}
    }

    Ok(())
}

/// Remove conexões fechadas
pub fn cleanup() {
    let mut conns = TCP_CONNECTIONS.lock();
    conns.retain(|_, c| c.state != TcpState::Closed && c.state != TcpState::TimeWait);
}

/// Obtém o estado de uma conexão
pub fn get_state(key: &TcpConnKey) -> Option<TcpState> {
    let conns = TCP_CONNECTIONS.lock();
    conns.get(key).map(|c| c.state)
}

/// Verifica se há dados disponíveis para leitura (para poll/select)
pub fn has_data(key: &TcpConnKey) -> bool {
    let conns = TCP_CONNECTIONS.lock();
    if let Some(conn) = conns.get(key) {
        !conn.recv_buffer.is_empty()
    } else {
        false
    }
}

/// Returns the number of bytes available to read (for FIONREAD ioctl)
pub fn available_data(key: &TcpConnKey) -> usize {
    let conns = TCP_CONNECTIONS.lock();
    if let Some(conn) = conns.get(key) {
        conn.recv_buffer.len()
    } else {
        0
    }
}

/// Verifica se há conexões pendentes em uma porta de listening (para poll/select)
pub fn has_pending_connection(port: u16) -> bool {
    let listeners = TCP_LISTENERS.lock();
    if let Some(queue) = listeners.get(&port) {
        let conns = TCP_CONNECTIONS.lock();
        for key in queue.iter() {
            if let Some(conn) = conns.get(key) {
                if conn.state == TcpState::Established {
                    return true;
                }
            }
        }
    }
    false
}

/// Verifica se podemos enviar dados (para poll/select)
pub fn can_send(key: &TcpConnKey) -> bool {
    let conns = TCP_CONNECTIONS.lock();
    if let Some(conn) = conns.get(key) {
        conn.state == TcpState::Established
    } else {
        false
    }
}
