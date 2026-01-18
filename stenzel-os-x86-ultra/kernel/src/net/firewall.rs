//! Firewall Implementation
//!
//! Packet filtering and network access control.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;

/// Unique rule identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RuleId(u64);

impl RuleId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Unique zone identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ZoneId(u64);

impl ZoneId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Network protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Any,
    Tcp,
    Udp,
    Icmp,
    Icmpv6,
    Gre,
    Esp,
    Ah,
}

impl Protocol {
    pub fn name(&self) -> &'static str {
        match self {
            Protocol::Any => "any",
            Protocol::Tcp => "tcp",
            Protocol::Udp => "udp",
            Protocol::Icmp => "icmp",
            Protocol::Icmpv6 => "icmpv6",
            Protocol::Gre => "gre",
            Protocol::Esp => "esp",
            Protocol::Ah => "ah",
        }
    }

    pub fn number(&self) -> Option<u8> {
        match self {
            Protocol::Any => None,
            Protocol::Tcp => Some(6),
            Protocol::Udp => Some(17),
            Protocol::Icmp => Some(1),
            Protocol::Icmpv6 => Some(58),
            Protocol::Gre => Some(47),
            Protocol::Esp => Some(50),
            Protocol::Ah => Some(51),
        }
    }
}

/// IP address type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpAddress {
    Any,
    Ipv4([u8; 4]),
    Ipv6([u8; 16]),
    Ipv4Cidr([u8; 4], u8),
    Ipv6Cidr([u8; 16], u8),
}

impl IpAddress {
    pub fn ipv4(a: u8, b: u8, c: u8, d: u8) -> Self {
        IpAddress::Ipv4([a, b, c, d])
    }

    pub fn ipv4_cidr(a: u8, b: u8, c: u8, d: u8, prefix: u8) -> Self {
        IpAddress::Ipv4Cidr([a, b, c, d], prefix)
    }

    pub fn localhost_v4() -> Self {
        IpAddress::Ipv4([127, 0, 0, 1])
    }

    pub fn loopback_network() -> Self {
        IpAddress::Ipv4Cidr([127, 0, 0, 0], 8)
    }

    pub fn private_10() -> Self {
        IpAddress::Ipv4Cidr([10, 0, 0, 0], 8)
    }

    pub fn private_172() -> Self {
        IpAddress::Ipv4Cidr([172, 16, 0, 0], 12)
    }

    pub fn private_192() -> Self {
        IpAddress::Ipv4Cidr([192, 168, 0, 0], 16)
    }
}

/// Port specification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Port {
    Any,
    Single(u16),
    Range(u16, u16),
    List(Vec<u16>),
}

impl Port {
    pub fn single(port: u16) -> Self {
        Port::Single(port)
    }

    pub fn range(start: u16, end: u16) -> Self {
        Port::Range(start, end)
    }

    pub fn matches(&self, port: u16) -> bool {
        match self {
            Port::Any => true,
            Port::Single(p) => *p == port,
            Port::Range(start, end) => port >= *start && port <= *end,
            Port::List(ports) => ports.contains(&port),
        }
    }
}

/// Rule action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Accept,
    Drop,
    Reject,
    Log,
    LogAccept,
    LogDrop,
    Mark(u32),
    Redirect(u16),
    Masquerade,
    Snat,
    Dnat,
}

impl Action {
    pub fn name(&self) -> &'static str {
        match self {
            Action::Accept => "ACCEPT",
            Action::Drop => "DROP",
            Action::Reject => "REJECT",
            Action::Log => "LOG",
            Action::LogAccept => "LOG+ACCEPT",
            Action::LogDrop => "LOG+DROP",
            Action::Mark(_) => "MARK",
            Action::Redirect(_) => "REDIRECT",
            Action::Masquerade => "MASQUERADE",
            Action::Snat => "SNAT",
            Action::Dnat => "DNAT",
        }
    }

    pub fn is_terminating(&self) -> bool {
        match self {
            Action::Accept | Action::Drop | Action::Reject => true,
            Action::LogAccept | Action::LogDrop => true,
            Action::Masquerade | Action::Snat | Action::Dnat => true,
            Action::Log | Action::Mark(_) => false,
            Action::Redirect(_) => true,
        }
    }
}

