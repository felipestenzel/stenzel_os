//! IPsec/IKEv2 Implementation
//!
//! Internet Protocol Security with IKEv2 key exchange.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;

/// IKE version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IkeVersion {
    V1,
    V2,
}

impl IkeVersion {
    pub fn name(&self) -> &'static str {
        match self {
            IkeVersion::V1 => "IKEv1",
            IkeVersion::V2 => "IKEv2",
        }
    }
}

/// Encryption algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionAlgorithm {
    Aes128,
    Aes192,
    Aes256,
    Aes128Gcm16,
    Aes256Gcm16,
    ChaCha20Poly1305,
    Des3,
    Null,
}

impl EncryptionAlgorithm {
    pub fn name(&self) -> &'static str {
        match self {
            EncryptionAlgorithm::Aes128 => "AES-128",
            EncryptionAlgorithm::Aes192 => "AES-192",
            EncryptionAlgorithm::Aes256 => "AES-256",
            EncryptionAlgorithm::Aes128Gcm16 => "AES-128-GCM",
            EncryptionAlgorithm::Aes256Gcm16 => "AES-256-GCM",
            EncryptionAlgorithm::ChaCha20Poly1305 => "CHACHA20-POLY1305",
            EncryptionAlgorithm::Des3 => "3DES",
            EncryptionAlgorithm::Null => "NULL",
        }
    }

    pub fn key_size(&self) -> usize {
        match self {
            EncryptionAlgorithm::Aes128 | EncryptionAlgorithm::Aes128Gcm16 => 16,
            EncryptionAlgorithm::Aes192 => 24,
            EncryptionAlgorithm::Aes256 | EncryptionAlgorithm::Aes256Gcm16 |
            EncryptionAlgorithm::ChaCha20Poly1305 => 32,
            EncryptionAlgorithm::Des3 => 24,
            EncryptionAlgorithm::Null => 0,
        }
    }

    pub fn is_aead(&self) -> bool {
        matches!(self,
            EncryptionAlgorithm::Aes128Gcm16 |
            EncryptionAlgorithm::Aes256Gcm16 |
            EncryptionAlgorithm::ChaCha20Poly1305)
    }
}

/// Integrity algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrityAlgorithm {
    HmacSha1_96,
    HmacSha256_128,
    HmacSha384_192,
    HmacSha512_256,
    Aes128Gmac,
    Aes256Gmac,
    None,
}

impl IntegrityAlgorithm {
    pub fn name(&self) -> &'static str {
        match self {
            IntegrityAlgorithm::HmacSha1_96 => "HMAC-SHA1-96",
            IntegrityAlgorithm::HmacSha256_128 => "HMAC-SHA256-128",
            IntegrityAlgorithm::HmacSha384_192 => "HMAC-SHA384-192",
            IntegrityAlgorithm::HmacSha512_256 => "HMAC-SHA512-256",
            IntegrityAlgorithm::Aes128Gmac => "AES-128-GMAC",
            IntegrityAlgorithm::Aes256Gmac => "AES-256-GMAC",
            IntegrityAlgorithm::None => "NONE",
        }
    }

    pub fn digest_size(&self) -> usize {
        match self {
            IntegrityAlgorithm::HmacSha1_96 => 12,
            IntegrityAlgorithm::HmacSha256_128 | IntegrityAlgorithm::Aes128Gmac => 16,
            IntegrityAlgorithm::HmacSha384_192 => 24,
            IntegrityAlgorithm::HmacSha512_256 | IntegrityAlgorithm::Aes256Gmac => 32,
            IntegrityAlgorithm::None => 0,
        }
    }
}

/// Diffie-Hellman group
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DhGroup {
    Modp768,    // Group 1
    Modp1024,   // Group 2
    Modp1536,   // Group 5
    Modp2048,   // Group 14
    Modp3072,   // Group 15
    Modp4096,   // Group 16
    Modp6144,   // Group 17
    Modp8192,   // Group 18
    Ecp256,     // Group 19
    Ecp384,     // Group 20
    Ecp521,     // Group 21
    Curve25519, // Group 31
    Curve448,   // Group 32
}

