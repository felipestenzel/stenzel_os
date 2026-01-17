//! Network Indicator Widget
//!
//! A popup widget for displaying network status, connections,
//! and network configuration options.

use alloc::string::String;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetState, WidgetEvent, Bounds, MouseButton, theme};

/// Network connection type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    /// Wired Ethernet connection
    Ethernet,
    /// Wireless WiFi connection
    WiFi,
    /// Cellular/Mobile data
    Cellular,
    /// VPN connection
    Vpn,
}

/// Connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Not connected
    Disconnected,
    /// Connection in progress
    Connecting,
    /// Connected successfully
    Connected,
    /// Connection failed
    Failed,
    /// Limited connectivity (no internet)
    Limited,
}

/// WiFi signal strength (0-4 bars)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalStrength {
    None,
    Weak,     // 1 bar
    Fair,     // 2 bars
    Good,     // 3 bars
    Excellent, // 4 bars
}

impl SignalStrength {
    /// From RSSI dBm value
    pub fn from_rssi(rssi: i32) -> Self {
        match rssi {
            _ if rssi >= -50 => SignalStrength::Excellent,
            _ if rssi >= -60 => SignalStrength::Good,
            _ if rssi >= -70 => SignalStrength::Fair,
            _ if rssi >= -80 => SignalStrength::Weak,
            _ => SignalStrength::None,
        }
    }

    /// Get number of bars
    pub fn bars(&self) -> u8 {
        match self {
            SignalStrength::None => 0,
            SignalStrength::Weak => 1,
            SignalStrength::Fair => 2,
            SignalStrength::Good => 3,
            SignalStrength::Excellent => 4,
        }
    }
}

/// Network interface
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    /// Interface name (e.g., "eth0", "wlan0")
    pub name: String,
    /// Display name (e.g., "Ethernet", "WiFi")
    pub display_name: String,
    /// Connection type
    pub conn_type: ConnectionType,
    /// Current status
    pub status: ConnectionStatus,
    /// IP address (if connected)
    pub ip_address: Option<String>,
    /// MAC address
    pub mac_address: String,
    /// Signal strength (for wireless)
    pub signal: SignalStrength,
    /// Connection name/SSID (for WiFi)
    pub connection_name: Option<String>,
    /// Speed in Mbps
    pub speed: Option<u32>,
    /// Data usage (bytes)
    pub data_rx: u64,
    pub data_tx: u64,
}

impl NetworkInterface {
    /// Create a new network interface
    pub fn new(name: &str, display_name: &str, conn_type: ConnectionType) -> Self {
        Self {
            name: String::from(name),
            display_name: String::from(display_name),
            conn_type,
            status: ConnectionStatus::Disconnected,
            ip_address: None,
            mac_address: String::from("00:00:00:00:00:00"),
            signal: SignalStrength::None,
            connection_name: None,
            speed: None,
            data_rx: 0,
            data_tx: 0,
        }
    }

    /// Set connected state
    pub fn set_connected(&mut self, ip: &str, name: Option<&str>) {
        self.status = ConnectionStatus::Connected;
        self.ip_address = Some(String::from(ip));
        self.connection_name = name.map(String::from);
    }

    /// Set disconnected state
    pub fn set_disconnected(&mut self) {
        self.status = ConnectionStatus::Disconnected;
        self.ip_address = None;
        self.connection_name = None;
        self.signal = SignalStrength::None;
    }
}

/// Available WiFi network
#[derive(Debug, Clone)]
pub struct WiFiNetwork {
    /// Network SSID
    pub ssid: String,
    /// Signal strength
    pub signal: SignalStrength,
    /// Whether network is secured
    pub secured: bool,
    /// Whether currently connected
    pub connected: bool,
    /// Whether this is a known network
    pub known: bool,
}

impl WiFiNetwork {
    /// Create a new WiFi network entry
    pub fn new(ssid: &str, signal: SignalStrength, secured: bool) -> Self {
        Self {
            ssid: String::from(ssid),
            signal,
            secured,
            connected: false,
            known: false,
        }
    }
}

/// Network click callback
pub type NetworkCallback = fn(NetworkAction);

/// Network actions
#[derive(Debug, Clone)]
pub enum NetworkAction {
    /// Connect to a WiFi network
    ConnectWiFi(String),
    /// Disconnect from current network
    Disconnect(String),
    /// Open network settings
    OpenSettings,
    /// Toggle airplane mode
    ToggleAirplaneMode,
    /// Refresh network list
    Refresh,
}

