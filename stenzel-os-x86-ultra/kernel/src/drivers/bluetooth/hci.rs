//! HCI (Host Controller Interface) Protocol
//!
//! Implements Bluetooth HCI commands, events, and packet handling.

extern crate alloc;

use alloc::vec::Vec;
use super::BdAddr;


/// HCI packet types
pub mod packet_types {
    pub const COMMAND: u8 = 0x01;
    pub const ACL_DATA: u8 = 0x02;
    pub const SCO_DATA: u8 = 0x03;
    pub const EVENT: u8 = 0x04;
    pub const ISO_DATA: u8 = 0x05;
}

/// Inquiry Access Codes (LAP)
pub const LAP_GIAC: u32 = 0x9E8B33; // General Inquiry Access Code
pub const LAP_LIAC: u32 = 0x9E8B00; // Limited Inquiry Access Code

/// HCI events
pub mod events {
    pub const INQUIRY_COMPLETE: u8 = 0x01;
    pub const INQUIRY_RESULT: u8 = 0x02;
    pub const CONNECTION_COMPLETE: u8 = 0x03;
    pub const CONNECTION_REQUEST: u8 = 0x04;
    pub const DISCONNECTION_COMPLETE: u8 = 0x05;
    pub const AUTHENTICATION_COMPLETE: u8 = 0x06;
    pub const REMOTE_NAME_REQUEST_COMPLETE: u8 = 0x07;
    pub const ENCRYPTION_CHANGE: u8 = 0x08;
    pub const CHANGE_CONNECTION_LINK_KEY_COMPLETE: u8 = 0x09;
    pub const MASTER_LINK_KEY_COMPLETE: u8 = 0x0A;
    pub const READ_REMOTE_SUPPORTED_FEATURES_COMPLETE: u8 = 0x0B;
    pub const READ_REMOTE_VERSION_INFORMATION_COMPLETE: u8 = 0x0C;
    pub const QOS_SETUP_COMPLETE: u8 = 0x0D;
    pub const COMMAND_COMPLETE: u8 = 0x0E;
    pub const COMMAND_STATUS: u8 = 0x0F;
    pub const HARDWARE_ERROR: u8 = 0x10;
    pub const FLUSH_OCCURRED: u8 = 0x11;
    pub const ROLE_CHANGE: u8 = 0x12;
    pub const NUMBER_OF_COMPLETED_PACKETS: u8 = 0x13;
    pub const MODE_CHANGE: u8 = 0x14;
    pub const RETURN_LINK_KEYS: u8 = 0x15;
    pub const PIN_CODE_REQUEST: u8 = 0x16;
    pub const LINK_KEY_REQUEST: u8 = 0x17;
    pub const LINK_KEY_NOTIFICATION: u8 = 0x18;
    pub const LOOPBACK_COMMAND: u8 = 0x19;
    pub const DATA_BUFFER_OVERFLOW: u8 = 0x1A;
    pub const MAX_SLOTS_CHANGE: u8 = 0x1B;
    pub const READ_CLOCK_OFFSET_COMPLETE: u8 = 0x1C;
    pub const CONNECTION_PACKET_TYPE_CHANGED: u8 = 0x1D;
    pub const QOS_VIOLATION: u8 = 0x1E;
    pub const PAGE_SCAN_REPETITION_MODE_CHANGE: u8 = 0x20;
    pub const FLOW_SPECIFICATION_COMPLETE: u8 = 0x21;
    pub const INQUIRY_RESULT_WITH_RSSI: u8 = 0x22;
    pub const READ_REMOTE_EXTENDED_FEATURES_COMPLETE: u8 = 0x23;
    pub const SYNCHRONOUS_CONNECTION_COMPLETE: u8 = 0x2C;
    pub const SYNCHRONOUS_CONNECTION_CHANGED: u8 = 0x2D;
    pub const SNIFF_SUBRATING: u8 = 0x2E;
    pub const EXTENDED_INQUIRY_RESULT: u8 = 0x2F;
    pub const ENCRYPTION_KEY_REFRESH_COMPLETE: u8 = 0x30;
    pub const IO_CAPABILITY_REQUEST: u8 = 0x31;
    pub const IO_CAPABILITY_RESPONSE: u8 = 0x32;
    pub const USER_CONFIRMATION_REQUEST: u8 = 0x33;
    pub const USER_PASSKEY_REQUEST: u8 = 0x34;
    pub const REMOTE_OOB_DATA_REQUEST: u8 = 0x35;
    pub const SIMPLE_PAIRING_COMPLETE: u8 = 0x36;
    pub const LINK_SUPERVISION_TIMEOUT_CHANGED: u8 = 0x38;
    pub const ENHANCED_FLUSH_COMPLETE: u8 = 0x39;
    pub const USER_PASSKEY_NOTIFICATION: u8 = 0x3B;
    pub const KEYPRESS_NOTIFICATION: u8 = 0x3C;
    pub const REMOTE_HOST_SUPPORTED_FEATURES_NOTIFICATION: u8 = 0x3D;
    pub const LE_META_EVENT: u8 = 0x3E;
    pub const NUMBER_OF_COMPLETED_DATA_BLOCKS: u8 = 0x48;
    pub const AUTHENTICATED_PAYLOAD_TIMEOUT_EXPIRED: u8 = 0x57;
}

