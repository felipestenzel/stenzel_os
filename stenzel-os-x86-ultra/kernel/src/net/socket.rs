//! Socket API para userspace
//!
//! Supports:
//! - AF_INET (IPv4) sockets: TCP and UDP
//! - AF_UNIX (Unix domain) sockets: local IPC

#![allow(dead_code)]

use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use super::{Ipv4Addr, tcp, udp};
use crate::sync::IrqSafeMutex;

/// Tipos de socket
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Stream,     // TCP or Unix stream
    Datagram,   // UDP or Unix datagram
    Raw,        // Raw IP
}

/// Domínio de socket
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketDomain {
    Inet,       // IPv4
    Inet6,      // IPv6 (não implementado)
    Unix,       // Unix domain (não implementado)
}

/// Estado do socket
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    Created,
    Bound,
    Listening,
    Connected,
    Closed,
}

/// Endereço de socket
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketAddr {
    pub ip: Ipv4Addr,
    pub port: u16,
}

impl SocketAddr {
    pub fn new(ip: Ipv4Addr, port: u16) -> Self {
        Self { ip, port }
    }

    pub fn any(port: u16) -> Self {
        Self { ip: Ipv4Addr::UNSPECIFIED, port }
    }

    /// Parse de sockaddr_in (estrutura C)
    pub fn from_sockaddr_in(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        // sin_family (2) + sin_port (2) + sin_addr (4)
        let port = u16::from_be_bytes([data[2], data[3]]);
        let ip = Ipv4Addr::from_bytes(&data[4..8]);
        Some(Self { ip, port })
    }

    /// Converte para sockaddr_in
    pub fn to_sockaddr_in(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0] = 2; // AF_INET (little endian)
        buf[1] = 0;
        buf[2] = (self.port >> 8) as u8;
        buf[3] = (self.port & 0xFF) as u8;
        buf[4..8].copy_from_slice(&self.ip.0);
        buf
    }
}

// ==================== Unix Domain Sockets ====================

/// Maximum path length for Unix socket addresses
const UNIX_PATH_MAX: usize = 108;

/// Unix socket address (sun_path)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnixSocketAddr {
    /// Pathname (empty string for unnamed sockets)
    pub path: String,
}

impl UnixSocketAddr {
    pub fn new(path: &str) -> Self {
        Self { path: String::from(path) }
    }

    pub fn unnamed() -> Self {
        Self { path: String::new() }
    }

    /// Parse from sockaddr_un structure
    pub fn from_sockaddr_un(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }
        // sun_family (2 bytes) + sun_path (up to 108 bytes)
        let family = u16::from_ne_bytes([data[0], data[1]]);
        if family != 1 {
            // AF_UNIX = 1
            return None;
        }
        if data.len() > 2 {
            // Find null terminator or end of data
            let path_bytes = &data[2..];
            let path_len = path_bytes.iter().position(|&b| b == 0).unwrap_or(path_bytes.len());
            let path = core::str::from_utf8(&path_bytes[..path_len]).ok()?;
            Some(Self { path: String::from(path) })
        } else {
            Some(Self::unnamed())
        }
    }

    /// Convert to sockaddr_un
    pub fn to_sockaddr_un(&self) -> [u8; 110] {
        let mut buf = [0u8; 110];
        buf[0] = 1; // AF_UNIX
        buf[1] = 0;
        let path_bytes = self.path.as_bytes();
        let copy_len = path_bytes.len().min(UNIX_PATH_MAX - 1);
        buf[2..2 + copy_len].copy_from_slice(&path_bytes[..copy_len]);
        buf
    }
}

/// Shared buffer for Unix stream sockets
struct UnixStreamBuffer {
    /// Data waiting to be read
    data: VecDeque<u8>,
    /// Maximum buffer size (default 64KB)
    max_size: usize,
    /// Reader has closed their end
    reader_closed: bool,
    /// Writer has closed their end
    writer_closed: bool,
}

impl UnixStreamBuffer {
    fn new() -> Self {
        Self {
            data: VecDeque::new(),
            max_size: 65536,
            reader_closed: false,
            writer_closed: false,
        }
    }

    fn write(&mut self, buf: &[u8]) -> usize {
        let available = self.max_size.saturating_sub(self.data.len());
        let to_write = buf.len().min(available);
        for &b in &buf[..to_write] {
            self.data.push_back(b);
        }
        to_write
    }

    fn read(&mut self, buf: &mut [u8]) -> usize {
        let to_read = buf.len().min(self.data.len());
        for i in 0..to_read {
            buf[i] = self.data.pop_front().unwrap();
        }
        to_read
    }

    fn available(&self) -> usize {
        self.data.len()
    }

