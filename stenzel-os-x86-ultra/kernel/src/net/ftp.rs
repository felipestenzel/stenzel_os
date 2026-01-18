//! FTP Client
//!
//! Implementation of FTP protocol (RFC 959) for file transfers.
//! Supports both active and passive modes, ASCII and binary transfers.
//!
//! Reference: RFC 959 (File Transfer Protocol)

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

use crate::util::{KResult, KError};
use super::tcp::{self, TcpConnKey};
use super::dns;
use super::Ipv4Addr;

// FTP Response Codes (RFC 959)
pub mod reply_codes {
    // Positive Preliminary (1xx)
    pub const RESTART_MARKER: u16 = 110;
    pub const SERVICE_READY_IN_MINUTES: u16 = 120;
    pub const DATA_CONN_ALREADY_OPEN: u16 = 125;
    pub const FILE_STATUS_OK: u16 = 150;

    // Positive Completion (2xx)
    pub const COMMAND_OK: u16 = 200;
    pub const COMMAND_NOT_IMPLEMENTED_SUPERFLUOUS: u16 = 202;
    pub const SYSTEM_STATUS: u16 = 211;
    pub const DIRECTORY_STATUS: u16 = 212;
    pub const FILE_STATUS: u16 = 213;
    pub const HELP_MESSAGE: u16 = 214;
    pub const SYSTEM_TYPE: u16 = 215;
    pub const SERVICE_READY: u16 = 220;
    pub const SERVICE_CLOSING: u16 = 221;
    pub const DATA_CONN_OPEN: u16 = 225;
    pub const CLOSING_DATA_CONN: u16 = 226;
    pub const ENTERING_PASSIVE_MODE: u16 = 227;
    pub const ENTERING_EXTENDED_PASSIVE: u16 = 229;
    pub const USER_LOGGED_IN: u16 = 230;
    pub const FILE_ACTION_OK: u16 = 250;
    pub const PATHNAME_CREATED: u16 = 257;

    // Positive Intermediate (3xx)
    pub const USER_NAME_OK: u16 = 331;
    pub const NEED_ACCOUNT: u16 = 332;
    pub const FILE_ACTION_PENDING: u16 = 350;

    // Transient Negative (4xx)
    pub const SERVICE_NOT_AVAILABLE: u16 = 421;
    pub const CANT_OPEN_DATA_CONN: u16 = 425;
    pub const CONN_CLOSED_TRANSFER_ABORTED: u16 = 426;
    pub const FILE_ACTION_NOT_TAKEN: u16 = 450;
    pub const ACTION_ABORTED: u16 = 451;
    pub const ACTION_NOT_TAKEN_NO_SPACE: u16 = 452;

    // Permanent Negative (5xx)
    pub const SYNTAX_ERROR: u16 = 500;
    pub const SYNTAX_ERROR_PARAM: u16 = 501;
    pub const COMMAND_NOT_IMPLEMENTED: u16 = 502;
    pub const BAD_COMMAND_SEQUENCE: u16 = 503;
    pub const COMMAND_NOT_IMPLEMENTED_FOR_PARAM: u16 = 504;
    pub const NOT_LOGGED_IN: u16 = 530;
    pub const NEED_ACCOUNT_FOR_STORING: u16 = 532;
    pub const FILE_UNAVAILABLE: u16 = 550;
    pub const PAGE_TYPE_UNKNOWN: u16 = 551;
    pub const EXCEEDED_STORAGE: u16 = 552;
    pub const FILE_NAME_NOT_ALLOWED: u16 = 553;

    /// Check if code indicates success
    pub fn is_success(code: u16) -> bool {
        code >= 200 && code < 300
    }

    /// Check if code indicates positive intermediate
    pub fn is_intermediate(code: u16) -> bool {
        code >= 300 && code < 400
    }

    /// Check if code indicates error
    pub fn is_error(code: u16) -> bool {
        code >= 400
    }
}

/// FTP Transfer Mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferMode {
    /// ASCII mode (text files)
    Ascii,
    /// Binary/Image mode (all other files)
    Binary,
}

impl TransferMode {
    fn command(&self) -> &'static str {
        match self {
            Self::Ascii => "A",
            Self::Binary => "I",
        }
    }
}

/// FTP Connection Mode for data transfer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    /// Server connects to client
    Active,
    /// Client connects to server
    Passive,
}

