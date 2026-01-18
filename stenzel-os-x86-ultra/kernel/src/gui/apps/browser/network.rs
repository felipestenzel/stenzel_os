//! Browser Network Module
//!
//! HTTP client for fetching web resources.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

/// HTTP request
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// Request method
    pub method: HttpMethod,
    /// URL
    pub url: Url,
    /// Headers
    pub headers: BTreeMap<String, String>,
    /// Body
    pub body: Option<Vec<u8>>,
    /// Timeout in milliseconds
    pub timeout: u32,
    /// Follow redirects
    pub follow_redirects: bool,
    /// Max redirects
    pub max_redirects: u8,
}

impl HttpRequest {
    /// Create GET request
    pub fn get(url: &str) -> Result<Self, UrlParseError> {
        let parsed_url = Url::parse(url)?;
        Ok(Self {
            method: HttpMethod::Get,
            url: parsed_url,
            headers: Self::default_headers(),
            body: None,
            timeout: 30000,
            follow_redirects: true,
            max_redirects: 5,
        })
    }

    /// Create POST request
    pub fn post(url: &str, body: Vec<u8>) -> Result<Self, UrlParseError> {
        let parsed_url = Url::parse(url)?;
        Ok(Self {
            method: HttpMethod::Post,
            url: parsed_url,
            headers: Self::default_headers(),
            body: Some(body),
            timeout: 30000,
            follow_redirects: true,
            max_redirects: 5,
        })
    }

    /// Add header
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.insert(String::from(name), String::from(value));
        self
    }

    /// Set timeout
    pub fn timeout(mut self, ms: u32) -> Self {
        self.timeout = ms;
        self
    }

    fn default_headers() -> BTreeMap<String, String> {
        let mut headers = BTreeMap::new();
        headers.insert(String::from("User-Agent"), String::from("StenzelOS-Browser/1.0"));
        headers.insert(String::from("Accept"), String::from("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"));
        headers.insert(String::from("Accept-Language"), String::from("en-US,en;q=0.5"));
        headers.insert(String::from("Accept-Encoding"), String::from("identity"));
        headers.insert(String::from("Connection"), String::from("keep-alive"));
        headers
    }
}

/// HTTP method
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Options,
    Patch,
}

impl HttpMethod {
    /// Get method string
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
            HttpMethod::Patch => "PATCH",
        }
    }
}

/// HTTP response
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// Status code
    pub status: u16,
    /// Status text
    pub status_text: String,
    /// Headers
    pub headers: BTreeMap<String, String>,
    /// Body
    pub body: Vec<u8>,
    /// Final URL (after redirects)
    pub url: String,
}

impl HttpResponse {
    /// Check if response is success (2xx)
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// Check if response is redirect (3xx)
    pub fn is_redirect(&self) -> bool {
        self.status >= 300 && self.status < 400
    }

    /// Get body as string
    pub fn text(&self) -> Result<String, core::str::Utf8Error> {
        core::str::from_utf8(&self.body).map(String::from)
    }

    /// Get content type
    pub fn content_type(&self) -> Option<&str> {
        self.headers.get("content-type").map(|s| s.as_str())
    }

    /// Get content length
    pub fn content_length(&self) -> Option<usize> {
        self.headers.get("content-length").and_then(|s| s.parse().ok())
    }

    /// Get header
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_lowercase()).map(|s| s.as_str())
    }
}

/// URL
#[derive(Debug, Clone)]
pub struct Url {
    /// Scheme (http, https)
    pub scheme: String,
    /// Host
    pub host: String,
    /// Port
    pub port: u16,
    /// Path
    pub path: String,
    /// Query string
    pub query: Option<String>,
    /// Fragment
    pub fragment: Option<String>,
    /// Username (for basic auth)
    pub username: Option<String>,
    /// Password (for basic auth)
    pub password: Option<String>,
}

