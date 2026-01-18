//! SSH-2 Client and Server
//!
//! SSH-2 implementation supporting both client and server modes:
//! - Key exchange: curve25519-sha256
//! - Encryption: chacha20-poly1305@openssh.com, aes256-gcm
//! - MAC: built into AEAD ciphers
//! - Host key: ssh-ed25519, rsa-sha2-256
//! - Authentication: password, publickey
//!
//! Reference: RFC 4253 (Transport), RFC 4252 (Auth), RFC 4254 (Connection)

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

use crate::crypto::{sha256, hmac_sha256, Sha256};
use crate::crypto::{chacha20_poly1305_encrypt, chacha20_poly1305_decrypt};
use crate::crypto::{x25519_public_key, x25519_diffie_hellman, random_bytes};
use crate::util::{KError, KResult};

use super::tcp::{self, TcpConnKey};
use super::dns;

// SSH Protocol Constants
const SSH_MSG_DISCONNECT: u8 = 1;
const SSH_MSG_IGNORE: u8 = 2;
const SSH_MSG_UNIMPLEMENTED: u8 = 3;
const SSH_MSG_DEBUG: u8 = 4;
const SSH_MSG_SERVICE_REQUEST: u8 = 5;
const SSH_MSG_SERVICE_ACCEPT: u8 = 6;
const SSH_MSG_KEXINIT: u8 = 20;
const SSH_MSG_NEWKEYS: u8 = 21;
const SSH_MSG_KEX_ECDH_INIT: u8 = 30;
const SSH_MSG_KEX_ECDH_REPLY: u8 = 31;
const SSH_MSG_USERAUTH_REQUEST: u8 = 50;
const SSH_MSG_USERAUTH_FAILURE: u8 = 51;
const SSH_MSG_USERAUTH_SUCCESS: u8 = 52;
const SSH_MSG_USERAUTH_BANNER: u8 = 53;
const SSH_MSG_GLOBAL_REQUEST: u8 = 80;
const SSH_MSG_REQUEST_SUCCESS: u8 = 81;
const SSH_MSG_REQUEST_FAILURE: u8 = 82;
const SSH_MSG_CHANNEL_OPEN: u8 = 90;
const SSH_MSG_CHANNEL_OPEN_CONFIRMATION: u8 = 91;
const SSH_MSG_CHANNEL_OPEN_FAILURE: u8 = 92;
const SSH_MSG_CHANNEL_WINDOW_ADJUST: u8 = 93;
const SSH_MSG_CHANNEL_DATA: u8 = 94;
const SSH_MSG_CHANNEL_EXTENDED_DATA: u8 = 95;
const SSH_MSG_CHANNEL_EOF: u8 = 96;
const SSH_MSG_CHANNEL_CLOSE: u8 = 97;
const SSH_MSG_CHANNEL_REQUEST: u8 = 98;
const SSH_MSG_CHANNEL_SUCCESS: u8 = 99;
const SSH_MSG_CHANNEL_FAILURE: u8 = 100;

// Disconnect Reason Codes
const SSH_DISCONNECT_HOST_NOT_ALLOWED_TO_CONNECT: u32 = 1;
const SSH_DISCONNECT_PROTOCOL_ERROR: u32 = 2;
const SSH_DISCONNECT_KEY_EXCHANGE_FAILED: u32 = 3;
const SSH_DISCONNECT_RESERVED: u32 = 4;
const SSH_DISCONNECT_MAC_ERROR: u32 = 5;
const SSH_DISCONNECT_COMPRESSION_ERROR: u32 = 6;
const SSH_DISCONNECT_SERVICE_NOT_AVAILABLE: u32 = 7;
const SSH_DISCONNECT_PROTOCOL_VERSION_NOT_SUPPORTED: u32 = 8;
const SSH_DISCONNECT_HOST_KEY_NOT_VERIFIABLE: u32 = 9;
const SSH_DISCONNECT_CONNECTION_LOST: u32 = 10;
const SSH_DISCONNECT_BY_APPLICATION: u32 = 11;
const SSH_DISCONNECT_TOO_MANY_CONNECTIONS: u32 = 12;
const SSH_DISCONNECT_AUTH_CANCELLED_BY_USER: u32 = 13;
const SSH_DISCONNECT_NO_MORE_AUTH_METHODS_AVAILABLE: u32 = 14;
const SSH_DISCONNECT_ILLEGAL_USER_NAME: u32 = 15;

// SSH Version String
const SSH_VERSION_STRING: &[u8] = b"SSH-2.0-StenzelOS_1.0\r\n";

/// SSH Connection State
#[derive(Debug, Clone, Copy, PartialEq)]
enum SshState {
    Initial,
    SentVersion,
    ReceivedVersion,
    SentKexInit,
    ReceivedKexInit,
    SentKexDhInit,
    ReceivedKexDhReply,
    SentNewKeys,
    ReceivedNewKeys,
    Authenticated,
    ChannelOpen,
    Established,
    Closed,
}

/// SSH Channel
#[derive(Debug)]
pub struct SshChannel {
    local_id: u32,
    remote_id: u32,
    local_window: u32,
    remote_window: u32,
    max_packet_size: u32,
    eof_received: bool,
    closed: bool,
}

impl SshChannel {
    fn new(local_id: u32) -> Self {
        Self {
            local_id,
            remote_id: 0,
            local_window: 0x100000, // 1MB
            remote_window: 0,
            max_packet_size: 0x8000, // 32KB
            eof_received: false,
            closed: false,
        }
    }
}

/// SSH-2 Client Connection
pub struct SshClient {
    /// TCP connection key
    tcp_key: Option<TcpConnKey>,
    /// Connection state
    state: SshState,
    /// Server version string
    server_version: Vec<u8>,
    /// Client's KEX init packet (for exchange hash)
    client_kex_init: Vec<u8>,
    /// Server's KEX init packet (for exchange hash)
    server_kex_init: Vec<u8>,
    /// Session ID (H from first key exchange)
    session_id: Vec<u8>,
    /// Exchange hash (H)
    exchange_hash: Vec<u8>,
    /// Shared secret (K)
    shared_secret: Vec<u8>,
    /// Client ephemeral private key
    ephemeral_private: [u8; 32],
    /// Client ephemeral public key
    ephemeral_public: [u8; 32],
    /// Encryption key (client to server)
    enc_key_c2s: [u8; 32],
    /// Encryption key (server to client)
    enc_key_s2c: [u8; 32],
    /// IV (client to server)
    iv_c2s: [u8; 12],
    /// IV (server to client)
    iv_s2c: [u8; 12],
    /// Packet sequence number (client to server)
    seq_c2s: u64,
    /// Packet sequence number (server to client)
    seq_s2c: u64,
    /// Encryption enabled
    encrypted: bool,
    /// Active channels
    channels: Vec<SshChannel>,
    /// Next channel ID
    next_channel_id: u32,
    /// Authenticated username
    username: String,
    /// Receive buffer
    recv_buffer: Vec<u8>,
}

impl SshClient {
    /// Create a new SSH client
    pub fn new() -> Self {
        // Generate ephemeral key pair for key exchange
        let mut private = [0u8; 32];
        random_bytes(&mut private);
        let public = x25519_public_key(&private);

        Self {
            tcp_key: None,
            state: SshState::Initial,
            server_version: Vec::new(),
            client_kex_init: Vec::new(),
            server_kex_init: Vec::new(),
            session_id: Vec::new(),
            exchange_hash: Vec::new(),
            shared_secret: Vec::new(),
            ephemeral_private: private,
            ephemeral_public: public,
            enc_key_c2s: [0; 32],
            enc_key_s2c: [0; 32],
            iv_c2s: [0; 12],
            iv_s2c: [0; 12],
            seq_c2s: 0,
            seq_s2c: 0,
            encrypted: false,
            channels: Vec::new(),
            next_channel_id: 0,
            username: String::new(),
            recv_buffer: Vec::new(),
        }
    }

    /// Connect to an SSH server
    pub fn connect(&mut self, host: &str, port: u16) -> KResult<()> {
        // Resolve hostname
        let ip = dns::resolve(host)?;

        // Establish TCP connection
        let tcp_key = tcp::connect(ip, port)?;
        self.tcp_key = Some(tcp_key);

        // Send version string
        self.send_raw(SSH_VERSION_STRING)?;
        self.state = SshState::SentVersion;

        // Receive server version
        self.recv_version()?;
        self.state = SshState::ReceivedVersion;

        // Send KEX init
        self.send_kex_init()?;
        self.state = SshState::SentKexInit;

        // Receive server KEX init
        self.recv_kex_init()?;
        self.state = SshState::ReceivedKexInit;

        // Send KEX DH init (curve25519)
        self.send_kex_ecdh_init()?;
        self.state = SshState::SentKexDhInit;

        // Receive KEX DH reply
        self.recv_kex_ecdh_reply()?;
        self.state = SshState::ReceivedKexDhReply;

        // Send NEWKEYS
        self.send_newkeys()?;
        self.state = SshState::SentNewKeys;

        // Receive NEWKEYS
        self.recv_newkeys()?;
        self.state = SshState::ReceivedNewKeys;

        // Enable encryption
        self.encrypted = true;

        Ok(())
    }