/// Network indicator popup widget
pub struct NetworkIndicator {
    id: WidgetId,
    bounds: Bounds,

    /// Network interfaces
    interfaces: Vec<NetworkInterface>,
    /// Available WiFi networks
    wifi_networks: Vec<WiFiNetwork>,
    /// Airplane mode enabled
    airplane_mode: bool,

    /// Whether popup is visible
    visible: bool,
    /// Widget state
    state: WidgetState,
    /// Hovered item
    hovered_item: Option<usize>,
    /// Expanded interface index
    expanded_interface: Option<usize>,
    /// Show WiFi networks
    show_wifi_list: bool,

    /// Callback for actions
    on_action: Option<NetworkCallback>,

    /// Colors
    bg_color: Color,
    text_color: Color,
    accent_color: Color,
    connected_color: Color,
    disconnected_color: Color,
    hover_color: Color,
}

impl NetworkIndicator {
    /// Popup dimensions
    const WIDTH: usize = 320;
    const BASE_HEIGHT: usize = 200;
    const INTERFACE_HEIGHT: usize = 56;
    const WIFI_ITEM_HEIGHT: usize = 40;
    const PADDING: usize = 16;

    /// Create a new network indicator
    pub fn new(x: isize, y: isize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, Self::WIDTH, Self::BASE_HEIGHT),
            interfaces: Vec::new(),
            wifi_networks: Vec::new(),
            airplane_mode: false,
            visible: false,
            state: WidgetState::Normal,
            hovered_item: None,
            expanded_interface: None,
            show_wifi_list: false,
            on_action: None,
            bg_color: Color::new(40, 40, 48),
            text_color: Color::WHITE,
            accent_color: Color::new(0, 120, 215),
            connected_color: Color::new(100, 255, 100),
            disconnected_color: Color::new(150, 150, 160),
            hover_color: Color::new(60, 60, 70),
        }
    }

    /// Show the popup
    pub fn show(&mut self) {
        self.visible = true;
        self.update_height();
    }

    /// Hide the popup
    pub fn hide(&mut self) {
        self.visible = false;
        self.show_wifi_list = false;
    }

    /// Toggle visibility
    pub fn toggle(&mut self) {
        if self.visible {
            self.hide();
        } else {
            self.show();
        }
    }

    /// Add a network interface
    pub fn add_interface(&mut self, interface: NetworkInterface) {
        self.interfaces.push(interface);
        self.update_height();
    }

    /// Remove an interface
    pub fn remove_interface(&mut self, name: &str) {
        self.interfaces.retain(|i| i.name != name);
        self.update_height();
    }

    /// Update interface by name
    pub fn update_interface(&mut self, name: &str, update: impl FnOnce(&mut NetworkInterface)) {
        if let Some(iface) = self.interfaces.iter_mut().find(|i| i.name == name) {
            update(iface);
        }
    }

    /// Set WiFi networks list
    pub fn set_wifi_networks(&mut self, networks: Vec<WiFiNetwork>) {
        self.wifi_networks = networks;
        self.update_height();
    }

    /// Add a WiFi network
    pub fn add_wifi_network(&mut self, network: WiFiNetwork) {
        // Update if exists, otherwise add
        if let Some(existing) = self.wifi_networks.iter_mut().find(|n| n.ssid == network.ssid) {
            existing.signal = network.signal;
            existing.connected = network.connected;
        } else {
            self.wifi_networks.push(network);
        }
        self.update_height();
    }

    /// Toggle airplane mode
    pub fn toggle_airplane_mode(&mut self) {
        self.airplane_mode = !self.airplane_mode;
        if let Some(callback) = self.on_action {
            callback(NetworkAction::ToggleAirplaneMode);
        }
    }

    /// Check if airplane mode is on
    pub fn is_airplane_mode(&self) -> bool {
        self.airplane_mode
    }

    /// Set action callback
    pub fn set_on_action(&mut self, callback: NetworkCallback) {
        self.on_action = Some(callback);
    }

    /// Toggle WiFi list visibility
    pub fn toggle_wifi_list(&mut self) {
        self.show_wifi_list = !self.show_wifi_list;
        self.update_height();
    }

    /// Get overall connection status
    pub fn overall_status(&self) -> ConnectionStatus {
        // Return best status among all interfaces
        for iface in &self.interfaces {
            if iface.status == ConnectionStatus::Connected {
                return ConnectionStatus::Connected;
            }
        }
        for iface in &self.interfaces {
            if iface.status == ConnectionStatus::Limited {
                return ConnectionStatus::Limited;
            }
            if iface.status == ConnectionStatus::Connecting {
                return ConnectionStatus::Connecting;
            }
        }
        ConnectionStatus::Disconnected
    }

    /// Update height based on content
    fn update_height(&mut self) {
        let mut height = 60; // Header

        // Interfaces
        height += self.interfaces.len() * Self::INTERFACE_HEIGHT;

        // WiFi list if shown
        if self.show_wifi_list {
            height += 40 + self.wifi_networks.len().min(5) * Self::WIFI_ITEM_HEIGHT;
        }

        // Bottom actions
        height += 48;

        self.bounds.height = height.max(Self::BASE_HEIGHT);
    }

    /// Get interface item bounds
    fn interface_bounds(&self, index: usize) -> Bounds {
        let x = self.bounds.x + Self::PADDING as isize;
        let y = self.bounds.y + 50 + (index * Self::INTERFACE_HEIGHT) as isize;
        let width = self.bounds.width - Self::PADDING * 2;
        Bounds::new(x, y, width, Self::INTERFACE_HEIGHT - 4)
    }

    /// Get WiFi network item bounds
    fn wifi_bounds(&self, index: usize) -> Bounds {
        let base_y = 50 + self.interfaces.len() * Self::INTERFACE_HEIGHT + 40;
        let x = self.bounds.x + Self::PADDING as isize + 8;
        let y = self.bounds.y + base_y as isize + (index * Self::WIFI_ITEM_HEIGHT) as isize;
        let width = self.bounds.width - Self::PADDING * 2 - 16;
        Bounds::new(x, y, width, Self::WIFI_ITEM_HEIGHT - 4)
    }

    /// Draw connection type icon
    fn draw_connection_icon(&self, surface: &mut Surface, x: usize, y: usize,
                            conn_type: ConnectionType, status: ConnectionStatus, signal: SignalStrength) {
        let color = match status {
            ConnectionStatus::Connected => self.connected_color,
            ConnectionStatus::Connecting => self.accent_color,
            _ => self.disconnected_color,
        };

        match conn_type {
            ConnectionType::Ethernet => {
                // Computer/network icon
                for px in 2..14 {
                    surface.set_pixel(x + px, y + 2, color);
                    surface.set_pixel(x + px, y + 10, color);
                }
                for py in 2..11 {
                    surface.set_pixel(x + 2, y + py, color);
                    surface.set_pixel(x + 13, y + py, color);
                }
                // Stand
                for px in 6..10 {
                    surface.set_pixel(x + px, y + 11, color);
                    surface.set_pixel(x + px, y + 12, color);
                }
                for px in 4..12 {
                    surface.set_pixel(x + px, y + 13, color);
                }
            }
            ConnectionType::WiFi => {
                // WiFi arcs
                let cx = x + 8;
                let base_y = y + 14;

                // Dot
                surface.set_pixel(cx, base_y, color);
                surface.set_pixel(cx - 1, base_y, color);
                surface.set_pixel(cx + 1, base_y, color);

                let bars = signal.bars();

                // Arcs based on signal
                for arc in 1usize..=3 {
                    let arc_color = if (arc as u8) <= bars {
                        color
                    } else {
                        Color::new(60, 60, 68)
                    };

                    for i in 0usize..3 {
                        let offset = arc * 2;
                        surface.set_pixel(cx - offset - i, base_y - offset - i, arc_color);
                        surface.set_pixel(cx + offset + i, base_y - offset - i, arc_color);
                    }
                }
            }
            ConnectionType::Cellular => {
                // Signal bars
                let bars = signal.bars();
                for bar in 0usize..4 {
                    let bar_color = if (bar as u8) < bars {
                        color
                    } else {
                        Color::new(60, 60, 68)
                    };
                    let bar_height = 4 + bar * 3;
                    let bar_x = x + 2 + bar * 4;
                    let bar_y = y + 14 - bar_height;
                    for py in 0..bar_height {
                        for px in 0..3 {
                            surface.set_pixel(bar_x + px, bar_y + py, bar_color);
                        }
                    }
                }
            }
            ConnectionType::Vpn => {
                // Shield icon
                for px in 4..12 {
                    surface.set_pixel(x + px, y + 2, color);
                }
                for py in 2..10 {
                    surface.set_pixel(x + 4, y + py, color);
                    surface.set_pixel(x + 11, y + py, color);
                }
                surface.set_pixel(x + 5, y + 10, color);
                surface.set_pixel(x + 10, y + 10, color);
                surface.set_pixel(x + 6, y + 11, color);
                surface.set_pixel(x + 9, y + 11, color);
                surface.set_pixel(x + 7, y + 12, color);
                surface.set_pixel(x + 8, y + 12, color);
                // Lock icon inside
                surface.set_pixel(x + 7, y + 5, color);
                surface.set_pixel(x + 8, y + 5, color);
                surface.set_pixel(x + 7, y + 6, color);
                surface.set_pixel(x + 8, y + 6, color);
            }
        }
    }

    /// Draw signal strength bars
    fn draw_signal_bars(&self, surface: &mut Surface, x: usize, y: usize, signal: SignalStrength) {
        let bars = signal.bars();
        for bar in 0..4u8 {
            let bar_color = if bar < bars {
                self.connected_color
            } else {
                Color::new(60, 60, 68)
            };
            let bar_height = 3 + (bar as usize) * 2;
            let bar_y = y + 10 - bar_height;
            for py in 0..bar_height {
                for px in 0..2 {
                    surface.set_pixel(x + (bar as usize) * 4 + px, bar_y + py, bar_color);
                }
            }
        }
    }
}