impl Url {
    /// Parse URL from string
    pub fn parse(url: &str) -> Result<Self, UrlParseError> {
        let url = url.trim();

        // Extract scheme
        let (scheme, rest) = if let Some(pos) = url.find("://") {
            (url[..pos].to_lowercase(), &url[pos + 3..])
        } else {
            // Default to https
            (String::from("https"), url)
        };

        // Validate scheme
        if scheme != "http" && scheme != "https" {
            return Err(UrlParseError::InvalidScheme);
        }

        // Extract fragment
        let (rest, fragment) = if let Some(pos) = rest.find('#') {
            (&rest[..pos], Some(String::from(&rest[pos + 1..])))
        } else {
            (rest, None)
        };

        // Extract query
        let (rest, query) = if let Some(pos) = rest.find('?') {
            (&rest[..pos], Some(String::from(&rest[pos + 1..])))
        } else {
            (rest, None)
        };

        // Extract path
        let (authority, path) = if let Some(pos) = rest.find('/') {
            (&rest[..pos], String::from(&rest[pos..]))
        } else {
            (rest, String::from("/"))
        };

        // Extract userinfo
        let (userinfo, host_port) = if let Some(pos) = authority.find('@') {
            (Some(&authority[..pos]), &authority[pos + 1..])
        } else {
            (None, authority)
        };

        let (username, password) = if let Some(ui) = userinfo {
            if let Some(pos) = ui.find(':') {
                (Some(String::from(&ui[..pos])), Some(String::from(&ui[pos + 1..])))
            } else {
                (Some(String::from(ui)), None)
            }
        } else {
            (None, None)
        };

        // Extract host and port
        let (host, port) = if let Some(pos) = host_port.rfind(':') {
            // Check if this is IPv6
            if host_port.contains('[') {
                if let Some(bracket_pos) = host_port.find(']') {
                    if pos > bracket_pos {
                        let port_str = &host_port[pos + 1..];
                        let port = port_str.parse().map_err(|_| UrlParseError::InvalidPort)?;
                        (String::from(&host_port[..pos]), port)
                    } else {
                        (String::from(host_port), Self::default_port(&scheme))
                    }
                } else {
                    return Err(UrlParseError::InvalidHost);
                }
            } else {
                let port_str = &host_port[pos + 1..];
                let port = port_str.parse().map_err(|_| UrlParseError::InvalidPort)?;
                (String::from(&host_port[..pos]), port)
            }
        } else {
            (String::from(host_port), Self::default_port(&scheme))
        };

        if host.is_empty() {
            return Err(UrlParseError::InvalidHost);
        }

        Ok(Self {
            scheme,
            host,
            port,
            path,
            query,
            fragment,
            username,
            password,
        })
    }

    /// Get full URL string
    pub fn to_string(&self) -> String {
        let mut url = alloc::format!("{}://{}", self.scheme, self.host);

        if self.port != Self::default_port(&self.scheme) {
            url.push(':');
            url.push_str(&alloc::format!("{}", self.port));
        }

        url.push_str(&self.path);

        if let Some(query) = &self.query {
            url.push('?');
            url.push_str(query);
        }

        if let Some(fragment) = &self.fragment {
            url.push('#');
            url.push_str(fragment);
        }

        url
    }

    /// Get origin
    pub fn origin(&self) -> String {
        alloc::format!("{}://{}:{}", self.scheme, self.host, self.port)
    }

    /// Resolve relative URL
    pub fn resolve(&self, relative: &str) -> Result<Self, UrlParseError> {
        if relative.contains("://") {
            // Absolute URL
            return Self::parse(relative);
        }

        if relative.starts_with("//") {
            // Protocol-relative URL
            return Self::parse(&alloc::format!("{}:{}", self.scheme, relative));
        }

        if relative.starts_with('/') {
            // Absolute path
            return Ok(Self {
                scheme: self.scheme.clone(),
                host: self.host.clone(),
                port: self.port,
                path: String::from(relative),
                query: None,
                fragment: None,
                username: self.username.clone(),
                password: self.password.clone(),
            });
        }

        // Relative path
        let base_path = if let Some(pos) = self.path.rfind('/') {
            &self.path[..pos + 1]
        } else {
            "/"
        };

        let new_path = alloc::format!("{}{}", base_path, relative);

        // Normalize path
        let normalized = normalize_path(&new_path);

        Ok(Self {
            scheme: self.scheme.clone(),
            host: self.host.clone(),
            port: self.port,
            path: normalized,
            query: None,
            fragment: None,
            username: self.username.clone(),
            password: self.password.clone(),
        })
    }