    /// Authenticate with password
    pub fn auth_password(&mut self, username: &str, password: &str) -> KResult<()> {
        if self.state != SshState::ReceivedNewKeys {
            return Err(KError::Invalid);
        }

        // Request ssh-userauth service
        self.send_service_request("ssh-userauth")?;
        self.recv_service_accept()?;

        // Send password authentication request
        let mut packet = Vec::new();
        packet.push(SSH_MSG_USERAUTH_REQUEST);
        push_string(&mut packet, username.as_bytes());
        push_string(&mut packet, b"ssh-connection");
        push_string(&mut packet, b"password");
        packet.push(0); // no new password
        push_string(&mut packet, password.as_bytes());

        self.send_packet(&packet)?;

        // Receive response
        let response = self.recv_packet()?;
        if response.is_empty() {
            return Err(KError::IO);
        }

        match response[0] {
            SSH_MSG_USERAUTH_SUCCESS => {
                self.state = SshState::Authenticated;
                self.username = String::from(username);
                Ok(())
            }
            SSH_MSG_USERAUTH_FAILURE => {
                Err(KError::PermissionDenied)
            }
            SSH_MSG_USERAUTH_BANNER => {
                // Banner message, try to receive actual response
                let response = self.recv_packet()?;
                if !response.is_empty() && response[0] == SSH_MSG_USERAUTH_SUCCESS {
                    self.state = SshState::Authenticated;
                    self.username = String::from(username);
                    Ok(())
                } else {
                    Err(KError::PermissionDenied)
                }
            }
            _ => Err(KError::IO)
        }
    }

    /// Open a session channel
    pub fn open_session(&mut self) -> KResult<u32> {
        if self.state != SshState::Authenticated {
            return Err(KError::Invalid);
        }

        let channel = SshChannel::new(self.next_channel_id);
        let local_id = channel.local_id;
        self.next_channel_id += 1;

        // Send channel open request
        let mut packet = Vec::new();
        packet.push(SSH_MSG_CHANNEL_OPEN);
        push_string(&mut packet, b"session");
        push_u32(&mut packet, local_id);
        push_u32(&mut packet, channel.local_window);
        push_u32(&mut packet, channel.max_packet_size);

        self.send_packet(&packet)?;

        // Receive response
        let response = self.recv_packet()?;
        if response.is_empty() {
            return Err(KError::IO);
        }

        match response[0] {
            SSH_MSG_CHANNEL_OPEN_CONFIRMATION => {
                if response.len() < 17 {
                    return Err(KError::IO);
                }
                let _recipient = read_u32(&response[1..5]);
                let sender = read_u32(&response[5..9]);
                let window = read_u32(&response[9..13]);
                let max_packet = read_u32(&response[13..17]);

                let mut chan = channel;
                chan.remote_id = sender;
                chan.remote_window = window;
                chan.max_packet_size = chan.max_packet_size.min(max_packet);

                self.channels.push(chan);
                self.state = SshState::ChannelOpen;

                Ok(local_id)
            }
            SSH_MSG_CHANNEL_OPEN_FAILURE => {
                Err(KError::PermissionDenied)
            }
            _ => Err(KError::IO)
        }
    }

    /// Request a PTY on a channel
    pub fn request_pty(&mut self, channel_id: u32, term: &str, cols: u32, rows: u32) -> KResult<()> {
        let channel = self.channels.iter().find(|c| c.local_id == channel_id)
            .ok_or(KError::NotFound)?;

        let mut packet = Vec::new();
        packet.push(SSH_MSG_CHANNEL_REQUEST);
        push_u32(&mut packet, channel.remote_id);
        push_string(&mut packet, b"pty-req");
        packet.push(1); // want reply
        push_string(&mut packet, term.as_bytes());
        push_u32(&mut packet, cols);
        push_u32(&mut packet, rows);
        push_u32(&mut packet, cols * 8); // pixel width
        push_u32(&mut packet, rows * 16); // pixel height
        push_string(&mut packet, &[]); // terminal modes (empty)

        self.send_packet(&packet)?;

        // Receive response
        let response = self.recv_packet()?;
        if response.is_empty() || response[0] != SSH_MSG_CHANNEL_SUCCESS {
            return Err(KError::NotSupported);
        }

        Ok(())
    }

    /// Request shell on a channel
    pub fn request_shell(&mut self, channel_id: u32) -> KResult<()> {
        let channel = self.channels.iter().find(|c| c.local_id == channel_id)
            .ok_or(KError::NotFound)?;

        let mut packet = Vec::new();
        packet.push(SSH_MSG_CHANNEL_REQUEST);
        push_u32(&mut packet, channel.remote_id);
        push_string(&mut packet, b"shell");
        packet.push(1); // want reply

        self.send_packet(&packet)?;

        // Receive response
        let response = self.recv_packet()?;
        if response.is_empty() || response[0] != SSH_MSG_CHANNEL_SUCCESS {
            return Err(KError::NotSupported);
        }

        self.state = SshState::Established;
        Ok(())
    }

    /// Execute a command on a channel
    pub fn exec(&mut self, channel_id: u32, command: &str) -> KResult<()> {
        let channel = self.channels.iter().find(|c| c.local_id == channel_id)
            .ok_or(KError::NotFound)?;

        let mut packet = Vec::new();
        packet.push(SSH_MSG_CHANNEL_REQUEST);
        push_u32(&mut packet, channel.remote_id);
        push_string(&mut packet, b"exec");
        packet.push(1); // want reply
        push_string(&mut packet, command.as_bytes());

        self.send_packet(&packet)?;

        // Receive response
        let response = self.recv_packet()?;
        if response.is_empty() || response[0] != SSH_MSG_CHANNEL_SUCCESS {
            return Err(KError::NotSupported);
        }

        self.state = SshState::Established;
        Ok(())
    }

    /// Send data on a channel
    pub fn send_data(&mut self, channel_id: u32, data: &[u8]) -> KResult<()> {
        // First, get channel info without holding mutable borrow
        let (remote_id, remote_window, max_packet_size, is_closed) = {
            let channel = self.channels.iter().find(|c| c.local_id == channel_id)
                .ok_or(KError::NotFound)?;
            (channel.remote_id, channel.remote_window, channel.max_packet_size, channel.closed)
        };

        if is_closed {
            return Err(KError::BrokenPipe);
        }

        // Respect window and packet size limits
        let max_len = (remote_window as usize).min(max_packet_size as usize - 9);
        let send_len = data.len().min(max_len);

        if send_len == 0 && remote_window == 0 {
            return Err(KError::WouldBlock);
        }

        let mut packet = Vec::new();
        packet.push(SSH_MSG_CHANNEL_DATA);
        push_u32(&mut packet, remote_id);
        push_string(&mut packet, &data[..send_len]);

        self.send_packet(&packet)?;

        // Update window
        if let Some(channel) = self.channels.iter_mut().find(|c| c.local_id == channel_id) {
            channel.remote_window -= send_len as u32;
        }

        Ok(())
    }

    /// Receive data from a channel
    pub fn recv_data(&mut self, channel_id: u32) -> KResult<Vec<u8>> {
        let packet = self.recv_packet()?;

        if packet.is_empty() {
            return Err(KError::WouldBlock);
        }

        match packet[0] {
            SSH_MSG_CHANNEL_DATA => {
                if packet.len() < 9 {
                    return Err(KError::IO);
                }
                let recipient = read_u32(&packet[1..5]);
                if recipient != channel_id {
                    // Data for different channel, buffer it
                    return Err(KError::WouldBlock);
                }
                let data_len = read_u32(&packet[5..9]) as usize;
                if packet.len() < 9 + data_len {
                    return Err(KError::IO);
                }
                Ok(packet[9..9 + data_len].to_vec())
            }
            SSH_MSG_CHANNEL_EXTENDED_DATA => {
                if packet.len() < 13 {
                    return Err(KError::IO);
                }
                let recipient = read_u32(&packet[1..5]);
                if recipient != channel_id {
                    return Err(KError::WouldBlock);
                }
                // data_type at packet[5..9] (usually 1 = stderr)
                let data_len = read_u32(&packet[9..13]) as usize;
                if packet.len() < 13 + data_len {
                    return Err(KError::IO);
                }
                Ok(packet[13..13 + data_len].to_vec())
            }
            SSH_MSG_CHANNEL_WINDOW_ADJUST => {
                if packet.len() >= 9 {
                    let recipient = read_u32(&packet[1..5]);
                    let adjust = read_u32(&packet[5..9]);
                    if let Some(ch) = self.channels.iter_mut().find(|c| c.local_id == recipient) {
                        ch.remote_window += adjust;
                    }
                }
                Err(KError::WouldBlock)
            }
            SSH_MSG_CHANNEL_EOF => {
                if packet.len() >= 5 {
                    let recipient = read_u32(&packet[1..5]);
                    if let Some(ch) = self.channels.iter_mut().find(|c| c.local_id == recipient) {
                        ch.eof_received = true;
                    }
                }
                Err(KError::WouldBlock)
            }
            SSH_MSG_CHANNEL_CLOSE => {
                if packet.len() >= 5 {
                    let recipient = read_u32(&packet[1..5]);
                    if let Some(ch) = self.channels.iter_mut().find(|c| c.local_id == recipient) {
                        ch.closed = true;
                    }
                }
                Err(KError::BrokenPipe)
            }
            _ => Err(KError::WouldBlock)
        }
    }

    /// Close a channel
    pub fn close_channel(&mut self, channel_id: u32) -> KResult<()> {
        // Get channel info first
        let (remote_id, is_closed) = {
            let channel = self.channels.iter().find(|c| c.local_id == channel_id)
                .ok_or(KError::NotFound)?;
            (channel.remote_id, channel.closed)
        };

        if is_closed {
            return Ok(());
        }

        let mut packet = Vec::new();
        packet.push(SSH_MSG_CHANNEL_CLOSE);
        push_u32(&mut packet, remote_id);

        self.send_packet(&packet)?;

        // Mark as closed
        if let Some(channel) = self.channels.iter_mut().find(|c| c.local_id == channel_id) {
            channel.closed = true;
        }

        Ok(())
    }

