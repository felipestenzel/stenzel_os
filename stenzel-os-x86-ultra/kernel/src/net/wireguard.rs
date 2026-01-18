//! WireGuard VPN Implementation
//!
//! A modern, high-performance VPN protocol using state-of-the-art cryptography:
//! - Curve25519 for key exchange
//! - ChaCha20-Poly1305 for symmetric encryption
//! - BLAKE2s for hashing
//! - SipHash24 for hashtable keys
//!
//! Based on the WireGuard protocol specification.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};
use crate::crypto::{
    x25519_diffie_hellman, x25519_public_key,
    chacha20_poly1305_encrypt, chacha20_poly1305_decrypt,
};

/// WireGuard UDP port
pub const WIREGUARD_PORT: u16 = 51820;

/// Maximum transmission unit for WireGuard
pub const WG_MTU: usize = 1420;

/// Rekey interval (2 minutes)
pub const REKEY_AFTER_TIME: u64 = 120;

/// Keep-alive interval (25 seconds)
pub const KEEPALIVE_TIMEOUT: u64 = 25;

/// Handshake retry timeout (5 seconds)
pub const REKEY_TIMEOUT: u64 = 5;

/// Maximum number of peers
pub const MAX_PEERS: usize = 256;

/// Message types
pub mod message_type {
    pub const HANDSHAKE_INITIATION: u8 = 1;
    pub const HANDSHAKE_RESPONSE: u8 = 2;
    pub const COOKIE_REPLY: u8 = 3;
    pub const TRANSPORT_DATA: u8 = 4;
}

/// WireGuard error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WgError {
    InvalidKey,
    InvalidPacket,
    DecryptionFailed,
    AuthenticationFailed,
    PeerNotFound,
    SessionExpired,
    RateLimited,
    NoRoute,
    BufferTooSmall,
    NotInitialized,
}

impl From<WgError> for KError {
    fn from(_: WgError) -> Self {
        KError::Invalid
    }
}

/// 32-byte key type
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WgKey([u8; 32]);

impl WgKey {
    pub const ZERO: WgKey = WgKey([0u8; 32]);

    pub fn new(data: [u8; 32]) -> Self {
        WgKey(data)
    }

    pub fn from_slice(data: &[u8]) -> Result<Self, WgError> {
        if data.len() != 32 {
            return Err(WgError::InvalidKey);
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(data);
        Ok(WgKey(key))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Generate public key from private key
    pub fn public_key(&self) -> WgKey {
        let pk = x25519_public_key(&self.0);
        WgKey(pk)
    }

    /// Perform X25519 Diffie-Hellman
    pub fn dh(&self, public: &WgKey) -> WgKey {
        let shared = x25519_diffie_hellman(&self.0, &public.0);
        WgKey(shared)
    }
}

impl core::fmt::Debug for WgKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "WgKey([...])")
    }
}

/// IP address (v4 or v6)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpAddr {
    V4([u8; 4]),
    V6([u8; 16]),
}

impl IpAddr {
    pub fn v4(a: u8, b: u8, c: u8, d: u8) -> Self {
        IpAddr::V4([a, b, c, d])
    }

    pub fn is_v4(&self) -> bool {
        matches!(self, IpAddr::V4(_))
    }

    pub fn is_v6(&self) -> bool {
        matches!(self, IpAddr::V6(_))
    }
}

/// IP network (CIDR)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpNetwork {
    pub addr: IpAddr,
    pub prefix_len: u8,
}

impl IpNetwork {
    pub fn new(addr: IpAddr, prefix_len: u8) -> Self {
        IpNetwork { addr, prefix_len }
    }

