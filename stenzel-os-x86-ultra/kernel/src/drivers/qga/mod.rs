//! QEMU Guest Agent
//!
//! Provides guest services for QEMU/KVM virtual machines via virtio-serial.

#![allow(dead_code)]

pub mod commands;
pub mod fsfreeze;
pub mod info;
pub mod exec;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;

/// QGA protocol version
pub const QGA_VERSION: &str = "2.12.0";

/// QGA sync ID for request/response matching
pub const QGA_SYNC_VALUE: u64 = 0x12345678ABCDEF00;

/// Maximum command response size
const MAX_RESPONSE_SIZE: usize = 65536;

/// QGA command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QgaCommand {
    GuestSync,
    GuestSyncDelimited,
    GuestPing,
    GuestGetTime,
    GuestSetTime,
    GuestInfo,
    GuestShutdown,
    GuestFileOpen,
    GuestFileClose,
    GuestFileRead,
    GuestFileWrite,
    GuestFileSeek,
    GuestFileFlush,
    GuestFsfreezeStatus,
    GuestFsfreezeFreeze,
    GuestFsfreezeThaw,
    GuestSuspendDisk,
    GuestSuspendRam,
    GuestSuspendHybrid,
    GuestNetworkGetInterfaces,
    GuestGetVcpus,
    GuestSetVcpus,
    GuestGetFsinfo,
    GuestGetMemoryBlocks,
    GuestSetMemoryBlocks,
    GuestGetMemoryBlockInfo,
    GuestExec,
    GuestExecStatus,
    GuestGetOsinfo,
    GuestGetTimezone,
    GuestGetUsers,
    GuestGetHost,
    GuestGetDisks,
    GuestGetDevices,
    Unknown,
}

impl QgaCommand {
    pub fn from_str(s: &str) -> Self {
        match s {
            "guest-sync" => Self::GuestSync,
            "guest-sync-delimited" => Self::GuestSyncDelimited,
            "guest-ping" => Self::GuestPing,
            "guest-get-time" => Self::GuestGetTime,
            "guest-set-time" => Self::GuestSetTime,
            "guest-info" => Self::GuestInfo,
            "guest-shutdown" => Self::GuestShutdown,
            "guest-file-open" => Self::GuestFileOpen,
            "guest-file-close" => Self::GuestFileClose,
            "guest-file-read" => Self::GuestFileRead,
            "guest-file-write" => Self::GuestFileWrite,
            "guest-file-seek" => Self::GuestFileSeek,
            "guest-file-flush" => Self::GuestFileFlush,
            "guest-fsfreeze-status" => Self::GuestFsfreezeStatus,
            "guest-fsfreeze-freeze" => Self::GuestFsfreezeFreeze,
            "guest-fsfreeze-thaw" => Self::GuestFsfreezeThaw,
            "guest-suspend-disk" => Self::GuestSuspendDisk,
            "guest-suspend-ram" => Self::GuestSuspendRam,
            "guest-suspend-hybrid" => Self::GuestSuspendHybrid,
            "guest-network-get-interfaces" => Self::GuestNetworkGetInterfaces,
            "guest-get-vcpus" => Self::GuestGetVcpus,
            "guest-set-vcpus" => Self::GuestSetVcpus,
            "guest-get-fsinfo" => Self::GuestGetFsinfo,
            "guest-get-memory-blocks" => Self::GuestGetMemoryBlocks,
            "guest-set-memory-blocks" => Self::GuestSetMemoryBlocks,
            "guest-get-memory-block-info" => Self::GuestGetMemoryBlockInfo,
            "guest-exec" => Self::GuestExec,
            "guest-exec-status" => Self::GuestExecStatus,
            "guest-get-osinfo" => Self::GuestGetOsinfo,
            "guest-get-timezone" => Self::GuestGetTimezone,
            "guest-get-users" => Self::GuestGetUsers,
            "guest-get-host-name" => Self::GuestGetHost,
            "guest-get-disks" => Self::GuestGetDisks,
            "guest-get-devices" => Self::GuestGetDevices,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GuestSync => "guest-sync",
            Self::GuestSyncDelimited => "guest-sync-delimited",
            Self::GuestPing => "guest-ping",
            Self::GuestGetTime => "guest-get-time",
            Self::GuestSetTime => "guest-set-time",
            Self::GuestInfo => "guest-info",
            Self::GuestShutdown => "guest-shutdown",
            Self::GuestFileOpen => "guest-file-open",
            Self::GuestFileClose => "guest-file-close",
            Self::GuestFileRead => "guest-file-read",
            Self::GuestFileWrite => "guest-file-write",
            Self::GuestFileSeek => "guest-file-seek",
            Self::GuestFileFlush => "guest-file-flush",
            Self::GuestFsfreezeStatus => "guest-fsfreeze-status",
            Self::GuestFsfreezeFreeze => "guest-fsfreeze-freeze",
            Self::GuestFsfreezeThaw => "guest-fsfreeze-thaw",
            Self::GuestSuspendDisk => "guest-suspend-disk",
            Self::GuestSuspendRam => "guest-suspend-ram",
            Self::GuestSuspendHybrid => "guest-suspend-hybrid",
            Self::GuestNetworkGetInterfaces => "guest-network-get-interfaces",
            Self::GuestGetVcpus => "guest-get-vcpus",
            Self::GuestSetVcpus => "guest-set-vcpus",
            Self::GuestGetFsinfo => "guest-get-fsinfo",
            Self::GuestGetMemoryBlocks => "guest-get-memory-blocks",
            Self::GuestSetMemoryBlocks => "guest-set-memory-blocks",
            Self::GuestGetMemoryBlockInfo => "guest-get-memory-block-info",
            Self::GuestExec => "guest-exec",
            Self::GuestExecStatus => "guest-exec-status",
            Self::GuestGetOsinfo => "guest-get-osinfo",
            Self::GuestGetTimezone => "guest-get-timezone",
            Self::GuestGetUsers => "guest-get-users",
            Self::GuestGetHost => "guest-get-host-name",
            Self::GuestGetDisks => "guest-get-disks",
            Self::GuestGetDevices => "guest-get-devices",
            Self::Unknown => "unknown",
        }
    }
}