/// LE Meta event sub-events
pub mod le_events {
    pub const CONNECTION_COMPLETE: u8 = 0x01;
    pub const ADVERTISING_REPORT: u8 = 0x02;
    pub const CONNECTION_UPDATE_COMPLETE: u8 = 0x03;
    pub const READ_REMOTE_FEATURES_COMPLETE: u8 = 0x04;
    pub const LONG_TERM_KEY_REQUEST: u8 = 0x05;
    pub const REMOTE_CONNECTION_PARAMETER_REQUEST: u8 = 0x06;
    pub const DATA_LENGTH_CHANGE: u8 = 0x07;
    pub const READ_LOCAL_P256_PUBLIC_KEY_COMPLETE: u8 = 0x08;
    pub const GENERATE_DHKEY_COMPLETE: u8 = 0x09;
    pub const ENHANCED_CONNECTION_COMPLETE: u8 = 0x0A;
    pub const DIRECTED_ADVERTISING_REPORT: u8 = 0x0B;
    pub const PHY_UPDATE_COMPLETE: u8 = 0x0C;
    pub const EXTENDED_ADVERTISING_REPORT: u8 = 0x0D;
    pub const PERIODIC_ADVERTISING_SYNC_ESTABLISHED: u8 = 0x0E;
    pub const PERIODIC_ADVERTISING_REPORT: u8 = 0x0F;
    pub const PERIODIC_ADVERTISING_SYNC_LOST: u8 = 0x10;
    pub const SCAN_TIMEOUT: u8 = 0x11;
    pub const ADVERTISING_SET_TERMINATED: u8 = 0x12;
    pub const SCAN_REQUEST_RECEIVED: u8 = 0x13;
    pub const CHANNEL_SELECTION_ALGORITHM: u8 = 0x14;
}

/// HCI error codes
pub mod errors {
    pub const SUCCESS: u8 = 0x00;
    pub const UNKNOWN_HCI_COMMAND: u8 = 0x01;
    pub const UNKNOWN_CONNECTION_ID: u8 = 0x02;
    pub const HARDWARE_FAILURE: u8 = 0x03;
    pub const PAGE_TIMEOUT: u8 = 0x04;
    pub const AUTHENTICATION_FAILURE: u8 = 0x05;
    pub const PIN_OR_KEY_MISSING: u8 = 0x06;
    pub const MEMORY_CAPACITY_EXCEEDED: u8 = 0x07;
    pub const CONNECTION_TIMEOUT: u8 = 0x08;
    pub const CONNECTION_LIMIT_EXCEEDED: u8 = 0x09;
    pub const SYNCHRONOUS_CONNECTION_LIMIT_EXCEEDED: u8 = 0x0A;
    pub const CONNECTION_ALREADY_EXISTS: u8 = 0x0B;
    pub const COMMAND_DISALLOWED: u8 = 0x0C;
    pub const CONNECTION_REJECTED_LIMITED_RESOURCES: u8 = 0x0D;
    pub const CONNECTION_REJECTED_SECURITY_REASONS: u8 = 0x0E;
    pub const CONNECTION_REJECTED_UNACCEPTABLE_BD_ADDR: u8 = 0x0F;
    pub const CONNECTION_ACCEPT_TIMEOUT_EXCEEDED: u8 = 0x10;
    pub const UNSUPPORTED_FEATURE_OR_PARAMETER: u8 = 0x11;
    pub const INVALID_HCI_COMMAND_PARAMETERS: u8 = 0x12;
    pub const REMOTE_USER_TERMINATED_CONNECTION: u8 = 0x13;
    pub const REMOTE_DEVICE_TERMINATED_LOW_RESOURCES: u8 = 0x14;
    pub const REMOTE_DEVICE_TERMINATED_POWER_OFF: u8 = 0x15;
    pub const CONNECTION_TERMINATED_BY_LOCAL_HOST: u8 = 0x16;
    pub const REPEATED_ATTEMPTS: u8 = 0x17;
    pub const PAIRING_NOT_ALLOWED: u8 = 0x18;
    pub const UNKNOWN_LMP_PDU: u8 = 0x19;
    pub const UNSUPPORTED_REMOTE_FEATURE: u8 = 0x1A;
    pub const SCO_OFFSET_REJECTED: u8 = 0x1B;
    pub const SCO_INTERVAL_REJECTED: u8 = 0x1C;
    pub const SCO_AIR_MODE_REJECTED: u8 = 0x1D;
    pub const INVALID_LMP_OR_LL_PARAMETERS: u8 = 0x1E;
    pub const UNSPECIFIED_ERROR: u8 = 0x1F;
    pub const UNSUPPORTED_LMP_OR_LL_PARAMETER: u8 = 0x20;
    pub const ROLE_CHANGE_NOT_ALLOWED: u8 = 0x21;
    pub const LMP_OR_LL_RESPONSE_TIMEOUT: u8 = 0x22;
    pub const LMP_ERROR_TRANSACTION_COLLISION: u8 = 0x23;
    pub const LMP_PDU_NOT_ALLOWED: u8 = 0x24;
    pub const ENCRYPTION_MODE_NOT_ACCEPTABLE: u8 = 0x25;
    pub const LINK_KEY_CANNOT_BE_CHANGED: u8 = 0x26;
    pub const REQUESTED_QOS_NOT_SUPPORTED: u8 = 0x27;
    pub const INSTANT_PASSED: u8 = 0x28;
    pub const PAIRING_WITH_UNIT_KEY_NOT_SUPPORTED: u8 = 0x29;
    pub const DIFFERENT_TRANSACTION_COLLISION: u8 = 0x2A;
}

