//! Audio Daemon (PipeWire-like)
//!
//! Central audio management system that handles:
//! - Audio routing between applications and devices
//! - Sample rate conversion
//! - Session management
//! - Device hotplug handling
//! - Client connections (PulseAudio, ALSA, JACK compatible)

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;

use super::mixer::{AudioMixer, ChannelId, ChannelType};
use super::{AudioConfig, SampleFormat, StreamDirection};

/// Unique node ID
pub type NodeId = u32;

/// Unique port ID
pub type PortId = u32;

/// Unique link ID
pub type LinkId = u32;

/// Unique client ID
pub type ClientId = u32;

/// Audio node type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    /// Audio source (e.g., application playback)
    Source,
    /// Audio sink (e.g., speakers, headphones)
    Sink,
    /// Audio filter/effect
    Filter,
    /// Audio monitor (loopback)
    Monitor,
    /// Virtual device
    Virtual,
}

/// Port direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection {
    Input,
    Output,
}

/// Port media type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Audio,
    Video,
    Midi,
}

/// Audio format info
#[derive(Debug, Clone)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub format: SampleFormat,
    pub channels: u8,
    pub channel_map: Vec<ChannelPosition>,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            format: SampleFormat::F32LE,
            channels: 2,
            channel_map: vec![ChannelPosition::FrontLeft, ChannelPosition::FrontRight],
        }
    }
}

/// Speaker/channel position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelPosition {
    Mono,
    FrontLeft,
    FrontRight,
    FrontCenter,
    Lfe, // Low Frequency Effects (subwoofer)
    RearLeft,
    RearRight,
    RearCenter,
    SideLeft,
    SideRight,
    TopCenter,
    TopFrontLeft,
    TopFrontRight,
    TopRearLeft,
    TopRearRight,
    Aux(u8),
}

/// Audio port
pub struct Port {
    id: PortId,
    node_id: NodeId,
    name: String,
    direction: PortDirection,
    media_type: MediaType,
    format: AudioFormat,
    linked_to: Option<PortId>,
    buffer: IrqSafeMutex<Vec<f32>>,
}

impl Port {
    pub fn new(
        id: PortId,
        node_id: NodeId,
        name: &str,
        direction: PortDirection,
        media_type: MediaType,
    ) -> Self {
        Self {
            id,
            node_id,
            name: name.to_string(),
            direction,
            media_type,
            format: AudioFormat::default(),
            linked_to: None,
            buffer: IrqSafeMutex::new(vec![0.0; 1024]),
        }
    }

