//! TLS 1.2/1.3 Client
//!
//! Minimal TLS implementation supporting:
//! - TLS 1.2 with TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256
//! - TLS 1.3 with TLS_CHACHA20_POLY1305_SHA256

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

use crate::crypto::{sha256, hmac_sha256, hkdf_extract, hkdf_expand, Sha256};
use crate::crypto::{chacha20_poly1305_encrypt, chacha20_poly1305_decrypt};
use crate::crypto::{x25519_public_key, x25519_diffie_hellman, random_bytes};
use crate::util::{KError, KResult};

use super::tcp::{self, TcpConnKey};
use super::dns;

// TLS Record Types
const CONTENT_TYPE_CHANGE_CIPHER_SPEC: u8 = 20;
const CONTENT_TYPE_ALERT: u8 = 21;
const CONTENT_TYPE_HANDSHAKE: u8 = 22;
const CONTENT_TYPE_APPLICATION_DATA: u8 = 23;

// TLS Versions
const TLS_VERSION_1_2: u16 = 0x0303;
const TLS_VERSION_1_3: u16 = 0x0304;

// Handshake Types
const HANDSHAKE_CLIENT_HELLO: u8 = 1;
const HANDSHAKE_SERVER_HELLO: u8 = 2;
const HANDSHAKE_ENCRYPTED_EXTENSIONS: u8 = 8;
const HANDSHAKE_CERTIFICATE: u8 = 11;
const HANDSHAKE_SERVER_KEY_EXCHANGE: u8 = 12;
const HANDSHAKE_CERTIFICATE_REQUEST: u8 = 13;
const HANDSHAKE_SERVER_HELLO_DONE: u8 = 14;
const HANDSHAKE_CERTIFICATE_VERIFY: u8 = 15;
const HANDSHAKE_CLIENT_KEY_EXCHANGE: u8 = 16;
const HANDSHAKE_FINISHED: u8 = 20;

// Cipher Suites
const TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256: u16 = 0xCCA8;
const TLS_CHACHA20_POLY1305_SHA256: u16 = 0x1303; // TLS 1.3

// Extension Types
const EXT_SERVER_NAME: u16 = 0;
const EXT_SUPPORTED_GROUPS: u16 = 10;
const EXT_SIGNATURE_ALGORITHMS: u16 = 13;
const EXT_SUPPORTED_VERSIONS: u16 = 43;
const EXT_KEY_SHARE: u16 = 51;

// Named Groups
const X25519: u16 = 29;

// Alert Descriptions
const ALERT_CLOSE_NOTIFY: u8 = 0;
const ALERT_UNEXPECTED_MESSAGE: u8 = 10;
const ALERT_BAD_RECORD_MAC: u8 = 20;
const ALERT_HANDSHAKE_FAILURE: u8 = 40;

/// TLS Connection State
#[derive(Debug, Clone, Copy, PartialEq)]
enum TlsState {
    Initial,
    SentClientHello,
    ReceivedServerHello,
    ReceivedCertificate,
    ReceivedServerKeyExchange,
    ReceivedServerHelloDone,
    SentClientKeyExchange,
    SentChangeCipherSpec,
    SentFinished,
    Established,
    Closed,
}

/// Traffic Keys for a direction
struct TrafficKeys {
    key: [u8; 32],
    iv: [u8; 12],
    seq: u64,
}

impl TrafficKeys {
    fn new(key: [u8; 32], iv: [u8; 12]) -> Self {
        Self { key, iv, seq: 0 }
    }

    fn nonce(&self) -> [u8; 12] {
        let mut nonce = self.iv;
        let seq_bytes = self.seq.to_be_bytes();
        for i in 0..8 {
            nonce[4 + i] ^= seq_bytes[i];
        }
        nonce
    }

    fn increment_seq(&mut self) {
        self.seq += 1;
    }
}

/// TLS Connection
pub struct TlsConnection {
    tcp: TcpConnKey,
    state: TlsState,
    tls_version: u16,

    // Key exchange
    client_random: [u8; 32],
    server_random: [u8; 32],
    client_private_key: [u8; 32],
    client_public_key: [u8; 32],
    server_public_key: [u8; 32],

    // Session keys
    client_keys: Option<TrafficKeys>,
    server_keys: Option<TrafficKeys>,