/// FTP Connection State
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FtpState {
    /// Not connected
    Disconnected,
    /// Connected, not logged in
    Connected,
    /// Logged in
    LoggedIn,
    /// Data transfer in progress
    Transferring,
}

/// FTP Response
#[derive(Debug, Clone)]
pub struct FtpResponse {
    /// Response code (3 digits)
    pub code: u16,
    /// Response message
    pub message: String,
}

impl FtpResponse {
    /// Parse response from server
    fn parse(data: &str) -> Option<Self> {
        if data.len() < 3 {
            return None;
        }
        let code: u16 = data[..3].parse().ok()?;
        let message = if data.len() > 4 {
            String::from(data[4..].trim())
        } else {
            String::new()
        };
        Some(Self { code, message })
    }

    /// Is success code?
    pub fn is_success(&self) -> bool {
        reply_codes::is_success(self.code)
    }

    /// Is error?
    pub fn is_error(&self) -> bool {
        reply_codes::is_error(self.code)
    }
}

/// Directory entry from LIST command
#[derive(Debug, Clone)]
pub struct FtpDirEntry {
    /// File name
    pub name: String,
    /// File size in bytes
    pub size: u64,
    /// Is directory
    pub is_dir: bool,
    /// Permissions string (e.g., "drwxr-xr-x")
    pub permissions: String,
    /// Owner
    pub owner: String,
    /// Group
    pub group: String,
    /// Modification date string
    pub date: String,
}

impl FtpDirEntry {
    /// Parse Unix-style directory listing line
    fn parse_unix(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            return None;
        }

        let permissions = String::from(parts[0]);
        let is_dir = permissions.starts_with('d');
        let size: u64 = parts[4].parse().unwrap_or(0);
        let owner = String::from(parts[2]);
        let group = String::from(parts[3]);
        let date = format!("{} {} {}", parts[5], parts[6], parts[7]);
        let name = parts[8..].join(" ");

        Some(Self {
            name,
            size,
            is_dir,
            permissions,
            owner,
            group,
            date,
        })
    }

    /// Parse Windows-style directory listing line
    fn parse_windows(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            return None;
        }

        let date = format!("{} {}", parts[0], parts[1]);
        let is_dir = parts[2] == "<DIR>";
        let size = if is_dir {
            0
        } else {
            parts[2].replace(',', "").parse().unwrap_or(0)
        };
        let name = parts[3..].join(" ");

        Some(Self {
            name,
            size,
            is_dir,
            permissions: if is_dir {
                String::from("drwxr-xr-x")
            } else {
                String::from("-rw-r--r--")
            },
            owner: String::new(),
            group: String::new(),
            date,
        })
    }

    /// Parse directory entry from line
    pub fn parse(line: &str) -> Option<Self> {
        // Try Unix format first, then Windows
        Self::parse_unix(line).or_else(|| Self::parse_windows(line))
    }
}

/// FTP Client
pub struct FtpClient {
    /// Control connection
    control: Option<TcpConnKey>,
    /// Connection state
    state: FtpState,
    /// Server address
    server_addr: Option<Ipv4Addr>,
    /// Server port
    server_port: u16,
    /// Transfer mode
    transfer_mode: TransferMode,
    /// Connection mode (active/passive)
    connection_mode: ConnectionMode,
    /// Current directory
    current_dir: String,
    /// Receive buffer
    recv_buffer: Vec<u8>,
    /// Last response
    last_response: Option<FtpResponse>,
    /// Logged in username
    username: Option<String>,
}

impl FtpClient {
    /// Create new FTP client
    pub fn new() -> Self {
        Self {
            control: None,
            state: FtpState::Disconnected,
            server_addr: None,
            server_port: 21,
            transfer_mode: TransferMode::Binary,
            connection_mode: ConnectionMode::Passive,
            current_dir: String::from("/"),
            recv_buffer: Vec::new(),
            last_response: None,
            username: None,
        }
    }

    /// Connect to FTP server
    pub fn connect(&mut self, host: &str, port: u16) -> KResult<FtpResponse> {
        let addr = dns::resolve(host)?;

        let tcp_key = tcp::connect(addr, port)?;
        self.control = Some(tcp_key);
        self.server_addr = Some(addr);
        self.server_port = port;

        // Read welcome message
        let response = self.read_response()?;
        if response.is_error() {
            self.disconnect()?;
            return Err(KError::PermissionDenied);
        }

        self.state = FtpState::Connected;
        Ok(response)
    }