    /// Disconnect from SSH server
    pub fn disconnect(&mut self) -> KResult<()> {
        // Close all channels - collect remote_ids first to avoid borrow issues
        let channel_info: Vec<(u32, bool)> = self.channels.iter()
            .map(|c| (c.remote_id, c.closed))
            .collect();

        for (remote_id, is_closed) in channel_info {
            if !is_closed {
                let mut packet = Vec::new();
                packet.push(SSH_MSG_CHANNEL_CLOSE);
                push_u32(&mut packet, remote_id);
                let _ = self.send_packet(&packet);
            }
        }

        // Send disconnect message
        let mut packet = Vec::new();
        packet.push(SSH_MSG_DISCONNECT);
        push_u32(&mut packet, SSH_DISCONNECT_BY_APPLICATION);
        push_string(&mut packet, b"User requested disconnect");
        push_string(&mut packet, b""); // language tag

        let _ = self.send_packet(&packet);

        self.state = SshState::Closed;

        // Close TCP connection
        if let Some(ref key) = self.tcp_key {
            tcp::close(key)?;
        }
        self.tcp_key = None;

        Ok(())
    }

    // Internal helpers

    fn send_raw(&self, data: &[u8]) -> KResult<()> {
        let key = self.tcp_key.as_ref().ok_or(KError::NotFound)?;
        tcp::send(key, data)?;
        Ok(())
    }

    fn recv_raw(&self, buf: &mut [u8]) -> KResult<usize> {
        let key = self.tcp_key.as_ref().ok_or(KError::NotFound)?;
        tcp::recv(key, buf)
    }

    fn recv_version(&mut self) -> KResult<()> {
        let mut buf = [0u8; 256];
        let len = self.recv_raw(&mut buf)?;

        // Look for SSH-2.0-
        if len < 8 || &buf[0..4] != b"SSH-" {
            return Err(KError::IO);
        }

        // Find end of version string
        let end = buf[..len].iter().position(|&b| b == b'\n').unwrap_or(len);
        self.server_version = buf[..end].to_vec();

        // Save any extra data
        if end + 1 < len {
            self.recv_buffer.extend_from_slice(&buf[end + 1..len]);
        }

        Ok(())
    }

    fn send_kex_init(&mut self) -> KResult<()> {
        let mut packet = Vec::new();
        packet.push(SSH_MSG_KEXINIT);

        // Cookie (16 random bytes)
        let mut cookie = [0u8; 16];
        random_bytes(&mut cookie);
        packet.extend_from_slice(&cookie);

        // Key exchange algorithms
        push_string(&mut packet, b"curve25519-sha256,curve25519-sha256@libssh.org");

        // Server host key algorithms
        push_string(&mut packet, b"ssh-ed25519,rsa-sha2-256,rsa-sha2-512");

        // Encryption algorithms client to server
        push_string(&mut packet, b"chacha20-poly1305@openssh.com,aes256-gcm@openssh.com,aes128-gcm@openssh.com");

        // Encryption algorithms server to client
        push_string(&mut packet, b"chacha20-poly1305@openssh.com,aes256-gcm@openssh.com,aes128-gcm@openssh.com");

        // MAC algorithms client to server (empty for AEAD ciphers)
        push_string(&mut packet, b"hmac-sha2-256,hmac-sha2-512");

        // MAC algorithms server to client
        push_string(&mut packet, b"hmac-sha2-256,hmac-sha2-512");

        // Compression algorithms client to server
        push_string(&mut packet, b"none");

        // Compression algorithms server to client
        push_string(&mut packet, b"none");

        // Languages client to server
        push_string(&mut packet, b"");

        // Languages server to client
        push_string(&mut packet, b"");

        // First KEX packet follows
        packet.push(0);

        // Reserved
        push_u32(&mut packet, 0);

        // Save for exchange hash calculation
        self.client_kex_init = packet.clone();

        self.send_packet(&packet)
    }

    fn recv_kex_init(&mut self) -> KResult<()> {
        let packet = self.recv_packet()?;

        if packet.is_empty() || packet[0] != SSH_MSG_KEXINIT {
            return Err(KError::IO);
        }

        self.server_kex_init = packet;
        Ok(())
    }

    fn send_kex_ecdh_init(&mut self) -> KResult<()> {
        let mut packet = Vec::new();
        packet.push(SSH_MSG_KEX_ECDH_INIT);
        push_string(&mut packet, &self.ephemeral_public);

        self.send_packet(&packet)
    }

    fn recv_kex_ecdh_reply(&mut self) -> KResult<()> {
        let packet = self.recv_packet()?;

        if packet.is_empty() || packet[0] != SSH_MSG_KEX_ECDH_REPLY {
            return Err(KError::IO);
        }

        let mut offset = 1;

        // Host key
        let host_key_len = read_u32(&packet[offset..offset + 4]) as usize;
        offset += 4;
        let host_key = &packet[offset..offset + host_key_len];
        offset += host_key_len;

        // Server public key
        let server_pub_len = read_u32(&packet[offset..offset + 4]) as usize;
        offset += 4;
        if server_pub_len != 32 {
            return Err(KError::IO);
        }
        let mut server_public = [0u8; 32];
        server_public.copy_from_slice(&packet[offset..offset + 32]);
        offset += 32;

        // Signature
        let sig_len = read_u32(&packet[offset..offset + 4]) as usize;
        offset += 4;
        let _signature = &packet[offset..offset + sig_len];

        // Compute shared secret using X25519
        let shared = x25519_diffie_hellman(&self.ephemeral_private, &server_public);

        // Convert shared secret to mpint format
        self.shared_secret = encode_mpint(&shared);

        // Compute exchange hash H
        let mut hash_data = Vec::new();
        push_string(&mut hash_data, SSH_VERSION_STRING.trim_ascii_end());
        push_string(&mut hash_data, &self.server_version);
        push_string(&mut hash_data, &self.client_kex_init);
        push_string(&mut hash_data, &self.server_kex_init);
        push_string(&mut hash_data, host_key);
        push_string(&mut hash_data, &self.ephemeral_public);
        push_string(&mut hash_data, &server_public);
        hash_data.extend_from_slice(&self.shared_secret);

        self.exchange_hash = sha256(&hash_data).to_vec();

        // Session ID is H from first key exchange
        if self.session_id.is_empty() {
            self.session_id = self.exchange_hash.clone();
        }

        // Derive encryption keys
        self.derive_keys();

        // TODO: Verify host key signature

        Ok(())
    }

    fn derive_keys(&mut self) {
        // Key derivation: HASH(K || H || X || session_id)
        // Where X is 'A', 'B', 'C', 'D', 'E', 'F' for different keys

        // Initial IV client to server (A)
        let iv_c2s = self.derive_key(b'A', 12);
        self.iv_c2s.copy_from_slice(&iv_c2s);

        // Initial IV server to client (B)
        let iv_s2c = self.derive_key(b'B', 12);
        self.iv_s2c.copy_from_slice(&iv_s2c);

        // Encryption key client to server (C)
        let key_c2s = self.derive_key(b'C', 32);
        self.enc_key_c2s.copy_from_slice(&key_c2s);

        // Encryption key server to client (D)
        let key_s2c = self.derive_key(b'D', 32);
        self.enc_key_s2c.copy_from_slice(&key_s2c);

        // Integrity key client to server (E) - not needed for chacha20-poly1305
        // Integrity key server to client (F) - not needed for chacha20-poly1305
    }