/// HCI command opcodes
pub mod commands {
    use alloc::vec::Vec;
    use super::super::BdAddr;

    // Link Control Commands (OGF = 0x01)
    pub const INQUIRY: u16 = 0x0401;
    pub const INQUIRY_CANCEL: u16 = 0x0402;
    pub const PERIODIC_INQUIRY_MODE: u16 = 0x0403;
    pub const EXIT_PERIODIC_INQUIRY_MODE: u16 = 0x0404;
    pub const CREATE_CONNECTION: u16 = 0x0405;
    pub const DISCONNECT: u16 = 0x0406;
    pub const ADD_SCO_CONNECTION: u16 = 0x0407; // Deprecated
    pub const CREATE_CONNECTION_CANCEL: u16 = 0x0408;
    pub const ACCEPT_CONNECTION_REQUEST: u16 = 0x0409;
    pub const REJECT_CONNECTION_REQUEST: u16 = 0x040A;
    pub const LINK_KEY_REQUEST_REPLY: u16 = 0x040B;
    pub const LINK_KEY_REQUEST_NEGATIVE_REPLY: u16 = 0x040C;
    pub const PIN_CODE_REQUEST_REPLY: u16 = 0x040D;
    pub const PIN_CODE_REQUEST_NEGATIVE_REPLY: u16 = 0x040E;
    pub const CHANGE_CONNECTION_PACKET_TYPE: u16 = 0x040F;
    pub const AUTHENTICATION_REQUESTED: u16 = 0x0411;
    pub const SET_CONNECTION_ENCRYPTION: u16 = 0x0413;
    pub const CHANGE_CONNECTION_LINK_KEY: u16 = 0x0415;
    pub const MASTER_LINK_KEY: u16 = 0x0417;
    pub const REMOTE_NAME_REQUEST: u16 = 0x0419;
    pub const REMOTE_NAME_REQUEST_CANCEL: u16 = 0x041A;
    pub const READ_REMOTE_SUPPORTED_FEATURES: u16 = 0x041B;
    pub const READ_REMOTE_EXTENDED_FEATURES: u16 = 0x041C;
    pub const READ_REMOTE_VERSION_INFORMATION: u16 = 0x041D;
    pub const READ_CLOCK_OFFSET: u16 = 0x041F;
    pub const READ_LMP_HANDLE: u16 = 0x0420;
    pub const SETUP_SYNCHRONOUS_CONNECTION: u16 = 0x0428;
    pub const ACCEPT_SYNCHRONOUS_CONNECTION_REQUEST: u16 = 0x0429;
    pub const REJECT_SYNCHRONOUS_CONNECTION_REQUEST: u16 = 0x042A;
    pub const IO_CAPABILITY_REQUEST_REPLY: u16 = 0x042B;
    pub const USER_CONFIRMATION_REQUEST_REPLY: u16 = 0x042C;
    pub const USER_CONFIRMATION_REQUEST_NEGATIVE_REPLY: u16 = 0x042D;
    pub const USER_PASSKEY_REQUEST_REPLY: u16 = 0x042E;
    pub const USER_PASSKEY_REQUEST_NEGATIVE_REPLY: u16 = 0x042F;
    pub const REMOTE_OOB_DATA_REQUEST_REPLY: u16 = 0x0430;
    pub const REMOTE_OOB_DATA_REQUEST_NEGATIVE_REPLY: u16 = 0x0433;
    pub const IO_CAPABILITY_REQUEST_NEGATIVE_REPLY: u16 = 0x0434;

    // Link Policy Commands (OGF = 0x02)
    pub const HOLD_MODE: u16 = 0x0801;
    pub const SNIFF_MODE: u16 = 0x0803;
    pub const EXIT_SNIFF_MODE: u16 = 0x0804;
    pub const PARK_STATE: u16 = 0x0805;
    pub const EXIT_PARK_STATE: u16 = 0x0806;
    pub const QOS_SETUP: u16 = 0x0807;
    pub const ROLE_DISCOVERY: u16 = 0x0809;
    pub const SWITCH_ROLE: u16 = 0x080B;
    pub const READ_LINK_POLICY_SETTINGS: u16 = 0x080C;
    pub const WRITE_LINK_POLICY_SETTINGS: u16 = 0x080D;
    pub const READ_DEFAULT_LINK_POLICY_SETTINGS: u16 = 0x080E;
    pub const WRITE_DEFAULT_LINK_POLICY_SETTINGS: u16 = 0x080F;
    pub const FLOW_SPECIFICATION: u16 = 0x0810;
    pub const SNIFF_SUBRATING: u16 = 0x0811;

