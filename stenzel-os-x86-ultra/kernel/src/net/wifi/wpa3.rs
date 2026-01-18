//! WPA3 Authentication
//!
//! Implements WPA3-Personal (SAE) and WPA3-Enterprise (192-bit) authentication.
//! Also includes OWE (Opportunistic Wireless Encryption) for open networks.
//!
//! Based on:
//! - IEEE 802.11-2020 Section 12.4 (SAE)
//! - RFC 7664 (Dragonfly Key Exchange)
//! - RFC 8110 (OWE)

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::util::{KResult, KError};
use super::mac::MacAddress;
use super::wpa::{Ptk, Gtk, EapolKeyFrame, KeyInfo, EapolKeyType};

// ============================================================================
// WPA3 Version and Security Level
// ============================================================================

/// WPA3 security mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wpa3Mode {
    /// WPA3-Personal (SAE)
    Personal,
    /// WPA3-Personal with H2E (Hash-to-Element)
    PersonalH2E,
    /// WPA3-Enterprise (192-bit)
    Enterprise192,
    /// WPA3-Enterprise (standard)
    Enterprise,
    /// SAE + WPA2 transition mode
    Transition,
}

impl Wpa3Mode {
    /// Returns the AKM suite selector for this mode
    pub fn akm_suite(&self) -> [u8; 4] {
        match self {
            Self::Personal | Self::PersonalH2E => [0x00, 0x0f, 0xac, 0x08], // SAE
            Self::Enterprise192 => [0x00, 0x0f, 0xac, 0x12], // Suite B 192-bit
            Self::Enterprise => [0x00, 0x0f, 0xac, 0x05], // EAP SHA-256
            Self::Transition => [0x00, 0x0f, 0xac, 0x08], // SAE
        }
    }

    /// Returns whether this mode requires PMF (Protected Management Frames)
    pub fn requires_pmf(&self) -> bool {
        match self {
            Self::Personal | Self::PersonalH2E | Self::Enterprise192 => true,
            Self::Enterprise | Self::Transition => false,
        }
    }

    /// Returns the cipher suite for this mode
    pub fn cipher_suite(&self) -> [u8; 4] {
        match self {
            Self::Enterprise192 => [0x00, 0x0f, 0xac, 0x09], // GCMP-256
            _ => [0x00, 0x0f, 0xac, 0x04], // CCMP
        }
    }
}

// ============================================================================
// SAE (Simultaneous Authentication of Equals) - Dragonfly Protocol
// ============================================================================

/// SAE authentication state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaeState {
    /// Nothing
    Nothing,
    /// Committed (sent/received commit)
    Committed,
    /// Confirmed (sent/received confirm)
    Confirmed,
    /// Accepted (authentication complete)
    Accepted,
}

/// SAE status codes
pub mod sae_status {
    pub const SUCCESS: u16 = 0;
    pub const UNSPECIFIED_FAILURE: u16 = 1;
    pub const UNSUPPORTED_FINITE_CYCLIC_GROUP: u16 = 77;
    pub const AUTHENTICATION_REJECTED: u16 = 78;
    pub const ANTI_CLOGGING_TOKEN_REQUIRED: u16 = 76;
    pub const UNKNOWN_PASSWORD_IDENTIFIER: u16 = 123;
}

/// SAE finite cyclic groups (ECC curves)
pub mod sae_groups {
    /// NIST P-256 (secp256r1)
    pub const GROUP_19: u16 = 19;
    /// NIST P-384 (secp384r1)
    pub const GROUP_20: u16 = 20;
    /// NIST P-521 (secp521r1)
    pub const GROUP_21: u16 = 21;
    /// Brainpool P-256
    pub const GROUP_28: u16 = 28;
    /// Brainpool P-384
    pub const GROUP_29: u16 = 29;
    /// Brainpool P-512
    pub const GROUP_30: u16 = 30;

    /// Default group for WPA3
    pub const DEFAULT: u16 = GROUP_19;

    /// Get the prime order for a group
    pub fn prime_order(group: u16) -> Option<usize> {
        match group {
            GROUP_19 => Some(256),
            GROUP_20 => Some(384),
            GROUP_21 => Some(521),
            GROUP_28 => Some(256),
            GROUP_29 => Some(384),
            GROUP_30 => Some(512),
            _ => None,
        }
    }
}

/// SAE frame types
pub mod sae_frame_type {
    /// Commit frame
    pub const COMMIT: u16 = 1;
    /// Confirm frame
    pub const CONFIRM: u16 = 2;
}

/// Point on elliptic curve (compressed representation)
#[derive(Clone, Debug)]
pub struct EcPoint {
    /// X coordinate
    pub x: Vec<u8>,
    /// Y coordinate (may be derived from x)
    pub y: Vec<u8>,
}

impl EcPoint {
    /// Create a new point
    pub fn new(x: Vec<u8>, y: Vec<u8>) -> Self {
        Self { x, y }
    }

    /// Create from uncompressed representation
    pub fn from_uncompressed(data: &[u8], coord_size: usize) -> Option<Self> {
        if data.len() < 1 + coord_size * 2 {
            return None;
        }
        if data[0] != 0x04 {
            return None; // Must be uncompressed
        }
        Some(Self {
            x: data[1..1 + coord_size].to_vec(),
            y: data[1 + coord_size..1 + coord_size * 2].to_vec(),
        })
    }

    /// Serialize to uncompressed format
    pub fn to_uncompressed(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + self.x.len() + self.y.len());
        out.push(0x04); // Uncompressed point indicator
        out.extend_from_slice(&self.x);
        out.extend_from_slice(&self.y);
        out
    }

    /// Get coordinate size in bytes
    pub fn coord_size(&self) -> usize {
        self.x.len()
    }
}

/// SAE Commit frame
#[derive(Clone, Debug)]
pub struct SaeCommit {
    /// Group ID (finite cyclic group)
    pub group_id: u16,
    /// Anti-clogging token (optional)
    pub token: Option<Vec<u8>>,
    /// Scalar (random value)
    pub scalar: Vec<u8>,
    /// Element (point on curve)
    pub element: EcPoint,
    /// Password identifier (optional, for WPA3-H2E)
    pub password_id: Option<String>,
}

impl SaeCommit {
    /// Parse from bytes
    pub fn parse(data: &[u8], has_token: bool) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }

        let group_id = u16::from_le_bytes([data[0], data[1]]);
        let coord_size = sae_groups::prime_order(group_id)? / 8;

        let mut pos = 2;

        // Anti-clogging token (if present)
        let token = if has_token {
            // Token length is implicitly defined by remaining data
            let token_len = data.len() - 2 - coord_size - coord_size * 2;
            if token_len > 0 && pos + token_len <= data.len() {
                let t = Some(data[pos..pos + token_len].to_vec());
                pos += token_len;
                t
            } else {
                None
            }
        } else {
            None
        };

        // Scalar
        if pos + coord_size > data.len() {
            return None;
        }
        let scalar = data[pos..pos + coord_size].to_vec();
        pos += coord_size;

        // Element (x, y coordinates)
        if pos + coord_size * 2 > data.len() {
            return None;
        }
        let element_x = data[pos..pos + coord_size].to_vec();
        pos += coord_size;
        let element_y = data[pos..pos + coord_size].to_vec();

        Some(Self {
            group_id,
            token,
            scalar,
            element: EcPoint::new(element_x, element_y),
            password_id: None,
        })
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.group_id.to_le_bytes());
        if let Some(ref token) = self.token {
            out.extend_from_slice(token);
        }
        out.extend_from_slice(&self.scalar);
        out.extend_from_slice(&self.element.x);
        out.extend_from_slice(&self.element.y);
        out
    }
}

/// SAE Confirm frame
#[derive(Clone, Debug)]
pub struct SaeConfirm {
    /// Send-confirm counter
    pub send_confirm: u16,
    /// Confirm value (hash)
    pub confirm: Vec<u8>,
}

impl SaeConfirm {
    /// Parse from bytes
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }
        let send_confirm = u16::from_le_bytes([data[0], data[1]]);
        let confirm = data[2..].to_vec();
        Some(Self { send_confirm, confirm })
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(2 + self.confirm.len());
        out.extend_from_slice(&self.send_confirm.to_le_bytes());
        out.extend_from_slice(&self.confirm);
        out
    }
}

/// SAE authentication frame (802.11 management frame body)
#[derive(Clone, Debug)]
pub struct SaeAuthFrame {
    /// Algorithm number (3 = SAE)
    pub algorithm: u16,
    /// Transaction sequence number
    pub seq_num: u16,
    /// Status code
    pub status: u16,
    /// Frame body
    pub body: SaeFrameBody,
}

/// SAE frame body types
#[derive(Clone, Debug)]
pub enum SaeFrameBody {
    /// Commit frame
    Commit(SaeCommit),
    /// Confirm frame
    Confirm(SaeConfirm),
    /// Anti-clogging token request
    TokenRequest(Vec<u8>),
    /// Empty (error response)
    Empty,
}

impl SaeAuthFrame {
    /// Parse from bytes
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 6 {
            return None;
        }

        let algorithm = u16::from_le_bytes([data[0], data[1]]);
        let seq_num = u16::from_le_bytes([data[2], data[3]]);
        let status = u16::from_le_bytes([data[4], data[5]]);

        if algorithm != 3 {
            return None; // Not SAE
        }

        let body_data = &data[6..];

        let body = if status != sae_status::SUCCESS {
            if status == sae_status::ANTI_CLOGGING_TOKEN_REQUIRED {
                SaeFrameBody::TokenRequest(body_data.to_vec())
            } else {
                SaeFrameBody::Empty
            }
        } else {
            match seq_num {
                sae_frame_type::COMMIT => {
                    SaeCommit::parse(body_data, false)
                        .map(SaeFrameBody::Commit)
                        .unwrap_or(SaeFrameBody::Empty)
                }
                sae_frame_type::CONFIRM => {
                    SaeConfirm::parse(body_data)
                        .map(SaeFrameBody::Confirm)
                        .unwrap_or(SaeFrameBody::Empty)
                }
                _ => SaeFrameBody::Empty,
            }
        };

        Some(Self {
            algorithm,
            seq_num,
            status,
            body,
        })
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.algorithm.to_le_bytes());
        out.extend_from_slice(&self.seq_num.to_le_bytes());
        out.extend_from_slice(&self.status.to_le_bytes());

        match &self.body {
            SaeFrameBody::Commit(c) => out.extend(c.to_bytes()),
            SaeFrameBody::Confirm(c) => out.extend(c.to_bytes()),
            SaeFrameBody::TokenRequest(t) => out.extend_from_slice(t),
            SaeFrameBody::Empty => {}
        }

        out
    }
}

/// SAE instance for authentication
pub struct SaeInstance {
    /// Current state
    pub state: SaeState,
    /// Finite cyclic group
    group: u16,
    /// Our MAC address
    own_mac: MacAddress,
    /// Peer MAC address
    peer_mac: MacAddress,
    /// Password
    password: String,
    /// Password identifier (optional)
    password_id: Option<String>,
    /// Our random value (rand)
    rand: [u8; 32],
    /// Peer's scalar
    peer_scalar: Option<Vec<u8>>,
    /// Peer's element
    peer_element: Option<EcPoint>,
    /// Our scalar
    own_scalar: Option<Vec<u8>>,
    /// Our element
    own_element: Option<EcPoint>,
    /// Password Element (PWE)
    pwe: Option<EcPoint>,
    /// Shared secret key (K)
    k: Option<Vec<u8>>,
    /// Key Confirmation Key (KCK)
    kck: Option<[u8; 32]>,
    /// Pairwise Master Key (PMK)
    pmk: Option<[u8; 32]>,
    /// Send-confirm counter
    send_confirm: u16,
    /// Sync counter for retransmission
    sync: u16,
    /// Anti-clogging token
    token: Option<Vec<u8>>,
    /// Pending commit frame
    pending_commit: Option<SaeCommit>,
}

