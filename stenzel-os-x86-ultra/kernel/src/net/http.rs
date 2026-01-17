//! HTTP/1.1 Client
//!
//! Simple HTTP client for making GET and POST requests.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::format;

use super::tcp::{self, TcpConnKey};
use super::dns;
use crate::util::{KError, KResult};

/// HTTP Methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Head,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Head => "HEAD",
        }
    }
}

/// HTTP Response
#[derive(Debug)]
pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Get a header value by name (case-insensitive)
    pub fn get_header(&self, name: &str) -> Option<&str> {
        let name_lower = name.to_ascii_lowercase();
        for (key, value) in &self.headers {
            if key.to_ascii_lowercase() == name_lower {
                return Some(value.as_str());
            }
        }
        None
    }

    /// Get Content-Length header
    pub fn content_length(&self) -> Option<usize> {
        self.get_header("Content-Length")
            .and_then(|s| s.parse().ok())
    }

    /// Get body as string (assuming UTF-8)
    pub fn body_as_string(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Parsed URL
#[derive(Debug)]
pub struct Url {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
}

impl Url {
    /// Parse a URL string
    pub fn parse(url: &str) -> KResult<Self> {
        let mut rest = url;

        // Parse scheme
        let scheme = if rest.starts_with("http://") {
            rest = &rest[7..];
            "http".into()
        } else if rest.starts_with("https://") {
            rest = &rest[8..];
            "https".into()
        } else {
            "http".into()
        };

        // Parse host and port
        let (host_port, path) = if let Some(idx) = rest.find('/') {
            (&rest[..idx], &rest[idx..])
        } else {
            (rest, "/")
        };

        let (host, port) = if let Some(idx) = host_port.rfind(':') {
            let h = &host_port[..idx];
            let p = host_port[idx + 1..].parse().unwrap_or(80);
            (h.into(), p)
        } else {
            let default_port = if scheme == "https" { 443 } else { 80 };
            (host_port.into(), default_port)
        };

        Ok(Self {
            scheme,
            host,
            port,
            path: path.into(),
        })
    }
}

/// HTTP Client
pub struct HttpClient {
    conn: Option<TcpConnKey>,
    host: String,
    port: u16,
}

impl HttpClient {
    /// Create a new HTTP client
    pub fn new() -> Self {
        Self {
            conn: None,
            host: String::new(),
            port: 80,
        }
    }

    /// Connect to a server
    pub fn connect(&mut self, host: &str, port: u16) -> KResult<()> {
        // Resolve hostname to IP
        let ip = dns::resolve(host)?;

        // Connect via TCP
        let conn = tcp::connect(ip, port)?;

        self.conn = Some(conn);
        self.host = host.into();
        self.port = port;

        Ok(())
    }

    /// Disconnect from server
    pub fn disconnect(&mut self) {
        if let Some(ref key) = self.conn {
            let _ = tcp::close(key);
        }
        self.conn = None;
    }

    /// Send an HTTP request and receive response
    pub fn request(
        &mut self,
        method: HttpMethod,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> KResult<HttpResponse> {
        let conn = self.conn.as_ref().ok_or(KError::NotSupported)?;

        // Build request
        let mut request = format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: keep-alive\r\nUser-Agent: StenzelOS/1.0\r\n",
            method.as_str(),
            path,
            self.host
        );

        // Add custom headers
        for (name, value) in headers {
            request.push_str(&format!("{}: {}\r\n", name, value));
        }

        // Add Content-Length for body
        if let Some(body) = body {
            request.push_str(&format!("Content-Length: {}\r\n", body.len()));
        }

        // End headers
        request.push_str("\r\n");

        // Send request header
        tcp::send(conn, request.as_bytes())?;

        // Send body if present
        if let Some(body) = body {
            tcp::send(conn, body)?;
        }

        // Receive response
        self.receive_response(conn)
    }

    /// Receive and parse HTTP response
    fn receive_response(&self, conn: &TcpConnKey) -> KResult<HttpResponse> {
        let mut response_data = Vec::with_capacity(4096);
        let mut buf = [0u8; 1024];

        // Receive data with timeout
        let timeout = 10000; // 10 seconds worth of iterations
        let mut iterations = 0;

        loop {
            // Poll network
            super::poll();

            // Try to read data
            let n = tcp::recv(conn, &mut buf)?;
            if n > 0 {
                response_data.extend_from_slice(&buf[..n]);
                iterations = 0; // Reset timeout on data received

                // Check if we have complete headers
                if let Some(header_end) = find_header_end(&response_data) {
                    // Parse headers
                    let (status_code, status_text, headers) = parse_headers(&response_data[..header_end])?;

                    // Calculate body start
                    let body_start = header_end + 4; // Skip \r\n\r\n

                    // Determine how much body to read
                    let content_length = headers.iter()
                        .find(|(k, _)| k.to_ascii_lowercase() == "content-length")
                        .and_then(|(_, v)| v.parse().ok())
                        .unwrap_or(0);

                    let is_chunked = headers.iter()
                        .any(|(k, v)| k.to_ascii_lowercase() == "transfer-encoding" &&
                                      v.to_ascii_lowercase().contains("chunked"));

                    // Read body
                    let body = if is_chunked {
                        // Read chunked body
                        read_chunked_body(&response_data[body_start..], conn)?
                    } else if content_length > 0 {
                        // Read fixed-length body
                        read_fixed_body(&response_data[body_start..], content_length, conn)?
                    } else {
                        // No body
                        Vec::new()
                    };

                    return Ok(HttpResponse {
                        status_code,
                        status_text,
                        headers,
                        body,
                    });
                }
            }

            iterations += 1;
            if iterations > timeout {
                break;
            }

            // Small delay
            for _ in 0..1000 {
                core::hint::spin_loop();
            }
        }

        Err(KError::NotSupported) // Timeout
    }

    /// Perform a GET request
    pub fn get(&mut self, url: &str) -> KResult<HttpResponse> {
        let parsed = Url::parse(url)?;

        // Connect if needed
        if self.conn.is_none() || self.host != parsed.host || self.port != parsed.port {
            self.disconnect();
            self.connect(&parsed.host, parsed.port)?;
        }

        self.request(HttpMethod::Get, &parsed.path, &[], None)
    }

    /// Perform a POST request
    pub fn post(&mut self, url: &str, content_type: &str, body: &[u8]) -> KResult<HttpResponse> {
        let parsed = Url::parse(url)?;

        // Connect if needed
        if self.conn.is_none() || self.host != parsed.host || self.port != parsed.port {
            self.disconnect();
            self.connect(&parsed.host, parsed.port)?;
        }

        self.request(
            HttpMethod::Post,
            &parsed.path,
            &[("Content-Type", content_type)],
            Some(body),
        )
    }
}

impl Drop for HttpClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}