    fn is_eof(&self) -> bool {
        self.writer_closed && self.data.is_empty()
    }
}

/// Unix socket pair connection state
struct UnixConnection {
    /// Buffer for data flowing in one direction (socket A -> socket B)
    buffer_a_to_b: UnixStreamBuffer,
    /// Buffer for data flowing in the other direction (socket B -> socket A)
    buffer_b_to_a: UnixStreamBuffer,
}

impl UnixConnection {
    fn new() -> Self {
        Self {
            buffer_a_to_b: UnixStreamBuffer::new(),
            buffer_b_to_a: UnixStreamBuffer::new(),
        }
    }
}

/// Unix domain socket listener (for servers)
struct UnixListener {
    /// Path this listener is bound to
    path: String,
    /// Pending connections (socket IDs waiting to be accepted)
    pending: VecDeque<u64>,
    /// Maximum pending connections
    backlog: usize,
}

/// Global registry of bound Unix sockets
static UNIX_LISTENERS: IrqSafeMutex<BTreeMap<String, UnixListener>> = IrqSafeMutex::new(BTreeMap::new());

/// Global registry of Unix connections
static UNIX_CONNECTIONS: IrqSafeMutex<BTreeMap<u64, Arc<IrqSafeMutex<UnixConnection>>>> = IrqSafeMutex::new(BTreeMap::new());

/// Next connection ID
static mut NEXT_UNIX_CONN_ID: u64 = 1;

/// Socket interno
pub struct Socket {
    pub id: u64,
    pub domain: SocketDomain,
    pub sock_type: SocketType,
    pub state: SocketState,
    pub local_addr: Option<SocketAddr>,
    pub remote_addr: Option<SocketAddr>,
    /// Para TCP: chave da conexão
    pub tcp_key: Option<tcp::TcpConnKey>,
    /// Para UDP: porta local alocada
    pub udp_port: Option<u16>,
    /// Para Unix domain sockets: endereço local
    pub unix_local_addr: Option<UnixSocketAddr>,
    /// Para Unix domain sockets: conexão ID (shared with peer)
    pub unix_conn_id: Option<u64>,
    /// Para Unix domain sockets: se este é o lado "A" da conexão (para direcionar buffers)
    pub unix_is_side_a: bool,
    /// Opções
    pub nonblocking: bool,
}

impl Socket {
    fn new(id: u64, domain: SocketDomain, sock_type: SocketType) -> Self {
        Self {
            id,
            domain,
            sock_type,
            state: SocketState::Created,
            local_addr: None,
            remote_addr: None,
            tcp_key: None,
            udp_port: None,
            unix_local_addr: None,
            unix_conn_id: None,
            unix_is_side_a: false,
            nonblocking: false,
        }
    }
}

/// Tabela global de sockets
static SOCKETS: IrqSafeMutex<BTreeMap<u64, Socket>> = IrqSafeMutex::new(BTreeMap::new());

/// Próximo ID de socket
static mut NEXT_SOCKET_ID: u64 = 1;

/// Inicializa o subsistema de sockets
pub fn init() {
    // Nada a fazer por enquanto
}

/// Cria um novo socket
pub fn socket(domain: i32, sock_type: i32, _protocol: i32) -> crate::util::KResult<u64> {
    let domain = match domain {
        1 => SocketDomain::Unix,  // AF_UNIX / AF_LOCAL
        2 => SocketDomain::Inet,  // AF_INET
        _ => return Err(crate::util::KError::NotSupported),
    };

    let sock_type = match sock_type & 0xFF {
        1 => SocketType::Stream,   // SOCK_STREAM
        2 => SocketType::Datagram, // SOCK_DGRAM
        3 => SocketType::Raw,      // SOCK_RAW
        _ => return Err(crate::util::KError::NotSupported),
    };

    let id = unsafe {
        let id = NEXT_SOCKET_ID;
        NEXT_SOCKET_ID += 1;
        id
    };

    let sock = Socket::new(id, domain, sock_type);

    let mut sockets = SOCKETS.lock();
    sockets.insert(id, sock);

    Ok(id)
}

/// Bind de um socket a um endereço
pub fn bind(sockfd: u64, addr: &SocketAddr) -> crate::util::KResult<()> {
    let mut sockets = SOCKETS.lock();
    let sock = sockets.get_mut(&sockfd).ok_or(crate::util::KError::Invalid)?;

    if sock.state != SocketState::Created {
        return Err(crate::util::KError::Invalid);
    }

    match sock.sock_type {
        SocketType::Datagram => {
            udp::bind(addr.port)?;
            sock.udp_port = Some(addr.port);
        }
        SocketType::Stream => {
            // TCP bind é feito no listen
        }
        _ => return Err(crate::util::KError::NotSupported),
    }

    sock.local_addr = Some(*addr);
    sock.state = SocketState::Bound;

    Ok(())
}

