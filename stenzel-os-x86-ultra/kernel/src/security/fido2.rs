//! FIDO2/WebAuthn Security Key Support
//!
//! Implements CTAP2 protocol for hardware security keys.
//! Supports registration, authentication, and resident credentials.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::kprintln;

/// FIDO2 manager state
static FIDO2_MANAGER: IrqSafeMutex<Option<Fido2Manager>> = IrqSafeMutex::new(None);

/// Statistics
static STATS: Fido2Stats = Fido2Stats::new();

/// CTAP2 commands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CtapCommand {
    MakeCredential = 0x01,
    GetAssertion = 0x02,
    GetInfo = 0x04,
    ClientPin = 0x06,
    Reset = 0x07,
    GetNextAssertion = 0x08,
    BioEnrollment = 0x09,
    CredentialManagement = 0x0A,
    Selection = 0x0B,
    LargeBlobs = 0x0C,
    Config = 0x0D,
}

/// CTAP status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CtapStatus {
    Ok = 0x00,
    InvalidCommand = 0x01,
    InvalidParameter = 0x02,
    InvalidLength = 0x03,
    InvalidSeq = 0x04,
    Timeout = 0x05,
    ChannelBusy = 0x06,
    LockRequired = 0x0A,
    InvalidChannel = 0x0B,
    CborUnexpectedType = 0x11,
    InvalidCbor = 0x12,
    MissingParameter = 0x14,
    LimitExceeded = 0x15,
    UnsupportedExtension = 0x16,
    FpDatabaseFull = 0x17,
    LargeBlobStorageFull = 0x18,
    CredentialExcluded = 0x19,
    Processing = 0x21,
    InvalidCredential = 0x22,
    UserActionPending = 0x23,
    OperationPending = 0x24,
    NoOperations = 0x25,
    UnsupportedAlgorithm = 0x26,
    OperationDenied = 0x27,
    KeyStoreFull = 0x28,
    NotBusy = 0x29,
    NoOperationPending = 0x2A,
    UnsupportedOption = 0x2B,
    InvalidOption = 0x2C,
    KeepaliveCancel = 0x2D,
    NoCredentials = 0x2E,
    UserActionTimeout = 0x2F,
    NotAllowed = 0x30,
    PinInvalid = 0x31,
    PinBlocked = 0x32,
    PinAuthInvalid = 0x33,
    PinAuthBlocked = 0x34,
    PinNotSet = 0x35,
    PinRequired = 0x36,
    PinPolicyViolation = 0x37,
    PinTokenExpired = 0x38,
    RequestTooLarge = 0x39,
    ActionTimeout = 0x3A,
    UpRequired = 0x3B,
    UvBlocked = 0x3C,
    IntegrityFailure = 0x3D,
    InvalidSubcommand = 0x3E,
    UvInvalid = 0x3F,
    UnauthorizedPermission = 0x40,
    Other = 0x7F,
}

impl CtapStatus {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0x00 => Self::Ok,
            0x01 => Self::InvalidCommand,
            0x02 => Self::InvalidParameter,
            0x03 => Self::InvalidLength,
            0x04 => Self::InvalidSeq,
            0x05 => Self::Timeout,
            0x06 => Self::ChannelBusy,
            0x0A => Self::LockRequired,
            0x0B => Self::InvalidChannel,
            0x11 => Self::CborUnexpectedType,
            0x12 => Self::InvalidCbor,
            0x14 => Self::MissingParameter,
            0x15 => Self::LimitExceeded,
            0x19 => Self::CredentialExcluded,
            0x21 => Self::Processing,
            0x22 => Self::InvalidCredential,
            0x23 => Self::UserActionPending,
            0x26 => Self::UnsupportedAlgorithm,
            0x27 => Self::OperationDenied,
            0x28 => Self::KeyStoreFull,
            0x2E => Self::NoCredentials,
            0x2F => Self::UserActionTimeout,
            0x30 => Self::NotAllowed,
            0x31 => Self::PinInvalid,
            0x32 => Self::PinBlocked,
            0x33 => Self::PinAuthInvalid,
            0x34 => Self::PinAuthBlocked,
            0x35 => Self::PinNotSet,
            0x36 => Self::PinRequired,
            0x3B => Self::UpRequired,
            _ => Self::Other,
        }
    }

    pub fn is_success(&self) -> bool {
        *self == Self::Ok
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ok => "Success",
            Self::InvalidCommand => "Invalid command",
            Self::InvalidParameter => "Invalid parameter",
            Self::InvalidLength => "Invalid length",
            Self::Timeout => "Timeout",
            Self::ChannelBusy => "Channel busy",
            Self::InvalidCbor => "Invalid CBOR",
            Self::MissingParameter => "Missing parameter",
            Self::CredentialExcluded => "Credential excluded",
            Self::UserActionPending => "User action pending",
            Self::UnsupportedAlgorithm => "Unsupported algorithm",
            Self::OperationDenied => "Operation denied",
            Self::KeyStoreFull => "Key store full",
            Self::NoCredentials => "No credentials",
            Self::UserActionTimeout => "User action timeout",
            Self::NotAllowed => "Not allowed",
            Self::PinInvalid => "PIN invalid",
            Self::PinBlocked => "PIN blocked",
            Self::PinNotSet => "PIN not set",
            Self::PinRequired => "PIN required",
            Self::UpRequired => "User presence required",
            _ => "Unknown error",
        }
    }
}