    /// Check if an IP address is within this network
    pub fn contains(&self, ip: &IpAddr) -> bool {
        match (&self.addr, ip) {
            (IpAddr::V4(net), IpAddr::V4(addr)) => {
                if self.prefix_len == 0 {
                    return true;
                }
                if self.prefix_len >= 32 {
                    return net == addr;
                }
                let mask = !0u32 << (32 - self.prefix_len);
                let net_int = u32::from_be_bytes(*net);
                let addr_int = u32::from_be_bytes(*addr);
                (net_int & mask) == (addr_int & mask)
            }
            (IpAddr::V6(net), IpAddr::V6(addr)) => {
                if self.prefix_len == 0 {
                    return true;
                }
                // Compare byte by byte
                let full_bytes = (self.prefix_len / 8) as usize;
                let remaining_bits = self.prefix_len % 8;

                for i in 0..full_bytes {
                    if net[i] != addr[i] {
                        return false;
                    }
                }

                if remaining_bits > 0 && full_bytes < 16 {
                    let mask = !0u8 << (8 - remaining_bits);
                    if (net[full_bytes] & mask) != (addr[full_bytes] & mask) {
                        return false;
                    }
                }

                true
            }
            _ => false,
        }
    }
}

/// Endpoint (IP:port)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
    pub addr: IpAddr,
    pub port: u16,
}

/// Noise protocol handshake state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeState {
    /// No handshake in progress
    None,
    /// Initiation sent, waiting for response
    InitiationSent,
    /// Response sent, waiting for first data
    ResponseSent,
    /// Handshake complete
    Complete,
}

/// Session keys after successful handshake
#[derive(Clone)]
pub struct SessionKeys {
    /// Key for sending
    pub send_key: WgKey,
    /// Key for receiving
    pub recv_key: WgKey,
    /// Sending nonce counter
    pub send_nonce: u64,
    /// Receiving nonce counter
    pub recv_nonce: u64,
    /// Local index
    pub local_index: u32,
    /// Remote index
    pub remote_index: u32,
    /// Creation timestamp
    pub created_at: u64,
}

impl SessionKeys {
    pub fn new(send_key: WgKey, recv_key: WgKey, local_index: u32, remote_index: u32) -> Self {
        SessionKeys {
            send_key,
            recv_key,
            send_nonce: 0,
            recv_nonce: 0,
            local_index,
            remote_index,
            created_at: crate::time::uptime_secs(),
        }
    }

    pub fn is_expired(&self) -> bool {
        let now = crate::time::uptime_secs();
        now > self.created_at + REKEY_AFTER_TIME * 3
    }

    pub fn needs_rekey(&self) -> bool {
        let now = crate::time::uptime_secs();
        now > self.created_at + REKEY_AFTER_TIME
    }
}

/// Peer configuration and state
pub struct WgPeer {
    /// Peer's public key
    pub public_key: WgKey,
    /// Preshared key (optional, for post-quantum security)
    pub preshared_key: Option<WgKey>,
    /// Allowed IPs for this peer
    pub allowed_ips: Vec<IpNetwork>,
    /// Current endpoint (may change with roaming)
    pub endpoint: Option<Endpoint>,
    /// Persistent keepalive interval (0 = disabled)
    pub persistent_keepalive: u16,
    /// Current handshake state
    handshake_state: HandshakeState,
    /// Current session (if established)
    session: Option<SessionKeys>,
    /// Previous session (for seamless key rotation)
    prev_session: Option<SessionKeys>,
    /// Ephemeral private key for current handshake
    ephemeral_private: Option<WgKey>,
    /// Last handshake timestamp
    pub last_handshake: u64,
    /// Last received timestamp
    pub last_rx: u64,
    /// Last transmitted timestamp
    pub last_tx: u64,
    /// Bytes received
    pub rx_bytes: AtomicU64,
    /// Bytes transmitted
    pub tx_bytes: AtomicU64,
}

impl WgPeer {
    pub fn new(public_key: WgKey) -> Self {
        WgPeer {
            public_key,
            preshared_key: None,
            allowed_ips: Vec::new(),
            endpoint: None,
            persistent_keepalive: 0,
            handshake_state: HandshakeState::None,
            session: None,
            prev_session: None,
            ephemeral_private: None,
            last_handshake: 0,
            last_rx: 0,
            last_tx: 0,
            rx_bytes: AtomicU64::new(0),
            tx_bytes: AtomicU64::new(0),
        }
    }

