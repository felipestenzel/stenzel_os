//! QGA Guest Execution
//!
//! Guest command execution for QEMU Guest Agent.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU32, AtomicBool, Ordering};

use crate::sync::IrqSafeMutex;
use super::{JsonValue, QgaError};

/// Process execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecStatus {
    /// Process is running
    Running,
    /// Process exited normally
    Exited,
    /// Process was killed by signal
    Signaled,
    /// Process status unknown
    Unknown,
}

/// Execution result
#[derive(Debug, Clone)]
pub struct ExecResult {
    /// Process ID
    pub pid: u32,
    /// Exit status (if exited)
    pub exitcode: Option<i32>,
    /// Signal number (if signaled)
    pub signal: Option<i32>,
    /// Standard output (base64 encoded)
    pub out_data: Option<String>,
    /// Standard error (base64 encoded)
    pub err_data: Option<String>,
    /// Output truncated
    pub out_truncated: bool,
    /// Error truncated
    pub err_truncated: bool,
    /// Process exited
    pub exited: bool,
}

impl ExecResult {
    /// Convert to JSON
    pub fn to_json(&self) -> JsonValue {
        let mut obj = BTreeMap::new();
        obj.insert("exited".to_string(), JsonValue::Bool(self.exited));

        if self.exited {
            if let Some(code) = self.exitcode {
                obj.insert("exitcode".to_string(), JsonValue::Number(code as i64));
            }
            if let Some(sig) = self.signal {
                obj.insert("signal".to_string(), JsonValue::Number(sig as i64));
            }
        }

        if let Some(ref out) = self.out_data {
            obj.insert("out-data".to_string(), JsonValue::String(out.clone()));
        }
        if let Some(ref err) = self.err_data {
            obj.insert("err-data".to_string(), JsonValue::String(err.clone()));
        }

        obj.insert("out-truncated".to_string(), JsonValue::Bool(self.out_truncated));
        obj.insert("err-truncated".to_string(), JsonValue::Bool(self.err_truncated));

        JsonValue::Object(obj)
    }
}

/// Running process info
#[derive(Debug)]
pub struct RunningProcess {
    /// Process ID
    pub pid: u32,
    /// Command path
    pub path: String,
    /// Arguments
    pub args: Vec<String>,
    /// Environment
    pub env: Vec<String>,
    /// Input data (base64)
    pub input_data: Option<String>,
    /// Capture stdout
    pub capture_output: bool,
    /// Status
    pub status: ExecStatus,
    /// Exit code (if exited)
    pub exitcode: Option<i32>,
    /// Signal (if signaled)
    pub signal: Option<i32>,
    /// Stdout buffer
    pub stdout: Vec<u8>,
    /// Stderr buffer
    pub stderr: Vec<u8>,
}

impl RunningProcess {
    /// Create new running process
    pub fn new(pid: u32, path: String, args: Vec<String>) -> Self {
        Self {
            pid,
            path,
            args,
            env: Vec::new(),
            input_data: None,
            capture_output: true,
            status: ExecStatus::Running,
            exitcode: None,
            signal: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    }

    /// Check if process has exited
    pub fn has_exited(&self) -> bool {
        self.status == ExecStatus::Exited || self.status == ExecStatus::Signaled
    }

    /// Get result
    pub fn get_result(&self) -> ExecResult {
        ExecResult {
            pid: self.pid,
            exitcode: self.exitcode,
            signal: self.signal,
            out_data: if self.capture_output && !self.stdout.is_empty() {
                Some(base64_encode(&self.stdout))
            } else {
                None
            },
            err_data: if self.capture_output && !self.stderr.is_empty() {
                Some(base64_encode(&self.stderr))
            } else {
                None
            },
            out_truncated: self.stdout.len() > 65536,
            err_truncated: self.stderr.len() > 65536,
            exited: self.has_exited(),
        }
    }

    /// Simulate process completion
    pub fn simulate_exit(&mut self, exitcode: i32) {
        self.status = ExecStatus::Exited;
        self.exitcode = Some(exitcode);
    }
}

/// Execution manager
pub struct ExecManager {
    /// Running processes
    processes: BTreeMap<u32, RunningProcess>,
    /// Next PID
    next_pid: AtomicU32,
    /// Maximum concurrent processes
    max_processes: u32,
    /// Initialized
    initialized: AtomicBool,
}

impl ExecManager {
    /// Create new execution manager
    pub fn new() -> Self {
        Self {
            processes: BTreeMap::new(),
            next_pid: AtomicU32::new(10000),
            max_processes: 100,
            initialized: AtomicBool::new(false),
        }
    }

    /// Initialize
    pub fn init(&mut self) {
        self.initialized.store(true, Ordering::Release);
        crate::kprintln!("qga-exec: Execution manager initialized");
    }

    /// Execute a command
    pub fn exec(&mut self, args: &BTreeMap<String, JsonValue>) -> Result<JsonValue, QgaError> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err(QgaError::NotSupported);
        }

        if self.processes.len() >= self.max_processes as usize {
            return Err(QgaError::Busy);
        }

        let path = args.get("path")
            .and_then(|v| v.as_string())
            .ok_or(QgaError::InvalidParameter)?
            .to_string();