    pub fn id(&self) -> PortId {
        self.id
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn direction(&self) -> PortDirection {
        self.direction
    }

    pub fn format(&self) -> &AudioFormat {
        &self.format
    }

    pub fn set_format(&mut self, format: AudioFormat) {
        self.format = format;
    }

    pub fn is_linked(&self) -> bool {
        self.linked_to.is_some()
    }

    pub fn linked_port(&self) -> Option<PortId> {
        self.linked_to
    }

    pub fn write_samples(&self, samples: &[f32]) -> usize {
        let mut buffer = self.buffer.lock();
        let to_write = samples.len().min(buffer.len());
        buffer[..to_write].copy_from_slice(&samples[..to_write]);
        to_write
    }

    pub fn read_samples(&self, output: &mut [f32]) -> usize {
        let buffer = self.buffer.lock();
        let to_read = output.len().min(buffer.len());
        output[..to_read].copy_from_slice(&buffer[..to_read]);
        to_read
    }
}

/// Audio node (device, application, or filter)
pub struct Node {
    id: NodeId,
    name: String,
    node_type: NodeType,
    client_id: Option<ClientId>,
    input_ports: Vec<PortId>,
    output_ports: Vec<PortId>,
    active: AtomicBool,
    volume: AtomicU32,   // 0-100
    muted: AtomicBool,
    latency_ns: AtomicU64,
    properties: BTreeMap<String, String>,
}

impl Node {
    pub fn new(id: NodeId, name: &str, node_type: NodeType) -> Self {
        Self {
            id,
            name: name.to_string(),
            node_type,
            client_id: None,
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            active: AtomicBool::new(false),
            volume: AtomicU32::new(100),
            muted: AtomicBool::new(false),
            latency_ns: AtomicU64::new(0),
            properties: BTreeMap::new(),
        }
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn node_type(&self) -> NodeType {
        self.node_type
    }

    pub fn set_client(&mut self, client_id: ClientId) {
        self.client_id = Some(client_id);
    }

    pub fn client_id(&self) -> Option<ClientId> {
        self.client_id
    }

    pub fn add_input_port(&mut self, port_id: PortId) {
        self.input_ports.push(port_id);
    }

    pub fn add_output_port(&mut self, port_id: PortId) {
        self.output_ports.push(port_id);
    }

    pub fn input_ports(&self) -> &[PortId] {
        &self.input_ports
    }

    pub fn output_ports(&self) -> &[PortId] {
        &self.output_ports
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
    }

    pub fn volume(&self) -> u8 {
        self.volume.load(Ordering::Relaxed) as u8
    }

    pub fn set_volume(&self, volume: u8) {
        self.volume.store(volume.min(100) as u32, Ordering::Relaxed);
    }

    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    pub fn latency_ns(&self) -> u64 {
        self.latency_ns.load(Ordering::Relaxed)
    }

    pub fn set_latency_ns(&self, latency: u64) {
        self.latency_ns.store(latency, Ordering::Relaxed);
    }

    pub fn set_property(&mut self, key: &str, value: &str) {
        self.properties.insert(key.to_string(), value.to_string());
    }

    pub fn get_property(&self, key: &str) -> Option<&String> {
        self.properties.get(key)
    }
}

/// Link between two ports
pub struct Link {
    id: LinkId,
    output_port: PortId,
    input_port: PortId,
    active: AtomicBool,
    passthrough: bool,
}

impl Link {
    pub fn new(id: LinkId, output_port: PortId, input_port: PortId) -> Self {
        Self {
            id,
            output_port,
            input_port,
            active: AtomicBool::new(true),
            passthrough: false,
        }
    }

    pub fn id(&self) -> LinkId {
        self.id
    }

    pub fn output_port(&self) -> PortId {
        self.output_port
    }

    pub fn input_port(&self) -> PortId {
        self.input_port
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
    }
}

/// Audio client (application connected to the daemon)
pub struct Client {
    id: ClientId,
    name: String,
    pid: Option<u32>,
    nodes: Vec<NodeId>,
    permissions: ClientPermissions,
    protocol: ClientProtocol,
}

impl Client {
    pub fn new(id: ClientId, name: &str, protocol: ClientProtocol) -> Self {
        Self {
            id,
            name: name.to_string(),
            pid: None,
            nodes: Vec::new(),
            permissions: ClientPermissions::default(),
            protocol,
        }
    }

    pub fn id(&self) -> ClientId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_pid(&mut self, pid: u32) {
        self.pid = Some(pid);
    }

    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    pub fn protocol(&self) -> ClientProtocol {
        self.protocol
    }

    pub fn add_node(&mut self, node_id: NodeId) {
        self.nodes.push(node_id);
    }

    pub fn nodes(&self) -> &[NodeId] {
        &self.nodes
    }

    pub fn permissions(&self) -> &ClientPermissions {
        &self.permissions
    }