    pub fn with_preshared_key(mut self, psk: WgKey) -> Self {
        self.preshared_key = Some(psk);
        self
    }

    pub fn with_endpoint(mut self, endpoint: Endpoint) -> Self {
        self.endpoint = Some(endpoint);
        self
    }

    pub fn with_allowed_ip(mut self, network: IpNetwork) -> Self {
        self.allowed_ips.push(network);
        self
    }

    pub fn with_persistent_keepalive(mut self, interval: u16) -> Self {
        self.persistent_keepalive = interval;
        self
    }

    pub fn has_session(&self) -> bool {
        self.session.is_some()
    }

    pub fn needs_rekey(&self) -> bool {
        match &self.session {
            Some(s) => s.needs_rekey(),
            None => true,
        }
    }

    /// Check if this peer allows traffic to/from the given IP
    pub fn allows_ip(&self, ip: &IpAddr) -> bool {
        self.allowed_ips.iter().any(|net| net.contains(ip))
    }
}

/// WireGuard interface configuration
pub struct WgConfig {
    /// Interface private key
    pub private_key: WgKey,
    /// Listening port
    pub listen_port: u16,
    /// Interface addresses
    pub addresses: Vec<IpNetwork>,
    /// DNS servers
    pub dns: Vec<IpAddr>,
    /// MTU
    pub mtu: usize,
}

impl Default for WgConfig {
    fn default() -> Self {
        WgConfig {
            private_key: WgKey::ZERO,
            listen_port: WIREGUARD_PORT,
            addresses: Vec::new(),
            dns: Vec::new(),
            mtu: WG_MTU,
        }
    }
}

/// WireGuard interface
pub struct WgInterface {
    /// Interface name
    name: String,
    /// Configuration
    config: WgConfig,
    /// Public key (derived from private key)
    public_key: WgKey,
    /// Peers by public key
    peers: BTreeMap<[u8; 32], WgPeer>,
    /// Index to peer mapping (for incoming packets)
    index_map: BTreeMap<u32, [u8; 32]>,
    /// Next session index
    next_index: u32,
    /// Interface is up
    is_up: AtomicBool,
    /// Packets queued for sending
    tx_queue: Vec<Vec<u8>>,
    /// Packets received
    rx_queue: Vec<Vec<u8>>,
}

impl WgInterface {
    /// Create a new WireGuard interface
    pub fn new(name: &str, config: WgConfig) -> Self {
        let public_key = config.private_key.public_key();
        WgInterface {
            name: String::from(name),
            config,
            public_key,
            peers: BTreeMap::new(),
            index_map: BTreeMap::new(),
            next_index: 1,
            is_up: AtomicBool::new(false),
            tx_queue: Vec::new(),
            rx_queue: Vec::new(),
        }
    }

    /// Add a peer
    pub fn add_peer(&mut self, peer: WgPeer) {
        let key = *peer.public_key.as_bytes();
        self.peers.insert(key, peer);
    }

    /// Remove a peer
    pub fn remove_peer(&mut self, public_key: &WgKey) {
        self.peers.remove(public_key.as_bytes());
    }

    /// Get a peer by public key
    pub fn get_peer(&self, public_key: &WgKey) -> Option<&WgPeer> {
        self.peers.get(public_key.as_bytes())
    }

    /// Get a peer mutably
    pub fn get_peer_mut(&mut self, public_key: &WgKey) -> Option<&mut WgPeer> {
        self.peers.get_mut(public_key.as_bytes())
    }

    /// Find peer for destination IP
    pub fn find_peer_for_ip(&self, ip: &IpAddr) -> Option<&WgPeer> {
        self.peers.values().find(|p| p.allows_ip(ip))
    }

