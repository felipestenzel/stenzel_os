//! Bluetooth Pairing
//!
//! Implements Bluetooth device pairing including:
//! - Secure Simple Pairing (SSP) for Bluetooth 2.1+
//! - Legacy PIN/Passkey pairing for older devices
//! - Security Manager Protocol (SMP) for BLE
//! - Link key management and storage
//!
//! References:
//! - Bluetooth Core Spec 5.3, Vol 2 Part F (Link Manager Protocol)
//! - Bluetooth Core Spec 5.3, Vol 3 Part H (Security Manager Protocol)

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use crate::sync::TicketSpinlock;
use super::{BdAddr, LinkKeyType};

/// IO Capability for Secure Simple Pairing
/// Determines what pairing method will be used
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IoCapability {
    /// Display only (e.g., device with screen but no input)
    DisplayOnly = 0x00,
    /// Display with yes/no buttons
    DisplayYesNo = 0x01,
    /// Keyboard only (e.g., keyboard device)
    KeyboardOnly = 0x02,
    /// No input, no output (e.g., headset)
    NoInputNoOutput = 0x03,
    /// Keyboard with display
    KeyboardDisplay = 0x04,
}

impl Default for IoCapability {
    fn default() -> Self {
        IoCapability::DisplayYesNo
    }
}

impl IoCapability {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(IoCapability::DisplayOnly),
            0x01 => Some(IoCapability::DisplayYesNo),
            0x02 => Some(IoCapability::KeyboardOnly),
            0x03 => Some(IoCapability::NoInputNoOutput),
            0x04 => Some(IoCapability::KeyboardDisplay),
            _ => None,
        }
    }
}

/// OOB Data Present flag
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OobDataPresent {
    NotPresent = 0x00,
    P192Present = 0x01,
    P256Present = 0x02,
    P192AndP256Present = 0x03,
}

impl Default for OobDataPresent {
    fn default() -> Self {
        OobDataPresent::NotPresent
    }
}

/// Authentication requirements for pairing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthRequirements {
    /// MITM protection required
    pub mitm_required: bool,
    /// Bonding requested
    pub bonding: bool,
    /// Secure Connections required
    pub secure_connections: bool,
    /// Keypress notifications
    pub keypress: bool,
    /// CT2 (Cross-Transport Key Derivation)
    pub ct2: bool,
}

impl Default for AuthRequirements {
    fn default() -> Self {
        Self {
            mitm_required: true,
            bonding: true,
            secure_connections: true,
            keypress: false,
            ct2: false,
        }
    }
}

impl AuthRequirements {
    /// Encode as byte for HCI commands
    pub fn to_byte(&self) -> u8 {
        let mut val = 0u8;
        if self.bonding {
            val |= 0x01;
        }
        if self.mitm_required {
            val |= 0x04;
        }
        if self.secure_connections {
            val |= 0x08;
        }
        if self.keypress {
            val |= 0x10;
        }
        if self.ct2 {
            val |= 0x20;
        }
        val
    }

    /// Decode from byte
    pub fn from_byte(val: u8) -> Self {
        Self {
            bonding: val & 0x01 != 0,
            mitm_required: val & 0x04 != 0,
            secure_connections: val & 0x08 != 0,
            keypress: val & 0x10 != 0,
            ct2: val & 0x20 != 0,
        }
    }
}

/// Pairing method determined by IO capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingMethod {
    /// Just works - no user interaction (no MITM protection)
    JustWorks,
    /// Numeric comparison - display 6-digit code on both devices
    NumericComparison,
    /// Passkey entry - user enters 6-digit code
    PasskeyEntry,
    /// Out of band - use external channel (NFC, etc.)
    OutOfBand,
}

impl PairingMethod {
    /// Determine pairing method from IO capabilities
    /// According to Bluetooth Core Spec Vol 3 Part C, Section 5.2.2.6
    pub fn determine(initiator: IoCapability, responder: IoCapability, mitm: bool) -> Self {
        if !mitm {
            return PairingMethod::JustWorks;
        }

        use IoCapability::*;
        match (initiator, responder) {
            // Display Only initiator
            (DisplayOnly, DisplayOnly) => PairingMethod::JustWorks,
            (DisplayOnly, DisplayYesNo) => PairingMethod::JustWorks,
            (DisplayOnly, KeyboardOnly) => PairingMethod::PasskeyEntry,
            (DisplayOnly, NoInputNoOutput) => PairingMethod::JustWorks,
            (DisplayOnly, KeyboardDisplay) => PairingMethod::PasskeyEntry,

            // Display Yes/No initiator
            (DisplayYesNo, DisplayOnly) => PairingMethod::JustWorks,
            (DisplayYesNo, DisplayYesNo) => PairingMethod::NumericComparison,
            (DisplayYesNo, KeyboardOnly) => PairingMethod::PasskeyEntry,
            (DisplayYesNo, NoInputNoOutput) => PairingMethod::JustWorks,
            (DisplayYesNo, KeyboardDisplay) => PairingMethod::NumericComparison,

            // Keyboard Only initiator
            (KeyboardOnly, DisplayOnly) => PairingMethod::PasskeyEntry,
            (KeyboardOnly, DisplayYesNo) => PairingMethod::PasskeyEntry,
            (KeyboardOnly, KeyboardOnly) => PairingMethod::PasskeyEntry,
            (KeyboardOnly, NoInputNoOutput) => PairingMethod::JustWorks,
            (KeyboardOnly, KeyboardDisplay) => PairingMethod::PasskeyEntry,

            // No Input No Output initiator
            (NoInputNoOutput, _) => PairingMethod::JustWorks,

            // Keyboard Display initiator
            (KeyboardDisplay, DisplayOnly) => PairingMethod::PasskeyEntry,
            (KeyboardDisplay, DisplayYesNo) => PairingMethod::NumericComparison,
            (KeyboardDisplay, KeyboardOnly) => PairingMethod::PasskeyEntry,
            (KeyboardDisplay, NoInputNoOutput) => PairingMethod::JustWorks,
            (KeyboardDisplay, KeyboardDisplay) => PairingMethod::NumericComparison,
        }
    }

    pub fn provides_mitm_protection(&self) -> bool {
        match self {
            PairingMethod::JustWorks => false,
            PairingMethod::NumericComparison => true,
            PairingMethod::PasskeyEntry => true,
            PairingMethod::OutOfBand => true,
        }
    }
}

/// Pairing state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingState {
    /// Not pairing
    Idle,
    /// Received pairing request, waiting for IO capabilities exchange
    IoCapabilityExchange,
    /// Waiting for public key exchange (SSP/LE SC)
    PublicKeyExchange,
    /// Performing authentication stage 1 (depends on pairing method)
    AuthenticationStage1,
    /// Performing authentication stage 2 (check values)
    AuthenticationStage2,
    /// Calculating link key / LTK
    LinkKeyCalculation,
    /// Bonding - storing keys
    Bonding,
    /// Pairing complete
    Complete,
    /// Pairing failed
    Failed,
}