/// Listen em um socket TCP
pub fn listen(sockfd: u64, _backlog: i32) -> crate::util::KResult<()> {
    let mut sockets = SOCKETS.lock();
    let sock = sockets.get_mut(&sockfd).ok_or(crate::util::KError::Invalid)?;

    if sock.sock_type != SocketType::Stream {
        return Err(crate::util::KError::NotSupported);
    }

    let port = sock.local_addr.ok_or(crate::util::KError::Invalid)?.port;

    tcp::listen(port)?;
    sock.state = SocketState::Listening;

    Ok(())
}

/// Accept em um socket TCP
pub fn accept(sockfd: u64) -> crate::util::KResult<(u64, SocketAddr)> {
    let (port, nonblocking) = {
        let sockets = SOCKETS.lock();
        let sock = sockets.get(&sockfd).ok_or(crate::util::KError::Invalid)?;

        if sock.state != SocketState::Listening {
            return Err(crate::util::KError::Invalid);
        }

        (sock.local_addr.ok_or(crate::util::KError::Invalid)?.port, sock.nonblocking)
    };

    // Timeout: ~30 segundos de espera máxima para blocking sockets
    let max_iterations = if nonblocking { 1 } else { 30_000 };

    // Espera uma conexão
    for _ in 0..max_iterations {
        super::poll();

        if let Some(key) = tcp::accept(port) {
            // Cria novo socket para a conexão
            let new_id = unsafe {
                let id = NEXT_SOCKET_ID;
                NEXT_SOCKET_ID += 1;
                id
            };

            let remote_addr = SocketAddr::new(key.remote_addr, key.remote_port);

            let mut new_sock = Socket::new(new_id, SocketDomain::Inet, SocketType::Stream);
            new_sock.state = SocketState::Connected;
            new_sock.local_addr = Some(SocketAddr::new(key.local_addr, key.local_port));
            new_sock.remote_addr = Some(remote_addr);
            new_sock.tcp_key = Some(key);

            let mut sockets = SOCKETS.lock();
            sockets.insert(new_id, new_sock);

            return Ok((new_id, remote_addr));
        }

        // Yield para outros processos (se não for nonblocking)
        if !nonblocking {
            // Yield CPU briefly
for _ in 0..1000 { core::hint::spin_loop(); }
        }
    }

    // Timeout ou would-block
    Err(crate::util::KError::WouldBlock)
}

/// Connect a um servidor remoto
pub fn connect(sockfd: u64, addr: &SocketAddr) -> crate::util::KResult<()> {
    let mut sockets = SOCKETS.lock();
    let sock = sockets.get_mut(&sockfd).ok_or(crate::util::KError::Invalid)?;

    match sock.sock_type {
        SocketType::Stream => {
            drop(sockets);
            let key = tcp::connect(addr.ip, addr.port)?;

            let mut sockets = SOCKETS.lock();
            let sock = sockets.get_mut(&sockfd).ok_or(crate::util::KError::Invalid)?;
            sock.tcp_key = Some(key);
            sock.remote_addr = Some(*addr);
            sock.local_addr = Some(SocketAddr::new(key.local_addr, key.local_port));
            sock.state = SocketState::Connected;
        }
        SocketType::Datagram => {
            // UDP "connect" apenas salva o endereço remoto
            sock.remote_addr = Some(*addr);
            sock.state = SocketState::Connected;

            // Aloca porta local se não tiver
            if sock.udp_port.is_none() {
                let port = udp::allocate_port();
                sock.udp_port = Some(port);
                sock.local_addr = Some(SocketAddr::any(port));
            }
        }
        _ => return Err(crate::util::KError::NotSupported),
    }

    Ok(())
}

/// Envia dados
pub fn send(sockfd: u64, data: &[u8]) -> crate::util::KResult<usize> {
    let sockets = SOCKETS.lock();
    let sock = sockets.get(&sockfd).ok_or(crate::util::KError::Invalid)?;

    match sock.sock_type {
        SocketType::Stream => {
            let key = sock.tcp_key.ok_or(crate::util::KError::Invalid)?;
            drop(sockets);
            tcp::send(&key, data)
        }
        SocketType::Datagram => {
            let remote = sock.remote_addr.ok_or(crate::util::KError::Invalid)?;
            let port = sock.udp_port.ok_or(crate::util::KError::Invalid)?;
            drop(sockets);
            udp::send(port, remote.ip, remote.port, data)?;
            Ok(data.len())
        }
        _ => Err(crate::util::KError::NotSupported),
    }
}