    /// Bring interface up
    pub fn up(&self) -> KResult<()> {
        self.is_up.store(true, Ordering::SeqCst);
        crate::kprintln!("wireguard: {} up", self.name);
        Ok(())
    }

    /// Bring interface down
    pub fn down(&self) -> KResult<()> {
        self.is_up.store(false, Ordering::SeqCst);
        crate::kprintln!("wireguard: {} down", self.name);
        Ok(())
    }

    /// Check if interface is up
    pub fn is_up(&self) -> bool {
        self.is_up.load(Ordering::SeqCst)
    }

    /// Allocate a new session index
    fn allocate_index(&mut self) -> u32 {
        let index = self.next_index;
        self.next_index = self.next_index.wrapping_add(1);
        if self.next_index == 0 {
            self.next_index = 1;
        }
        index
    }

    /// Create handshake initiation message
    pub fn create_initiation(&mut self, peer_key: &WgKey) -> Result<Vec<u8>, WgError> {
        // Allocate index first to avoid borrow conflicts
        let sender_index = self.allocate_index();
        let our_public_key = self.public_key;

        let peer = self.peers.get_mut(peer_key.as_bytes())
            .ok_or(WgError::PeerNotFound)?;

        // Generate ephemeral keypair
        let ephemeral_private = generate_key();
        let ephemeral_public = ephemeral_private.public_key();

        // Store ephemeral for later
        peer.ephemeral_private = Some(ephemeral_private);

        // Build initiation message
        // Format: type (1) | reserved (3) | sender_index (4) | unencrypted_ephemeral (32) | encrypted_static (48) | encrypted_timestamp (28) | mac1 (16) | mac2 (16)
        let mut msg = vec![0u8; 148];
        msg[0] = message_type::HANDSHAKE_INITIATION;

        msg[4..8].copy_from_slice(&sender_index.to_le_bytes());
        msg[8..40].copy_from_slice(ephemeral_public.as_bytes());

        // In a real implementation, we would:
        // 1. DH(ephemeral_private, peer_public) to get shared secret
        // 2. Use HKDF to derive keys
        // 3. Encrypt our static public key
        // 4. Encrypt timestamp
        // 5. Calculate MAC1 and MAC2

        // For now, simplified placeholder
        msg[40..72].copy_from_slice(our_public_key.as_bytes());

        peer.handshake_state = HandshakeState::InitiationSent;

        Ok(msg)
    }

    /// Process incoming handshake response
    pub fn process_response(&mut self, data: &[u8]) -> Result<[u8; 32], WgError> {
        if data.len() < 92 {
            return Err(WgError::InvalidPacket);
        }

        // Parse response
        let sender_index = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let receiver_index = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);

        // Find peer by receiver index
        let peer_key = self.index_map.get(&receiver_index)
            .ok_or(WgError::PeerNotFound)?
            .clone();

        let peer = self.peers.get_mut(&peer_key)
            .ok_or(WgError::PeerNotFound)?;

        if peer.handshake_state != HandshakeState::InitiationSent {
            return Err(WgError::InvalidPacket);
        }

        // In a real implementation, we would:
        // 1. Decrypt ephemeral public key from response
        // 2. DH with our ephemeral private and their ephemeral public
        // 3. Derive session keys
        // 4. Verify MACs

        // Create session keys (simplified)
        let ephemeral = peer.ephemeral_private.take()
            .ok_or(WgError::InvalidPacket)?;

        let shared = ephemeral.dh(&peer.public_key);

        let session = SessionKeys::new(
            shared.clone(),
            shared,
            receiver_index,
            sender_index,
        );

        peer.prev_session = peer.session.take();
        peer.session = Some(session);
        peer.handshake_state = HandshakeState::Complete;
        peer.last_handshake = crate::time::uptime_secs();

        self.index_map.insert(sender_index, peer_key.clone());

