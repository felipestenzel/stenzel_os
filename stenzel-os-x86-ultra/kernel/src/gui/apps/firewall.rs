//! Firewall GUI Application
//!
//! Graphical interface for managing firewall rules.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use crate::gui::window_manager::Rect;
use crate::net::firewall::{
    FirewallManager, Rule, RuleId, Zone, ZoneId, Service,
    Protocol, Port, Action, Direction, ConnState, IpAddress,
    FirewallStats, ConnTrackEntry, FirewallError, FirewallResult,
};

/// View mode for the firewall application
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirewallView {
    Rules,
    Zones,
    Services,
    Connections,
    Logs,
    Settings,
}

impl FirewallView {
    pub fn name(&self) -> &'static str {
        match self {
            FirewallView::Rules => "Rules",
            FirewallView::Zones => "Zones",
            FirewallView::Services => "Services",
            FirewallView::Connections => "Connections",
            FirewallView::Logs => "Logs",
            FirewallView::Settings => "Settings",
        }
    }
}

/// Rule filter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleFilter {
    All,
    Inbound,
    Outbound,
    Forward,
    Enabled,
    Disabled,
}

impl RuleFilter {
    pub fn name(&self) -> &'static str {
        match self {
            RuleFilter::All => "All",
            RuleFilter::Inbound => "Inbound",
            RuleFilter::Outbound => "Outbound",
            RuleFilter::Forward => "Forward",
            RuleFilter::Enabled => "Enabled",
            RuleFilter::Disabled => "Disabled",
        }
    }
}

/// Log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: u64,
    pub action: Action,
    pub protocol: Protocol,
    pub src_addr: String,
    pub dst_addr: String,
    pub src_port: u16,
    pub dst_port: u16,
    pub rule_name: String,
    pub interface: Option<String>,
}

impl LogEntry {
    pub fn format_time(&self) -> String {
        let secs = self.timestamp % 60;
        let mins = (self.timestamp / 60) % 60;
        let hours = (self.timestamp / 3600) % 24;
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    }
}

/// Quick action for common firewall operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickAction {
    AllowPort,
    BlockPort,
    AllowApp,
    BlockApp,
    AllowService,
    BlockService,
}

impl QuickAction {
    pub fn name(&self) -> &'static str {
        match self {
            QuickAction::AllowPort => "Allow Port",
            QuickAction::BlockPort => "Block Port",
            QuickAction::AllowApp => "Allow App",
            QuickAction::BlockApp => "Block App",
            QuickAction::AllowService => "Allow Service",
            QuickAction::BlockService => "Block Service",
        }
    }
}

/// Firewall GUI application
pub struct FirewallApp {
    manager: FirewallManager,
    view: FirewallView,
    rule_filter: RuleFilter,
    selected_rule: Option<RuleId>,
    selected_zone: Option<ZoneId>,
    selected_service: Option<usize>,
    selected_connection: Option<usize>,
    logs: Vec<LogEntry>,
    max_logs: usize,

    // UI state
    bounds: Rect,
    visible: bool,
    focused: bool,
    scroll_offset: i32,
    show_add_rule_dialog: bool,
    show_edit_rule_dialog: bool,
    show_add_zone_dialog: bool,

    // Edit rule state
    edit_rule_name: String,
    edit_rule_description: String,
    edit_rule_direction: Direction,
    edit_rule_protocol: Protocol,
    edit_rule_src_port: String,
    edit_rule_dst_port: String,
    edit_rule_action: Action,
    edit_rule_enabled: bool,

    // Search
    search_query: String,

    // Status
    status_message: Option<String>,
}

impl FirewallApp {
    pub fn new() -> Self {
        let mut manager = FirewallManager::new();
        manager.add_sample_data();

        Self {
            manager,
            view: FirewallView::Rules,
            rule_filter: RuleFilter::All,
            selected_rule: None,
            selected_zone: None,
            selected_service: None,
            selected_connection: None,
            logs: Vec::new(),
            max_logs: 1000,
            bounds: Rect::new(0, 0, 1000, 700),
            visible: true,
            focused: true,
            scroll_offset: 0,
            show_add_rule_dialog: false,
            show_edit_rule_dialog: false,
            show_add_zone_dialog: false,
            edit_rule_name: String::new(),
            edit_rule_description: String::new(),
            edit_rule_direction: Direction::Inbound,
            edit_rule_protocol: Protocol::Any,
            edit_rule_src_port: String::new(),
            edit_rule_dst_port: String::new(),
            edit_rule_action: Action::Accept,
            edit_rule_enabled: true,
            search_query: String::new(),
            status_message: None,
        }
    }