impl DhGroup {
    pub fn name(&self) -> &'static str {
        match self {
            DhGroup::Modp768 => "MODP-768",
            DhGroup::Modp1024 => "MODP-1024",
            DhGroup::Modp1536 => "MODP-1536",
            DhGroup::Modp2048 => "MODP-2048",
            DhGroup::Modp3072 => "MODP-3072",
            DhGroup::Modp4096 => "MODP-4096",
            DhGroup::Modp6144 => "MODP-6144",
            DhGroup::Modp8192 => "MODP-8192",
            DhGroup::Ecp256 => "ECP-256",
            DhGroup::Ecp384 => "ECP-384",
            DhGroup::Ecp521 => "ECP-521",
            DhGroup::Curve25519 => "CURVE25519",
            DhGroup::Curve448 => "CURVE448",
        }
    }

    pub fn group_number(&self) -> u16 {
        match self {
            DhGroup::Modp768 => 1,
            DhGroup::Modp1024 => 2,
            DhGroup::Modp1536 => 5,
            DhGroup::Modp2048 => 14,
            DhGroup::Modp3072 => 15,
            DhGroup::Modp4096 => 16,
            DhGroup::Modp6144 => 17,
            DhGroup::Modp8192 => 18,
            DhGroup::Ecp256 => 19,
            DhGroup::Ecp384 => 20,
            DhGroup::Ecp521 => 21,
            DhGroup::Curve25519 => 31,
            DhGroup::Curve448 => 32,
        }
    }

    pub fn key_size(&self) -> usize {
        match self {
            DhGroup::Modp768 => 96,
            DhGroup::Modp1024 => 128,
            DhGroup::Modp1536 => 192,
            DhGroup::Modp2048 => 256,
            DhGroup::Modp3072 => 384,
            DhGroup::Modp4096 => 512,
            DhGroup::Modp6144 => 768,
            DhGroup::Modp8192 => 1024,
            DhGroup::Ecp256 | DhGroup::Curve25519 => 32,
            DhGroup::Ecp384 => 48,
            DhGroup::Ecp521 => 66,
            DhGroup::Curve448 => 56,
        }
    }
}

/// PRF algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrfAlgorithm {
    HmacSha1,
    HmacSha256,
    HmacSha384,
    HmacSha512,
}

impl PrfAlgorithm {
    pub fn name(&self) -> &'static str {
        match self {
            PrfAlgorithm::HmacSha1 => "HMAC-SHA1",
            PrfAlgorithm::HmacSha256 => "HMAC-SHA256",
            PrfAlgorithm::HmacSha384 => "HMAC-SHA384",
            PrfAlgorithm::HmacSha512 => "HMAC-SHA512",
        }
    }
}

/// Authentication method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    Psk,              // Pre-shared key
    RsaSig,           // RSA signature
    EcdsaSig256,      // ECDSA with SHA-256
    EcdsaSig384,      // ECDSA with SHA-384
    EcdsaSig512,      // ECDSA with SHA-512
    Eap,              // EAP (Extensible Authentication Protocol)
}

impl AuthMethod {
    pub fn name(&self) -> &'static str {
        match self {
            AuthMethod::Psk => "PSK",
            AuthMethod::RsaSig => "RSA",
            AuthMethod::EcdsaSig256 => "ECDSA-256",
            AuthMethod::EcdsaSig384 => "ECDSA-384",
            AuthMethod::EcdsaSig512 => "ECDSA-512",
            AuthMethod::Eap => "EAP",
        }
    }
}

/// IPsec protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpsecProtocol {
    Esp,  // Encapsulating Security Payload
    Ah,   // Authentication Header
}

impl IpsecProtocol {
    pub fn name(&self) -> &'static str {
        match self {
            IpsecProtocol::Esp => "ESP",
            IpsecProtocol::Ah => "AH",
        }
    }

    pub fn protocol_number(&self) -> u8 {
        match self {
            IpsecProtocol::Esp => 50,
            IpsecProtocol::Ah => 51,
        }
    }
}

/// IPsec mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpsecMode {
    Transport,
    Tunnel,
}

impl IpsecMode {
    pub fn name(&self) -> &'static str {
        match self {
            IpsecMode::Transport => "Transport",
            IpsecMode::Tunnel => "Tunnel",
        }
    }
}

/// IKE SA state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IkeSaState {
    Idle,
    InitSent,
    InitReceived,
    AuthSent,
    AuthReceived,
    Established,
    RekeyInit,
    RekeySent,
    Deleting,
    Deleted,
}

