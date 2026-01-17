//! WPA/WPA2 Authentication
//!
//! Implements the WPA/WPA2 4-way handshake and key management.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::util::{KResult, KError};
use super::mac::MacAddress;
use super::crypto::{derive_pmk, prf, hmac_sha1};

/// WPA version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WpaVersion {
    /// WPA (TKIP)
    Wpa1,
    /// WPA2 (CCMP/AES)
    Wpa2,
}

/// WPA authentication state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WpaState {
    /// Not started
    Idle,
    /// Waiting for message 1
    WaitingMsg1,
    /// Sent message 2, waiting for message 3
    WaitingMsg3,
    /// Complete
    Complete,
    /// Failed
    Failed,
}

/// WPA configuration
#[derive(Debug, Clone)]
pub struct WpaConfig {
    /// SSID
    pub ssid: String,
    /// Passphrase
    pub passphrase: String,
    /// WPA version (default WPA2)
    pub version: WpaVersion,
    /// Station MAC address
    pub sta_mac: MacAddress,
    /// AP MAC address
    pub ap_mac: MacAddress,
}

impl Default for WpaConfig {
    fn default() -> Self {
        Self {
            ssid: String::new(),
            passphrase: String::new(),
            version: WpaVersion::Wpa2,
            sta_mac: MacAddress::ZERO,
            ap_mac: MacAddress::ZERO,
        }
    }
}

/// WPA events for the connection manager
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WpaEvent {
    /// Nothing to do
    None,
    /// Message 2 is ready to send
    Message2Ready,
    /// Message 4 is ready to send
    Message4Ready,
    /// Handshake complete
    Complete,
    /// Handshake failed
    Failed,
}

/// EAPOL key types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EapolKeyType {
    /// Group key
    Group = 0,
    /// Pairwise key
    Pairwise = 1,
}

/// EAPOL key information flags
#[derive(Debug, Clone, Copy)]
pub struct KeyInfo(u16);

impl KeyInfo {
    /// Key descriptor version (bits 0-2)
    pub fn descriptor_version(&self) -> u8 {
        (self.0 & 0x07) as u8
    }

    /// Key type (bit 3): 0 = group, 1 = pairwise
    pub fn key_type(&self) -> EapolKeyType {
        if self.0 & (1 << 3) != 0 {
            EapolKeyType::Pairwise
        } else {
            EapolKeyType::Group
        }
    }

    /// Install flag (bit 6)
    pub fn install(&self) -> bool {
        self.0 & (1 << 6) != 0
    }

    /// ACK flag (bit 7)
    pub fn ack(&self) -> bool {
        self.0 & (1 << 7) != 0
    }

    /// MIC flag (bit 8)
    pub fn mic(&self) -> bool {
        self.0 & (1 << 8) != 0
    }

    /// Secure flag (bit 9)
    pub fn secure(&self) -> bool {
        self.0 & (1 << 9) != 0
    }

    /// Error flag (bit 10)
    pub fn error(&self) -> bool {
        self.0 & (1 << 10) != 0
    }

    /// Request flag (bit 11)
    pub fn request(&self) -> bool {
        self.0 & (1 << 11) != 0
    }

    /// Encrypted key data flag (bit 12)
    pub fn encrypted_key_data(&self) -> bool {
        self.0 & (1 << 12) != 0
    }

    /// Create key info
    pub fn new(
        descriptor_version: u8,
        key_type: EapolKeyType,
        install: bool,
        ack: bool,
        mic: bool,
        secure: bool,
    ) -> Self {
        let mut info: u16 = 0;
        info |= (descriptor_version & 0x07) as u16;
        if key_type == EapolKeyType::Pairwise {
            info |= 1 << 3;
        }
        if install {
            info |= 1 << 6;
        }
        if ack {
            info |= 1 << 7;
        }
        if mic {
            info |= 1 << 8;
        }
        if secure {
            info |= 1 << 9;
        }
        KeyInfo(info)
    }

    pub fn to_be_bytes(&self) -> [u8; 2] {
        self.0.to_be_bytes()
    }