/// Pairing context for ongoing pairing operation
#[derive(Debug)]
pub struct PairingContext {
    /// Remote device address
    pub remote_address: BdAddr,
    /// Connection handle
    pub handle: u16,
    /// Current pairing state
    pub state: PairingState,
    /// Local IO capability
    pub local_io_cap: IoCapability,
    /// Remote IO capability
    pub remote_io_cap: Option<IoCapability>,
    /// Local authentication requirements
    pub local_auth_req: AuthRequirements,
    /// Remote authentication requirements
    pub remote_auth_req: Option<AuthRequirements>,
    /// Determined pairing method
    pub pairing_method: Option<PairingMethod>,
    /// Local OOB data present
    pub local_oob: OobDataPresent,
    /// Remote OOB data present
    pub remote_oob: Option<OobDataPresent>,
    /// Numeric comparison value (if applicable)
    pub numeric_value: Option<u32>,
    /// Passkey for entry (if applicable)
    pub passkey: Option<u32>,
    /// User confirmed (for numeric comparison)
    pub user_confirmed: Option<bool>,
    /// Initiator flag
    pub is_initiator: bool,
    /// Legacy pairing mode (PIN-based)
    pub legacy_pairing: bool,
    /// PIN code (for legacy pairing)
    pub pin_code: Option<[u8; 16]>,
    /// PIN length
    pub pin_length: usize,
    /// Link key generated
    pub link_key: Option<[u8; 16]>,
    /// Link key type
    pub link_key_type: Option<LinkKeyType>,
    /// Secure Connections flag
    pub secure_connections: bool,
    /// Error code if failed
    pub error: Option<u8>,
}

impl PairingContext {
    pub fn new(remote_address: BdAddr, handle: u16, is_initiator: bool) -> Self {
        Self {
            remote_address,
            handle,
            state: PairingState::Idle,
            local_io_cap: IoCapability::default(),
            remote_io_cap: None,
            local_auth_req: AuthRequirements::default(),
            remote_auth_req: None,
            pairing_method: None,
            local_oob: OobDataPresent::NotPresent,
            remote_oob: None,
            numeric_value: None,
            passkey: None,
            user_confirmed: None,
            is_initiator,
            legacy_pairing: false,
            pin_code: None,
            pin_length: 0,
            link_key: None,
            link_key_type: None,
            secure_connections: false,
            error: None,
        }
    }

    /// Set local IO capability
    pub fn set_io_capability(&mut self, io_cap: IoCapability) {
        self.local_io_cap = io_cap;
    }

    /// Set local authentication requirements
    pub fn set_auth_requirements(&mut self, auth_req: AuthRequirements) {
        self.local_auth_req = auth_req;
    }

    /// Process remote IO capability response
    pub fn set_remote_io_capability(&mut self, io_cap: IoCapability, oob: OobDataPresent, auth_req: AuthRequirements) {
        self.remote_io_cap = Some(io_cap);
        self.remote_oob = Some(oob);
        self.remote_auth_req = Some(auth_req);

        // Determine pairing method
        let mitm = self.local_auth_req.mitm_required || auth_req.mitm_required;
        self.pairing_method = Some(PairingMethod::determine(self.local_io_cap, io_cap, mitm));
    }

    /// Set numeric comparison value
    pub fn set_numeric_value(&mut self, value: u32) {
        self.numeric_value = Some(value);
    }

    /// User confirms numeric comparison
    pub fn confirm_numeric(&mut self, confirmed: bool) {
        self.user_confirmed = Some(confirmed);
    }

    /// Set passkey for entry
    pub fn set_passkey(&mut self, passkey: u32) {
        self.passkey = Some(passkey);
    }

    /// Set PIN for legacy pairing
    pub fn set_pin(&mut self, pin: &[u8]) {
        let len = pin.len().min(16);
        let mut code = [0u8; 16];
        code[..len].copy_from_slice(&pin[..len]);
        self.pin_code = Some(code);
        self.pin_length = len;
        self.legacy_pairing = true;
    }

    /// Store generated link key
    pub fn set_link_key(&mut self, key: [u8; 16], key_type: LinkKeyType) {
        self.link_key = Some(key);
        self.link_key_type = Some(key_type);
    }

    /// Mark pairing as complete
    pub fn complete(&mut self) {
        self.state = PairingState::Complete;
    }

    /// Mark pairing as failed
    pub fn fail(&mut self, error: u8) {
        self.state = PairingState::Failed;
        self.error = Some(error);
    }

    /// Check if pairing provides MITM protection
    pub fn has_mitm_protection(&self) -> bool {
        self.pairing_method
            .map(|m| m.provides_mitm_protection())
            .unwrap_or(false)
    }
}

/// Stored link key information
#[derive(Debug, Clone)]
pub struct StoredLinkKey {
    /// Device address
    pub address: BdAddr,
    /// Link key (128 bits)
    pub key: [u8; 16],
    /// Key type
    pub key_type: LinkKeyType,
    /// Is authenticated (MITM protection)
    pub authenticated: bool,
    /// Creation timestamp (tick count when created)
    pub created: u64,
    /// Last used timestamp
    pub last_used: u64,
}

/// Link key storage manager
pub struct LinkKeyStorage {
    /// Stored keys by device address
    keys: BTreeMap<[u8; 6], StoredLinkKey>,
    /// Maximum keys to store
    max_keys: usize,
}

impl LinkKeyStorage {
    pub const fn new() -> Self {
        Self {
            keys: BTreeMap::new(),
            max_keys: 32,
        }
    }

    /// Store a link key
    pub fn store(&mut self, address: BdAddr, key: [u8; 16], key_type: LinkKeyType, authenticated: bool) {
        let now = crate::time::ticks();

        // If at capacity, remove oldest unused key
        if self.keys.len() >= self.max_keys {
            let oldest = self.keys.iter()
                .min_by_key(|(_, v)| v.last_used)
                .map(|(k, _)| *k);

            if let Some(addr) = oldest {
                self.keys.remove(&addr);
            }
        }

        self.keys.insert(address.0, StoredLinkKey {
            address,
            key,
            key_type,
            authenticated,
            created: now,
            last_used: now,
        });
    }

    /// Retrieve a link key
    pub fn get(&mut self, address: &BdAddr) -> Option<&StoredLinkKey> {
        if let Some(key) = self.keys.get_mut(&address.0) {
            key.last_used = crate::time::ticks();
            Some(key)
        } else {
            None
        }
    }

    /// Check if we have a link key for device
    pub fn has_key(&self, address: &BdAddr) -> bool {
        self.keys.contains_key(&address.0)
    }

    /// Remove a link key
    pub fn remove(&mut self, address: &BdAddr) -> bool {
        self.keys.remove(&address.0).is_some()
    }