/// Find the end of HTTP headers (\r\n\r\n)
fn find_header_end(data: &[u8]) -> Option<usize> {
    for i in 0..data.len().saturating_sub(3) {
        if &data[i..i + 4] == b"\r\n\r\n" {
            return Some(i);
        }
    }
    None
}

/// Parse HTTP response headers
fn parse_headers(data: &[u8]) -> KResult<(u16, String, Vec<(String, String)>)> {
    let text = core::str::from_utf8(data).map_err(|_| KError::Invalid)?;
    let mut lines = text.lines();

    // Parse status line: HTTP/1.1 200 OK
    let status_line = lines.next().ok_or(KError::Invalid)?;
    let mut parts = status_line.split_whitespace();

    let _version = parts.next().ok_or(KError::Invalid)?;
    let status_code: u16 = parts.next()
        .ok_or(KError::Invalid)?
        .parse()
        .map_err(|_| KError::Invalid)?;
    let status_text: String = parts.collect::<Vec<_>>().join(" ");

    // Parse headers
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some(idx) = line.find(':') {
            let name = line[..idx].trim().to_string();
            let value = line[idx + 1..].trim().to_string();
            headers.push((name, value));
        }
    }

    Ok((status_code, status_text, headers))
}

/// Read fixed-length body
fn read_fixed_body(initial: &[u8], length: usize, conn: &TcpConnKey) -> KResult<Vec<u8>> {
    let mut body = initial.to_vec();

    // Read remaining bytes
    let mut buf = [0u8; 1024];
    let timeout = 5000;
    let mut iterations = 0;

    while body.len() < length {
        super::poll();

        let n = tcp::recv(conn, &mut buf)?;
        if n > 0 {
            body.extend_from_slice(&buf[..n]);
            iterations = 0;
        }

        iterations += 1;
        if iterations > timeout {
            break;
        }

        for _ in 0..1000 {
            core::hint::spin_loop();
        }
    }

    // Truncate to exact length
    body.truncate(length);
    Ok(body)
}