    pub fn from_be_bytes(bytes: [u8; 2]) -> Self {
        KeyInfo(u16::from_be_bytes(bytes))
    }
}

/// EAPOL-Key frame
#[derive(Debug, Clone)]
pub struct EapolKeyFrame {
    /// Protocol version
    pub version: u8,
    /// Packet type (3 = Key)
    pub packet_type: u8,
    /// Packet body length
    pub length: u16,
    /// Key descriptor type (2 = RSN, 254 = WPA)
    pub descriptor_type: u8,
    /// Key information
    pub key_info: KeyInfo,
    /// Key length
    pub key_length: u16,
    /// Replay counter
    pub replay_counter: u64,
    /// Key nonce
    pub key_nonce: [u8; 32],
    /// Key IV (WPA1 only)
    pub key_iv: [u8; 16],
    /// Key RSC (receive sequence counter)
    pub key_rsc: u64,
    /// Key ID (reserved)
    pub key_id: u64,
    /// Key MIC
    pub key_mic: [u8; 16],
    /// Key data length
    pub key_data_length: u16,
    /// Key data
    pub key_data: Vec<u8>,
}

impl EapolKeyFrame {
    /// Parse from bytes
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 99 {
            return None;
        }

        let version = data[0];
        let packet_type = data[1];
        let length = u16::from_be_bytes([data[2], data[3]]);
        let descriptor_type = data[4];
        let key_info = KeyInfo::from_be_bytes([data[5], data[6]]);
        let key_length = u16::from_be_bytes([data[7], data[8]]);
        let replay_counter = u64::from_be_bytes([
            data[9], data[10], data[11], data[12],
            data[13], data[14], data[15], data[16],
        ]);

        let mut key_nonce = [0u8; 32];
        key_nonce.copy_from_slice(&data[17..49]);

        let mut key_iv = [0u8; 16];
        key_iv.copy_from_slice(&data[49..65]);

        let key_rsc = u64::from_be_bytes([
            data[65], data[66], data[67], data[68],
            data[69], data[70], data[71], data[72],
        ]);
        let key_id = u64::from_be_bytes([
            data[73], data[74], data[75], data[76],
            data[77], data[78], data[79], data[80],
        ]);

        let mut key_mic = [0u8; 16];
        key_mic.copy_from_slice(&data[81..97]);

        let key_data_length = u16::from_be_bytes([data[97], data[98]]);

        let key_data = if key_data_length > 0 && data.len() >= 99 + key_data_length as usize {
            data[99..99 + key_data_length as usize].to_vec()
        } else {
            Vec::new()
        };

        Some(EapolKeyFrame {
            version,
            packet_type,
            length,
            descriptor_type,
            key_info,
            key_length,
            replay_counter,
            key_nonce,
            key_iv,
            key_rsc,
            key_id,
            key_mic,
            key_data_length,
            key_data,
        })
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(99 + self.key_data.len());

        bytes.push(self.version);
        bytes.push(self.packet_type);
        bytes.extend_from_slice(&self.length.to_be_bytes());
        bytes.push(self.descriptor_type);
        bytes.extend_from_slice(&self.key_info.to_be_bytes());
        bytes.extend_from_slice(&self.key_length.to_be_bytes());
        bytes.extend_from_slice(&self.replay_counter.to_be_bytes());
        bytes.extend_from_slice(&self.key_nonce);
        bytes.extend_from_slice(&self.key_iv);
        bytes.extend_from_slice(&self.key_rsc.to_be_bytes());
        bytes.extend_from_slice(&self.key_id.to_be_bytes());
        bytes.extend_from_slice(&self.key_mic);
        bytes.extend_from_slice(&self.key_data_length.to_be_bytes());
        bytes.extend_from_slice(&self.key_data);