    // Controller & Baseband Commands (OGF = 0x03)
    pub const SET_EVENT_MASK: u16 = 0x0C01;
    pub const RESET: u16 = 0x0C03;
    pub const SET_EVENT_FILTER: u16 = 0x0C05;
    pub const FLUSH: u16 = 0x0C08;
    pub const READ_PIN_TYPE: u16 = 0x0C09;
    pub const WRITE_PIN_TYPE: u16 = 0x0C0A;
    pub const CREATE_NEW_UNIT_KEY: u16 = 0x0C0B;
    pub const READ_STORED_LINK_KEY: u16 = 0x0C0D;
    pub const WRITE_STORED_LINK_KEY: u16 = 0x0C11;
    pub const DELETE_STORED_LINK_KEY: u16 = 0x0C12;
    pub const WRITE_LOCAL_NAME: u16 = 0x0C13;
    pub const READ_LOCAL_NAME: u16 = 0x0C14;
    pub const READ_CONNECTION_ACCEPT_TIMEOUT: u16 = 0x0C15;
    pub const WRITE_CONNECTION_ACCEPT_TIMEOUT: u16 = 0x0C16;
    pub const READ_PAGE_TIMEOUT: u16 = 0x0C17;
    pub const WRITE_PAGE_TIMEOUT: u16 = 0x0C18;
    pub const READ_SCAN_ENABLE: u16 = 0x0C19;
    pub const WRITE_SCAN_ENABLE: u16 = 0x0C1A;
    pub const READ_PAGE_SCAN_ACTIVITY: u16 = 0x0C1B;
    pub const WRITE_PAGE_SCAN_ACTIVITY: u16 = 0x0C1C;
    pub const READ_INQUIRY_SCAN_ACTIVITY: u16 = 0x0C1D;
    pub const WRITE_INQUIRY_SCAN_ACTIVITY: u16 = 0x0C1E;
    pub const READ_AUTHENTICATION_ENABLE: u16 = 0x0C1F;
    pub const WRITE_AUTHENTICATION_ENABLE: u16 = 0x0C20;
    pub const READ_CLASS_OF_DEVICE: u16 = 0x0C23;
    pub const WRITE_CLASS_OF_DEVICE: u16 = 0x0C24;
    pub const READ_VOICE_SETTING: u16 = 0x0C25;
    pub const WRITE_VOICE_SETTING: u16 = 0x0C26;
    pub const READ_AUTOMATIC_FLUSH_TIMEOUT: u16 = 0x0C27;
    pub const WRITE_AUTOMATIC_FLUSH_TIMEOUT: u16 = 0x0C28;
    pub const READ_NUM_BROADCAST_RETRANSMISSIONS: u16 = 0x0C29;
    pub const WRITE_NUM_BROADCAST_RETRANSMISSIONS: u16 = 0x0C2A;
    pub const READ_HOLD_MODE_ACTIVITY: u16 = 0x0C2B;
    pub const WRITE_HOLD_MODE_ACTIVITY: u16 = 0x0C2C;
    pub const READ_TRANSMIT_POWER_LEVEL: u16 = 0x0C2D;
    pub const READ_SYNCHRONOUS_FLOW_CONTROL_ENABLE: u16 = 0x0C2E;
    pub const WRITE_SYNCHRONOUS_FLOW_CONTROL_ENABLE: u16 = 0x0C2F;
    pub const SET_CONTROLLER_TO_HOST_FLOW_CONTROL: u16 = 0x0C31;
    pub const HOST_BUFFER_SIZE: u16 = 0x0C33;
    pub const HOST_NUMBER_OF_COMPLETED_PACKETS: u16 = 0x0C35;
    pub const READ_LINK_SUPERVISION_TIMEOUT: u16 = 0x0C36;
    pub const WRITE_LINK_SUPERVISION_TIMEOUT: u16 = 0x0C37;
    pub const READ_NUMBER_OF_SUPPORTED_IAC: u16 = 0x0C38;
    pub const READ_CURRENT_IAC_LAP: u16 = 0x0C39;
    pub const WRITE_CURRENT_IAC_LAP: u16 = 0x0C3A;
    pub const SET_AFH_HOST_CHANNEL_CLASSIFICATION: u16 = 0x0C3F;
    pub const READ_INQUIRY_SCAN_TYPE: u16 = 0x0C42;
    pub const WRITE_INQUIRY_SCAN_TYPE: u16 = 0x0C43;
    pub const READ_INQUIRY_MODE: u16 = 0x0C44;
    pub const WRITE_INQUIRY_MODE: u16 = 0x0C45;
    pub const READ_PAGE_SCAN_TYPE: u16 = 0x0C46;
    pub const WRITE_PAGE_SCAN_TYPE: u16 = 0x0C47;
    pub const READ_AFH_CHANNEL_ASSESSMENT_MODE: u16 = 0x0C48;
    pub const WRITE_AFH_CHANNEL_ASSESSMENT_MODE: u16 = 0x0C49;
    pub const READ_EXTENDED_INQUIRY_RESPONSE: u16 = 0x0C51;
    pub const WRITE_EXTENDED_INQUIRY_RESPONSE: u16 = 0x0C52;
    pub const REFRESH_ENCRYPTION_KEY: u16 = 0x0C53;
    pub const READ_SIMPLE_PAIRING_MODE: u16 = 0x0C55;
    pub const WRITE_SIMPLE_PAIRING_MODE: u16 = 0x0C56;
    pub const READ_LOCAL_OOB_DATA: u16 = 0x0C57;
    pub const READ_INQUIRY_RESPONSE_TRANSMIT_POWER_LEVEL: u16 = 0x0C58;
    pub const WRITE_INQUIRY_TRANSMIT_POWER_LEVEL: u16 = 0x0C59;
    pub const READ_DEFAULT_ERRONEOUS_DATA_REPORTING: u16 = 0x0C5A;
    pub const WRITE_DEFAULT_ERRONEOUS_DATA_REPORTING: u16 = 0x0C5B;
    pub const ENHANCED_FLUSH: u16 = 0x0C5F;
    pub const SEND_KEYPRESS_NOTIFICATION: u16 = 0x0C60;
    pub const READ_LE_HOST_SUPPORT: u16 = 0x0C6C;
    pub const WRITE_LE_HOST_SUPPORT: u16 = 0x0C6D;
    pub const READ_SECURE_CONNECTIONS_HOST_SUPPORT: u16 = 0x0C79;
    pub const WRITE_SECURE_CONNECTIONS_HOST_SUPPORT: u16 = 0x0C7A;
    pub const READ_AUTHENTICATED_PAYLOAD_TIMEOUT: u16 = 0x0C7B;
    pub const WRITE_AUTHENTICATED_PAYLOAD_TIMEOUT: u16 = 0x0C7C;