/// Rule direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Inbound,
    Outbound,
    Forward,
}

impl Direction {
    pub fn name(&self) -> &'static str {
        match self {
            Direction::Inbound => "IN",
            Direction::Outbound => "OUT",
            Direction::Forward => "FWD",
        }
    }
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    New,
    Established,
    Related,
    Invalid,
}

impl ConnState {
    pub fn name(&self) -> &'static str {
        match self {
            ConnState::New => "NEW",
            ConnState::Established => "ESTABLISHED",
            ConnState::Related => "RELATED",
            ConnState::Invalid => "INVALID",
        }
    }
}

/// Firewall rule
#[derive(Debug, Clone)]
pub struct Rule {
    pub id: RuleId,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub direction: Direction,
    pub protocol: Protocol,
    pub src_addr: IpAddress,
    pub dst_addr: IpAddress,
    pub src_port: Port,
    pub dst_port: Port,
    pub action: Action,
    pub log: bool,
    pub log_prefix: Option<String>,
    pub conn_states: Vec<ConnState>,
    pub interface_in: Option<String>,
    pub interface_out: Option<String>,
    pub zone: Option<ZoneId>,
    pub priority: u32,
    pub hit_count: u64,
    pub last_hit: u64,
    pub created: u64,
    pub modified: u64,
}

impl Rule {
    pub fn new(id: RuleId, name: &str) -> Self {
        Self {
            id,
            name: String::from(name),
            description: String::new(),
            enabled: true,
            direction: Direction::Inbound,
            protocol: Protocol::Any,
            src_addr: IpAddress::Any,
            dst_addr: IpAddress::Any,
            src_port: Port::Any,
            dst_port: Port::Any,
            action: Action::Accept,
            log: false,
            log_prefix: None,
            conn_states: Vec::new(),
            interface_in: None,
            interface_out: None,
            zone: None,
            priority: 1000,
            hit_count: 0,
            last_hit: 0,
            created: 0,
            modified: 0,
        }
    }

    pub fn allow_inbound(id: RuleId, name: &str, port: u16, protocol: Protocol) -> Self {
        let mut rule = Self::new(id, name);
        rule.direction = Direction::Inbound;
        rule.protocol = protocol;
        rule.dst_port = Port::Single(port);
        rule.action = Action::Accept;
        rule
    }

    pub fn deny_inbound(id: RuleId, name: &str) -> Self {
        let mut rule = Self::new(id, name);
        rule.direction = Direction::Inbound;
        rule.action = Action::Drop;
        rule
    }

    pub fn allow_outbound(id: RuleId, name: &str) -> Self {
        let mut rule = Self::new(id, name);
        rule.direction = Direction::Outbound;
        rule.action = Action::Accept;
        rule
    }

    pub fn record_hit(&mut self, timestamp: u64) {
        self.hit_count += 1;
        self.last_hit = timestamp;
    }
}

/// Network zone
#[derive(Debug, Clone)]
pub struct Zone {
    pub id: ZoneId,
    pub name: String,
    pub description: String,
    pub interfaces: Vec<String>,
    pub default_action: Action,
    pub allow_icmp: bool,
    pub allow_icmpv6: bool,
    pub masquerade: bool,
    pub forward: bool,
}

impl Zone {
    pub fn new(id: ZoneId, name: &str) -> Self {
        Self {
            id,
            name: String::from(name),
            description: String::new(),
            interfaces: Vec::new(),
            default_action: Action::Drop,
            allow_icmp: true,
            allow_icmpv6: true,
            masquerade: false,
            forward: false,
        }
    }