/// COSE algorithm identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum CoseAlgorithm {
    Es256 = -7,      // ECDSA w/ SHA-256
    EdDsa = -8,      // EdDSA
    Es384 = -35,     // ECDSA w/ SHA-384
    Es512 = -36,     // ECDSA w/ SHA-512
    Rs256 = -257,    // RSA w/ SHA-256
    Rs384 = -258,    // RSA w/ SHA-384
    Rs512 = -259,    // RSA w/ SHA-512
    Ps256 = -37,     // RSASSA-PSS w/ SHA-256
}

impl CoseAlgorithm {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            -7 => Some(Self::Es256),
            -8 => Some(Self::EdDsa),
            -35 => Some(Self::Es384),
            -36 => Some(Self::Es512),
            -257 => Some(Self::Rs256),
            -258 => Some(Self::Rs384),
            -259 => Some(Self::Rs512),
            -37 => Some(Self::Ps256),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Es256 => "ES256",
            Self::EdDsa => "EdDSA",
            Self::Es384 => "ES384",
            Self::Es512 => "ES512",
            Self::Rs256 => "RS256",
            Self::Rs384 => "RS384",
            Self::Rs512 => "RS512",
            Self::Ps256 => "PS256",
        }
    }
}

/// Transport type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Usb,
    Nfc,
    Ble,
    Internal,
    Hybrid,
}

impl Transport {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Usb => "usb",
            Self::Nfc => "nfc",
            Self::Ble => "ble",
            Self::Internal => "internal",
            Self::Hybrid => "hybrid",
        }
    }
}

/// Authenticator attachment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attachment {
    Platform,
    CrossPlatform,
}

/// User verification requirement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserVerification {
    Required,
    Preferred,
    Discouraged,
}

/// Resident key requirement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidentKey {
    Required,
    Preferred,
    Discouraged,
}

/// Attestation preference
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attestation {
    None,
    Indirect,
    Direct,
    Enterprise,
}

/// Authenticator info (from GetInfo)
#[derive(Debug, Clone)]
pub struct AuthenticatorInfo {
    /// CTAP versions supported
    pub versions: Vec<String>,
    /// Extensions supported
    pub extensions: Vec<String>,
    /// AAGUID
    pub aaguid: [u8; 16],
    /// Options
    pub options: AuthenticatorOptions,
    /// Max message size
    pub max_msg_size: Option<u32>,
    /// PIN protocols supported
    pub pin_protocols: Vec<u8>,
    /// Max credential count in list
    pub max_cred_count_in_list: Option<u32>,
    /// Max credential ID length
    pub max_cred_id_length: Option<u32>,
    /// Transports
    pub transports: Vec<Transport>,
    /// Algorithms supported
    pub algorithms: Vec<CoseAlgorithm>,
    /// Max large blob array size
    pub max_large_blob_array: Option<u32>,
    /// Force PIN change
    pub force_pin_change: bool,
    /// Min PIN length
    pub min_pin_length: Option<u32>,
    /// Firmware version
    pub firmware_version: Option<u32>,
    /// Max cred blob length
    pub max_cred_blob_length: Option<u32>,
    /// Max RPIDs for setMinPINLength
    pub max_rpids_for_min_pin_length: Option<u32>,
    /// Preferred platform UV attempts
    pub preferred_platform_uv_attempts: Option<u32>,
    /// UV modality
    pub uv_modality: Option<u32>,
    /// Remaining discoverable credentials
    pub remaining_discoverable_credentials: Option<u32>,
}

/// Authenticator options
#[derive(Debug, Clone, Default)]
pub struct AuthenticatorOptions {
    pub platform_device: bool,
    pub resident_key: bool,
    pub client_pin: Option<bool>,
    pub user_presence: bool,
    pub user_verification: Option<bool>,
    pub pin_uv_auth_token: bool,
    pub no_mc_ga_permissions_with_client_pin: bool,
    pub large_blobs: bool,
    pub enterprise_attestation: bool,
    pub bio_enroll: Option<bool>,
    pub user_verification_mgmt_preview: Option<bool>,
    pub uv_bio_enroll: Option<bool>,
    pub authnr_cfg: bool,
    pub uv_acfg: bool,
    pub cred_mgmt: Option<bool>,
    pub cred_blob: bool,
    pub set_min_pin_length: bool,
    pub make_cred_uv_not_rqd: bool,
    pub always_uv: bool,
}