    /// Login with username and password
    pub fn login(&mut self, username: &str, password: &str) -> KResult<FtpResponse> {
        if self.state == FtpState::Disconnected {
            return Err(KError::Invalid);
        }

        // Send USER command
        let response = self.send_command(&format!("USER {}", username))?;

        if response.code == reply_codes::USER_LOGGED_IN {
            // No password needed
            self.state = FtpState::LoggedIn;
            self.username = Some(String::from(username));
            return Ok(response);
        }

        if response.code != reply_codes::USER_NAME_OK {
            return Err(KError::PermissionDenied);
        }

        // Send PASS command
        let response = self.send_command(&format!("PASS {}", password))?;

        if response.code == reply_codes::USER_LOGGED_IN {
            self.state = FtpState::LoggedIn;
            self.username = Some(String::from(username));
            Ok(response)
        } else {
            Err(KError::PermissionDenied)
        }
    }

    /// Login anonymously
    pub fn login_anonymous(&mut self) -> KResult<FtpResponse> {
        self.login("anonymous", "anonymous@stenzel.os")
    }

    /// Disconnect from server
    pub fn disconnect(&mut self) -> KResult<()> {
        if self.state != FtpState::Disconnected {
            // Send QUIT command
            let _ = self.send_command("QUIT");
        }

        if let Some(ref key) = self.control {
            tcp::close(key)?;
        }

        self.control = None;
        self.state = FtpState::Disconnected;
        self.server_addr = None;
        self.username = None;
        self.current_dir = String::from("/");

        Ok(())
    }

    /// Set transfer mode
    pub fn set_transfer_mode(&mut self, mode: TransferMode) -> KResult<FtpResponse> {
        let response = self.send_command(&format!("TYPE {}", mode.command()))?;
        if response.is_success() {
            self.transfer_mode = mode;
        }
        Ok(response)
    }

    /// Set connection mode (active/passive)
    pub fn set_connection_mode(&mut self, mode: ConnectionMode) {
        self.connection_mode = mode;
    }

    /// Get current directory
    pub fn pwd(&mut self) -> KResult<String> {
        let response = self.send_command("PWD")?;

        if response.code != reply_codes::PATHNAME_CREATED {
            return Err(KError::IO);
        }

        // Parse directory from response like: 257 "/path" is current directory
        if let Some(start) = response.message.find('"') {
            if let Some(end) = response.message[start + 1..].find('"') {
                let path = &response.message[start + 1..start + 1 + end];
                self.current_dir = String::from(path);
                return Ok(String::from(path));
            }
        }

        Ok(response.message)
    }

    /// Change directory
    pub fn cwd(&mut self, path: &str) -> KResult<FtpResponse> {
        let response = self.send_command(&format!("CWD {}", path))?;
        if response.is_success() {
            if path.starts_with('/') {
                self.current_dir = String::from(path);
            } else {
                self.current_dir = format!("{}/{}", self.current_dir.trim_end_matches('/'), path);
            }
        }
        Ok(response)
    }

    /// Change to parent directory
    pub fn cdup(&mut self) -> KResult<FtpResponse> {
        let response = self.send_command("CDUP")?;
        if response.is_success() {
            // Update current_dir
            if let Some(pos) = self.current_dir.rfind('/') {
                if pos > 0 {
                    self.current_dir.truncate(pos);
                } else {
                    self.current_dir = String::from("/");
                }
            }
        }
        Ok(response)
    }

    /// List directory contents
    pub fn list(&mut self, path: Option<&str>) -> KResult<Vec<FtpDirEntry>> {
        // Open data connection
        let data_conn = self.open_data_connection()?;

        // Send LIST command
        let cmd = if let Some(p) = path {
            format!("LIST {}", p)
        } else {
            String::from("LIST")
        };

        let response = self.send_command(&cmd)?;

        if response.code != reply_codes::FILE_STATUS_OK &&
           response.code != reply_codes::DATA_CONN_ALREADY_OPEN {
            tcp::close(&data_conn)?;
            return Err(KError::IO);
        }

        // Read data
        let data = self.read_data_connection(&data_conn)?;

        // Read transfer complete response
        let _ = self.read_response();

        // Parse directory listing
        let listing = String::from_utf8_lossy(&data);
        let entries: Vec<FtpDirEntry> = listing
            .lines()
            .filter_map(FtpDirEntry::parse)
            .collect();

        Ok(entries)
    }