    // Informational Parameters (OGF = 0x04)
    pub const READ_LOCAL_VERSION: u16 = 0x1001;
    pub const READ_LOCAL_SUPPORTED_COMMANDS: u16 = 0x1002;
    pub const READ_LOCAL_FEATURES: u16 = 0x1003;
    pub const READ_LOCAL_EXTENDED_FEATURES: u16 = 0x1004;
    pub const READ_BUFFER_SIZE: u16 = 0x1005;
    pub const READ_BD_ADDR: u16 = 0x1009;
    pub const READ_DATA_BLOCK_SIZE: u16 = 0x100A;
    pub const READ_LOCAL_SUPPORTED_CODECS: u16 = 0x100B;

    // Status Parameters (OGF = 0x05)
    pub const READ_FAILED_CONTACT_COUNTER: u16 = 0x1401;
    pub const RESET_FAILED_CONTACT_COUNTER: u16 = 0x1402;
    pub const READ_LINK_QUALITY: u16 = 0x1403;
    pub const READ_RSSI: u16 = 0x1405;
    pub const READ_AFH_CHANNEL_MAP: u16 = 0x1406;
    pub const READ_CLOCK: u16 = 0x1407;
    pub const READ_ENCRYPTION_KEY_SIZE: u16 = 0x1408;
    pub const READ_LOCAL_AMP_INFO: u16 = 0x1409;
    pub const READ_LOCAL_AMP_ASSOC: u16 = 0x140A;
    pub const WRITE_REMOTE_AMP_ASSOC: u16 = 0x140B;
    pub const GET_MWS_TRANSPORT_LAYER_CONFIGURATION: u16 = 0x140C;
    pub const SET_TRIGGERED_CLOCK_CAPTURE: u16 = 0x140D;