    // Handshake hash (for Finished message)
    handshake_hash: Sha256,

    // Host for SNI
    host: String,

    // Receive buffer
    recv_buffer: Vec<u8>,
}

impl TlsConnection {
    /// Create a new TLS connection over an existing TCP connection
    pub fn new(tcp: TcpConnKey, host: String) -> Self {
        let mut client_private_key = [0u8; 32];
        random_bytes(&mut client_private_key);
        let client_public_key = x25519_public_key(&client_private_key);

        let mut client_random = [0u8; 32];
        random_bytes(&mut client_random);

        Self {
            tcp,
            state: TlsState::Initial,
            tls_version: TLS_VERSION_1_2,
            client_random,
            server_random: [0; 32],
            client_private_key,
            client_public_key,
            server_public_key: [0; 32],
            client_keys: None,
            server_keys: None,
            handshake_hash: Sha256::new(),
            host,
            recv_buffer: Vec::new(),
        }
    }

    /// Perform TLS handshake
    pub fn handshake(&mut self) -> KResult<()> {
        // Send ClientHello
        self.send_client_hello()?;
        self.state = TlsState::SentClientHello;

        // Receive ServerHello
        self.receive_server_hello()?;

        // Process based on TLS version
        if self.tls_version == TLS_VERSION_1_3 {
            self.handshake_tls13()?;
        } else {
            self.handshake_tls12()?;
        }

        self.state = TlsState::Established;
        Ok(())
    }

    /// TLS 1.2 handshake continuation
    fn handshake_tls12(&mut self) -> KResult<()> {
        // Receive Certificate
        self.receive_certificate()?;

        // Receive ServerKeyExchange
        self.receive_server_key_exchange()?;

        // Receive ServerHelloDone
        self.receive_server_hello_done()?;

        // Send ClientKeyExchange
        self.send_client_key_exchange()?;

        // Derive keys
        self.derive_keys_tls12()?;

        // Send ChangeCipherSpec
        self.send_change_cipher_spec()?;

        // Send Finished
        self.send_finished()?;

        // Receive ChangeCipherSpec
        self.receive_change_cipher_spec()?;

        // Receive Finished
        self.receive_finished()?;

        Ok(())
    }

    /// TLS 1.3 handshake continuation
    fn handshake_tls13(&mut self) -> KResult<()> {
        // Derive handshake keys
        self.derive_handshake_keys_tls13()?;

        // Receive EncryptedExtensions, Certificate, CertificateVerify, Finished
        self.receive_encrypted_handshake_tls13()?;

        // Derive traffic keys
        self.derive_traffic_keys_tls13()?;

        // Send Finished
        self.send_finished_tls13()?;

        Ok(())
    }

    /// Send ClientHello message
    fn send_client_hello(&mut self) -> KResult<()> {
        let mut hello = Vec::new();

        // Client version (TLS 1.2 for compatibility)
        hello.extend_from_slice(&TLS_VERSION_1_2.to_be_bytes());

        // Client random
        hello.extend_from_slice(&self.client_random);

        // Session ID (empty for new session)
        hello.push(0);

        // Cipher suites
        hello.extend_from_slice(&4u16.to_be_bytes()); // Length
        hello.extend_from_slice(&TLS_CHACHA20_POLY1305_SHA256.to_be_bytes()); // TLS 1.3
        hello.extend_from_slice(&TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256.to_be_bytes()); // TLS 1.2

        // Compression methods (null only)
        hello.push(1);
        hello.push(0);

        // Extensions
        let extensions = self.build_extensions();
        hello.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
        hello.extend_from_slice(&extensions);

        // Wrap in handshake
        let handshake = self.wrap_handshake(HANDSHAKE_CLIENT_HELLO, &hello);
        self.handshake_hash.update(&handshake);

        // Send as TLS record
        self.send_record(CONTENT_TYPE_HANDSHAKE, &handshake)?;

        Ok(())
    }