    /// List directory contents (NLST - names only)
    pub fn nlst(&mut self, path: Option<&str>) -> KResult<Vec<String>> {
        let data_conn = self.open_data_connection()?;

        let cmd = if let Some(p) = path {
            format!("NLST {}", p)
        } else {
            String::from("NLST")
        };

        let response = self.send_command(&cmd)?;

        if response.code != reply_codes::FILE_STATUS_OK &&
           response.code != reply_codes::DATA_CONN_ALREADY_OPEN {
            tcp::close(&data_conn)?;
            return Err(KError::IO);
        }

        let data = self.read_data_connection(&data_conn)?;
        let _ = self.read_response();

        let listing = String::from_utf8_lossy(&data);
        let names: Vec<String> = listing
            .lines()
            .map(|s| String::from(s.trim()))
            .filter(|s| !s.is_empty())
            .collect();

        Ok(names)
    }

    /// Download a file
    pub fn retrieve(&mut self, remote_path: &str) -> KResult<Vec<u8>> {
        let data_conn = self.open_data_connection()?;

        let response = self.send_command(&format!("RETR {}", remote_path))?;

        if response.code != reply_codes::FILE_STATUS_OK &&
           response.code != reply_codes::DATA_CONN_ALREADY_OPEN {
            tcp::close(&data_conn)?;
            return Err(KError::NotFound);
        }

        let data = self.read_data_connection(&data_conn)?;
        let _ = self.read_response();

        Ok(data)
    }

    /// Upload a file
    pub fn store(&mut self, remote_path: &str, data: &[u8]) -> KResult<FtpResponse> {
        let data_conn = self.open_data_connection()?;

        let response = self.send_command(&format!("STOR {}", remote_path))?;

        if response.code != reply_codes::FILE_STATUS_OK &&
           response.code != reply_codes::DATA_CONN_ALREADY_OPEN {
            tcp::close(&data_conn)?;
            return Err(KError::IO);
        }

        // Send data
        self.write_data_connection(&data_conn, data)?;
        tcp::close(&data_conn)?;

        // Read transfer complete response
        self.read_response()
    }

    /// Append to a file
    pub fn append(&mut self, remote_path: &str, data: &[u8]) -> KResult<FtpResponse> {
        let data_conn = self.open_data_connection()?;

        let response = self.send_command(&format!("APPE {}", remote_path))?;

        if response.code != reply_codes::FILE_STATUS_OK &&
           response.code != reply_codes::DATA_CONN_ALREADY_OPEN {
            tcp::close(&data_conn)?;
            return Err(KError::IO);
        }

        self.write_data_connection(&data_conn, data)?;
        tcp::close(&data_conn)?;

        self.read_response()
    }

    /// Delete a file
    pub fn delete(&mut self, path: &str) -> KResult<FtpResponse> {
        self.send_command(&format!("DELE {}", path))
    }

    /// Create directory
    pub fn mkdir(&mut self, path: &str) -> KResult<FtpResponse> {
        self.send_command(&format!("MKD {}", path))
    }

    /// Remove directory
    pub fn rmdir(&mut self, path: &str) -> KResult<FtpResponse> {
        self.send_command(&format!("RMD {}", path))
    }

    /// Rename file (two-step: RNFR then RNTO)
    pub fn rename(&mut self, from: &str, to: &str) -> KResult<FtpResponse> {
        let response = self.send_command(&format!("RNFR {}", from))?;

        if response.code != reply_codes::FILE_ACTION_PENDING {
            return Err(KError::NotFound);
        }

        self.send_command(&format!("RNTO {}", to))
    }

    /// Get file size
    pub fn size(&mut self, path: &str) -> KResult<u64> {
        let response = self.send_command(&format!("SIZE {}", path))?;

        if !response.is_success() {
            return Err(KError::NotFound);
        }

        response.message.trim().parse()
            .map_err(|_| KError::Invalid)
    }

    /// Get modification time
    pub fn mdtm(&mut self, path: &str) -> KResult<String> {
        let response = self.send_command(&format!("MDTM {}", path))?;

        if !response.is_success() {
            return Err(KError::NotFound);
        }

        Ok(response.message)
    }

    /// Send NOOP (keep-alive)
    pub fn noop(&mut self) -> KResult<FtpResponse> {
        self.send_command("NOOP")
    }