    /// Clear all stored keys
    pub fn clear(&mut self) {
        self.keys.clear();
    }

    /// Get number of stored keys
    pub fn count(&self) -> usize {
        self.keys.len()
    }

    /// List all paired devices
    pub fn paired_devices(&self) -> Vec<(BdAddr, bool)> {
        self.keys.values()
            .map(|k| (k.address, k.authenticated))
            .collect()
    }

    /// Export keys (for backup)
    pub fn export(&self) -> Vec<StoredLinkKey> {
        self.keys.values().cloned().collect()
    }

    /// Import keys (from backup)
    pub fn import(&mut self, keys: Vec<StoredLinkKey>) {
        for key in keys {
            if self.keys.len() < self.max_keys {
                self.keys.insert(key.address.0, key);
            }
        }
    }
}

/// Global link key storage
pub static LINK_KEY_STORAGE: TicketSpinlock<LinkKeyStorage> =
    TicketSpinlock::new(LinkKeyStorage::new());

// =============================================================================
// HCI Commands for Pairing
// =============================================================================

/// HCI command opcodes for pairing
pub mod commands {
    use alloc::vec::Vec;
    use super::super::BdAddr;

    /// Link Control Commands - OGF 0x01
    pub const AUTHENTICATION_REQUESTED: u16 = 0x0411;
    pub const SET_CONNECTION_ENCRYPTION: u16 = 0x0413;
    pub const LINK_KEY_REQUEST_REPLY: u16 = 0x040B;
    pub const LINK_KEY_REQUEST_NEGATIVE_REPLY: u16 = 0x040C;
    pub const PIN_CODE_REQUEST_REPLY: u16 = 0x040D;
    pub const PIN_CODE_REQUEST_NEGATIVE_REPLY: u16 = 0x040E;
    pub const IO_CAPABILITY_REQUEST_REPLY: u16 = 0x042B;
    pub const USER_CONFIRMATION_REQUEST_REPLY: u16 = 0x042C;
    pub const USER_CONFIRMATION_REQUEST_NEGATIVE_REPLY: u16 = 0x042D;
    pub const USER_PASSKEY_REQUEST_REPLY: u16 = 0x042E;
    pub const USER_PASSKEY_REQUEST_NEGATIVE_REPLY: u16 = 0x042F;
    pub const IO_CAPABILITY_REQUEST_NEGATIVE_REPLY: u16 = 0x0434;

    /// Host Controller & Baseband Commands - OGF 0x03
    pub const READ_SIMPLE_PAIRING_MODE: u16 = 0x0C55;
    pub const WRITE_SIMPLE_PAIRING_MODE: u16 = 0x0C56;
    pub const READ_LOCAL_OOB_DATA: u16 = 0x0C57;
    pub const READ_SECURE_CONNECTIONS_HOST_SUPPORT: u16 = 0x0C79;
    pub const WRITE_SECURE_CONNECTIONS_HOST_SUPPORT: u16 = 0x0C7A;
    pub const READ_LOCAL_OOB_EXTENDED_DATA: u16 = 0x0C7D;

    /// Build HCI command packet
    fn build_command(opcode: u16, params: &[u8]) -> Vec<u8> {
        let mut cmd = Vec::with_capacity(3 + params.len());
        cmd.push((opcode & 0xFF) as u8);
        cmd.push((opcode >> 8) as u8);
        cmd.push(params.len() as u8);
        cmd.extend_from_slice(params);
        cmd
    }

    /// Request authentication for connection
    pub fn authentication_requested(handle: u16) -> Vec<u8> {
        let params = [
            (handle & 0xFF) as u8,
            ((handle >> 8) & 0x0F) as u8,
        ];
        build_command(AUTHENTICATION_REQUESTED, &params)
    }

    /// Set connection encryption
    pub fn set_connection_encryption(handle: u16, enable: bool) -> Vec<u8> {
        let params = [
            (handle & 0xFF) as u8,
            ((handle >> 8) & 0x0F) as u8,
            if enable { 0x01 } else { 0x00 },
        ];
        build_command(SET_CONNECTION_ENCRYPTION, &params)
    }

    /// Reply to link key request with stored key
    pub fn link_key_request_reply(address: &BdAddr, key: &[u8; 16]) -> Vec<u8> {
        let mut params = Vec::with_capacity(22);
        params.extend_from_slice(&address.0);
        params.extend_from_slice(key);
        build_command(LINK_KEY_REQUEST_REPLY, &params)
    }

    /// Negative reply to link key request (no stored key)
    pub fn link_key_request_negative_reply(address: &BdAddr) -> Vec<u8> {
        build_command(LINK_KEY_REQUEST_NEGATIVE_REPLY, &address.0)
    }

    /// Reply to PIN code request (legacy pairing)
    pub fn pin_code_request_reply(address: &BdAddr, pin: &[u8], pin_len: u8) -> Vec<u8> {
        let mut params = Vec::with_capacity(23);
        params.extend_from_slice(&address.0);
        params.push(pin_len);
        let mut pin_padded = [0u8; 16];
        let len = (pin_len as usize).min(pin.len()).min(16);
        pin_padded[..len].copy_from_slice(&pin[..len]);
        params.extend_from_slice(&pin_padded);
        build_command(PIN_CODE_REQUEST_REPLY, &params)
    }

    /// Negative reply to PIN code request
    pub fn pin_code_request_negative_reply(address: &BdAddr) -> Vec<u8> {
        build_command(PIN_CODE_REQUEST_NEGATIVE_REPLY, &address.0)
    }

    /// Reply to IO capability request (SSP)
    pub fn io_capability_request_reply(
        address: &BdAddr,
        io_cap: super::IoCapability,
        oob: super::OobDataPresent,
        auth_req: &super::AuthRequirements,
    ) -> Vec<u8> {
        let mut params = Vec::with_capacity(9);
        params.extend_from_slice(&address.0);
        params.push(io_cap as u8);
        params.push(oob as u8);
        params.push(auth_req.to_byte());
        build_command(IO_CAPABILITY_REQUEST_REPLY, &params)
    }

    /// Negative reply to IO capability request
    pub fn io_capability_request_negative_reply(address: &BdAddr, reason: u8) -> Vec<u8> {
        let mut params = Vec::with_capacity(7);
        params.extend_from_slice(&address.0);
        params.push(reason);
        build_command(IO_CAPABILITY_REQUEST_NEGATIVE_REPLY, &params)
    }

    /// Confirm numeric comparison
    pub fn user_confirmation_request_reply(address: &BdAddr) -> Vec<u8> {
        build_command(USER_CONFIRMATION_REQUEST_REPLY, &address.0)
    }

    /// Reject numeric comparison
    pub fn user_confirmation_request_negative_reply(address: &BdAddr) -> Vec<u8> {
        build_command(USER_CONFIRMATION_REQUEST_NEGATIVE_REPLY, &address.0)
    }

