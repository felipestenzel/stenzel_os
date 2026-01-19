//! SMS/MMS Messaging subsystem.
//!
//! Provides:
//! - SMS encoding/decoding (GSM 7-bit, UCS-2)
//! - Multi-part SMS (concatenated messages)
//! - SMS status reports (delivery receipts)
//! - MMS support (multimedia messages)
//! - Message storage management
//! - WAP Push handling

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use alloc::format;
use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// Maximum SMS length in GSM 7-bit encoding
const MAX_SMS_LENGTH_7BIT: usize = 160;
/// Maximum SMS length in UCS-2 encoding
const MAX_SMS_LENGTH_UCS2: usize = 70;
/// Maximum concatenated SMS parts
const MAX_SMS_PARTS: usize = 255;
/// Maximum MMS size (300KB)
const MAX_MMS_SIZE: usize = 300 * 1024;
/// Maximum stored messages
const MAX_STORED_MESSAGES: usize = 500;

/// SMS encoding type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SmsEncoding {
    #[default]
    Gsm7Bit,
    Gsm7BitExtended,
    Ucs2,
    Binary,
}

impl SmsEncoding {
    pub fn max_chars(&self) -> usize {
        match self {
            SmsEncoding::Gsm7Bit | SmsEncoding::Gsm7BitExtended => MAX_SMS_LENGTH_7BIT,
            SmsEncoding::Ucs2 => MAX_SMS_LENGTH_UCS2,
            SmsEncoding::Binary => 140,
        }
    }

    pub fn max_chars_multipart(&self) -> usize {
        // With UDH, capacity is reduced
        match self {
            SmsEncoding::Gsm7Bit | SmsEncoding::Gsm7BitExtended => 153,
            SmsEncoding::Ucs2 => 67,
            SmsEncoding::Binary => 134,
        }
    }

    pub fn data_coding_scheme(&self) -> u8 {
        match self {
            SmsEncoding::Gsm7Bit | SmsEncoding::Gsm7BitExtended => 0x00,
            SmsEncoding::Ucs2 => 0x08,
            SmsEncoding::Binary => 0x04,
        }
    }
}

/// SMS message type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SmsType {
    #[default]
    /// Normal SMS
    Sms,
    /// Flash SMS (displayed immediately)
    Flash,
    /// Voice mail notification
    VoiceMail,
    /// Status report
    StatusReport,
    /// WAP Push
    WapPush,
    /// Class 0 (immediate display)
    Class0,
    /// Class 1 (ME specific)
    Class1,
    /// Class 2 (SIM specific)
    Class2,
    /// Class 3 (TE specific)
    Class3,
}

/// SMS status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SmsStatus {
    #[default]
    Unknown,
    Received,
    Read,
    Sent,
    Delivered,
    Failed,
    Pending,
    Draft,
}

impl SmsStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SmsStatus::Unknown => "Unknown",
            SmsStatus::Received => "Received",
            SmsStatus::Read => "Read",
            SmsStatus::Sent => "Sent",
            SmsStatus::Delivered => "Delivered",
            SmsStatus::Failed => "Failed",
            SmsStatus::Pending => "Pending",
            SmsStatus::Draft => "Draft",
        }
    }
}

/// Phone number type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumberType {
    #[default]
    Unknown,
    International,
    National,
    NetworkSpecific,
    Subscriber,
    Alphanumeric,
    Abbreviated,
}

impl NumberType {
    pub fn type_of_number(&self) -> u8 {
        match self {
            NumberType::Unknown => 0b000,
            NumberType::International => 0b001,
            NumberType::National => 0b010,
            NumberType::NetworkSpecific => 0b011,
            NumberType::Subscriber => 0b100,
            NumberType::Alphanumeric => 0b101,
            NumberType::Abbreviated => 0b110,
        }
    }

    pub fn numbering_plan(&self) -> u8 {
        match self {
            NumberType::Alphanumeric => 0b0000,
            _ => 0b0001, // ISDN/telephone
        }
    }