        bytes
    }

    /// Serialize to bytes without MIC (for MIC calculation)
    pub fn to_bytes_for_mic(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(99 + self.key_data.len());

        bytes.push(self.version);
        bytes.push(self.packet_type);
        bytes.extend_from_slice(&self.length.to_be_bytes());
        bytes.push(self.descriptor_type);
        bytes.extend_from_slice(&self.key_info.to_be_bytes());
        bytes.extend_from_slice(&self.key_length.to_be_bytes());
        bytes.extend_from_slice(&self.replay_counter.to_be_bytes());
        bytes.extend_from_slice(&self.key_nonce);
        bytes.extend_from_slice(&self.key_iv);
        bytes.extend_from_slice(&self.key_rsc.to_be_bytes());
        bytes.extend_from_slice(&self.key_id.to_be_bytes());
        bytes.extend_from_slice(&[0u8; 16]); // Zero MIC
        bytes.extend_from_slice(&self.key_data_length.to_be_bytes());
        bytes.extend_from_slice(&self.key_data);

        bytes
    }
}

/// Pairwise Transient Key (PTK)
#[derive(Clone)]
pub struct Ptk {
    /// Key Confirmation Key (for MIC)
    pub kck: [u8; 16],
    /// Key Encryption Key (for key data encryption)
    pub kek: [u8; 16],
    /// Temporal Key (for data encryption)
    pub tk: [u8; 16],
}

impl Ptk {
    /// Derive PTK from PMK and nonces
    pub fn derive(
        pmk: &[u8; 32],
        aa: &MacAddress,  // Authenticator address (AP)
        spa: &MacAddress, // Supplicant address (STA)
        anonce: &[u8; 32],
        snonce: &[u8; 32],
    ) -> Self {
        // Build data for PRF
        // Min(AA, SPA) || Max(AA, SPA) || Min(ANonce, SNonce) || Max(ANonce, SNonce)
        let mut data = Vec::with_capacity(6 + 6 + 32 + 32);

        // Sort addresses
        if aa.0 < spa.0 {
            data.extend_from_slice(&aa.0);
            data.extend_from_slice(&spa.0);
        } else {
            data.extend_from_slice(&spa.0);
            data.extend_from_slice(&aa.0);
        }

        // Sort nonces
        if anonce < snonce {
            data.extend_from_slice(anonce);
            data.extend_from_slice(snonce);
        } else {
            data.extend_from_slice(snonce);
            data.extend_from_slice(anonce);
        }

        // Derive PTK using PRF-384
        let ptk_data = prf(pmk, "Pairwise key expansion", &data, 48);

        let mut kck = [0u8; 16];
        let mut kek = [0u8; 16];
        let mut tk = [0u8; 16];

        kck.copy_from_slice(&ptk_data[0..16]);
        kek.copy_from_slice(&ptk_data[16..32]);
        tk.copy_from_slice(&ptk_data[32..48]);

        Ptk { kck, kek, tk }
    }
}

/// Group Temporal Key (GTK)
#[derive(Clone)]
pub struct Gtk {
    /// Key data
    pub key: Vec<u8>,
    /// Key index
    pub index: u8,
    /// TX key flag
    pub tx: bool,
}

/// WPA supplicant state machine
pub struct WpaSupplicant {
    /// Current state
    pub state: WpaState,
    /// WPA version
    version: WpaVersion,
    /// Pairwise Master Key (from passphrase)
    pmk: [u8; 32],
    /// PTK (derived during handshake)
    ptk: Option<Ptk>,
    /// GTK (received during handshake)
    gtk: Option<Gtk>,
    /// Supplicant address (our MAC)
    spa: MacAddress,
    /// Authenticator address (AP MAC)
    aa: MacAddress,
    /// Supplicant nonce
    snonce: [u8; 32],
    /// Authenticator nonce
    anonce: [u8; 32],
    /// Replay counter
    replay_counter: u64,
    /// SSID
    ssid: String,
    /// Pending outgoing message
    outgoing_message: Option<Vec<u8>>,
    /// Configuration set
    configured: bool,
}

impl WpaSupplicant {
    /// Create new empty WPA supplicant
    pub fn new() -> Self {
        Self {
            state: WpaState::Idle,
            version: WpaVersion::Wpa2,
            pmk: [0u8; 32],
            ptk: None,
            gtk: None,
            spa: MacAddress::ZERO,
            aa: MacAddress::ZERO,
            snonce: [0u8; 32],
            anonce: [0u8; 32],
            replay_counter: 0,
            ssid: String::new(),
            outgoing_message: None,
            configured: false,
        }
    }