    /// Get system type
    pub fn syst(&mut self) -> KResult<String> {
        let response = self.send_command("SYST")?;
        Ok(response.message)
    }

    /// Get server features
    pub fn feat(&mut self) -> KResult<Vec<String>> {
        let response = self.send_command("FEAT")?;

        let features: Vec<String> = response.message
            .lines()
            .map(|s| String::from(s.trim()))
            .filter(|s| !s.is_empty() && !s.starts_with("211"))
            .collect();

        Ok(features)
    }

    /// Abort current transfer
    pub fn abort(&mut self) -> KResult<FtpResponse> {
        self.send_command("ABOR")
    }

    /// Get state
    pub fn state(&self) -> FtpState {
        self.state
    }

    /// Get current directory
    pub fn current_dir(&self) -> &str {
        &self.current_dir
    }

    /// Is connected?
    pub fn is_connected(&self) -> bool {
        self.state != FtpState::Disconnected
    }

    /// Is logged in?
    pub fn is_logged_in(&self) -> bool {
        self.state == FtpState::LoggedIn
    }

    /// Get last response
    pub fn last_response(&self) -> Option<&FtpResponse> {
        self.last_response.as_ref()
    }

    // Internal methods

    /// Send command and read response
    fn send_command(&mut self, cmd: &str) -> KResult<FtpResponse> {
        let key = self.control.as_ref().ok_or(KError::Invalid)?;

        let cmd_line = format!("{}\r\n", cmd);
        tcp::send(key, cmd_line.as_bytes())?;

        self.read_response()
    }

    /// Read response from server
    fn read_response(&mut self) -> KResult<FtpResponse> {
        let key = self.control.as_ref().ok_or(KError::Invalid)?;

        let mut response_text = String::new();
        let mut buf = [0u8; 1024];

        loop {
            // Try to read more data
            match tcp::recv(key, &mut buf) {
                Ok(n) if n > 0 => {
                    self.recv_buffer.extend_from_slice(&buf[..n]);
                }
                _ => {}
            }

            // Try to parse response from buffer
            let text = String::from_utf8_lossy(&self.recv_buffer);

            // Check for complete response (ends with \r\n)
            if let Some(pos) = text.find("\r\n") {
                response_text = text[..pos].to_string();

                // Handle multi-line responses (code followed by -)
                if response_text.len() >= 4 && response_text.chars().nth(3) == Some('-') {
                    let code = &response_text[..3];
                    let end_marker = format!("{} ", code);

                    // Look for end of multi-line response
                    if let Some(end_pos) = text.find(&end_marker) {
                        if let Some(line_end) = text[end_pos..].find("\r\n") {
                            let final_pos = end_pos + line_end;
                            response_text = text[..final_pos].to_string();
                            self.recv_buffer = self.recv_buffer[final_pos + 2..].to_vec();

                            let response = FtpResponse::parse(&response_text[end_pos..])
                                .ok_or(KError::IO)?;
                            self.last_response = Some(response.clone());
                            return Ok(response);
                        }
                    }
                    continue;
                }

                // Single line response
                self.recv_buffer = self.recv_buffer[pos + 2..].to_vec();
                break;
            }
        }

        let response = FtpResponse::parse(&response_text).ok_or(KError::IO)?;
        self.last_response = Some(response.clone());
        Ok(response)
    }

    /// Open data connection (passive or active mode)
    fn open_data_connection(&mut self) -> KResult<TcpConnKey> {
        match self.connection_mode {
            ConnectionMode::Passive => self.open_passive_connection(),
            ConnectionMode::Active => self.open_active_connection(),
        }
    }

    /// Open passive data connection
    fn open_passive_connection(&mut self) -> KResult<TcpConnKey> {
        let response = self.send_command("PASV")?;

        if response.code != reply_codes::ENTERING_PASSIVE_MODE {
            return Err(KError::NotSupported);
        }

        // Parse response like: 227 Entering Passive Mode (h1,h2,h3,h4,p1,p2)
        let msg = &response.message;
        let start = msg.find('(').ok_or(KError::IO)?;
        let end = msg.find(')').ok_or(KError::IO)?;
        let nums_str = &msg[start + 1..end];

        let nums: Vec<u8> = nums_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        if nums.len() != 6 {
            return Err(KError::IO);
        }

        let addr = Ipv4Addr::new(nums[0], nums[1], nums[2], nums[3]);
        let port = (nums[4] as u16) * 256 + (nums[5] as u16);

        tcp::connect(addr, port)
    }