    /// Reply with passkey
    pub fn user_passkey_request_reply(address: &BdAddr, passkey: u32) -> Vec<u8> {
        let mut params = Vec::with_capacity(10);
        params.extend_from_slice(&address.0);
        params.extend_from_slice(&passkey.to_le_bytes());
        build_command(USER_PASSKEY_REQUEST_REPLY, &params)
    }

    /// Reject passkey request
    pub fn user_passkey_request_negative_reply(address: &BdAddr) -> Vec<u8> {
        build_command(USER_PASSKEY_REQUEST_NEGATIVE_REPLY, &address.0)
    }

    /// Enable/disable Simple Pairing Mode
    pub fn write_simple_pairing_mode(enable: bool) -> Vec<u8> {
        build_command(WRITE_SIMPLE_PAIRING_MODE, &[if enable { 0x01 } else { 0x00 }])
    }

    /// Read Simple Pairing Mode status
    pub fn read_simple_pairing_mode() -> Vec<u8> {
        build_command(READ_SIMPLE_PAIRING_MODE, &[])
    }

    /// Enable/disable Secure Connections host support
    pub fn write_secure_connections_host_support(enable: bool) -> Vec<u8> {
        build_command(WRITE_SECURE_CONNECTIONS_HOST_SUPPORT, &[if enable { 0x01 } else { 0x00 }])
    }

    /// Read Secure Connections host support
    pub fn read_secure_connections_host_support() -> Vec<u8> {
        build_command(READ_SECURE_CONNECTIONS_HOST_SUPPORT, &[])
    }

    /// Read local OOB data (for out-of-band pairing)
    pub fn read_local_oob_data() -> Vec<u8> {
        build_command(READ_LOCAL_OOB_DATA, &[])
    }

    /// Read local OOB extended data (P-192 and P-256)
    pub fn read_local_oob_extended_data() -> Vec<u8> {
        build_command(READ_LOCAL_OOB_EXTENDED_DATA, &[])
    }
}

/// HCI events related to pairing
pub mod events {
    pub const LINK_KEY_REQUEST: u8 = 0x17;
    pub const LINK_KEY_NOTIFICATION: u8 = 0x18;
    pub const PIN_CODE_REQUEST: u8 = 0x16;
    pub const AUTHENTICATION_COMPLETE: u8 = 0x06;
    pub const ENCRYPTION_CHANGE: u8 = 0x08;
    pub const ENCRYPTION_KEY_REFRESH_COMPLETE: u8 = 0x30;
    pub const IO_CAPABILITY_REQUEST: u8 = 0x31;
    pub const IO_CAPABILITY_RESPONSE: u8 = 0x32;
    pub const USER_CONFIRMATION_REQUEST: u8 = 0x33;
    pub const USER_PASSKEY_REQUEST: u8 = 0x34;
    pub const SIMPLE_PAIRING_COMPLETE: u8 = 0x36;
    pub const USER_PASSKEY_NOTIFICATION: u8 = 0x3B;
    pub const KEYPRESS_NOTIFICATION: u8 = 0x3C;
}

// =============================================================================
// Security Manager Protocol (SMP) for BLE
// =============================================================================

/// SMP module for Bluetooth Low Energy pairing
pub mod smp {
    use alloc::vec;
    use alloc::vec::Vec;

    /// SMP command codes
    pub mod code {
        pub const PAIRING_REQUEST: u8 = 0x01;
        pub const PAIRING_RESPONSE: u8 = 0x02;
        pub const PAIRING_CONFIRM: u8 = 0x03;
        pub const PAIRING_RANDOM: u8 = 0x04;
        pub const PAIRING_FAILED: u8 = 0x05;
        pub const ENCRYPTION_INFORMATION: u8 = 0x06;
        pub const CENTRAL_IDENTIFICATION: u8 = 0x07;
        pub const IDENTITY_INFORMATION: u8 = 0x08;
        pub const IDENTITY_ADDRESS_INFORMATION: u8 = 0x09;
        pub const SIGNING_INFORMATION: u8 = 0x0A;
        pub const SECURITY_REQUEST: u8 = 0x0B;
        pub const PAIRING_PUBLIC_KEY: u8 = 0x0C;
        pub const PAIRING_DHKEY_CHECK: u8 = 0x0D;
        pub const PAIRING_KEYPRESS_NOTIFICATION: u8 = 0x0E;
    }

    /// SMP error codes
    pub mod error {
        pub const PASSKEY_ENTRY_FAILED: u8 = 0x01;
        pub const OOB_NOT_AVAILABLE: u8 = 0x02;
        pub const AUTHENTICATION_REQUIREMENTS: u8 = 0x03;
        pub const CONFIRM_VALUE_FAILED: u8 = 0x04;
        pub const PAIRING_NOT_SUPPORTED: u8 = 0x05;
        pub const ENCRYPTION_KEY_SIZE: u8 = 0x06;
        pub const COMMAND_NOT_SUPPORTED: u8 = 0x07;
        pub const UNSPECIFIED_REASON: u8 = 0x08;
        pub const REPEATED_ATTEMPTS: u8 = 0x09;
        pub const INVALID_PARAMETERS: u8 = 0x0A;
        pub const DHKEY_CHECK_FAILED: u8 = 0x0B;
        pub const NUMERIC_COMPARISON_FAILED: u8 = 0x0C;
        pub const BREDR_PAIRING_IN_PROGRESS: u8 = 0x0D;
        pub const CT_KEY_DERIVATION_NOT_ALLOWED: u8 = 0x0E;
        pub const KEY_REJECTED: u8 = 0x0F;
    }