    pub fn set_permissions(&mut self, permissions: ClientPermissions) {
        self.permissions = permissions;
    }
}

/// Client permissions
#[derive(Debug, Clone, Default)]
pub struct ClientPermissions {
    pub can_capture: bool,
    pub can_playback: bool,
    pub can_manage_devices: bool,
    pub can_configure_routing: bool,
}

/// Client protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientProtocol {
    Native,      // Native daemon protocol
    PulseAudio,  // PulseAudio compatibility
    Alsa,        // ALSA compatibility
    Jack,        // JACK compatibility
}

/// Audio session (group of related streams)
pub struct Session {
    id: u32,
    name: String,
    client_id: ClientId,
    nodes: Vec<NodeId>,
    volume: AtomicU32,
    muted: AtomicBool,
}

impl Session {
    pub fn new(id: u32, name: &str, client_id: ClientId) -> Self {
        Self {
            id,
            name: name.to_string(),
            client_id,
            nodes: Vec::new(),
            volume: AtomicU32::new(100),
            muted: AtomicBool::new(false),
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn client_id(&self) -> ClientId {
        self.client_id
    }

    pub fn add_node(&mut self, node_id: NodeId) {
        self.nodes.push(node_id);
    }

    pub fn nodes(&self) -> &[NodeId] {
        &self.nodes
    }

    pub fn volume(&self) -> u8 {
        self.volume.load(Ordering::Relaxed) as u8
    }

    pub fn set_volume(&self, volume: u8) {
        self.volume.store(volume.min(100) as u32, Ordering::Relaxed);
    }

    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }
}

/// Audio device info
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub id: NodeId,
    pub name: String,
    pub description: String,
    pub device_class: DeviceClass,
    pub direction: StreamDirection,
    pub channels: u8,
    pub sample_rate: u32,
    pub is_default: bool,
    pub is_available: bool,
}

/// Device class
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    Speakers,
    Headphones,
    Hdmi,
    Bluetooth,
    Usb,
    Microphone,
    Webcam,
    Virtual,
    Unknown,
}

/// Stream info for applications
#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub id: NodeId,
    pub client_name: String,
    pub stream_name: String,
    pub direction: StreamDirection,
    pub volume: u8,
    pub muted: bool,
    pub device_id: Option<NodeId>,
}

/// Audio daemon events
#[derive(Debug, Clone)]
pub enum DaemonEvent {
    NodeAdded(NodeId),
    NodeRemoved(NodeId),
    LinkCreated(LinkId),
    LinkDestroyed(LinkId),
    ClientConnected(ClientId),
    ClientDisconnected(ClientId),
    DeviceAdded(DeviceInfo),
    DeviceRemoved(NodeId),
    DefaultDeviceChanged(StreamDirection, NodeId),
    VolumeChanged(NodeId, u8),
    MuteChanged(NodeId, bool),
}

/// Event callback type
pub type EventCallback = fn(DaemonEvent);

/// Audio Daemon - Central audio management
pub struct AudioDaemon {
    /// Daemon name
    name: String,
    /// All nodes
    nodes: BTreeMap<NodeId, Node>,
    /// All ports
    ports: BTreeMap<PortId, Port>,
    /// All links
    links: BTreeMap<LinkId, Link>,
    /// Connected clients
    clients: BTreeMap<ClientId, Client>,
    /// Audio sessions
    sessions: BTreeMap<u32, Session>,
    /// Next node ID
    next_node_id: AtomicU32,
    /// Next port ID
    next_port_id: AtomicU32,
    /// Next link ID
    next_link_id: AtomicU32,
    /// Next client ID
    next_client_id: AtomicU32,
    /// Next session ID
    next_session_id: AtomicU32,
    /// Default playback device
    default_sink: Option<NodeId>,
    /// Default capture device
    default_source: Option<NodeId>,
    /// Global sample rate
    sample_rate: u32,
    /// Buffer size in frames
    buffer_frames: u32,
    /// Daemon running
    running: AtomicBool,
    /// Event callbacks
    event_callbacks: Vec<EventCallback>,
    /// Audio mixer integration
    mixer_channel: Option<ChannelId>,
}