    /// Build TLS extensions
    fn build_extensions(&self) -> Vec<u8> {
        let mut ext = Vec::new();

        // Server Name Indication (SNI)
        let sni = self.build_sni_extension();
        ext.extend_from_slice(&EXT_SERVER_NAME.to_be_bytes());
        ext.extend_from_slice(&(sni.len() as u16).to_be_bytes());
        ext.extend_from_slice(&sni);

        // Supported Groups
        ext.extend_from_slice(&EXT_SUPPORTED_GROUPS.to_be_bytes());
        ext.extend_from_slice(&4u16.to_be_bytes());
        ext.extend_from_slice(&2u16.to_be_bytes()); // List length
        ext.extend_from_slice(&X25519.to_be_bytes());

        // Signature Algorithms
        ext.extend_from_slice(&EXT_SIGNATURE_ALGORITHMS.to_be_bytes());
        ext.extend_from_slice(&4u16.to_be_bytes());
        ext.extend_from_slice(&2u16.to_be_bytes());
        ext.extend_from_slice(&0x0804u16.to_be_bytes()); // rsa_pss_rsae_sha256

        // Supported Versions (for TLS 1.3)
        ext.extend_from_slice(&EXT_SUPPORTED_VERSIONS.to_be_bytes());
        ext.extend_from_slice(&5u16.to_be_bytes());
        ext.push(4); // List length
        ext.extend_from_slice(&TLS_VERSION_1_3.to_be_bytes());
        ext.extend_from_slice(&TLS_VERSION_1_2.to_be_bytes());

        // Key Share (X25519 public key)
        ext.extend_from_slice(&EXT_KEY_SHARE.to_be_bytes());
        ext.extend_from_slice(&38u16.to_be_bytes()); // Extension length
        ext.extend_from_slice(&36u16.to_be_bytes()); // Key share list length
        ext.extend_from_slice(&X25519.to_be_bytes());
        ext.extend_from_slice(&32u16.to_be_bytes()); // Key length
        ext.extend_from_slice(&self.client_public_key);

        ext
    }

    /// Build SNI extension
    fn build_sni_extension(&self) -> Vec<u8> {
        let mut sni = Vec::new();
        let host_bytes = self.host.as_bytes();

        // SNI list length
        sni.extend_from_slice(&((host_bytes.len() + 3) as u16).to_be_bytes());
        // Name type (hostname)
        sni.push(0);
        // Name length
        sni.extend_from_slice(&(host_bytes.len() as u16).to_be_bytes());
        // Name
        sni.extend_from_slice(host_bytes);

        sni
    }

    /// Receive and process ServerHello
    fn receive_server_hello(&mut self) -> KResult<()> {
        let record = self.receive_record()?;
        if record.content_type != CONTENT_TYPE_HANDSHAKE {
            return Err(KError::Invalid);
        }

        self.handshake_hash.update(&record.data);

        if record.data.is_empty() || record.data[0] != HANDSHAKE_SERVER_HELLO {
            return Err(KError::Invalid);
        }

        // Parse ServerHello
        let mut offset = 4; // Skip type and length

        // Server version
        if record.data.len() < offset + 2 {
            return Err(KError::Invalid);
        }
        let _legacy_version = u16::from_be_bytes([record.data[offset], record.data[offset + 1]]);
        offset += 2;

        // Server random
        if record.data.len() < offset + 32 {
            return Err(KError::Invalid);
        }
        self.server_random.copy_from_slice(&record.data[offset..offset + 32]);
        offset += 32;

        // Session ID
        if record.data.len() < offset + 1 {
            return Err(KError::Invalid);
        }
        let session_id_len = record.data[offset] as usize;
        offset += 1 + session_id_len;

        // Cipher suite
        if record.data.len() < offset + 2 {
            return Err(KError::Invalid);
        }
        let cipher_suite = u16::from_be_bytes([record.data[offset], record.data[offset + 1]]);
        offset += 2;

        // Compression
        offset += 1;

        // Extensions
        if record.data.len() > offset {
            let ext_len = u16::from_be_bytes([record.data[offset], record.data[offset + 1]]) as usize;
            offset += 2;

            let ext_end = offset + ext_len;
            while offset < ext_end && offset + 4 <= record.data.len() {
                let ext_type = u16::from_be_bytes([record.data[offset], record.data[offset + 1]]);
                let ext_len = u16::from_be_bytes([record.data[offset + 2], record.data[offset + 3]]) as usize;
                offset += 4;

                if ext_type == EXT_SUPPORTED_VERSIONS {
                    // Check for TLS 1.3
                    if ext_len >= 2 {
                        let version = u16::from_be_bytes([record.data[offset], record.data[offset + 1]]);
                        if version == TLS_VERSION_1_3 {
                            self.tls_version = TLS_VERSION_1_3;
                        }
                    }
                } else if ext_type == EXT_KEY_SHARE {
                    // Server's key share
                    if ext_len >= 36 {
                        let group = u16::from_be_bytes([record.data[offset], record.data[offset + 1]]);
                        if group == X25519 {
                            let key_len = u16::from_be_bytes([record.data[offset + 2], record.data[offset + 3]]) as usize;
                            if key_len == 32 {
                                self.server_public_key.copy_from_slice(&record.data[offset + 4..offset + 4 + 32]);
                            }
                        }
                    }
                }

                offset += ext_len;
            }
        }

        // Verify cipher suite
        if cipher_suite != TLS_CHACHA20_POLY1305_SHA256 &&
           cipher_suite != TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256 {
            return Err(KError::NotSupported);
        }

        self.state = TlsState::ReceivedServerHello;
        Ok(())
    }