    pub fn public(id: ZoneId) -> Self {
        let mut zone = Self::new(id, "public");
        zone.description = String::from("Public network - untrusted");
        zone.default_action = Action::Drop;
        zone
    }

    pub fn home(id: ZoneId) -> Self {
        let mut zone = Self::new(id, "home");
        zone.description = String::from("Home network - semi-trusted");
        zone.default_action = Action::Drop;
        zone
    }

    pub fn work(id: ZoneId) -> Self {
        let mut zone = Self::new(id, "work");
        zone.description = String::from("Work network - trusted");
        zone.default_action = Action::Accept;
        zone
    }

    pub fn trusted(id: ZoneId) -> Self {
        let mut zone = Self::new(id, "trusted");
        zone.description = String::from("Trusted network - all traffic allowed");
        zone.default_action = Action::Accept;
        zone.forward = true;
        zone
    }
}

/// Predefined service
#[derive(Debug, Clone)]
pub struct Service {
    pub name: String,
    pub description: String,
    pub protocol: Protocol,
    pub ports: Vec<u16>,
}

impl Service {
    pub fn new(name: &str, description: &str, protocol: Protocol, ports: Vec<u16>) -> Self {
        Self {
            name: String::from(name),
            description: String::from(description),
            protocol,
            ports,
        }
    }

    pub fn ssh() -> Self {
        Self::new("ssh", "Secure Shell", Protocol::Tcp, vec![22])
    }

    pub fn http() -> Self {
        Self::new("http", "HTTP Web Server", Protocol::Tcp, vec![80])
    }

    pub fn https() -> Self {
        Self::new("https", "HTTPS Web Server", Protocol::Tcp, vec![443])
    }

    pub fn dns() -> Self {
        Self::new("dns", "Domain Name System", Protocol::Udp, vec![53])
    }

    pub fn dhcp() -> Self {
        Self::new("dhcp", "DHCP Client", Protocol::Udp, vec![67, 68])
    }

    pub fn ntp() -> Self {
        Self::new("ntp", "Network Time Protocol", Protocol::Udp, vec![123])
    }

    pub fn smtp() -> Self {
        Self::new("smtp", "SMTP Mail", Protocol::Tcp, vec![25, 465, 587])
    }

    pub fn imap() -> Self {
        Self::new("imap", "IMAP Mail", Protocol::Tcp, vec![143, 993])
    }

    pub fn pop3() -> Self {
        Self::new("pop3", "POP3 Mail", Protocol::Tcp, vec![110, 995])
    }

    pub fn ftp() -> Self {
        Self::new("ftp", "File Transfer Protocol", Protocol::Tcp, vec![20, 21])
    }

    pub fn smb() -> Self {
        Self::new("smb", "SMB/CIFS File Sharing", Protocol::Tcp, vec![139, 445])
    }

    pub fn rdp() -> Self {
        Self::new("rdp", "Remote Desktop Protocol", Protocol::Tcp, vec![3389])
    }

    pub fn vnc() -> Self {
        Self::new("vnc", "Virtual Network Computing", Protocol::Tcp, vec![5900])
    }

    pub fn wireguard() -> Self {
        Self::new("wireguard", "WireGuard VPN", Protocol::Udp, vec![51820])
    }

    pub fn openvpn() -> Self {
        Self::new("openvpn", "OpenVPN", Protocol::Udp, vec![1194])
    }
}

/// Default services list
pub fn builtin_services() -> Vec<Service> {
    vec![
        Service::ssh(),
        Service::http(),
        Service::https(),
        Service::dns(),
        Service::dhcp(),
        Service::ntp(),
        Service::smtp(),
        Service::imap(),
        Service::pop3(),
        Service::ftp(),
        Service::smb(),
        Service::rdp(),
        Service::vnc(),
        Service::wireguard(),
        Service::openvpn(),
    ]
}