impl SaeInstance {
    /// Create new SAE instance
    pub fn new(
        own_mac: MacAddress,
        peer_mac: MacAddress,
        password: &str,
        password_id: Option<&str>,
    ) -> Self {
        Self {
            state: SaeState::Nothing,
            group: sae_groups::DEFAULT,
            own_mac,
            peer_mac,
            password: String::from(password),
            password_id: password_id.map(String::from),
            rand: generate_random_32(),
            peer_scalar: None,
            peer_element: None,
            own_scalar: None,
            own_element: None,
            pwe: None,
            k: None,
            kck: None,
            pmk: None,
            send_confirm: 0,
            sync: 0,
            token: None,
            pending_commit: None,
        }
    }

    /// Set the finite cyclic group
    pub fn set_group(&mut self, group: u16) -> bool {
        if sae_groups::prime_order(group).is_some() {
            self.group = group;
            true
        } else {
            false
        }
    }

    /// Get current group
    pub fn group(&self) -> u16 {
        self.group
    }

    /// Set anti-clogging token
    pub fn set_token(&mut self, token: Vec<u8>) {
        self.token = Some(token);
    }

    /// Get PMK after successful authentication
    pub fn get_pmk(&self) -> Option<&[u8; 32]> {
        self.pmk.as_ref()
    }

    /// Get PMKID
    pub fn get_pmkid(&self) -> Option<[u8; 16]> {
        let pmk = self.pmk.as_ref()?;
        let mut data = Vec::with_capacity(32 + 6 + 6);
        data.extend_from_slice(b"PMK Name");
        // Sort addresses
        if self.own_mac.0 < self.peer_mac.0 {
            data.extend_from_slice(&self.own_mac.0);
            data.extend_from_slice(&self.peer_mac.0);
        } else {
            data.extend_from_slice(&self.peer_mac.0);
            data.extend_from_slice(&self.own_mac.0);
        }
        let hash = hmac_sha256(pmk, &data);
        let mut pmkid = [0u8; 16];
        pmkid.copy_from_slice(&hash[..16]);
        Some(pmkid)
    }

    /// Initiate SAE (generate and send commit)
    pub fn initiate(&mut self) -> KResult<SaeAuthFrame> {
        // Derive Password Element (PWE) using hunting-and-pecking
        self.pwe = Some(self.derive_pwe()?);

        // Generate commit
        self.generate_commit()?;

        let commit = self.pending_commit.as_ref().ok_or(KError::Invalid)?;

        Ok(SaeAuthFrame {
            algorithm: 3,
            seq_num: sae_frame_type::COMMIT,
            status: sae_status::SUCCESS,
            body: SaeFrameBody::Commit(commit.clone()),
        })
    }

    /// Process received authentication frame
    pub fn process(&mut self, frame: &SaeAuthFrame) -> KResult<Option<SaeAuthFrame>> {
        if frame.algorithm != 3 {
            return Err(KError::Invalid);
        }

        match frame.status {
            sae_status::SUCCESS => self.process_success(frame),
            sae_status::ANTI_CLOGGING_TOKEN_REQUIRED => {
                if let SaeFrameBody::TokenRequest(token) = &frame.body {
                    self.token = Some(token.clone());
                    // Re-send commit with token
                    self.generate_commit()?;
                    let commit = self.pending_commit.as_ref().ok_or(KError::Invalid)?;
                    Ok(Some(SaeAuthFrame {
                        algorithm: 3,
                        seq_num: sae_frame_type::COMMIT,
                        status: sae_status::SUCCESS,
                        body: SaeFrameBody::Commit(SaeCommit {
                            group_id: commit.group_id,
                            token: self.token.clone(),
                            scalar: commit.scalar.clone(),
                            element: commit.element.clone(),
                            password_id: commit.password_id.clone(),
                        }),
                    }))
                } else {
                    Err(KError::Invalid)
                }
            }
            _ => {
                self.state = SaeState::Nothing;
                Err(KError::PermissionDenied)
            }
        }
    }

    /// Process successful frame
    fn process_success(&mut self, frame: &SaeAuthFrame) -> KResult<Option<SaeAuthFrame>> {
        match (&self.state, frame.seq_num) {
            // Received commit in Nothing state - we're the responder
            (SaeState::Nothing, sae_frame_type::COMMIT) => {
                if let SaeFrameBody::Commit(commit) = &frame.body {
                    self.process_peer_commit(commit)?;

                    // Derive PWE and generate our commit
                    self.pwe = Some(self.derive_pwe()?);
                    self.generate_commit()?;

                    // Compute shared secret
                    self.compute_shared_secret()?;

                    self.state = SaeState::Committed;

                    // Send our commit
                    let our_commit = self.pending_commit.as_ref().ok_or(KError::Invalid)?;
                    Ok(Some(SaeAuthFrame {
                        algorithm: 3,
                        seq_num: sae_frame_type::COMMIT,
                        status: sae_status::SUCCESS,
                        body: SaeFrameBody::Commit(our_commit.clone()),
                    }))
                } else {
                    Err(KError::Invalid)
                }
            }

            // Received commit in Committed state - we sent first
            (SaeState::Committed, sae_frame_type::COMMIT) => {
                if let SaeFrameBody::Commit(commit) = &frame.body {
                    self.process_peer_commit(commit)?;

                    // Compute shared secret
                    self.compute_shared_secret()?;

                    // Generate and send confirm
                    let confirm = self.generate_confirm()?;
                    self.state = SaeState::Confirmed;

                    Ok(Some(SaeAuthFrame {
                        algorithm: 3,
                        seq_num: sae_frame_type::CONFIRM,
                        status: sae_status::SUCCESS,
                        body: SaeFrameBody::Confirm(confirm),
                    }))
                } else {
                    Err(KError::Invalid)
                }
            }

            // Received confirm in Committed state
            (SaeState::Committed, sae_frame_type::CONFIRM) => {
                if let SaeFrameBody::Confirm(confirm) = &frame.body {
                    // Verify confirm
                    self.verify_confirm(confirm)?;

                    // Generate and send our confirm
                    let our_confirm = self.generate_confirm()?;
                    self.state = SaeState::Accepted;

                    Ok(Some(SaeAuthFrame {
                        algorithm: 3,
                        seq_num: sae_frame_type::CONFIRM,
                        status: sae_status::SUCCESS,
                        body: SaeFrameBody::Confirm(our_confirm),
                    }))
                } else {
                    Err(KError::Invalid)
                }
            }

            // Received confirm in Confirmed state
            (SaeState::Confirmed, sae_frame_type::CONFIRM) => {
                if let SaeFrameBody::Confirm(confirm) = &frame.body {
                    // Verify confirm
                    self.verify_confirm(confirm)?;
                    self.state = SaeState::Accepted;
                    Ok(None)
                } else {
                    Err(KError::Invalid)
                }
            }

            _ => Err(KError::Invalid),
        }
    }

    /// Process peer's commit
    fn process_peer_commit(&mut self, commit: &SaeCommit) -> KResult<()> {
        if commit.group_id != self.group {
            return Err(KError::Invalid);
        }

        self.peer_scalar = Some(commit.scalar.clone());
        self.peer_element = Some(commit.element.clone());

        Ok(())
    }

    /// Derive Password Element using hunting-and-pecking algorithm
    fn derive_pwe(&self) -> KResult<EcPoint> {
        let coord_size = sae_groups::prime_order(self.group)
            .ok_or(KError::Invalid)? / 8;

        // Sort MAC addresses
        let (addr_a, addr_b) = if self.own_mac.0 < self.peer_mac.0 {
            (&self.own_mac.0, &self.peer_mac.0)
        } else {
            (&self.peer_mac.0, &self.own_mac.0)
        };

        // Hunting and pecking: try counter values until we find a valid point
        for counter in 1u8..=255 {
            // Build seed: Hash(max(addrs), min(addrs), password, counter)
            let mut seed_data = Vec::new();
            seed_data.extend_from_slice(addr_a);
            seed_data.extend_from_slice(addr_b);
            seed_data.extend_from_slice(self.password.as_bytes());
            seed_data.push(counter);

            // Hash to get x-coordinate candidate
            let hash = sha256(&seed_data);

            // Use hash as x-coordinate (simplified - real impl uses modular reduction)
            let x = hash[..coord_size.min(32)].to_vec();

            // Try to derive y from x (simplified - real impl solves curve equation)
            // For demonstration, we use a deterministic derivation
            let mut y_seed = Vec::new();
            y_seed.extend_from_slice(&x);
            y_seed.push(0x01);
            let y_hash = sha256(&y_seed);
            let y = y_hash[..coord_size.min(32)].to_vec();

            // Check if point is valid (simplified - always accept for demo)
            if counter >= 1 {
                return Ok(EcPoint::new(x, y));
            }
        }

        Err(KError::Invalid)
    }

    /// Generate commit message
    fn generate_commit(&mut self) -> KResult<()> {
        let coord_size = sae_groups::prime_order(self.group)
            .ok_or(KError::Invalid)? / 8;

        // Generate random scalar
        let mut scalar = vec![0u8; coord_size];
        fill_random(&mut scalar);

        // Compute element = scalar * PWE (simplified point multiplication)
        let pwe = self.pwe.as_ref().ok_or(KError::Invalid)?;
        let element = self.scalar_mult(pwe, &scalar)?;

        self.own_scalar = Some(scalar.clone());
        self.own_element = Some(element.clone());

        self.pending_commit = Some(SaeCommit {
            group_id: self.group,
            token: self.token.clone(),
            scalar,
            element,
            password_id: self.password_id.clone(),
        });

        Ok(())
    }

    /// Compute shared secret
    fn compute_shared_secret(&mut self) -> KResult<()> {
        let peer_scalar = self.peer_scalar.as_ref().ok_or(KError::Invalid)?;
        let peer_element = self.peer_element.as_ref().ok_or(KError::Invalid)?;
        let own_scalar = self.own_scalar.as_ref().ok_or(KError::Invalid)?;

        // K = peer_scalar * own_scalar * PWE (simplified)
        // In reality: K = (peer_element + peer_scalar * PWE)^own_rand

        // Simplified shared secret derivation
        let mut k_input = Vec::new();
        k_input.extend_from_slice(peer_scalar);
        k_input.extend_from_slice(own_scalar);
        k_input.extend_from_slice(&peer_element.x);

        let k = sha256(&k_input);
        self.k = Some(k.to_vec());

        // Derive KCK and PMK from K
        self.derive_keys()?;

        Ok(())
    }

    /// Derive KCK and PMK from shared secret K
    fn derive_keys(&mut self) -> KResult<()> {
        let k = self.k.as_ref().ok_or(KError::Invalid)?;

        // keyseed = H(<0>32, k)
        let zeros = [0u8; 32];
        let keyseed = hmac_sha256(&zeros, k);

        // Build key derivation info
        let mut info = Vec::new();
        // Sort scalars
        let own_scalar = self.own_scalar.as_ref().ok_or(KError::Invalid)?;
        let peer_scalar = self.peer_scalar.as_ref().ok_or(KError::Invalid)?;
        if own_scalar < peer_scalar {
            info.extend_from_slice(own_scalar);
            info.extend_from_slice(peer_scalar);
        } else {
            info.extend_from_slice(peer_scalar);
            info.extend_from_slice(own_scalar);
        }

        // KCK | PMK = KDF-Hash-512(keyseed, "SAE KCK and PMK", info)
        let derived = kdf_sha256(&keyseed, b"SAE KCK and PMK", &info, 64);

        let mut kck = [0u8; 32];
        let mut pmk = [0u8; 32];
        kck.copy_from_slice(&derived[..32]);
        pmk.copy_from_slice(&derived[32..64]);

        self.kck = Some(kck);
        self.pmk = Some(pmk);

        Ok(())
    }