impl IkeSaState {
    pub fn name(&self) -> &'static str {
        match self {
            IkeSaState::Idle => "Idle",
            IkeSaState::InitSent => "Init Sent",
            IkeSaState::InitReceived => "Init Received",
            IkeSaState::AuthSent => "Auth Sent",
            IkeSaState::AuthReceived => "Auth Received",
            IkeSaState::Established => "Established",
            IkeSaState::RekeyInit => "Rekey Init",
            IkeSaState::RekeySent => "Rekey Sent",
            IkeSaState::Deleting => "Deleting",
            IkeSaState::Deleted => "Deleted",
        }
    }

    pub fn is_established(&self) -> bool {
        matches!(self, IkeSaState::Established)
    }
}

/// Child SA state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildSaState {
    Inactive,
    Creating,
    Established,
    Rekeying,
    Deleting,
    Deleted,
}

impl ChildSaState {
    pub fn name(&self) -> &'static str {
        match self {
            ChildSaState::Inactive => "Inactive",
            ChildSaState::Creating => "Creating",
            ChildSaState::Established => "Established",
            ChildSaState::Rekeying => "Rekeying",
            ChildSaState::Deleting => "Deleting",
            ChildSaState::Deleted => "Deleted",
        }
    }
}

/// IKE exchange type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeType {
    IkeSaInit = 34,
    IkeAuth = 35,
    CreateChildSa = 36,
    Informational = 37,
}

impl ExchangeType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            34 => Some(ExchangeType::IkeSaInit),
            35 => Some(ExchangeType::IkeAuth),
            36 => Some(ExchangeType::CreateChildSa),
            37 => Some(ExchangeType::Informational),
            _ => None,
        }
    }
}

/// IKE payload type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadType {
    NoNextPayload = 0,
    SecurityAssociation = 33,
    KeyExchange = 34,
    IdInitiator = 35,
    IdResponder = 36,
    Certificate = 37,
    CertificateRequest = 38,
    Authentication = 39,
    Nonce = 40,
    Notify = 41,
    Delete = 42,
    VendorId = 43,
    TrafficSelectorInit = 44,
    TrafficSelectorResp = 45,
    EncryptedAndAuthenticated = 46,
    Configuration = 47,
    Eap = 48,
}

impl PayloadType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(PayloadType::NoNextPayload),
            33 => Some(PayloadType::SecurityAssociation),
            34 => Some(PayloadType::KeyExchange),
            35 => Some(PayloadType::IdInitiator),
            36 => Some(PayloadType::IdResponder),
            37 => Some(PayloadType::Certificate),
            38 => Some(PayloadType::CertificateRequest),
            39 => Some(PayloadType::Authentication),
            40 => Some(PayloadType::Nonce),
            41 => Some(PayloadType::Notify),
            42 => Some(PayloadType::Delete),
            43 => Some(PayloadType::VendorId),
            44 => Some(PayloadType::TrafficSelectorInit),
            45 => Some(PayloadType::TrafficSelectorResp),
            46 => Some(PayloadType::EncryptedAndAuthenticated),
            47 => Some(PayloadType::Configuration),
            48 => Some(PayloadType::Eap),
            _ => None,
        }
    }
}

/// IKE header
#[derive(Debug, Clone)]
pub struct IkeHeader {
    pub initiator_spi: [u8; 8],
    pub responder_spi: [u8; 8],
    pub next_payload: PayloadType,
    pub version: u8,
    pub exchange_type: ExchangeType,
    pub flags: u8,
    pub message_id: u32,
    pub length: u32,
}

impl IkeHeader {
    pub const SIZE: usize = 28;

    pub fn new(exchange_type: ExchangeType) -> Self {
        Self {
            initiator_spi: [0; 8],
            responder_spi: [0; 8],
            next_payload: PayloadType::NoNextPayload,
            version: 0x20, // IKEv2
            exchange_type,
            flags: 0,
            message_id: 0,
            length: Self::SIZE as u32,
        }
    }

    pub fn is_initiator(&self) -> bool {
        (self.flags & 0x08) != 0
    }

    pub fn is_response(&self) -> bool {
        (self.flags & 0x20) != 0
    }
}

/// Traffic selector
#[derive(Debug, Clone)]
pub struct TrafficSelector {
    pub ts_type: u8,
    pub ip_protocol: u8,
    pub start_port: u16,
    pub end_port: u16,
    pub start_address: Vec<u8>,
    pub end_address: Vec<u8>,
}