/// Connection tracking entry
#[derive(Debug, Clone)]
pub struct ConnTrackEntry {
    pub protocol: Protocol,
    pub src_addr: [u8; 4],
    pub dst_addr: [u8; 4],
    pub src_port: u16,
    pub dst_port: u16,
    pub state: ConnState,
    pub packets_in: u64,
    pub packets_out: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub created: u64,
    pub last_seen: u64,
    pub timeout: u64,
}

impl ConnTrackEntry {
    pub fn new(protocol: Protocol, src: [u8; 4], dst: [u8; 4], sport: u16, dport: u16) -> Self {
        Self {
            protocol,
            src_addr: src,
            dst_addr: dst,
            src_port: sport,
            dst_port: dport,
            state: ConnState::New,
            packets_in: 0,
            packets_out: 0,
            bytes_in: 0,
            bytes_out: 0,
            created: 0,
            last_seen: 0,
            timeout: 300, // 5 minutes default
        }
    }

    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time > self.last_seen + self.timeout
    }
}

/// Firewall statistics
#[derive(Debug, Clone, Default)]
pub struct FirewallStats {
    pub packets_accepted: u64,
    pub packets_dropped: u64,
    pub packets_rejected: u64,
    pub bytes_accepted: u64,
    pub bytes_dropped: u64,
    pub active_connections: u64,
    pub total_rules: u64,
    pub enabled_rules: u64,
}

/// Firewall error
#[derive(Debug, Clone)]
pub enum FirewallError {
    RuleNotFound,
    ZoneNotFound,
    InvalidRule,
    DuplicateRule,
    MaxRulesReached,
}

pub type FirewallResult<T> = Result<T, FirewallError>;

/// Firewall manager
pub struct FirewallManager {
    rules: BTreeMap<RuleId, Rule>,
    zones: BTreeMap<ZoneId, Zone>,
    services: Vec<Service>,
    conntrack: Vec<ConnTrackEntry>,

    next_rule_id: u64,
    next_zone_id: u64,

    enabled: bool,
    default_inbound_action: Action,
    default_outbound_action: Action,
    default_forward_action: Action,

    enable_logging: bool,
    log_dropped: bool,
    log_rejected: bool,

    stats: FirewallStats,
    current_time: u64,

    max_rules: usize,
    max_conntrack: usize,
}

impl FirewallManager {
    pub fn new() -> Self {
        Self {
            rules: BTreeMap::new(),
            zones: BTreeMap::new(),
            services: builtin_services(),
            conntrack: Vec::new(),
            next_rule_id: 1,
            next_zone_id: 1,
            enabled: true,
            default_inbound_action: Action::Drop,
            default_outbound_action: Action::Accept,
            default_forward_action: Action::Drop,
            enable_logging: true,
            log_dropped: true,
            log_rejected: true,
            stats: FirewallStats::default(),
            current_time: 0,
            max_rules: 10000,
            max_conntrack: 65536,
        }
    }

    /// Enable/disable firewall
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set default actions
    pub fn set_default_inbound(&mut self, action: Action) {
        self.default_inbound_action = action;
    }

    pub fn set_default_outbound(&mut self, action: Action) {
        self.default_outbound_action = action;
    }

    pub fn set_default_forward(&mut self, action: Action) {
        self.default_forward_action = action;
    }

    /// Add a rule
    pub fn add_rule(&mut self, mut rule: Rule) -> FirewallResult<RuleId> {
        if self.rules.len() >= self.max_rules {
            return Err(FirewallError::MaxRulesReached);
        }

        rule.id = RuleId::new(self.next_rule_id);
        self.next_rule_id += 1;
        rule.created = self.current_time;
        rule.modified = self.current_time;

        let id = rule.id;
        self.rules.insert(id, rule);
        self.stats.total_rules += 1;
        self.stats.enabled_rules += 1;

        Ok(id)
    }