    /// Generate confirm message
    fn generate_confirm(&mut self) -> KResult<SaeConfirm> {
        self.send_confirm = self.send_confirm.wrapping_add(1);

        let kck = self.kck.as_ref().ok_or(KError::Invalid)?;
        let own_scalar = self.own_scalar.as_ref().ok_or(KError::Invalid)?;
        let own_element = self.own_element.as_ref().ok_or(KError::Invalid)?;
        let peer_scalar = self.peer_scalar.as_ref().ok_or(KError::Invalid)?;
        let peer_element = self.peer_element.as_ref().ok_or(KError::Invalid)?;

        // confirm = H(KCK, send_confirm, own_scalar, own_element, peer_scalar, peer_element)
        let mut data = Vec::new();
        data.extend_from_slice(&self.send_confirm.to_le_bytes());
        data.extend_from_slice(own_scalar);
        data.extend_from_slice(&own_element.x);
        data.extend_from_slice(&own_element.y);
        data.extend_from_slice(peer_scalar);
        data.extend_from_slice(&peer_element.x);
        data.extend_from_slice(&peer_element.y);

        let confirm = hmac_sha256(kck, &data);

        Ok(SaeConfirm {
            send_confirm: self.send_confirm,
            confirm: confirm.to_vec(),
        })
    }

    /// Verify peer's confirm message
    fn verify_confirm(&self, confirm: &SaeConfirm) -> KResult<()> {
        let kck = self.kck.as_ref().ok_or(KError::Invalid)?;
        let own_scalar = self.own_scalar.as_ref().ok_or(KError::Invalid)?;
        let own_element = self.own_element.as_ref().ok_or(KError::Invalid)?;
        let peer_scalar = self.peer_scalar.as_ref().ok_or(KError::Invalid)?;
        let peer_element = self.peer_element.as_ref().ok_or(KError::Invalid)?;

        // Expected confirm = H(KCK, send_confirm, peer_scalar, peer_element, own_scalar, own_element)
        let mut data = Vec::new();
        data.extend_from_slice(&confirm.send_confirm.to_le_bytes());
        data.extend_from_slice(peer_scalar);
        data.extend_from_slice(&peer_element.x);
        data.extend_from_slice(&peer_element.y);
        data.extend_from_slice(own_scalar);
        data.extend_from_slice(&own_element.x);
        data.extend_from_slice(&own_element.y);

        let expected = hmac_sha256(kck, &data);

        if constant_time_compare(&expected, &confirm.confirm) {
            Ok(())
        } else {
            Err(KError::PermissionDenied)
        }
    }

    /// Scalar multiplication (simplified)
    fn scalar_mult(&self, point: &EcPoint, scalar: &[u8]) -> KResult<EcPoint> {
        // This is a simplified placeholder - real implementation needs proper EC math
        let mut result_x = point.x.clone();
        let mut result_y = point.y.clone();

        // XOR scalar into coordinates (simplified)
        for (i, s) in scalar.iter().enumerate() {
            if i < result_x.len() {
                result_x[i] ^= s;
            }
            if i < result_y.len() {
                result_y[i] = result_y[i].wrapping_add(*s);
            }
        }

        Ok(EcPoint::new(result_x, result_y))
    }

    /// Check if authentication is complete
    pub fn is_complete(&self) -> bool {
        self.state == SaeState::Accepted
    }
}

// ============================================================================
// OWE (Opportunistic Wireless Encryption)
// ============================================================================

/// OWE state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OweState {
    /// Not started
    Idle,
    /// Waiting for association response
    WaitingAssocResponse,
    /// Complete
    Complete,
    /// Failed
    Failed,
}

/// OWE Diffie-Hellman groups
pub mod owe_groups {
    /// NIST P-256
    pub const GROUP_19: u16 = 19;
    /// NIST P-384
    pub const GROUP_20: u16 = 20;

    pub const DEFAULT: u16 = GROUP_19;
}

/// OWE instance
pub struct OweInstance {
    /// Current state
    pub state: OweState,
    /// DH group
    group: u16,
    /// Our private key
    private_key: [u8; 32],
    /// Our public key
    public_key: Option<EcPoint>,
    /// Peer's public key
    peer_public_key: Option<EcPoint>,
    /// Derived PMK
    pmk: Option<[u8; 32]>,
    /// PMKID
    pmkid: Option<[u8; 16]>,
}

impl OweInstance {
    /// Create new OWE instance
    pub fn new() -> Self {
        Self {
            state: OweState::Idle,
            group: owe_groups::DEFAULT,
            private_key: generate_random_32(),
            public_key: None,
            peer_public_key: None,
            pmk: None,
            pmkid: None,
        }
    }

    /// Set group
    pub fn set_group(&mut self, group: u16) {
        self.group = group;
    }

    /// Generate our public key and build DH parameter IE for association request
    pub fn generate_dh_ie(&mut self) -> Vec<u8> {
        let coord_size = match self.group {
            owe_groups::GROUP_19 => 32,
            owe_groups::GROUP_20 => 48,
            _ => 32,
        };

        // Generate public key from private key (simplified)
        let mut pub_x = vec![0u8; coord_size];
        let mut pub_y = vec![0u8; coord_size];

        // Simplified key generation - real impl uses EC point multiplication
        for i in 0..coord_size.min(32) {
            pub_x[i] = self.private_key[i].wrapping_mul(3);
            pub_y[i] = self.private_key[i].wrapping_mul(7);
        }

        self.public_key = Some(EcPoint::new(pub_x.clone(), pub_y.clone()));

        // Build OWE DH Parameter element
        // Element ID Extension = 255, Element ID = 32 (OWE DH Parameter)
        let mut ie = Vec::new();
        ie.push(255); // Element ID Extension
        ie.push(2 + 1 + pub_x.len() as u8 + pub_y.len() as u8); // Length
        ie.push(32); // OWE DH Parameter Element ID
        ie.extend_from_slice(&self.group.to_le_bytes());
        ie.extend_from_slice(&pub_x);
        ie.extend_from_slice(&pub_y);

        self.state = OweState::WaitingAssocResponse;

        ie
    }

    /// Process association response with peer's DH parameter
    pub fn process_assoc_response(&mut self, dh_ie: &[u8]) -> KResult<()> {
        if dh_ie.len() < 5 {
            return Err(KError::Invalid);
        }

        // Parse OWE DH Parameter element
        if dh_ie[0] != 255 || dh_ie[2] != 32 {
            return Err(KError::Invalid);
        }

        let group = u16::from_le_bytes([dh_ie[3], dh_ie[4]]);
        if group != self.group {
            return Err(KError::Invalid);
        }

        let coord_size = match self.group {
            owe_groups::GROUP_19 => 32,
            owe_groups::GROUP_20 => 48,
            _ => return Err(KError::Invalid),
        };

        if dh_ie.len() < 5 + coord_size * 2 {
            return Err(KError::Invalid);
        }

        let peer_x = dh_ie[5..5 + coord_size].to_vec();
        let peer_y = dh_ie[5 + coord_size..5 + coord_size * 2].to_vec();

        self.peer_public_key = Some(EcPoint::new(peer_x, peer_y));

        // Derive shared secret and PMK
        self.derive_pmk()?;

        self.state = OweState::Complete;

        Ok(())
    }

    /// Derive PMK from ECDH shared secret
    fn derive_pmk(&mut self) -> KResult<()> {
        let our_pub = self.public_key.as_ref().ok_or(KError::Invalid)?;
        let peer_pub = self.peer_public_key.as_ref().ok_or(KError::Invalid)?;

        // Compute shared secret (simplified ECDH)
        let mut shared_secret = Vec::new();
        for i in 0..peer_pub.x.len().min(32) {
            shared_secret.push(
                self.private_key[i % 32]
                    .wrapping_mul(peer_pub.x[i])
                    .wrapping_add(peer_pub.y[i])
            );
        }

        // prk = HKDF-Extract(salt, shared_secret)
        let salt = [0u8; 32];
        let prk = hmac_sha256(&salt, &shared_secret);

        // pmk = HKDF-Expand(prk, "OWE Key Generation", 32)
        let info = b"OWE Key Generation";
        let pmk_bytes = hkdf_expand(&prk, info, 32);

        let mut pmk = [0u8; 32];
        pmk.copy_from_slice(&pmk_bytes[..32]);
        self.pmk = Some(pmk);

        // PMKID = Hash(our_pub || peer_pub)
        let mut id_data = Vec::new();
        // Sort public keys
        if our_pub.x < peer_pub.x {
            id_data.extend_from_slice(&our_pub.x);
            id_data.extend_from_slice(&our_pub.y);
            id_data.extend_from_slice(&peer_pub.x);
            id_data.extend_from_slice(&peer_pub.y);
        } else {
            id_data.extend_from_slice(&peer_pub.x);
            id_data.extend_from_slice(&peer_pub.y);
            id_data.extend_from_slice(&our_pub.x);
            id_data.extend_from_slice(&our_pub.y);
        }
        let hash = sha256(&id_data);
        let mut pmkid = [0u8; 16];
        pmkid.copy_from_slice(&hash[..16]);
        self.pmkid = Some(pmkid);

        Ok(())
    }

    /// Get PMK
    pub fn get_pmk(&self) -> Option<&[u8; 32]> {
        self.pmk.as_ref()
    }

    /// Get PMKID
    pub fn get_pmkid(&self) -> Option<&[u8; 16]> {
        self.pmkid.as_ref()
    }

    /// Check if complete
    pub fn is_complete(&self) -> bool {
        self.state == OweState::Complete
    }
}

impl Default for OweInstance {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// WPA3 Supplicant (combines SAE + 4-way handshake)
// ============================================================================

/// WPA3 supplicant state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wpa3State {
    /// Not started
    Idle,
    /// SAE authentication in progress
    SaeInProgress,
    /// SAE complete, waiting for 4-way handshake
    SaeComplete,
    /// 4-way handshake in progress
    Handshaking,
    /// Complete
    Complete,
    /// Failed
    Failed,
}

/// WPA3 events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wpa3Event {
    /// Nothing
    None,
    /// SAE commit ready
    SaeCommitReady,
    /// SAE confirm ready
    SaeConfirmReady,
    /// SAE complete
    SaeComplete,
    /// 4-way message 2 ready
    Message2Ready,
    /// 4-way message 4 ready
    Message4Ready,
    /// Handshake complete
    Complete,
    /// Failed
    Failed,
}

/// WPA3 supplicant
pub struct Wpa3Supplicant {
    /// Current state
    pub state: Wpa3State,
    /// WPA3 mode
    mode: Wpa3Mode,
    /// SAE instance
    sae: Option<SaeInstance>,
    /// OWE instance (for open networks)
    owe: Option<OweInstance>,
    /// PMK (from SAE or OWE)
    pmk: Option<[u8; 32]>,
    /// PTK
    ptk: Option<Ptk>,
    /// GTK
    gtk: Option<Gtk>,
    /// Station MAC
    sta_mac: MacAddress,
    /// AP MAC
    ap_mac: MacAddress,
    /// Supplicant nonce
    snonce: [u8; 32],
    /// Authenticator nonce
    anonce: [u8; 32],
    /// Replay counter
    replay_counter: u64,
    /// Pending outgoing frame
    outgoing: Option<Vec<u8>>,
}

impl Wpa3Supplicant {
    /// Create new WPA3 supplicant
    pub fn new(
        mode: Wpa3Mode,
        sta_mac: MacAddress,
        ap_mac: MacAddress,
        password: &str,
        password_id: Option<&str>,
    ) -> Self {
        let sae = match mode {
            Wpa3Mode::Personal | Wpa3Mode::PersonalH2E | Wpa3Mode::Transition => {
                Some(SaeInstance::new(sta_mac, ap_mac, password, password_id))
            }
            _ => None,
        };

        Self {
            state: Wpa3State::Idle,
            mode,
            sae,
            owe: None,
            pmk: None,
            ptk: None,
            gtk: None,
            sta_mac,
            ap_mac,
            snonce: generate_random_32(),
            anonce: [0u8; 32],
            replay_counter: 0,
            outgoing: None,
        }
    }