        // Parse arguments array
        let cmd_args: Vec<String> = args.get("arg")
            .and_then(|v| {
                if let JsonValue::Array(arr) = v {
                    Some(arr.iter()
                        .filter_map(|a| a.as_string().map(|s| s.to_string()))
                        .collect())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let capture_output = args.get("capture-output")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Allocate PID
        let pid = self.next_pid.fetch_add(1, Ordering::Relaxed);

        // Create process entry
        let mut process = RunningProcess::new(pid, path.clone(), cmd_args.clone());
        process.capture_output = capture_output;

        // Check for input data
        if let Some(input) = args.get("input-data").and_then(|v| v.as_string()) {
            process.input_data = Some(input.to_string());
        }

        // In a real implementation, would actually execute the command
        // For now, simulate immediate completion for certain commands
        match path.as_str() {
            "/bin/echo" | "/usr/bin/echo" => {
                let output = cmd_args.join(" ");
                process.stdout = output.into_bytes();
                process.simulate_exit(0);
            }
            "/bin/true" | "/usr/bin/true" => {
                process.simulate_exit(0);
            }
            "/bin/false" | "/usr/bin/false" => {
                process.simulate_exit(1);
            }
            "/bin/uname" | "/usr/bin/uname" => {
                process.stdout = b"Stenzel OS 1.0.0 x86_64".to_vec();
                process.simulate_exit(0);
            }
            "/bin/hostname" | "/usr/bin/hostname" => {
                process.stdout = b"stenzel".to_vec();
                process.simulate_exit(0);
            }
            "/bin/whoami" | "/usr/bin/whoami" => {
                process.stdout = b"root".to_vec();
                process.simulate_exit(0);
            }
            "/bin/pwd" | "/usr/bin/pwd" => {
                process.stdout = b"/root".to_vec();
                process.simulate_exit(0);
            }
            "/bin/date" | "/usr/bin/date" => {
                let ticks = crate::time::ticks() / 1000;
                process.stdout = alloc::format!("Uptime: {} seconds", ticks).into_bytes();
                process.simulate_exit(0);
            }
            _ => {
                // Unknown command - simulate not found
                process.stderr = alloc::format!("{}: command not found", path).into_bytes();
                process.simulate_exit(127);
            }
        }

        crate::kprintln!("qga-exec: Started process {} ({})", pid, path);

        self.processes.insert(pid, process);

        // Return PID
        let mut result = BTreeMap::new();
        result.insert("pid".to_string(), JsonValue::Number(pid as i64));
        Ok(JsonValue::Object(result))
    }

    /// Get execution status
    pub fn exec_status(&self, args: &BTreeMap<String, JsonValue>) -> Result<JsonValue, QgaError> {
        let pid = args.get("pid")
            .and_then(|v| v.as_i64())
            .ok_or(QgaError::InvalidParameter)? as u32;

        let process = self.processes.get(&pid)
            .ok_or(QgaError::InvalidParameter)?;

        Ok(process.get_result().to_json())
    }

    /// Clean up completed processes
    pub fn cleanup_completed(&mut self) {
        self.processes.retain(|_, p| !p.has_exited());
    }

    /// Get running process count
    pub fn running_count(&self) -> usize {
        self.processes.values().filter(|p| !p.has_exited()).count()
    }
}

impl Default for ExecManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global execution manager
static EXEC_MGR: IrqSafeMutex<Option<ExecManager>> = IrqSafeMutex::new(None);

/// Initialize execution manager
pub fn init() {
    let mut mgr = ExecManager::new();
    mgr.init();
    *EXEC_MGR.lock() = Some(mgr);
}

/// Execute command
pub fn exec(args: &BTreeMap<String, JsonValue>) -> Result<JsonValue, QgaError> {
    EXEC_MGR.lock()
        .as_mut()
        .ok_or(QgaError::NotSupported)?
        .exec(args)
}

/// Get execution status
pub fn exec_status(args: &BTreeMap<String, JsonValue>) -> Result<JsonValue, QgaError> {
    EXEC_MGR.lock()
        .as_ref()
        .ok_or(QgaError::NotSupported)?
        .exec_status(args)
}

/// Simple base64 encoding
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::new();

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;

        let combined = (b0 << 16) | (b1 << 8) | b2;

        result.push(ALPHABET[((combined >> 18) & 0x3F) as usize] as char);
        result.push(ALPHABET[((combined >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[((combined >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[(combined & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

/// Simple base64 decoding
pub fn base64_decode(encoded: &str) -> Vec<u8> {
    fn decode_char(c: char) -> Option<u8> {
        match c {
            'A'..='Z' => Some(c as u8 - b'A'),
            'a'..='z' => Some(c as u8 - b'a' + 26),
            '0'..='9' => Some(c as u8 - b'0' + 52),
            '+' => Some(62),
            '/' => Some(63),
            '=' => Some(0),
            _ => None,
        }
    }

    let mut result = Vec::new();
    let chars: Vec<char> = encoded.chars().collect();

    for chunk in chars.chunks(4) {
        if chunk.len() < 4 {
            break;
        }

        let b0 = decode_char(chunk[0]).unwrap_or(0) as u32;
        let b1 = decode_char(chunk[1]).unwrap_or(0) as u32;
        let b2 = decode_char(chunk[2]).unwrap_or(0) as u32;
        let b3 = decode_char(chunk[3]).unwrap_or(0) as u32;

        let combined = (b0 << 18) | (b1 << 12) | (b2 << 6) | b3;

        result.push((combined >> 16) as u8);

        if chunk[2] != '=' {
            result.push((combined >> 8) as u8);
        }

        if chunk[3] != '=' {
            result.push(combined as u8);
        }
    }

    result
}