    /// Get current view
    pub fn view(&self) -> FirewallView {
        self.view
    }

    /// Set current view
    pub fn set_view(&mut self, view: FirewallView) {
        self.view = view;
        self.scroll_offset = 0;
        self.selected_rule = None;
        self.selected_zone = None;
        self.selected_service = None;
        self.selected_connection = None;
    }

    /// Get rule filter
    pub fn rule_filter(&self) -> RuleFilter {
        self.rule_filter
    }

    /// Set rule filter
    pub fn set_rule_filter(&mut self, filter: RuleFilter) {
        self.rule_filter = filter;
        self.scroll_offset = 0;
    }

    /// Get filtered rules
    pub fn filtered_rules(&self) -> Vec<&Rule> {
        let rules = self.manager.rules();

        rules.into_iter().filter(|rule| {
            match self.rule_filter {
                RuleFilter::All => true,
                RuleFilter::Inbound => rule.direction == Direction::Inbound,
                RuleFilter::Outbound => rule.direction == Direction::Outbound,
                RuleFilter::Forward => rule.direction == Direction::Forward,
                RuleFilter::Enabled => rule.enabled,
                RuleFilter::Disabled => !rule.enabled,
            }
        }).filter(|rule| {
            if self.search_query.is_empty() {
                true
            } else {
                let query = self.search_query.to_lowercase();
                rule.name.to_lowercase().contains(&query) ||
                rule.description.to_lowercase().contains(&query)
            }
        }).collect()
    }

    /// Get zones
    pub fn zones(&self) -> Vec<&Zone> {
        self.manager.zones()
    }

    /// Get services
    pub fn services(&self) -> &[Service] {
        self.manager.services()
    }

    /// Get connections
    pub fn connections(&self) -> &[ConnTrackEntry] {
        self.manager.conntrack_entries()
    }

    /// Get logs
    pub fn logs(&self) -> &[LogEntry] {
        &self.logs
    }

    /// Get statistics
    pub fn stats(&self) -> &FirewallStats {
        self.manager.stats()
    }

    /// Is firewall enabled
    pub fn is_enabled(&self) -> bool {
        self.manager.is_enabled()
    }

    /// Toggle firewall
    pub fn toggle_enabled(&mut self) {
        let enabled = !self.manager.is_enabled();
        self.manager.set_enabled(enabled);
        self.set_status(if enabled {
            "Firewall enabled"
        } else {
            "Firewall disabled"
        });
    }

    /// Select rule
    pub fn select_rule(&mut self, id: RuleId) {
        self.selected_rule = Some(id);
    }

    /// Get selected rule
    pub fn selected_rule(&self) -> Option<&Rule> {
        self.selected_rule.and_then(|id| self.manager.get_rule(id))
    }

    /// Toggle rule enabled
    pub fn toggle_rule_enabled(&mut self) {
        if let Some(id) = self.selected_rule {
            if let Some(rule) = self.manager.get_rule(id) {
                let enabled = !rule.enabled;
                if self.manager.set_rule_enabled(id, enabled).is_ok() {
                    self.set_status(if enabled {
                        "Rule enabled"
                    } else {
                        "Rule disabled"
                    });
                }
            }
        }
    }

    /// Delete selected rule
    pub fn delete_selected_rule(&mut self) {
        if let Some(id) = self.selected_rule {
            if self.manager.remove_rule(id).is_ok() {
                self.selected_rule = None;
                self.set_status("Rule deleted");
            }
        }
    }

    /// Start adding a new rule
    pub fn start_add_rule(&mut self) {
        self.show_add_rule_dialog = true;
        self.edit_rule_name = String::new();
        self.edit_rule_description = String::new();
        self.edit_rule_direction = Direction::Inbound;
        self.edit_rule_protocol = Protocol::Tcp;
        self.edit_rule_src_port = String::new();
        self.edit_rule_dst_port = String::new();
        self.edit_rule_action = Action::Accept;
        self.edit_rule_enabled = true;
    }