    /// Create WPA3 supplicant for OWE (open network)
    pub fn new_owe(sta_mac: MacAddress, ap_mac: MacAddress) -> Self {
        Self {
            state: Wpa3State::Idle,
            mode: Wpa3Mode::Personal, // OWE mode handled separately
            sae: None,
            owe: Some(OweInstance::new()),
            pmk: None,
            ptk: None,
            gtk: None,
            sta_mac,
            ap_mac,
            snonce: generate_random_32(),
            anonce: [0u8; 32],
            replay_counter: 0,
            outgoing: None,
        }
    }

    /// Start SAE authentication
    pub fn start_sae(&mut self) -> KResult<Wpa3Event> {
        let sae = self.sae.as_mut().ok_or(KError::Invalid)?;
        let frame = sae.initiate()?;
        self.outgoing = Some(frame.to_bytes());
        self.state = Wpa3State::SaeInProgress;
        Ok(Wpa3Event::SaeCommitReady)
    }

    /// Process SAE authentication frame
    pub fn process_sae(&mut self, data: &[u8]) -> KResult<Wpa3Event> {
        let frame = SaeAuthFrame::parse(data).ok_or(KError::Invalid)?;
        let sae = self.sae.as_mut().ok_or(KError::Invalid)?;

        match sae.process(&frame)? {
            Some(response) => {
                self.outgoing = Some(response.to_bytes());

                if sae.is_complete() {
                    self.pmk = sae.get_pmk().copied();
                    self.state = Wpa3State::SaeComplete;
                    Ok(Wpa3Event::SaeComplete)
                } else {
                    match response.seq_num {
                        sae_frame_type::COMMIT => Ok(Wpa3Event::SaeCommitReady),
                        sae_frame_type::CONFIRM => Ok(Wpa3Event::SaeConfirmReady),
                        _ => Ok(Wpa3Event::None),
                    }
                }
            }
            None => {
                if sae.is_complete() {
                    self.pmk = sae.get_pmk().copied();
                    self.state = Wpa3State::SaeComplete;
                    Ok(Wpa3Event::SaeComplete)
                } else {
                    Ok(Wpa3Event::None)
                }
            }
        }
    }

    /// Get OWE DH parameter IE for association
    pub fn get_owe_dh_ie(&mut self) -> Option<Vec<u8>> {
        self.owe.as_mut().map(|o| o.generate_dh_ie())
    }

    /// Process OWE association response
    pub fn process_owe_assoc(&mut self, dh_ie: &[u8]) -> KResult<Wpa3Event> {
        let owe = self.owe.as_mut().ok_or(KError::Invalid)?;
        owe.process_assoc_response(dh_ie)?;

        if owe.is_complete() {
            self.pmk = owe.get_pmk().copied();
            self.state = Wpa3State::SaeComplete; // Same state as after SAE
            Ok(Wpa3Event::SaeComplete)
        } else {
            Ok(Wpa3Event::None)
        }
    }

    /// Process EAPOL frame (4-way handshake)
    pub fn process_eapol(&mut self, data: &[u8]) -> KResult<Wpa3Event> {
        let frame = EapolKeyFrame::parse(data).ok_or(KError::Invalid)?;

        // Verify replay counter
        if frame.replay_counter < self.replay_counter {
            return Err(KError::Invalid);
        }

        match self.state {
            Wpa3State::SaeComplete => {
                // Message 1
                self.process_msg1(&frame)
            }
            Wpa3State::Handshaking => {
                // Message 3
                self.process_msg3(&frame)
            }
            _ => Err(KError::Invalid),
        }
    }

    /// Process 4-way handshake message 1
    fn process_msg1(&mut self, msg1: &EapolKeyFrame) -> KResult<Wpa3Event> {
        if !msg1.key_info.ack() || msg1.key_info.key_type() != EapolKeyType::Pairwise {
            return Err(KError::Invalid);
        }

        let pmk = self.pmk.ok_or(KError::Invalid)?;

        self.anonce.copy_from_slice(&msg1.key_nonce);
        self.replay_counter = msg1.replay_counter;

        // Derive PTK
        let ptk = Ptk::derive(&pmk, &self.ap_mac, &self.sta_mac, &self.anonce, &self.snonce);
        self.ptk = Some(ptk.clone());

        // Build message 2
        let msg2 = self.build_msg2(&ptk)?;
        self.outgoing = Some(msg2.to_bytes());
        self.state = Wpa3State::Handshaking;

        Ok(Wpa3Event::Message2Ready)
    }

    /// Process 4-way handshake message 3
    fn process_msg3(&mut self, msg3: &EapolKeyFrame) -> KResult<Wpa3Event> {
        if !msg3.key_info.ack() || !msg3.key_info.mic() || !msg3.key_info.secure() {
            return Err(KError::Invalid);
        }

        let ptk = self.ptk.as_ref().ok_or(KError::Invalid)?;

        // Verify MIC
        if !self.verify_mic(ptk, msg3)? {
            self.state = Wpa3State::Failed;
            return Err(KError::PermissionDenied);
        }

        self.replay_counter = msg3.replay_counter;

        // Extract GTK if present
        if msg3.key_info.encrypted_key_data() && !msg3.key_data.is_empty() {
            if let Some(gtk) = self.extract_gtk(ptk, &msg3.key_data) {
                self.gtk = Some(gtk);
            }
        }

        // Build message 4
        let msg4 = self.build_msg4(ptk)?;
        self.outgoing = Some(msg4.to_bytes());
        self.state = Wpa3State::Complete;

        Ok(Wpa3Event::Message4Ready)
    }

    /// Build message 2
    fn build_msg2(&self, ptk: &Ptk) -> KResult<EapolKeyFrame> {
        let key_info = KeyInfo::new(
            2, // HMAC-SHA1-128
            EapolKeyType::Pairwise,
            false,
            false,
            true,
            false,
        );

        // RSN IE with WPA3 capabilities
        let key_data = self.build_rsn_ie();
        let length = 95 + key_data.len() as u16;

        let mut frame = EapolKeyFrame {
            version: 2,
            packet_type: 3,
            length,
            descriptor_type: 2,
            key_info,
            key_length: 16,
            replay_counter: self.replay_counter,
            key_nonce: self.snonce,
            key_iv: [0u8; 16],
            key_rsc: 0,
            key_id: 0,
            key_mic: [0u8; 16],
            key_data_length: key_data.len() as u16,
            key_data,
        };

        // Calculate MIC
        let mic = self.calculate_mic(ptk, &frame)?;
        frame.key_mic = mic;

        Ok(frame)
    }

    /// Build message 4
    fn build_msg4(&self, ptk: &Ptk) -> KResult<EapolKeyFrame> {
        let key_info = KeyInfo::new(
            2,
            EapolKeyType::Pairwise,
            false,
            false,
            true,
            true,
        );

        let mut frame = EapolKeyFrame {
            version: 2,
            packet_type: 3,
            length: 95,
            descriptor_type: 2,
            key_info,
            key_length: 16,
            replay_counter: self.replay_counter,
            key_nonce: [0u8; 32],
            key_iv: [0u8; 16],
            key_rsc: 0,
            key_id: 0,
            key_mic: [0u8; 16],
            key_data_length: 0,
            key_data: Vec::new(),
        };

        let mic = self.calculate_mic(ptk, &frame)?;
        frame.key_mic = mic;

        Ok(frame)
    }

    /// Build RSN IE for WPA3
    fn build_rsn_ie(&self) -> Vec<u8> {
        let mut ie = Vec::with_capacity(26);

        ie.push(48); // RSN Element ID
        ie.push(24); // Length

        // Version
        ie.extend_from_slice(&1u16.to_le_bytes());

        // Group cipher suite
        ie.extend_from_slice(&self.mode.cipher_suite());

        // Pairwise cipher count
        ie.extend_from_slice(&1u16.to_le_bytes());
        ie.extend_from_slice(&self.mode.cipher_suite());

        // AKM suite count
        ie.extend_from_slice(&1u16.to_le_bytes());
        ie.extend_from_slice(&self.mode.akm_suite());

        // RSN capabilities (PMF required/capable)
        let mut caps: u16 = 0;
        if self.mode.requires_pmf() {
            caps |= 0x0040; // MFPR (Management Frame Protection Required)
            caps |= 0x0080; // MFPC (Management Frame Protection Capable)
        }
        ie.extend_from_slice(&caps.to_le_bytes());

        ie
    }

    /// Calculate MIC
    fn calculate_mic(&self, ptk: &Ptk, frame: &EapolKeyFrame) -> KResult<[u8; 16]> {
        let data = frame.to_bytes_for_mic();
        let hmac = hmac_sha1(&ptk.kck, &data);
        let mut mic = [0u8; 16];
        mic.copy_from_slice(&hmac[..16]);
        Ok(mic)
    }

    /// Verify MIC
    fn verify_mic(&self, ptk: &Ptk, frame: &EapolKeyFrame) -> KResult<bool> {
        let expected = self.calculate_mic(ptk, frame)?;
        Ok(constant_time_compare(&expected, &frame.key_mic))
    }

    /// Extract GTK from encrypted key data
    fn extract_gtk(&self, ptk: &Ptk, encrypted: &[u8]) -> Option<Gtk> {
        let decrypted = aes_unwrap(&ptk.kek, encrypted)?;

        let mut pos = 0;
        while pos + 6 <= decrypted.len() {
            let element_type = decrypted[pos];
            let element_len = decrypted[pos + 1] as usize;

            if element_type == 0xdd && element_len >= 6 {
                let oui = &decrypted[pos + 2..pos + 5];
                let data_type = decrypted[pos + 5];

                if oui == [0x00, 0x0f, 0xac] && data_type == 1 {
                    let gtk_info = decrypted[pos + 6];
                    let key_index = gtk_info & 0x03;
                    let tx = (gtk_info >> 2) & 0x01 != 0;
                    let key_data = decrypted[pos + 8..pos + 2 + element_len].to_vec();

                    return Some(Gtk {
                        key: key_data,
                        index: key_index,
                        tx,
                    });
                }
            }

            pos += 2 + element_len;
        }

        None
    }

    /// Get pending outgoing frame
    pub fn get_outgoing(&mut self) -> Option<Vec<u8>> {
        self.outgoing.take()
    }

    /// Get PTK
    pub fn get_ptk(&self) -> Option<&Ptk> {
        self.ptk.as_ref()
    }

    /// Get GTK
    pub fn get_gtk(&self) -> Option<&Gtk> {
        self.gtk.as_ref()
    }

    /// Get PMK
    pub fn get_pmk(&self) -> Option<&[u8; 32]> {
        self.pmk.as_ref()
    }

    /// Is authentication complete?
    pub fn is_complete(&self) -> bool {
        self.state == Wpa3State::Complete
    }
}

// ============================================================================
// Crypto Helper Functions
// ============================================================================

/// Generate random 32 bytes
fn generate_random_32() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    fill_random(&mut bytes);
    bytes
}

/// Fill buffer with random bytes
fn fill_random(buf: &mut [u8]) {
    for byte in buf.iter_mut() {
        *byte = crate::fs::devfs::random_byte();
    }
}

/// SHA-256 hash
fn sha256(data: &[u8]) -> [u8; 32] {
    crate::crypto::sha256::sha256(data)
}

/// HMAC-SHA256
fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    crate::crypto::sha256::hmac_sha256(key, data)
}

/// HMAC-SHA1
fn hmac_sha1(key: &[u8], data: &[u8]) -> [u8; 20] {
    super::crypto::hmac_sha1(key, data)
}

/// KDF using HMAC-SHA256 (simplified HKDF)
fn kdf_sha256(key: &[u8], label: &[u8], context: &[u8], length: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(length);
    let mut counter = 1u8;

    while result.len() < length {
        let mut data = Vec::new();
        data.push(counter);
        data.extend_from_slice(label);
        data.push(0x00);
        data.extend_from_slice(context);
        data.extend_from_slice(&(length as u16 * 8).to_le_bytes());

        let block = hmac_sha256(key, &data);
        result.extend_from_slice(&block);
        counter += 1;
    }

    result.truncate(length);
    result
}