    /// SMP IO capability (same values as BR/EDR but different context)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub enum SmIoCapability {
        DisplayOnly = 0x00,
        DisplayYesNo = 0x01,
        KeyboardOnly = 0x02,
        NoInputNoOutput = 0x03,
        KeyboardDisplay = 0x04,
    }

    /// Key distribution flags
    #[derive(Debug, Clone, Copy, Default)]
    pub struct KeyDistribution {
        /// Encryption key (LTK for LE, Link Key for BR/EDR via CTKD)
        pub enc_key: bool,
        /// Identity information (IRK + Identity Address)
        pub id_key: bool,
        /// Signature key (CSRK)
        pub sign_key: bool,
        /// Link key (BR/EDR link key via CTKD)
        pub link_key: bool,
    }

    impl KeyDistribution {
        pub fn to_byte(&self) -> u8 {
            let mut val = 0u8;
            if self.enc_key { val |= 0x01; }
            if self.id_key { val |= 0x02; }
            if self.sign_key { val |= 0x04; }
            if self.link_key { val |= 0x08; }
            val
        }

        pub fn from_byte(val: u8) -> Self {
            Self {
                enc_key: val & 0x01 != 0,
                id_key: val & 0x02 != 0,
                sign_key: val & 0x04 != 0,
                link_key: val & 0x08 != 0,
            }
        }
    }

    /// SMP pairing request/response structure
    #[derive(Debug, Clone)]
    pub struct SmPairingParams {
        pub io_capability: SmIoCapability,
        pub oob_data_flag: bool,
        pub bonding: bool,
        pub mitm: bool,
        pub secure_connections: bool,
        pub keypress: bool,
        pub ct2: bool,
        pub max_key_size: u8,
        pub initiator_key_dist: KeyDistribution,
        pub responder_key_dist: KeyDistribution,
    }

    impl Default for SmPairingParams {
        fn default() -> Self {
            Self {
                io_capability: SmIoCapability::DisplayYesNo,
                oob_data_flag: false,
                bonding: true,
                mitm: true,
                secure_connections: true,
                keypress: false,
                ct2: false,
                max_key_size: 16,
                initiator_key_dist: KeyDistribution {
                    enc_key: true,
                    id_key: true,
                    sign_key: false,
                    link_key: false,
                },
                responder_key_dist: KeyDistribution {
                    enc_key: true,
                    id_key: true,
                    sign_key: false,
                    link_key: false,
                },
            }
        }
    }

    impl SmPairingParams {
        /// Encode auth_req byte
        fn auth_req_byte(&self) -> u8 {
            let mut val = 0u8;
            if self.bonding { val |= 0x01; }
            if self.mitm { val |= 0x04; }
            if self.secure_connections { val |= 0x08; }
            if self.keypress { val |= 0x10; }
            if self.ct2 { val |= 0x20; }
            val
        }

        /// Build pairing request PDU
        pub fn build_pairing_request(&self) -> Vec<u8> {
            vec![
                code::PAIRING_REQUEST,
                self.io_capability as u8,
                if self.oob_data_flag { 0x01 } else { 0x00 },
                self.auth_req_byte(),
                self.max_key_size,
                self.initiator_key_dist.to_byte(),
                self.responder_key_dist.to_byte(),
            ]
        }

        /// Build pairing response PDU
        pub fn build_pairing_response(&self) -> Vec<u8> {
            vec![
                code::PAIRING_RESPONSE,
                self.io_capability as u8,
                if self.oob_data_flag { 0x01 } else { 0x00 },
                self.auth_req_byte(),
                self.max_key_size,
                self.initiator_key_dist.to_byte(),
                self.responder_key_dist.to_byte(),
            ]
        }

        /// Parse from received PDU
        pub fn from_pdu(pdu: &[u8]) -> Option<Self> {
            if pdu.len() < 7 {
                return None;
            }

            let io_cap = match pdu[1] {
                0x00 => SmIoCapability::DisplayOnly,
                0x01 => SmIoCapability::DisplayYesNo,
                0x02 => SmIoCapability::KeyboardOnly,
                0x03 => SmIoCapability::NoInputNoOutput,
                0x04 => SmIoCapability::KeyboardDisplay,
                _ => return None,
            };

            let oob = pdu[2] != 0;
            let auth = pdu[3];
            let max_key = pdu[4];
            let init_dist = KeyDistribution::from_byte(pdu[5]);
            let resp_dist = KeyDistribution::from_byte(pdu[6]);

            Some(Self {
                io_capability: io_cap,
                oob_data_flag: oob,
                bonding: auth & 0x01 != 0,
                mitm: auth & 0x04 != 0,
                secure_connections: auth & 0x08 != 0,
                keypress: auth & 0x10 != 0,
                ct2: auth & 0x20 != 0,
                max_key_size: max_key,
                initiator_key_dist: init_dist,
                responder_key_dist: resp_dist,
            })
        }
    }

    /// Build pairing confirm PDU
    pub fn build_pairing_confirm(confirm_value: &[u8; 16]) -> Vec<u8> {
        let mut pdu = Vec::with_capacity(17);
        pdu.push(code::PAIRING_CONFIRM);
        pdu.extend_from_slice(confirm_value);
        pdu
    }

    /// Build pairing random PDU
    pub fn build_pairing_random(random_value: &[u8; 16]) -> Vec<u8> {
        let mut pdu = Vec::with_capacity(17);
        pdu.push(code::PAIRING_RANDOM);
        pdu.extend_from_slice(random_value);
        pdu
    }

    /// Build pairing failed PDU
    pub fn build_pairing_failed(reason: u8) -> Vec<u8> {
        vec![code::PAIRING_FAILED, reason]
    }

    /// Build encryption information PDU (LTK)
    pub fn build_encryption_information(ltk: &[u8; 16]) -> Vec<u8> {
        let mut pdu = Vec::with_capacity(17);
        pdu.push(code::ENCRYPTION_INFORMATION);
        pdu.extend_from_slice(ltk);
        pdu
    }

    /// Build central identification PDU (EDIV + Rand)
    pub fn build_central_identification(ediv: u16, rand: u64) -> Vec<u8> {
        let mut pdu = Vec::with_capacity(11);
        pdu.push(code::CENTRAL_IDENTIFICATION);
        pdu.extend_from_slice(&ediv.to_le_bytes());
        pdu.extend_from_slice(&rand.to_le_bytes());
        pdu
    }

    /// Build identity information PDU (IRK)
    pub fn build_identity_information(irk: &[u8; 16]) -> Vec<u8> {
        let mut pdu = Vec::with_capacity(17);
        pdu.push(code::IDENTITY_INFORMATION);
        pdu.extend_from_slice(irk);
        pdu
    }

    /// Build identity address information PDU
    pub fn build_identity_address_information(addr_type: u8, addr: &[u8; 6]) -> Vec<u8> {
        let mut pdu = Vec::with_capacity(8);
        pdu.push(code::IDENTITY_ADDRESS_INFORMATION);
        pdu.push(addr_type);
        pdu.extend_from_slice(addr);
        pdu
    }

    /// Build signing information PDU (CSRK)
    pub fn build_signing_information(csrk: &[u8; 16]) -> Vec<u8> {
        let mut pdu = Vec::with_capacity(17);
        pdu.push(code::SIGNING_INFORMATION);
        pdu.extend_from_slice(csrk);
        pdu
    }

    /// Build security request PDU
    pub fn build_security_request(bonding: bool, mitm: bool, sc: bool) -> Vec<u8> {
        let mut auth = 0u8;
        if bonding { auth |= 0x01; }
        if mitm { auth |= 0x04; }
        if sc { auth |= 0x08; }
        vec![code::SECURITY_REQUEST, auth]
    }

    /// Build pairing public key PDU
    pub fn build_pairing_public_key(x: &[u8; 32], y: &[u8; 32]) -> Vec<u8> {
        let mut pdu = Vec::with_capacity(65);
        pdu.push(code::PAIRING_PUBLIC_KEY);
        pdu.extend_from_slice(x);
        pdu.extend_from_slice(y);
        pdu
    }

    /// Build DHKey check PDU
    pub fn build_pairing_dhkey_check(check: &[u8; 16]) -> Vec<u8> {
        let mut pdu = Vec::with_capacity(17);
        pdu.push(code::PAIRING_DHKEY_CHECK);
        pdu.extend_from_slice(check);
        pdu
    }

    /// Keypress notification types
    pub mod keypress {
        pub const ENTRY_STARTED: u8 = 0x00;
        pub const DIGIT_ENTERED: u8 = 0x01;
        pub const DIGIT_ERASED: u8 = 0x02;
        pub const CLEARED: u8 = 0x03;
        pub const ENTRY_COMPLETED: u8 = 0x04;
    }

    /// Build keypress notification PDU
    pub fn build_keypress_notification(notification_type: u8) -> Vec<u8> {
        vec![code::PAIRING_KEYPRESS_NOTIFICATION, notification_type]
    }
}