    /// Remove a rule
    pub fn remove_rule(&mut self, id: RuleId) -> FirewallResult<()> {
        if let Some(rule) = self.rules.remove(&id) {
            self.stats.total_rules -= 1;
            if rule.enabled {
                self.stats.enabled_rules -= 1;
            }
            Ok(())
        } else {
            Err(FirewallError::RuleNotFound)
        }
    }

    /// Get rule
    pub fn get_rule(&self, id: RuleId) -> Option<&Rule> {
        self.rules.get(&id)
    }

    /// Get rule mut
    pub fn get_rule_mut(&mut self, id: RuleId) -> Option<&mut Rule> {
        self.rules.get_mut(&id)
    }

    /// Enable/disable rule
    pub fn set_rule_enabled(&mut self, id: RuleId, enabled: bool) -> FirewallResult<()> {
        if let Some(rule) = self.rules.get_mut(&id) {
            if rule.enabled != enabled {
                rule.enabled = enabled;
                if enabled {
                    self.stats.enabled_rules += 1;
                } else {
                    self.stats.enabled_rules -= 1;
                }
            }
            Ok(())
        } else {
            Err(FirewallError::RuleNotFound)
        }
    }

    /// Get all rules
    pub fn rules(&self) -> Vec<&Rule> {
        let mut rules: Vec<_> = self.rules.values().collect();
        rules.sort_by_key(|r| r.priority);
        rules
    }

    /// Get rules by direction
    pub fn rules_by_direction(&self, direction: Direction) -> Vec<&Rule> {
        let mut rules: Vec<_> = self.rules.values()
            .filter(|r| r.direction == direction)
            .collect();
        rules.sort_by_key(|r| r.priority);
        rules
    }

    /// Add a zone
    pub fn add_zone(&mut self, mut zone: Zone) -> ZoneId {
        zone.id = ZoneId::new(self.next_zone_id);
        self.next_zone_id += 1;
        let id = zone.id;
        self.zones.insert(id, zone);
        id
    }

    /// Get zone
    pub fn get_zone(&self, id: ZoneId) -> Option<&Zone> {
        self.zones.get(&id)
    }

    /// Get zone by name
    pub fn get_zone_by_name(&self, name: &str) -> Option<&Zone> {
        self.zones.values().find(|z| z.name == name)
    }

    /// Get all zones
    pub fn zones(&self) -> Vec<&Zone> {
        self.zones.values().collect()
    }

    /// Get services
    pub fn services(&self) -> &[Service] {
        &self.services
    }

    /// Get service by name
    pub fn get_service(&self, name: &str) -> Option<&Service> {
        self.services.iter().find(|s| s.name == name)
    }

    /// Create rule from service
    pub fn allow_service(&mut self, service_name: &str, direction: Direction) -> FirewallResult<Vec<RuleId>> {
        let service = self.services.iter()
            .find(|s| s.name == service_name)
            .cloned()
            .ok_or(FirewallError::InvalidRule)?;

        let mut rule_ids = Vec::new();

        for port in &service.ports {
            let mut rule = Rule::new(RuleId::new(0), &service.name);
            rule.description = service.description.clone();
            rule.direction = direction;
            rule.protocol = service.protocol;
            rule.dst_port = Port::Single(*port);
            rule.action = Action::Accept;

            let id = self.add_rule(rule)?;
            rule_ids.push(id);
        }

        Ok(rule_ids)
    }

    /// Get connection tracking entries
    pub fn conntrack_entries(&self) -> &[ConnTrackEntry] {
        &self.conntrack
    }

    /// Get active connection count
    pub fn active_connections(&self) -> usize {
        self.conntrack.len()
    }

    /// Clean expired connections
    pub fn cleanup_conntrack(&mut self) {
        self.conntrack.retain(|e| !e.is_expired(self.current_time));
        self.stats.active_connections = self.conntrack.len() as u64;
    }