/// Envia dados para um endereço específico (UDP)
pub fn sendto(sockfd: u64, data: &[u8], addr: &SocketAddr) -> crate::util::KResult<usize> {
    let mut sockets = SOCKETS.lock();
    let sock = sockets.get_mut(&sockfd).ok_or(crate::util::KError::Invalid)?;

    if sock.sock_type != SocketType::Datagram {
        return Err(crate::util::KError::NotSupported);
    }

    // Aloca porta local se necessário
    if sock.udp_port.is_none() {
        let port = udp::allocate_port();
        sock.udp_port = Some(port);
        sock.local_addr = Some(SocketAddr::any(port));
    }

    let port = sock.udp_port.unwrap();
    drop(sockets);

    udp::send(port, addr.ip, addr.port, data)?;
    Ok(data.len())
}

/// Recebe dados
pub fn recv(sockfd: u64, buf: &mut [u8]) -> crate::util::KResult<usize> {
    let (sock_type, key_or_port, nonblocking) = {
        let sockets = SOCKETS.lock();
        let sock = sockets.get(&sockfd).ok_or(crate::util::KError::Invalid)?;
        let nonblocking = sock.nonblocking;

        match sock.sock_type {
            SocketType::Stream => {
                let key = sock.tcp_key.ok_or(crate::util::KError::Invalid)?;
                (SocketType::Stream, Some((key, 0u16)), nonblocking)
            }
            SocketType::Datagram => {
                let port = sock.udp_port.ok_or(crate::util::KError::Invalid)?;
                (SocketType::Datagram, Some((tcp::TcpConnKey {
                    local_addr: super::Ipv4Addr::UNSPECIFIED,
                    local_port: port,
                    remote_addr: super::Ipv4Addr::UNSPECIFIED,
                    remote_port: 0,
                }, port)), nonblocking)
            }
            _ => return Err(crate::util::KError::NotSupported),
        }
    };

    // Timeout: ~30 segundos de espera máxima para blocking sockets
    let max_iterations = if nonblocking { 1 } else { 30_000 };

    match sock_type {
        SocketType::Stream => {
            let (key, _) = key_or_port.unwrap();
            for _ in 0..max_iterations {
                super::poll();

                let result = tcp::recv(&key, buf)?;
                if result > 0 {
                    return Ok(result);
                }

                if nonblocking {
                    return Err(crate::util::KError::WouldBlock);
                }
                // Yield CPU briefly
for _ in 0..1000 { core::hint::spin_loop(); }
            }
            Err(crate::util::KError::WouldBlock)
        }
        SocketType::Datagram => {
            let (_, port) = key_or_port.unwrap();

            // Espera dados com timeout
            for _ in 0..max_iterations {
                super::poll();

                if let Some(dgram) = udp::recv(port) {
                    let len = dgram.data.len().min(buf.len());
                    buf[..len].copy_from_slice(&dgram.data[..len]);
                    return Ok(len);
                }

                if nonblocking {
                    return Err(crate::util::KError::WouldBlock);
                }
                // Yield CPU briefly
for _ in 0..1000 { core::hint::spin_loop(); }
            }
            Err(crate::util::KError::WouldBlock)
        }
        _ => Err(crate::util::KError::NotSupported),
    }
}

/// Recebe dados com endereço de origem (UDP)
pub fn recvfrom(sockfd: u64, buf: &mut [u8]) -> crate::util::KResult<(usize, SocketAddr)> {
    let (port, nonblocking) = {
        let sockets = SOCKETS.lock();
        let sock = sockets.get(&sockfd).ok_or(crate::util::KError::Invalid)?;

        if sock.sock_type != SocketType::Datagram {
            return Err(crate::util::KError::NotSupported);
        }

        (sock.udp_port.ok_or(crate::util::KError::Invalid)?, sock.nonblocking)
    };

    // Timeout: ~30 segundos de espera máxima para blocking sockets
    let max_iterations = if nonblocking { 1 } else { 30_000 };

    for _ in 0..max_iterations {
        super::poll();

        if let Some(dgram) = udp::recv(port) {
            let len = dgram.data.len().min(buf.len());
            buf[..len].copy_from_slice(&dgram.data[..len]);
            let addr = SocketAddr::new(dgram.src_addr, dgram.src_port);
            return Ok((len, addr));
        }

        if nonblocking {
            return Err(crate::util::KError::WouldBlock);
        }
        // Yield CPU briefly
for _ in 0..1000 { core::hint::spin_loop(); }
    }

    Err(crate::util::KError::WouldBlock)
}