    // LE Controller Commands (OGF = 0x08)
    pub const LE_SET_EVENT_MASK: u16 = 0x2001;
    pub const LE_READ_BUFFER_SIZE: u16 = 0x2002;
    pub const LE_READ_LOCAL_SUPPORTED_FEATURES: u16 = 0x2003;
    pub const LE_SET_RANDOM_ADDRESS: u16 = 0x2005;
    pub const LE_SET_ADVERTISING_PARAMETERS: u16 = 0x2006;
    pub const LE_READ_ADVERTISING_CHANNEL_TX_POWER: u16 = 0x2007;
    pub const LE_SET_ADVERTISING_DATA: u16 = 0x2008;
    pub const LE_SET_SCAN_RESPONSE_DATA: u16 = 0x2009;
    pub const LE_SET_ADVERTISING_ENABLE: u16 = 0x200A;
    pub const LE_SET_SCAN_PARAMETERS: u16 = 0x200B;
    pub const LE_SET_SCAN_ENABLE: u16 = 0x200C;
    pub const LE_CREATE_CONNECTION: u16 = 0x200D;
    pub const LE_CREATE_CONNECTION_CANCEL: u16 = 0x200E;
    pub const LE_READ_WHITE_LIST_SIZE: u16 = 0x200F;
    pub const LE_CLEAR_WHITE_LIST: u16 = 0x2010;
    pub const LE_ADD_DEVICE_TO_WHITE_LIST: u16 = 0x2011;
    pub const LE_REMOVE_DEVICE_FROM_WHITE_LIST: u16 = 0x2012;
    pub const LE_CONNECTION_UPDATE: u16 = 0x2013;
    pub const LE_SET_HOST_CHANNEL_CLASSIFICATION: u16 = 0x2014;
    pub const LE_READ_CHANNEL_MAP: u16 = 0x2015;
    pub const LE_READ_REMOTE_FEATURES: u16 = 0x2016;
    pub const LE_ENCRYPT: u16 = 0x2017;
    pub const LE_RAND: u16 = 0x2018;
    pub const LE_START_ENCRYPTION: u16 = 0x2019;
    pub const LE_LONG_TERM_KEY_REQUEST_REPLY: u16 = 0x201A;
    pub const LE_LONG_TERM_KEY_REQUEST_NEGATIVE_REPLY: u16 = 0x201B;
    pub const LE_READ_SUPPORTED_STATES: u16 = 0x201C;
    pub const LE_RECEIVER_TEST: u16 = 0x201D;
    pub const LE_TRANSMITTER_TEST: u16 = 0x201E;
    pub const LE_TEST_END: u16 = 0x201F;
    pub const LE_REMOTE_CONNECTION_PARAMETER_REQUEST_REPLY: u16 = 0x2020;
    pub const LE_REMOTE_CONNECTION_PARAMETER_REQUEST_NEGATIVE_REPLY: u16 = 0x2021;
    pub const LE_SET_DATA_LENGTH: u16 = 0x2022;
    pub const LE_READ_SUGGESTED_DEFAULT_DATA_LENGTH: u16 = 0x2023;
    pub const LE_WRITE_SUGGESTED_DEFAULT_DATA_LENGTH: u16 = 0x2024;
    pub const LE_READ_LOCAL_P256_PUBLIC_KEY: u16 = 0x2025;
    pub const LE_GENERATE_DHKEY: u16 = 0x2026;
    pub const LE_ADD_DEVICE_TO_RESOLVING_LIST: u16 = 0x2027;
    pub const LE_REMOVE_DEVICE_FROM_RESOLVING_LIST: u16 = 0x2028;
    pub const LE_CLEAR_RESOLVING_LIST: u16 = 0x2029;
    pub const LE_READ_RESOLVING_LIST_SIZE: u16 = 0x202A;
    pub const LE_READ_PEER_RESOLVABLE_ADDRESS: u16 = 0x202B;
    pub const LE_READ_LOCAL_RESOLVABLE_ADDRESS: u16 = 0x202C;
    pub const LE_SET_ADDRESS_RESOLUTION_ENABLE: u16 = 0x202D;
    pub const LE_SET_RESOLVABLE_PRIVATE_ADDRESS_TIMEOUT: u16 = 0x202E;
    pub const LE_READ_MAXIMUM_DATA_LENGTH: u16 = 0x202F;
    pub const LE_READ_PHY: u16 = 0x2030;
    pub const LE_SET_DEFAULT_PHY: u16 = 0x2031;
    pub const LE_SET_PHY: u16 = 0x2032;
    pub const LE_ENHANCED_RECEIVER_TEST: u16 = 0x2033;
    pub const LE_ENHANCED_TRANSMITTER_TEST: u16 = 0x2034;
    pub const LE_SET_ADVERTISING_SET_RANDOM_ADDRESS: u16 = 0x2035;
    pub const LE_SET_EXTENDED_ADVERTISING_PARAMETERS: u16 = 0x2036;
    pub const LE_SET_EXTENDED_ADVERTISING_DATA: u16 = 0x2037;
    pub const LE_SET_EXTENDED_SCAN_RESPONSE_DATA: u16 = 0x2038;
    pub const LE_SET_EXTENDED_ADVERTISING_ENABLE: u16 = 0x2039;
    pub const LE_READ_MAXIMUM_ADVERTISING_DATA_LENGTH: u16 = 0x203A;
    pub const LE_READ_NUMBER_OF_SUPPORTED_ADVERTISING_SETS: u16 = 0x203B;
    pub const LE_REMOVE_ADVERTISING_SET: u16 = 0x203C;
    pub const LE_CLEAR_ADVERTISING_SETS: u16 = 0x203D;
    pub const LE_SET_PERIODIC_ADVERTISING_PARAMETERS: u16 = 0x203E;
    pub const LE_SET_PERIODIC_ADVERTISING_DATA: u16 = 0x203F;
    pub const LE_SET_PERIODIC_ADVERTISING_ENABLE: u16 = 0x2040;
    pub const LE_SET_EXTENDED_SCAN_PARAMETERS: u16 = 0x2041;
    pub const LE_SET_EXTENDED_SCAN_ENABLE: u16 = 0x2042;
    pub const LE_EXTENDED_CREATE_CONNECTION: u16 = 0x2043;
    pub const LE_PERIODIC_ADVERTISING_CREATE_SYNC: u16 = 0x2044;
    pub const LE_PERIODIC_ADVERTISING_CREATE_SYNC_CANCEL: u16 = 0x2045;
    pub const LE_PERIODIC_ADVERTISING_TERMINATE_SYNC: u16 = 0x2046;
    pub const LE_ADD_DEVICE_TO_PERIODIC_ADVERTISER_LIST: u16 = 0x2047;
    pub const LE_REMOVE_DEVICE_FROM_PERIODIC_ADVERTISER_LIST: u16 = 0x2048;
    pub const LE_CLEAR_PERIODIC_ADVERTISER_LIST: u16 = 0x2049;
    pub const LE_READ_PERIODIC_ADVERTISER_LIST_SIZE: u16 = 0x204A;
    pub const LE_READ_TRANSMIT_POWER: u16 = 0x204B;
    pub const LE_READ_RF_PATH_COMPENSATION: u16 = 0x204C;
    pub const LE_WRITE_RF_PATH_COMPENSATION: u16 = 0x204D;
    pub const LE_SET_PRIVACY_MODE: u16 = 0x204E;

    // Command builders

    /// Build Reset command
    pub fn reset() -> Vec<u8> {
        build_command(RESET, &[])
    }

    /// Build Inquiry command
    pub fn inquiry(lap: u32, length: u8, num_responses: u8) -> Vec<u8> {
        let params = [
            (lap & 0xFF) as u8,
            ((lap >> 8) & 0xFF) as u8,
            ((lap >> 16) & 0xFF) as u8,
            length,
            num_responses,
        ];
        build_command(INQUIRY, &params)
    }

    /// Build Inquiry Cancel command
    pub fn inquiry_cancel() -> Vec<u8> {
        build_command(INQUIRY_CANCEL, &[])
    }

    /// Build Create Connection command
    pub fn create_connection(
        address: &BdAddr,
        packet_type: u16,
        page_scan_rep_mode: u8,
        reserved: u8,
        clock_offset: u16,
        allow_role_switch: u8,
    ) -> Vec<u8> {
        let mut params = Vec::with_capacity(13);
        params.extend_from_slice(&address.0);
        params.push((packet_type & 0xFF) as u8);
        params.push((packet_type >> 8) as u8);
        params.push(page_scan_rep_mode);
        params.push(reserved);
        params.push((clock_offset & 0xFF) as u8);
        params.push((clock_offset >> 8) as u8);
        params.push(allow_role_switch);
        build_command(CREATE_CONNECTION, &params)
    }