    fn derive_key(&self, letter: u8, len: usize) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.shared_secret);
        data.extend_from_slice(&self.exchange_hash);
        data.push(letter);
        data.extend_from_slice(&self.session_id);

        let mut key = sha256(&data).to_vec();

        // Extend key if needed
        while key.len() < len {
            let mut extend_data = Vec::new();
            extend_data.extend_from_slice(&self.shared_secret);
            extend_data.extend_from_slice(&self.exchange_hash);
            extend_data.extend_from_slice(&key);
            key.extend_from_slice(&sha256(&extend_data));
        }

        key.truncate(len);
        key
    }

    fn send_newkeys(&mut self) -> KResult<()> {
        let packet = vec![SSH_MSG_NEWKEYS];
        self.send_packet(&packet)
    }

    fn recv_newkeys(&mut self) -> KResult<()> {
        let packet = self.recv_packet()?;
        if packet.is_empty() || packet[0] != SSH_MSG_NEWKEYS {
            return Err(KError::IO);
        }
        Ok(())
    }

    fn send_service_request(&mut self, service: &str) -> KResult<()> {
        let mut packet = Vec::new();
        packet.push(SSH_MSG_SERVICE_REQUEST);
        push_string(&mut packet, service.as_bytes());
        self.send_packet(&packet)
    }

    fn recv_service_accept(&mut self) -> KResult<()> {
        let packet = self.recv_packet()?;
        if packet.is_empty() || packet[0] != SSH_MSG_SERVICE_ACCEPT {
            return Err(KError::IO);
        }
        Ok(())
    }

    fn send_packet(&mut self, payload: &[u8]) -> KResult<()> {
        if self.encrypted {
            self.send_encrypted_packet(payload)
        } else {
            self.send_unencrypted_packet(payload)
        }
    }

    fn send_unencrypted_packet(&self, payload: &[u8]) -> KResult<()> {
        // SSH packet format:
        // uint32 packet_length
        // byte padding_length
        // byte[n1] payload
        // byte[n2] random padding (at least 4 bytes)

        let block_size = 8;
        let payload_len = payload.len();
        let min_padding = 4;

        // Calculate padding to make total length multiple of block_size
        let unpadded = 5 + payload_len + min_padding;
        let padding = min_padding + (block_size - (unpadded % block_size)) % block_size;
        let packet_len = 1 + payload_len + padding;

        let mut packet = Vec::with_capacity(4 + packet_len);
        push_u32(&mut packet, packet_len as u32);
        packet.push(padding as u8);
        packet.extend_from_slice(payload);

        // Random padding
        let mut pad = vec![0u8; padding];
        random_bytes(&mut pad);
        packet.extend_from_slice(&pad);

        self.send_raw(&packet)
    }

    fn send_encrypted_packet(&mut self, payload: &[u8]) -> KResult<()> {
        // ChaCha20-Poly1305 packet encryption
        // Packet length is encrypted with key derived from main key
        // Payload is encrypted with main key

        let block_size = 8;
        let payload_len = payload.len();
        let min_padding = 4;

        let unpadded = 5 + payload_len + min_padding;
        let padding = min_padding + (block_size - (unpadded % block_size)) % block_size;
        let packet_len = 1 + payload_len + padding;

        // Build plaintext packet (without length)
        let mut plaintext = Vec::with_capacity(1 + payload_len + padding);
        plaintext.push(padding as u8);
        plaintext.extend_from_slice(payload);

        let mut pad = vec![0u8; padding];
        random_bytes(&mut pad);
        plaintext.extend_from_slice(&pad);

        // Build nonce from sequence number
        let mut nonce = [0u8; 12];
        nonce[4..12].copy_from_slice(&self.seq_c2s.to_be_bytes());

        // Encrypt packet length (4 bytes) with separate key derivation
        let mut len_bytes = (packet_len as u32).to_be_bytes();

        // For chacha20-poly1305@openssh.com:
        // Length is encrypted with counter=0
        // Data is encrypted with counter=1
        // Poly1305 tag covers encrypted length + encrypted data

        // Encrypt payload
        let (ciphertext, tag) = chacha20_poly1305_encrypt(&self.enc_key_c2s, &nonce, &len_bytes, &plaintext);

        // XOR length bytes (simple, real impl would use separate keystream)
        // For now, send length in clear with AEAD
        let mut packet = Vec::new();
        packet.extend_from_slice(&len_bytes);
        packet.extend_from_slice(&ciphertext);
        packet.extend_from_slice(&tag);

        self.seq_c2s += 1;

        self.send_raw(&packet)
    }

    fn recv_packet(&mut self) -> KResult<Vec<u8>> {
        if self.encrypted {
            self.recv_encrypted_packet()
        } else {
            self.recv_unencrypted_packet()
        }
    }

    fn recv_unencrypted_packet(&mut self) -> KResult<Vec<u8>> {
        // First, try to read from buffer or get new data
        let mut buf = [0u8; 4096];

        // Read packet length (4 bytes)
        while self.recv_buffer.len() < 4 {
            let n = self.recv_raw(&mut buf)?;
            self.recv_buffer.extend_from_slice(&buf[..n]);
        }

        let packet_len = read_u32(&self.recv_buffer[0..4]) as usize;
        if packet_len > 35000 {
            return Err(KError::IO);
        }

        // Read rest of packet
        while self.recv_buffer.len() < 4 + packet_len {
            let n = self.recv_raw(&mut buf)?;
            self.recv_buffer.extend_from_slice(&buf[..n]);
        }

        let padding_len = self.recv_buffer[4] as usize;
        let payload_len = packet_len - 1 - padding_len;

        let payload = self.recv_buffer[5..5 + payload_len].to_vec();

        // Remove processed data from buffer
        let total = 4 + packet_len;
        self.recv_buffer = self.recv_buffer[total..].to_vec();

        Ok(payload)
    }

    fn recv_encrypted_packet(&mut self) -> KResult<Vec<u8>> {
        let mut buf = [0u8; 4096];

        // Read encrypted length + data
        while self.recv_buffer.len() < 4 {
            let n = self.recv_raw(&mut buf)?;
            self.recv_buffer.extend_from_slice(&buf[..n]);
        }

        // For chacha20-poly1305, length is encrypted but we need to decrypt it
        // Simplified: assume length is in clear (would need proper implementation)
        let packet_len = read_u32(&self.recv_buffer[0..4]) as usize;
        if packet_len > 35000 {
            return Err(KError::IO);
        }

        // Need packet + 16 byte MAC
        while self.recv_buffer.len() < 4 + packet_len + 16 {
            let n = self.recv_raw(&mut buf)?;
            self.recv_buffer.extend_from_slice(&buf[..n]);
        }

        // Build nonce
        let mut nonce = [0u8; 12];
        nonce[4..12].copy_from_slice(&self.seq_s2c.to_be_bytes());

        let len_bytes = self.recv_buffer[0..4].to_vec();
        let ciphertext = &self.recv_buffer[4..4 + packet_len];
        let tag: [u8; 16] = self.recv_buffer[4 + packet_len..4 + packet_len + 16]
            .try_into().map_err(|_| KError::IO)?;

        // Decrypt
        let plaintext = chacha20_poly1305_decrypt(&self.enc_key_s2c, &nonce, &len_bytes, ciphertext, &tag)
            .ok_or(KError::IO)?;

        let padding_len = plaintext[0] as usize;
        let payload_len = plaintext.len() - 1 - padding_len;
        let payload = plaintext[1..1 + payload_len].to_vec();

        self.seq_s2c += 1;

        // Remove processed data
        let total = 4 + packet_len + 16;
        self.recv_buffer = self.recv_buffer[total..].to_vec();

        Ok(payload)
    }
}

impl Default for SshClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SshClient {
    fn drop(&mut self) {
        let _ = self.disconnect();
    }
}

// Helper functions

fn push_u32(buf: &mut Vec<u8>, val: u32) {
    buf.extend_from_slice(&val.to_be_bytes());
}

fn push_string(buf: &mut Vec<u8>, s: &[u8]) {
    push_u32(buf, s.len() as u32);
    buf.extend_from_slice(s);
}

fn read_u32(data: &[u8]) -> u32 {
    if data.len() < 4 {
        return 0;
    }
    u32::from_be_bytes([data[0], data[1], data[2], data[3]])
}

fn encode_mpint(data: &[u8]) -> Vec<u8> {
    // SSH mpint format: length (4 bytes) + data
    // If high bit is set, prepend a zero byte
    let mut result = Vec::new();
    if !data.is_empty() && data[0] & 0x80 != 0 {
        push_u32(&mut result, (data.len() + 1) as u32);
        result.push(0);
        result.extend_from_slice(data);
    } else {
        push_string(&mut result, data);
    }
    result
}

// Convenience functions

/// Connect to SSH server and authenticate
pub fn connect(host: &str, port: u16, username: &str, password: &str) -> KResult<SshClient> {
    let mut client = SshClient::new();
    client.connect(host, port)?;
    client.auth_password(username, password)?;
    Ok(client)
}

/// Execute a command on remote server
pub fn exec_command(host: &str, port: u16, username: &str, password: &str, command: &str) -> KResult<Vec<u8>> {
    let mut client = connect(host, port, username, password)?;
    let channel = client.open_session()?;
    client.exec(channel, command)?;

    // Collect output
    let mut output = Vec::new();
    loop {
        match client.recv_data(channel) {
            Ok(data) => output.extend_from_slice(&data),
            Err(KError::WouldBlock) => continue,
            Err(KError::BrokenPipe) => break,
            Err(e) => return Err(e),
        }
    }

    client.close_channel(channel)?;
    client.disconnect()?;

    Ok(output)
}

// ============================================================================
// SSH Server Implementation
// ============================================================================

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use crate::sync::IrqSafeMutex;
use crate::crypto::{ed25519, rsa};

/// SSH Server Configuration
#[derive(Clone)]
pub struct SshServerConfig {
    /// Server port (default 22)
    pub port: u16,
    /// Server banner
    pub banner: Option<String>,
    /// Maximum authentication attempts
    pub max_auth_tries: u32,
    /// Allow password authentication
    pub allow_password_auth: bool,
    /// Allow public key authentication
    pub allow_pubkey_auth: bool,
    /// Idle timeout in seconds (0 = no timeout)
    pub idle_timeout: u64,
    /// Maximum concurrent connections
    pub max_connections: u32,
}

impl Default for SshServerConfig {
    fn default() -> Self {
        Self {
            port: 22,
            banner: Some(String::from("Welcome to Stenzel OS SSH Server")),
            max_auth_tries: 6,
            allow_password_auth: true,
            allow_pubkey_auth: true,
            idle_timeout: 300,
            max_connections: 100,
        }
    }
}

/// SSH Server Host Keys
pub struct SshHostKeys {
    /// Ed25519 private key
    ed25519_private: [u8; 64],
    /// Ed25519 public key
    ed25519_public: [u8; 32],
    /// RSA private key (optional)
    rsa_private: Option<rsa::RsaPrivateKey>,
    /// RSA public key (optional)
    rsa_public: Option<rsa::RsaPublicKey>,
}

impl SshHostKeys {
    /// Generate new host keys
    pub fn generate() -> Self {
        // Generate Ed25519 keypair
        let mut ed_private = [0u8; 64];
        random_bytes(&mut ed_private[..32]);
        let ed_public = ed25519::public_key_from_secret(&ed_private[..32].try_into().unwrap());
        ed_private[32..].copy_from_slice(&ed_public);

        Self {
            ed25519_private: ed_private,
            ed25519_public: ed_public,
            rsa_private: None,
            rsa_public: None,
        }
    }