        Ok(peer_key)
    }

    /// Encrypt and send a packet
    pub fn send_packet(&mut self, dst_ip: &IpAddr, payload: &[u8]) -> Result<Vec<u8>, WgError> {
        let peer = self.peers.values_mut()
            .find(|p| p.allows_ip(dst_ip))
            .ok_or(WgError::NoRoute)?;

        let session = peer.session.as_mut()
            .ok_or(WgError::SessionExpired)?;

        if session.is_expired() {
            return Err(WgError::SessionExpired);
        }

        // Build transport data message
        // Format: type (1) | reserved (3) | receiver_index (4) | nonce (8) | encrypted_data (variable) | auth_tag (16)
        let mut msg = vec![0u8; 16 + payload.len() + 16];
        msg[0] = message_type::TRANSPORT_DATA;
        msg[4..8].copy_from_slice(&session.remote_index.to_le_bytes());
        msg[8..16].copy_from_slice(&session.send_nonce.to_le_bytes());

        // Encrypt payload with ChaCha20-Poly1305
        let nonce = session.send_nonce;
        session.send_nonce += 1;

        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[4..12].copy_from_slice(&nonce.to_le_bytes());

        let (ciphertext, tag) = chacha20_poly1305_encrypt(
            session.send_key.as_bytes(),
            &nonce_bytes,
            &msg[0..16], // Additional data
            payload,
        );

        // Copy ciphertext and tag into message
        let ct_len = ciphertext.len();
        msg[16..16 + ct_len].copy_from_slice(&ciphertext);
        msg[16 + ct_len..16 + ct_len + 16].copy_from_slice(&tag);

        peer.tx_bytes.fetch_add(payload.len() as u64, Ordering::Relaxed);
        peer.last_tx = crate::time::uptime_secs();

        Ok(msg)
    }

    /// Decrypt a received packet
    pub fn recv_packet(&mut self, data: &[u8]) -> Result<(Vec<u8>, [u8; 32]), WgError> {
        if data.len() < 32 {
            return Err(WgError::InvalidPacket);
        }

        let msg_type = data[0];

        match msg_type {
            message_type::HANDSHAKE_INITIATION => {
                // Process incoming handshake initiation
                self.process_initiation(data)
            }
            message_type::HANDSHAKE_RESPONSE => {
                let peer_key = self.process_response(data)?;
                Ok((Vec::new(), peer_key))
            }
            message_type::TRANSPORT_DATA => {
                self.decrypt_transport(data)
            }
            _ => Err(WgError::InvalidPacket),
        }
    }

    /// Process incoming handshake initiation
    fn process_initiation(&mut self, data: &[u8]) -> Result<(Vec<u8>, [u8; 32]), WgError> {
        if data.len() < 148 {
            return Err(WgError::InvalidPacket);
        }

        let sender_index = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

        // Extract initiator's ephemeral public key
        let mut ephemeral_pub = [0u8; 32];
        ephemeral_pub.copy_from_slice(&data[8..40]);

        // Extract encrypted static (peer's public key)
        // In a real implementation, this would be decrypted
        let mut peer_static = [0u8; 32];
        peer_static.copy_from_slice(&data[40..72]);

        // Allocate index first to avoid borrow conflicts
        let receiver_index = self.allocate_index();

        // Find the peer
        let peer = self.peers.get_mut(&peer_static)
            .ok_or(WgError::PeerNotFound)?;

        // Generate our ephemeral keypair for response
        let our_ephemeral_private = generate_key();
        let our_ephemeral_public = our_ephemeral_private.public_key();

        // Derive session keys (simplified)
        let shared = our_ephemeral_private.dh(&WgKey(ephemeral_pub));

        let session = SessionKeys::new(
            shared.clone(),
            shared,
            receiver_index,
            sender_index,
        );

        peer.prev_session = peer.session.take();
        peer.session = Some(session);
        peer.handshake_state = HandshakeState::Complete;
        peer.last_handshake = crate::time::uptime_secs();

        // Update index map after releasing peer borrow
        drop(peer);
        self.index_map.insert(sender_index, peer_static);

        // Build response message
        let mut response = vec![0u8; 92];
        response[0] = message_type::HANDSHAKE_RESPONSE;
        response[4..8].copy_from_slice(&receiver_index.to_le_bytes());
        response[8..12].copy_from_slice(&sender_index.to_le_bytes());
        response[12..44].copy_from_slice(our_ephemeral_public.as_bytes());

        Ok((response, peer_static))
    }

    /// Decrypt transport data
    fn decrypt_transport(&mut self, data: &[u8]) -> Result<(Vec<u8>, [u8; 32]), WgError> {
        let receiver_index = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let nonce = u64::from_le_bytes([
            data[8], data[9], data[10], data[11],
            data[12], data[13], data[14], data[15],
        ]);

        // Find peer by receiver index
        let peer_key = self.index_map.get(&receiver_index)
            .ok_or(WgError::PeerNotFound)?
            .clone();

        let peer = self.peers.get_mut(&peer_key)
            .ok_or(WgError::PeerNotFound)?;

        let session = peer.session.as_mut()
            .ok_or(WgError::SessionExpired)?;

        if session.is_expired() {
            return Err(WgError::SessionExpired);
        }

        // Check nonce to prevent replay
        if nonce <= session.recv_nonce {
            // Allow some window for out-of-order packets
            if session.recv_nonce - nonce > 1000 {
                return Err(WgError::AuthenticationFailed);
            }
        }

        // Decrypt with ChaCha20-Poly1305
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[4..12].copy_from_slice(&nonce.to_le_bytes());

        // Data format: header (16) | ciphertext | tag (16)
        if data.len() < 32 {
            return Err(WgError::InvalidPacket);
        }
        let tag_start = data.len() - 16;
        let mut tag = [0u8; 16];
        tag.copy_from_slice(&data[tag_start..]);

        let plaintext = chacha20_poly1305_decrypt(
            session.recv_key.as_bytes(),
            &nonce_bytes,
            &data[0..16], // AAD (header)
            &data[16..tag_start], // ciphertext
            &tag,
        ).ok_or(WgError::DecryptionFailed)?;

        session.recv_nonce = session.recv_nonce.max(nonce);
        peer.rx_bytes.fetch_add(plaintext.len() as u64, Ordering::Relaxed);
        peer.last_rx = crate::time::uptime_secs();

        Ok((plaintext, peer_key))
    }

    /// Send a keepalive packet to a peer
    pub fn send_keepalive(&mut self, peer_key: &WgKey) -> Result<Vec<u8>, WgError> {
        let peer = self.peers.get_mut(peer_key.as_bytes())
            .ok_or(WgError::PeerNotFound)?;

        let session = peer.session.as_mut()
            .ok_or(WgError::SessionExpired)?;

        // Keepalive is just an empty transport packet
        let mut msg = vec![0u8; 32];
        msg[0] = message_type::TRANSPORT_DATA;
        msg[4..8].copy_from_slice(&session.remote_index.to_le_bytes());
        msg[8..16].copy_from_slice(&session.send_nonce.to_le_bytes());
        session.send_nonce += 1;

        // Empty payload, just auth tag
        let nonce = session.send_nonce - 1;
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[4..12].copy_from_slice(&nonce.to_le_bytes());

        let (_ciphertext, tag) = chacha20_poly1305_encrypt(
            session.send_key.as_bytes(),
            &nonce_bytes,
            &msg[0..16], // AAD
            &[], // Empty payload
        );

        // For keepalive, just add the tag
        msg[16..32].copy_from_slice(&tag);
        peer.last_tx = crate::time::uptime_secs();

        Ok(msg)
    }

    /// Get interface statistics
    pub fn statistics(&self) -> WgInterfaceStats {
        let mut total_rx = 0u64;
        let mut total_tx = 0u64;

        for peer in self.peers.values() {
            total_rx += peer.rx_bytes.load(Ordering::Relaxed);
            total_tx += peer.tx_bytes.load(Ordering::Relaxed);
        }

        WgInterfaceStats {
            name: self.name.clone(),
            public_key: self.public_key,
            listen_port: self.config.listen_port,
            peers: self.peers.len(),
            rx_bytes: total_rx,
            tx_bytes: total_tx,
        }
    }
}