/// Relying party entity
#[derive(Debug, Clone)]
pub struct RelyingParty {
    pub id: String,
    pub name: Option<String>,
}

/// User entity
#[derive(Debug, Clone)]
pub struct UserEntity {
    pub id: Vec<u8>,
    pub name: Option<String>,
    pub display_name: Option<String>,
}

/// Public key credential parameters
#[derive(Debug, Clone)]
pub struct PubKeyCredParams {
    pub cred_type: String,
    pub alg: CoseAlgorithm,
}

/// Credential descriptor
#[derive(Debug, Clone)]
pub struct PublicKeyCredentialDescriptor {
    pub cred_type: String,
    pub id: Vec<u8>,
    pub transports: Option<Vec<Transport>>,
}

/// Make credential options
#[derive(Debug, Clone)]
pub struct MakeCredentialOptions {
    pub rp: RelyingParty,
    pub user: UserEntity,
    pub challenge: Vec<u8>,
    pub pub_key_cred_params: Vec<PubKeyCredParams>,
    pub exclude_list: Option<Vec<PublicKeyCredentialDescriptor>>,
    pub extensions: Option<BTreeMap<String, Vec<u8>>>,
    pub options: Option<CredentialOptions>,
    pub pin_auth: Option<Vec<u8>>,
    pub pin_protocol: Option<u8>,
    pub enterprise_attestation: Option<u32>,
}

/// Credential options (rk, uv)
#[derive(Debug, Clone, Default)]
pub struct CredentialOptions {
    pub rk: Option<bool>,
    pub uv: Option<bool>,
    pub up: Option<bool>,
}

/// Make credential result
#[derive(Debug, Clone)]
pub struct MakeCredentialResult {
    pub fmt: String,
    pub auth_data: Vec<u8>,
    pub att_stmt: AttestationStatement,
    pub ep_att: Option<bool>,
    pub large_blob_key: Option<Vec<u8>>,
}

/// Attestation statement
#[derive(Debug, Clone)]
pub enum AttestationStatement {
    None,
    Packed {
        alg: CoseAlgorithm,
        sig: Vec<u8>,
        x5c: Option<Vec<Vec<u8>>>,
    },
    Tpm {
        ver: String,
        alg: CoseAlgorithm,
        x5c: Vec<Vec<u8>>,
        sig: Vec<u8>,
        cert_info: Vec<u8>,
        pub_area: Vec<u8>,
    },
    FidoU2f {
        sig: Vec<u8>,
        x5c: Vec<Vec<u8>>,
    },
    AndroidKey {
        alg: CoseAlgorithm,
        sig: Vec<u8>,
        x5c: Vec<Vec<u8>>,
    },
    Apple {
        x5c: Vec<Vec<u8>>,
    },
}

/// Get assertion options
#[derive(Debug, Clone)]
pub struct GetAssertionOptions {
    pub rp_id: String,
    pub challenge: Vec<u8>,
    pub allow_list: Option<Vec<PublicKeyCredentialDescriptor>>,
    pub extensions: Option<BTreeMap<String, Vec<u8>>>,
    pub options: Option<CredentialOptions>,
    pub pin_auth: Option<Vec<u8>>,
    pub pin_protocol: Option<u8>,
}

/// Get assertion result
#[derive(Debug, Clone)]
pub struct GetAssertionResult {
    pub credential: Option<PublicKeyCredentialDescriptor>,
    pub auth_data: Vec<u8>,
    pub signature: Vec<u8>,
    pub user: Option<UserEntity>,
    pub number_of_credentials: Option<u32>,
    pub user_selected: Option<bool>,
    pub large_blob_key: Option<Vec<u8>>,
}

/// Registered authenticator
#[derive(Debug, Clone)]
pub struct RegisteredAuthenticator {
    pub id: u64,
    pub info: AuthenticatorInfo,
    pub transport: Transport,
    pub device_path: Option<String>,
    pub connected: bool,
    pub last_used: u64,
}

/// Stored credential
#[derive(Debug, Clone)]
pub struct StoredCredential {
    pub id: Vec<u8>,
    pub rp_id: String,
    pub rp_name: Option<String>,
    pub user_id: Vec<u8>,
    pub user_name: Option<String>,
    pub user_display_name: Option<String>,
    pub public_key: Vec<u8>,
    pub algorithm: CoseAlgorithm,
    pub sign_count: u32,
    pub created_at: u64,
    pub last_used: Option<u64>,
    pub transports: Vec<Transport>,
    pub backup_eligible: bool,
    pub backup_state: bool,
    pub authenticator_id: u64,
}