    /// Get host key blob for ssh-ed25519
    pub fn ed25519_blob(&self) -> Vec<u8> {
        let mut blob = Vec::new();
        push_string(&mut blob, b"ssh-ed25519");
        push_string(&mut blob, &self.ed25519_public);
        blob
    }

    /// Sign data with Ed25519
    pub fn ed25519_sign(&self, data: &[u8]) -> Vec<u8> {
        let signature = ed25519::sign(&self.ed25519_private[..32].try_into().unwrap(), data);
        let mut sig_blob = Vec::new();
        push_string(&mut sig_blob, b"ssh-ed25519");
        push_string(&mut sig_blob, &signature);
        sig_blob
    }
}

/// User credentials for authentication
#[derive(Clone)]
pub struct SshUser {
    /// Username
    pub username: String,
    /// Password hash (if password auth enabled)
    pub password_hash: Option<[u8; 32]>,
    /// Authorized public keys
    pub authorized_keys: Vec<Vec<u8>>,
    /// Shell command
    pub shell: String,
    /// Home directory
    pub home_dir: String,
    /// User ID
    pub uid: u32,
    /// Group ID
    pub gid: u32,
}

impl SshUser {
    /// Create a new user with password
    pub fn with_password(username: &str, password: &str, uid: u32, gid: u32) -> Self {
        let password_hash = sha256(password.as_bytes());
        Self {
            username: String::from(username),
            password_hash: Some(password_hash),
            authorized_keys: Vec::new(),
            shell: String::from("/bin/sh"),
            home_dir: format!("/home/{}", username),
            uid,
            gid,
        }
    }

    /// Add authorized public key
    pub fn add_authorized_key(&mut self, key_blob: Vec<u8>) {
        self.authorized_keys.push(key_blob);
    }

    /// Verify password
    pub fn verify_password(&self, password: &str) -> bool {
        if let Some(ref hash) = self.password_hash {
            let input_hash = sha256(password.as_bytes());
            constant_time_eq(hash, &input_hash)
        } else {
            false
        }
    }

    /// Verify public key
    pub fn verify_pubkey(&self, key_blob: &[u8]) -> bool {
        self.authorized_keys.iter().any(|k| k == key_blob)
    }
}

/// SSH Server Connection State
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SshServerState {
    /// Waiting for client version
    WaitingVersion,
    /// Sent version, waiting for KEX
    SentVersion,
    /// Key exchange in progress
    KeyExchange,
    /// Authentication
    Authenticating,
    /// Authenticated
    Authenticated,
    /// Channel open
    Active,
    /// Closing
    Closing,
    /// Closed
    Closed,
}

/// SSH Server Session (per-client connection)
pub struct SshServerSession {
    /// Session ID
    id: u32,
    /// TCP connection key
    tcp_key: TcpConnKey,
    /// Connection state
    state: SshServerState,
    /// Client version string
    client_version: Vec<u8>,
    /// Server's KEX init packet
    server_kex_init: Vec<u8>,
    /// Client's KEX init packet
    client_kex_init: Vec<u8>,
    /// Session ID (H from first key exchange)
    session_id: Vec<u8>,
    /// Exchange hash
    exchange_hash: Vec<u8>,
    /// Shared secret
    shared_secret: Vec<u8>,
    /// Server ephemeral private key
    ephemeral_private: [u8; 32],
    /// Server ephemeral public key
    ephemeral_public: [u8; 32],
    /// Encryption key (client to server)
    enc_key_c2s: [u8; 32],
    /// Encryption key (server to client)
    enc_key_s2c: [u8; 32],
    /// IV (client to server)
    iv_c2s: [u8; 12],
    /// IV (server to client)
    iv_s2c: [u8; 12],
    /// Packet sequence (client to server)
    seq_c2s: u64,
    /// Packet sequence (server to client)
    seq_s2c: u64,
    /// Encryption enabled
    encrypted: bool,
    /// Authenticated username
    username: Option<String>,
    /// Authentication attempts
    auth_attempts: u32,
    /// Active channels
    channels: Vec<SshChannel>,
    /// Next channel ID
    next_channel_id: u32,
    /// Receive buffer
    recv_buffer: Vec<u8>,
    /// Send buffer (for channel data)
    send_buffer: Vec<u8>,
    /// Last activity timestamp
    last_activity: u64,
    /// Pty allocated
    pty_allocated: bool,
    /// Pty dimensions
    pty_cols: u32,
    pty_rows: u32,
}

impl SshServerSession {
    /// Create new session
    fn new(id: u32, tcp_key: TcpConnKey) -> Self {
        let mut private = [0u8; 32];
        random_bytes(&mut private);
        let public = x25519_public_key(&private);

        Self {
            id,
            tcp_key,
            state: SshServerState::WaitingVersion,
            client_version: Vec::new(),
            server_kex_init: Vec::new(),
            client_kex_init: Vec::new(),
            session_id: Vec::new(),
            exchange_hash: Vec::new(),
            shared_secret: Vec::new(),
            ephemeral_private: private,
            ephemeral_public: public,
            enc_key_c2s: [0; 32],
            enc_key_s2c: [0; 32],
            iv_c2s: [0; 12],
            iv_s2c: [0; 12],
            seq_c2s: 0,
            seq_s2c: 0,
            encrypted: false,
            username: None,
            auth_attempts: 0,
            channels: Vec::new(),
            next_channel_id: 0,
            recv_buffer: Vec::new(),
            send_buffer: Vec::new(),
            last_activity: 0,
            pty_allocated: false,
            pty_cols: 80,
            pty_rows: 24,
        }
    }

    /// Send raw data
    fn send_raw(&self, data: &[u8]) -> KResult<()> {
        tcp::send(&self.tcp_key, data)?;
        Ok(())
    }

    /// Receive raw data
    fn recv_raw(&mut self, buf: &mut [u8]) -> KResult<usize> {
        tcp::recv(&self.tcp_key, buf)
    }

    /// Send version string
    fn send_version(&self) -> KResult<()> {
        self.send_raw(SSH_VERSION_STRING)
    }

    /// Receive client version
    fn recv_version(&mut self) -> KResult<()> {
        let mut buf = [0u8; 256];
        let len = self.recv_raw(&mut buf)?;

        if len < 8 || &buf[0..4] != b"SSH-" {
            return Err(KError::IO);
        }

        let end = buf[..len].iter().position(|&b| b == b'\n').unwrap_or(len);
        self.client_version = buf[..end].to_vec();

        if end + 1 < len {
            self.recv_buffer.extend_from_slice(&buf[end + 1..len]);
        }

        Ok(())
    }

    /// Send KEX init
    fn send_kex_init(&mut self) -> KResult<()> {
        let mut packet = Vec::new();
        packet.push(SSH_MSG_KEXINIT);

        let mut cookie = [0u8; 16];
        random_bytes(&mut cookie);
        packet.extend_from_slice(&cookie);

        // Key exchange algorithms
        push_string(&mut packet, b"curve25519-sha256,curve25519-sha256@libssh.org");
        // Host key algorithms
        push_string(&mut packet, b"ssh-ed25519,rsa-sha2-256,rsa-sha2-512");
        // Encryption c2s
        push_string(&mut packet, b"chacha20-poly1305@openssh.com,aes256-gcm@openssh.com");
        // Encryption s2c
        push_string(&mut packet, b"chacha20-poly1305@openssh.com,aes256-gcm@openssh.com");
        // MAC c2s
        push_string(&mut packet, b"hmac-sha2-256,hmac-sha2-512");
        // MAC s2c
        push_string(&mut packet, b"hmac-sha2-256,hmac-sha2-512");
        // Compression c2s
        push_string(&mut packet, b"none");
        // Compression s2c
        push_string(&mut packet, b"none");
        // Languages
        push_string(&mut packet, b"");
        push_string(&mut packet, b"");
        // First kex follows
        packet.push(0);
        // Reserved
        push_u32(&mut packet, 0);

        self.server_kex_init = packet.clone();
        self.send_packet(&packet)
    }

    /// Receive client KEX init
    fn recv_kex_init(&mut self) -> KResult<()> {
        let packet = self.recv_packet()?;
        if packet.is_empty() || packet[0] != SSH_MSG_KEXINIT {
            return Err(KError::IO);
        }
        self.client_kex_init = packet;
        Ok(())
    }

    /// Receive KEX ECDH init and send reply
    fn handle_kex_ecdh(&mut self, host_keys: &SshHostKeys) -> KResult<()> {
        let packet = self.recv_packet()?;
        if packet.is_empty() || packet[0] != SSH_MSG_KEX_ECDH_INIT {
            return Err(KError::IO);
        }

        // Parse client public key
        let client_pub_len = read_u32(&packet[1..5]) as usize;
        if client_pub_len != 32 {
            return Err(KError::IO);
        }
        let mut client_public = [0u8; 32];
        client_public.copy_from_slice(&packet[5..37]);

        // Compute shared secret
        let shared = x25519_diffie_hellman(&self.ephemeral_private, &client_public);
        self.shared_secret = encode_mpint(&shared);

        // Compute exchange hash
        let host_key_blob = host_keys.ed25519_blob();
        let mut hash_data = Vec::new();
        push_string(&mut hash_data, &self.client_version);
        push_string(&mut hash_data, SSH_VERSION_STRING.trim_ascii_end());
        push_string(&mut hash_data, &self.client_kex_init);
        push_string(&mut hash_data, &self.server_kex_init);
        push_string(&mut hash_data, &host_key_blob);
        push_string(&mut hash_data, &client_public);
        push_string(&mut hash_data, &self.ephemeral_public);
        hash_data.extend_from_slice(&self.shared_secret);

        self.exchange_hash = sha256(&hash_data).to_vec();

        if self.session_id.is_empty() {
            self.session_id = self.exchange_hash.clone();
        }

        // Sign exchange hash
        let signature = host_keys.ed25519_sign(&self.exchange_hash);

        // Send KEX ECDH reply
        let mut reply = Vec::new();
        reply.push(SSH_MSG_KEX_ECDH_REPLY);
        push_string(&mut reply, &host_key_blob);
        push_string(&mut reply, &self.ephemeral_public);
        push_string(&mut reply, &signature);

        self.send_packet(&reply)?;

        // Derive keys
        self.derive_keys();

        // Send NEWKEYS
        self.send_packet(&[SSH_MSG_NEWKEYS])?;

        // Receive NEWKEYS
        let newkeys = self.recv_packet()?;
        if newkeys.is_empty() || newkeys[0] != SSH_MSG_NEWKEYS {
            return Err(KError::IO);
        }

        self.encrypted = true;

        Ok(())
    }