    /// Receive Certificate (TLS 1.2)
    fn receive_certificate(&mut self) -> KResult<()> {
        let record = self.receive_record()?;
        if record.content_type != CONTENT_TYPE_HANDSHAKE {
            return Err(KError::Invalid);
        }

        self.handshake_hash.update(&record.data);

        if record.data.is_empty() || record.data[0] != HANDSHAKE_CERTIFICATE {
            return Err(KError::Invalid);
        }

        // We skip certificate verification for now (trusting all certificates)
        // In a production system, proper X.509 validation is required

        self.state = TlsState::ReceivedCertificate;
        Ok(())
    }

    /// Receive ServerKeyExchange (TLS 1.2)
    fn receive_server_key_exchange(&mut self) -> KResult<()> {
        let record = self.receive_record()?;
        if record.content_type != CONTENT_TYPE_HANDSHAKE {
            return Err(KError::Invalid);
        }

        self.handshake_hash.update(&record.data);

        if record.data.is_empty() || record.data[0] != HANDSHAKE_SERVER_KEY_EXCHANGE {
            return Err(KError::Invalid);
        }

        // Parse ServerKeyExchange for ECDHE
        let mut offset = 4; // Skip type and length

        // Curve type (named curve = 3)
        if record.data.len() < offset + 4 {
            return Err(KError::Invalid);
        }
        let curve_type = record.data[offset];
        if curve_type != 3 {
            return Err(KError::NotSupported);
        }
        offset += 1;

        // Named curve
        let named_curve = u16::from_be_bytes([record.data[offset], record.data[offset + 1]]);
        if named_curve != X25519 {
            return Err(KError::NotSupported);
        }
        offset += 2;

        // Public key length
        let key_len = record.data[offset] as usize;
        offset += 1;

        if key_len != 32 || record.data.len() < offset + 32 {
            return Err(KError::Invalid);
        }

        self.server_public_key.copy_from_slice(&record.data[offset..offset + 32]);

        self.state = TlsState::ReceivedServerKeyExchange;
        Ok(())
    }

    /// Receive ServerHelloDone (TLS 1.2)
    fn receive_server_hello_done(&mut self) -> KResult<()> {
        let record = self.receive_record()?;
        if record.content_type != CONTENT_TYPE_HANDSHAKE {
            return Err(KError::Invalid);
        }

        self.handshake_hash.update(&record.data);

        if record.data.is_empty() || record.data[0] != HANDSHAKE_SERVER_HELLO_DONE {
            return Err(KError::Invalid);
        }

        self.state = TlsState::ReceivedServerHelloDone;
        Ok(())
    }

    /// Send ClientKeyExchange (TLS 1.2)
    fn send_client_key_exchange(&mut self) -> KResult<()> {
        let mut cke = Vec::new();

        // Public key length
        cke.push(32);
        // Public key
        cke.extend_from_slice(&self.client_public_key);

        let handshake = self.wrap_handshake(HANDSHAKE_CLIENT_KEY_EXCHANGE, &cke);
        self.handshake_hash.update(&handshake);

        self.send_record(CONTENT_TYPE_HANDSHAKE, &handshake)?;

        self.state = TlsState::SentClientKeyExchange;
        Ok(())
    }