    /// Build Disconnect command
    pub fn disconnect(handle: u16, reason: u8) -> Vec<u8> {
        let params = [
            (handle & 0xFF) as u8,
            ((handle >> 8) & 0x0F) as u8,
            reason,
        ];
        build_command(DISCONNECT, &params)
    }

    /// Build Read Local Version command
    pub fn read_local_version() -> Vec<u8> {
        build_command(READ_LOCAL_VERSION, &[])
    }

    /// Build Read BD_ADDR command
    pub fn read_bd_addr() -> Vec<u8> {
        build_command(READ_BD_ADDR, &[])
    }

    /// Build Read Buffer Size command
    pub fn read_buffer_size() -> Vec<u8> {
        build_command(READ_BUFFER_SIZE, &[])
    }

    /// Build Read Local Supported Features command
    pub fn read_local_features() -> Vec<u8> {
        build_command(READ_LOCAL_FEATURES, &[])
    }

    /// Build Remote Name Request command
    pub fn remote_name_request(
        address: &BdAddr,
        page_scan_rep_mode: u8,
        reserved: u8,
        clock_offset: u16,
    ) -> Vec<u8> {
        let mut params = Vec::with_capacity(10);
        params.extend_from_slice(&address.0);
        params.push(page_scan_rep_mode);
        params.push(reserved);
        params.push((clock_offset & 0xFF) as u8);
        params.push((clock_offset >> 8) as u8);
        build_command(REMOTE_NAME_REQUEST, &params)
    }

    /// Build Write Scan Enable command
    pub fn write_scan_enable(scan_enable: u8) -> Vec<u8> {
        build_command(WRITE_SCAN_ENABLE, &[scan_enable])
    }

    /// Build Write Class Of Device command
    pub fn write_class_of_device(class: &[u8; 3]) -> Vec<u8> {
        build_command(WRITE_CLASS_OF_DEVICE, class)
    }

    /// Build Write Local Name command
    pub fn write_local_name(name: &str) -> Vec<u8> {
        let mut params = [0u8; 248];
        let len = name.len().min(247);
        params[..len].copy_from_slice(name.as_bytes());
        build_command(WRITE_LOCAL_NAME, &params)
    }

    /// Build Write Simple Pairing Mode command
    pub fn write_simple_pairing_mode(enable: bool) -> Vec<u8> {
        build_command(WRITE_SIMPLE_PAIRING_MODE, &[if enable { 1 } else { 0 }])
    }

    /// Build Write Inquiry Mode command
    pub fn write_inquiry_mode(mode: u8) -> Vec<u8> {
        build_command(WRITE_INQUIRY_MODE, &[mode])
    }

    /// Build Set Event Mask command
    pub fn set_event_mask(mask: u64) -> Vec<u8> {
        let params = mask.to_le_bytes();
        build_command(SET_EVENT_MASK, &params)
    }

    /// Build Accept Connection Request command
    pub fn accept_connection_request(address: &BdAddr, role: u8) -> Vec<u8> {
        let mut params = Vec::with_capacity(7);
        params.extend_from_slice(&address.0);
        params.push(role);
        build_command(ACCEPT_CONNECTION_REQUEST, &params)
    }

    /// Build Reject Connection Request command
    pub fn reject_connection_request(address: &BdAddr, reason: u8) -> Vec<u8> {
        let mut params = Vec::with_capacity(7);
        params.extend_from_slice(&address.0);
        params.push(reason);
        build_command(REJECT_CONNECTION_REQUEST, &params)
    }

    /// Build Link Key Request Reply command
    pub fn link_key_request_reply(address: &BdAddr, link_key: &[u8; 16]) -> Vec<u8> {
        let mut params = Vec::with_capacity(22);
        params.extend_from_slice(&address.0);
        params.extend_from_slice(link_key);
        build_command(LINK_KEY_REQUEST_REPLY, &params)
    }

    /// Build Link Key Request Negative Reply command
    pub fn link_key_request_negative_reply(address: &BdAddr) -> Vec<u8> {
        build_command(LINK_KEY_REQUEST_NEGATIVE_REPLY, &address.0)
    }

    /// Build PIN Code Request Reply command
    pub fn pin_code_request_reply(address: &BdAddr, pin_code: &[u8], pin_length: u8) -> Vec<u8> {
        let mut params = Vec::with_capacity(23);
        params.extend_from_slice(&address.0);
        params.push(pin_length);
        let mut pin = [0u8; 16];
        pin[..pin_code.len().min(16)].copy_from_slice(pin_code);
        params.extend_from_slice(&pin);
        build_command(PIN_CODE_REQUEST_REPLY, &params)
    }

    /// Build IO Capability Request Reply command
    pub fn io_capability_request_reply(
        address: &BdAddr,
        io_capability: u8,
        oob_data_present: u8,
        authentication_requirements: u8,
    ) -> Vec<u8> {
        let mut params = Vec::with_capacity(9);
        params.extend_from_slice(&address.0);
        params.push(io_capability);
        params.push(oob_data_present);
        params.push(authentication_requirements);
        build_command(IO_CAPABILITY_REQUEST_REPLY, &params)
    }