/// Fecha um socket
pub fn close(sockfd: u64) -> crate::util::KResult<()> {
    // First, handle Unix socket cleanup (before removing from SOCKETS)
    unix_close(sockfd);

    let mut sockets = SOCKETS.lock();
    let sock = sockets.remove(&sockfd).ok_or(crate::util::KError::Invalid)?;

    match sock.domain {
        SocketDomain::Unix => {
            // Already handled by unix_close() above
        }
        SocketDomain::Inet | SocketDomain::Inet6 => {
            match sock.sock_type {
                SocketType::Stream => {
                    if let Some(key) = sock.tcp_key {
                        drop(sockets);
                        let _ = tcp::close(&key);
                    }
                }
                SocketType::Datagram => {
                    if let Some(port) = sock.udp_port {
                        udp::unbind(port);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// Obtém o endereço local
pub fn getsockname(sockfd: u64) -> crate::util::KResult<SocketAddr> {
    let sockets = SOCKETS.lock();
    let sock = sockets.get(&sockfd).ok_or(crate::util::KError::Invalid)?;
    sock.local_addr.ok_or(crate::util::KError::Invalid)
}

/// Obtém o endereço remoto
pub fn getpeername(sockfd: u64) -> crate::util::KResult<SocketAddr> {
    let sockets = SOCKETS.lock();
    let sock = sockets.get(&sockfd).ok_or(crate::util::KError::Invalid)?;
    sock.remote_addr.ok_or(crate::util::KError::Invalid)
}

/// Seta opções do socket
pub fn setsockopt(sockfd: u64, _level: i32, _optname: i32, _optval: &[u8]) -> crate::util::KResult<()> {
    let sockets = SOCKETS.lock();
    let _sock = sockets.get(&sockfd).ok_or(crate::util::KError::Invalid)?;
    // TODO: implementar opções específicas
    Ok(())
}

/// Obtém opções do socket
pub fn getsockopt(sockfd: u64, _level: i32, _optname: i32, buf: &mut [u8]) -> crate::util::KResult<usize> {
    let sockets = SOCKETS.lock();
    let _sock = sockets.get(&sockfd).ok_or(crate::util::KError::Invalid)?;
    // TODO: implementar opções específicas
    if buf.len() >= 4 {
        buf[..4].copy_from_slice(&[0, 0, 0, 0]);
        Ok(4)
    } else {
        Ok(0)
    }
}

/// Shutdown parcial do socket
pub fn shutdown(sockfd: u64, how: i32) -> crate::util::KResult<()> {
    let sockets = SOCKETS.lock();
    let sock = sockets.get(&sockfd).ok_or(crate::util::KError::Invalid)?;

    if sock.sock_type == SocketType::Stream {
        if let Some(key) = sock.tcp_key {
            if how == 1 || how == 2 { // SHUT_WR or SHUT_RDWR
                drop(sockets);
                let _ = tcp::close(&key);
            }
        }
    }

    Ok(())
}

/// Verifica se o socket tem dados prontos para leitura (para poll/select)
pub fn poll_read(sockfd: u64) -> bool {
    let sockets = SOCKETS.lock();
    let sock = match sockets.get(&sockfd) {
        Some(s) => s,
        None => return false,
    };

    // Check domain first
    if sock.domain == SocketDomain::Unix {
        drop(sockets);
        return unix_poll_read(sockfd);
    }

    match sock.sock_type {
        SocketType::Stream => {
            if let Some(ref key) = sock.tcp_key {
                // Verifica se há dados no buffer de recepção TCP
                tcp::has_data(key)
            } else if sock.state == SocketState::Listening {
                // Socket de escuta: verifica se há conexões pendentes
                if let Some(addr) = sock.local_addr {
                    tcp::has_pending_connection(addr.port)
                } else {
                    false
                }
            } else {
                false
            }
        }
        SocketType::Datagram => {
            if let Some(port) = sock.udp_port {
                udp::has_data(port)
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Verifica se o socket está pronto para escrita (para poll/select)
pub fn poll_write(sockfd: u64) -> bool {
    let sockets = SOCKETS.lock();
    let sock = match sockets.get(&sockfd) {
        Some(s) => s,
        None => return false,
    };

    // Check domain first
    if sock.domain == SocketDomain::Unix {
        // Unix sockets are always ready for writing (unless buffer is full)
        if sock.state == SocketState::Connected {
            if let Some(conn_id) = sock.unix_conn_id {
                let is_side_a = sock.unix_is_side_a;
                drop(sockets);
                let connections = UNIX_CONNECTIONS.lock();
                if let Some(conn_arc) = connections.get(&conn_id) {
                    let conn = conn_arc.lock();
                    let buffer = if is_side_a {
                        &conn.buffer_a_to_b
                    } else {
                        &conn.buffer_b_to_a
                    };
                    return !buffer.reader_closed && buffer.available() < buffer.max_size;
                }
            }
        }
        return false;
    }

    match sock.sock_type {
        SocketType::Stream => {
            if let Some(ref key) = sock.tcp_key {
                // Verifica se podemos enviar (conexão estabelecida e buffer não cheio)
                tcp::can_send(key)
            } else {
                false
            }
        }
        SocketType::Datagram => {
            // UDP sempre pode enviar
            sock.udp_port.is_some() || sock.state == SocketState::Created
        }
        _ => false,
    }
}

/// Returns the number of bytes available to read from the socket (for FIONREAD ioctl)
pub fn available(sockfd: u64) -> usize {
    let sockets = SOCKETS.lock();
    let sock = match sockets.get(&sockfd) {
        Some(s) => s,
        None => return 0,
    };

    // Extract the info we need while holding the lock
    let domain = sock.domain;
    let state = sock.state;
    let sock_type = sock.sock_type;
    let conn_id = sock.unix_conn_id;
    let is_side_a = sock.unix_is_side_a;
    let tcp_key = sock.tcp_key.clone();
    let udp_port = sock.udp_port;

    // Drop the lock before accessing other resources
    drop(sockets);

    // Check domain first
    if domain == SocketDomain::Unix {
        if state == SocketState::Connected {
            if let Some(cid) = conn_id {
                let connections = UNIX_CONNECTIONS.lock();
                if let Some(conn_arc) = connections.get(&cid) {
                    let conn = conn_arc.lock();
                    let buffer = if is_side_a {
                        &conn.buffer_b_to_a // A reads from B's buffer
                    } else {
                        &conn.buffer_a_to_b // B reads from A's buffer
                    };
                    return buffer.available();
                }
            }
        }
        return 0;
    }

    match sock_type {
        SocketType::Stream => {
            if let Some(key) = tcp_key {
                tcp::available_data(&key)
            } else {
                0
            }
        }
        SocketType::Datagram => {
            if let Some(port) = udp_port {
                udp::available_data(port)
            } else {
                0
            }
        }
        _ => 0,
    }
}

// ==================== Unix Domain Socket Operations ====================

/// Bind a Unix domain socket to a path
pub fn unix_bind(sockfd: u64, addr: &UnixSocketAddr) -> crate::util::KResult<()> {
    let mut sockets = SOCKETS.lock();
    let sock = sockets.get_mut(&sockfd).ok_or(crate::util::KError::Invalid)?;

    if sock.domain != SocketDomain::Unix {
        return Err(crate::util::KError::Invalid);
    }

    if sock.state != SocketState::Created {
        return Err(crate::util::KError::Invalid);
    }

    // Check if path is already bound
    {
        let listeners = UNIX_LISTENERS.lock();
        if listeners.contains_key(&addr.path) {
            return Err(crate::util::KError::AlreadyExists);
        }
    }

    sock.unix_local_addr = Some(addr.clone());
    sock.state = SocketState::Bound;

    Ok(())
}

/// Listen on a Unix domain socket
pub fn unix_listen(sockfd: u64, backlog: i32) -> crate::util::KResult<()> {
    let mut sockets = SOCKETS.lock();
    let sock = sockets.get_mut(&sockfd).ok_or(crate::util::KError::Invalid)?;

    if sock.domain != SocketDomain::Unix {
        return Err(crate::util::KError::Invalid);
    }

    if sock.sock_type != SocketType::Stream {
        return Err(crate::util::KError::NotSupported);
    }

    let addr = sock.unix_local_addr.as_ref().ok_or(crate::util::KError::Invalid)?;

    // Create listener entry
    let mut listeners = UNIX_LISTENERS.lock();

    // Check again (someone might have bound meanwhile)
    if listeners.contains_key(&addr.path) {
        return Err(crate::util::KError::AlreadyExists);
    }

    listeners.insert(addr.path.clone(), UnixListener {
        path: addr.path.clone(),
        pending: VecDeque::new(),
        backlog: backlog.max(1) as usize,
    });

    sock.state = SocketState::Listening;

    Ok(())
}

/// Accept a connection on a Unix domain socket
pub fn unix_accept(sockfd: u64) -> crate::util::KResult<(u64, UnixSocketAddr)> {
    let (path, nonblocking) = {
        let sockets = SOCKETS.lock();
        let sock = sockets.get(&sockfd).ok_or(crate::util::KError::Invalid)?;

        if sock.domain != SocketDomain::Unix || sock.state != SocketState::Listening {
            return Err(crate::util::KError::Invalid);
        }

        let addr = sock.unix_local_addr.as_ref().ok_or(crate::util::KError::Invalid)?;
        (addr.path.clone(), sock.nonblocking)
    };

    let max_iterations = if nonblocking { 1 } else { 30_000 };

    for _ in 0..max_iterations {
        // Check for pending connections
        {
            let mut listeners = UNIX_LISTENERS.lock();
            if let Some(listener) = listeners.get_mut(&path) {
                if let Some(client_sock_id) = listener.pending.pop_front() {
                    // Create a new socket for the accepted connection
                    let new_id = unsafe {
                        let id = NEXT_SOCKET_ID;
                        NEXT_SOCKET_ID += 1;
                        id
                    };

                    // Get connection from client socket
                    let conn_id = {
                        let sockets = SOCKETS.lock();
                        let client_sock = sockets.get(&client_sock_id).ok_or(crate::util::KError::Invalid)?;
                        client_sock.unix_conn_id.ok_or(crate::util::KError::Invalid)?
                    };

                    // Create server-side socket
                    let mut new_sock = Socket::new(new_id, SocketDomain::Unix, SocketType::Stream);
                    new_sock.state = SocketState::Connected;
                    new_sock.unix_local_addr = Some(UnixSocketAddr::new(&path));
                    new_sock.unix_conn_id = Some(conn_id);
                    new_sock.unix_is_side_a = false; // Server is side B

                    let mut sockets = SOCKETS.lock();
                    sockets.insert(new_id, new_sock);

                    return Ok((new_id, UnixSocketAddr::unnamed()));
                }
            }
        }

        if nonblocking {
            return Err(crate::util::KError::WouldBlock);
        }

        // Yield CPU briefly
        for _ in 0..1000 { core::hint::spin_loop(); }
    }

    Err(crate::util::KError::WouldBlock)
}

/// Connect to a Unix domain socket
pub fn unix_connect(sockfd: u64, addr: &UnixSocketAddr) -> crate::util::KResult<()> {
    // First, check if there's a listener at the target path
    {
        let mut listeners = UNIX_LISTENERS.lock();
        let listener = listeners.get_mut(&addr.path).ok_or(crate::util::KError::NotFound)?;

        if listener.pending.len() >= listener.backlog {
            return Err(crate::util::KError::WouldBlock);
        }

        // Create a new connection
        let conn_id = unsafe {
            let id = NEXT_UNIX_CONN_ID;
            NEXT_UNIX_CONN_ID += 1;
            id
        };

        let conn = Arc::new(IrqSafeMutex::new(UnixConnection::new()));

        {
            let mut connections = UNIX_CONNECTIONS.lock();
            connections.insert(conn_id, conn);
        }

        // Update the connecting socket
        {
            let mut sockets = SOCKETS.lock();
            let sock = sockets.get_mut(&sockfd).ok_or(crate::util::KError::Invalid)?;

            if sock.domain != SocketDomain::Unix {
                return Err(crate::util::KError::Invalid);
            }

            sock.state = SocketState::Connected;
            sock.unix_conn_id = Some(conn_id);
            sock.unix_is_side_a = true; // Client is side A
        }

        // Add to listener's pending queue
        listener.pending.push_back(sockfd);
    }

    Ok(())
}

/// Send data on a Unix domain socket
pub fn unix_send(sockfd: u64, data: &[u8]) -> crate::util::KResult<usize> {
    let (conn_id, is_side_a, nonblocking) = {
        let sockets = SOCKETS.lock();
        let sock = sockets.get(&sockfd).ok_or(crate::util::KError::Invalid)?;

        if sock.domain != SocketDomain::Unix || sock.state != SocketState::Connected {
            return Err(crate::util::KError::Invalid);
        }

        let conn_id = sock.unix_conn_id.ok_or(crate::util::KError::Invalid)?;
        (conn_id, sock.unix_is_side_a, sock.nonblocking)
    };

    let connections = UNIX_CONNECTIONS.lock();
    let conn_arc = connections.get(&conn_id).ok_or(crate::util::KError::Invalid)?.clone();
    drop(connections);

    let max_iterations = if nonblocking { 1 } else { 30_000 };

    for _ in 0..max_iterations {
        let mut conn = conn_arc.lock();

        // Select the correct buffer based on which side we are
        let buffer = if is_side_a {
            &mut conn.buffer_a_to_b
        } else {
            &mut conn.buffer_b_to_a
        };

        // Check if the reader has closed
        if buffer.reader_closed {
            return Err(crate::util::KError::BrokenPipe);
        }

        let written = buffer.write(data);
        if written > 0 {
            return Ok(written);
        }

        drop(conn);

        if nonblocking {
            return Err(crate::util::KError::WouldBlock);
        }

        // Yield CPU briefly
        for _ in 0..1000 { core::hint::spin_loop(); }
    }

    Err(crate::util::KError::WouldBlock)
}

/// Receive data from a Unix domain socket
pub fn unix_recv(sockfd: u64, buf: &mut [u8]) -> crate::util::KResult<usize> {
    let (conn_id, is_side_a, nonblocking) = {
        let sockets = SOCKETS.lock();
        let sock = sockets.get(&sockfd).ok_or(crate::util::KError::Invalid)?;

        if sock.domain != SocketDomain::Unix || sock.state != SocketState::Connected {
            return Err(crate::util::KError::Invalid);
        }

        let conn_id = sock.unix_conn_id.ok_or(crate::util::KError::Invalid)?;
        (conn_id, sock.unix_is_side_a, sock.nonblocking)
    };

    let connections = UNIX_CONNECTIONS.lock();
    let conn_arc = connections.get(&conn_id).ok_or(crate::util::KError::Invalid)?.clone();
    drop(connections);

    let max_iterations = if nonblocking { 1 } else { 30_000 };

    for _ in 0..max_iterations {
        let mut conn = conn_arc.lock();

        // Select the correct buffer based on which side we are (reversed from send)
        let buffer = if is_side_a {
            &mut conn.buffer_b_to_a
        } else {
            &mut conn.buffer_a_to_b
        };

        // Check for EOF
        if buffer.is_eof() {
            return Ok(0); // EOF
        }

        let read = buffer.read(buf);
        if read > 0 {
            return Ok(read);
        }

        drop(conn);

        if nonblocking {
            return Err(crate::util::KError::WouldBlock);
        }

        // Yield CPU briefly
        for _ in 0..1000 { core::hint::spin_loop(); }
    }

    Err(crate::util::KError::WouldBlock)
}

/// Close a Unix domain socket
pub fn unix_close(sockfd: u64) {
    let (conn_id, is_side_a, path) = {
        let sockets = SOCKETS.lock();
        if let Some(sock) = sockets.get(&sockfd) {
            if sock.domain == SocketDomain::Unix {
                (sock.unix_conn_id, sock.unix_is_side_a, sock.unix_local_addr.as_ref().map(|a| a.path.clone()))
            } else {
                (None, false, None)
            }
        } else {
            (None, false, None)
        }
    };

    // Mark our side as closed in the connection
    if let Some(conn_id) = conn_id {
        let connections = UNIX_CONNECTIONS.lock();
        if let Some(conn_arc) = connections.get(&conn_id) {
            let mut conn = conn_arc.lock();
            if is_side_a {
                conn.buffer_a_to_b.writer_closed = true;
                conn.buffer_b_to_a.reader_closed = true;
            } else {
                conn.buffer_b_to_a.writer_closed = true;
                conn.buffer_a_to_b.reader_closed = true;
            }
        }
    }

    // Remove from listeners if this was a listening socket
    if let Some(path) = path {
        let mut listeners = UNIX_LISTENERS.lock();
        listeners.remove(&path);
    }
}

/// Get the domain of a socket
pub fn get_domain(sockfd: u64) -> Option<SocketDomain> {
    let sockets = SOCKETS.lock();
    sockets.get(&sockfd).map(|s| s.domain)
}

/// Check if a Unix socket has data available
pub fn unix_poll_read(sockfd: u64) -> bool {
    let (conn_id, is_side_a) = {
        let sockets = SOCKETS.lock();
        if let Some(sock) = sockets.get(&sockfd) {
            if sock.domain == SocketDomain::Unix && sock.state == SocketState::Connected {
                if let Some(conn_id) = sock.unix_conn_id {
                    (Some(conn_id), sock.unix_is_side_a)
                } else {
                    (None, false)
                }
            } else if sock.domain == SocketDomain::Unix && sock.state == SocketState::Listening {
                // Check for pending connections
                if let Some(ref addr) = sock.unix_local_addr {
                    let listeners = UNIX_LISTENERS.lock();
                    if let Some(listener) = listeners.get(&addr.path) {
                        return !listener.pending.is_empty();
                    }
                }
                return false;
            } else {
                (None, false)
            }
        } else {
            (None, false)
        }
    };

    if let Some(conn_id) = conn_id {
        let connections = UNIX_CONNECTIONS.lock();
        if let Some(conn_arc) = connections.get(&conn_id) {
            let conn = conn_arc.lock();
            let buffer = if is_side_a {
                &conn.buffer_b_to_a
            } else {
                &conn.buffer_a_to_b
            };
            return buffer.available() > 0 || buffer.is_eof();
        }
    }

    false
}