impl TrafficSelector {
    pub fn ipv4_any() -> Self {
        Self {
            ts_type: 7, // TS_IPV4_ADDR_RANGE
            ip_protocol: 0, // Any
            start_port: 0,
            end_port: 65535,
            start_address: vec![0, 0, 0, 0],
            end_address: vec![255, 255, 255, 255],
        }
    }

    pub fn ipv4_subnet(addr: [u8; 4], prefix: u8) -> Self {
        let mask = if prefix >= 32 {
            0xFFFFFFFF
        } else {
            !((1u32 << (32 - prefix)) - 1)
        };

        let start = u32::from_be_bytes(addr) & mask;
        let end = start | !mask;

        Self {
            ts_type: 7,
            ip_protocol: 0,
            start_port: 0,
            end_port: 65535,
            start_address: start.to_be_bytes().to_vec(),
            end_address: end.to_be_bytes().to_vec(),
        }
    }
}

/// IKE proposal
#[derive(Debug, Clone)]
pub struct IkeProposal {
    pub encryption: EncryptionAlgorithm,
    pub integrity: IntegrityAlgorithm,
    pub prf: PrfAlgorithm,
    pub dh_group: DhGroup,
}

impl IkeProposal {
    pub fn default_aead() -> Self {
        Self {
            encryption: EncryptionAlgorithm::Aes256Gcm16,
            integrity: IntegrityAlgorithm::None,
            prf: PrfAlgorithm::HmacSha256,
            dh_group: DhGroup::Curve25519,
        }
    }

    pub fn default_cbc() -> Self {
        Self {
            encryption: EncryptionAlgorithm::Aes256,
            integrity: IntegrityAlgorithm::HmacSha256_128,
            prf: PrfAlgorithm::HmacSha256,
            dh_group: DhGroup::Modp2048,
        }
    }
}

/// Child SA proposal
#[derive(Debug, Clone)]
pub struct ChildProposal {
    pub protocol: IpsecProtocol,
    pub encryption: EncryptionAlgorithm,
    pub integrity: IntegrityAlgorithm,
    pub dh_group: Option<DhGroup>,
    pub esn: bool, // Extended Sequence Numbers
}

impl ChildProposal {
    pub fn default_esp() -> Self {
        Self {
            protocol: IpsecProtocol::Esp,
            encryption: EncryptionAlgorithm::Aes256Gcm16,
            integrity: IntegrityAlgorithm::None,
            dh_group: Some(DhGroup::Curve25519),
            esn: false,
        }
    }
}

/// Security Policy
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub local_ts: Vec<TrafficSelector>,
    pub remote_ts: Vec<TrafficSelector>,
    pub mode: IpsecMode,
    pub protocol: IpsecProtocol,
}

impl SecurityPolicy {
    pub fn tunnel_all() -> Self {
        Self {
            local_ts: vec![TrafficSelector::ipv4_any()],
            remote_ts: vec![TrafficSelector::ipv4_any()],
            mode: IpsecMode::Tunnel,
            protocol: IpsecProtocol::Esp,
        }
    }
}

/// IKE SA
#[derive(Debug)]
pub struct IkeSa {
    pub initiator_spi: [u8; 8],
    pub responder_spi: [u8; 8],
    pub state: IkeSaState,
    pub version: IkeVersion,
    pub is_initiator: bool,
    pub proposal: IkeProposal,
    pub auth_method: AuthMethod,

    // Keys
    pub sk_d: Vec<u8>,  // Key derivation key
    pub sk_ai: Vec<u8>, // Auth key initiator
    pub sk_ar: Vec<u8>, // Auth key responder
    pub sk_ei: Vec<u8>, // Encryption key initiator
    pub sk_er: Vec<u8>, // Encryption key responder
    pub sk_pi: Vec<u8>, // PRF key initiator
    pub sk_pr: Vec<u8>, // PRF key responder

    // Nonces
    pub nonce_i: Vec<u8>,
    pub nonce_r: Vec<u8>,

    pub message_id: u32,
    pub created: u64,
    pub last_activity: u64,
    pub lifetime: u64,
    pub rekey_at: u64,
}