/// QGA error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QgaError {
    CommandNotFound,
    InvalidParameter,
    InvalidHandle,
    OperationFailed,
    PermissionDenied,
    NotSupported,
    Busy,
    IoError,
    ProtocolError,
}

impl QgaError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CommandNotFound => "CommandNotFound",
            Self::InvalidParameter => "InvalidParameter",
            Self::InvalidHandle => "InvalidHandle",
            Self::OperationFailed => "OperationFailed",
            Self::PermissionDenied => "PermissionDenied",
            Self::NotSupported => "NotSupported",
            Self::Busy => "Busy",
            Self::IoError => "IoError",
            Self::ProtocolError => "ProtocolError",
        }
    }
}

/// QGA request from host
#[derive(Debug, Clone)]
pub struct QgaRequest {
    pub execute: String,
    pub arguments: BTreeMap<String, JsonValue>,
}

/// QGA response to host
#[derive(Debug, Clone)]
pub struct QgaResponse {
    pub return_value: Option<JsonValue>,
    pub error: Option<QgaErrorResponse>,
}

/// QGA error response
#[derive(Debug, Clone)]
pub struct QgaErrorResponse {
    pub class: String,
    pub desc: String,
}

/// Simple JSON value representation
#[derive(Debug, Clone)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(i64),
    Float(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    pub fn to_json_string(&self) -> String {
        match self {
            JsonValue::Null => "null".to_string(),
            JsonValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            JsonValue::Number(n) => alloc::format!("{}", n),
            JsonValue::Float(f) => alloc::format!("{}", f),
            JsonValue::String(s) => alloc::format!("\"{}\"", escape_json_string(s)),
            JsonValue::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|v| v.to_json_string()).collect();
                alloc::format!("[{}]", items.join(","))
            }
            JsonValue::Object(obj) => {
                let items: Vec<String> = obj.iter()
                    .map(|(k, v)| alloc::format!("\"{}\":{}", escape_json_string(k), v.to_json_string()))
                    .collect();
                alloc::format!("{{{}}}", items.join(","))
            }
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match self {
            JsonValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            JsonValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsonValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

fn escape_json_string(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                result.push_str(&alloc::format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

/// QGA statistics
#[derive(Debug, Default)]
pub struct QgaStats {
    pub commands_received: AtomicU64,
    pub commands_processed: AtomicU64,
    pub commands_failed: AtomicU64,
    pub bytes_received: AtomicU64,
    pub bytes_sent: AtomicU64,
}

/// QGA channel state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QgaState {
    Disconnected,
    Connected,
    Syncing,
    Ready,
    Frozen,
}

/// Open file handle
#[derive(Debug)]
pub struct QgaFileHandle {
    pub handle: i64,
    pub path: String,
    pub mode: String,
    pub position: u64,
}

/// QEMU Guest Agent
pub struct QemuGuestAgent {
    /// Current state
    state: QgaState,
    /// Sync ID received from host
    sync_id: u64,
    /// VirtIO serial port
    virtio_port: Option<u64>,
    /// Receive buffer
    rx_buffer: Vec<u8>,
    /// Transmit buffer
    tx_buffer: Vec<u8>,
    /// Open file handles
    file_handles: BTreeMap<i64, QgaFileHandle>,
    /// Next file handle ID
    next_handle_id: i64,
    /// Filesystem frozen
    fs_frozen: bool,
    /// Frozen filesystem count
    frozen_fs_count: u32,
    /// Supported commands
    supported_commands: Vec<QgaCommand>,
    /// Blacklisted commands
    blacklisted_commands: Vec<QgaCommand>,
    /// Initialized flag
    initialized: AtomicBool,
    /// Statistics
    stats: QgaStats,
}

impl QemuGuestAgent {
    /// Create new guest agent
    pub fn new() -> Self {
        Self {
            state: QgaState::Disconnected,
            sync_id: 0,
            virtio_port: None,
            rx_buffer: Vec::with_capacity(MAX_RESPONSE_SIZE),
            tx_buffer: Vec::with_capacity(MAX_RESPONSE_SIZE),
            file_handles: BTreeMap::new(),
            next_handle_id: 1,
            fs_frozen: false,
            frozen_fs_count: 0,
            supported_commands: Self::default_supported_commands(),
            blacklisted_commands: Vec::new(),
            initialized: AtomicBool::new(false),
            stats: QgaStats::default(),
        }
    }

    /// Default supported commands
    fn default_supported_commands() -> Vec<QgaCommand> {
        vec![
            QgaCommand::GuestSync,
            QgaCommand::GuestSyncDelimited,
            QgaCommand::GuestPing,
            QgaCommand::GuestGetTime,
            QgaCommand::GuestSetTime,
            QgaCommand::GuestInfo,
            QgaCommand::GuestShutdown,
            QgaCommand::GuestFileOpen,
            QgaCommand::GuestFileClose,
            QgaCommand::GuestFileRead,
            QgaCommand::GuestFileWrite,
            QgaCommand::GuestFileSeek,
            QgaCommand::GuestFileFlush,
            QgaCommand::GuestFsfreezeStatus,
            QgaCommand::GuestFsfreezeFreeze,
            QgaCommand::GuestFsfreezeThaw,
            QgaCommand::GuestNetworkGetInterfaces,
            QgaCommand::GuestGetVcpus,
            QgaCommand::GuestGetFsinfo,
            QgaCommand::GuestExec,
            QgaCommand::GuestExecStatus,
            QgaCommand::GuestGetOsinfo,
            QgaCommand::GuestGetTimezone,
            QgaCommand::GuestGetHost,
            QgaCommand::GuestGetDisks,
        ]
    }

    /// Initialize with VirtIO serial port
    pub fn init(&mut self, virtio_port: u64) -> Result<(), &'static str> {
        self.virtio_port = Some(virtio_port);
        self.state = QgaState::Connected;
        self.initialized.store(true, Ordering::Release);

        crate::kprintln!("qga: QEMU Guest Agent v{} initialized", QGA_VERSION);
        Ok(())
    }

    /// Get current state
    pub fn state(&self) -> QgaState {
        self.state
    }

    /// Is command supported?
    pub fn is_command_supported(&self, cmd: QgaCommand) -> bool {
        self.supported_commands.contains(&cmd) && !self.blacklisted_commands.contains(&cmd)
    }

    /// Blacklist a command
    pub fn blacklist_command(&mut self, cmd: QgaCommand) {
        if !self.blacklisted_commands.contains(&cmd) {
            self.blacklisted_commands.push(cmd);
        }
    }

    /// Process received data
    pub fn process_data(&mut self, data: &[u8]) -> Option<Vec<u8>> {
        self.stats.bytes_received.fetch_add(data.len() as u64, Ordering::Relaxed);
        self.rx_buffer.extend_from_slice(data);

        // Look for newline-delimited JSON
        if let Some(pos) = self.rx_buffer.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.rx_buffer.drain(..=pos).collect();
            let line_str = core::str::from_utf8(&line[..line.len()-1]).ok()?;

            self.stats.commands_received.fetch_add(1, Ordering::Relaxed);

            let response = self.handle_request(line_str);
            let response_bytes = response.into_bytes();

            self.stats.bytes_sent.fetch_add(response_bytes.len() as u64, Ordering::Relaxed);
            return Some(response_bytes);
        }

        None
    }

    /// Handle a JSON request
    fn handle_request(&mut self, request: &str) -> String {
        // Simple JSON parsing for QGA protocol
        let request = self.parse_request(request);

        match request {
            Ok(req) => {
                let result = self.execute_command(&req);
                self.format_response(result)
            }
            Err(e) => {
                self.stats.commands_failed.fetch_add(1, Ordering::Relaxed);
                self.format_error(e)
            }
        }
    }

    /// Parse JSON request (simplified)
    fn parse_request(&self, json: &str) -> Result<QgaRequest, QgaError> {
        // Very simplified JSON parsing
        // In a real implementation, use a proper JSON parser

        // Look for "execute" field
        let execute = self.extract_string_field(json, "execute")
            .ok_or(QgaError::ProtocolError)?;

        // Extract arguments object (if present)
        let arguments = self.extract_arguments(json);

        Ok(QgaRequest { execute, arguments })
    }

    /// Extract string field from JSON
    fn extract_string_field(&self, json: &str, field: &str) -> Option<String> {
        let pattern = alloc::format!("\"{}\"", field);
        let start = json.find(&pattern)?;
        let rest = &json[start + pattern.len()..];

        // Skip whitespace and colon
        let rest = rest.trim_start();
        let rest = rest.strip_prefix(':')?;
        let rest = rest.trim_start();

        // Extract string value
        if rest.starts_with('"') {
            let end = rest[1..].find('"')?;
            Some(rest[1..end+1].to_string())
        } else {
            None
        }
    }

    /// Extract arguments from JSON
    fn extract_arguments(&self, json: &str) -> BTreeMap<String, JsonValue> {
        let mut args = BTreeMap::new();

        // Look for "arguments" field
        if let Some(start) = json.find("\"arguments\"") {
            let rest = &json[start + 11..];
            if let Some(obj_start) = rest.find('{') {
                // Very simplified - just extract simple string/number arguments
                let obj = &rest[obj_start..];
                if let Some(obj_end) = obj.find('}') {
                    let obj_content = &obj[1..obj_end];

                    // Parse simple key-value pairs
                    for part in obj_content.split(',') {
                        if let Some(colon) = part.find(':') {
                            let key = part[..colon].trim().trim_matches('"');
                            let value = part[colon+1..].trim();

                            if value.starts_with('"') && value.ends_with('"') {
                                args.insert(
                                    key.to_string(),
                                    JsonValue::String(value[1..value.len()-1].to_string())
                                );
                            } else if let Ok(n) = value.parse::<i64>() {
                                args.insert(key.to_string(), JsonValue::Number(n));
                            } else if value == "true" {
                                args.insert(key.to_string(), JsonValue::Bool(true));
                            } else if value == "false" {
                                args.insert(key.to_string(), JsonValue::Bool(false));
                            }
                        }
                    }
                }
            }
        }

        args
    }

    /// Execute a command
    fn execute_command(&mut self, request: &QgaRequest) -> Result<JsonValue, QgaError> {
        let cmd = QgaCommand::from_str(&request.execute);

        if !self.is_command_supported(cmd) {
            return Err(QgaError::CommandNotFound);
        }

        // Check if filesystem is frozen (most commands blocked)
        if self.fs_frozen {
            match cmd {
                QgaCommand::GuestFsfreezeStatus |
                QgaCommand::GuestFsfreezeThaw |
                QgaCommand::GuestPing |
                QgaCommand::GuestSync |
                QgaCommand::GuestSyncDelimited => {}
                _ => return Err(QgaError::Busy),
            }
        }

        match cmd {
            QgaCommand::GuestSync | QgaCommand::GuestSyncDelimited => {
                self.handle_sync(&request.arguments)
            }
            QgaCommand::GuestPing => {
                Ok(JsonValue::Object(BTreeMap::new()))
            }
            QgaCommand::GuestGetTime => {
                self.handle_get_time()
            }
            QgaCommand::GuestSetTime => {
                self.handle_set_time(&request.arguments)
            }
            QgaCommand::GuestInfo => {
                self.handle_guest_info()
            }
            QgaCommand::GuestShutdown => {
                self.handle_shutdown(&request.arguments)
            }
            QgaCommand::GuestFileOpen => {
                self.handle_file_open(&request.arguments)
            }
            QgaCommand::GuestFileClose => {
                self.handle_file_close(&request.arguments)
            }
            QgaCommand::GuestFileRead => {
                self.handle_file_read(&request.arguments)
            }
            QgaCommand::GuestFileWrite => {
                self.handle_file_write(&request.arguments)
            }
            QgaCommand::GuestFileSeek => {
                self.handle_file_seek(&request.arguments)
            }
            QgaCommand::GuestFileFlush => {
                self.handle_file_flush(&request.arguments)
            }
            QgaCommand::GuestFsfreezeStatus => {
                self.handle_fsfreeze_status()
            }
            QgaCommand::GuestFsfreezeFreeze => {
                self.handle_fsfreeze_freeze()
            }
            QgaCommand::GuestFsfreezeThaw => {
                self.handle_fsfreeze_thaw()
            }
            QgaCommand::GuestNetworkGetInterfaces => {
                self.handle_network_get_interfaces()
            }
            QgaCommand::GuestGetVcpus => {
                self.handle_get_vcpus()
            }
            QgaCommand::GuestGetFsinfo => {
                self.handle_get_fsinfo()
            }
            QgaCommand::GuestExec => {
                self.handle_exec(&request.arguments)
            }
            QgaCommand::GuestExecStatus => {
                self.handle_exec_status(&request.arguments)
            }
            QgaCommand::GuestGetOsinfo => {
                self.handle_get_osinfo()
            }
            QgaCommand::GuestGetTimezone => {
                self.handle_get_timezone()
            }
            QgaCommand::GuestGetHost => {
                self.handle_get_hostname()
            }
            QgaCommand::GuestGetDisks => {
                self.handle_get_disks()
            }
            _ => Err(QgaError::NotSupported),
        }
    }

    /// Handle guest-sync command
    fn handle_sync(&mut self, args: &BTreeMap<String, JsonValue>) -> Result<JsonValue, QgaError> {
        let id = args.get("id")
            .and_then(|v| v.as_i64())
            .ok_or(QgaError::InvalidParameter)?;

        self.sync_id = id as u64;
        self.state = QgaState::Ready;

        Ok(JsonValue::Number(id))
    }

    /// Handle guest-get-time
    fn handle_get_time(&self) -> Result<JsonValue, QgaError> {
        // Return nanoseconds since epoch
        let ticks = crate::time::ticks();
        // Assume 1000 ticks per second, convert to nanoseconds
        let nanos = ticks * 1_000_000;
        Ok(JsonValue::Number(nanos as i64))
    }

    /// Handle guest-set-time
    fn handle_set_time(&mut self, args: &BTreeMap<String, JsonValue>) -> Result<JsonValue, QgaError> {
        let _time = args.get("time")
            .and_then(|v| v.as_i64())
            .ok_or(QgaError::InvalidParameter)?;

        // In a real implementation, would set system time
        Ok(JsonValue::Object(BTreeMap::new()))
    }

    /// Handle guest-info
    fn handle_guest_info(&self) -> Result<JsonValue, QgaError> {
        let mut info = BTreeMap::new();
        info.insert("version".to_string(), JsonValue::String(QGA_VERSION.to_string()));

        // Build supported commands list
        let commands: Vec<JsonValue> = self.supported_commands.iter()
            .filter(|cmd| !self.blacklisted_commands.contains(cmd))
            .map(|cmd| {
                let mut cmd_info = BTreeMap::new();
                cmd_info.insert("name".to_string(), JsonValue::String(cmd.as_str().to_string()));
                cmd_info.insert("enabled".to_string(), JsonValue::Bool(true));
                cmd_info.insert("success-response".to_string(), JsonValue::Bool(true));
                JsonValue::Object(cmd_info)
            })
            .collect();

        info.insert("supported_commands".to_string(), JsonValue::Array(commands));

        Ok(JsonValue::Object(info))
    }

    /// Handle guest-shutdown
    fn handle_shutdown(&mut self, args: &BTreeMap<String, JsonValue>) -> Result<JsonValue, QgaError> {
        let mode = args.get("mode")
            .and_then(|v| v.as_string())
            .unwrap_or("powerdown");

        match mode {
            "powerdown" => {
                crate::kprintln!("qga: Shutdown requested by host");
                // In real implementation, initiate shutdown
            }
            "halt" => {
                crate::kprintln!("qga: Halt requested by host");
            }
            "reboot" => {
                crate::kprintln!("qga: Reboot requested by host");
            }
            _ => return Err(QgaError::InvalidParameter),
        }

        // This command doesn't return (or returns empty on async)
        Ok(JsonValue::Object(BTreeMap::new()))
    }

    /// Handle guest-file-open
    fn handle_file_open(&mut self, args: &BTreeMap<String, JsonValue>) -> Result<JsonValue, QgaError> {
        let path = args.get("path")
            .and_then(|v| v.as_string())
            .ok_or(QgaError::InvalidParameter)?
            .to_string();

        let mode = args.get("mode")
            .and_then(|v| v.as_string())
            .unwrap_or("r")
            .to_string();

        let handle_id = self.next_handle_id;
        self.next_handle_id += 1;

        let handle = QgaFileHandle {
            handle: handle_id,
            path,
            mode,
            position: 0,
        };

        self.file_handles.insert(handle_id, handle);

        Ok(JsonValue::Number(handle_id))
    }

    /// Handle guest-file-close
    fn handle_file_close(&mut self, args: &BTreeMap<String, JsonValue>) -> Result<JsonValue, QgaError> {
        let handle = args.get("handle")
            .and_then(|v| v.as_i64())
            .ok_or(QgaError::InvalidParameter)?;

        if self.file_handles.remove(&handle).is_none() {
            return Err(QgaError::InvalidHandle);
        }

        Ok(JsonValue::Object(BTreeMap::new()))
    }

    /// Handle guest-file-read
    fn handle_file_read(&mut self, args: &BTreeMap<String, JsonValue>) -> Result<JsonValue, QgaError> {
        let handle = args.get("handle")
            .and_then(|v| v.as_i64())
            .ok_or(QgaError::InvalidParameter)?;

        let count = args.get("count")
            .and_then(|v| v.as_i64())
            .unwrap_or(4096) as usize;

        let _file = self.file_handles.get_mut(&handle)
            .ok_or(QgaError::InvalidHandle)?;

        // In real implementation, would read from file
        // Return base64-encoded data
        let mut result = BTreeMap::new();
        result.insert("count".to_string(), JsonValue::Number(0));
        result.insert("buf-b64".to_string(), JsonValue::String(String::new()));
        result.insert("eof".to_string(), JsonValue::Bool(count == 0));

        Ok(JsonValue::Object(result))
    }

    /// Handle guest-file-write
    fn handle_file_write(&mut self, args: &BTreeMap<String, JsonValue>) -> Result<JsonValue, QgaError> {
        let handle = args.get("handle")
            .and_then(|v| v.as_i64())
            .ok_or(QgaError::InvalidParameter)?;

        let _buf_b64 = args.get("buf-b64")
            .and_then(|v| v.as_string())
            .ok_or(QgaError::InvalidParameter)?;

        let _file = self.file_handles.get_mut(&handle)
            .ok_or(QgaError::InvalidHandle)?;

        // In real implementation, would decode base64 and write to file
        let mut result = BTreeMap::new();
        result.insert("count".to_string(), JsonValue::Number(0));
        result.insert("eof".to_string(), JsonValue::Bool(false));

        Ok(JsonValue::Object(result))
    }

    /// Handle guest-file-seek
    fn handle_file_seek(&mut self, args: &BTreeMap<String, JsonValue>) -> Result<JsonValue, QgaError> {
        let handle = args.get("handle")
            .and_then(|v| v.as_i64())
            .ok_or(QgaError::InvalidParameter)?;

        let offset = args.get("offset")
            .and_then(|v| v.as_i64())
            .ok_or(QgaError::InvalidParameter)?;

        let whence = args.get("whence")
            .and_then(|v| v.as_i64())
            .unwrap_or(0); // SEEK_SET

        let file = self.file_handles.get_mut(&handle)
            .ok_or(QgaError::InvalidHandle)?;

        match whence {
            0 => file.position = offset as u64, // SEEK_SET
            1 => file.position = (file.position as i64 + offset) as u64, // SEEK_CUR
            2 => return Err(QgaError::NotSupported), // SEEK_END needs file size
            _ => return Err(QgaError::InvalidParameter),
        }

        let mut result = BTreeMap::new();
        result.insert("position".to_string(), JsonValue::Number(file.position as i64));

        Ok(JsonValue::Object(result))
    }

    /// Handle guest-file-flush
    fn handle_file_flush(&mut self, args: &BTreeMap<String, JsonValue>) -> Result<JsonValue, QgaError> {
        let handle = args.get("handle")
            .and_then(|v| v.as_i64())
            .ok_or(QgaError::InvalidParameter)?;

        if !self.file_handles.contains_key(&handle) {
            return Err(QgaError::InvalidHandle);
        }

        // In real implementation, would flush file buffers
        Ok(JsonValue::Object(BTreeMap::new()))
    }

    /// Handle guest-fsfreeze-status
    fn handle_fsfreeze_status(&self) -> Result<JsonValue, QgaError> {
        let status = if self.fs_frozen { "frozen" } else { "thawed" };
        Ok(JsonValue::String(status.to_string()))
    }

    /// Handle guest-fsfreeze-freeze
    fn handle_fsfreeze_freeze(&mut self) -> Result<JsonValue, QgaError> {
        if self.fs_frozen {
            return Err(QgaError::Busy);
        }

        // In real implementation, would freeze all filesystems
        self.fs_frozen = true;
        self.frozen_fs_count = 1; // Number of frozen filesystems
        self.state = QgaState::Frozen;

        crate::kprintln!("qga: Filesystems frozen");
        Ok(JsonValue::Number(self.frozen_fs_count as i64))
    }

    /// Handle guest-fsfreeze-thaw
    fn handle_fsfreeze_thaw(&mut self) -> Result<JsonValue, QgaError> {
        if !self.fs_frozen {
            return Ok(JsonValue::Number(0));
        }

        // In real implementation, would thaw all filesystems
        let count = self.frozen_fs_count;
        self.fs_frozen = false;
        self.frozen_fs_count = 0;
        self.state = QgaState::Ready;

        crate::kprintln!("qga: Filesystems thawed");
        Ok(JsonValue::Number(count as i64))
    }

    /// Handle guest-network-get-interfaces
    fn handle_network_get_interfaces(&self) -> Result<JsonValue, QgaError> {
        // Return mock network interface info
        let mut iface = BTreeMap::new();
        iface.insert("name".to_string(), JsonValue::String("eth0".to_string()));
        iface.insert("hardware-address".to_string(), JsonValue::String("00:00:00:00:00:00".to_string()));

        let mut ip_addr = BTreeMap::new();
        ip_addr.insert("ip-address".to_string(), JsonValue::String("10.0.2.15".to_string()));
        ip_addr.insert("ip-address-type".to_string(), JsonValue::String("ipv4".to_string()));
        ip_addr.insert("prefix".to_string(), JsonValue::Number(24));

        iface.insert("ip-addresses".to_string(), JsonValue::Array(vec![JsonValue::Object(ip_addr)]));

        Ok(JsonValue::Array(vec![JsonValue::Object(iface)]))
    }

    /// Handle guest-get-vcpus
    fn handle_get_vcpus(&self) -> Result<JsonValue, QgaError> {
        // Return vCPU info
        let mut vcpu = BTreeMap::new();
        vcpu.insert("logical-id".to_string(), JsonValue::Number(0));
        vcpu.insert("online".to_string(), JsonValue::Bool(true));
        vcpu.insert("can-offline".to_string(), JsonValue::Bool(false));

        Ok(JsonValue::Array(vec![JsonValue::Object(vcpu)]))
    }

    /// Handle guest-get-fsinfo
    fn handle_get_fsinfo(&self) -> Result<JsonValue, QgaError> {
        // Return filesystem info
        let mut fs = BTreeMap::new();
        fs.insert("mountpoint".to_string(), JsonValue::String("/".to_string()));
        fs.insert("name".to_string(), JsonValue::String("rootfs".to_string()));
        fs.insert("type".to_string(), JsonValue::String("ext4".to_string()));
        fs.insert("total-bytes".to_string(), JsonValue::Number(1073741824)); // 1GB
        fs.insert("used-bytes".to_string(), JsonValue::Number(536870912)); // 512MB

        Ok(JsonValue::Array(vec![JsonValue::Object(fs)]))
    }

    /// Handle guest-exec
    fn handle_exec(&mut self, args: &BTreeMap<String, JsonValue>) -> Result<JsonValue, QgaError> {
        let _path = args.get("path")
            .and_then(|v| v.as_string())
            .ok_or(QgaError::InvalidParameter)?;

        // In real implementation, would execute command
        // Return PID for status polling
        let mut result = BTreeMap::new();
        result.insert("pid".to_string(), JsonValue::Number(12345));

        Ok(JsonValue::Object(result))
    }

    /// Handle guest-exec-status
    fn handle_exec_status(&self, args: &BTreeMap<String, JsonValue>) -> Result<JsonValue, QgaError> {
        let _pid = args.get("pid")
            .and_then(|v| v.as_i64())
            .ok_or(QgaError::InvalidParameter)?;

        // In real implementation, would check process status
        let mut result = BTreeMap::new();
        result.insert("exited".to_string(), JsonValue::Bool(true));
        result.insert("exitcode".to_string(), JsonValue::Number(0));
        result.insert("signal".to_string(), JsonValue::Number(0));
        result.insert("out-data".to_string(), JsonValue::String(String::new()));
        result.insert("err-data".to_string(), JsonValue::String(String::new()));

        Ok(JsonValue::Object(result))
    }

    /// Handle guest-get-osinfo
    fn handle_get_osinfo(&self) -> Result<JsonValue, QgaError> {
        let mut info = BTreeMap::new();
        info.insert("id".to_string(), JsonValue::String("stenzel".to_string()));
        info.insert("name".to_string(), JsonValue::String("Stenzel OS".to_string()));
        info.insert("pretty-name".to_string(), JsonValue::String("Stenzel OS".to_string()));
        info.insert("version".to_string(), JsonValue::String("1.0".to_string()));
        info.insert("version-id".to_string(), JsonValue::String("1.0".to_string()));
        info.insert("machine".to_string(), JsonValue::String("x86_64".to_string()));
        info.insert("kernel-release".to_string(), JsonValue::String("1.0.0".to_string()));
        info.insert("kernel-version".to_string(), JsonValue::String("Stenzel OS Kernel".to_string()));

        Ok(JsonValue::Object(info))
    }

    /// Handle guest-get-timezone
    fn handle_get_timezone(&self) -> Result<JsonValue, QgaError> {
        let mut tz = BTreeMap::new();
        tz.insert("zone".to_string(), JsonValue::String("UTC".to_string()));
        tz.insert("offset".to_string(), JsonValue::Number(0));

        Ok(JsonValue::Object(tz))
    }

    /// Handle guest-get-host-name
    fn handle_get_hostname(&self) -> Result<JsonValue, QgaError> {
        let mut info = BTreeMap::new();
        info.insert("host-name".to_string(), JsonValue::String("stenzel".to_string()));

        Ok(JsonValue::Object(info))
    }

    /// Handle guest-get-disks
    fn handle_get_disks(&self) -> Result<JsonValue, QgaError> {
        // Return disk info
        let mut disk = BTreeMap::new();
        disk.insert("name".to_string(), JsonValue::String("vda".to_string()));
        disk.insert("partition".to_string(), JsonValue::Bool(false));
        disk.insert("dependents".to_string(), JsonValue::Array(Vec::new()));

        Ok(JsonValue::Array(vec![JsonValue::Object(disk)]))
    }

    /// Format successful response
    fn format_response(&self, result: Result<JsonValue, QgaError>) -> String {
        match result {
            Ok(value) => {
                alloc::format!("{{\"return\":{}}}\n", value.to_json_string())
            }
            Err(e) => self.format_error(e),
        }
    }

    /// Format error response
    fn format_error(&self, error: QgaError) -> String {
        alloc::format!(
            "{{\"error\":{{\"class\":\"{}\",\"desc\":\"{}\"}}}}\n",
            error.as_str(),
            error.as_str()
        )
    }

    /// Get statistics
    pub fn stats(&self) -> &QgaStats {
        &self.stats
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        alloc::format!(
            "QEMU Guest Agent v{}: state={:?} cmds={} frozen={}",
            QGA_VERSION,
            self.state,
            self.stats.commands_processed.load(Ordering::Relaxed),
            self.fs_frozen
        )
    }
}

impl Default for QemuGuestAgent {
    fn default() -> Self {
        Self::new()
    }
}

// Global QGA instance
static QGA: IrqSafeMutex<Option<QemuGuestAgent>> = IrqSafeMutex::new(None);

/// Initialize QEMU Guest Agent
pub fn init() {
    let agent = QemuGuestAgent::new();
    *QGA.lock() = Some(agent);
    crate::kprintln!("qga: QEMU Guest Agent ready");
}

/// Initialize with VirtIO serial port
pub fn init_with_port(virtio_port: u64) -> Result<(), &'static str> {
    let mut guard = QGA.lock();
    if guard.is_none() {
        *guard = Some(QemuGuestAgent::new());
    }
    if let Some(agent) = guard.as_mut() {
        agent.init(virtio_port)?;
    }
    Ok(())
}

/// Process incoming data
pub fn process_data(data: &[u8]) -> Option<Vec<u8>> {
    QGA.lock()
        .as_mut()
        .and_then(|agent| agent.process_data(data))
}

/// Get status string
pub fn status() -> String {
    QGA.lock()
        .as_ref()
        .map(|agent| agent.format_status())
        .unwrap_or_else(|| "QGA not initialized".to_string())
}

/// Is filesystem frozen?
pub fn is_frozen() -> bool {
    QGA.lock()
        .as_ref()
        .map(|agent| agent.fs_frozen)
        .unwrap_or(false)
}