    pub fn to_byte(&self) -> u8 {
        0x80 | (self.type_of_number() << 4) | self.numbering_plan()
    }
}

/// SMS address (phone number)
#[derive(Debug, Clone, Default)]
pub struct SmsAddress {
    pub number: String,
    pub number_type: NumberType,
}

impl SmsAddress {
    pub fn new(number: String) -> Self {
        let number_type = if number.starts_with('+') {
            NumberType::International
        } else if number.chars().all(|c| c.is_ascii_digit() || c == '+') {
            NumberType::National
        } else {
            NumberType::Alphanumeric
        };

        Self { number, number_type }
    }

    pub fn international(number: &str) -> Self {
        let num = if number.starts_with('+') {
            String::from(number)
        } else {
            let mut s = String::from("+");
            s.push_str(number);
            s
        };

        Self {
            number: num,
            number_type: NumberType::International,
        }
    }

    /// Encode to PDU format
    pub fn to_pdu(&self) -> Vec<u8> {
        let mut pdu = Vec::new();

        // Remove '+' for encoding
        let digits: String = self.number.chars()
            .filter(|c| c.is_ascii_digit())
            .collect();

        // Address length (number of digits)
        pdu.push(digits.len() as u8);

        // Type of address
        pdu.push(self.number_type.to_byte());

        // BCD encoded digits (swapped nibbles)
        let mut i = 0;
        while i < digits.len() {
            let d1 = digits.chars().nth(i).unwrap().to_digit(16).unwrap() as u8;
            let d2 = if i + 1 < digits.len() {
                digits.chars().nth(i + 1).unwrap().to_digit(16).unwrap() as u8
            } else {
                0x0F // Padding
            };
            pdu.push((d2 << 4) | d1);
            i += 2;
        }

        pdu
    }
}

/// SMS timestamp
#[derive(Debug, Clone, Default)]
pub struct SmsTimestamp {
    pub year: u8,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub timezone: i8, // Quarter hours from UTC
}

impl SmsTimestamp {
    pub fn now() -> Self {
        // Would get from RTC
        Self {
            year: 26,
            month: 1,
            day: 18,
            hour: 12,
            minute: 0,
            second: 0,
            timezone: 0,
        }
    }

    /// Encode to PDU format (7 bytes, BCD swapped)
    pub fn to_pdu(&self) -> [u8; 7] {
        [
            Self::bcd_swap(self.year),
            Self::bcd_swap(self.month),
            Self::bcd_swap(self.day),
            Self::bcd_swap(self.hour),
            Self::bcd_swap(self.minute),
            Self::bcd_swap(self.second),
            Self::bcd_swap(self.timezone.unsigned_abs()),
        ]
    }

    fn bcd_swap(val: u8) -> u8 {
        let high = val / 10;
        let low = val % 10;
        (low << 4) | high
    }

    pub fn to_string(&self) -> String {
        use core::fmt::Write;
        let mut s = String::new();
        let _ = write!(s, "20{:02}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day,
            self.hour, self.minute, self.second);
        s
    }
}

/// User Data Header (for multipart SMS)
#[derive(Debug, Clone, Default)]
pub struct UserDataHeader {
    /// Concatenated message reference
    pub concat_ref: Option<ConcatInfo>,
    /// Port addressing
    pub port_addressing: Option<PortAddressing>,
    /// Other information elements
    pub other: Vec<(u8, Vec<u8>)>,
}

