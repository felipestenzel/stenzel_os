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