    fn default_port(scheme: &str) -> u16 {
        match scheme {
            "http" => 80,
            "https" => 443,
            _ => 80,
        }
    }
}

/// URL parse error
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UrlParseError {
    /// Invalid scheme
    InvalidScheme,
    /// Invalid host
    InvalidHost,
    /// Invalid port
    InvalidPort,
    /// Invalid URL
    Invalid,
}

/// Network error
#[derive(Debug, Clone)]
pub enum NetworkError {
    /// Connection failed
    ConnectionFailed,
    /// Timeout
    Timeout,
    /// DNS resolution failed
    DnsError,
    /// TLS error
    TlsError,
    /// Invalid response
    InvalidResponse,
    /// Too many redirects
    TooManyRedirects,
    /// URL parse error
    UrlError(UrlParseError),
    /// IO error
    IoError,
    /// Other error
    Other(String),
}

/// HTTP client
pub struct HttpClient {
    /// Connection pool (host -> connection)
    connections: BTreeMap<String, Connection>,
    /// Cookie jar
    cookies: CookieJar,
    /// Default timeout
    default_timeout: u32,
}

impl HttpClient {
    /// Create new HTTP client
    pub fn new() -> Self {
        Self {
            connections: BTreeMap::new(),
            cookies: CookieJar::new(),
            default_timeout: 30000,
        }
    }

    /// Send HTTP request
    pub fn send(&mut self, request: HttpRequest) -> Result<HttpResponse, NetworkError> {
        let mut request = request;
        let mut redirects = 0;

        loop {
            // Build HTTP request
            let http_data = self.build_request(&request)?;

            // Send and receive
            let response_data = self.send_raw(&request.url, &http_data, request.timeout)?;

            // Parse response
            let response = self.parse_response(&response_data, &request.url)?;

            // Handle cookies
            self.handle_cookies(&response, &request.url);

            // Handle redirects
            if response.is_redirect() && request.follow_redirects {
                redirects += 1;
                if redirects > request.max_redirects {
                    return Err(NetworkError::TooManyRedirects);
                }

                if let Some(location) = response.header("location") {
                    let new_url = request.url.resolve(location)
                        .map_err(NetworkError::UrlError)?;
                    request.url = new_url;
                    continue;
                }
            }

            return Ok(response);
        }
    }

    /// Send GET request
    pub fn get(&mut self, url: &str) -> Result<HttpResponse, NetworkError> {
        let request = HttpRequest::get(url).map_err(NetworkError::UrlError)?;
        self.send(request)
    }

    /// Send POST request
    pub fn post(&mut self, url: &str, body: Vec<u8>) -> Result<HttpResponse, NetworkError> {
        let request = HttpRequest::post(url, body).map_err(NetworkError::UrlError)?;
        self.send(request)
    }