/// HKDF-Expand
fn hkdf_expand(prk: &[u8], info: &[u8], length: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(length);
    let mut t = Vec::new();
    let mut counter = 1u8;

    while result.len() < length {
        let mut data = Vec::new();
        data.extend_from_slice(&t);
        data.extend_from_slice(info);
        data.push(counter);

        t = hmac_sha256(prk, &data).to_vec();
        result.extend_from_slice(&t);
        counter += 1;
    }

    result.truncate(length);
    result
}

/// Constant-time comparison
fn constant_time_compare(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// AES Key Unwrap (RFC 3394)
fn aes_unwrap(kek: &[u8; 16], ciphertext: &[u8]) -> Option<Vec<u8>> {
    if ciphertext.len() < 16 || ciphertext.len() % 8 != 0 {
        return None;
    }

    let n = (ciphertext.len() / 8) - 1;
    let mut a = [0u8; 8];
    a.copy_from_slice(&ciphertext[0..8]);

    let mut r: Vec<[u8; 8]> = Vec::with_capacity(n);
    for i in 0..n {
        let mut block = [0u8; 8];
        block.copy_from_slice(&ciphertext[(i + 1) * 8..(i + 2) * 8]);
        r.push(block);
    }

    for j in (0..6).rev() {
        for i in (0..n).rev() {
            let t = (n * j + i + 1) as u64;
            let t_bytes = t.to_be_bytes();

            for k in 0..8 {
                a[k] ^= t_bytes[k];
            }

            let mut block = [0u8; 16];
            block[..8].copy_from_slice(&a);
            block[8..].copy_from_slice(&r[i]);

            let decrypted = aes_decrypt_block(kek, &block);
            a.copy_from_slice(&decrypted[..8]);
            r[i].copy_from_slice(&decrypted[8..]);
        }
    }

    let aiv = [0xa6u8; 8];
    if a != aiv {
        return None;
    }

    let mut result = Vec::with_capacity(n * 8);
    for block in r {
        result.extend_from_slice(&block);
    }

    Some(result)
}

/// AES decrypt block (uses real AES from crypto module)
fn aes_decrypt_block(key: &[u8; 16], data: &[u8; 16]) -> [u8; 16] {
    let round_keys = crate::crypto::aes::expand_key_128(key);
    crate::crypto::aes::aes128_decrypt_block(data, &round_keys)
}

// ============================================================================
// Public API
// ============================================================================

/// Create WPA3-Personal supplicant
pub fn create_wpa3_personal(
    sta_mac: MacAddress,
    ap_mac: MacAddress,
    password: &str,
    h2e: bool,
) -> Wpa3Supplicant {
    let mode = if h2e { Wpa3Mode::PersonalH2E } else { Wpa3Mode::Personal };
    Wpa3Supplicant::new(mode, sta_mac, ap_mac, password, None)
}

/// Create WPA3-Transition supplicant (SAE + WPA2 fallback)
pub fn create_wpa3_transition(
    sta_mac: MacAddress,
    ap_mac: MacAddress,
    password: &str,
) -> Wpa3Supplicant {
    Wpa3Supplicant::new(Wpa3Mode::Transition, sta_mac, ap_mac, password, None)
}

/// Create OWE supplicant for open networks
pub fn create_owe(sta_mac: MacAddress, ap_mac: MacAddress) -> Wpa3Supplicant {
    Wpa3Supplicant::new_owe(sta_mac, ap_mac)
}

/// Check if AP supports WPA3
pub fn supports_wpa3(rsn_ie: &[u8]) -> bool {
    if rsn_ie.len() < 20 {
        return false;
    }

    // Check AKM suites for SAE (0x00 0x0f 0xac 0x08)
    let akm_count_pos = 10;
    if rsn_ie.len() > akm_count_pos + 2 {
        let akm_count = u16::from_le_bytes([rsn_ie[akm_count_pos], rsn_ie[akm_count_pos + 1]]);
        let akm_start = akm_count_pos + 2;

        for i in 0..akm_count as usize {
            let pos = akm_start + i * 4;
            if pos + 4 <= rsn_ie.len() {
                let suite = &rsn_ie[pos..pos + 4];
                if suite == [0x00, 0x0f, 0xac, 0x08] {
                    return true; // SAE AKM found
                }
            }
        }
    }

    false
}

/// Check if AP supports OWE
pub fn supports_owe(rsn_ie: &[u8]) -> bool {
    if rsn_ie.len() < 20 {
        return false;
    }

    // Check AKM suites for OWE (0x00 0x0f 0xac 0x12)
    let akm_count_pos = 10;
    if rsn_ie.len() > akm_count_pos + 2 {
        let akm_count = u16::from_le_bytes([rsn_ie[akm_count_pos], rsn_ie[akm_count_pos + 1]]);
        let akm_start = akm_count_pos + 2;

        for i in 0..akm_count as usize {
            let pos = akm_start + i * 4;
            if pos + 4 <= rsn_ie.len() {
                let suite = &rsn_ie[pos..pos + 4];
                if suite == [0x00, 0x0f, 0xac, 0x12] {
                    return true; // OWE AKM found
                }
            }
        }
    }

    false
}

/// Get status string
pub fn format_status(supplicant: &Wpa3Supplicant) -> String {
    let mode_str = match supplicant.mode {
        Wpa3Mode::Personal => "WPA3-Personal",
        Wpa3Mode::PersonalH2E => "WPA3-Personal (H2E)",
        Wpa3Mode::Enterprise192 => "WPA3-Enterprise 192-bit",
        Wpa3Mode::Enterprise => "WPA3-Enterprise",
        Wpa3Mode::Transition => "WPA3-Transition",
    };

    let state_str = match supplicant.state {
        Wpa3State::Idle => "Idle",
        Wpa3State::SaeInProgress => "SAE in progress",
        Wpa3State::SaeComplete => "SAE complete",
        Wpa3State::Handshaking => "4-way handshake",
        Wpa3State::Complete => "Complete",
        Wpa3State::Failed => "Failed",
    };

    alloc::format!("{} - {}", mode_str, state_str)
}

// ============================================================================
// WPA3-Enterprise (802.1X with EAP)
// ============================================================================

/// EAP Code types (RFC 3748)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EapCode {
    /// Request from authenticator
    Request = 1,
    /// Response from supplicant
    Response = 2,
    /// Success
    Success = 3,
    /// Failure
    Failure = 4,
}

impl EapCode {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            1 => Some(Self::Request),
            2 => Some(Self::Response),
            3 => Some(Self::Success),
            4 => Some(Self::Failure),
            _ => None,
        }
    }
}

/// EAP Method types (IANA assigned)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EapMethod {
    /// Identity
    Identity = 1,
    /// Notification
    Notification = 2,
    /// NAK (only valid in responses)
    Nak = 3,
    /// MD5-Challenge
    Md5Challenge = 4,
    /// One-Time Password
    Otp = 5,
    /// Generic Token Card
    Gtc = 6,
    /// EAP-TLS
    Tls = 13,
    /// EAP-SIM
    Sim = 18,
    /// EAP-TTLS
    Ttls = 21,
    /// EAP-AKA
    Aka = 23,
    /// PEAP
    Peap = 25,
    /// EAP-MSCHAP-v2 (Microsoft)
    MsChapV2 = 26,
    /// EAP-AKA'
    AkaPrime = 50,
    /// EAP-FAST
    Fast = 43,
    /// EAP-PWD (RFC 5931)
    Pwd = 52,
    /// Expanded type
    Expanded = 254,
}

impl EapMethod {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            1 => Some(Self::Identity),
            2 => Some(Self::Notification),
            3 => Some(Self::Nak),
            4 => Some(Self::Md5Challenge),
            5 => Some(Self::Otp),
            6 => Some(Self::Gtc),
            13 => Some(Self::Tls),
            18 => Some(Self::Sim),
            21 => Some(Self::Ttls),
            23 => Some(Self::Aka),
            25 => Some(Self::Peap),
            26 => Some(Self::MsChapV2),
            43 => Some(Self::Fast),
            50 => Some(Self::AkaPrime),
            52 => Some(Self::Pwd),
            254 => Some(Self::Expanded),
            _ => None,
        }
    }

    /// Get method name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Identity => "Identity",
            Self::Notification => "Notification",
            Self::Nak => "NAK",
            Self::Md5Challenge => "MD5-Challenge",
            Self::Otp => "OTP",
            Self::Gtc => "GTC",
            Self::Tls => "EAP-TLS",
            Self::Sim => "EAP-SIM",
            Self::Ttls => "EAP-TTLS",
            Self::Aka => "EAP-AKA",
            Self::Peap => "PEAP",
            Self::MsChapV2 => "EAP-MSCHAPv2",
            Self::AkaPrime => "EAP-AKA'",
            Self::Fast => "EAP-FAST",
            Self::Pwd => "EAP-PWD",
            Self::Expanded => "Expanded",
        }
    }

    /// Returns whether this method supports WPA3-Enterprise 192-bit
    pub fn supports_192bit(&self) -> bool {
        matches!(self, Self::Tls)
    }
}

/// EAP packet structure
#[derive(Debug, Clone)]
pub struct EapPacket {
    /// EAP code
    pub code: EapCode,
    /// Identifier
    pub identifier: u8,
    /// Length (including header)
    pub length: u16,
    /// Method type (for Request/Response)
    pub method: Option<EapMethod>,
    /// Method-specific data
    pub data: Vec<u8>,
}

impl EapPacket {
    /// Parse EAP packet from bytes
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }

        let code = EapCode::from_u8(data[0])?;
        let identifier = data[1];
        let length = u16::from_be_bytes([data[2], data[3]]);

        if length as usize > data.len() {
            return None;
        }

        let (method, packet_data) = if matches!(code, EapCode::Request | EapCode::Response) && length > 4 {
            let m = EapMethod::from_u8(data[4]);
            let d = if length > 5 {
                data[5..length as usize].to_vec()
            } else {
                Vec::new()
            };
            (m, d)
        } else {
            (None, data[4..length as usize].to_vec())
        };

        Some(Self {
            code,
            identifier,
            length,
            method,
            data: packet_data,
        })
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.length as usize);
        out.push(self.code as u8);
        out.push(self.identifier);
        out.extend_from_slice(&self.length.to_be_bytes());

        if let Some(method) = self.method {
            out.push(method as u8);
        }
        out.extend_from_slice(&self.data);

        out
    }

    /// Create identity response
    pub fn identity_response(identifier: u8, identity: &str) -> Self {
        let data = identity.as_bytes().to_vec();
        let length = 5 + data.len() as u16;
        Self {
            code: EapCode::Response,
            identifier,
            length,
            method: Some(EapMethod::Identity),
            data,
        }
    }

    /// Create NAK response (decline method, suggest alternatives)
    pub fn nak_response(identifier: u8, desired_methods: &[EapMethod]) -> Self {
        let data: Vec<u8> = desired_methods.iter().map(|m| *m as u8).collect();
        let length = 5 + data.len() as u16;
        Self {
            code: EapCode::Response,
            identifier,
            length,
            method: Some(EapMethod::Nak),
            data,
        }
    }
}

/// EAPOL (EAP over LAN) frame types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EapolType {
    /// EAP packet
    EapPacket = 0,
    /// EAPOL-Start
    Start = 1,
    /// EAPOL-Logoff
    Logoff = 2,
    /// EAPOL-Key
    Key = 3,
    /// EAPOL-Encapsulated-ASF-Alert
    Alert = 4,
    /// EAPOL-MKA (MACsec)
    Mka = 5,
    /// EAPOL-Announcement (Generic)
    AnnouncementGeneric = 6,
    /// EAPOL-Announcement (Specific)
    AnnouncementSpecific = 7,
    /// EAPOL-Announcement-Request
    AnnouncementReq = 8,
}