// =============================================================================
// Pairing Manager
// =============================================================================

/// Pairing event for callbacks
#[derive(Debug, Clone)]
pub enum PairingEvent {
    /// Pairing started
    Started { address: BdAddr, legacy: bool },
    /// IO capability exchange completed
    IoCapabilityExchanged { address: BdAddr, method: PairingMethod },
    /// Numeric comparison display request
    NumericComparison { address: BdAddr, value: u32 },
    /// Passkey display for remote entry
    PasskeyDisplay { address: BdAddr, passkey: u32 },
    /// Passkey entry request
    PasskeyRequest { address: BdAddr },
    /// PIN code request (legacy)
    PinCodeRequest { address: BdAddr },
    /// Pairing succeeded
    Success { address: BdAddr, authenticated: bool },
    /// Pairing failed
    Failed { address: BdAddr, reason: u8 },
}

/// Pairing manager
pub struct PairingManager {
    /// Active pairing contexts
    contexts: Vec<PairingContext>,
    /// Default IO capability
    default_io_cap: IoCapability,
    /// Default authentication requirements
    default_auth_req: AuthRequirements,
    /// Event callbacks (index into callback array)
    event_callbacks: Vec<fn(PairingEvent)>,
    /// Auto-accept Just Works pairing
    auto_accept_just_works: bool,
    /// Default PIN for legacy pairing
    default_pin: Option<[u8; 16]>,
    /// Default PIN length
    default_pin_length: usize,
}

impl PairingManager {
    pub const fn new() -> Self {
        Self {
            contexts: Vec::new(),
            default_io_cap: IoCapability::DisplayYesNo,
            default_auth_req: AuthRequirements {
                mitm_required: true,
                bonding: true,
                secure_connections: true,
                keypress: false,
                ct2: false,
            },
            event_callbacks: Vec::new(),
            auto_accept_just_works: false,
            default_pin: None,
            default_pin_length: 0,
        }
    }

    /// Set default IO capability
    pub fn set_io_capability(&mut self, io_cap: IoCapability) {
        self.default_io_cap = io_cap;
    }

    /// Set default auth requirements
    pub fn set_auth_requirements(&mut self, auth_req: AuthRequirements) {
        self.default_auth_req = auth_req;
    }

    /// Set auto-accept for Just Works pairing
    pub fn set_auto_accept_just_works(&mut self, auto: bool) {
        self.auto_accept_just_works = auto;
    }

    /// Set default PIN for legacy pairing
    pub fn set_default_pin(&mut self, pin: &[u8]) {
        let len = pin.len().min(16);
        let mut code = [0u8; 16];
        code[..len].copy_from_slice(&pin[..len]);
        self.default_pin = Some(code);
        self.default_pin_length = len;
    }

    /// Register event callback
    pub fn on_event(&mut self, callback: fn(PairingEvent)) {
        self.event_callbacks.push(callback);
    }

    /// Fire event to callbacks
    fn fire_event(&self, event: PairingEvent) {
        for callback in &self.event_callbacks {
            callback(event.clone());
        }
    }

    /// Start pairing with a device
    pub fn start_pairing(&mut self, address: BdAddr, handle: u16) -> Result<(), &'static str> {
        // Check if already pairing with this device
        if self.get_context(&address).is_some() {
            return Err("Already pairing with this device");
        }

        let mut ctx = PairingContext::new(address, handle, true);
        ctx.set_io_capability(self.default_io_cap);
        ctx.set_auth_requirements(self.default_auth_req);
        ctx.state = PairingState::IoCapabilityExchange;

        self.contexts.push(ctx);

        self.fire_event(PairingEvent::Started {
            address,
            legacy: false,
        });