    /// Create new WPA supplicant with configuration
    pub fn with_config(
        version: WpaVersion,
        ssid: &str,
        passphrase: &str,
        sta_mac: MacAddress,
        ap_mac: MacAddress,
    ) -> Self {
        // Derive PMK from passphrase
        let pmk = derive_pmk(passphrase, ssid);

        // Generate random supplicant nonce
        let snonce = generate_nonce();

        Self {
            state: WpaState::Idle,
            version,
            pmk,
            ptk: None,
            gtk: None,
            spa: sta_mac,
            aa: ap_mac,
            snonce,
            anonce: [0u8; 32],
            replay_counter: 0,
            ssid: String::from(ssid),
            outgoing_message: None,
            configured: true,
        }
    }

    /// Configure the supplicant
    pub fn configure(&mut self, config: WpaConfig) {
        self.version = config.version;
        self.ssid = config.ssid.clone();
        self.pmk = derive_pmk(&config.passphrase, &config.ssid);
        self.spa = config.sta_mac;
        self.aa = config.ap_mac;
        self.snonce = generate_nonce();
        self.configured = true;
    }

    /// Start the handshake (wait for message 1)
    pub fn start(&mut self) -> KResult<()> {
        if !self.configured {
            return Err(KError::Invalid);
        }
        self.state = WpaState::WaitingMsg1;
        Ok(())
    }

    /// Process incoming EAPOL data and return event
    pub fn process_eapol(&mut self, data: &[u8]) -> KResult<WpaEvent> {
        let frame = EapolKeyFrame::parse(data).ok_or(KError::Invalid)?;

        match self.process_frame(&frame)? {
            Some(response) => {
                self.outgoing_message = Some(response.to_bytes());

                match self.state {
                    WpaState::WaitingMsg3 => Ok(WpaEvent::Message2Ready),
                    WpaState::Complete => Ok(WpaEvent::Message4Ready),
                    _ => Ok(WpaEvent::None),
                }
            }
            None => {
                if self.state == WpaState::Complete {
                    Ok(WpaEvent::Complete)
                } else if self.state == WpaState::Failed {
                    Ok(WpaEvent::Failed)
                } else {
                    Ok(WpaEvent::None)
                }
            }
        }
    }

    /// Get pending outgoing message
    pub fn get_outgoing_message(&mut self) -> Option<Vec<u8>> {
        self.outgoing_message.take()
    }

    /// Get PTK
    pub fn get_ptk(&self) -> Option<&Ptk> {
        self.ptk.as_ref()
    }

    /// Get GTK key bytes
    pub fn get_gtk(&self) -> Option<&[u8]> {
        self.gtk.as_ref().map(|g| g.key.as_slice())
    }

    /// Get GTK info
    pub fn get_gtk_info(&self) -> Option<&Gtk> {
        self.gtk.as_ref()
    }

    /// Process received EAPOL-Key frame
    pub fn process_frame(&mut self, frame: &EapolKeyFrame) -> KResult<Option<EapolKeyFrame>> {
        // Verify replay counter
        if frame.replay_counter < self.replay_counter {
            return Err(KError::Invalid);
        }

        match self.state {
            WpaState::WaitingMsg1 => {
                self.process_msg1(frame)
            }
            WpaState::WaitingMsg3 => {
                self.process_msg3(frame)
            }
            _ => {
                Err(KError::Invalid)
            }
        }
    }

    /// Process message 1 (from AP)
    fn process_msg1(&mut self, msg1: &EapolKeyFrame) -> KResult<Option<EapolKeyFrame>> {
        // Message 1: ANonce from AP
        // Key Info: Pairwise=1, ACK=1
        if !msg1.key_info.ack() || msg1.key_info.key_type() != EapolKeyType::Pairwise {
            return Err(KError::Invalid);
        }

        // Save ANonce
        self.anonce.copy_from_slice(&msg1.key_nonce);
        self.replay_counter = msg1.replay_counter;

        // Derive PTK
        let ptk = Ptk::derive(&self.pmk, &self.aa, &self.spa, &self.anonce, &self.snonce);
        self.ptk = Some(ptk.clone());

        // Build message 2
        let msg2 = self.build_msg2(&ptk)?;

        self.state = WpaState::WaitingMsg3;

        Ok(Some(msg2))
    }