impl IkeSa {
    pub fn new(is_initiator: bool) -> Self {
        let spi = [0u8; 8]; // Would use RNG

        Self {
            initiator_spi: if is_initiator { spi } else { [0; 8] },
            responder_spi: if is_initiator { [0; 8] } else { spi },
            state: IkeSaState::Idle,
            version: IkeVersion::V2,
            is_initiator,
            proposal: IkeProposal::default_aead(),
            auth_method: AuthMethod::Psk,
            sk_d: Vec::new(),
            sk_ai: Vec::new(),
            sk_ar: Vec::new(),
            sk_ei: Vec::new(),
            sk_er: Vec::new(),
            sk_pi: Vec::new(),
            sk_pr: Vec::new(),
            nonce_i: Vec::new(),
            nonce_r: Vec::new(),
            message_id: 0,
            created: 0,
            last_activity: 0,
            lifetime: 86400, // 24 hours default
            rekey_at: 79200, // 22 hours
        }
    }

    pub fn needs_rekey(&self, current_time: u64) -> bool {
        current_time >= self.created + self.rekey_at
    }

    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time >= self.created + self.lifetime
    }
}

/// Child SA
#[derive(Debug)]
pub struct ChildSa {
    pub spi_in: u32,
    pub spi_out: u32,
    pub state: ChildSaState,
    pub proposal: ChildProposal,
    pub policy: SecurityPolicy,

    // Keys
    pub encrypt_key_in: Vec<u8>,
    pub encrypt_key_out: Vec<u8>,
    pub auth_key_in: Vec<u8>,
    pub auth_key_out: Vec<u8>,

    pub seq_num_in: u64,
    pub seq_num_out: u64,
    pub anti_replay_window: u64,

    pub created: u64,
    pub lifetime_bytes: u64,
    pub lifetime_time: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

impl ChildSa {
    pub fn new() -> Self {
        Self {
            spi_in: 0, // Would use RNG
            spi_out: 0,
            state: ChildSaState::Inactive,
            proposal: ChildProposal::default_esp(),
            policy: SecurityPolicy::tunnel_all(),
            encrypt_key_in: Vec::new(),
            encrypt_key_out: Vec::new(),
            auth_key_in: Vec::new(),
            auth_key_out: Vec::new(),
            seq_num_in: 0,
            seq_num_out: 0,
            anti_replay_window: 0,
            created: 0,
            lifetime_bytes: 0,
            lifetime_time: 28800, // 8 hours default
            bytes_in: 0,
            bytes_out: 0,
        }
    }
}

/// IPsec connection configuration
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub name: String,
    pub remote: String,
    pub local_id: Option<String>,
    pub remote_id: Option<String>,
    pub auth_method: AuthMethod,
    pub psk: Option<String>,
    pub local_cert: Option<Vec<u8>>,
    pub local_key: Option<Vec<u8>>,
    pub ca_cert: Option<Vec<u8>>,
    pub ike_proposal: IkeProposal,
    pub child_proposal: ChildProposal,
    pub local_ts: Vec<TrafficSelector>,
    pub remote_ts: Vec<TrafficSelector>,
    pub mode: IpsecMode,
    pub dpd_delay: u32,     // Dead Peer Detection
    pub dpd_timeout: u32,
    pub rekey_time: u64,
    pub auto_start: bool,
}

impl ConnectionConfig {
    pub fn new(name: &str, remote: &str) -> Self {
        Self {
            name: String::from(name),
            remote: String::from(remote),
            local_id: None,
            remote_id: None,
            auth_method: AuthMethod::Psk,
            psk: None,
            local_cert: None,
            local_key: None,
            ca_cert: None,
            ike_proposal: IkeProposal::default_aead(),
            child_proposal: ChildProposal::default_esp(),
            local_ts: vec![TrafficSelector::ipv4_any()],
            remote_ts: vec![TrafficSelector::ipv4_any()],
            mode: IpsecMode::Tunnel,
            dpd_delay: 30,
            dpd_timeout: 150,
            rekey_time: 79200,
            auto_start: false,
        }
    }

    pub fn with_psk(mut self, psk: &str) -> Self {
        self.psk = Some(String::from(psk));
        self
    }
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disabled,
    Disconnected,
    Connecting,
    Established,
    Rekeying,
    Disconnecting,
    Error,
}