        Ok(())
    }

    /// Get pairing context for device
    pub fn get_context(&self, address: &BdAddr) -> Option<&PairingContext> {
        self.contexts.iter().find(|c| c.remote_address == *address)
    }

    /// Get pairing context for device (mutable)
    pub fn get_context_mut(&mut self, address: &BdAddr) -> Option<&mut PairingContext> {
        self.contexts.iter_mut().find(|c| c.remote_address == *address)
    }

    /// Handle IO capability request (we need to send our capabilities)
    pub fn handle_io_capability_request(&mut self, address: &BdAddr) -> Option<Vec<u8>> {
        // Create context if not exists (we're responding to remote initiation)
        if self.get_context(address).is_none() {
            let ctx = PairingContext::new(*address, 0, false);
            self.contexts.push(ctx);

            self.fire_event(PairingEvent::Started {
                address: *address,
                legacy: false,
            });
        }

        // Copy values before getting mutable borrow
        let io_cap = self.default_io_cap;
        let auth_req = self.default_auth_req;

        if let Some(ctx) = self.get_context_mut(address) {
            ctx.set_io_capability(io_cap);
            ctx.set_auth_requirements(auth_req);
            ctx.state = PairingState::IoCapabilityExchange;

            return Some(commands::io_capability_request_reply(
                address,
                ctx.local_io_cap,
                ctx.local_oob,
                &ctx.local_auth_req,
            ));
        }

        None
    }

    /// Handle IO capability response from remote
    pub fn handle_io_capability_response(&mut self, params: &[u8]) -> Option<PairingMethod> {
        if params.len() < 9 {
            return None;
        }

        let address = BdAddr::from_slice(params)?;
        let io_cap = IoCapability::from_u8(params[6])?;
        let oob = match params[7] {
            0x00 => OobDataPresent::NotPresent,
            0x01 => OobDataPresent::P192Present,
            0x02 => OobDataPresent::P256Present,
            0x03 => OobDataPresent::P192AndP256Present,
            _ => OobDataPresent::NotPresent,
        };
        let auth_req = AuthRequirements::from_byte(params[8]);

        if let Some(ctx) = self.get_context_mut(&address) {
            ctx.set_remote_io_capability(io_cap, oob, auth_req);

            if let Some(method) = ctx.pairing_method {
                self.fire_event(PairingEvent::IoCapabilityExchanged {
                    address,
                    method,
                });
                return Some(method);
            }
        }

        None
    }

    /// Handle user confirmation request (numeric comparison)
    pub fn handle_user_confirmation_request(&mut self, params: &[u8]) -> Option<Vec<u8>> {
        if params.len() < 10 {
            return None;
        }

        let address = BdAddr::from_slice(params)?;
        let numeric_value = u32::from_le_bytes([params[6], params[7], params[8], params[9]]);

        // Copy values before mutable borrow
        let auto_accept = self.auto_accept_just_works;

        let mut should_auto_accept = false;
        if let Some(ctx) = self.get_context_mut(&address) {
            ctx.set_numeric_value(numeric_value);
            ctx.state = PairingState::AuthenticationStage1;

            // Check if we should auto-accept
            if auto_accept && ctx.pairing_method == Some(PairingMethod::JustWorks) {
                should_auto_accept = true;
            }
        }

        // Fire event after releasing mutable borrow
        self.fire_event(PairingEvent::NumericComparison {
            address,
            value: numeric_value,
        });

        // Auto-accept Just Works (numeric value will be 0)
        if should_auto_accept {
            return Some(commands::user_confirmation_request_reply(&address));
        }

        None
    }

    /// Handle user passkey request
    pub fn handle_user_passkey_request(&mut self, params: &[u8]) -> Option<Vec<u8>> {
        if params.len() < 6 {
            return None;
        }

        let address = BdAddr::from_slice(params)?;

        if let Some(ctx) = self.get_context_mut(&address) {
            ctx.state = PairingState::AuthenticationStage1;

            self.fire_event(PairingEvent::PasskeyRequest { address });
        }

        None
    }

    /// Handle user passkey notification (passkey to display)
    pub fn handle_user_passkey_notification(&mut self, params: &[u8]) {
        if params.len() < 10 {
            return;
        }

        if let Some(address) = BdAddr::from_slice(params) {
            let passkey = u32::from_le_bytes([params[6], params[7], params[8], params[9]]);

            if let Some(ctx) = self.get_context_mut(&address) {
                ctx.set_passkey(passkey);
            }

            self.fire_event(PairingEvent::PasskeyDisplay { address, passkey });
        }
    }

    /// Handle link key request
    pub fn handle_link_key_request(&mut self, params: &[u8]) -> Vec<u8> {
        if params.len() < 6 {
            return Vec::new();
        }

        let address = match BdAddr::from_slice(params) {
            Some(a) => a,
            None => return Vec::new(),
        };

        // Check if we have a stored link key
        let mut storage = LINK_KEY_STORAGE.lock();
        if let Some(stored) = storage.get(&address) {
            return commands::link_key_request_reply(&address, &stored.key);
        }

        // No stored key - negative reply
        commands::link_key_request_negative_reply(&address)
    }

    /// Handle link key notification (store received key)
    pub fn handle_link_key_notification(&mut self, params: &[u8]) {
        if params.len() < 23 {
            return;
        }

        let address = match BdAddr::from_slice(params) {
            Some(a) => a,
            None => return,
        };

        let mut key = [0u8; 16];
        key.copy_from_slice(&params[6..22]);
        let key_type_byte = params[22];

        let key_type = match key_type_byte {
            0x00 => LinkKeyType::Combination,
            0x01 => LinkKeyType::LocalUnit,
            0x02 => LinkKeyType::RemoteUnit,
            0x03 => LinkKeyType::DebugCombination,
            0x04 => LinkKeyType::UnauthenticatedP192,
            0x05 => LinkKeyType::AuthenticatedP192,
            0x06 => LinkKeyType::ChangedCombination,
            0x07 => LinkKeyType::UnauthenticatedP256,
            0x08 => LinkKeyType::AuthenticatedP256,
            _ => LinkKeyType::Combination,
        };

        // Determine if authenticated
        let authenticated = matches!(
            key_type,
            LinkKeyType::AuthenticatedP192 | LinkKeyType::AuthenticatedP256
        );

        // Store the key
        let mut storage = LINK_KEY_STORAGE.lock();
        storage.store(address, key, key_type, authenticated);

        // Update pairing context
        if let Some(ctx) = self.get_context_mut(&address) {
            ctx.set_link_key(key, key_type);
            ctx.state = PairingState::Bonding;
        }

        crate::kprintln!("bt: stored link key for {} (type {:?})", address.to_string(), key_type);
    }

    /// Handle PIN code request (legacy pairing)
    pub fn handle_pin_code_request(&mut self, params: &[u8]) -> Option<Vec<u8>> {
        if params.len() < 6 {
            return None;
        }

        let address = BdAddr::from_slice(params)?;

        // Copy values before mutable borrow
        let default_pin = self.default_pin;
        let default_pin_length = self.default_pin_length;

        // Create legacy pairing context if not exists
        let is_new = self.get_context(&address).is_none();
        if is_new {
            let mut ctx = PairingContext::new(address, 0, false);
            ctx.legacy_pairing = true;
            self.contexts.push(ctx);
        }

        // Fire started event if new context
        if is_new {
            self.fire_event(PairingEvent::Started {
                address,
                legacy: true,
            });
        }

        let mut use_default_pin = false;
        if let Some(ctx) = self.get_context_mut(&address) {
            ctx.legacy_pairing = true;
            ctx.state = PairingState::AuthenticationStage1;

            // If we have a default PIN, use it
            if let Some(pin) = default_pin {
                ctx.pin_code = Some(pin);
                ctx.pin_length = default_pin_length;
                use_default_pin = true;
            }
        }

        // Return PIN reply if we have a default PIN
        if use_default_pin {
            if let Some(pin) = default_pin {
                return Some(commands::pin_code_request_reply(
                    &address,
                    &pin,
                    default_pin_length as u8,
                ));
            }
        }

        // Fire request event if no default PIN
        self.fire_event(PairingEvent::PinCodeRequest { address });

        None
    }

    /// Handle Simple Pairing Complete event
    pub fn handle_simple_pairing_complete(&mut self, params: &[u8]) {
        if params.len() < 7 {
            return;
        }

        let status = params[0];
        let address = match BdAddr::from_slice(&params[1..]) {
            Some(a) => a,
            None => return,
        };

        let mut fire_failed = false;
        if let Some(ctx) = self.get_context_mut(&address) {
            if status == 0 {
                ctx.state = PairingState::LinkKeyCalculation;
            } else {
                ctx.fail(status);
                fire_failed = true;
            }
        }

        // Fire event after releasing mutable borrow
        if fire_failed {
            self.fire_event(PairingEvent::Failed {
                address,
                reason: status,
            });
        }
    }

    /// Handle Authentication Complete event
    pub fn handle_authentication_complete(&mut self, params: &[u8]) {
        if params.len() < 3 {
            return;
        }

        let status = params[0];
        let _handle = u16::from_le_bytes([params[1], params[2]]);

        // Collect event info first, then fire events
        let mut event_info: Option<(BdAddr, bool, u8)> = None; // (address, success, status/authenticated)

        for ctx in &mut self.contexts {
            if ctx.state != PairingState::Failed && ctx.state != PairingState::Complete {
                let addr = ctx.remote_address;
                if status == 0 {
                    let authenticated = ctx.has_mitm_protection();
                    ctx.complete();
                    event_info = Some((addr, true, if authenticated { 1 } else { 0 }));
                } else {
                    ctx.fail(status);
                    event_info = Some((addr, false, status));
                }
                break;
            }
        }

        // Fire events after releasing mutable borrow
        if let Some((address, success, data)) = event_info {
            if success {
                self.fire_event(PairingEvent::Success {
                    address,
                    authenticated: data != 0,
                });
            } else {
                self.fire_event(PairingEvent::Failed {
                    address,
                    reason: data,
                });
            }
        }
    }

    /// User confirms numeric comparison
    pub fn confirm_numeric_comparison(&mut self, address: &BdAddr, accept: bool) -> Option<Vec<u8>> {
        if let Some(ctx) = self.get_context_mut(address) {
            ctx.confirm_numeric(accept);
            if accept {
                return Some(commands::user_confirmation_request_reply(address));
            } else {
                return Some(commands::user_confirmation_request_negative_reply(address));
            }
        }
        None
    }

    /// User enters passkey
    pub fn enter_passkey(&mut self, address: &BdAddr, passkey: Option<u32>) -> Option<Vec<u8>> {
        if let Some(ctx) = self.get_context_mut(address) {
            if let Some(pk) = passkey {
                ctx.set_passkey(pk);
                return Some(commands::user_passkey_request_reply(address, pk));
            } else {
                return Some(commands::user_passkey_request_negative_reply(address));
            }
        }
        None
    }

    /// User enters PIN code (legacy)
    pub fn enter_pin(&mut self, address: &BdAddr, pin: Option<&[u8]>) -> Option<Vec<u8>> {
        if let Some(ctx) = self.get_context_mut(address) {
            if let Some(p) = pin {
                ctx.set_pin(p);
                return Some(commands::pin_code_request_reply(address, p, p.len() as u8));
            } else {
                return Some(commands::pin_code_request_negative_reply(address));
            }
        }
        None
    }

    /// Cancel ongoing pairing
    pub fn cancel_pairing(&mut self, address: &BdAddr) {
        if let Some(pos) = self.contexts.iter().position(|c| c.remote_address == *address) {
            self.contexts.remove(pos);
        }
    }

    /// Remove pairing (unpair device)
    pub fn unpair(&mut self, address: &BdAddr) -> bool {
        let mut storage = LINK_KEY_STORAGE.lock();
        storage.remove(address)
    }

    /// Check if device is paired
    pub fn is_paired(&self, address: &BdAddr) -> bool {
        LINK_KEY_STORAGE.lock().has_key(address)
    }

    /// Get list of paired devices
    pub fn paired_devices(&self) -> Vec<(BdAddr, bool)> {
        LINK_KEY_STORAGE.lock().paired_devices()
    }

    /// Cleanup completed/failed pairing contexts
    pub fn cleanup(&mut self) {
        self.contexts.retain(|c| {
            c.state != PairingState::Complete && c.state != PairingState::Failed
        });
    }
}