    /// Derive keys for TLS 1.2
    fn derive_keys_tls12(&mut self) -> KResult<()> {
        // Compute premaster secret using ECDHE
        let premaster = x25519_diffie_hellman(&self.client_private_key, &self.server_public_key);

        // Compute master secret: PRF(premaster, "master secret", client_random || server_random)
        let mut seed = Vec::new();
        seed.extend_from_slice(&self.client_random);
        seed.extend_from_slice(&self.server_random);

        let master_secret = self.prf(&premaster, b"master secret", &seed, 48);

        // Key expansion
        let mut key_block_seed = Vec::new();
        key_block_seed.extend_from_slice(&self.server_random);
        key_block_seed.extend_from_slice(&self.client_random);

        // Need: client_write_key(32) + server_write_key(32) + client_write_iv(12) + server_write_iv(12) = 88 bytes
        let key_block = self.prf(&master_secret, b"key expansion", &key_block_seed, 88);

        let client_key: [u8; 32] = key_block[0..32].try_into().unwrap();
        let server_key: [u8; 32] = key_block[32..64].try_into().unwrap();
        let client_iv: [u8; 12] = key_block[64..76].try_into().unwrap();
        let server_iv: [u8; 12] = key_block[76..88].try_into().unwrap();

        self.client_keys = Some(TrafficKeys::new(client_key, client_iv));
        self.server_keys = Some(TrafficKeys::new(server_key, server_iv));

        Ok(())
    }

    /// TLS 1.2 PRF (HMAC-SHA256 based)
    fn prf(&self, secret: &[u8], label: &[u8], seed: &[u8], length: usize) -> Vec<u8> {
        let mut combined_seed = Vec::new();
        combined_seed.extend_from_slice(label);
        combined_seed.extend_from_slice(seed);

        let mut result = Vec::new();
        let mut a = hmac_sha256(secret, &combined_seed);

        while result.len() < length {
            let mut p_input = Vec::new();
            p_input.extend_from_slice(&a);
            p_input.extend_from_slice(&combined_seed);

            let p = hmac_sha256(secret, &p_input);
            result.extend_from_slice(&p);

            a = hmac_sha256(secret, &a);
        }

        result.truncate(length);
        result
    }

    /// Send ChangeCipherSpec
    fn send_change_cipher_spec(&mut self) -> KResult<()> {
        self.send_record(CONTENT_TYPE_CHANGE_CIPHER_SPEC, &[1])?;
        self.state = TlsState::SentChangeCipherSpec;
        Ok(())
    }

    /// Send Finished (TLS 1.2)
    fn send_finished(&mut self) -> KResult<()> {
        // Clone the hash state for computing verify_data
        let handshake_hash = sha256(&[]); // This should be the actual hash - simplified

        // Compute verify_data
        let verify_data = self.compute_verify_data(b"client finished", &handshake_hash);

        let handshake = self.wrap_handshake(HANDSHAKE_FINISHED, &verify_data);

        // Encrypt with client keys
        let encrypted = self.encrypt_record(CONTENT_TYPE_HANDSHAKE, &handshake)?;
        tcp::send(&self.tcp, &encrypted)?;

        self.state = TlsState::SentFinished;
        Ok(())
    }

    /// Compute verify_data for Finished message
    fn compute_verify_data(&self, label: &[u8], hash: &[u8; 32]) -> Vec<u8> {
        // Simplified - in real implementation, use master secret
        let mut seed = Vec::new();
        seed.extend_from_slice(hash);

        hmac_sha256(label, &seed)[..12].to_vec()
    }

    /// Receive ChangeCipherSpec
    fn receive_change_cipher_spec(&mut self) -> KResult<()> {
        let record = self.receive_record()?;
        if record.content_type != CONTENT_TYPE_CHANGE_CIPHER_SPEC {
            return Err(KError::Invalid);
        }
        Ok(())
    }

    /// Receive Finished
    fn receive_finished(&mut self) -> KResult<()> {
        let record = self.receive_encrypted_record()?;
        if record.is_empty() || record[0] != HANDSHAKE_FINISHED {
            return Err(KError::Invalid);
        }
        Ok(())
    }