impl UserDataHeader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_concat(ref_num: u16, total: u8, seq: u8) -> Self {
        Self {
            concat_ref: Some(ConcatInfo {
                ref_num,
                total_parts: total,
                seq_num: seq,
                is_16bit: ref_num > 255,
            }),
            ..Default::default()
        }
    }

    pub fn len(&self) -> usize {
        let mut len = 1; // UDHL

        if let Some(ref concat) = self.concat_ref {
            len += if concat.is_16bit { 6 } else { 5 };
        }

        if let Some(ref _ports) = self.port_addressing {
            len += 6;
        }

        for (_, data) in &self.other {
            len += 2 + data.len();
        }

        len
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Calculate UDHL
        let udhl = self.len() - 1;
        bytes.push(udhl as u8);

        // Concatenated message IE
        if let Some(ref concat) = self.concat_ref {
            if concat.is_16bit {
                bytes.push(0x08); // IEI for 16-bit ref
                bytes.push(4);   // IEI length
                bytes.push((concat.ref_num >> 8) as u8);
                bytes.push(concat.ref_num as u8);
            } else {
                bytes.push(0x00); // IEI for 8-bit ref
                bytes.push(3);   // IEI length
                bytes.push(concat.ref_num as u8);
            }
            bytes.push(concat.total_parts);
            bytes.push(concat.seq_num);
        }

        // Port addressing IE
        if let Some(ref ports) = self.port_addressing {
            bytes.push(0x05); // IEI
            bytes.push(4);   // Length
            bytes.push((ports.dest_port >> 8) as u8);
            bytes.push(ports.dest_port as u8);
            bytes.push((ports.src_port >> 8) as u8);
            bytes.push(ports.src_port as u8);
        }

        // Other IEs
        for (iei, data) in &self.other {
            bytes.push(*iei);
            bytes.push(data.len() as u8);
            bytes.extend_from_slice(data);
        }

        bytes
    }
}

/// Concatenated message info
#[derive(Debug, Clone)]
pub struct ConcatInfo {
    pub ref_num: u16,
    pub total_parts: u8,
    pub seq_num: u8,
    pub is_16bit: bool,
}

/// Port addressing for WAP Push, etc.
#[derive(Debug, Clone)]
pub struct PortAddressing {
    pub dest_port: u16,
    pub src_port: u16,
}

/// SMS message
#[derive(Debug, Clone)]
pub struct SmsMessage {
    /// Message ID
    pub id: u64,
    /// Message type
    pub msg_type: SmsType,
    /// Status
    pub status: SmsStatus,
    /// Sender/recipient address
    pub address: SmsAddress,
    /// Service center address
    pub smsc: Option<SmsAddress>,
    /// Timestamp
    pub timestamp: SmsTimestamp,
    /// Encoding
    pub encoding: SmsEncoding,
    /// Message text
    pub text: String,
    /// Raw data (for binary messages)
    pub data: Vec<u8>,
    /// User data header
    pub udh: Option<UserDataHeader>,
    /// Delivery report requested
    pub delivery_report: bool,
    /// Reply path
    pub reply_path: bool,
    /// Part of multipart message
    pub multipart_ref: Option<u16>,
    /// Multipart sequence
    pub multipart_seq: Option<u8>,
    /// Multipart total
    pub multipart_total: Option<u8>,
}

impl SmsMessage {
    pub fn new(address: SmsAddress, text: String) -> Self {
        let encoding = Self::detect_encoding(&text);

        Self {
            id: 0,
            msg_type: SmsType::Sms,
            status: SmsStatus::Draft,
            address,
            smsc: None,
            timestamp: SmsTimestamp::now(),
            encoding,
            text,
            data: Vec::new(),
            udh: None,
            delivery_report: false,
            reply_path: false,
            multipart_ref: None,
            multipart_seq: None,
            multipart_total: None,
        }
    }

    /// Detect optimal encoding for text
    pub fn detect_encoding(text: &str) -> SmsEncoding {
        for c in text.chars() {
            if !Self::is_gsm7_char(c) {
                return SmsEncoding::Ucs2;
            }
        }
        SmsEncoding::Gsm7Bit
    }

    /// Check if character is in GSM 7-bit alphabet
    fn is_gsm7_char(c: char) -> bool {
        // Basic GSM 7-bit characters
        matches!(c,
            '@' | '£' | '$' | '¥' | 'è' | 'é' | 'ù' | 'ì' | 'ò' | 'Ç' |
            '\n' | 'Ø' | 'ø' | '\r' | 'Å' | 'å' | 'Δ' | '_' | 'Φ' | 'Γ' |
            'Λ' | 'Ω' | 'Π' | 'Ψ' | 'Σ' | 'Θ' | 'Ξ' | ' '..='~' | 'Ä' |
            'Ö' | 'Ñ' | 'Ü' | '§' | '¿' | 'ä' | 'ö' | 'ñ' | 'ü' | 'à' |
            'Æ' | 'æ' | 'ß' | 'É'
        )
    }