/// Global pairing manager
pub static PAIRING_MANAGER: TicketSpinlock<PairingManager> =
    TicketSpinlock::new(PairingManager::new());

// =============================================================================
// Public API
// =============================================================================

/// Initialize pairing subsystem
pub fn init() {
    crate::kprintln!("bluetooth: pairing subsystem initialized");
}

/// Start pairing with a device
pub fn pair(address: BdAddr, handle: u16) -> Result<(), &'static str> {
    PAIRING_MANAGER.lock().start_pairing(address, handle)
}

/// Check if device is paired
pub fn is_paired(address: &BdAddr) -> bool {
    PAIRING_MANAGER.lock().is_paired(address)
}

/// Unpair a device
pub fn unpair(address: &BdAddr) -> bool {
    PAIRING_MANAGER.lock().unpair(address)
}

/// Get paired devices list
pub fn paired_devices() -> Vec<(BdAddr, bool)> {
    PAIRING_MANAGER.lock().paired_devices()
}

/// Set default IO capability
pub fn set_io_capability(io_cap: IoCapability) {
    PAIRING_MANAGER.lock().set_io_capability(io_cap);
}

/// Set default authentication requirements
pub fn set_auth_requirements(auth_req: AuthRequirements) {
    PAIRING_MANAGER.lock().set_auth_requirements(auth_req);
}

/// Set auto-accept for Just Works pairing
pub fn set_auto_accept_just_works(auto: bool) {
    PAIRING_MANAGER.lock().set_auto_accept_just_works(auto);
}

/// Set default PIN for legacy pairing
pub fn set_default_pin(pin: &[u8]) {
    PAIRING_MANAGER.lock().set_default_pin(pin);
}

/// Confirm numeric comparison
pub fn confirm_numeric(address: &BdAddr, accept: bool) -> Option<Vec<u8>> {
    PAIRING_MANAGER.lock().confirm_numeric_comparison(address, accept)
}

/// Enter passkey for pairing
pub fn enter_passkey(address: &BdAddr, passkey: Option<u32>) -> Option<Vec<u8>> {
    PAIRING_MANAGER.lock().enter_passkey(address, passkey)
}

/// Enter PIN for legacy pairing
pub fn enter_pin(address: &BdAddr, pin: Option<&[u8]>) -> Option<Vec<u8>> {
    PAIRING_MANAGER.lock().enter_pin(address, pin)
}

/// Cancel ongoing pairing
pub fn cancel(address: &BdAddr) {
    PAIRING_MANAGER.lock().cancel_pairing(address);
}

/// Format pairing status
pub fn format_status() -> String {
    use core::fmt::Write;
    let mut output = String::new();

    let manager = PAIRING_MANAGER.lock();
    let storage = LINK_KEY_STORAGE.lock();

    writeln!(output, "Bluetooth Pairing:").ok();
    writeln!(output, "  IO Capability: {:?}", manager.default_io_cap).ok();
    writeln!(output, "  Auth Requirements: MITM={} Bonding={} SC={}",
        manager.default_auth_req.mitm_required,
        manager.default_auth_req.bonding,
        manager.default_auth_req.secure_connections
    ).ok();

    writeln!(output, "\nPaired Devices: {}", storage.count()).ok();
    for (addr, auth) in storage.paired_devices() {
        writeln!(output, "  {} {}", addr.to_string(),
            if auth { "[Authenticated]" } else { "[Unauthenticated]" }
        ).ok();
    }

    if !manager.contexts.is_empty() {
        writeln!(output, "\nActive Pairing:").ok();
        for ctx in &manager.contexts {
            writeln!(output, "  {} - {:?}", ctx.remote_address.to_string(), ctx.state).ok();
        }
    }

    output
}