/// Read chunked transfer-encoded body
fn read_chunked_body(initial: &[u8], conn: &TcpConnKey) -> KResult<Vec<u8>> {
    let mut data = initial.to_vec();
    let mut body = Vec::new();
    let mut buf = [0u8; 1024];
    let timeout = 5000;

    loop {
        // Read more data if needed
        let mut iterations = 0;
        while !has_complete_chunk(&data) && iterations < timeout {
            super::poll();

            let n = tcp::recv(conn, &mut buf)?;
            if n > 0 {
                data.extend_from_slice(&buf[..n]);
                iterations = 0;
            }

            iterations += 1;
            for _ in 0..1000 {
                core::hint::spin_loop();
            }
        }

        // Parse chunk size
        let line_end = data.iter().position(|&b| b == b'\n').ok_or(KError::Invalid)?;
        let size_line = core::str::from_utf8(&data[..line_end])
            .map_err(|_| KError::Invalid)?
            .trim();

        // Parse hex chunk size
        let chunk_size = usize::from_str_radix(size_line.trim_end_matches('\r'), 16)
            .map_err(|_| KError::Invalid)?;

        if chunk_size == 0 {
            break; // End of chunked data
        }

        // Extract chunk data
        let chunk_start = line_end + 1;
        let chunk_end = chunk_start + chunk_size;

        if data.len() < chunk_end + 2 {
            // Need more data
            continue;
        }

        body.extend_from_slice(&data[chunk_start..chunk_end]);

        // Remove processed chunk (including trailing \r\n)
        data.drain(..chunk_end + 2);
    }

    Ok(body)
}

/// Check if we have a complete chunk in the buffer
fn has_complete_chunk(data: &[u8]) -> bool {
    // Find the chunk size line
    if let Some(line_end) = data.iter().position(|&b| b == b'\n') {
        if let Ok(size_str) = core::str::from_utf8(&data[..line_end]) {
            if let Ok(size) = usize::from_str_radix(size_str.trim(), 16) {
                return data.len() >= line_end + 1 + size + 2;
            }
        }
    }
    false
}

// ============================================================================
// Convenience functions
// ============================================================================

/// Perform a simple HTTP GET request
pub fn get(url: &str) -> KResult<HttpResponse> {
    let mut client = HttpClient::new();
    client.get(url)
}

/// Perform a simple HTTP POST request
pub fn post(url: &str, content_type: &str, body: &[u8]) -> KResult<HttpResponse> {
    let mut client = HttpClient::new();
    client.post(url, content_type, body)
}

/// Download a URL and return the body as bytes
pub fn download(url: &str) -> KResult<Vec<u8>> {
    let response = get(url)?;
    if response.status_code >= 200 && response.status_code < 300 {
        Ok(response.body)
    } else {
        Err(KError::NotSupported)
    }
}

/// Download a URL and return the body as a string
pub fn download_string(url: &str) -> KResult<String> {
    let response = get(url)?;
    if response.status_code >= 200 && response.status_code < 300 {
        Ok(response.body_as_string())
    } else {
        Err(KError::NotSupported)
    }
}

/// Initialize HTTP subsystem
pub fn init() {
    crate::kprintln!("http: initialized");
}