impl Widget for NetworkIndicator {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn bounds(&self) -> Bounds {
        self.bounds
    }

    fn set_position(&mut self, x: isize, y: isize) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn set_size(&mut self, width: usize, height: usize) {
        self.bounds.width = width;
        self.bounds.height = height;
    }

    fn is_enabled(&self) -> bool {
        true
    }

    fn set_enabled(&mut self, _enabled: bool) {}

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        if !self.visible {
            return false;
        }

        match event {
            WidgetEvent::MouseMove { x, y } => {
                // Check interfaces
                for i in 0..self.interfaces.len() {
                    if self.interface_bounds(i).contains(*x, *y) {
                        self.hovered_item = Some(i);
                        return true;
                    }
                }

                // Check WiFi networks
                if self.show_wifi_list {
                    for i in 0..self.wifi_networks.len().min(5) {
                        if self.wifi_bounds(i).contains(*x, *y) {
                            self.hovered_item = Some(100 + i);
                            return true;
                        }
                    }
                }

                self.hovered_item = None;
                true
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                // Check interfaces
                for i in 0..self.interfaces.len() {
                    if self.interface_bounds(i).contains(*x, *y) {
                        let iface = &self.interfaces[i];
                        if iface.conn_type == ConnectionType::WiFi {
                            self.toggle_wifi_list();
                        } else if iface.status == ConnectionStatus::Connected {
                            if let Some(callback) = self.on_action {
                                callback(NetworkAction::Disconnect(iface.name.clone()));
                            }
                        }
                        return true;
                    }
                }

                // Check WiFi networks
                if self.show_wifi_list {
                    for i in 0..self.wifi_networks.len().min(5) {
                        if self.wifi_bounds(i).contains(*x, *y) {
                            let network = &self.wifi_networks[i];
                            if !network.connected {
                                if let Some(callback) = self.on_action {
                                    callback(NetworkAction::ConnectWiFi(network.ssid.clone()));
                                }
                            }
                            return true;
                        }
                    }
                }

                // Click outside to close
                if !self.bounds.contains(*x, *y) {
                    self.hide();
                }

                true
            }
            WidgetEvent::Blur => {
                self.hide();
                true
            }
            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        // Background
        for py in 0..h {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, self.bg_color);
            }
        }

        // Border
        let border_color = Color::new(80, 80, 90);
        for px in 0..w {
            surface.set_pixel(x + px, y, border_color);
            surface.set_pixel(x + px, y + h - 1, border_color);
        }
        for py in 0..h {
            surface.set_pixel(x, y + py, border_color);
            surface.set_pixel(x + w - 1, y + py, border_color);
        }

        // Title
        let title = "Network";
        let title_x = x + Self::PADDING;
        let title_y = y + 14;
        for (i, c) in title.chars().enumerate() {
            draw_char(surface, title_x + i * 8, title_y, c, self.text_color);
        }

        // Status indicator
        let status = self.overall_status();
        let status_text = match status {
            ConnectionStatus::Connected => "Connected",
            ConnectionStatus::Connecting => "Connecting...",
            ConnectionStatus::Limited => "Limited",
            ConnectionStatus::Failed => "Failed",
            ConnectionStatus::Disconnected => "Disconnected",
        };
        let status_color = match status {
            ConnectionStatus::Connected => self.connected_color,
            ConnectionStatus::Connecting => self.accent_color,
            _ => self.disconnected_color,
        };
        let status_x = x + Self::PADDING;
        let status_y = y + 30;
        for (i, c) in status_text.chars().enumerate() {
            draw_char(surface, status_x + i * 8, status_y, c, status_color);
        }

        // Interfaces
        for (i, iface) in self.interfaces.iter().enumerate() {
            let ib = self.interface_bounds(i);
            let ix = ib.x.max(0) as usize;
            let iy = ib.y.max(0) as usize;

            // Hover highlight
            if self.hovered_item == Some(i) {
                for py in 0..ib.height {
                    for px in 0..ib.width {
                        surface.set_pixel(ix + px, iy + py, self.hover_color);
                    }
                }
            }

            // Connection icon
            self.draw_connection_icon(surface, ix + 4, iy + 8, iface.conn_type, iface.status, iface.signal);

            // Interface name
            let name_x = ix + 28;
            let name_y = iy + 8;
            for (ci, c) in iface.display_name.chars().take(20).enumerate() {
                draw_char(surface, name_x + ci * 8, name_y, c, self.text_color);
            }

            // Connection name / status
            let detail = if let Some(ref conn_name) = iface.connection_name {
                conn_name.as_str()
            } else {
                match iface.status {
                    ConnectionStatus::Disconnected => "Not connected",
                    ConnectionStatus::Connecting => "Connecting...",
                    ConnectionStatus::Connected => "Connected",
                    ConnectionStatus::Limited => "Limited connectivity",
                    ConnectionStatus::Failed => "Connection failed",
                }
            };
            let detail_y = iy + 24;
            let detail_color = if iface.status == ConnectionStatus::Connected {
                Color::new(180, 180, 190)
            } else {
                self.disconnected_color
            };
            for (ci, c) in detail.chars().take(30).enumerate() {
                draw_char(surface, name_x + ci * 8, detail_y, c, detail_color);
            }

            // IP address if connected
            if let Some(ref ip) = iface.ip_address {
                let ip_x = ix + ib.width - ip.len() * 8 - 8;
                for (ci, c) in ip.chars().enumerate() {
                    draw_char(surface, ip_x + ci * 8, name_y, c, Color::new(150, 150, 160));
                }
            }

            // Signal for wireless
            if iface.conn_type == ConnectionType::WiFi && iface.status == ConnectionStatus::Connected {
                self.draw_signal_bars(surface, ix + ib.width - 24, iy + 30, iface.signal);
            }

            // Expand arrow for WiFi
            if iface.conn_type == ConnectionType::WiFi {
                let arrow_x = ix + ib.width - 16;
                let arrow_y = iy + ib.height / 2;
                let arrow_color = self.text_color;
                if self.show_wifi_list {
                    // Up arrow
                    surface.set_pixel(arrow_x + 4, arrow_y - 2, arrow_color);
                    surface.set_pixel(arrow_x + 3, arrow_y - 1, arrow_color);
                    surface.set_pixel(arrow_x + 5, arrow_y - 1, arrow_color);
                    surface.set_pixel(arrow_x + 2, arrow_y, arrow_color);
                    surface.set_pixel(arrow_x + 6, arrow_y, arrow_color);
                } else {
                    // Down arrow
                    surface.set_pixel(arrow_x + 2, arrow_y - 2, arrow_color);
                    surface.set_pixel(arrow_x + 6, arrow_y - 2, arrow_color);
                    surface.set_pixel(arrow_x + 3, arrow_y - 1, arrow_color);
                    surface.set_pixel(arrow_x + 5, arrow_y - 1, arrow_color);
                    surface.set_pixel(arrow_x + 4, arrow_y, arrow_color);
                }
            }
        }

        // WiFi network list
        if self.show_wifi_list && !self.wifi_networks.is_empty() {
            let list_y = y + 50 + self.interfaces.len() * Self::INTERFACE_HEIGHT;

            // Divider
            for px in Self::PADDING..(w - Self::PADDING) {
                surface.set_pixel(x + px, list_y, border_color);
            }

            // "Available networks" label
            let label = "Available Networks";
            let label_y = list_y + 12;
            for (i, c) in label.chars().enumerate() {
                draw_char(surface, x + Self::PADDING + 8 + i * 8, label_y, c, Color::new(150, 150, 160));
            }

            // Network list
            for (i, network) in self.wifi_networks.iter().take(5).enumerate() {
                let nb = self.wifi_bounds(i);
                let nx = nb.x.max(0) as usize;
                let ny = nb.y.max(0) as usize;

                // Hover highlight
                if self.hovered_item == Some(100 + i) {
                    for py in 0..nb.height {
                        for px in 0..nb.width {
                            surface.set_pixel(nx + px, ny + py, self.hover_color);
                        }
                    }
                }

                // Connected indicator
                if network.connected {
                    for py in 4..nb.height - 4 {
                        surface.set_pixel(nx + 2, ny + py, self.connected_color);
                        surface.set_pixel(nx + 3, ny + py, self.connected_color);
                    }
                }

                // Signal bars
                self.draw_signal_bars(surface, nx + 8, ny + 12, network.signal);

                // SSID
                let ssid_x = nx + 28;
                let ssid_y = ny + 8;
                for (ci, c) in network.ssid.chars().take(25).enumerate() {
                    draw_char(surface, ssid_x + ci * 8, ssid_y, c, self.text_color);
                }

                // Lock icon if secured
                if network.secured {
                    let lock_x = nx + nb.width - 20;
                    let lock_y = ny + 10;
                    // Simple lock shape
                    for px in 2..6 {
                        surface.set_pixel(lock_x + px, lock_y, self.text_color);
                        surface.set_pixel(lock_x + px, lock_y + 5, self.text_color);
                    }
                    for py in 0..6 {
                        surface.set_pixel(lock_x + 2, lock_y + py, self.text_color);
                        surface.set_pixel(lock_x + 5, lock_y + py, self.text_color);
                    }
                    // Shackle
                    surface.set_pixel(lock_x + 3, lock_y - 1, self.text_color);
                    surface.set_pixel(lock_x + 4, lock_y - 1, self.text_color);
                    surface.set_pixel(lock_x + 3, lock_y - 2, self.text_color);
                    surface.set_pixel(lock_x + 4, lock_y - 2, self.text_color);
                }

                // "Known" indicator
                if network.known {
                    let known_y = ny + 22;
                    let known_text = "Known network";
                    for (ci, c) in known_text.chars().enumerate() {
                        draw_char(surface, ssid_x + ci * 8, known_y, c, Color::new(120, 120, 130));
                    }
                }
            }
        }

        // Bottom actions area
        let actions_y = y + h - 44;

        // Divider
        for px in Self::PADDING..(w - Self::PADDING) {
            surface.set_pixel(x + px, actions_y, border_color);
        }

        // Airplane mode toggle
        let airplane_x = x + Self::PADDING;
        let airplane_y = actions_y + 12;
        let airplane_text = if self.airplane_mode { "Airplane mode: On" } else { "Airplane mode: Off" };
        let airplane_color = if self.airplane_mode { self.accent_color } else { self.text_color };
        for (i, c) in airplane_text.chars().enumerate() {
            draw_char(surface, airplane_x + i * 8, airplane_y, c, airplane_color);
        }

        // Settings link
        let settings_text = "Network settings";
        let settings_x = x + w - Self::PADDING - settings_text.len() * 8;
        for (i, c) in settings_text.chars().enumerate() {
            draw_char(surface, settings_x + i * 8, airplane_y, c, self.accent_color);
        }
    }
}

// Helper function
fn draw_char(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;

    if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
        for row in 0..DEFAULT_FONT.height {
            let byte = glyph[row];
            for col in 0..DEFAULT_FONT.width {
                if (byte >> (DEFAULT_FONT.width - 1 - col)) & 1 != 0 {
                    surface.set_pixel(x + col, y + row, color);
                }
            }
        }
    }
}

/// Global network state
use spin::Mutex;

static NETWORK_STATE: Mutex<NetworkState> = Mutex::new(NetworkState::new());

struct NetworkState {
    connected: bool,
    airplane_mode: bool,
}

impl NetworkState {
    const fn new() -> Self {
        Self {
            connected: false,
            airplane_mode: false,
        }
    }
}

/// Check if network is connected
pub fn is_connected() -> bool {
    NETWORK_STATE.lock().connected
}

/// Set network connected state
pub fn set_connected(connected: bool) {
    NETWORK_STATE.lock().connected = connected;
}

/// Check airplane mode
pub fn is_airplane_mode() -> bool {
    NETWORK_STATE.lock().airplane_mode
}

/// Set airplane mode
pub fn set_airplane_mode(enabled: bool) {
    NETWORK_STATE.lock().airplane_mode = enabled;
}
