//! VMware Guest-Host RPC
//!
//! Communication channel between guest and VMware host.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// RPC channel types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcChannel {
    TcLo,
    RpCI,
}

/// RPC commands
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum RpcCmd {
    Open = 0,
    SendLen = 1,
    SendData = 2,
    RecvLen = 3,
    RecvData = 4,
    RecvStatus = 5,
    Close = 6,
}

/// RPC message types
pub mod msg_type {
    pub const INFO_DNS_NAME: &str = "info-get dns-name";
    pub const INFO_IP_ADDRESS: &str = "info-get ip-address";
    pub const INFO_BUILD_NUMBER: &str = "info-get build-number";
    pub const TOOLS_SET_VERSION: &str = "tools.set.version";
    pub const TOOLS_CAPABILITY: &str = "tools.capability.hgfs_server";
    pub const GUESTINFO_GET: &str = "info-get guestinfo.";
    pub const LOG: &str = "log";
    pub const RESET: &str = "reset";
}

/// RPC state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcState {
    Closed,
    Opened,
    Sending,
    Receiving,
    Error,
}

/// RPC statistics
#[derive(Debug, Default)]
pub struct RpcStats {
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub errors: AtomicU64,
}

/// RPC channel handle
pub struct RpcHandle {
    /// Channel type
    channel: RpcChannel,
    /// Channel cookie
    cookie: u32,
    /// State
    state: RpcState,
    /// Receive buffer
    recv_buffer: Vec<u8>,
    /// Receive length
    recv_len: usize,
}

impl RpcHandle {
    /// Create new handle
    fn new(channel: RpcChannel) -> Self {
        Self {
            channel,
            cookie: 0,
            state: RpcState::Closed,
            recv_buffer: Vec::new(),
            recv_len: 0,
        }
    }
}

/// VMware RPC client
pub struct VmwareRpc {
    /// TcLo channel (guest to host)
    tclo: RpcHandle,
    /// RpCI channel (host to guest)
    rpci: RpcHandle,
    /// Initialized
    initialized: AtomicBool,
    /// Statistics
    stats: RpcStats,
}

impl VmwareRpc {
    /// Magic values
    const VMWARE_MAGIC: u32 = 0x564D5868;
    const RPC_MAGIC: u32 = 0x49435052; // "RPCI"

    /// Create new RPC client
    pub fn new() -> Self {
        Self {
            tclo: RpcHandle::new(RpcChannel::TcLo),
            rpci: RpcHandle::new(RpcChannel::RpCI),
            initialized: AtomicBool::new(false),
            stats: RpcStats::default(),
        }
    }

    /// Execute RPC backdoor command
    fn backdoor(&self, cmd: RpcCmd, channel: RpcChannel, data: u32) -> Option<(u32, u32, u32)> {
        let channel_id = match channel {
            RpcChannel::TcLo => 0x54434C4F, // "TCLO"
            RpcChannel::RpCI => 0x49435052, // "RPCI"
        };

        let mut eax: u32;
        let mut ebx: u32;
        let mut ecx: u32;

        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!(
                "push rbx",
                "mov eax, {magic:e}",
                "mov ebx, {data:e}",
                "mov ecx, {cmd:e}",
                "mov edx, {channel:e}",
                "mov dx, 0x5658",
                "in eax, dx",
                "mov {eax_out:e}, eax",
                "mov {ebx_out:e}, ebx",
                "mov {ecx_out:e}, ecx",
                "pop rbx",
                magic = in(reg) Self::VMWARE_MAGIC,
                data = in(reg) data,
                cmd = in(reg) (cmd as u32 | (0x1E << 16)),
                channel = in(reg) channel_id,
                eax_out = out(reg) eax,
                ebx_out = out(reg) ebx,
                ecx_out = out(reg) ecx,
                options(nostack, nomem)
            );
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            eax = 0;
            ebx = 0;
            ecx = 0;
        }