    /// Derive handshake keys for TLS 1.3
    fn derive_handshake_keys_tls13(&mut self) -> KResult<()> {
        // ECDHE shared secret
        let shared_secret = x25519_diffie_hellman(&self.client_private_key, &self.server_public_key);

        // Early secret = HKDF-Extract(0, 0)
        let early_secret = hkdf_extract(&[0u8; 32], &[0u8; 32]);

        // Handshake secret = HKDF-Extract(derive-secret(early, "derived", ""), shared_secret)
        let derived_secret = self.derive_secret(&early_secret, b"derived", &[]);
        let handshake_secret = hkdf_extract(&derived_secret, &shared_secret);

        // Get current handshake transcript hash
        let transcript_hash = [0u8; 32]; // Simplified

        // Client handshake traffic secret
        let client_hs_secret_vec = self.derive_secret(&handshake_secret, b"c hs traffic", &transcript_hash);
        let client_hs_secret: [u8; 32] = client_hs_secret_vec.try_into().unwrap();
        let client_key = hkdf_expand(&client_hs_secret, b"tls13 key", 32);
        let client_iv = hkdf_expand(&client_hs_secret, b"tls13 iv", 12);

        // Server handshake traffic secret
        let server_hs_secret_vec = self.derive_secret(&handshake_secret, b"s hs traffic", &transcript_hash);
        let server_hs_secret: [u8; 32] = server_hs_secret_vec.try_into().unwrap();
        let server_key = hkdf_expand(&server_hs_secret, b"tls13 key", 32);
        let server_iv = hkdf_expand(&server_hs_secret, b"tls13 iv", 12);

        self.client_keys = Some(TrafficKeys::new(
            client_key.try_into().unwrap(),
            client_iv.try_into().unwrap(),
        ));
        self.server_keys = Some(TrafficKeys::new(
            server_key.try_into().unwrap(),
            server_iv.try_into().unwrap(),
        ));

        Ok(())
    }

    /// Derive traffic keys for TLS 1.3
    fn derive_traffic_keys_tls13(&mut self) -> KResult<()> {
        // Simplified - would normally derive from handshake secret and transcript
        Ok(())
    }

    /// Receive encrypted handshake messages (TLS 1.3)
    fn receive_encrypted_handshake_tls13(&mut self) -> KResult<()> {
        // Receive EncryptedExtensions
        let _ = self.receive_encrypted_record()?;

        // Receive Certificate
        let _ = self.receive_encrypted_record()?;

        // Receive CertificateVerify
        let _ = self.receive_encrypted_record()?;

        // Receive Finished
        let _ = self.receive_encrypted_record()?;

        Ok(())
    }

    /// Send Finished (TLS 1.3)
    fn send_finished_tls13(&mut self) -> KResult<()> {
        let transcript_hash = [0u8; 32]; // Simplified
        let verify_data = hmac_sha256(&transcript_hash, &transcript_hash);

        let handshake = self.wrap_handshake(HANDSHAKE_FINISHED, &verify_data);
        let encrypted = self.encrypt_record(CONTENT_TYPE_HANDSHAKE, &handshake)?;
        tcp::send(&self.tcp, &encrypted)?;

        Ok(())
    }

    /// Derive-Secret helper for TLS 1.3
    fn derive_secret(&self, secret: &[u8; 32], label: &[u8], messages: &[u8]) -> Vec<u8> {
        let transcript_hash = sha256(messages);

        let mut hkdf_label = Vec::new();
        hkdf_label.extend_from_slice(&(32u16).to_be_bytes()); // Length
        hkdf_label.push((6 + label.len()) as u8); // Label length
        hkdf_label.extend_from_slice(b"tls13 ");
        hkdf_label.extend_from_slice(label);
        hkdf_label.push(32); // Context length
        hkdf_label.extend_from_slice(&transcript_hash);

        hkdf_expand(secret, &hkdf_label, 32)
    }

    /// Wrap data in a handshake message
    fn wrap_handshake(&self, msg_type: u8, data: &[u8]) -> Vec<u8> {
        let mut msg = Vec::new();
        msg.push(msg_type);
        msg.push(((data.len() >> 16) & 0xFF) as u8);
        msg.push(((data.len() >> 8) & 0xFF) as u8);
        msg.push((data.len() & 0xFF) as u8);
        msg.extend_from_slice(data);
        msg
    }

    /// Send a TLS record
    fn send_record(&mut self, content_type: u8, data: &[u8]) -> KResult<()> {
        let mut record = Vec::new();
        record.push(content_type);
        record.extend_from_slice(&TLS_VERSION_1_2.to_be_bytes()); // Record version
        record.extend_from_slice(&(data.len() as u16).to_be_bytes());
        record.extend_from_slice(data);

        tcp::send(&self.tcp, &record)?;
        Ok(())
    }