impl ConnectionState {
    pub fn name(&self) -> &'static str {
        match self {
            ConnectionState::Disabled => "Disabled",
            ConnectionState::Disconnected => "Disconnected",
            ConnectionState::Connecting => "Connecting",
            ConnectionState::Established => "Established",
            ConnectionState::Rekeying => "Rekeying",
            ConnectionState::Disconnecting => "Disconnecting",
            ConnectionState::Error => "Error",
        }
    }
}

/// Connection statistics
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub packets_in: u64,
    pub packets_out: u64,
    pub connect_time: u64,
    pub last_handshake: u64,
    pub rekeys: u64,
}

/// IPsec error
#[derive(Debug, Clone)]
pub enum IpsecError {
    ConnectionFailed,
    AuthenticationFailed,
    ProposalMismatch,
    InvalidPacket,
    Timeout,
    CertificateError,
    ConfigurationError,
}

pub type IpsecResult<T> = Result<T, IpsecError>;

/// IPsec connection
pub struct IpsecConnection {
    pub config: ConnectionConfig,
    pub state: ConnectionState,
    pub ike_sa: Option<IkeSa>,
    pub child_sas: Vec<ChildSa>,
    pub stats: ConnectionStats,
    pub local_ip: Option<String>,
    pub remote_ip: Option<String>,
    pub last_error: Option<String>,
    current_time: u64,
}

impl IpsecConnection {
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            config,
            state: ConnectionState::Disconnected,
            ike_sa: None,
            child_sas: Vec::new(),
            stats: ConnectionStats::default(),
            local_ip: None,
            remote_ip: None,
            last_error: None,
            current_time: 0,
        }
    }

    /// Start connection
    pub fn connect(&mut self) -> IpsecResult<()> {
        if self.state == ConnectionState::Established {
            return Ok(());
        }

        self.state = ConnectionState::Connecting;
        self.stats.connect_time = self.current_time;

        // Create IKE SA
        let mut ike_sa = IkeSa::new(true);
        ike_sa.auth_method = self.config.auth_method;
        ike_sa.proposal = self.config.ike_proposal.clone();
        ike_sa.created = self.current_time;
        ike_sa.state = IkeSaState::InitSent;

        self.ike_sa = Some(ike_sa);
        Ok(())
    }

    /// Disconnect
    pub fn disconnect(&mut self) {
        self.state = ConnectionState::Disconnecting;

        // Send DELETE for child SAs
        // Send DELETE for IKE SA

        self.ike_sa = None;
        self.child_sas.clear();
        self.state = ConnectionState::Disconnected;
    }

    /// Process incoming packet
    pub fn process_packet(&mut self, data: &[u8]) -> IpsecResult<Option<Vec<u8>>> {
        if data.len() < IkeHeader::SIZE {
            return Err(IpsecError::InvalidPacket);
        }

        self.stats.packets_in += 1;
        self.stats.bytes_in += data.len() as u64;

        // Process based on state
        match self.state {
            ConnectionState::Connecting => {
                // Process IKE_SA_INIT response or IKE_AUTH response
                if let Some(ref mut ike_sa) = self.ike_sa {
                    match ike_sa.state {
                        IkeSaState::InitSent => {
                            ike_sa.state = IkeSaState::AuthSent;
                        }
                        IkeSaState::AuthSent => {
                            ike_sa.state = IkeSaState::Established;
                            self.state = ConnectionState::Established;

                            // Create child SA
                            let mut child_sa = ChildSa::new();
                            child_sa.state = ChildSaState::Established;
                            child_sa.created = self.current_time;
                            self.child_sas.push(child_sa);
                        }
                        _ => {}
                    }
                }
            }
            ConnectionState::Established => {
                // Process ESP packet
                if data.len() > 8 {
                    // Would decrypt here
                    let decrypted = data[8..].to_vec();
                    return Ok(Some(decrypted));
                }
            }
            _ => {}
        }

        Ok(None)
    }

    /// Create ESP packet
    pub fn create_esp_packet(&mut self, data: &[u8]) -> IpsecResult<Vec<u8>> {
        if self.state != ConnectionState::Established {
            return Err(IpsecError::ConnectionFailed);
        }

        let child_sa = self.child_sas.first_mut()
            .ok_or(IpsecError::ConnectionFailed)?;

        child_sa.seq_num_out += 1;
        child_sa.bytes_out += data.len() as u64;

        let mut packet = Vec::new();

        // SPI (4 bytes)
        packet.extend_from_slice(&child_sa.spi_out.to_be_bytes());

        // Sequence number (4 bytes)
        packet.extend_from_slice(&(child_sa.seq_num_out as u32).to_be_bytes());

        // Would encrypt here
        packet.extend_from_slice(data);

        self.stats.packets_out += 1;
        self.stats.bytes_out += packet.len() as u64;

        Ok(packet)
    }

    /// Set current time
    pub fn set_current_time(&mut self, time: u64) {
        self.current_time = time;
    }

    /// Simulate connection for demo
    pub fn simulate_connection(&mut self) {
        self.state = ConnectionState::Established;
        self.local_ip = Some(String::from("10.10.10.1"));
        self.remote_ip = Some(String::from("10.10.10.2"));

        let mut ike_sa = IkeSa::new(true);
        ike_sa.state = IkeSaState::Established;
        ike_sa.created = self.current_time;
        self.ike_sa = Some(ike_sa);

        let mut child_sa = ChildSa::new();
        child_sa.state = ChildSaState::Established;
        child_sa.spi_in = 0x12345678;
        child_sa.spi_out = 0x87654321;
        child_sa.created = self.current_time;
        self.child_sas.push(child_sa);

        self.stats.connect_time = self.current_time;
        self.stats.last_handshake = self.current_time;
    }
}