    /// Open active data connection (PORT command)
    fn open_active_connection(&mut self) -> KResult<TcpConnKey> {
        // For active mode, we'd need to listen on a port and tell the server
        // This is more complex and less commonly used, so we'll return an error
        // In a full implementation, you'd:
        // 1. Listen on a random port
        // 2. Send PORT h1,h2,h3,h4,p1,p2 command
        // 3. Accept the server's connection
        Err(KError::NotSupported)
    }

    /// Read all data from data connection
    fn read_data_connection(&mut self, key: &TcpConnKey) -> KResult<Vec<u8>> {
        let mut data = Vec::new();
        let mut buf = [0u8; 4096];

        loop {
            match tcp::recv(key, &mut buf) {
                Ok(0) => break,
                Ok(n) => data.extend_from_slice(&buf[..n]),
                Err(KError::WouldBlock) => {
                    // Check if connection is still alive
                    if tcp::get_state(key).is_none() {
                        break;
                    }
                    continue;
                }
                Err(_) => break,
            }
        }

        tcp::close(key)?;
        Ok(data)
    }

    /// Write data to data connection
    fn write_data_connection(&self, key: &TcpConnKey, data: &[u8]) -> KResult<()> {
        let mut offset = 0;
        while offset < data.len() {
            match tcp::send(key, &data[offset..]) {
                Ok(n) => offset += n,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

impl Default for FtpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FtpClient {
    fn drop(&mut self) {
        let _ = self.disconnect();
    }
}

// Convenience functions

/// Connect to FTP server and login
pub fn connect_and_login(host: &str, port: u16, username: &str, password: &str) -> KResult<FtpClient> {
    let mut client = FtpClient::new();
    client.connect(host, port)?;
    client.login(username, password)?;
    Ok(client)
}

/// Connect to FTP server anonymously
pub fn connect_anonymous(host: &str, port: u16) -> KResult<FtpClient> {
    let mut client = FtpClient::new();
    client.connect(host, port)?;
    client.login_anonymous()?;
    Ok(client)
}

/// Download a file from FTP server
pub fn download_file(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    remote_path: &str,
) -> KResult<Vec<u8>> {
    let mut client = connect_and_login(host, port, username, password)?;
    client.set_transfer_mode(TransferMode::Binary)?;
    let data = client.retrieve(remote_path)?;
    client.disconnect()?;
    Ok(data)
}

/// Upload a file to FTP server
pub fn upload_file(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    remote_path: &str,
    data: &[u8],
) -> KResult<()> {
    let mut client = connect_and_login(host, port, username, password)?;
    client.set_transfer_mode(TransferMode::Binary)?;
    client.store(remote_path, data)?;
    client.disconnect()?;
    Ok(())
}

/// List directory contents on FTP server
pub fn list_dir(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    path: Option<&str>,
) -> KResult<Vec<FtpDirEntry>> {
    let mut client = connect_and_login(host, port, username, password)?;
    let entries = client.list(path)?;
    client.disconnect()?;
    Ok(entries)
}

/// Get file size from FTP server
pub fn get_file_size(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    path: &str,
) -> KResult<u64> {
    let mut client = connect_and_login(host, port, username, password)?;
    let size = client.size(path)?;
    client.disconnect()?;
    Ok(size)
}

/// Format FTP client status
pub fn format_status(client: &FtpClient) -> String {
    let state_str = match client.state {
        FtpState::Disconnected => "Disconnected",
        FtpState::Connected => "Connected",
        FtpState::LoggedIn => "Logged in",
        FtpState::Transferring => "Transferring",
    };

    let mode_str = match client.connection_mode {
        ConnectionMode::Active => "Active",
        ConnectionMode::Passive => "Passive",
    };

    let transfer_str = match client.transfer_mode {
        TransferMode::Ascii => "ASCII",
        TransferMode::Binary => "Binary",
    };

    if let Some(ref user) = client.username {
        format!("FTP: {} ({}@{}, {}, {})",
            state_str, user,
            client.server_addr.map(|a| a.to_string()).unwrap_or_else(|| String::from("?")),
            mode_str, transfer_str)
    } else {
        format!("FTP: {}", state_str)
    }
}