    /// Calculate number of parts needed
    pub fn parts_needed(&self) -> usize {
        let char_count = self.text.chars().count();
        let max_single = self.encoding.max_chars();
        let max_multi = self.encoding.max_chars_multipart();

        if char_count <= max_single {
            1
        } else {
            (char_count + max_multi - 1) / max_multi
        }
    }

    /// Encode message to PDU format
    pub fn to_pdu(&self) -> Vec<u8> {
        let mut pdu = Vec::new();

        // SMSC address (empty = use default)
        pdu.push(0x00);

        // First octet
        let mut first_octet = 0x01; // SMS-SUBMIT
        if self.udh.is_some() {
            first_octet |= 0x40; // UDHI
        }
        if self.delivery_report {
            first_octet |= 0x20; // SRR
        }
        if self.reply_path {
            first_octet |= 0x80; // RP
        }
        pdu.push(first_octet);

        // Message reference (auto-assigned)
        pdu.push(0x00);

        // Destination address
        pdu.extend_from_slice(&self.address.to_pdu());

        // Protocol identifier
        pdu.push(0x00);

        // Data coding scheme
        pdu.push(self.encoding.data_coding_scheme());

        // Validity period (relative, 1 day)
        pdu.push(0xA7);

        // User data
        let ud = self.encode_user_data();
        pdu.push(ud.len() as u8);
        pdu.extend_from_slice(&ud);

        pdu
    }

    /// Encode user data
    fn encode_user_data(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Add UDH if present
        if let Some(ref udh) = self.udh {
            data.extend_from_slice(&udh.to_bytes());
        }

        // Encode text based on encoding
        match self.encoding {
            SmsEncoding::Gsm7Bit | SmsEncoding::Gsm7BitExtended => {
                data.extend_from_slice(&self.encode_gsm7(&self.text));
            }
            SmsEncoding::Ucs2 => {
                data.extend_from_slice(&self.encode_ucs2(&self.text));
            }
            SmsEncoding::Binary => {
                data.extend_from_slice(&self.data);
            }
        }

        data
    }

    /// Encode text as GSM 7-bit packed
    fn encode_gsm7(&self, text: &str) -> Vec<u8> {
        let septets: Vec<u8> = text.chars()
            .map(|c| Self::char_to_gsm7(c))
            .collect();

        // Pack 7-bit values into bytes
        let mut packed = Vec::new();
        let mut shift = 0;
        let mut prev = 0u8;

        for septet in septets {
            if shift == 0 {
                prev = septet;
            } else {
                packed.push(prev | (septet << (8 - shift)));
                prev = septet >> shift;
            }
            shift = (shift + 1) % 8;
            if shift == 0 {
                packed.push(prev);
                prev = 0;
            }
        }

        if shift != 0 {
            packed.push(prev);
        }

        packed
    }

    /// Convert character to GSM 7-bit code
    fn char_to_gsm7(c: char) -> u8 {
        match c {
            '@' => 0x00,
            '£' => 0x01,
            '$' => 0x02,
            '¥' => 0x03,
            'è' => 0x04,
            'é' => 0x05,
            'ù' => 0x06,
            'ì' => 0x07,
            'ò' => 0x08,
            'Ç' => 0x09,
            '\n' => 0x0A,
            'Ø' => 0x0B,
            'ø' => 0x0C,
            '\r' => 0x0D,
            'Å' => 0x0E,
            'å' => 0x0F,
            'Δ' => 0x10,
            '_' => 0x11,
            'Φ' => 0x12,
            'Γ' => 0x13,
            'Λ' => 0x14,
            'Ω' => 0x15,
            'Π' => 0x16,
            'Ψ' => 0x17,
            'Σ' => 0x18,
            'Θ' => 0x19,
            'Ξ' => 0x1A,
            ' ' => 0x20,
            '!'..='?' => c as u8,
            'A'..='Z' => c as u8,
            'a'..='z' => c as u8,
            'Ä' => 0x5B,
            'Ö' => 0x5C,
            'Ñ' => 0x5D,
            'Ü' => 0x5E,
            '§' => 0x5F,
            '¿' => 0x60,
            'ä' => 0x7B,
            'ö' => 0x7C,
            'ñ' => 0x7D,
            'ü' => 0x7E,
            'à' => 0x7F,
            _ => 0x3F, // '?' for unknown
        }
    }