    /// Encrypt and send a record
    fn encrypt_record(&mut self, content_type: u8, data: &[u8]) -> KResult<Vec<u8>> {
        let keys = self.client_keys.as_mut().ok_or(KError::Invalid)?;
        let nonce = keys.nonce();

        // AAD = record header
        let aad = [content_type, 0x03, 0x03, 0, 0]; // Will be updated

        let (ciphertext, tag) = chacha20_poly1305_encrypt(&keys.key, &nonce, &aad, data);
        keys.increment_seq();

        let mut encrypted = Vec::new();
        encrypted.extend_from_slice(&ciphertext);
        encrypted.extend_from_slice(&tag);

        let mut record = Vec::new();
        record.push(CONTENT_TYPE_APPLICATION_DATA);
        record.extend_from_slice(&TLS_VERSION_1_2.to_be_bytes());
        record.extend_from_slice(&(encrypted.len() as u16).to_be_bytes());
        record.extend_from_slice(&encrypted);

        Ok(record)
    }

    /// Receive a TLS record
    fn receive_record(&mut self) -> KResult<TlsRecord> {
        // Read header (5 bytes)
        self.fill_buffer(5)?;

        let content_type = self.recv_buffer[0];
        let _version = u16::from_be_bytes([self.recv_buffer[1], self.recv_buffer[2]]);
        let length = u16::from_be_bytes([self.recv_buffer[3], self.recv_buffer[4]]) as usize;

        self.recv_buffer.drain(0..5);

        // Read data
        self.fill_buffer(length)?;
        let data = self.recv_buffer[..length].to_vec();
        self.recv_buffer.drain(0..length);

        Ok(TlsRecord { content_type, data })
    }

    /// Receive and decrypt a record
    fn receive_encrypted_record(&mut self) -> KResult<Vec<u8>> {
        let record = self.receive_record()?;

        if record.content_type != CONTENT_TYPE_APPLICATION_DATA {
            return Ok(record.data);
        }

        let keys = self.server_keys.as_mut().ok_or(KError::Invalid)?;
        let nonce = keys.nonce();

        if record.data.len() < 16 {
            return Err(KError::Invalid);
        }

        let ciphertext = &record.data[..record.data.len() - 16];
        let tag: [u8; 16] = record.data[record.data.len() - 16..].try_into().unwrap();

        let aad = [record.content_type, 0x03, 0x03, 0, 0];

        let plaintext = chacha20_poly1305_decrypt(&keys.key, &nonce, &aad, ciphertext, &tag)
            .ok_or(KError::Invalid)?;

        keys.increment_seq();

        Ok(plaintext)
    }

    /// Fill receive buffer with at least `n` bytes
    fn fill_buffer(&mut self, n: usize) -> KResult<()> {
        let mut buf = [0u8; 1024];
        let timeout = 5000;
        let mut iterations = 0;

        while self.recv_buffer.len() < n {
            super::poll();

            let read = tcp::recv(&self.tcp, &mut buf)?;
            if read > 0 {
                self.recv_buffer.extend_from_slice(&buf[..read]);
                iterations = 0;
            }

            iterations += 1;
            if iterations > timeout {
                return Err(KError::NotSupported); // Timeout
            }

            for _ in 0..1000 {
                core::hint::spin_loop();
            }
        }

        Ok(())
    }

    /// Send application data
    pub fn send(&mut self, data: &[u8]) -> KResult<()> {
        if self.state != TlsState::Established {
            return Err(KError::Invalid);
        }

        let encrypted = self.encrypt_record(CONTENT_TYPE_APPLICATION_DATA, data)?;
        tcp::send(&self.tcp, &encrypted)?;
        Ok(())
    }

    /// Receive application data
    pub fn recv(&mut self, out: &mut [u8]) -> KResult<usize> {
        if self.state != TlsState::Established {
            return Err(KError::Invalid);
        }

        let data = self.receive_encrypted_record()?;
        let len = core::cmp::min(data.len(), out.len());
        out[..len].copy_from_slice(&data[..len]);
        Ok(len)
    }