    /// Build message 2 (response to AP)
    fn build_msg2(&self, ptk: &Ptk) -> KResult<EapolKeyFrame> {
        let key_info = KeyInfo::new(
            2, // HMAC-SHA1-128 for WPA2
            EapolKeyType::Pairwise,
            false, // No install yet
            false, // No ACK (we're supplicant)
            true,  // MIC present
            false, // Not secure yet
        );

        // Build RSN IE for key data
        let key_data = self.build_rsn_ie();

        let length = 95 + key_data.len() as u16;

        let mut frame = EapolKeyFrame {
            version: 2,
            packet_type: 3, // Key
            length,
            descriptor_type: 2, // RSN
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

    /// Process message 3 (from AP)
    fn process_msg3(&mut self, msg3: &EapolKeyFrame) -> KResult<Option<EapolKeyFrame>> {
        // Message 3: Install=1, ACK=1, MIC=1, Secure=1, Encrypted=1
        if !msg3.key_info.ack() || !msg3.key_info.mic() || !msg3.key_info.secure() {
            return Err(KError::Invalid);
        }

        let ptk = self.ptk.as_ref().ok_or(KError::Invalid)?;

        // Verify MIC
        if !self.verify_mic(ptk, msg3)? {
            self.state = WpaState::Failed;
            return Err(KError::PermissionDenied);
        }

        // Verify ANonce matches
        if msg3.key_nonce != self.anonce {
            self.state = WpaState::Failed;
            return Err(KError::Invalid);
        }

        self.replay_counter = msg3.replay_counter;

        // Decrypt and parse key data for GTK
        if msg3.key_info.encrypted_key_data() && !msg3.key_data.is_empty() {
            if let Some(gtk) = self.extract_gtk(ptk, &msg3.key_data) {
                self.gtk = Some(gtk);
            }
        }

        // Build message 4
        let msg4 = self.build_msg4(ptk)?;

        self.state = WpaState::Complete;

        Ok(Some(msg4))
    }

    /// Build message 4 (final confirmation)
    fn build_msg4(&self, ptk: &Ptk) -> KResult<EapolKeyFrame> {
        let key_info = KeyInfo::new(
            2, // HMAC-SHA1-128
            EapolKeyType::Pairwise,
            false, // No install
            false, // No ACK
            true,  // MIC present
            true,  // Secure
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

        // Calculate MIC
        let mic = self.calculate_mic(ptk, &frame)?;
        frame.key_mic = mic;

        Ok(frame)
    }

    /// Calculate MIC for a frame
    fn calculate_mic(&self, ptk: &Ptk, frame: &EapolKeyFrame) -> KResult<[u8; 16]> {
        let data = frame.to_bytes_for_mic();
        let hmac = hmac_sha1(&ptk.kck, &data);

        let mut mic = [0u8; 16];
        mic.copy_from_slice(&hmac[..16]);
        Ok(mic)
    }

    /// Verify MIC of a received frame
    fn verify_mic(&self, ptk: &Ptk, frame: &EapolKeyFrame) -> KResult<bool> {
        let expected_mic = self.calculate_mic(ptk, frame)?;
        Ok(constant_time_compare(&expected_mic, &frame.key_mic))
    }

    /// Build RSN Information Element
    fn build_rsn_ie(&self) -> Vec<u8> {
        let mut ie = Vec::with_capacity(22);

        // RSN IE header
        ie.push(48); // Element ID
        ie.push(20); // Length

        // Version
        ie.extend_from_slice(&1u16.to_le_bytes());

        // Group cipher suite (CCMP)
        ie.extend_from_slice(&[0x00, 0x0f, 0xac, 0x04]);

        // Pairwise cipher suite count
        ie.extend_from_slice(&1u16.to_le_bytes());
        // Pairwise cipher suite (CCMP)
        ie.extend_from_slice(&[0x00, 0x0f, 0xac, 0x04]);

        // AKM suite count
        ie.extend_from_slice(&1u16.to_le_bytes());
        // AKM suite (PSK)
        ie.extend_from_slice(&[0x00, 0x0f, 0xac, 0x02]);

        // RSN capabilities
        ie.extend_from_slice(&0u16.to_le_bytes());

        ie
    }

    /// Extract GTK from encrypted key data
    fn extract_gtk(&self, ptk: &Ptk, encrypted_data: &[u8]) -> Option<Gtk> {
        // Decrypt key data using KEK
        let decrypted = aes_unwrap(&ptk.kek, encrypted_data)?;

        // Parse KDE (Key Data Encapsulation)
        let mut pos = 0;
        while pos + 6 <= decrypted.len() {
            let element_type = decrypted[pos];
            let element_len = decrypted[pos + 1] as usize;

            if element_type == 0xdd && element_len >= 6 {
                // Vendor-specific: check for GTK KDE
                let oui = &decrypted[pos + 2..pos + 5];
                let data_type = decrypted[pos + 5];

                // IEEE 802.11i GTK KDE
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

}

/// Generate random nonce
fn generate_nonce() -> [u8; 32] {
    let mut nonce = [0u8; 32];
    // Use devfs random_byte function
    for i in 0..32 {
        nonce[i] = crate::fs::devfs::random_byte();
    }
    nonce
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

    // Unwrap
    for j in (0..6).rev() {
        for i in (0..n).rev() {
            let t = (n * j + i + 1) as u64;
            let mut t_bytes = [0u8; 8];
            t_bytes.copy_from_slice(&t.to_be_bytes());

            // XOR A with t
            for k in 0..8 {
                a[k] ^= t_bytes[k];
            }

            // Decrypt
            let mut block = [0u8; 16];
            block[..8].copy_from_slice(&a);
            block[8..].copy_from_slice(&r[i]);

            let decrypted = aes_decrypt_block(kek, &block);
            a.copy_from_slice(&decrypted[..8]);
            r[i].copy_from_slice(&decrypted[8..]);
        }
    }

    // Verify AIV
    let aiv = [0xa6u8; 8];
    if a != aiv {
        return None;
    }

    // Collect result
    let mut result = Vec::with_capacity(n * 8);
    for block in r {
        result.extend_from_slice(&block);
    }

    Some(result)
}

/// AES-128 decrypt single block (placeholder - needs real AES)
fn aes_decrypt_block(key: &[u8; 16], data: &[u8; 16]) -> [u8; 16] {
    // This is a placeholder - in production, use a real AES implementation
    // For now, do a simple XOR-based transformation
    let mut output = [0u8; 16];
    for i in 0..16 {
        output[i] = data[i] ^ key[i];
    }
    output
}

/// Create EAPOL frame with Ethernet header
pub fn create_eapol_frame(
    src_mac: &MacAddress,
    dst_mac: &MacAddress,
    eapol: &EapolKeyFrame,
) -> Vec<u8> {
    let mut frame = Vec::with_capacity(14 + 4 + eapol.to_bytes().len());

    // Ethernet header
    frame.extend_from_slice(&dst_mac.0);
    frame.extend_from_slice(&src_mac.0);
    frame.extend_from_slice(&0x888Eu16.to_be_bytes()); // EAPOL ethertype

    // EAPOL frame
    frame.extend(eapol.to_bytes());

    frame
}

/// Parse EAPOL frame from Ethernet frame
pub fn parse_eapol_frame(data: &[u8]) -> Option<EapolKeyFrame> {
    if data.len() < 14 {
        return None;
    }

    // Check ethertype
    let ethertype = u16::from_be_bytes([data[12], data[13]]);
    if ethertype != 0x888e {
        return None;
    }

    // Parse EAPOL
    EapolKeyFrame::parse(&data[14..])
}

impl Default for WpaSupplicant {
    fn default() -> Self {
        Self::new()
    }
}