    fn build_request(&self, request: &HttpRequest) -> Result<Vec<u8>, NetworkError> {
        let mut http = String::new();

        // Request line
        http.push_str(request.method.as_str());
        http.push(' ');
        http.push_str(&request.url.path);
        if let Some(query) = &request.url.query {
            http.push('?');
            http.push_str(query);
        }
        http.push_str(" HTTP/1.1\r\n");

        // Host header
        http.push_str("Host: ");
        http.push_str(&request.url.host);
        if request.url.port != Url::default_port(&request.url.scheme) {
            http.push(':');
            http.push_str(&alloc::format!("{}", request.url.port));
        }
        http.push_str("\r\n");

        // Other headers
        for (name, value) in &request.headers {
            http.push_str(name);
            http.push_str(": ");
            http.push_str(value);
            http.push_str("\r\n");
        }

        // Cookie header
        let cookies = self.cookies.get_for_url(&request.url);
        if !cookies.is_empty() {
            http.push_str("Cookie: ");
            let cookie_str: Vec<String> = cookies.iter()
                .map(|c| alloc::format!("{}={}", c.name, c.value))
                .collect();
            http.push_str(&cookie_str.join("; "));
            http.push_str("\r\n");
        }

        // Content length for body
        if let Some(body) = &request.body {
            http.push_str("Content-Length: ");
            http.push_str(&alloc::format!("{}", body.len()));
            http.push_str("\r\n");
        }

        http.push_str("\r\n");

        // Body
        let mut data = http.into_bytes();
        if let Some(body) = &request.body {
            data.extend_from_slice(body);
        }

        Ok(data)
    }

    fn send_raw(&mut self, url: &Url, data: &[u8], _timeout: u32) -> Result<Vec<u8>, NetworkError> {
        // This would use the kernel's TCP/TLS stack
        // For now, return a stub response

        // In a real implementation:
        // 1. Check connection pool for existing connection
        // 2. Create new TCP connection if needed
        // 3. If HTTPS, establish TLS
        // 4. Send data
        // 5. Receive response
        // 6. Return response data

        // Stub: return a simple response
        let response = alloc::format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/html\r\n\
             Content-Length: {}\r\n\
             \r\n\
             <!DOCTYPE html>\n\
             <html>\n\
             <head><title>Stenzel Browser</title></head>\n\
             <body>\n\
             <h1>Welcome to Stenzel OS Browser</h1>\n\
             <p>Requested URL: {}</p>\n\
             </body>\n\
             </html>",
            url.to_string().len() + 150,
            url.to_string()
        );

