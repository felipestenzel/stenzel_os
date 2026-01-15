//! Socket API para userspace

#![allow(dead_code)]

use alloc::collections::BTreeMap;

use super::{Ipv4Addr, tcp, udp};
use crate::sync::IrqSafeMutex;

/// Tipos de socket
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Stream,     // TCP
    Datagram,   // UDP
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
    let mut sockets = SOCKETS.lock();
    let sock = sockets.remove(&sockfd).ok_or(crate::util::KError::Invalid)?;

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