/// FIDO2 error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fido2Error {
    NotInitialized,
    NoAuthenticator,
    AuthenticatorNotFound,
    CtapError(CtapStatus),
    Timeout,
    Cancelled,
    UserNotPresent,
    PinRequired,
    PinInvalid,
    PinBlocked,
    CredentialNotFound,
    InvalidParameter,
    UnsupportedAlgorithm,
    TransportError,
    CborError,
    InternalError,
}

impl Fido2Error {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotInitialized => "Not initialized",
            Self::NoAuthenticator => "No authenticator",
            Self::AuthenticatorNotFound => "Authenticator not found",
            Self::CtapError(s) => s.as_str(),
            Self::Timeout => "Timeout",
            Self::Cancelled => "Cancelled",
            Self::UserNotPresent => "User not present",
            Self::PinRequired => "PIN required",
            Self::PinInvalid => "PIN invalid",
            Self::PinBlocked => "PIN blocked",
            Self::CredentialNotFound => "Credential not found",
            Self::InvalidParameter => "Invalid parameter",
            Self::UnsupportedAlgorithm => "Unsupported algorithm",
            Self::TransportError => "Transport error",
            Self::CborError => "CBOR error",
            Self::InternalError => "Internal error",
        }
    }
}

pub type Fido2Result<T> = Result<T, Fido2Error>;

/// Statistics
pub struct Fido2Stats {
    authenticators_registered: AtomicU64,
    registrations: AtomicU64,
    authentications: AtomicU64,
    failed_authentications: AtomicU64,
    pin_attempts: AtomicU64,
    timeouts: AtomicU64,
}

impl Fido2Stats {
    const fn new() -> Self {
        Self {
            authenticators_registered: AtomicU64::new(0),
            registrations: AtomicU64::new(0),
            authentications: AtomicU64::new(0),
            failed_authentications: AtomicU64::new(0),
            pin_attempts: AtomicU64::new(0),
            timeouts: AtomicU64::new(0),
        }
    }
}

/// FIDO2 Manager
pub struct Fido2Manager {
    /// Registered authenticators
    authenticators: Vec<RegisteredAuthenticator>,
    /// Next authenticator ID
    next_auth_id: u64,
    /// Stored credentials
    credentials: Vec<StoredCredential>,
    /// Preferred algorithms
    preferred_algorithms: Vec<CoseAlgorithm>,
    /// Default timeout (ms)
    default_timeout: u32,
    /// Allow platform authenticators
    allow_platform: bool,
    /// Allow cross-platform authenticators
    allow_cross_platform: bool,
    /// Require user verification
    require_uv: bool,
    /// Prefer resident keys
    prefer_resident: bool,
    /// Current operation
    current_operation: Option<CurrentOperation>,
    /// PIN retry count
    pin_retries: u8,
}

/// Current operation state
#[derive(Debug)]
pub struct CurrentOperation {
    pub op_type: OperationType,
    pub rp_id: String,
    pub challenge: Vec<u8>,
    pub started_at: u64,
    pub timeout_at: u64,
    pub authenticator_id: Option<u64>,
    pub user_presence_confirmed: bool,
    pub user_verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Registration,
    Authentication,
    PinChange,
    Reset,
}

impl Fido2Manager {
    fn new() -> Self {
        Self {
            authenticators: Vec::new(),
            next_auth_id: 1,
            credentials: Vec::new(),
            preferred_algorithms: vec![
                CoseAlgorithm::Es256,
                CoseAlgorithm::EdDsa,
                CoseAlgorithm::Rs256,
            ],
            default_timeout: 60000, // 60 seconds
            allow_platform: true,
            allow_cross_platform: true,
            require_uv: false,
            prefer_resident: false,
            current_operation: None,
            pin_retries: 8,
        }
    }

    /// Register an authenticator
    pub fn register_authenticator(
        &mut self,
        transport: Transport,
        device_path: Option<String>,
    ) -> Fido2Result<u64> {
        kprintln!("fido2: Registering authenticator via {:?}", transport);

        // Get authenticator info
        let info = self.get_authenticator_info_internal(transport)?;

        let auth_id = self.next_auth_id;
        self.next_auth_id += 1;

        let auth = RegisteredAuthenticator {
            id: auth_id,
            info,
            transport,
            device_path,
            connected: true,
            last_used: crate::time::uptime_ms(),
        };

        self.authenticators.push(auth);
        STATS.authenticators_registered.fetch_add(1, Ordering::Relaxed);

        kprintln!("fido2: Registered authenticator {}", auth_id);
        Ok(auth_id)
    }