    /// Encode text as UCS-2 (UTF-16BE)
    fn encode_ucs2(&self, text: &str) -> Vec<u8> {
        let mut data = Vec::new();
        for c in text.encode_utf16() {
            data.push((c >> 8) as u8);
            data.push(c as u8);
        }
        data
    }
}

/// MMS content type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MmsContentType {
    #[default]
    TextPlain,
    TextHtml,
    ImageJpeg,
    ImagePng,
    ImageGif,
    AudioMp3,
    AudioAmr,
    VideoMp4,
    Video3gp,
    ApplicationSmil,
    Multipart,
}

impl MmsContentType {
    pub fn mime_type(&self) -> &'static str {
        match self {
            MmsContentType::TextPlain => "text/plain",
            MmsContentType::TextHtml => "text/html",
            MmsContentType::ImageJpeg => "image/jpeg",
            MmsContentType::ImagePng => "image/png",
            MmsContentType::ImageGif => "image/gif",
            MmsContentType::AudioMp3 => "audio/mpeg",
            MmsContentType::AudioAmr => "audio/amr",
            MmsContentType::VideoMp4 => "video/mp4",
            MmsContentType::Video3gp => "video/3gpp",
            MmsContentType::ApplicationSmil => "application/smil",
            MmsContentType::Multipart => "multipart/related",
        }
    }

    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "txt" => MmsContentType::TextPlain,
            "html" | "htm" => MmsContentType::TextHtml,
            "jpg" | "jpeg" => MmsContentType::ImageJpeg,
            "png" => MmsContentType::ImagePng,
            "gif" => MmsContentType::ImageGif,
            "mp3" => MmsContentType::AudioMp3,
            "amr" => MmsContentType::AudioAmr,
            "mp4" => MmsContentType::VideoMp4,
            "3gp" => MmsContentType::Video3gp,
            "smil" => MmsContentType::ApplicationSmil,
            _ => MmsContentType::TextPlain,
        }
    }
}

/// MMS attachment
#[derive(Debug, Clone)]
pub struct MmsAttachment {
    /// Content ID
    pub content_id: String,
    /// Content type
    pub content_type: MmsContentType,
    /// Filename
    pub filename: String,
    /// Data
    pub data: Vec<u8>,
}

impl MmsAttachment {
    pub fn new(filename: String, data: Vec<u8>) -> Self {
        let ext = filename.rsplit('.').next().unwrap_or("");
        let content_type = MmsContentType::from_extension(ext);

        Self {
            content_id: filename.clone(),
            content_type,
            filename,
            data,
        }
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }
}

/// MMS message
#[derive(Debug, Clone)]
pub struct MmsMessage {
    /// Message ID
    pub id: u64,
    /// Transaction ID
    pub transaction_id: String,
    /// Status
    pub status: SmsStatus,
    /// Sender
    pub from: SmsAddress,
    /// Recipients
    pub to: Vec<SmsAddress>,
    /// CC recipients
    pub cc: Vec<SmsAddress>,
    /// Subject
    pub subject: String,
    /// Timestamp
    pub timestamp: SmsTimestamp,
    /// Text body
    pub text: String,
    /// Attachments
    pub attachments: Vec<MmsAttachment>,
    /// Delivery report requested
    pub delivery_report: bool,
    /// Read report requested
    pub read_report: bool,
    /// Priority (0-2)
    pub priority: u8,
    /// Expiry (seconds)
    pub expiry: u32,
}