        if ecx & 0x10000 != 0 {
            Some((eax, ebx, ecx))
        } else {
            None
        }
    }

    /// Initialize RPC
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Open TcLo channel
        if let Some((_, cookie, _)) = self.backdoor(RpcCmd::Open, RpcChannel::TcLo, 0) {
            self.tclo.cookie = cookie;
            self.tclo.state = RpcState::Opened;
        } else {
            return Err("Failed to open TcLo channel");
        }

        // Open RpCI channel
        if let Some((_, cookie, _)) = self.backdoor(RpcCmd::Open, RpcChannel::RpCI, 0) {
            self.rpci.cookie = cookie;
            self.rpci.state = RpcState::Opened;
        } else {
            self.close_channel(RpcChannel::TcLo);
            return Err("Failed to open RpCI channel");
        }

        self.initialized.store(true, Ordering::Release);
        crate::kprintln!("vmware-rpc: Initialized");
        Ok(())
    }

    /// Get handle state
    fn get_handle_state(&self, channel: RpcChannel) -> RpcState {
        match channel {
            RpcChannel::TcLo => self.tclo.state,
            RpcChannel::RpCI => self.rpci.state,
        }
    }

    /// Get handle cookie
    fn get_handle_cookie(&self, channel: RpcChannel) -> u32 {
        match channel {
            RpcChannel::TcLo => self.tclo.cookie,
            RpcChannel::RpCI => self.rpci.cookie,
        }
    }

    /// Set handle state
    fn set_handle_state(&mut self, channel: RpcChannel, state: RpcState) {
        match channel {
            RpcChannel::TcLo => self.tclo.state = state,
            RpcChannel::RpCI => self.rpci.state = state,
        }
    }

    /// Set handle cookie
    fn set_handle_cookie(&mut self, channel: RpcChannel, cookie: u32) {
        match channel {
            RpcChannel::TcLo => self.tclo.cookie = cookie,
            RpcChannel::RpCI => self.rpci.cookie = cookie,
        }
    }

    /// Close channel
    fn close_channel(&mut self, channel: RpcChannel) {
        if self.get_handle_state(channel) != RpcState::Closed {
            let cookie = self.get_handle_cookie(channel);
            let _ = self.backdoor(RpcCmd::Close, channel, cookie);
            self.set_handle_state(channel, RpcState::Closed);
            self.set_handle_cookie(channel, 0);
        }
    }

    /// Send message
    pub fn send(&mut self, channel: RpcChannel, message: &str) -> Result<(), &'static str> {
        if self.get_handle_state(channel) != RpcState::Opened {
            return Err("Channel not open");
        }

        let data = message.as_bytes();
        let len = data.len();

        // Send length
        self.set_handle_state(channel, RpcState::Sending);
        if self.backdoor(RpcCmd::SendLen, channel, len as u32).is_none() {
            self.set_handle_state(channel, RpcState::Error);
            self.stats.errors.fetch_add(1, Ordering::Relaxed);
            return Err("Failed to send length");
        }

        // Send data (4 bytes at a time)
        for chunk in data.chunks(4) {
            let mut word = 0u32;
            for (i, &byte) in chunk.iter().enumerate() {
                word |= (byte as u32) << (i * 8);
            }

            if self.backdoor(RpcCmd::SendData, channel, word).is_none() {
                self.set_handle_state(channel, RpcState::Error);
                self.stats.errors.fetch_add(1, Ordering::Relaxed);
                return Err("Failed to send data");
            }
        }

        self.set_handle_state(channel, RpcState::Opened);
        self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.stats.bytes_sent.fetch_add(len as u64, Ordering::Relaxed);
        Ok(())
    }

    /// Receive message
    pub fn receive(&mut self, channel: RpcChannel) -> Result<String, &'static str> {
        if self.get_handle_state(channel) != RpcState::Opened {
            return Err("Channel not open");
        }

        self.set_handle_state(channel, RpcState::Receiving);

        // Get length
        let len = if let Some((_, _, status)) = self.backdoor(RpcCmd::RecvLen, channel, 0) {
            (status >> 16) as usize
        } else {
            self.set_handle_state(channel, RpcState::Error);
            self.stats.errors.fetch_add(1, Ordering::Relaxed);
            return Err("Failed to receive length");
        };

        if len == 0 {
            self.set_handle_state(channel, RpcState::Opened);
            return Ok(String::new());
        }

        // Receive data into local buffer first
        let mut recv_buffer = Vec::with_capacity(len);
        let mut received = 0;

        while received < len {
            if let Some((_, data, _)) = self.backdoor(RpcCmd::RecvData, channel, 0) {
                let bytes = data.to_le_bytes();
                for &byte in &bytes {
                    if received < len {
                        recv_buffer.push(byte);
                        received += 1;
                    }
                }
            } else {
                self.set_handle_state(channel, RpcState::Error);
                self.stats.errors.fetch_add(1, Ordering::Relaxed);
                return Err("Failed to receive data");
            }
        }

        // Finish receive
        let _ = self.backdoor(RpcCmd::RecvStatus, channel, 1);

        self.set_handle_state(channel, RpcState::Opened);
        self.stats.messages_received.fetch_add(1, Ordering::Relaxed);
        self.stats.bytes_received.fetch_add(len as u64, Ordering::Relaxed);

        String::from_utf8(recv_buffer)
            .map_err(|_| "Invalid UTF-8 in response")
    }

    /// Send and receive (RPC call)
    pub fn call(&mut self, message: &str) -> Result<String, &'static str> {
        self.send(RpcChannel::TcLo, message)?;
        self.receive(RpcChannel::TcLo)
    }

    /// Get guest info
    pub fn get_guest_info(&mut self, key: &str) -> Result<String, &'static str> {
        let msg = alloc::format!("{}{}",  msg_type::GUESTINFO_GET, key);
        self.call(&msg)
    }

    /// Set tools version
    pub fn set_tools_version(&mut self, version: u32) -> Result<(), &'static str> {
        let msg = alloc::format!("{} {}", msg_type::TOOLS_SET_VERSION, version);
        self.send(RpcChannel::TcLo, &msg)
    }

    /// Log message to host
    pub fn log(&mut self, message: &str) -> Result<(), &'static str> {
        let msg = alloc::format!("{} {}", msg_type::LOG, message);
        self.send(RpcChannel::TcLo, &msg)
    }

    /// Get statistics
    pub fn stats(&self) -> &RpcStats {
        &self.stats
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!(
            "VMware RPC: TcLo={:?} RpCI={:?} sent={} recv={}",
            self.tclo.state, self.rpci.state,
            self.stats.messages_sent.load(Ordering::Relaxed),
            self.stats.messages_received.load(Ordering::Relaxed)
        )
    }
}

impl Default for VmwareRpc {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for VmwareRpc {
    fn drop(&mut self) {
        self.close_channel(RpcChannel::TcLo);
        self.close_channel(RpcChannel::RpCI);
    }
}