/// IPsec manager
pub struct IpsecManager {
    connections: BTreeMap<u64, IpsecConnection>,
    configs: BTreeMap<u64, ConnectionConfig>,
    next_id: u64,
    current_time: u64,
}

impl IpsecManager {
    pub fn new() -> Self {
        Self {
            connections: BTreeMap::new(),
            configs: BTreeMap::new(),
            next_id: 1,
            current_time: 0,
        }
    }

    /// Add connection config
    pub fn add_connection(&mut self, config: ConnectionConfig) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.configs.insert(id, config);
        id
    }

    /// Remove connection
    pub fn remove_connection(&mut self, id: u64) {
        self.disconnect(id);
        self.configs.remove(&id);
    }

    /// Get config
    pub fn get_config(&self, id: u64) -> Option<&ConnectionConfig> {
        self.configs.get(&id)
    }

    /// Get all configs
    pub fn configs(&self) -> Vec<(&u64, &ConnectionConfig)> {
        self.configs.iter().collect()
    }

    /// Connect
    pub fn connect(&mut self, id: u64) -> IpsecResult<()> {
        let config = self.configs.get(&id)
            .ok_or(IpsecError::ConfigurationError)?
            .clone();

        let mut conn = IpsecConnection::new(config);
        conn.set_current_time(self.current_time);
        conn.connect()?;

        self.connections.insert(id, conn);
        Ok(())
    }

    /// Disconnect
    pub fn disconnect(&mut self, id: u64) {
        if let Some(conn) = self.connections.get_mut(&id) {
            conn.disconnect();
        }
        self.connections.remove(&id);
    }

    /// Get connection
    pub fn get_connection(&self, id: u64) -> Option<&IpsecConnection> {
        self.connections.get(&id)
    }

    /// Get connection state
    pub fn get_state(&self, id: u64) -> ConnectionState {
        self.connections.get(&id)
            .map(|c| c.state)
            .unwrap_or(ConnectionState::Disconnected)
    }

    /// Set current time
    pub fn set_current_time(&mut self, time: u64) {
        self.current_time = time;
        for conn in self.connections.values_mut() {
            conn.set_current_time(time);
        }
    }

    /// Add sample data
    pub fn add_sample_data(&mut self) {
        self.current_time = 1705600000;

        // Sample configs
        let config1 = ConnectionConfig::new("Work VPN", "vpn.company.com")
            .with_psk("secret123");
        let id1 = self.add_connection(config1);

        let mut config2 = ConnectionConfig::new("Home Server", "home.example.net");
        config2.auth_method = AuthMethod::RsaSig;
        config2.ike_proposal = IkeProposal::default_cbc();
        let _id2 = self.add_connection(config2);

        // Simulate one connection
        if let Ok(()) = self.connect(id1) {
            if let Some(conn) = self.connections.get_mut(&id1) {
                conn.simulate_connection();
            }
        }
    }
}

impl Default for IpsecManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize IPsec module
pub fn init() -> IpsecManager {
    let mut manager = IpsecManager::new();
    manager.add_sample_data();
    manager
}