        Ok(response.into_bytes())
    }

    fn parse_response(&self, data: &[u8], url: &Url) -> Result<HttpResponse, NetworkError> {
        let text = core::str::from_utf8(data).map_err(|_| NetworkError::InvalidResponse)?;

        // Find end of headers
        let header_end = text.find("\r\n\r\n").ok_or(NetworkError::InvalidResponse)?;
        let (header_section, body_section) = text.split_at(header_end);
        let body = &body_section[4..]; // Skip \r\n\r\n

        // Parse status line
        let mut lines = header_section.lines();
        let status_line = lines.next().ok_or(NetworkError::InvalidResponse)?;

        let mut parts = status_line.splitn(3, ' ');
        let _version = parts.next().ok_or(NetworkError::InvalidResponse)?;
        let status: u16 = parts.next()
            .ok_or(NetworkError::InvalidResponse)?
            .parse()
            .map_err(|_| NetworkError::InvalidResponse)?;
        let status_text = parts.next().unwrap_or("OK");

        // Parse headers
        let mut headers = BTreeMap::new();
        for line in lines {
            if let Some(pos) = line.find(':') {
                let name = line[..pos].trim().to_lowercase();
                let value = line[pos + 1..].trim();
                headers.insert(name, String::from(value));
            }
        }

        Ok(HttpResponse {
            status,
            status_text: String::from(status_text),
            headers,
            body: body.as_bytes().to_vec(),
            url: url.to_string(),
        })
    }

    fn handle_cookies(&mut self, response: &HttpResponse, url: &Url) {
        if let Some(set_cookie) = response.header("set-cookie") {
            if let Some(cookie) = Cookie::parse(set_cookie, url) {
                self.cookies.add(cookie);
            }
        }
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Connection (placeholder)
struct Connection {
    host: String,
    port: u16,
}

/// Cookie
#[derive(Debug, Clone)]
pub struct Cookie {
    /// Cookie name
    pub name: String,
    /// Cookie value
    pub value: String,
    /// Domain
    pub domain: String,
    /// Path
    pub path: String,
    /// Expiry timestamp
    pub expires: Option<u64>,
    /// Secure flag
    pub secure: bool,
    /// HttpOnly flag
    pub http_only: bool,
    /// SameSite
    pub same_site: SameSite,
}

impl Cookie {
    /// Parse Set-Cookie header
    pub fn parse(header: &str, url: &Url) -> Option<Self> {
        let mut parts = header.split(';');
        let name_value = parts.next()?.trim();

        let (name, value) = if let Some(pos) = name_value.find('=') {
            (name_value[..pos].trim(), name_value[pos + 1..].trim())
        } else {
            return None;
        };

        let mut cookie = Self {
            name: String::from(name),
            value: String::from(value),
            domain: url.host.clone(),
            path: String::from("/"),
            expires: None,
            secure: false,
            http_only: false,
            same_site: SameSite::Lax,
        };

        // Parse attributes
        for part in parts {
            let part = part.trim();
            if let Some(pos) = part.find('=') {
                let attr_name = part[..pos].trim().to_lowercase();
                let attr_value = part[pos + 1..].trim();

                match attr_name.as_str() {
                    "domain" => cookie.domain = String::from(attr_value.trim_start_matches('.')),
                    "path" => cookie.path = String::from(attr_value),
                    "max-age" => {
                        if let Ok(seconds) = attr_value.parse::<u64>() {
                            cookie.expires = Some(crate::time::uptime_secs() + seconds);
                        }
                    }
                    "samesite" => {
                        cookie.same_site = match attr_value.to_lowercase().as_str() {
                            "strict" => SameSite::Strict,
                            "none" => SameSite::None,
                            _ => SameSite::Lax,
                        };
                    }
                    _ => {}
                }
            } else {
                match part.to_lowercase().as_str() {
                    "secure" => cookie.secure = true,
                    "httponly" => cookie.http_only = true,
                    _ => {}
                }
            }
        }

        Some(cookie)
    }

    /// Check if cookie is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires {
            crate::time::uptime_secs() > expires
        } else {
            false
        }
    }

    /// Check if cookie matches URL
    pub fn matches(&self, url: &Url) -> bool {
        // Check domain
        if !url.host.ends_with(&self.domain) {
            return false;
        }

        // Check path
        if !url.path.starts_with(&self.path) {
            return false;
        }

        // Check secure
        if self.secure && url.scheme != "https" {
            return false;
        }

        // Check expiry
        if self.is_expired() {
            return false;
        }

        true
    }
}

/// SameSite attribute
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

/// Cookie jar
#[derive(Debug, Clone)]
pub struct CookieJar {
    cookies: Vec<Cookie>,
}

impl CookieJar {
    /// Create new cookie jar
    pub fn new() -> Self {
        Self { cookies: Vec::new() }
    }

    /// Add cookie
    pub fn add(&mut self, cookie: Cookie) {
        // Remove existing cookie with same name/domain/path
        self.cookies.retain(|c| {
            !(c.name == cookie.name && c.domain == cookie.domain && c.path == cookie.path)
        });
        self.cookies.push(cookie);
    }

    /// Get cookies for URL
    pub fn get_for_url(&self, url: &Url) -> Vec<&Cookie> {
        self.cookies.iter()
            .filter(|c| c.matches(url))
            .collect()
    }

    /// Remove expired cookies
    pub fn cleanup(&mut self) {
        self.cookies.retain(|c| !c.is_expired());
    }

    /// Clear all cookies
    pub fn clear(&mut self) {
        self.cookies.clear();
    }
}

impl Default for CookieJar {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions

fn normalize_path(path: &str) -> String {
    let mut segments: Vec<&str> = Vec::new();

    for segment in path.split('/') {
        match segment {
            "" | "." => continue,
            ".." => { segments.pop(); }
            _ => segments.push(segment),
        }
    }

    let mut result = String::from("/");
    result.push_str(&segments.join("/"));

    if path.ends_with('/') && !result.ends_with('/') {
        result.push('/');
    }

    result
}