impl EapolType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::EapPacket),
            1 => Some(Self::Start),
            2 => Some(Self::Logoff),
            3 => Some(Self::Key),
            4 => Some(Self::Alert),
            5 => Some(Self::Mka),
            6 => Some(Self::AnnouncementGeneric),
            7 => Some(Self::AnnouncementSpecific),
            8 => Some(Self::AnnouncementReq),
            _ => None,
        }
    }
}

/// EAPOL frame structure
#[derive(Debug, Clone)]
pub struct EapolFrame {
    /// Protocol version (1, 2, or 3)
    pub version: u8,
    /// Frame type
    pub frame_type: EapolType,
    /// Body length
    pub body_length: u16,
    /// Frame body
    pub body: Vec<u8>,
}

impl EapolFrame {
    /// Parse EAPOL frame
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }

        let version = data[0];
        let frame_type = EapolType::from_u8(data[1])?;
        let body_length = u16::from_be_bytes([data[2], data[3]]);

        if data.len() < 4 + body_length as usize {
            return None;
        }

        let body = data[4..4 + body_length as usize].to_vec();

        Some(Self {
            version,
            frame_type,
            body_length,
            body,
        })
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + self.body.len());
        out.push(self.version);
        out.push(self.frame_type as u8);
        out.extend_from_slice(&self.body_length.to_be_bytes());
        out.extend_from_slice(&self.body);
        out
    }

    /// Create EAPOL-Start frame
    pub fn start() -> Self {
        Self {
            version: 2,
            frame_type: EapolType::Start,
            body_length: 0,
            body: Vec::new(),
        }
    }

    /// Create EAPOL-Logoff frame
    pub fn logoff() -> Self {
        Self {
            version: 2,
            frame_type: EapolType::Logoff,
            body_length: 0,
            body: Vec::new(),
        }
    }

    /// Wrap EAP packet in EAPOL frame
    pub fn wrap_eap(eap: &EapPacket) -> Self {
        let body = eap.to_bytes();
        Self {
            version: 2,
            frame_type: EapolType::EapPacket,
            body_length: body.len() as u16,
            body,
        }
    }
}

/// TLS record types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TlsContentType {
    ChangeCipherSpec = 20,
    Alert = 21,
    Handshake = 22,
    ApplicationData = 23,
}

/// TLS alert levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TlsAlertLevel {
    Warning = 1,
    Fatal = 2,
}

/// EAP-TLS flags
pub mod eap_tls_flags {
    pub const LENGTH_INCLUDED: u8 = 0x80;
    pub const MORE_FRAGMENTS: u8 = 0x40;
    pub const START: u8 = 0x20;
    pub const OUTER_TLV_LENGTH: u8 = 0x10;
}

/// EAP-TLS state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EapTlsState {
    /// Not started
    Idle,
    /// Waiting for server hello
    WaitingServerHello,
    /// Processing server certificate
    ProcessingCertificate,
    /// Sending client certificate
    SendingCertificate,
    /// TLS handshake complete
    TlsComplete,
    /// EAP success received
    Success,
    /// Failed
    Failed,
}

/// TLS cipher suites for WPA3-Enterprise 192-bit (RFC 8422)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tls192BitCipherSuite {
    /// TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384
    EcdheEcdsaAes256GcmSha384,
    /// TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384
    EcdheRsaAes256GcmSha384,
    /// TLS_DHE_RSA_WITH_AES_256_GCM_SHA384
    DheRsaAes256GcmSha384,
}

impl Tls192BitCipherSuite {
    /// Get cipher suite bytes for TLS ClientHello
    pub fn to_bytes(&self) -> [u8; 2] {
        match self {
            Self::EcdheEcdsaAes256GcmSha384 => [0xc0, 0x2c],
            Self::EcdheRsaAes256GcmSha384 => [0xc0, 0x30],
            Self::DheRsaAes256GcmSha384 => [0x00, 0x9f],
        }
    }

    /// Get name
    pub fn name(&self) -> &'static str {
        match self {
            Self::EcdheEcdsaAes256GcmSha384 => "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384",
            Self::EcdheRsaAes256GcmSha384 => "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
            Self::DheRsaAes256GcmSha384 => "TLS_DHE_RSA_WITH_AES_256_GCM_SHA384",
        }
    }
}

/// Certificate type
#[derive(Debug, Clone)]
pub struct Certificate {
    /// DER-encoded certificate data
    pub data: Vec<u8>,
    /// Subject common name
    pub subject_cn: Option<String>,
    /// Issuer common name
    pub issuer_cn: Option<String>,
    /// Not valid before (Unix timestamp)
    pub not_before: u64,
    /// Not valid after (Unix timestamp)
    pub not_after: u64,
}

impl Certificate {
    /// Parse certificate from DER
    pub fn from_der(der: &[u8]) -> Option<Self> {
        // Simplified parsing - real implementation needs full ASN.1
        if der.len() < 10 {
            return None;
        }

        Some(Self {
            data: der.to_vec(),
            subject_cn: None,
            issuer_cn: None,
            not_before: 0,
            not_after: u64::MAX,
        })
    }

    /// Parse certificate from PEM
    pub fn from_pem(pem: &str) -> Option<Self> {
        // Find base64 content between markers
        let start_marker = "-----BEGIN CERTIFICATE-----";
        let end_marker = "-----END CERTIFICATE-----";

        let start = pem.find(start_marker)? + start_marker.len();
        let end = pem.find(end_marker)?;

        let b64_content: String = pem[start..end]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();

        // Base64 decode (simplified)
        let der = base64_decode(&b64_content)?;
        Self::from_der(&der)
    }
}

/// Private key type
#[derive(Debug, Clone)]
pub struct PrivateKey {
    /// Key data (PKCS#8 or raw)
    pub data: Vec<u8>,
    /// Key type
    pub key_type: PrivateKeyType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateKeyType {
    Rsa,
    EcdsaP256,
    EcdsaP384,
    EcdsaP521,
}

impl PrivateKey {
    /// Parse from PEM
    pub fn from_pem(pem: &str) -> Option<Self> {
        let markers = [
            ("-----BEGIN PRIVATE KEY-----", "-----END PRIVATE KEY-----"),
            ("-----BEGIN RSA PRIVATE KEY-----", "-----END RSA PRIVATE KEY-----"),
            ("-----BEGIN EC PRIVATE KEY-----", "-----END EC PRIVATE KEY-----"),
        ];

        for (start_marker, end_marker) in markers {
            if let Some(start) = pem.find(start_marker) {
                let start = start + start_marker.len();
                if let Some(end) = pem.find(end_marker) {
                    let b64_content: String = pem[start..end]
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect();

                    if let Some(data) = base64_decode(&b64_content) {
                        return Some(Self {
                            data,
                            key_type: if start_marker.contains("RSA") {
                                PrivateKeyType::Rsa
                            } else {
                                PrivateKeyType::EcdsaP384 // Default for WPA3 192-bit
                            },
                        });
                    }
                }
            }
        }
        None
    }
}

/// Enterprise credentials
#[derive(Debug, Clone)]
pub struct EnterpriseCredentials {
    /// Identity (username or anonymous identity)
    pub identity: String,
    /// Password (for EAP-TTLS/PEAP inner methods)
    pub password: Option<String>,
    /// Client certificate
    pub client_cert: Option<Certificate>,
    /// Client private key
    pub client_key: Option<PrivateKey>,
    /// CA certificate for server verification
    pub ca_cert: Option<Certificate>,
    /// Allow insecure (no server verification)
    pub allow_insecure: bool,
    /// Anonymous identity (for TTLS/PEAP outer)
    pub anonymous_identity: Option<String>,
    /// Domain constraint (server cert must match)
    pub domain_constraint: Option<String>,
}

impl EnterpriseCredentials {
    /// Create credentials for EAP-TLS
    pub fn tls(
        identity: &str,
        client_cert: Certificate,
        client_key: PrivateKey,
        ca_cert: Option<Certificate>,
    ) -> Self {
        Self {
            identity: String::from(identity),
            password: None,
            client_cert: Some(client_cert),
            client_key: Some(client_key),
            ca_cert,
            allow_insecure: false,
            anonymous_identity: None,
            domain_constraint: None,
        }
    }

    /// Create credentials for EAP-TTLS/PEAP
    pub fn password_based(
        identity: &str,
        password: &str,
        ca_cert: Option<Certificate>,
    ) -> Self {
        Self {
            identity: String::from(identity),
            password: Some(String::from(password)),
            client_cert: None,
            client_key: None,
            ca_cert,
            allow_insecure: false,
            anonymous_identity: None,
            domain_constraint: None,
        }
    }
}

/// WPA3-Enterprise 802.1X supplicant state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eap8021xState {
    /// Disconnected
    Disconnected,
    /// EAPOL-Start sent
    Started,
    /// Waiting for EAP-Request/Identity
    WaitingIdentityRequest,
    /// Identity sent
    IdentitySent,
    /// EAP method negotiation
    Negotiating,
    /// Method-specific authentication
    Authenticating,
    /// Success
    Authenticated,
    /// Failed
    Failed,
}

/// WPA3-Enterprise supplicant
pub struct Wpa3EnterpriseSupplicant {
    /// Current state
    pub state: Eap8021xState,
    /// Credentials
    credentials: EnterpriseCredentials,
    /// Preferred EAP methods
    preferred_methods: Vec<EapMethod>,
    /// Currently negotiated method
    current_method: Option<EapMethod>,
    /// Last EAP identifier
    last_identifier: u8,
    /// Station MAC
    sta_mac: MacAddress,
    /// AP MAC (authenticator)
    ap_mac: MacAddress,
    /// Is WPA3-Enterprise 192-bit mode
    is_192bit: bool,
    /// TLS state (for EAP-TLS)
    tls_state: EapTlsState,
    /// TLS session data
    tls_session: Option<TlsSession>,
    /// Derived MSK (Master Session Key)
    msk: Option<[u8; 64]>,
    /// Derived EMSK (Extended MSK)
    emsk: Option<[u8; 64]>,
    /// Pending output frame
    outgoing: Option<Vec<u8>>,
    /// Retry counter
    retry_count: u32,
    /// Max retries
    max_retries: u32,
}

/// TLS session state
#[derive(Debug)]
pub struct TlsSession {
    /// Client random
    pub client_random: [u8; 32],
    /// Server random
    pub server_random: [u8; 32],
    /// Pre-master secret
    pub pre_master_secret: Option<Vec<u8>>,
    /// Master secret
    pub master_secret: Option<[u8; 48]>,
    /// Pending handshake messages
    pub handshake_messages: Vec<u8>,
    /// Selected cipher suite
    pub cipher_suite: Option<Tls192BitCipherSuite>,
    /// Session ID
    pub session_id: Vec<u8>,
}

impl TlsSession {
    pub fn new() -> Self {
        let mut client_random = [0u8; 32];
        fill_random(&mut client_random);

        Self {
            client_random,
            server_random: [0u8; 32],
            pre_master_secret: None,
            master_secret: None,
            handshake_messages: Vec::new(),
            cipher_suite: None,
            session_id: Vec::new(),
        }
    }
}

impl Default for TlsSession {
    fn default() -> Self {
        Self::new()
    }
}

impl Wpa3EnterpriseSupplicant {
    /// Create new enterprise supplicant
    pub fn new(
        sta_mac: MacAddress,
        ap_mac: MacAddress,
        credentials: EnterpriseCredentials,
        is_192bit: bool,
    ) -> Self {
        let preferred_methods = if is_192bit {
            // WPA3-Enterprise 192-bit only allows EAP-TLS
            vec![EapMethod::Tls]
        } else {
            // Standard enterprise allows various methods
            vec![
                EapMethod::Tls,
                EapMethod::Ttls,
                EapMethod::Peap,
                EapMethod::Pwd,
            ]
        };

        Self {
            state: Eap8021xState::Disconnected,
            credentials,
            preferred_methods,
            current_method: None,
            last_identifier: 0,
            sta_mac,
            ap_mac,
            is_192bit,
            tls_state: EapTlsState::Idle,
            tls_session: None,
            msk: None,
            emsk: None,
            outgoing: None,
            retry_count: 0,
            max_retries: 5,
        }
    }