    /// Confirm adding rule
    pub fn confirm_add_rule(&mut self) {
        let mut rule = Rule::new(RuleId::new(0), &self.edit_rule_name);
        rule.description = self.edit_rule_description.clone();
        rule.direction = self.edit_rule_direction;
        rule.protocol = self.edit_rule_protocol;
        rule.action = self.edit_rule_action;
        rule.enabled = self.edit_rule_enabled;

        // Parse destination port
        if !self.edit_rule_dst_port.is_empty() {
            if let Ok(port) = self.edit_rule_dst_port.parse::<u16>() {
                rule.dst_port = Port::Single(port);
            }
        }

        // Parse source port
        if !self.edit_rule_src_port.is_empty() {
            if let Ok(port) = self.edit_rule_src_port.parse::<u16>() {
                rule.src_port = Port::Single(port);
            }
        }

        match self.manager.add_rule(rule) {
            Ok(id) => {
                self.selected_rule = Some(id);
                self.set_status("Rule added successfully");
            }
            Err(_) => {
                self.set_status("Failed to add rule");
            }
        }

        self.show_add_rule_dialog = false;
    }

    /// Cancel adding rule
    pub fn cancel_add_rule(&mut self) {
        self.show_add_rule_dialog = false;
    }

    /// Quick allow service
    pub fn allow_service(&mut self, service_name: &str) {
        match self.manager.allow_service(service_name, Direction::Inbound) {
            Ok(_) => {
                self.set_status(&format!("Service '{}' allowed", service_name));
            }
            Err(_) => {
                self.set_status(&format!("Failed to allow service '{}'", service_name));
            }
        }
    }

    /// Add zone
    pub fn add_zone(&mut self, name: &str) -> ZoneId {
        let zone = Zone::new(ZoneId::new(0), name);
        self.manager.add_zone(zone)
    }

    /// Add log entry
    pub fn add_log(&mut self, entry: LogEntry) {
        if self.logs.len() >= self.max_logs {
            self.logs.remove(0);
        }
        self.logs.push(entry);
    }

    /// Clear logs
    pub fn clear_logs(&mut self) {
        self.logs.clear();
        self.set_status("Logs cleared");
    }

    /// Set search query
    pub fn set_search_query(&mut self, query: &str) {
        self.search_query = String::from(query);
    }

    /// Set status message
    fn set_status(&mut self, message: &str) {
        self.status_message = Some(String::from(message));
    }