    /// Get authenticator info
    fn get_authenticator_info_internal(&self, _transport: Transport) -> Fido2Result<AuthenticatorInfo> {
        // This would communicate with the actual authenticator
        // Placeholder implementation
        Ok(AuthenticatorInfo {
            versions: vec!["FIDO_2_0".to_string(), "FIDO_2_1".to_string()],
            extensions: vec!["credProtect".to_string(), "hmac-secret".to_string()],
            aaguid: [0u8; 16],
            options: AuthenticatorOptions {
                platform_device: false,
                resident_key: true,
                client_pin: Some(true),
                user_presence: true,
                user_verification: Some(true),
                pin_uv_auth_token: true,
                large_blobs: false,
                cred_mgmt: Some(true),
                ..Default::default()
            },
            max_msg_size: Some(1200),
            pin_protocols: vec![2, 1],
            max_cred_count_in_list: Some(8),
            max_cred_id_length: Some(128),
            transports: vec![Transport::Usb],
            algorithms: vec![CoseAlgorithm::Es256, CoseAlgorithm::EdDsa],
            max_large_blob_array: None,
            force_pin_change: false,
            min_pin_length: Some(4),
            firmware_version: Some(0x00050000),
            max_cred_blob_length: Some(32),
            max_rpids_for_min_pin_length: Some(5),
            preferred_platform_uv_attempts: Some(3),
            uv_modality: None,
            remaining_discoverable_credentials: Some(25),
        })
    }

    /// Unregister an authenticator
    pub fn unregister_authenticator(&mut self, auth_id: u64) -> Fido2Result<()> {
        let idx = self.authenticators.iter()
            .position(|a| a.id == auth_id)
            .ok_or(Fido2Error::AuthenticatorNotFound)?;

        self.authenticators.remove(idx);
        kprintln!("fido2: Unregistered authenticator {}", auth_id);
        Ok(())
    }

    /// List connected authenticators
    pub fn list_authenticators(&self) -> &[RegisteredAuthenticator] {
        &self.authenticators
    }

    /// Get authenticator by ID
    pub fn get_authenticator(&self, auth_id: u64) -> Option<&RegisteredAuthenticator> {
        self.authenticators.iter().find(|a| a.id == auth_id)
    }

    /// Make credential (registration)
    pub fn make_credential(
        &mut self,
        options: MakeCredentialOptions,
        auth_id: Option<u64>,
    ) -> Fido2Result<MakeCredentialResult> {
        kprintln!("fido2: MakeCredential for RP: {}", options.rp.id);

        // Find authenticator
        let auth = if let Some(id) = auth_id {
            self.authenticators.iter()
                .find(|a| a.id == id && a.connected)
                .ok_or(Fido2Error::AuthenticatorNotFound)?
        } else {
            self.authenticators.iter()
                .find(|a| a.connected)
                .ok_or(Fido2Error::NoAuthenticator)?
        };

        // Check if credential already exists (exclude list)
        if let Some(ref exclude) = options.exclude_list {
            for desc in exclude {
                if self.credentials.iter().any(|c| c.id == desc.id && c.rp_id == options.rp.id) {
                    return Err(Fido2Error::CtapError(CtapStatus::CredentialExcluded));
                }
            }
        }

        // Select algorithm
        let algorithm = options.pub_key_cred_params.iter()
            .find(|p| auth.info.algorithms.contains(&p.alg))
            .map(|p| p.alg)
            .ok_or(Fido2Error::UnsupportedAlgorithm)?;

        // Start operation
        self.current_operation = Some(CurrentOperation {
            op_type: OperationType::Registration,
            rp_id: options.rp.id.clone(),
            challenge: options.challenge.clone(),
            started_at: crate::time::uptime_ms(),
            timeout_at: crate::time::uptime_ms() + self.default_timeout as u64,
            authenticator_id: Some(auth.id),
            user_presence_confirmed: false,
            user_verified: false,
        });

        // Generate credential (placeholder)
        let credential_id = self.generate_credential_id();
        let (public_key, auth_data) = self.generate_credential_key(
            &options.rp.id,
            &credential_id,
            algorithm,
        );

        // Store credential
        let credential = StoredCredential {
            id: credential_id.clone(),
            rp_id: options.rp.id.clone(),
            rp_name: options.rp.name.clone(),
            user_id: options.user.id.clone(),
            user_name: options.user.name.clone(),
            user_display_name: options.user.display_name.clone(),
            public_key,
            algorithm,
            sign_count: 0,
            created_at: crate::time::uptime_ms(),
            last_used: None,
            transports: vec![auth.transport],
            backup_eligible: false,
            backup_state: false,
            authenticator_id: auth.id,
        };

        self.credentials.push(credential);
        self.current_operation = None;

        STATS.registrations.fetch_add(1, Ordering::Relaxed);

        // Create attestation
        let att_stmt = AttestationStatement::Packed {
            alg: algorithm,
            sig: self.sign_attestation(&auth_data, &options.challenge),
            x5c: None, // Self-attestation
        };

        kprintln!("fido2: Credential created successfully");

        Ok(MakeCredentialResult {
            fmt: "packed".to_string(),
            auth_data,
            att_stmt,
            ep_att: None,
            large_blob_key: None,
        })
    }