impl AudioDaemon {
    /// Create new audio daemon
    pub const fn new() -> Self {
        Self {
            name: String::new(),
            nodes: BTreeMap::new(),
            ports: BTreeMap::new(),
            links: BTreeMap::new(),
            clients: BTreeMap::new(),
            sessions: BTreeMap::new(),
            next_node_id: AtomicU32::new(1),
            next_port_id: AtomicU32::new(1),
            next_link_id: AtomicU32::new(1),
            next_client_id: AtomicU32::new(1),
            next_session_id: AtomicU32::new(1),
            default_sink: None,
            default_source: None,
            sample_rate: 48000,
            buffer_frames: 1024,
            running: AtomicBool::new(false),
            event_callbacks: Vec::new(),
            mixer_channel: None,
        }
    }

    /// Initialize the daemon
    pub fn init(&mut self, name: &str) {
        self.name = name.to_string();
        self.running.store(true, Ordering::Relaxed);

        // Create default sink node (speakers)
        let sink_id = self.create_node("Default Speakers", NodeType::Sink);
        if let Some(node) = self.nodes.get_mut(&sink_id) {
            node.set_property("device.class", "speakers");
            node.set_property("audio.position", "FL,FR");
        }
        self.default_sink = Some(sink_id);

        // Create default source node (microphone)
        let source_id = self.create_node("Default Microphone", NodeType::Source);
        if let Some(node) = self.nodes.get_mut(&source_id) {
            node.set_property("device.class", "microphone");
        }
        self.default_source = Some(source_id);

        // Create ports for default devices
        let sink_out = self.create_port(sink_id, "output_FL", PortDirection::Output, MediaType::Audio);
        let sink_out_r = self.create_port(sink_id, "output_FR", PortDirection::Output, MediaType::Audio);
        if let Some(node) = self.nodes.get_mut(&sink_id) {
            node.add_output_port(sink_out);
            node.add_output_port(sink_out_r);
        }

        let source_in = self.create_port(source_id, "input_MONO", PortDirection::Input, MediaType::Audio);
        if let Some(node) = self.nodes.get_mut(&source_id) {
            node.add_input_port(source_in);
        }

        crate::kprintln!("audio_daemon: initialized '{}'", name);
    }

    /// Create a new node
    pub fn create_node(&mut self, name: &str, node_type: NodeType) -> NodeId {
        let id = self.next_node_id.fetch_add(1, Ordering::Relaxed);
        let node = Node::new(id, name, node_type);
        self.nodes.insert(id, node);
        self.emit_event(DaemonEvent::NodeAdded(id));
        id
    }

    /// Remove a node
    pub fn remove_node(&mut self, id: NodeId) -> bool {
        if self.nodes.remove(&id).is_some() {
            // Remove associated ports and links
            let port_ids: Vec<_> = self.ports
                .iter()
                .filter(|(_, p)| p.node_id() == id)
                .map(|(id, _)| *id)
                .collect();

            for port_id in port_ids {
                self.remove_port(port_id);
            }

            self.emit_event(DaemonEvent::NodeRemoved(id));
            true
        } else {
            false
        }
    }