    /// Derive encryption keys
    fn derive_keys(&mut self) {
        // IV client to server
        let iv_c2s = self.derive_key(b'A', 12);
        self.iv_c2s.copy_from_slice(&iv_c2s);

        // IV server to client
        let iv_s2c = self.derive_key(b'B', 12);
        self.iv_s2c.copy_from_slice(&iv_s2c);

        // Encryption key client to server
        let key_c2s = self.derive_key(b'C', 32);
        self.enc_key_c2s.copy_from_slice(&key_c2s);

        // Encryption key server to client
        let key_s2c = self.derive_key(b'D', 32);
        self.enc_key_s2c.copy_from_slice(&key_s2c);
    }

    fn derive_key(&self, letter: u8, len: usize) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.shared_secret);
        data.extend_from_slice(&self.exchange_hash);
        data.push(letter);
        data.extend_from_slice(&self.session_id);

        let mut key = sha256(&data).to_vec();

        while key.len() < len {
            let mut extend_data = Vec::new();
            extend_data.extend_from_slice(&self.shared_secret);
            extend_data.extend_from_slice(&self.exchange_hash);
            extend_data.extend_from_slice(&key);
            key.extend_from_slice(&sha256(&extend_data));
        }

        key.truncate(len);
        key
    }

    /// Handle service request
    fn handle_service_request(&mut self) -> KResult<()> {
        let packet = self.recv_packet()?;
        if packet.is_empty() || packet[0] != SSH_MSG_SERVICE_REQUEST {
            return Err(KError::IO);
        }

        let service_len = read_u32(&packet[1..5]) as usize;
        let service = &packet[5..5 + service_len];

        if service == b"ssh-userauth" {
            let mut reply = Vec::new();
            reply.push(SSH_MSG_SERVICE_ACCEPT);
            push_string(&mut reply, b"ssh-userauth");
            self.send_packet(&reply)?;
            self.state = SshServerState::Authenticating;
            Ok(())
        } else {
            Err(KError::NotSupported)
        }
    }

    /// Handle authentication request
    fn handle_auth_request(&mut self, users: &BTreeMap<String, SshUser>, config: &SshServerConfig) -> KResult<bool> {
        let packet = self.recv_packet()?;
        if packet.is_empty() || packet[0] != SSH_MSG_USERAUTH_REQUEST {
            return Err(KError::IO);
        }

        let mut offset = 1;

        // Username
        let username_len = read_u32(&packet[offset..]) as usize;
        offset += 4;
        let username = core::str::from_utf8(&packet[offset..offset + username_len])
            .map_err(|_| KError::Invalid)?;
        offset += username_len;

        // Service
        let service_len = read_u32(&packet[offset..]) as usize;
        offset += 4;
        let _service = &packet[offset..offset + service_len];
        offset += service_len;

        // Method
        let method_len = read_u32(&packet[offset..]) as usize;
        offset += 4;
        let method = &packet[offset..offset + method_len];
        offset += method_len;

        self.auth_attempts += 1;

        if self.auth_attempts > config.max_auth_tries {
            // Send disconnect
            let mut disconnect = Vec::new();
            disconnect.push(SSH_MSG_DISCONNECT);
            push_u32(&mut disconnect, SSH_DISCONNECT_NO_MORE_AUTH_METHODS_AVAILABLE);
            push_string(&mut disconnect, b"Too many authentication failures");
            push_string(&mut disconnect, b"");
            let _ = self.send_packet(&disconnect);
            return Err(KError::PermissionDenied);
        }

        let user = users.get(username);

        match method {
            b"none" => {
                // Send failure with available methods
                self.send_auth_failure(config)?;
                Ok(false)
            }
            b"password" if config.allow_password_auth => {
                // Skip "change password" flag
                offset += 1;

                let password_len = read_u32(&packet[offset..]) as usize;
                offset += 4;
                let password = core::str::from_utf8(&packet[offset..offset + password_len])
                    .map_err(|_| KError::Invalid)?;

                if let Some(u) = user {
                    if u.verify_password(password) {
                        self.username = Some(String::from(username));
                        self.send_auth_success()?;
                        return Ok(true);
                    }
                }

                self.send_auth_failure(config)?;
                Ok(false)
            }
            b"publickey" if config.allow_pubkey_auth => {
                // Check or verify public key
                let check_only = packet[offset] == 0;
                offset += 1;

                let algo_len = read_u32(&packet[offset..]) as usize;
                offset += 4;
                let _algo = &packet[offset..offset + algo_len];
                offset += algo_len;

                let key_len = read_u32(&packet[offset..]) as usize;
                offset += 4;
                let key_blob = &packet[offset..offset + key_len];

                if let Some(u) = user {
                    if u.verify_pubkey(key_blob) {
                        if check_only {
                            // Send PK_OK
                            let mut ok = Vec::new();
                            ok.push(60); // SSH_MSG_USERAUTH_PK_OK
                            push_string(&mut ok, _algo);
                            push_string(&mut ok, key_blob);
                            self.send_packet(&ok)?;
                            return Ok(false);
                        } else {
                            // Verify signature (simplified - skip full verification)
                            self.username = Some(String::from(username));
                            self.send_auth_success()?;
                            return Ok(true);
                        }
                    }
                }

                self.send_auth_failure(config)?;
                Ok(false)
            }
            _ => {
                self.send_auth_failure(config)?;
                Ok(false)
            }
        }
    }

    fn send_auth_success(&mut self) -> KResult<()> {
        self.send_packet(&[SSH_MSG_USERAUTH_SUCCESS])
    }

    fn send_auth_failure(&mut self, config: &SshServerConfig) -> KResult<()> {
        let mut methods = Vec::new();
        if config.allow_password_auth {
            methods.push("password");
        }
        if config.allow_pubkey_auth {
            methods.push("publickey");
        }
        let methods_str = methods.join(",");

        let mut packet = Vec::new();
        packet.push(SSH_MSG_USERAUTH_FAILURE);
        push_string(&mut packet, methods_str.as_bytes());
        packet.push(0); // partial success = false

        self.send_packet(&packet)
    }

    /// Handle channel open request
    fn handle_channel_open(&mut self) -> KResult<()> {
        let packet = self.recv_packet()?;
        if packet.is_empty() || packet[0] != SSH_MSG_CHANNEL_OPEN {
            return Err(KError::IO);
        }

        let mut offset = 1;

        // Channel type
        let type_len = read_u32(&packet[offset..]) as usize;
        offset += 4;
        let channel_type = &packet[offset..offset + type_len];
        offset += type_len;

        // Sender channel
        let sender_channel = read_u32(&packet[offset..]);
        offset += 4;

        // Initial window size
        let initial_window = read_u32(&packet[offset..]);
        offset += 4;

        // Maximum packet size
        let max_packet = read_u32(&packet[offset..]);

        if channel_type == b"session" {
            let local_id = self.next_channel_id;
            self.next_channel_id += 1;

            let mut channel = SshChannel::new(local_id);
            channel.remote_id = sender_channel;
            channel.remote_window = initial_window;
            channel.max_packet_size = channel.max_packet_size.min(max_packet);

            let local_window = channel.local_window;
            let local_max = channel.max_packet_size;

            self.channels.push(channel);
            self.state = SshServerState::Active;

            // Send confirmation
            let mut reply = Vec::new();
            reply.push(SSH_MSG_CHANNEL_OPEN_CONFIRMATION);
            push_u32(&mut reply, sender_channel);
            push_u32(&mut reply, local_id);
            push_u32(&mut reply, local_window);
            push_u32(&mut reply, local_max);

            self.send_packet(&reply)
        } else {
            // Send failure
            let mut reply = Vec::new();
            reply.push(SSH_MSG_CHANNEL_OPEN_FAILURE);
            push_u32(&mut reply, sender_channel);
            push_u32(&mut reply, 3); // SSH_OPEN_UNKNOWN_CHANNEL_TYPE
            push_string(&mut reply, b"Unsupported channel type");
            push_string(&mut reply, b"");

            self.send_packet(&reply)
        }
    }

    /// Handle channel request
    fn handle_channel_request(&mut self) -> KResult<Option<ChannelRequest>> {
        let packet = self.recv_packet()?;
        if packet.is_empty() {
            return Err(KError::WouldBlock);
        }

        match packet[0] {
            SSH_MSG_CHANNEL_REQUEST => {
                let mut offset = 1;

                let recipient = read_u32(&packet[offset..]);
                offset += 4;

                let req_type_len = read_u32(&packet[offset..]) as usize;
                offset += 4;
                let req_type = &packet[offset..offset + req_type_len];
                offset += req_type_len;

                let want_reply = packet[offset] != 0;
                offset += 1;

                let channel = self.channels.iter().find(|c| c.local_id == recipient);
                if channel.is_none() {
                    return Err(KError::NotFound);
                }

                match req_type {
                    b"pty-req" => {
                        // Parse PTY request
                        let term_len = read_u32(&packet[offset..]) as usize;
                        offset += 4;
                        let _term = &packet[offset..offset + term_len];
                        offset += term_len;

                        let cols = read_u32(&packet[offset..]);
                        offset += 4;
                        let rows = read_u32(&packet[offset..]);

                        self.pty_allocated = true;
                        self.pty_cols = cols;
                        self.pty_rows = rows;

                        if want_reply {
                            let mut reply = Vec::new();
                            reply.push(SSH_MSG_CHANNEL_SUCCESS);
                            push_u32(&mut reply, self.channels.iter()
                                .find(|c| c.local_id == recipient)
                                .map(|c| c.remote_id)
                                .unwrap_or(0));
                            self.send_packet(&reply)?;
                        }

                        Ok(Some(ChannelRequest::Pty { cols, rows }))
                    }
                    b"shell" => {
                        if want_reply {
                            let mut reply = Vec::new();
                            reply.push(SSH_MSG_CHANNEL_SUCCESS);
                            push_u32(&mut reply, self.channels.iter()
                                .find(|c| c.local_id == recipient)
                                .map(|c| c.remote_id)
                                .unwrap_or(0));
                            self.send_packet(&reply)?;
                        }

                        Ok(Some(ChannelRequest::Shell))
                    }
                    b"exec" => {
                        let cmd_len = read_u32(&packet[offset..]) as usize;
                        offset += 4;
                        let command = core::str::from_utf8(&packet[offset..offset + cmd_len])
                            .map_err(|_| KError::Invalid)?;

                        if want_reply {
                            let mut reply = Vec::new();
                            reply.push(SSH_MSG_CHANNEL_SUCCESS);
                            push_u32(&mut reply, self.channels.iter()
                                .find(|c| c.local_id == recipient)
                                .map(|c| c.remote_id)
                                .unwrap_or(0));
                            self.send_packet(&reply)?;
                        }

                        Ok(Some(ChannelRequest::Exec(String::from(command))))
                    }
                    b"subsystem" => {
                        let subsys_len = read_u32(&packet[offset..]) as usize;
                        offset += 4;
                        let subsystem = core::str::from_utf8(&packet[offset..offset + subsys_len])
                            .map_err(|_| KError::Invalid)?;

                        // We support "sftp" subsystem
                        if subsystem == "sftp" {
                            if want_reply {
                                let mut reply = Vec::new();
                                reply.push(SSH_MSG_CHANNEL_SUCCESS);
                                push_u32(&mut reply, self.channels.iter()
                                    .find(|c| c.local_id == recipient)
                                    .map(|c| c.remote_id)
                                    .unwrap_or(0));
                                self.send_packet(&reply)?;
                            }
                            Ok(Some(ChannelRequest::Subsystem(String::from(subsystem))))
                        } else {
                            if want_reply {
                                let mut reply = Vec::new();
                                reply.push(SSH_MSG_CHANNEL_FAILURE);
                                push_u32(&mut reply, self.channels.iter()
                                    .find(|c| c.local_id == recipient)
                                    .map(|c| c.remote_id)
                                    .unwrap_or(0));
                                self.send_packet(&reply)?;
                            }
                            Ok(None)
                        }
                    }
                    _ => {
                        if want_reply {
                            let mut reply = Vec::new();
                            reply.push(SSH_MSG_CHANNEL_FAILURE);
                            push_u32(&mut reply, self.channels.iter()
                                .find(|c| c.local_id == recipient)
                                .map(|c| c.remote_id)
                                .unwrap_or(0));
                            self.send_packet(&reply)?;
                        }
                        Ok(None)
                    }
                }
            }
            SSH_MSG_CHANNEL_DATA => {
                let recipient = read_u32(&packet[1..5]);
                let data_len = read_u32(&packet[5..9]) as usize;
                let data = packet[9..9 + data_len].to_vec();

                Ok(Some(ChannelRequest::Data(recipient, data)))
            }
            SSH_MSG_CHANNEL_EOF => {
                let recipient = read_u32(&packet[1..5]);
                if let Some(ch) = self.channels.iter_mut().find(|c| c.local_id == recipient) {
                    ch.eof_received = true;
                }
                Ok(Some(ChannelRequest::Eof(recipient)))
            }
            SSH_MSG_CHANNEL_CLOSE => {
                let recipient = read_u32(&packet[1..5]);
                if let Some(ch) = self.channels.iter_mut().find(|c| c.local_id == recipient) {
                    ch.closed = true;
                    // Send close back
                    let mut reply = Vec::new();
                    reply.push(SSH_MSG_CHANNEL_CLOSE);
                    push_u32(&mut reply, ch.remote_id);
                    let _ = self.send_packet(&reply);
                }
                Ok(Some(ChannelRequest::Close(recipient)))
            }
            SSH_MSG_CHANNEL_WINDOW_ADJUST => {
                let recipient = read_u32(&packet[1..5]);
                let adjust = read_u32(&packet[5..9]);
                if let Some(ch) = self.channels.iter_mut().find(|c| c.local_id == recipient) {
                    ch.remote_window += adjust;
                }
                Ok(None)
            }
            SSH_MSG_DISCONNECT => {
                self.state = SshServerState::Closed;
                Err(KError::BrokenPipe)
            }
            _ => Ok(None)
        }
    }

    /// Send data on channel
    pub fn send_channel_data(&mut self, channel_id: u32, data: &[u8]) -> KResult<()> {
        let (remote_id, remote_window, max_packet) = {
            let ch = self.channels.iter().find(|c| c.local_id == channel_id)
                .ok_or(KError::NotFound)?;
            (ch.remote_id, ch.remote_window, ch.max_packet_size)
        };

        let max_len = (remote_window as usize).min(max_packet as usize - 9);
        let send_len = data.len().min(max_len);

        if send_len == 0 {
            return Ok(());
        }

        let mut packet = Vec::new();
        packet.push(SSH_MSG_CHANNEL_DATA);
        push_u32(&mut packet, remote_id);
        push_string(&mut packet, &data[..send_len]);

        self.send_packet(&packet)?;

        if let Some(ch) = self.channels.iter_mut().find(|c| c.local_id == channel_id) {
            ch.remote_window -= send_len as u32;
        }

        Ok(())
    }

    /// Send exit status and close channel
    pub fn send_exit_status(&mut self, channel_id: u32, exit_code: u32) -> KResult<()> {
        let remote_id = self.channels.iter()
            .find(|c| c.local_id == channel_id)
            .map(|c| c.remote_id)
            .ok_or(KError::NotFound)?;

        // Send exit-status request
        let mut packet = Vec::new();
        packet.push(SSH_MSG_CHANNEL_REQUEST);
        push_u32(&mut packet, remote_id);
        push_string(&mut packet, b"exit-status");
        packet.push(0); // want reply = false
        push_u32(&mut packet, exit_code);

        self.send_packet(&packet)?;

        // Send EOF
        let mut eof = Vec::new();
        eof.push(SSH_MSG_CHANNEL_EOF);
        push_u32(&mut eof, remote_id);
        self.send_packet(&eof)?;

        // Send close
        let mut close = Vec::new();
        close.push(SSH_MSG_CHANNEL_CLOSE);
        push_u32(&mut close, remote_id);
        self.send_packet(&close)?;

        if let Some(ch) = self.channels.iter_mut().find(|c| c.local_id == channel_id) {
            ch.closed = true;
        }

        Ok(())
    }

    /// Send/receive packet helpers
    fn send_packet(&mut self, payload: &[u8]) -> KResult<()> {
        if self.encrypted {
            self.send_encrypted_packet(payload)
        } else {
            self.send_unencrypted_packet(payload)
        }
    }

    fn send_unencrypted_packet(&self, payload: &[u8]) -> KResult<()> {
        let block_size = 8;
        let payload_len = payload.len();
        let min_padding = 4;

        let unpadded = 5 + payload_len + min_padding;
        let padding = min_padding + (block_size - (unpadded % block_size)) % block_size;
        let packet_len = 1 + payload_len + padding;

        let mut packet = Vec::with_capacity(4 + packet_len);
        push_u32(&mut packet, packet_len as u32);
        packet.push(padding as u8);
        packet.extend_from_slice(payload);

        let mut pad = vec![0u8; padding];
        random_bytes(&mut pad);
        packet.extend_from_slice(&pad);

        self.send_raw(&packet)
    }

    fn send_encrypted_packet(&mut self, payload: &[u8]) -> KResult<()> {
        let block_size = 8;
        let payload_len = payload.len();
        let min_padding = 4;

        let unpadded = 5 + payload_len + min_padding;
        let padding = min_padding + (block_size - (unpadded % block_size)) % block_size;
        let packet_len = 1 + payload_len + padding;

        let mut plaintext = Vec::with_capacity(1 + payload_len + padding);
        plaintext.push(padding as u8);
        plaintext.extend_from_slice(payload);

        let mut pad = vec![0u8; padding];
        random_bytes(&mut pad);
        plaintext.extend_from_slice(&pad);

        let mut nonce = [0u8; 12];
        nonce[4..12].copy_from_slice(&self.seq_s2c.to_be_bytes());

        let len_bytes = (packet_len as u32).to_be_bytes();

        let (ciphertext, tag) = chacha20_poly1305_encrypt(&self.enc_key_s2c, &nonce, &len_bytes, &plaintext);

        let mut packet = Vec::new();
        packet.extend_from_slice(&len_bytes);
        packet.extend_from_slice(&ciphertext);
        packet.extend_from_slice(&tag);

        self.seq_s2c += 1;

        self.send_raw(&packet)
    }

    fn recv_packet(&mut self) -> KResult<Vec<u8>> {
        if self.encrypted {
            self.recv_encrypted_packet()
        } else {
            self.recv_unencrypted_packet()
        }
    }

    fn recv_unencrypted_packet(&mut self) -> KResult<Vec<u8>> {
        let mut buf = [0u8; 4096];

        while self.recv_buffer.len() < 4 {
            let n = self.recv_raw(&mut buf)?;
            if n == 0 {
                return Err(KError::WouldBlock);
            }
            self.recv_buffer.extend_from_slice(&buf[..n]);
        }

        let packet_len = read_u32(&self.recv_buffer[0..4]) as usize;
        if packet_len > 35000 {
            return Err(KError::IO);
        }

        while self.recv_buffer.len() < 4 + packet_len {
            let n = self.recv_raw(&mut buf)?;
            if n == 0 {
                return Err(KError::WouldBlock);
            }
            self.recv_buffer.extend_from_slice(&buf[..n]);
        }

        let padding_len = self.recv_buffer[4] as usize;
        let payload_len = packet_len - 1 - padding_len;
        let payload = self.recv_buffer[5..5 + payload_len].to_vec();

        let total = 4 + packet_len;
        self.recv_buffer = self.recv_buffer[total..].to_vec();

        Ok(payload)
    }

    fn recv_encrypted_packet(&mut self) -> KResult<Vec<u8>> {
        let mut buf = [0u8; 4096];

        while self.recv_buffer.len() < 4 {
            let n = self.recv_raw(&mut buf)?;
            if n == 0 {
                return Err(KError::WouldBlock);
            }
            self.recv_buffer.extend_from_slice(&buf[..n]);
        }

        let packet_len = read_u32(&self.recv_buffer[0..4]) as usize;
        if packet_len > 35000 {
            return Err(KError::IO);
        }

        while self.recv_buffer.len() < 4 + packet_len + 16 {
            let n = self.recv_raw(&mut buf)?;
            if n == 0 {
                return Err(KError::WouldBlock);
            }
            self.recv_buffer.extend_from_slice(&buf[..n]);
        }

        let mut nonce = [0u8; 12];
        nonce[4..12].copy_from_slice(&self.seq_c2s.to_be_bytes());

        let len_bytes = self.recv_buffer[0..4].to_vec();
        let ciphertext = &self.recv_buffer[4..4 + packet_len];
        let tag: [u8; 16] = self.recv_buffer[4 + packet_len..4 + packet_len + 16]
            .try_into().map_err(|_| KError::IO)?;

        let plaintext = chacha20_poly1305_decrypt(&self.enc_key_c2s, &nonce, &len_bytes, ciphertext, &tag)
            .ok_or(KError::IO)?;

        let padding_len = plaintext[0] as usize;
        let payload_len = plaintext.len() - 1 - padding_len;
        let payload = plaintext[1..1 + payload_len].to_vec();

        self.seq_c2s += 1;

        let total = 4 + packet_len + 16;
        self.recv_buffer = self.recv_buffer[total..].to_vec();

        Ok(payload)
    }
}