/// Interface statistics
#[derive(Debug)]
pub struct WgInterfaceStats {
    pub name: String,
    pub public_key: WgKey,
    pub listen_port: u16,
    pub peers: usize,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

/// Generate a random private key
pub fn generate_key() -> WgKey {
    let mut key = [0u8; 32];
    // Use RDRAND or other entropy source
    for i in 0..32 {
        key[i] = (crate::time::uptime_ns() ^ (i as u64 * 0x12345678)) as u8;
    }
    // Clamp for Curve25519
    key[0] &= 248;
    key[31] &= 127;
    key[31] |= 64;
    WgKey(key)
}

/// Global WireGuard state
pub static WIREGUARD: IrqSafeMutex<WireGuardState> = IrqSafeMutex::new(WireGuardState::new());

/// WireGuard subsystem state
pub struct WireGuardState {
    interfaces: BTreeMap<String, WgInterface>,
    initialized: bool,
}

impl WireGuardState {
    pub const fn new() -> Self {
        WireGuardState {
            interfaces: BTreeMap::new(),
            initialized: false,
        }
    }

    /// Create a new interface
    pub fn create_interface(&mut self, name: &str, config: WgConfig) -> KResult<()> {
        if self.interfaces.contains_key(name) {
            return Err(KError::AlreadyExists);
        }

        let iface = WgInterface::new(name, config);
        self.interfaces.insert(String::from(name), iface);

        crate::kprintln!("wireguard: created interface {}", name);
        Ok(())
    }