impl MmsMessage {
    pub fn new(to: SmsAddress, subject: String, text: String) -> Self {
        Self {
            id: 0,
            transaction_id: String::new(),
            status: SmsStatus::Draft,
            from: SmsAddress::default(),
            to: vec![to],
            cc: Vec::new(),
            subject,
            timestamp: SmsTimestamp::now(),
            text,
            attachments: Vec::new(),
            delivery_report: false,
            read_report: false,
            priority: 1, // Normal
            expiry: 604800, // 1 week
        }
    }

    pub fn add_attachment(&mut self, attachment: MmsAttachment) -> KResult<()> {
        let total_size: usize = self.attachments.iter().map(|a| a.size()).sum();
        if total_size + attachment.size() > MAX_MMS_SIZE {
            return Err(KError::NoMemory);
        }
        self.attachments.push(attachment);
        Ok(())
    }

    pub fn total_size(&self) -> usize {
        self.text.len() + self.attachments.iter().map(|a| a.size()).sum::<usize>()
    }

    /// Encode to MMS PDU (M-Send.req)
    pub fn to_pdu(&self) -> Vec<u8> {
        let mut pdu = Vec::new();

        // X-Mms-Message-Type: m-send-req
        pdu.push(0x8C);
        pdu.push(0x80);

        // X-Mms-Transaction-ID
        pdu.push(0x98);
        pdu.extend_from_slice(self.transaction_id.as_bytes());
        pdu.push(0x00);

        // X-Mms-MMS-Version: 1.0
        pdu.push(0x8D);
        pdu.push(0x90);

        // From
        pdu.push(0x89);
        pdu.push(0x01);
        pdu.push(0x81); // Insert-address-token

        // To
        for to in &self.to {
            pdu.push(0x97);
            let addr = to.number.as_bytes();
            pdu.extend_from_slice(addr);
            pdu.push(0x00);
        }

        // Subject
        if !self.subject.is_empty() {
            pdu.push(0x96);
            pdu.extend_from_slice(self.subject.as_bytes());
            pdu.push(0x00);
        }

        // Content-Type: multipart/related
        pdu.push(0x84);
        pdu.push(0xB3); // multipart/related

        // Body parts would follow...

        pdu
    }
}