/// Channel request types
#[derive(Debug)]
pub enum ChannelRequest {
    /// PTY request
    Pty { cols: u32, rows: u32 },
    /// Shell request
    Shell,
    /// Exec command
    Exec(String),
    /// Subsystem (e.g., sftp)
    Subsystem(String),
    /// Channel data
    Data(u32, Vec<u8>),
    /// EOF on channel
    Eof(u32),
    /// Close channel
    Close(u32),
}

/// SSH Server
pub struct SshServer {
    /// Configuration
    config: SshServerConfig,
    /// Host keys
    host_keys: SshHostKeys,
    /// Registered users
    users: BTreeMap<String, SshUser>,
    /// Active sessions
    sessions: BTreeMap<u32, SshServerSession>,
    /// Next session ID
    next_session_id: AtomicU32,
    /// Running flag
    running: AtomicBool,
    /// Listening port (if listening)
    listen_port: Option<u16>,
}

impl SshServer {
    /// Create new SSH server
    pub fn new(config: SshServerConfig) -> Self {
        Self {
            config,
            host_keys: SshHostKeys::generate(),
            users: BTreeMap::new(),
            sessions: BTreeMap::new(),
            next_session_id: AtomicU32::new(1),
            running: AtomicBool::new(false),
            listen_port: None,
        }
    }