    /// Get assertion (authentication)
    pub fn get_assertion(
        &mut self,
        options: GetAssertionOptions,
        auth_id: Option<u64>,
    ) -> Fido2Result<GetAssertionResult> {
        kprintln!("fido2: GetAssertion for RP: {}", options.rp_id);

        // Find matching credentials and extract needed data
        let (num_credentials, cred_id, user_entity, sign_count, auth_transport) = {
            let matching_creds: Vec<_> = if let Some(ref allow) = options.allow_list {
                // Match by credential ID
                self.credentials.iter()
                    .filter(|c| c.rp_id == options.rp_id && allow.iter().any(|d| d.id == c.id))
                    .collect()
            } else {
                // Discoverable credentials for this RP
                self.credentials.iter()
                    .filter(|c| c.rp_id == options.rp_id)
                    .collect()
            };

            if matching_creds.is_empty() {
                STATS.failed_authentications.fetch_add(1, Ordering::Relaxed);
                return Err(Fido2Error::CredentialNotFound);
            }

            // Find authenticator
            let auth = if let Some(id) = auth_id {
                self.authenticators.iter()
                    .find(|a| a.id == id && a.connected)
                    .ok_or(Fido2Error::AuthenticatorNotFound)?
            } else {
                self.authenticators.iter()
                    .find(|a| a.connected)
                    .ok_or(Fido2Error::NoAuthenticator)?
            };

            // Use first matching credential
            let cred = matching_creds[0];

            let num_creds = matching_creds.len() as u32;
            let cred_id = cred.id.clone();
            let user_entity = UserEntity {
                id: cred.user_id.clone(),
                name: cred.user_name.clone(),
                display_name: cred.user_display_name.clone(),
            };
            let sign_count = cred.sign_count;
            let auth_transport = auth.transport;
            let auth_id_val = auth.id;

            // Start operation
            self.current_operation = Some(CurrentOperation {
                op_type: OperationType::Authentication,
                rp_id: options.rp_id.clone(),
                challenge: options.challenge.clone(),
                started_at: crate::time::uptime_ms(),
                timeout_at: crate::time::uptime_ms() + self.default_timeout as u64,
                authenticator_id: Some(auth_id_val),
                user_presence_confirmed: false,
                user_verified: false,
            });

            (num_creds, cred_id, user_entity, sign_count, auth_transport)
        };

        // Generate authenticator data
        let auth_data = self.generate_auth_data(&options.rp_id, sign_count + 1, false);

        // Sign (placeholder - would need credential private key)
        let mut signature = Vec::new();
        signature.extend_from_slice(&auth_data);
        signature.extend_from_slice(&options.challenge);
        signature.resize(64, 0);

        self.current_operation = None;

        // Update sign count
        if let Some(c) = self.credentials.iter_mut().find(|c| c.id == cred_id) {
            c.sign_count += 1;
            c.last_used = Some(crate::time::uptime_ms());
        }

        STATS.authentications.fetch_add(1, Ordering::Relaxed);

        kprintln!("fido2: Assertion successful");

        Ok(GetAssertionResult {
            credential: Some(PublicKeyCredentialDescriptor {
                cred_type: "public-key".to_string(),
                id: cred_id,
                transports: Some(vec![auth_transport]),
            }),
            auth_data,
            signature,
            user: Some(user_entity),
            number_of_credentials: Some(num_credentials),
            user_selected: Some(false),
            large_blob_key: None,
        })
    }

    /// Set/change PIN
    pub fn set_pin(&mut self, auth_id: u64, new_pin: &str) -> Fido2Result<()> {
        kprintln!("fido2: Setting PIN for authenticator {}", auth_id);

        let _auth = self.authenticators.iter()
            .find(|a| a.id == auth_id && a.connected)
            .ok_or(Fido2Error::AuthenticatorNotFound)?;

        if new_pin.len() < 4 {
            return Err(Fido2Error::InvalidParameter);
        }

        // Would send ClientPIN command to authenticator
        STATS.pin_attempts.fetch_add(1, Ordering::Relaxed);

        kprintln!("fido2: PIN set successfully");
        Ok(())
    }