    /// Get node by ID
    pub fn get_node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    /// Get node by ID (mutable)
    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(&id)
    }

    /// Create a port
    pub fn create_port(
        &mut self,
        node_id: NodeId,
        name: &str,
        direction: PortDirection,
        media_type: MediaType,
    ) -> PortId {
        let id = self.next_port_id.fetch_add(1, Ordering::Relaxed);
        let port = Port::new(id, node_id, name, direction, media_type);
        self.ports.insert(id, port);
        id
    }

    /// Remove a port
    pub fn remove_port(&mut self, id: PortId) -> bool {
        // Remove any links connected to this port
        let link_ids: Vec<_> = self.links
            .iter()
            .filter(|(_, l)| l.output_port() == id || l.input_port() == id)
            .map(|(id, _)| *id)
            .collect();

        for link_id in link_ids {
            self.remove_link(link_id);
        }

        self.ports.remove(&id).is_some()
    }

    /// Get port by ID
    pub fn get_port(&self, id: PortId) -> Option<&Port> {
        self.ports.get(&id)
    }

    /// Create a link between two ports
    pub fn create_link(&mut self, output_port: PortId, input_port: PortId) -> Option<LinkId> {
        // Validate ports exist and are compatible
        let out_port = self.ports.get(&output_port)?;
        let in_port = self.ports.get(&input_port)?;

        if out_port.direction() != PortDirection::Output {
            return None;
        }
        if in_port.direction() != PortDirection::Input {
            return None;
        }
        if out_port.media_type != in_port.media_type {
            return None;
        }

        let id = self.next_link_id.fetch_add(1, Ordering::Relaxed);
        let link = Link::new(id, output_port, input_port);
        self.links.insert(id, link);

        // Update port linked_to
        if let Some(port) = self.ports.get_mut(&output_port) {
            port.linked_to = Some(input_port);
        }
        if let Some(port) = self.ports.get_mut(&input_port) {
            port.linked_to = Some(output_port);
        }

        self.emit_event(DaemonEvent::LinkCreated(id));
        Some(id)
    }

    /// Remove a link
    pub fn remove_link(&mut self, id: LinkId) -> bool {
        if let Some(link) = self.links.remove(&id) {
            // Clear linked_to on ports
            if let Some(port) = self.ports.get_mut(&link.output_port()) {
                port.linked_to = None;
            }
            if let Some(port) = self.ports.get_mut(&link.input_port()) {
                port.linked_to = None;
            }
            self.emit_event(DaemonEvent::LinkDestroyed(id));
            true
        } else {
            false
        }
    }

    /// Connect a client
    pub fn connect_client(&mut self, name: &str, protocol: ClientProtocol) -> ClientId {
        let id = self.next_client_id.fetch_add(1, Ordering::Relaxed);
        let mut client = Client::new(id, name, protocol);

        // Set default permissions based on protocol
        let permissions = match protocol {
            ClientProtocol::Native => ClientPermissions {
                can_capture: true,
                can_playback: true,
                can_manage_devices: true,
                can_configure_routing: true,
            },
            ClientProtocol::PulseAudio | ClientProtocol::Alsa => ClientPermissions {
                can_capture: true,
                can_playback: true,
                can_manage_devices: false,
                can_configure_routing: false,
            },
            ClientProtocol::Jack => ClientPermissions {
                can_capture: true,
                can_playback: true,
                can_manage_devices: false,
                can_configure_routing: true,
            },
        };
        client.set_permissions(permissions);

        self.clients.insert(id, client);
        self.emit_event(DaemonEvent::ClientConnected(id));
        id
    }

    /// Disconnect a client
    pub fn disconnect_client(&mut self, id: ClientId) -> bool {
        if let Some(client) = self.clients.remove(&id) {
            // Remove client's nodes
            for node_id in client.nodes() {
                self.remove_node(*node_id);
            }
            self.emit_event(DaemonEvent::ClientDisconnected(id));
            true
        } else {
            false
        }
    }

    /// Get client by ID
    pub fn get_client(&self, id: ClientId) -> Option<&Client> {
        self.clients.get(&id)
    }

    /// Create a playback stream for a client
    pub fn create_playback_stream(
        &mut self,
        client_id: ClientId,
        name: &str,
        format: &AudioFormat,
    ) -> Option<NodeId> {
        // Verify client exists and has permission
        let client = self.clients.get(&client_id)?;
        if !client.permissions().can_playback {
            return None;
        }

        // Create stream node
        let node_id = self.create_node(name, NodeType::Source);
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.set_client(client_id);
            node.set_property("media.class", "Stream/Output/Audio");
            node.set_property("stream.name", name);
        }

        // Create output ports
        for (i, pos) in format.channel_map.iter().enumerate() {
            let port_name = alloc::format!("output_{}", i);
            let port_id = self.create_port(node_id, &port_name, PortDirection::Output, MediaType::Audio);
            if let Some(node) = self.nodes.get_mut(&node_id) {
                node.add_output_port(port_id);
            }
        }

        // Add node to client
        if let Some(client) = self.clients.get_mut(&client_id) {
            client.add_node(node_id);
        }

        // Auto-connect to default sink
        if let Some(sink_id) = self.default_sink {
            self.link_nodes(node_id, sink_id);
        }

        Some(node_id)
    }

    /// Create a capture stream for a client
    pub fn create_capture_stream(
        &mut self,
        client_id: ClientId,
        name: &str,
        format: &AudioFormat,
    ) -> Option<NodeId> {
        // Verify client exists and has permission
        let client = self.clients.get(&client_id)?;
        if !client.permissions().can_capture {
            return None;
        }

        // Create stream node
        let node_id = self.create_node(name, NodeType::Sink);
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.set_client(client_id);
            node.set_property("media.class", "Stream/Input/Audio");
            node.set_property("stream.name", name);
        }

        // Create input ports
        for (i, _pos) in format.channel_map.iter().enumerate() {
            let port_name = alloc::format!("input_{}", i);
            let port_id = self.create_port(node_id, &port_name, PortDirection::Input, MediaType::Audio);
            if let Some(node) = self.nodes.get_mut(&node_id) {
                node.add_input_port(port_id);
            }
        }

        // Add node to client
        if let Some(client) = self.clients.get_mut(&client_id) {
            client.add_node(node_id);
        }

        // Auto-connect from default source
        if let Some(source_id) = self.default_source {
            self.link_nodes(source_id, node_id);
        }

        Some(node_id)
    }

    /// Link two nodes (auto-connect matching ports)
    pub fn link_nodes(&mut self, source_id: NodeId, sink_id: NodeId) -> Vec<LinkId> {
        let mut created_links = Vec::new();

        let source_ports: Vec<_> = self.nodes.get(&source_id)
            .map(|n| n.output_ports().to_vec())
            .unwrap_or_default();
        let sink_ports: Vec<_> = self.nodes.get(&sink_id)
            .map(|n| n.input_ports().to_vec())
            .unwrap_or_default();

        // Simple matching: connect in order
        for (out_port, in_port) in source_ports.iter().zip(sink_ports.iter()) {
            if let Some(link_id) = self.create_link(*out_port, *in_port) {
                created_links.push(link_id);
            }
        }

        created_links
    }

    /// Set default playback device
    pub fn set_default_sink(&mut self, node_id: NodeId) -> bool {
        if self.nodes.contains_key(&node_id) {
            self.default_sink = Some(node_id);
            self.emit_event(DaemonEvent::DefaultDeviceChanged(StreamDirection::Playback, node_id));
            true
        } else {
            false
        }
    }

    /// Set default capture device
    pub fn set_default_source(&mut self, node_id: NodeId) -> bool {
        if self.nodes.contains_key(&node_id) {
            self.default_source = Some(node_id);
            self.emit_event(DaemonEvent::DefaultDeviceChanged(StreamDirection::Capture, node_id));
            true
        } else {
            false
        }
    }

    /// Get default sink
    pub fn default_sink(&self) -> Option<NodeId> {
        self.default_sink
    }

    /// Get default source
    pub fn default_source(&self) -> Option<NodeId> {
        self.default_source
    }

    /// List all devices
    pub fn list_devices(&self) -> Vec<DeviceInfo> {
        let mut devices = Vec::new();

        for (id, node) in &self.nodes {
            // Only include sink/source nodes that are devices (not client streams)
            if node.client_id().is_some() {
                continue;
            }

            let direction = match node.node_type() {
                NodeType::Sink => StreamDirection::Playback,
                NodeType::Source => StreamDirection::Capture,
                _ => continue,
            };

            let device_class = match node.get_property("device.class").map(|s| s.as_str()) {
                Some("speakers") => DeviceClass::Speakers,
                Some("headphones") => DeviceClass::Headphones,
                Some("hdmi") => DeviceClass::Hdmi,
                Some("bluetooth") => DeviceClass::Bluetooth,
                Some("usb") => DeviceClass::Usb,
                Some("microphone") => DeviceClass::Microphone,
                Some("webcam") => DeviceClass::Webcam,
                Some("virtual") => DeviceClass::Virtual,
                _ => DeviceClass::Unknown,
            };

            let is_default = match direction {
                StreamDirection::Playback => self.default_sink == Some(*id),
                StreamDirection::Capture => self.default_source == Some(*id),
            };

            devices.push(DeviceInfo {
                id: *id,
                name: node.name().to_string(),
                description: node.get_property("device.description")
                    .cloned()
                    .unwrap_or_else(|| node.name().to_string()),
                device_class,
                direction,
                channels: 2, // Default
                sample_rate: self.sample_rate,
                is_default,
                is_available: node.is_active(),
            });
        }

        devices
    }

    /// List all streams (client audio)
    pub fn list_streams(&self) -> Vec<StreamInfo> {
        let mut streams = Vec::new();

        for (id, node) in &self.nodes {
            // Only include client streams
            let client_id = match node.client_id() {
                Some(id) => id,
                None => continue,
            };

            let client_name = self.clients.get(&client_id)
                .map(|c| c.name().to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            let direction = match node.node_type() {
                NodeType::Source => StreamDirection::Playback,
                NodeType::Sink => StreamDirection::Capture,
                _ => continue,
            };

            // Find connected device
            let device_id = node.output_ports().first()
                .and_then(|p| self.ports.get(p))
                .and_then(|p| p.linked_port())
                .and_then(|p| self.ports.get(&p))
                .map(|p| p.node_id());

            streams.push(StreamInfo {
                id: *id,
                client_name,
                stream_name: node.name().to_string(),
                direction,
                volume: node.volume(),
                muted: node.is_muted(),
                device_id,
            });
        }

        streams
    }

    /// Set node volume
    pub fn set_node_volume(&mut self, node_id: NodeId, volume: u8) -> bool {
        if let Some(node) = self.nodes.get(&node_id) {
            node.set_volume(volume);
            self.emit_event(DaemonEvent::VolumeChanged(node_id, volume));
            true
        } else {
            false
        }
    }

    /// Set node mute
    pub fn set_node_mute(&mut self, node_id: NodeId, muted: bool) -> bool {
        if let Some(node) = self.nodes.get(&node_id) {
            node.set_muted(muted);
            self.emit_event(DaemonEvent::MuteChanged(node_id, muted));
            true
        } else {
            false
        }
    }

    /// Register event callback
    pub fn on_event(&mut self, callback: EventCallback) {
        self.event_callbacks.push(callback);
    }

    /// Emit an event
    fn emit_event(&self, event: DaemonEvent) {
        for callback in &self.event_callbacks {
            callback(event.clone());
        }
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Set sample rate
    pub fn set_sample_rate(&mut self, rate: u32) {
        self.sample_rate = rate;
    }

    /// Get buffer frames
    pub fn buffer_frames(&self) -> u32 {
        self.buffer_frames
    }

    /// Set buffer frames
    pub fn set_buffer_frames(&mut self, frames: u32) {
        self.buffer_frames = frames;
    }

    /// Is daemon running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Stop daemon
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    /// Process audio (called from audio thread)
    pub fn process(&mut self, frames: usize) {
        if !self.is_running() {
            return;
        }

        // Process all active links
        let link_ids: Vec<_> = self.links.keys().copied().collect();

        for link_id in link_ids {
            let link = match self.links.get(&link_id) {
                Some(l) if l.is_active() => l,
                _ => continue,
            };

            let out_port_id = link.output_port();
            let in_port_id = link.input_port();

            // Read from output port
            let mut buffer = vec![0.0f32; frames];
            if let Some(out_port) = self.ports.get(&out_port_id) {
                out_port.read_samples(&mut buffer);

                // Apply node volume
                if let Some(node) = self.nodes.get(&out_port.node_id()) {
                    if node.is_muted() {
                        buffer.fill(0.0);
                    } else {
                        let vol = node.volume() as f32 / 100.0;
                        for sample in &mut buffer {
                            *sample *= vol;
                        }
                    }
                }
            }

            // Write to input port
            if let Some(in_port) = self.ports.get(&in_port_id) {
                in_port.write_samples(&buffer);
            }
        }
    }

    /// Get statistics
    pub fn stats(&self) -> DaemonStats {
        DaemonStats {
            node_count: self.nodes.len(),
            port_count: self.ports.len(),
            link_count: self.links.len(),
            client_count: self.clients.len(),
            sample_rate: self.sample_rate,
            buffer_frames: self.buffer_frames,
        }
    }
}

/// Daemon statistics
#[derive(Debug, Clone)]
pub struct DaemonStats {
    pub node_count: usize,
    pub port_count: usize,
    pub link_count: usize,
    pub client_count: usize,
    pub sample_rate: u32,
    pub buffer_frames: u32,
}

// =============================================================================
// Global daemon instance
// =============================================================================

static AUDIO_DAEMON: IrqSafeMutex<Option<AudioDaemon>> = IrqSafeMutex::new(None);

/// Initialize global audio daemon
pub fn init() {
    let mut daemon = AudioDaemon::new();
    daemon.init("stenzel-audio");
    *AUDIO_DAEMON.lock() = Some(daemon);
    crate::kprintln!("audio_daemon: global daemon initialized");
}

/// Get daemon lock
pub fn with_daemon<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&AudioDaemon) -> R,
{
    AUDIO_DAEMON.lock().as_ref().map(f)
}