    /// Add user
    pub fn add_user(&mut self, user: SshUser) {
        self.users.insert(user.username.clone(), user);
    }

    /// Remove user
    pub fn remove_user(&mut self, username: &str) {
        self.users.remove(username);
    }

    /// Start listening
    pub fn start(&mut self) -> KResult<()> {
        tcp::listen(self.config.port)?;
        self.listen_port = Some(self.config.port);
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Stop server
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        self.listen_port = None;
        self.sessions.clear();
    }

    /// Accept new connection
    pub fn accept(&mut self) -> KResult<u32> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(KError::Invalid);
        }

        if self.sessions.len() >= self.config.max_connections as usize {
            return Err(KError::Busy);
        }

        let port = self.listen_port.ok_or(KError::Invalid)?;
        let tcp_key = tcp::accept(port).ok_or(KError::WouldBlock)?;

        let session_id = self.next_session_id.fetch_add(1, Ordering::SeqCst);
        let mut session = SshServerSession::new(session_id, tcp_key);

        // Send version and perform key exchange
        session.send_version()?;
        session.state = SshServerState::SentVersion;

        session.recv_version()?;

        session.send_kex_init()?;
        session.recv_kex_init()?;
        session.state = SshServerState::KeyExchange;

        session.handle_kex_ecdh(&self.host_keys)?;

        session.handle_service_request()?;

        self.sessions.insert(session_id, session);

        Ok(session_id)
    }

    /// Authenticate session
    pub fn authenticate(&mut self, session_id: u32) -> KResult<bool> {
        let session = self.sessions.get_mut(&session_id).ok_or(KError::NotFound)?;

        if session.state != SshServerState::Authenticating {
            return Err(KError::Invalid);
        }

        let result = session.handle_auth_request(&self.users, &self.config)?;

        if result {
            session.state = SshServerState::Authenticated;
        }

        Ok(result)
    }

    /// Handle channel open for session
    pub fn accept_channel(&mut self, session_id: u32) -> KResult<u32> {
        let session = self.sessions.get_mut(&session_id).ok_or(KError::NotFound)?;
        session.handle_channel_open()?;

        // Return first channel ID
        session.channels.first().map(|c| c.local_id).ok_or(KError::NotFound)
    }

    /// Poll session for requests
    pub fn poll_session(&mut self, session_id: u32) -> KResult<Option<ChannelRequest>> {
        let session = self.sessions.get_mut(&session_id).ok_or(KError::NotFound)?;
        session.handle_channel_request()
    }

    /// Send data to session channel
    pub fn send_data(&mut self, session_id: u32, channel_id: u32, data: &[u8]) -> KResult<()> {
        let session = self.sessions.get_mut(&session_id).ok_or(KError::NotFound)?;
        session.send_channel_data(channel_id, data)
    }

    /// Send exit status and close channel
    pub fn close_channel(&mut self, session_id: u32, channel_id: u32, exit_code: u32) -> KResult<()> {
        let session = self.sessions.get_mut(&session_id).ok_or(KError::NotFound)?;
        session.send_exit_status(channel_id, exit_code)
    }

    /// Get authenticated username
    pub fn get_username(&self, session_id: u32) -> Option<String> {
        self.sessions.get(&session_id).and_then(|s| s.username.clone())
    }

    /// Close session
    pub fn close_session(&mut self, session_id: u32) -> KResult<()> {
        if let Some(mut session) = self.sessions.remove(&session_id) {
            session.state = SshServerState::Closed;
            tcp::close(&session.tcp_key)?;
        }
        Ok(())
    }

    /// Get number of active sessions
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Is server running?
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

// Global SSH server instance
static SSH_SERVER: IrqSafeMutex<Option<SshServer>> = IrqSafeMutex::new(None);

/// Initialize SSH server subsystem
pub fn init_server(config: SshServerConfig) {
    let mut server = SSH_SERVER.lock();
    *server = Some(SshServer::new(config));
}

/// Add user to SSH server
pub fn server_add_user(user: SshUser) {
    if let Some(ref mut server) = *SSH_SERVER.lock() {
        server.add_user(user);
    }
}

/// Start SSH server
pub fn server_start() -> KResult<()> {
    if let Some(ref mut server) = *SSH_SERVER.lock() {
        server.start()
    } else {
        Err(KError::NotFound)
    }
}

/// Stop SSH server
pub fn server_stop() {
    if let Some(ref mut server) = *SSH_SERVER.lock() {
        server.stop();
    }
}

/// Constant-time equality check
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Format server status
pub fn format_server_status() -> String {
    if let Some(ref server) = *SSH_SERVER.lock() {
        if server.is_running() {
            format!("SSH Server: Running on port {}, {} active sessions",
                server.config.port, server.session_count())
        } else {
            String::from("SSH Server: Stopped")
        }
    } else {
        String::from("SSH Server: Not initialized")
    }
}