    /// Verify PIN
    pub fn verify_pin(&mut self, auth_id: u64, pin: &str) -> Fido2Result<Vec<u8>> {
        kprintln!("fido2: Verifying PIN for authenticator {}", auth_id);

        let _auth = self.authenticators.iter()
            .find(|a| a.id == auth_id && a.connected)
            .ok_or(Fido2Error::AuthenticatorNotFound)?;

        if pin.is_empty() {
            return Err(Fido2Error::PinInvalid);
        }

        STATS.pin_attempts.fetch_add(1, Ordering::Relaxed);

        // Would verify PIN and get pinUvAuthToken
        // Placeholder: return dummy token
        Ok(vec![0u8; 32])
    }

    /// Reset authenticator
    pub fn reset(&mut self, auth_id: u64) -> Fido2Result<()> {
        kprintln!("fido2: Resetting authenticator {}", auth_id);

        let _auth = self.authenticators.iter()
            .find(|a| a.id == auth_id && a.connected)
            .ok_or(Fido2Error::AuthenticatorNotFound)?;

        // Remove credentials for this authenticator
        self.credentials.retain(|c| c.authenticator_id != auth_id);

        kprintln!("fido2: Authenticator reset");
        Ok(())
    }

    /// List credentials for an RP
    pub fn list_credentials(&self, rp_id: &str) -> Vec<&StoredCredential> {
        self.credentials.iter()
            .filter(|c| c.rp_id == rp_id)
            .collect()
    }

    /// List all RPs with credentials
    pub fn list_rps(&self) -> Vec<String> {
        let mut rps: Vec<String> = self.credentials.iter()
            .map(|c| c.rp_id.clone())
            .collect();
        rps.sort();
        rps.dedup();
        rps
    }

    /// Delete a credential
    pub fn delete_credential(&mut self, credential_id: &[u8]) -> Fido2Result<()> {
        let idx = self.credentials.iter()
            .position(|c| c.id == credential_id)
            .ok_or(Fido2Error::CredentialNotFound)?;

        self.credentials.remove(idx);
        kprintln!("fido2: Deleted credential");
        Ok(())
    }

    /// Cancel current operation
    pub fn cancel(&mut self) {
        if self.current_operation.is_some() {
            kprintln!("fido2: Operation cancelled");
            self.current_operation = None;
        }
    }

    /// Get statistics
    pub fn get_stats(&self) -> (u64, u64, u64, u64, u64) {
        (
            STATS.authenticators_registered.load(Ordering::Relaxed),
            STATS.registrations.load(Ordering::Relaxed),
            STATS.authentications.load(Ordering::Relaxed),
            STATS.failed_authentications.load(Ordering::Relaxed),
            STATS.timeouts.load(Ordering::Relaxed),
        )
    }

    // Internal helpers

    fn generate_credential_id(&self) -> Vec<u8> {
        // Would use cryptographic RNG
        let mut id = vec![0u8; 32];
        for (i, byte) in id.iter_mut().enumerate() {
            *byte = (i * 17 + 42) as u8;
        }
        id
    }

    fn generate_credential_key(
        &self,
        rp_id: &str,
        credential_id: &[u8],
        algorithm: CoseAlgorithm,
    ) -> (Vec<u8>, Vec<u8>) {
        // Generate public key (placeholder)
        let public_key = match algorithm {
            CoseAlgorithm::Es256 => vec![0x04; 65], // Uncompressed EC point
            CoseAlgorithm::EdDsa => vec![0u8; 32],
            _ => vec![0u8; 256], // RSA
        };

        // Generate authenticator data
        let auth_data = self.generate_auth_data(rp_id, 0, true);

        // Would include attested credential data
        let mut full_auth_data = auth_data;
        full_auth_data.extend_from_slice(credential_id);
        full_auth_data.extend_from_slice(&public_key);

        (public_key, full_auth_data)
    }

    fn generate_auth_data(&self, rp_id: &str, sign_count: u32, attested: bool) -> Vec<u8> {
        let mut data = Vec::new();

        // RP ID hash (SHA-256)
        let rp_hash = self.sha256(rp_id.as_bytes());
        data.extend_from_slice(&rp_hash);

        // Flags
        let mut flags: u8 = 0x01; // UP
        if attested {
            flags |= 0x40; // AT
        }
        data.push(flags);

        // Sign count
        data.extend_from_slice(&sign_count.to_be_bytes());

        data
    }

    fn sha256(&self, data: &[u8]) -> [u8; 32] {
        // Placeholder SHA-256
        let mut hash = [0u8; 32];
        for (i, byte) in data.iter().enumerate() {
            hash[i % 32] ^= byte;
        }
        hash
    }