    /// Get statistics
    pub fn stats(&self) -> &FirewallStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = FirewallStats::default();
        self.stats.total_rules = self.rules.len() as u64;
        self.stats.enabled_rules = self.rules.values().filter(|r| r.enabled).count() as u64;
        self.stats.active_connections = self.conntrack.len() as u64;
    }

    /// Set current time
    pub fn set_current_time(&mut self, time: u64) {
        self.current_time = time;
    }

    /// Add sample data for demo
    pub fn add_sample_data(&mut self) {
        self.current_time = 1705600000;

        // Create zones
        let public_id = self.add_zone(Zone::public(ZoneId::new(0)));
        let home_id = self.add_zone(Zone::home(ZoneId::new(0)));
        let _work_id = self.add_zone(Zone::work(ZoneId::new(0)));

        // Allow established connections
        let mut established = Rule::new(RuleId::new(0), "Allow Established");
        established.description = String::from("Allow established and related connections");
        established.direction = Direction::Inbound;
        established.conn_states = vec![ConnState::Established, ConnState::Related];
        established.action = Action::Accept;
        established.priority = 100;
        let _ = self.add_rule(established);

        // Allow loopback
        let mut loopback = Rule::new(RuleId::new(0), "Allow Loopback");
        loopback.description = String::from("Allow all traffic on loopback interface");
        loopback.direction = Direction::Inbound;
        loopback.src_addr = IpAddress::loopback_network();
        loopback.action = Action::Accept;
        loopback.priority = 50;
        let _ = self.add_rule(loopback);

        // Allow SSH
        let mut ssh = Rule::allow_inbound(RuleId::new(0), "Allow SSH", 22, Protocol::Tcp);
        ssh.description = String::from("Allow incoming SSH connections");
        ssh.zone = Some(home_id);
        ssh.priority = 200;
        let _ = self.add_rule(ssh);

        // Allow HTTP/HTTPS
        let mut http = Rule::allow_inbound(RuleId::new(0), "Allow HTTP", 80, Protocol::Tcp);
        http.description = String::from("Allow incoming HTTP connections");
        http.priority = 300;
        let _ = self.add_rule(http);

        let mut https = Rule::allow_inbound(RuleId::new(0), "Allow HTTPS", 443, Protocol::Tcp);
        https.description = String::from("Allow incoming HTTPS connections");
        https.priority = 301;
        let _ = self.add_rule(https);

        // Block ICMP from public
        let mut block_icmp = Rule::new(RuleId::new(0), "Block Public ICMP");
        block_icmp.description = String::from("Block ICMP from public networks");
        block_icmp.direction = Direction::Inbound;
        block_icmp.protocol = Protocol::Icmp;
        block_icmp.zone = Some(public_id);
        block_icmp.action = Action::Drop;
        block_icmp.log = true;
        block_icmp.priority = 400;
        let _ = self.add_rule(block_icmp);

        // Allow all outbound
        let mut outbound = Rule::allow_outbound(RuleId::new(0), "Allow All Outbound");
        outbound.description = String::from("Allow all outgoing traffic");
        outbound.priority = 1000;
        let _ = self.add_rule(outbound);

        // Default deny
        let mut deny = Rule::deny_inbound(RuleId::new(0), "Default Deny");
        deny.description = String::from("Default deny all incoming traffic");
        deny.log = true;
        deny.log_prefix = Some(String::from("DROPPED: "));
        deny.priority = 65535;
        let _ = self.add_rule(deny);

        // Sample conntrack entries
        self.conntrack.push(ConnTrackEntry::new(
            Protocol::Tcp,
            [192, 168, 1, 100],
            [93, 184, 216, 34],
            52431,
            443,
        ));
        self.conntrack.push(ConnTrackEntry::new(
            Protocol::Udp,
            [192, 168, 1, 100],
            [8, 8, 8, 8],
            53241,
            53,
        ));

        self.stats.active_connections = self.conntrack.len() as u64;
    }
}

impl Default for FirewallManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize firewall module
pub fn init() -> FirewallManager {
    let mut manager = FirewallManager::new();
    manager.add_sample_data();
    manager
}