/// Get daemon lock (mutable)
pub fn with_daemon_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut AudioDaemon) -> R,
{
    AUDIO_DAEMON.lock().as_mut().map(f)
}

/// Connect a client to the daemon
pub fn connect_client(name: &str, protocol: ClientProtocol) -> Option<ClientId> {
    with_daemon_mut(|d| d.connect_client(name, protocol))
}

/// Disconnect a client
pub fn disconnect_client(client_id: ClientId) -> bool {
    with_daemon_mut(|d| d.disconnect_client(client_id)).unwrap_or(false)
}

/// Create a playback stream
pub fn create_playback_stream(client_id: ClientId, name: &str) -> Option<NodeId> {
    with_daemon_mut(|d| {
        d.create_playback_stream(client_id, name, &AudioFormat::default())
    }).flatten()
}

/// Create a capture stream
pub fn create_capture_stream(client_id: ClientId, name: &str) -> Option<NodeId> {
    with_daemon_mut(|d| {
        d.create_capture_stream(client_id, name, &AudioFormat::default())
    }).flatten()
}

/// List devices
pub fn list_devices() -> Vec<DeviceInfo> {
    with_daemon(|d| d.list_devices()).unwrap_or_default()
}

/// List streams
pub fn list_streams() -> Vec<StreamInfo> {
    with_daemon(|d| d.list_streams()).unwrap_or_default()
}

/// Set stream volume
pub fn set_stream_volume(node_id: NodeId, volume: u8) -> bool {
    with_daemon_mut(|d| d.set_node_volume(node_id, volume)).unwrap_or(false)
}

/// Set stream mute
pub fn set_stream_mute(node_id: NodeId, muted: bool) -> bool {
    with_daemon_mut(|d| d.set_node_mute(node_id, muted)).unwrap_or(false)
}

/// Get default sink
pub fn get_default_sink() -> Option<NodeId> {
    with_daemon(|d| d.default_sink()).flatten()
}

/// Get default source
pub fn get_default_source() -> Option<NodeId> {
    with_daemon(|d| d.default_source()).flatten()
}

/// Set default sink
pub fn set_default_sink(node_id: NodeId) -> bool {
    with_daemon_mut(|d| d.set_default_sink(node_id)).unwrap_or(false)
}

/// Set default source
pub fn set_default_source(node_id: NodeId) -> bool {
    with_daemon_mut(|d| d.set_default_source(node_id)).unwrap_or(false)
}