    fn sign_attestation(&self, auth_data: &[u8], challenge: &[u8]) -> Vec<u8> {
        // Would sign with attestation key
        let mut sig = Vec::new();
        sig.extend_from_slice(auth_data);
        sig.extend_from_slice(challenge);
        // Placeholder signature
        sig.resize(64, 0);
        sig
    }

    fn sign_assertion(
        &self,
        auth_data: &[u8],
        challenge: &[u8],
        _credential: &StoredCredential,
    ) -> Vec<u8> {
        // Would sign with credential private key
        let mut sig = Vec::new();
        sig.extend_from_slice(auth_data);
        sig.extend_from_slice(challenge);
        // Placeholder signature
        sig.resize(64, 0);
        sig
    }
}

// Public API

/// Initialize FIDO2 subsystem
pub fn init() {
    let mut guard = FIDO2_MANAGER.lock();
    if guard.is_none() {
        *guard = Some(Fido2Manager::new());
        kprintln!("fido2: Initialized");
    }
}

/// Register an authenticator
pub fn register_authenticator(transport: Transport, device_path: Option<String>) -> Fido2Result<u64> {
    let mut guard = FIDO2_MANAGER.lock();
    let manager = guard.as_mut().ok_or(Fido2Error::NotInitialized)?;
    manager.register_authenticator(transport, device_path)
}

/// Unregister an authenticator
pub fn unregister_authenticator(auth_id: u64) -> Fido2Result<()> {
    let mut guard = FIDO2_MANAGER.lock();
    let manager = guard.as_mut().ok_or(Fido2Error::NotInitialized)?;
    manager.unregister_authenticator(auth_id)
}

/// List authenticators
pub fn list_authenticators() -> Vec<RegisteredAuthenticator> {
    let guard = FIDO2_MANAGER.lock();
    guard.as_ref()
        .map(|m| m.authenticators.clone())
        .unwrap_or_default()
}

/// Make credential (register)
pub fn make_credential(
    options: MakeCredentialOptions,
    auth_id: Option<u64>,
) -> Fido2Result<MakeCredentialResult> {
    let mut guard = FIDO2_MANAGER.lock();
    let manager = guard.as_mut().ok_or(Fido2Error::NotInitialized)?;
    manager.make_credential(options, auth_id)
}

/// Get assertion (authenticate)
pub fn get_assertion(
    options: GetAssertionOptions,
    auth_id: Option<u64>,
) -> Fido2Result<GetAssertionResult> {
    let mut guard = FIDO2_MANAGER.lock();
    let manager = guard.as_mut().ok_or(Fido2Error::NotInitialized)?;
    manager.get_assertion(options, auth_id)
}

/// Set PIN
pub fn set_pin(auth_id: u64, new_pin: &str) -> Fido2Result<()> {
    let mut guard = FIDO2_MANAGER.lock();
    let manager = guard.as_mut().ok_or(Fido2Error::NotInitialized)?;
    manager.set_pin(auth_id, new_pin)
}

/// Verify PIN
pub fn verify_pin(auth_id: u64, pin: &str) -> Fido2Result<Vec<u8>> {
    let mut guard = FIDO2_MANAGER.lock();
    let manager = guard.as_mut().ok_or(Fido2Error::NotInitialized)?;
    manager.verify_pin(auth_id, pin)
}

/// Reset authenticator
pub fn reset_authenticator(auth_id: u64) -> Fido2Result<()> {
    let mut guard = FIDO2_MANAGER.lock();
    let manager = guard.as_mut().ok_or(Fido2Error::NotInitialized)?;
    manager.reset(auth_id)
}

/// List credentials for RP
pub fn list_credentials(rp_id: &str) -> Vec<StoredCredential> {
    let guard = FIDO2_MANAGER.lock();
    guard.as_ref()
        .map(|m| m.list_credentials(rp_id).into_iter().cloned().collect())
        .unwrap_or_default()
}

/// List RPs with credentials
pub fn list_rps() -> Vec<String> {
    let guard = FIDO2_MANAGER.lock();
    guard.as_ref()
        .map(|m| m.list_rps())
        .unwrap_or_default()
}

/// Delete credential
pub fn delete_credential(credential_id: &[u8]) -> Fido2Result<()> {
    let mut guard = FIDO2_MANAGER.lock();
    let manager = guard.as_mut().ok_or(Fido2Error::NotInitialized)?;
    manager.delete_credential(credential_id)
}

/// Cancel current operation
pub fn cancel() {
    let mut guard = FIDO2_MANAGER.lock();
    if let Some(manager) = guard.as_mut() {
        manager.cancel();
    }
}

/// Get statistics
pub fn get_stats() -> (u64, u64, u64, u64, u64) {
    let guard = FIDO2_MANAGER.lock();
    guard.as_ref()
        .map(|m| m.get_stats())
        .unwrap_or((0, 0, 0, 0, 0))
}