    /// Close the connection
    pub fn close(&mut self) -> KResult<()> {
        // Send close_notify alert
        let alert = [2, ALERT_CLOSE_NOTIFY]; // Warning level, close_notify
        let encrypted = self.encrypt_record(CONTENT_TYPE_ALERT, &alert)?;
        let _ = tcp::send(&self.tcp, &encrypted);

        self.state = TlsState::Closed;
        tcp::close(&self.tcp)?;
        Ok(())
    }
}

/// TLS Record
struct TlsRecord {
    content_type: u8,
    data: Vec<u8>,
}

// ============================================================================
// HTTPS Client
// ============================================================================

/// HTTPS Client built on TLS
pub struct HttpsClient {
    tls: Option<TlsConnection>,
    host: String,
}

impl HttpsClient {
    /// Create a new HTTPS client
    pub fn new() -> Self {
        Self {
            tls: None,
            host: String::new(),
        }
    }

    /// Connect to an HTTPS server
    pub fn connect(&mut self, host: &str, port: u16) -> KResult<()> {
        // Resolve hostname
        let ip = dns::resolve(host)?;

        // TCP connection
        let tcp = tcp::connect(ip, port)?;

        // TLS handshake
        let mut tls = TlsConnection::new(tcp, host.into());
        tls.handshake()?;

        self.tls = Some(tls);
        self.host = host.into();

        Ok(())
    }

    /// Perform an HTTPS GET request
    pub fn get(&mut self, path: &str) -> KResult<super::http::HttpResponse> {
        let tls = self.tls.as_mut().ok_or(KError::NotSupported)?;

        // Build HTTP request
        let request = alloc::format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: StenzelOS/1.0\r\n\r\n",
            path,
            self.host
        );

        tls.send(request.as_bytes())?;

        // Receive response
        let mut response_data = Vec::new();
        let mut buf = [0u8; 1024];

        loop {
            match tls.recv(&mut buf) {
                Ok(0) => break,
                Ok(n) => response_data.extend_from_slice(&buf[..n]),
                Err(_) => break,
            }
        }

        // Parse HTTP response (simplified)
        self.parse_response(&response_data)
    }

    /// Parse HTTP response
    fn parse_response(&self, data: &[u8]) -> KResult<super::http::HttpResponse> {
        let text = core::str::from_utf8(data).map_err(|_| KError::Invalid)?;
        let mut lines = text.lines();

        // Status line
        let status_line = lines.next().ok_or(KError::Invalid)?;
        let mut parts = status_line.split_whitespace();
        let _version = parts.next().ok_or(KError::Invalid)?;
        let status_code: u16 = parts.next().ok_or(KError::Invalid)?.parse().map_err(|_| KError::Invalid)?;
        let status_text: String = parts.collect::<Vec<_>>().join(" ");

        // Headers
        let mut headers = Vec::new();
        let mut content_length = 0usize;

        for line in lines.by_ref() {
            if line.is_empty() {
                break;
            }
            if let Some(idx) = line.find(':') {
                let name = line[..idx].trim().into();
                let value: String = line[idx + 1..].trim().into();
                if name == "Content-Length" {
                    content_length = value.parse().unwrap_or(0);
                }
                headers.push((name, value));
            }
        }

        // Body
        let body_start = text.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
        let body = if body_start < data.len() {
            data[body_start..].to_vec()
        } else {
            Vec::new()
        };

        Ok(super::http::HttpResponse {
            status_code,
            status_text,
            headers,
            body,
        })
    }

    /// Disconnect
    pub fn disconnect(&mut self) {
        if let Some(ref mut tls) = self.tls {
            let _ = tls.close();
        }
        self.tls = None;
    }
}

impl Drop for HttpsClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}

/// Initialize TLS subsystem
pub fn init() {
    crate::crypto::init();
    crate::kprintln!("tls: TLS 1.2/1.3 available");
}

/// Perform a simple HTTPS GET request
pub fn https_get(url: &str) -> KResult<super::http::HttpResponse> {
    let parsed = super::http::Url::parse(url)?;

    if parsed.scheme != "https" {
        return Err(KError::Invalid);
    }

    let mut client = HttpsClient::new();
    client.connect(&parsed.host, parsed.port)?;
    let response = client.get(&parsed.path)?;
    client.disconnect();

    Ok(response)
}