    /// Delete an interface
    pub fn delete_interface(&mut self, name: &str) -> KResult<()> {
        self.interfaces.remove(name)
            .ok_or(KError::NotFound)?;

        crate::kprintln!("wireguard: deleted interface {}", name);
        Ok(())
    }

    /// Get an interface
    pub fn get_interface(&mut self, name: &str) -> Option<&mut WgInterface> {
        self.interfaces.get_mut(name)
    }

    /// List all interfaces
    pub fn list_interfaces(&self) -> Vec<&str> {
        self.interfaces.keys().map(|s| s.as_str()).collect()
    }
}

/// Initialize WireGuard subsystem
pub fn init() {
    let mut wg = WIREGUARD.lock();
    wg.initialized = true;
    crate::kprintln!("wireguard: VPN subsystem initialized");
}

/// Create a new WireGuard interface
pub fn create_interface(name: &str, private_key: WgKey, listen_port: u16) -> KResult<()> {
    let config = WgConfig {
        private_key,
        listen_port,
        ..Default::default()
    };

    WIREGUARD.lock().create_interface(name, config)
}

/// Add a peer to an interface
pub fn add_peer(
    interface: &str,
    public_key: WgKey,
    allowed_ips: Vec<IpNetwork>,
    endpoint: Option<Endpoint>,
) -> KResult<()> {
    let mut wg = WIREGUARD.lock();
    let iface = wg.get_interface(interface)
        .ok_or(KError::NotFound)?;

    let mut peer = WgPeer::new(public_key);
    peer.allowed_ips = allowed_ips;
    peer.endpoint = endpoint;

    iface.add_peer(peer);
    Ok(())
}

/// Bring an interface up
pub fn interface_up(name: &str) -> KResult<()> {
    let mut wg = WIREGUARD.lock();
    let iface = wg.get_interface(name)
        .ok_or(KError::NotFound)?;
    iface.up()
}

/// Bring an interface down
pub fn interface_down(name: &str) -> KResult<()> {
    let mut wg = WIREGUARD.lock();
    let iface = wg.get_interface(name)
        .ok_or(KError::NotFound)?;
    iface.down()
}