    /// Build User Confirmation Request Reply command
    pub fn user_confirmation_request_reply(address: &BdAddr) -> Vec<u8> {
        build_command(USER_CONFIRMATION_REQUEST_REPLY, &address.0)
    }

    /// Build User Passkey Request Reply command
    pub fn user_passkey_request_reply(address: &BdAddr, passkey: u32) -> Vec<u8> {
        let mut params = Vec::with_capacity(10);
        params.extend_from_slice(&address.0);
        params.extend_from_slice(&passkey.to_le_bytes());
        build_command(USER_PASSKEY_REQUEST_REPLY, &params)
    }

    // LE Commands

    /// Build LE Set Event Mask command
    pub fn le_set_event_mask(mask: u64) -> Vec<u8> {
        let params = mask.to_le_bytes();
        build_command(LE_SET_EVENT_MASK, &params)
    }

    /// Build LE Set Scan Parameters command
    pub fn le_set_scan_parameters(
        scan_type: u8,
        scan_interval: u16,
        scan_window: u16,
        own_address_type: u8,
        scanning_filter_policy: u8,
    ) -> Vec<u8> {
        let params = [
            scan_type,
            (scan_interval & 0xFF) as u8,
            (scan_interval >> 8) as u8,
            (scan_window & 0xFF) as u8,
            (scan_window >> 8) as u8,
            own_address_type,
            scanning_filter_policy,
        ];
        build_command(LE_SET_SCAN_PARAMETERS, &params)
    }

    /// Build LE Set Scan Enable command
    pub fn le_set_scan_enable(enable: bool, filter_duplicates: bool) -> Vec<u8> {
        let params = [
            if enable { 1 } else { 0 },
            if filter_duplicates { 1 } else { 0 },
        ];
        build_command(LE_SET_SCAN_ENABLE, &params)
    }

    /// Build LE Set Advertising Parameters command
    pub fn le_set_advertising_parameters(
        adv_interval_min: u16,
        adv_interval_max: u16,
        adv_type: u8,
        own_address_type: u8,
        peer_address_type: u8,
        peer_address: &BdAddr,
        channel_map: u8,
        filter_policy: u8,
    ) -> Vec<u8> {
        let mut params = Vec::with_capacity(15);
        params.push((adv_interval_min & 0xFF) as u8);
        params.push((adv_interval_min >> 8) as u8);
        params.push((adv_interval_max & 0xFF) as u8);
        params.push((adv_interval_max >> 8) as u8);
        params.push(adv_type);
        params.push(own_address_type);
        params.push(peer_address_type);
        params.extend_from_slice(&peer_address.0);
        params.push(channel_map);
        params.push(filter_policy);
        build_command(LE_SET_ADVERTISING_PARAMETERS, &params)
    }

    /// Build LE Set Advertising Data command
    pub fn le_set_advertising_data(data: &[u8]) -> Vec<u8> {
        let mut params = [0u8; 32];
        params[0] = data.len().min(31) as u8;
        params[1..1 + data.len().min(31)].copy_from_slice(&data[..data.len().min(31)]);
        build_command(LE_SET_ADVERTISING_DATA, &params)
    }

    /// Build LE Set Advertising Enable command
    pub fn le_set_advertising_enable(enable: bool) -> Vec<u8> {
        build_command(LE_SET_ADVERTISING_ENABLE, &[if enable { 1 } else { 0 }])
    }

    /// Helper function to build HCI command packet
    fn build_command(opcode: u16, params: &[u8]) -> Vec<u8> {
        let mut cmd = Vec::with_capacity(4 + params.len());
        cmd.push(super::packet_types::COMMAND);
        cmd.push((opcode & 0xFF) as u8);
        cmd.push((opcode >> 8) as u8);
        cmd.push(params.len() as u8);
        cmd.extend_from_slice(params);
        cmd
    }
}

/// HCI ACL data packet header
#[derive(Debug, Clone, Copy)]
pub struct AclHeader {
    pub handle: u16,
    pub pb_flag: u8,
    pub bc_flag: u8,
    pub length: u16,
}

impl AclHeader {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }

        let handle_flags = u16::from_le_bytes([bytes[0], bytes[1]]);
        let handle = handle_flags & 0x0FFF;
        let pb_flag = ((handle_flags >> 12) & 0x03) as u8;
        let bc_flag = ((handle_flags >> 14) & 0x03) as u8;
        let length = u16::from_le_bytes([bytes[2], bytes[3]]);

        Some(Self {
            handle,
            pb_flag,
            bc_flag,
            length,
        })
    }

    pub fn to_bytes(&self) -> [u8; 4] {
        let handle_flags = self.handle | ((self.pb_flag as u16) << 12) | ((self.bc_flag as u16) << 14);
        [
            (handle_flags & 0xFF) as u8,
            (handle_flags >> 8) as u8,
            (self.length & 0xFF) as u8,
            (self.length >> 8) as u8,
        ]
    }
}

/// Build ACL data packet
pub fn build_acl_packet(handle: u16, pb_flag: u8, bc_flag: u8, data: &[u8]) -> Vec<u8> {
    let header = AclHeader {
        handle,
        pb_flag,
        bc_flag,
        length: data.len() as u16,
    };

    let mut packet = Vec::with_capacity(5 + data.len());
    packet.push(packet_types::ACL_DATA);
    packet.extend_from_slice(&header.to_bytes());
    packet.extend_from_slice(data);
    packet
}