    /// Get status message
    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    /// Clear status message
    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.manager.reset_stats();
        self.set_status("Statistics reset");
    }

    /// Get manager
    pub fn manager(&self) -> &FirewallManager {
        &self.manager
    }

    /// Get manager mut
    pub fn manager_mut(&mut self) -> &mut FirewallManager {
        &mut self.manager
    }

    // Widget trait implementation helpers

    pub fn id(&self) -> &'static str {
        "firewall"
    }

    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    pub fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    pub fn set_size(&mut self, width: u32, height: u32) {
        self.bounds.width = width;
        self.bounds.height = height;
    }

    pub fn widget_is_enabled(&self) -> bool {
        true
    }

    pub fn widget_set_enabled(&mut self, _enabled: bool) {}

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, key: char) {
        match key {
            '1' => self.set_view(FirewallView::Rules),
            '2' => self.set_view(FirewallView::Zones),
            '3' => self.set_view(FirewallView::Services),
            '4' => self.set_view(FirewallView::Connections),
            '5' => self.set_view(FirewallView::Logs),
            '6' => self.set_view(FirewallView::Settings),
            'n' | 'N' => self.start_add_rule(),
            'd' | 'D' => self.delete_selected_rule(),
            'e' | 'E' => self.toggle_rule_enabled(),
            'f' | 'F' => self.toggle_enabled(),
            'c' | 'C' => {
                if self.view == FirewallView::Logs {
                    self.clear_logs();
                }
            }
            _ => {}
        }
    }

    /// Handle navigation
    pub fn navigate(&mut self, up: bool) {
        let rules = self.filtered_rules();
        if rules.is_empty() {
            return;
        }

        let current_index = self.selected_rule
            .and_then(|id| rules.iter().position(|r| r.id == id))
            .unwrap_or(0);

        let new_index = if up {
            if current_index > 0 { current_index - 1 } else { rules.len() - 1 }
        } else {
            if current_index < rules.len() - 1 { current_index + 1 } else { 0 }
        };

        if let Some(rule) = rules.get(new_index) {
            self.selected_rule = Some(rule.id);
        }
    }

    /// Draw helper - format IP address
    pub fn format_ip(addr: &IpAddress) -> String {
        match addr {
            IpAddress::Any => String::from("*"),
            IpAddress::Ipv4(ip) => format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]),
            IpAddress::Ipv4Cidr(ip, prefix) => {
                format!("{}.{}.{}.{}/{}", ip[0], ip[1], ip[2], ip[3], prefix)
            }
            IpAddress::Ipv6(ip) => {
                // Simplified IPv6 formatting
                format!("{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
                    u16::from_be_bytes([ip[0], ip[1]]),
                    u16::from_be_bytes([ip[2], ip[3]]),
                    u16::from_be_bytes([ip[4], ip[5]]),
                    u16::from_be_bytes([ip[6], ip[7]]),
                    u16::from_be_bytes([ip[8], ip[9]]),
                    u16::from_be_bytes([ip[10], ip[11]]),
                    u16::from_be_bytes([ip[12], ip[13]]),
                    u16::from_be_bytes([ip[14], ip[15]]))
            }
            IpAddress::Ipv6Cidr(ip, prefix) => {
                format!("{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}/{}",
                    u16::from_be_bytes([ip[0], ip[1]]),
                    u16::from_be_bytes([ip[2], ip[3]]),
                    u16::from_be_bytes([ip[4], ip[5]]),
                    u16::from_be_bytes([ip[6], ip[7]]),
                    u16::from_be_bytes([ip[8], ip[9]]),
                    u16::from_be_bytes([ip[10], ip[11]]),
                    u16::from_be_bytes([ip[12], ip[13]]),
                    u16::from_be_bytes([ip[14], ip[15]]),
                    prefix)
            }
        }
    }

    /// Draw helper - format port
    pub fn format_port(port: &Port) -> String {
        match port {
            Port::Any => String::from("*"),
            Port::Single(p) => format!("{}", p),
            Port::Range(start, end) => format!("{}-{}", start, end),
            Port::List(ports) => {
                let strs: Vec<String> = ports.iter().map(|p| format!("{}", p)).collect();
                strs.join(",")
            }
        }
    }

    /// Format connection for display
    pub fn format_connection(conn: &ConnTrackEntry) -> String {
        format!("{} {}.{}.{}.{}:{} -> {}.{}.{}.{}:{} [{}]",
            conn.protocol.name(),
            conn.src_addr[0], conn.src_addr[1], conn.src_addr[2], conn.src_addr[3],
            conn.src_port,
            conn.dst_addr[0], conn.dst_addr[1], conn.dst_addr[2], conn.dst_addr[3],
            conn.dst_port,
            conn.state.name())
    }

    /// Get rule action color
    pub fn action_color(action: Action) -> u32 {
        match action {
            Action::Accept | Action::LogAccept => 0xFF4CAF50, // Green
            Action::Drop | Action::LogDrop => 0xFFF44336, // Red
            Action::Reject => 0xFFFF9800, // Orange
            Action::Log => 0xFF2196F3, // Blue
            _ => 0xFFFFFFFF, // White
        }
    }

    /// Sample render for demo
    pub fn render_demo(&self) -> Vec<String> {
        let mut lines = Vec::new();

        lines.push(format!("╔═══════════════════════════════════════════════════════════════════╗"));
        lines.push(format!("║               STENZEL OS FIREWALL                                   ║"));
        lines.push(format!("║  Status: {}                                                        ║",
            if self.is_enabled() { "ENABLED " } else { "DISABLED" }));
        lines.push(format!("╠═══════════════════════════════════════════════════════════════════╣"));

        // View tabs
        lines.push(format!("║ [1]Rules  [2]Zones  [3]Services  [4]Connections  [5]Logs  [6]Settings ║"));
        lines.push(format!("╠═══════════════════════════════════════════════════════════════════╣"));

        match self.view {
            FirewallView::Rules => {
                lines.push(format!("║ FIREWALL RULES                                   Filter: {:10} ║",
                    self.rule_filter.name()));
                lines.push(format!("╟───────────────────────────────────────────────────────────────────╢"));
                lines.push(format!("║ {:3} {:20} {:6} {:5} {:8} {:10} ║",
                    "#", "Name", "Dir", "Proto", "Port", "Action"));
                lines.push(format!("╟───────────────────────────────────────────────────────────────────╢"));

                for (i, rule) in self.filtered_rules().iter().enumerate() {
                    let selected = self.selected_rule.map(|id| id == rule.id).unwrap_or(false);
                    let marker = if selected { ">" } else { " " };
                    let status = if rule.enabled { "✓" } else { "✗" };

                    lines.push(format!("║{}{}{:3} {:20} {:6} {:5} {:8} {:10}║",
                        marker,
                        status,
                        i + 1,
                        if rule.name.len() > 18 { &rule.name[..18] } else { &rule.name },
                        rule.direction.name(),
                        rule.protocol.name(),
                        Self::format_port(&rule.dst_port),
                        rule.action.name()));
                }
            }
            FirewallView::Zones => {
                lines.push(format!("║ NETWORK ZONES                                                     ║"));
                lines.push(format!("╟───────────────────────────────────────────────────────────────────╢"));

                for zone in self.zones() {
                    lines.push(format!("║ {:15} - {:40} ║",
                        zone.name,
                        if zone.description.len() > 38 { &zone.description[..38] } else { &zone.description }));
                    lines.push(format!("║   Default: {:10} ICMP: {:5} Forward: {:5}                 ║",
                        zone.default_action.name(),
                        if zone.allow_icmp { "Yes" } else { "No" },
                        if zone.forward { "Yes" } else { "No" }));
                }
            }
            FirewallView::Services => {
                lines.push(format!("║ PREDEFINED SERVICES                                               ║"));
                lines.push(format!("╟───────────────────────────────────────────────────────────────────╢"));

                for service in self.services() {
                    let ports_str: Vec<String> = service.ports.iter().map(|p| format!("{}", p)).collect();
                    lines.push(format!("║ {:15} {:5} {:20} {:20} ║",
                        service.name,
                        service.protocol.name(),
                        ports_str.join(","),
                        if service.description.len() > 18 { &service.description[..18] } else { &service.description }));
                }
            }
            FirewallView::Connections => {
                lines.push(format!("║ ACTIVE CONNECTIONS ({})                                          ║",
                    self.connections().len()));
                lines.push(format!("╟───────────────────────────────────────────────────────────────────╢"));

                for conn in self.connections() {
                    lines.push(format!("║ {}║", Self::format_connection(conn)));
                }
            }
            FirewallView::Logs => {
                lines.push(format!("║ FIREWALL LOGS ({} entries)                                       ║",
                    self.logs.len()));
                lines.push(format!("╟───────────────────────────────────────────────────────────────────╢"));

                for log in self.logs.iter().rev().take(10) {
                    lines.push(format!("║ {} {:8} {} {}:{} -> {}:{}                 ║",
                        log.format_time(),
                        log.action.name(),
                        log.protocol.name(),
                        log.src_addr,
                        log.src_port,
                        log.dst_addr,
                        log.dst_port));
                }
            }
            FirewallView::Settings => {
                let stats = self.stats();
                lines.push(format!("║ FIREWALL SETTINGS & STATISTICS                                   ║"));
                lines.push(format!("╟───────────────────────────────────────────────────────────────────╢"));
                lines.push(format!("║ Firewall: {:10}                                              ║",
                    if self.is_enabled() { "ENABLED" } else { "DISABLED" }));
                lines.push(format!("║                                                                   ║"));
                lines.push(format!("║ Statistics:                                                       ║"));
                lines.push(format!("║   Packets Accepted: {:10}   Bytes: {:15}             ║",
                    stats.packets_accepted, stats.bytes_accepted));
                lines.push(format!("║   Packets Dropped:  {:10}   Bytes: {:15}             ║",
                    stats.packets_dropped, stats.bytes_dropped));
                lines.push(format!("║   Packets Rejected: {:10}                                     ║",
                    stats.packets_rejected));
                lines.push(format!("║   Active Connections: {:8}                                     ║",
                    stats.active_connections));
                lines.push(format!("║   Total Rules: {:8}  Enabled: {:8}                         ║",
                    stats.total_rules, stats.enabled_rules));
            }
        }

        lines.push(format!("╟───────────────────────────────────────────────────────────────────╢"));
        lines.push(format!("║ [N]ew Rule  [D]elete  [E]nable/Disable  [F]irewall Toggle        ║"));
        lines.push(format!("╚═══════════════════════════════════════════════════════════════════╝"));

        lines
    }
}

impl Default for FirewallApp {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize firewall GUI module
pub fn init() -> FirewallApp {
    FirewallApp::new()
}