    /// Start 802.1X authentication (send EAPOL-Start)
    pub fn start(&mut self) -> KResult<()> {
        let frame = EapolFrame::start();
        self.outgoing = Some(frame.to_bytes());
        self.state = Eap8021xState::Started;
        Ok(())
    }

    /// Process received EAPOL frame
    pub fn process_eapol(&mut self, data: &[u8]) -> KResult<()> {
        let frame = EapolFrame::parse(data).ok_or(KError::Invalid)?;

        match frame.frame_type {
            EapolType::EapPacket => {
                let eap = EapPacket::parse(&frame.body).ok_or(KError::Invalid)?;
                self.process_eap(&eap)?;
            }
            EapolType::Key => {
                // Handle EAPOL-Key (4-way handshake) if authenticated
                if self.state == Eap8021xState::Authenticated {
                    // Delegate to standard key handling
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Process EAP packet
    fn process_eap(&mut self, eap: &EapPacket) -> KResult<()> {
        self.last_identifier = eap.identifier;

        match eap.code {
            EapCode::Request => self.handle_request(eap),
            EapCode::Success => self.handle_success(),
            EapCode::Failure => self.handle_failure(),
            _ => Ok(()),
        }
    }

    /// Handle EAP-Request
    fn handle_request(&mut self, request: &EapPacket) -> KResult<()> {
        let method = request.method.ok_or(KError::Invalid)?;

        match method {
            EapMethod::Identity => {
                // Respond with identity
                let identity = self.credentials.anonymous_identity
                    .as_ref()
                    .unwrap_or(&self.credentials.identity);

                let response = EapPacket::identity_response(request.identifier, identity);
                self.send_eap_response(&response)?;
                self.state = Eap8021xState::IdentitySent;
            }
            EapMethod::Notification => {
                // Just acknowledge
                let response = EapPacket {
                    code: EapCode::Response,
                    identifier: request.identifier,
                    length: 5,
                    method: Some(EapMethod::Notification),
                    data: Vec::new(),
                };
                self.send_eap_response(&response)?;
            }
            EapMethod::Tls => {
                self.current_method = Some(EapMethod::Tls);
                self.handle_eap_tls(request)?;
            }
            EapMethod::Ttls => {
                if !self.is_192bit {
                    self.current_method = Some(EapMethod::Ttls);
                    self.handle_eap_ttls(request)?;
                } else {
                    // NAK - WPA3-192bit only allows TLS
                    let response = EapPacket::nak_response(request.identifier, &[EapMethod::Tls]);
                    self.send_eap_response(&response)?;
                }
            }
            EapMethod::Peap => {
                if !self.is_192bit {
                    self.current_method = Some(EapMethod::Peap);
                    self.handle_peap(request)?;
                } else {
                    let response = EapPacket::nak_response(request.identifier, &[EapMethod::Tls]);
                    self.send_eap_response(&response)?;
                }
            }
            _ => {
                // NAK with preferred methods
                let response = EapPacket::nak_response(request.identifier, &self.preferred_methods);
                self.send_eap_response(&response)?;
            }
        }

        Ok(())
    }

    /// Handle EAP-TLS
    fn handle_eap_tls(&mut self, request: &EapPacket) -> KResult<()> {
        if request.data.is_empty() {
            return Err(KError::Invalid);
        }

        let flags = request.data[0];
        let tls_data = if flags & eap_tls_flags::LENGTH_INCLUDED != 0 && request.data.len() > 5 {
            &request.data[5..]
        } else {
            &request.data[1..]
        };

        if flags & eap_tls_flags::START != 0 {
            // EAP-TLS start - send ClientHello
            self.tls_session = Some(TlsSession::new());
            self.tls_state = EapTlsState::WaitingServerHello;

            let client_hello = self.build_tls_client_hello()?;
            self.send_eap_tls_response(request.identifier, &client_hello, false)?;
        } else if !tls_data.is_empty() {
            // Process TLS handshake data
            self.process_tls_handshake(request.identifier, tls_data)?;
        } else {
            // Empty request after handshake - send empty response
            self.send_eap_tls_response(request.identifier, &[], false)?;
        }

        Ok(())
    }

    /// Build TLS ClientHello (for WPA3-Enterprise 192-bit)
    fn build_tls_client_hello(&mut self) -> KResult<Vec<u8>> {
        let session = self.tls_session.as_mut().ok_or(KError::Invalid)?;

        let mut hello = Vec::with_capacity(256);

        // TLS record header
        hello.push(TlsContentType::Handshake as u8);
        hello.extend_from_slice(&[0x03, 0x03]); // TLS 1.2

        let record_length_pos = hello.len();
        hello.extend_from_slice(&[0x00, 0x00]); // Placeholder for length

        // Handshake header
        hello.push(1); // ClientHello
        let handshake_length_pos = hello.len();
        hello.extend_from_slice(&[0x00, 0x00, 0x00]); // Placeholder

        // Client version (TLS 1.2)
        hello.extend_from_slice(&[0x03, 0x03]);

        // Client random
        hello.extend_from_slice(&session.client_random);

        // Session ID (empty for new session)
        hello.push(0);

        // Cipher suites for WPA3-Enterprise 192-bit
        let cipher_suites = if self.is_192bit {
            vec![
                Tls192BitCipherSuite::EcdheEcdsaAes256GcmSha384,
                Tls192BitCipherSuite::EcdheRsaAes256GcmSha384,
                Tls192BitCipherSuite::DheRsaAes256GcmSha384,
            ]
        } else {
            // Standard cipher suites
            vec![Tls192BitCipherSuite::EcdheRsaAes256GcmSha384]
        };

        hello.extend_from_slice(&((cipher_suites.len() * 2) as u16).to_be_bytes());
        for suite in cipher_suites {
            hello.extend_from_slice(&suite.to_bytes());
        }

        // Compression methods (null only)
        hello.push(1);
        hello.push(0);

        // Extensions
        let extensions_length_pos = hello.len();
        hello.extend_from_slice(&[0x00, 0x00]); // Placeholder

        // Supported groups extension (for ECDHE)
        hello.extend_from_slice(&[0x00, 0x0a]); // Extension type
        hello.extend_from_slice(&[0x00, 0x06]); // Extension length
        hello.extend_from_slice(&[0x00, 0x04]); // Groups length
        hello.extend_from_slice(&[0x00, 0x18]); // secp384r1 (for 192-bit)
        hello.extend_from_slice(&[0x00, 0x17]); // secp256r1

        // Signature algorithms extension
        hello.extend_from_slice(&[0x00, 0x0d]); // Extension type
        hello.extend_from_slice(&[0x00, 0x08]); // Extension length
        hello.extend_from_slice(&[0x00, 0x06]); // Algorithms length
        hello.extend_from_slice(&[0x05, 0x03]); // ECDSA-SECP384r1-SHA384
        hello.extend_from_slice(&[0x04, 0x03]); // ECDSA-SECP256r1-SHA256
        hello.extend_from_slice(&[0x05, 0x01]); // RSA-PKCS1-SHA384

        // Update extension length
        let extensions_length = (hello.len() - extensions_length_pos - 2) as u16;
        hello[extensions_length_pos..extensions_length_pos + 2]
            .copy_from_slice(&extensions_length.to_be_bytes());

        // Update handshake length
        let handshake_length = hello.len() - handshake_length_pos - 3;
        hello[handshake_length_pos] = ((handshake_length >> 16) & 0xff) as u8;
        hello[handshake_length_pos + 1] = ((handshake_length >> 8) & 0xff) as u8;
        hello[handshake_length_pos + 2] = (handshake_length & 0xff) as u8;

        // Update record length
        let record_length = (hello.len() - record_length_pos - 2) as u16;
        hello[record_length_pos..record_length_pos + 2]
            .copy_from_slice(&record_length.to_be_bytes());

        // Save for Finished computation
        session.handshake_messages.extend_from_slice(&hello[5..]);

        Ok(hello)
    }

    /// Process TLS handshake data
    fn process_tls_handshake(&mut self, identifier: u8, data: &[u8]) -> KResult<()> {
        let session = self.tls_session.as_mut().ok_or(KError::Invalid)?;
        session.handshake_messages.extend_from_slice(data);

        // Parse TLS records
        let mut pos = 0;
        while pos + 5 <= data.len() {
            let content_type = data[pos];
            let _version = u16::from_be_bytes([data[pos + 1], data[pos + 2]]);
            let length = u16::from_be_bytes([data[pos + 3], data[pos + 4]]) as usize;

            if pos + 5 + length > data.len() {
                break;
            }

            let record_data = &data[pos + 5..pos + 5 + length];
            pos += 5 + length;

            match content_type {
                22 => self.process_tls_handshake_record(record_data)?,
                21 => {
                    // Alert
                    self.tls_state = EapTlsState::Failed;
                    self.state = Eap8021xState::Failed;
                    return Err(KError::PermissionDenied);
                }
                20 => {
                    // ChangeCipherSpec
                    // Continue to next record
                }
                _ => {}
            }
        }

        // Send appropriate response based on state
        match self.tls_state {
            EapTlsState::WaitingServerHello => {
                // After receiving ServerHello, Certificate, etc.
                // Send ClientKeyExchange, ChangeCipherSpec, Finished
                let response = self.build_tls_key_exchange()?;
                self.send_eap_tls_response(identifier, &response, false)?;
                self.tls_state = EapTlsState::TlsComplete;
            }
            EapTlsState::TlsComplete => {
                // TLS handshake complete, derive MSK
                self.derive_msk()?;
                self.send_eap_tls_response(identifier, &[], false)?;
            }
            _ => {
                self.send_eap_tls_response(identifier, &[], false)?;
            }
        }

        Ok(())
    }

    /// Process TLS handshake record
    fn process_tls_handshake_record(&mut self, data: &[u8]) -> KResult<()> {
        if data.is_empty() {
            return Ok(());
        }

        let handshake_type = data[0];
        let _length = if data.len() >= 4 {
            ((data[1] as usize) << 16) | ((data[2] as usize) << 8) | (data[3] as usize)
        } else {
            return Err(KError::Invalid);
        };

        match handshake_type {
            2 => {
                // ServerHello
                if data.len() >= 38 {
                    let session = self.tls_session.as_mut().ok_or(KError::Invalid)?;
                    session.server_random.copy_from_slice(&data[6..38]);
                }
            }
            11 => {
                // Certificate
                self.tls_state = EapTlsState::ProcessingCertificate;
            }
            12 => {
                // ServerKeyExchange
            }
            13 => {
                // CertificateRequest
                self.tls_state = EapTlsState::SendingCertificate;
            }
            14 => {
                // ServerHelloDone
            }
            _ => {}
        }

        Ok(())
    }

    /// Build TLS client key exchange
    fn build_tls_key_exchange(&mut self) -> KResult<Vec<u8>> {
        let mut response = Vec::with_capacity(256);

        let session = self.tls_session.as_mut().ok_or(KError::Invalid)?;

        // Certificate (if required)
        if self.tls_state == EapTlsState::SendingCertificate {
            if let Some(ref cert) = self.credentials.client_cert {
                // TLS record: Certificate
                response.push(TlsContentType::Handshake as u8);
                response.extend_from_slice(&[0x03, 0x03]);
                let cert_record_len_pos = response.len();
                response.extend_from_slice(&[0x00, 0x00]);

                response.push(11); // Certificate
                let cert_len = cert.data.len() + 6;
                response.push(((cert_len >> 16) & 0xff) as u8);
                response.push(((cert_len >> 8) & 0xff) as u8);
                response.push((cert_len & 0xff) as u8);

                let certs_len = cert.data.len() + 3;
                response.push(((certs_len >> 16) & 0xff) as u8);
                response.push(((certs_len >> 8) & 0xff) as u8);
                response.push((certs_len & 0xff) as u8);

                let single_cert_len = cert.data.len();
                response.push(((single_cert_len >> 16) & 0xff) as u8);
                response.push(((single_cert_len >> 8) & 0xff) as u8);
                response.push((single_cert_len & 0xff) as u8);
                response.extend_from_slice(&cert.data);

                let cert_record_len = response.len() - cert_record_len_pos - 2;
                response[cert_record_len_pos] = ((cert_record_len >> 8) & 0xff) as u8;
                response[cert_record_len_pos + 1] = (cert_record_len & 0xff) as u8;
            }
        }

        // ClientKeyExchange (ECDHE)
        response.push(TlsContentType::Handshake as u8);
        response.extend_from_slice(&[0x03, 0x03]);
        let kex_len_pos = response.len();
        response.extend_from_slice(&[0x00, 0x00]);

        response.push(16); // ClientKeyExchange

        // Generate ECDHE public key (simplified - 65 bytes for P-384 uncompressed)
        let mut ecdhe_public = vec![0x04u8]; // Uncompressed point
        let mut random_bytes = [0u8; 96];
        fill_random(&mut random_bytes);
        ecdhe_public.extend_from_slice(&random_bytes);

        let kex_len = 1 + ecdhe_public.len();
        response.push(((kex_len >> 16) & 0xff) as u8);
        response.push(((kex_len >> 8) & 0xff) as u8);
        response.push((kex_len & 0xff) as u8);

        response.push(ecdhe_public.len() as u8);
        response.extend_from_slice(&ecdhe_public);

        let kex_record_len = response.len() - kex_len_pos - 2;
        response[kex_len_pos] = ((kex_record_len >> 8) & 0xff) as u8;
        response[kex_len_pos + 1] = (kex_record_len & 0xff) as u8;

        // Generate pre-master secret
        let mut pms = vec![0u8; 48];
        fill_random(&mut pms);
        session.pre_master_secret = Some(pms);

        // CertificateVerify (if we sent certificate)
        if self.credentials.client_cert.is_some() && self.credentials.client_key.is_some() {
            response.push(TlsContentType::Handshake as u8);
            response.extend_from_slice(&[0x03, 0x03]);
            let cv_len_pos = response.len();
            response.extend_from_slice(&[0x00, 0x00]);

            response.push(15); // CertificateVerify

            // Signature (simplified)
            let mut signature = [0u8; 72];
            fill_random(&mut signature);

            let cv_len = 4 + signature.len();
            response.push(((cv_len >> 16) & 0xff) as u8);
            response.push(((cv_len >> 8) & 0xff) as u8);
            response.push((cv_len & 0xff) as u8);

            // Signature algorithm (ECDSA-SECP384r1-SHA384)
            response.extend_from_slice(&[0x05, 0x03]);
            response.extend_from_slice(&(signature.len() as u16).to_be_bytes());
            response.extend_from_slice(&signature);

            let cv_record_len = response.len() - cv_len_pos - 2;
            response[cv_len_pos] = ((cv_record_len >> 8) & 0xff) as u8;
            response[cv_len_pos + 1] = (cv_record_len & 0xff) as u8;
        }

        // ChangeCipherSpec
        response.push(TlsContentType::ChangeCipherSpec as u8);
        response.extend_from_slice(&[0x03, 0x03]);
        response.extend_from_slice(&[0x00, 0x01, 0x01]);

        // Finished (encrypted - simplified)
        response.push(TlsContentType::Handshake as u8);
        response.extend_from_slice(&[0x03, 0x03]);

        let mut finished_data = vec![0u8; 28]; // 12 bytes verify_data + overhead
        fill_random(&mut finished_data);

        response.extend_from_slice(&(finished_data.len() as u16).to_be_bytes());
        response.extend_from_slice(&finished_data);

        Ok(response)
    }

    /// Derive MSK from TLS session
    fn derive_msk(&mut self) -> KResult<()> {
        let session = self.tls_session.as_ref().ok_or(KError::Invalid)?;

        // PRF(master_secret, "client EAP encryption", client_random + server_random)
        let mut seed = Vec::with_capacity(64);
        seed.extend_from_slice(&session.client_random);
        seed.extend_from_slice(&session.server_random);

        // Simplified MSK derivation
        let msk_material = kdf_sha256(
            session.pre_master_secret.as_ref().ok_or(KError::Invalid)?,
            b"client EAP encryption",
            &seed,
            128,
        );

        let mut msk = [0u8; 64];
        let mut emsk = [0u8; 64];
        msk.copy_from_slice(&msk_material[..64]);
        emsk.copy_from_slice(&msk_material[64..128]);

        self.msk = Some(msk);
        self.emsk = Some(emsk);

        Ok(())
    }

    /// Handle EAP-TTLS
    fn handle_eap_ttls(&mut self, request: &EapPacket) -> KResult<()> {
        // Similar to EAP-TLS but with inner authentication
        self.handle_eap_tls(request)
    }

    /// Handle PEAP
    fn handle_peap(&mut self, request: &EapPacket) -> KResult<()> {
        // Similar to EAP-TLS but with inner authentication
        self.handle_eap_tls(request)
    }

    /// Send EAP-TLS response
    fn send_eap_tls_response(
        &mut self,
        identifier: u8,
        tls_data: &[u8],
        more_fragments: bool,
    ) -> KResult<()> {
        let mut data = Vec::with_capacity(tls_data.len() + 5);

        let mut flags = 0u8;
        if more_fragments {
            flags |= eap_tls_flags::MORE_FRAGMENTS;
        }
        if !tls_data.is_empty() && tls_data.len() > 1000 {
            flags |= eap_tls_flags::LENGTH_INCLUDED;
            data.push(flags);
            data.extend_from_slice(&(tls_data.len() as u32).to_be_bytes());
        } else {
            data.push(flags);
        }
        data.extend_from_slice(tls_data);

        let response = EapPacket {
            code: EapCode::Response,
            identifier,
            length: 5 + data.len() as u16,
            method: Some(EapMethod::Tls),
            data,
        };

        self.send_eap_response(&response)
    }

    /// Send EAP response wrapped in EAPOL
    fn send_eap_response(&mut self, eap: &EapPacket) -> KResult<()> {
        let frame = EapolFrame::wrap_eap(eap);
        self.outgoing = Some(frame.to_bytes());
        Ok(())
    }

    /// Handle EAP-Success
    fn handle_success(&mut self) -> KResult<()> {
        if self.msk.is_some() {
            self.state = Eap8021xState::Authenticated;
            self.tls_state = EapTlsState::Success;
        } else {
            self.state = Eap8021xState::Failed;
        }
        Ok(())
    }

    /// Handle EAP-Failure
    fn handle_failure(&mut self) -> KResult<()> {
        self.state = Eap8021xState::Failed;
        self.tls_state = EapTlsState::Failed;
        Ok(())
    }

    /// Get pending outgoing frame
    pub fn get_outgoing(&mut self) -> Option<Vec<u8>> {
        self.outgoing.take()
    }

    /// Get MSK (for PMK derivation)
    pub fn get_msk(&self) -> Option<&[u8; 64]> {
        self.msk.as_ref()
    }

    /// Get PMK (first 32 bytes of MSK)
    pub fn get_pmk(&self) -> Option<[u8; 32]> {
        self.msk.map(|msk| {
            let mut pmk = [0u8; 32];
            pmk.copy_from_slice(&msk[..32]);
            pmk
        })
    }

    /// Is authentication complete?
    pub fn is_complete(&self) -> bool {
        self.state == Eap8021xState::Authenticated
    }

    /// Get current EAP method
    pub fn current_method(&self) -> Option<EapMethod> {
        self.current_method
    }

    /// Get status string
    pub fn status(&self) -> String {
        let mode = if self.is_192bit {
            "WPA3-Enterprise 192-bit"
        } else {
            "WPA3-Enterprise"
        };

        let method = self.current_method
            .map(|m| m.name())
            .unwrap_or("None");

        let state = match self.state {
            Eap8021xState::Disconnected => "Disconnected",
            Eap8021xState::Started => "Started",
            Eap8021xState::WaitingIdentityRequest => "Waiting for identity",
            Eap8021xState::IdentitySent => "Identity sent",
            Eap8021xState::Negotiating => "Negotiating",
            Eap8021xState::Authenticating => "Authenticating",
            Eap8021xState::Authenticated => "Authenticated",
            Eap8021xState::Failed => "Failed",
        };

        alloc::format!("{} ({}) - {}", mode, method, state)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Simple base64 decode
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    const DECODE_TABLE: [i8; 128] = [
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 62, -1, -1, -1, 63,
        52, 53, 54, 55, 56, 57, 58, 59, 60, 61, -1, -1, -1, -1, -1, -1,
        -1,  0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14,
        15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, -1, -1, -1, -1, -1,
        -1, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
        41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, -1, -1, -1, -1, -1,
    ];

    let input = input.trim_end_matches('=');
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u8;

    for c in input.chars() {
        let val = if c as usize >= 128 {
            return None;
        } else {
            DECODE_TABLE[c as usize]
        };

        if val < 0 {
            return None;
        }

        buffer = (buffer << 6) | (val as u32);
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xff) as u8);
        }
    }

    Some(output)
}

/// Create WPA3-Enterprise supplicant (standard)
pub fn create_wpa3_enterprise(
    sta_mac: MacAddress,
    ap_mac: MacAddress,
    credentials: EnterpriseCredentials,
) -> Wpa3EnterpriseSupplicant {
    Wpa3EnterpriseSupplicant::new(sta_mac, ap_mac, credentials, false)
}

/// Create WPA3-Enterprise 192-bit supplicant
pub fn create_wpa3_enterprise_192bit(
    sta_mac: MacAddress,
    ap_mac: MacAddress,
    credentials: EnterpriseCredentials,
) -> Wpa3EnterpriseSupplicant {
    Wpa3EnterpriseSupplicant::new(sta_mac, ap_mac, credentials, true)
}

/// Check if AP supports WPA3-Enterprise
pub fn supports_wpa3_enterprise(rsn_ie: &[u8]) -> bool {
    if rsn_ie.len() < 20 {
        return false;
    }

    let akm_count_pos = 10;
    if rsn_ie.len() > akm_count_pos + 2 {
        let akm_count = u16::from_le_bytes([rsn_ie[akm_count_pos], rsn_ie[akm_count_pos + 1]]);
        let akm_start = akm_count_pos + 2;

        for i in 0..akm_count as usize {
            let pos = akm_start + i * 4;
            if pos + 4 <= rsn_ie.len() {
                let suite = &rsn_ie[pos..pos + 4];
                // EAP SHA-256 or Suite B 192-bit
                if suite == [0x00, 0x0f, 0xac, 0x05] || suite == [0x00, 0x0f, 0xac, 0x12] {
                    return true;
                }
            }
        }
    }

    false
}

/// Check if AP supports WPA3-Enterprise 192-bit
pub fn supports_wpa3_enterprise_192bit(rsn_ie: &[u8]) -> bool {
    if rsn_ie.len() < 20 {
        return false;
    }

    // Check for Suite B 192-bit AKM (0x00 0x0f 0xac 0x12)
    let akm_count_pos = 10;
    if rsn_ie.len() > akm_count_pos + 2 {
        let akm_count = u16::from_le_bytes([rsn_ie[akm_count_pos], rsn_ie[akm_count_pos + 1]]);
        let akm_start = akm_count_pos + 2;

        for i in 0..akm_count as usize {
            let pos = akm_start + i * 4;
            if pos + 4 <= rsn_ie.len() {
                let suite = &rsn_ie[pos..pos + 4];
                if suite == [0x00, 0x0f, 0xac, 0x12] {
                    return true;
                }
            }
        }
    }

    // Also check cipher suite for GCMP-256
    if rsn_ie.len() >= 8 {
        let group_cipher = &rsn_ie[4..8];
        if group_cipher == [0x00, 0x0f, 0xac, 0x09] {
            return true; // GCMP-256
        }
    }

    false
}