/// WAP Push message
#[derive(Debug, Clone)]
pub struct WapPush {
    /// Content type
    pub content_type: WapContentType,
    /// URL
    pub url: String,
    /// Text
    pub text: String,
    /// Action
    pub action: WapAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WapContentType {
    #[default]
    ServiceIndication,
    ServiceLoading,
    CacheOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WapAction {
    #[default]
    Signal,
    Execute,
}

/// Message statistics
#[derive(Debug, Default)]
pub struct SmsStats {
    pub sms_sent: AtomicU64,
    pub sms_received: AtomicU64,
    pub sms_failed: AtomicU64,
    pub mms_sent: AtomicU64,
    pub mms_received: AtomicU64,
    pub mms_failed: AtomicU64,
}

impl SmsStats {
    pub const fn new() -> Self {
        Self {
            sms_sent: AtomicU64::new(0),
            sms_received: AtomicU64::new(0),
            sms_failed: AtomicU64::new(0),
            mms_sent: AtomicU64::new(0),
            mms_received: AtomicU64::new(0),
            mms_failed: AtomicU64::new(0),
        }
    }
}

/// SMS/MMS Manager
pub struct SmsManager {
    /// Stored SMS messages
    sms_inbox: Vec<SmsMessage>,
    sms_sent: Vec<SmsMessage>,
    sms_drafts: Vec<SmsMessage>,
    /// Stored MMS messages
    mms_inbox: Vec<MmsMessage>,
    mms_sent: Vec<MmsMessage>,
    mms_drafts: Vec<MmsMessage>,
    /// Multipart assembly buffer
    multipart_buffer: Vec<SmsMessage>,
    /// Next message ID
    next_id: AtomicU64,
    /// Statistics
    stats: SmsStats,
    /// SMSC address
    smsc: Option<SmsAddress>,
    /// MMS proxy
    mms_proxy: Option<String>,
    /// MMS port
    mms_port: u16,
}

impl SmsManager {
    pub const fn new() -> Self {
        Self {
            sms_inbox: Vec::new(),
            sms_sent: Vec::new(),
            sms_drafts: Vec::new(),
            mms_inbox: Vec::new(),
            mms_sent: Vec::new(),
            mms_drafts: Vec::new(),
            multipart_buffer: Vec::new(),
            next_id: AtomicU64::new(1),
            stats: SmsStats::new(),
            smsc: None,
            mms_proxy: None,
            mms_port: 8080,
        }
    }

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Send SMS
    pub fn send_sms(&mut self, to: &str, text: &str) -> KResult<u64> {
        let address = SmsAddress::new(String::from(to));
        let mut message = SmsMessage::new(address, String::from(text));
        message.id = self.next_id();

        let parts = message.parts_needed();

        if parts == 1 {
            // Single part SMS
            let _pdu = message.to_pdu();
            // Would send via modem AT+CMGS
            message.status = SmsStatus::Sent;
            self.stats.sms_sent.fetch_add(1, Ordering::Relaxed);
        } else {
            // Multipart SMS
            let ref_num = (message.id & 0xFFFF) as u16;
            let text_chars: Vec<char> = message.text.chars().collect();
            let max_chars = message.encoding.max_chars_multipart();

            for i in 0..parts {
                let start = i * max_chars;
                let end = core::cmp::min(start + max_chars, text_chars.len());
                let part_text: String = text_chars[start..end].iter().collect();

                let mut part = SmsMessage::new(message.address.clone(), part_text);
                part.id = self.next_id();
                part.udh = Some(UserDataHeader::with_concat(
                    ref_num,
                    parts as u8,
                    (i + 1) as u8,
                ));
                part.multipart_ref = Some(ref_num);
                part.multipart_seq = Some((i + 1) as u8);
                part.multipart_total = Some(parts as u8);

                let _pdu = part.to_pdu();
                // Would send via modem
                self.stats.sms_sent.fetch_add(1, Ordering::Relaxed);
            }

            message.status = SmsStatus::Sent;
        }

        let id = message.id;
        self.sms_sent.push(message);

        Ok(id)
    }

    /// Receive SMS (called when modem receives message)
    pub fn receive_sms(&mut self, _pdu: &[u8]) -> KResult<u64> {
        // Would parse PDU
        // Placeholder: create dummy message
        let address = SmsAddress::new(String::from("+1234567890"));
        let mut message = SmsMessage::new(address, String::from("Received message"));
        message.id = self.next_id();
        message.status = SmsStatus::Received;

        let id = message.id;
        self.sms_inbox.push(message);
        self.stats.sms_received.fetch_add(1, Ordering::Relaxed);

        Ok(id)
    }

    /// Send MMS
    pub fn send_mms(&mut self, to: &str, subject: &str, text: &str) -> KResult<u64> {
        let address = SmsAddress::new(String::from(to));
        let mut message = MmsMessage::new(address, String::from(subject), String::from(text));
        message.id = self.next_id();
        message.transaction_id = format!("{:016x}", message.id);

        let _pdu = message.to_pdu();
        // Would send via MMS proxy
        message.status = SmsStatus::Sent;
        self.stats.mms_sent.fetch_add(1, Ordering::Relaxed);

        let id = message.id;
        self.mms_sent.push(message);

        Ok(id)
    }

    /// Get inbox messages
    pub fn inbox(&self) -> &[SmsMessage] {
        &self.sms_inbox
    }

    /// Get sent messages
    pub fn sent(&self) -> &[SmsMessage] {
        &self.sms_sent
    }

    /// Get MMS inbox
    pub fn mms_inbox(&self) -> &[MmsMessage] {
        &self.mms_inbox
    }

    /// Delete message
    pub fn delete_sms(&mut self, id: u64) -> KResult<()> {
        if let Some(pos) = self.sms_inbox.iter().position(|m| m.id == id) {
            self.sms_inbox.remove(pos);
            return Ok(());
        }
        if let Some(pos) = self.sms_sent.iter().position(|m| m.id == id) {
            self.sms_sent.remove(pos);
            return Ok(());
        }
        Err(KError::NotFound)
    }

    /// Mark message as read
    pub fn mark_read(&mut self, id: u64) -> KResult<()> {
        if let Some(msg) = self.sms_inbox.iter_mut().find(|m| m.id == id) {
            msg.status = SmsStatus::Read;
            return Ok(());
        }
        Err(KError::NotFound)
    }

    /// Get unread count
    pub fn unread_count(&self) -> usize {
        self.sms_inbox.iter()
            .filter(|m| m.status == SmsStatus::Received)
            .count()
    }

    /// Set SMSC address
    pub fn set_smsc(&mut self, smsc: &str) {
        self.smsc = Some(SmsAddress::new(String::from(smsc)));
    }

    /// Set MMS proxy
    pub fn set_mms_proxy(&mut self, proxy: &str, port: u16) {
        self.mms_proxy = Some(String::from(proxy));
        self.mms_port = port;
    }

    /// Get statistics
    pub fn stats(&self) -> SmsStatsSnapshot {
        SmsStatsSnapshot {
            sms_sent: self.stats.sms_sent.load(Ordering::Relaxed),
            sms_received: self.stats.sms_received.load(Ordering::Relaxed),
            sms_failed: self.stats.sms_failed.load(Ordering::Relaxed),
            mms_sent: self.stats.mms_sent.load(Ordering::Relaxed),
            mms_received: self.stats.mms_received.load(Ordering::Relaxed),
            mms_failed: self.stats.mms_failed.load(Ordering::Relaxed),
            inbox_count: self.sms_inbox.len(),
            sent_count: self.sms_sent.len(),
            unread_count: self.unread_count(),
        }
    }
}

/// Statistics snapshot
#[derive(Debug, Clone)]
pub struct SmsStatsSnapshot {
    pub sms_sent: u64,
    pub sms_received: u64,
    pub sms_failed: u64,
    pub mms_sent: u64,
    pub mms_received: u64,
    pub mms_failed: u64,
    pub inbox_count: usize,
    pub sent_count: usize,
    pub unread_count: usize,
}

/// Global SMS manager
static SMS_MANAGER: IrqSafeMutex<SmsManager> = IrqSafeMutex::new(SmsManager::new());

/// Initialize SMS subsystem
pub fn init() {
    // Nothing to initialize
}

/// Send SMS
pub fn send_sms(to: &str, text: &str) -> KResult<u64> {
    SMS_MANAGER.lock().send_sms(to, text)
}

/// Receive SMS (from modem)
pub fn receive_sms(pdu: &[u8]) -> KResult<u64> {
    SMS_MANAGER.lock().receive_sms(pdu)
}

/// Send MMS
pub fn send_mms(to: &str, subject: &str, text: &str) -> KResult<u64> {
    SMS_MANAGER.lock().send_mms(to, subject, text)
}

/// Get inbox
pub fn inbox() -> Vec<SmsMessage> {
    SMS_MANAGER.lock().inbox().to_vec()
}

/// Get sent messages
pub fn sent() -> Vec<SmsMessage> {
    SMS_MANAGER.lock().sent().to_vec()
}

/// Delete message
pub fn delete_sms(id: u64) -> KResult<()> {
    SMS_MANAGER.lock().delete_sms(id)
}

/// Mark message as read
pub fn mark_read(id: u64) -> KResult<()> {
    SMS_MANAGER.lock().mark_read(id)
}

/// Get unread count
pub fn unread_count() -> usize {
    SMS_MANAGER.lock().unread_count()
}

/// Get statistics
pub fn stats() -> SmsStatsSnapshot {
    SMS_MANAGER.lock().stats()
}
