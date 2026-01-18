//! WebRTC Implementation
//!
//! Real-time communication for video/audio calls and data channels.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;

/// Unique peer connection identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerConnectionId(u64);

impl PeerConnectionId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Unique media stream identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MediaStreamId(u64);

impl MediaStreamId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Unique data channel identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DataChannelId(u64);

impl DataChannelId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Unique track identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(u64);

impl TrackId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// ICE connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IceConnectionState {
    New,
    Checking,
    Connected,
    Completed,
    Failed,
    Disconnected,
    Closed,
}

impl IceConnectionState {
    pub fn name(&self) -> &'static str {
        match self {
            IceConnectionState::New => "New",
            IceConnectionState::Checking => "Checking",
            IceConnectionState::Connected => "Connected",
            IceConnectionState::Completed => "Completed",
            IceConnectionState::Failed => "Failed",
            IceConnectionState::Disconnected => "Disconnected",
            IceConnectionState::Closed => "Closed",
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self, IceConnectionState::Connected | IceConnectionState::Completed)
    }
}

/// ICE gathering state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IceGatheringState {
    New,
    Gathering,
    Complete,
}

impl IceGatheringState {
    pub fn name(&self) -> &'static str {
        match self {
            IceGatheringState::New => "New",
            IceGatheringState::Gathering => "Gathering",
            IceGatheringState::Complete => "Complete",
        }
    }
}

/// Signaling state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalingState {
    Stable,
    HaveLocalOffer,
    HaveRemoteOffer,
    HaveLocalPranswer,
    HaveRemotePranswer,
    Closed,
}

impl SignalingState {
    pub fn name(&self) -> &'static str {
        match self {
            SignalingState::Stable => "Stable",
            SignalingState::HaveLocalOffer => "Have Local Offer",
            SignalingState::HaveRemoteOffer => "Have Remote Offer",
            SignalingState::HaveLocalPranswer => "Have Local Pranswer",
            SignalingState::HaveRemotePranswer => "Have Remote Pranswer",
            SignalingState::Closed => "Closed",
        }
    }
}

/// Peer connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerConnectionState {
    New,
    Connecting,
    Connected,
    Disconnected,
    Failed,
    Closed,
}

impl PeerConnectionState {
    pub fn name(&self) -> &'static str {
        match self {
            PeerConnectionState::New => "New",
            PeerConnectionState::Connecting => "Connecting",
            PeerConnectionState::Connected => "Connected",
            PeerConnectionState::Disconnected => "Disconnected",
            PeerConnectionState::Failed => "Failed",
            PeerConnectionState::Closed => "Closed",
        }
    }
}

/// Media track kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Audio,
    Video,
}

impl MediaKind {
    pub fn name(&self) -> &'static str {
        match self {
            MediaKind::Audio => "audio",
            MediaKind::Video => "video",
        }
    }
}

/// Media track state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackState {
    Live,
    Ended,
}

/// ICE candidate type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IceCandidateType {
    Host,
    Srflx,  // Server reflexive
    Prflx,  // Peer reflexive
    Relay,
}

impl IceCandidateType {
    pub fn name(&self) -> &'static str {
        match self {
            IceCandidateType::Host => "host",
            IceCandidateType::Srflx => "srflx",
            IceCandidateType::Prflx => "prflx",
            IceCandidateType::Relay => "relay",
        }
    }
}

/// ICE candidate
#[derive(Debug, Clone)]
pub struct IceCandidate {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_m_line_index: Option<u16>,
    pub candidate_type: IceCandidateType,
    pub protocol: IceProtocol,
    pub address: String,
    pub port: u16,
    pub priority: u32,
    pub foundation: String,
}

impl IceCandidate {
    pub fn new(candidate: &str) -> Self {
        Self {
            candidate: String::from(candidate),
            sdp_mid: None,
            sdp_m_line_index: None,
            candidate_type: IceCandidateType::Host,
            protocol: IceProtocol::Udp,
            address: String::new(),
            port: 0,
            priority: 0,
            foundation: String::new(),
        }
    }

    pub fn to_sdp(&self) -> String {
        format!(
            "candidate:{} 1 {} {} {} {} typ {}",
            self.foundation,
            self.protocol.name(),
            self.priority,
            self.address,
            self.port,
            self.candidate_type.name()
        )
    }
}

/// ICE protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IceProtocol {
    Udp,
    Tcp,
}

impl IceProtocol {
    pub fn name(&self) -> &'static str {
        match self {
            IceProtocol::Udp => "udp",
            IceProtocol::Tcp => "tcp",
        }
    }
}

/// ICE server configuration
#[derive(Debug, Clone)]
pub struct IceServer {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

impl IceServer {
    pub fn stun(url: &str) -> Self {
        Self {
            urls: vec![String::from(url)],
            username: None,
            credential: None,
        }
    }

    pub fn turn(url: &str, username: &str, credential: &str) -> Self {
        Self {
            urls: vec![String::from(url)],
            username: Some(String::from(username)),
            credential: Some(String::from(credential)),
        }
    }
}

/// SDP type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdpType {
    Offer,
    Answer,
    Pranswer,
    Rollback,
}

impl SdpType {
    pub fn name(&self) -> &'static str {
        match self {
            SdpType::Offer => "offer",
            SdpType::Answer => "answer",
            SdpType::Pranswer => "pranswer",
            SdpType::Rollback => "rollback",
        }
    }
}

/// Session description
#[derive(Debug, Clone)]
pub struct SessionDescription {
    pub sdp_type: SdpType,
    pub sdp: String,
}

impl SessionDescription {
    pub fn new(sdp_type: SdpType, sdp: &str) -> Self {
        Self {
            sdp_type,
            sdp: String::from(sdp),
        }
    }

    /// Generate a basic SDP offer
    pub fn create_offer(video: bool, audio: bool) -> Self {
        let mut sdp = String::from("v=0\r\n");
        sdp.push_str("o=- 0 0 IN IP4 0.0.0.0\r\n");
        sdp.push_str("s=-\r\n");
        sdp.push_str("t=0 0\r\n");
        sdp.push_str("a=group:BUNDLE 0 1\r\n");
        sdp.push_str("a=msid-semantic: WMS\r\n");

        if audio {
            sdp.push_str("m=audio 9 UDP/TLS/RTP/SAVPF 111\r\n");
            sdp.push_str("c=IN IP4 0.0.0.0\r\n");
            sdp.push_str("a=rtcp:9 IN IP4 0.0.0.0\r\n");
            sdp.push_str("a=ice-ufrag:ABCD\r\n");
            sdp.push_str("a=ice-pwd:12345678901234567890\r\n");
            sdp.push_str("a=fingerprint:sha-256 00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00\r\n");
            sdp.push_str("a=setup:actpass\r\n");
            sdp.push_str("a=mid:0\r\n");
            sdp.push_str("a=sendrecv\r\n");
            sdp.push_str("a=rtpmap:111 opus/48000/2\r\n");
        }

        if video {
            sdp.push_str("m=video 9 UDP/TLS/RTP/SAVPF 96\r\n");
            sdp.push_str("c=IN IP4 0.0.0.0\r\n");
            sdp.push_str("a=rtcp:9 IN IP4 0.0.0.0\r\n");
            sdp.push_str("a=ice-ufrag:ABCD\r\n");
            sdp.push_str("a=ice-pwd:12345678901234567890\r\n");
            sdp.push_str("a=fingerprint:sha-256 00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00\r\n");
            sdp.push_str("a=setup:actpass\r\n");
            sdp.push_str("a=mid:1\r\n");
            sdp.push_str("a=sendrecv\r\n");
            sdp.push_str("a=rtpmap:96 VP8/90000\r\n");
        }

        Self::new(SdpType::Offer, &sdp)
    }

    /// Generate SDP answer
    pub fn create_answer(offer: &SessionDescription) -> Self {
        // Simplified answer generation
        let sdp = offer.sdp.replace("a=setup:actpass", "a=setup:active");
        Self::new(SdpType::Answer, &sdp)
    }
}

/// Media stream track
#[derive(Debug, Clone)]
pub struct MediaStreamTrack {
    pub id: TrackId,
    pub kind: MediaKind,
    pub label: String,
    pub state: TrackState,
    pub muted: bool,
    pub enabled: bool,
    pub constraints: MediaTrackConstraints,
}

impl MediaStreamTrack {
    pub fn new(id: TrackId, kind: MediaKind, label: &str) -> Self {
        Self {
            id,
            kind,
            label: String::from(label),
            state: TrackState::Live,
            muted: false,
            enabled: true,
            constraints: MediaTrackConstraints::default(),
        }
    }

    pub fn is_audio(&self) -> bool {
        self.kind == MediaKind::Audio
    }

    pub fn is_video(&self) -> bool {
        self.kind == MediaKind::Video
    }

    pub fn stop(&mut self) {
        self.state = TrackState::Ended;
        self.enabled = false;
    }
}

/// Media track constraints
#[derive(Debug, Clone)]
pub struct MediaTrackConstraints {
    // Audio constraints
    pub audio_enabled: bool,
    pub echo_cancellation: bool,
    pub noise_suppression: bool,
    pub auto_gain_control: bool,
    pub sample_rate: Option<u32>,
    pub channel_count: Option<u8>,

    // Video constraints
    pub video_enabled: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<u32>,
    pub facing_mode: FacingMode,
    pub aspect_ratio: Option<f32>,
}

impl Default for MediaTrackConstraints {
    fn default() -> Self {
        Self {
            audio_enabled: true,
            echo_cancellation: true,
            noise_suppression: true,
            auto_gain_control: true,
            sample_rate: None,
            channel_count: None,
            video_enabled: true,
            width: Some(1280),
            height: Some(720),
            frame_rate: Some(30),
            facing_mode: FacingMode::User,
            aspect_ratio: None,
        }
    }
}

/// Camera facing mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacingMode {
    User,       // Front camera
    Environment, // Back camera
}

impl FacingMode {
    pub fn name(&self) -> &'static str {
        match self {
            FacingMode::User => "user",
            FacingMode::Environment => "environment",
        }
    }
}

/// Media stream
#[derive(Debug, Clone)]
pub struct MediaStream {
    pub id: MediaStreamId,
    pub tracks: Vec<MediaStreamTrack>,
    pub active: bool,
}

impl MediaStream {
    pub fn new(id: MediaStreamId) -> Self {
        Self {
            id,
            tracks: Vec::new(),
            active: true,
        }
    }

    pub fn add_track(&mut self, track: MediaStreamTrack) {
        self.tracks.push(track);
    }

    pub fn remove_track(&mut self, track_id: TrackId) {
        self.tracks.retain(|t| t.id != track_id);
    }

    pub fn get_audio_tracks(&self) -> Vec<&MediaStreamTrack> {
        self.tracks.iter().filter(|t| t.is_audio()).collect()
    }

    pub fn get_video_tracks(&self) -> Vec<&MediaStreamTrack> {
        self.tracks.iter().filter(|t| t.is_video()).collect()
    }

    pub fn stop(&mut self) {
        for track in &mut self.tracks {
            track.stop();
        }
        self.active = false;
    }
}

/// Data channel state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataChannelState {
    Connecting,
    Open,
    Closing,
    Closed,
}

impl DataChannelState {
    pub fn name(&self) -> &'static str {
        match self {
            DataChannelState::Connecting => "connecting",
            DataChannelState::Open => "open",
            DataChannelState::Closing => "closing",
            DataChannelState::Closed => "closed",
        }
    }
}

/// Data channel
#[derive(Debug, Clone)]
pub struct DataChannel {
    pub id: DataChannelId,
    pub label: String,
    pub state: DataChannelState,
    pub ordered: bool,
    pub max_packet_life_time: Option<u16>,
    pub max_retransmits: Option<u16>,
    pub protocol: String,
    pub negotiated: bool,
    pub buffered_amount: usize,
    pub buffered_amount_low_threshold: usize,
}

impl DataChannel {
    pub fn new(id: DataChannelId, label: &str) -> Self {
        Self {
            id,
            label: String::from(label),
            state: DataChannelState::Connecting,
            ordered: true,
            max_packet_life_time: None,
            max_retransmits: None,
            protocol: String::new(),
            negotiated: false,
            buffered_amount: 0,
            buffered_amount_low_threshold: 0,
        }
    }

    pub fn is_open(&self) -> bool {
        self.state == DataChannelState::Open
    }

    pub fn close(&mut self) {
        self.state = DataChannelState::Closed;
    }
}

/// RTCPeerConnection configuration
#[derive(Debug, Clone)]
pub struct RtcConfiguration {
    pub ice_servers: Vec<IceServer>,
    pub ice_transport_policy: IceTransportPolicy,
    pub bundle_policy: BundlePolicy,
    pub rtcp_mux_policy: RtcpMuxPolicy,
    pub certificates: Vec<RtcCertificate>,
}

impl Default for RtcConfiguration {
    fn default() -> Self {
        Self {
            ice_servers: vec![
                IceServer::stun("stun:stun.l.google.com:19302"),
            ],
            ice_transport_policy: IceTransportPolicy::All,
            bundle_policy: BundlePolicy::Balanced,
            rtcp_mux_policy: RtcpMuxPolicy::Require,
            certificates: Vec::new(),
        }
    }
}

/// ICE transport policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IceTransportPolicy {
    All,
    Relay,
}

/// Bundle policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundlePolicy {
    Balanced,
    MaxCompat,
    MaxBundle,
}

/// RTCP mux policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtcpMuxPolicy {
    Negotiate,
    Require,
}

/// RTC certificate
#[derive(Debug, Clone)]
pub struct RtcCertificate {
    pub expires: u64,
    pub fingerprint: String,
    pub algorithm: String,
}

impl RtcCertificate {
    pub fn new(expires: u64) -> Self {
        Self {
            expires,
            fingerprint: String::from("00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00"),
            algorithm: String::from("sha-256"),
        }
    }
}

/// Peer connection
#[derive(Debug)]
pub struct PeerConnection {
    pub id: PeerConnectionId,
    pub config: RtcConfiguration,
    pub connection_state: PeerConnectionState,
    pub ice_connection_state: IceConnectionState,
    pub ice_gathering_state: IceGatheringState,
    pub signaling_state: SignalingState,
    pub local_description: Option<SessionDescription>,
    pub remote_description: Option<SessionDescription>,
    pub local_candidates: Vec<IceCandidate>,
    pub remote_candidates: Vec<IceCandidate>,
    pub local_streams: Vec<MediaStream>,
    pub remote_streams: Vec<MediaStream>,
    pub data_channels: Vec<DataChannel>,
    pub created: u64,
}

impl PeerConnection {
    pub fn new(id: PeerConnectionId, config: RtcConfiguration) -> Self {
        Self {
            id,
            config,
            connection_state: PeerConnectionState::New,
            ice_connection_state: IceConnectionState::New,
            ice_gathering_state: IceGatheringState::New,
            signaling_state: SignalingState::Stable,
            local_description: None,
            remote_description: None,
            local_candidates: Vec::new(),
            remote_candidates: Vec::new(),
            local_streams: Vec::new(),
            remote_streams: Vec::new(),
            data_channels: Vec::new(),
            created: 0,
        }
    }

    pub fn create_offer(&mut self, has_video: bool, has_audio: bool) -> SessionDescription {
        let offer = SessionDescription::create_offer(has_video, has_audio);
        self.local_description = Some(offer.clone());
        self.signaling_state = SignalingState::HaveLocalOffer;
        offer
    }

    pub fn create_answer(&mut self) -> WebRtcResult<SessionDescription> {
        if self.remote_description.is_none() {
            return Err(WebRtcError::InvalidState);
        }

        let answer = SessionDescription::create_answer(self.remote_description.as_ref().unwrap());
        self.local_description = Some(answer.clone());
        self.signaling_state = SignalingState::Stable;
        Ok(answer)
    }

    pub fn set_local_description(&mut self, desc: SessionDescription) {
        self.local_description = Some(desc);
        self.ice_gathering_state = IceGatheringState::Gathering;
    }

    pub fn set_remote_description(&mut self, desc: SessionDescription) -> WebRtcResult<()> {
        match desc.sdp_type {
            SdpType::Offer => {
                self.signaling_state = SignalingState::HaveRemoteOffer;
            }
            SdpType::Answer => {
                self.signaling_state = SignalingState::Stable;
                self.connection_state = PeerConnectionState::Connecting;
            }
            SdpType::Pranswer => {
                self.signaling_state = SignalingState::HaveRemotePranswer;
            }
            SdpType::Rollback => {
                self.signaling_state = SignalingState::Stable;
            }
        }
        self.remote_description = Some(desc);
        Ok(())
    }

    pub fn add_ice_candidate(&mut self, candidate: IceCandidate) {
        self.remote_candidates.push(candidate);

        // Simulate ICE state progression
        if self.ice_connection_state == IceConnectionState::New {
            self.ice_connection_state = IceConnectionState::Checking;
        }
    }

    pub fn add_track(&mut self, track: MediaStreamTrack, stream: &MediaStream) {
        // Find or create local stream
        if let Some(local_stream) = self.local_streams.iter_mut().find(|s| s.id == stream.id) {
            local_stream.add_track(track);
        } else {
            let mut new_stream = stream.clone();
            new_stream.add_track(track);
            self.local_streams.push(new_stream);
        }
    }

    pub fn create_data_channel(&mut self, label: &str, channel_id: DataChannelId) -> DataChannel {
        let channel = DataChannel::new(channel_id, label);
        self.data_channels.push(channel.clone());
        channel
    }

    pub fn close(&mut self) {
        self.connection_state = PeerConnectionState::Closed;
        self.ice_connection_state = IceConnectionState::Closed;
        self.signaling_state = SignalingState::Closed;

        for stream in &mut self.local_streams {
            stream.stop();
        }
        for channel in &mut self.data_channels {
            channel.close();
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connection_state == PeerConnectionState::Connected
    }

    /// Simulate connection establishment
    pub fn simulate_connection(&mut self) {
        if self.local_description.is_some() && self.remote_description.is_some() {
            self.ice_gathering_state = IceGatheringState::Complete;
            self.ice_connection_state = IceConnectionState::Connected;
            self.connection_state = PeerConnectionState::Connected;

            // Open data channels
            for channel in &mut self.data_channels {
                channel.state = DataChannelState::Open;
            }
        }
    }
}

/// WebRTC error
#[derive(Debug, Clone)]
pub enum WebRtcError {
    InvalidState,
    InvalidSdp,
    IceConnectionFailed,
    PermissionDenied,
    DeviceNotFound,
    DataChannelError,
    NetworkError,
}

pub type WebRtcResult<T> = Result<T, WebRtcError>;

/// RTC statistics
#[derive(Debug, Clone)]
pub struct RtcStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub packets_lost: u64,
    pub jitter: f64,
    pub round_trip_time: f64,
    pub timestamp: u64,
}

impl Default for RtcStats {
    fn default() -> Self {
        Self {
            bytes_sent: 0,
            bytes_received: 0,
            packets_sent: 0,
            packets_received: 0,
            packets_lost: 0,
            jitter: 0.0,
            round_trip_time: 0.0,
            timestamp: 0,
        }
    }
}

/// WebRTC manager
pub struct WebRtcManager {
    connections: BTreeMap<PeerConnectionId, PeerConnection>,
    media_streams: BTreeMap<MediaStreamId, MediaStream>,

    next_connection_id: u64,
    next_stream_id: u64,
    next_track_id: u64,
    next_channel_id: u64,

    // Device access
    media_devices_available: bool,
    camera_permission: bool,
    microphone_permission: bool,

    current_time: u64,
}

impl WebRtcManager {
    pub fn new() -> Self {
        Self {
            connections: BTreeMap::new(),
            media_streams: BTreeMap::new(),
            next_connection_id: 1,
            next_stream_id: 1,
            next_track_id: 1,
            next_channel_id: 1,
            media_devices_available: true,
            camera_permission: false,
            microphone_permission: false,
            current_time: 0,
        }
    }

    /// Create a new peer connection
    pub fn create_peer_connection(&mut self, config: RtcConfiguration) -> PeerConnectionId {
        let id = PeerConnectionId::new(self.next_connection_id);
        self.next_connection_id += 1;

        let mut connection = PeerConnection::new(id, config);
        connection.created = self.current_time;

        self.connections.insert(id, connection);
        id
    }

    /// Get peer connection
    pub fn get_connection(&self, id: PeerConnectionId) -> Option<&PeerConnection> {
        self.connections.get(&id)
    }

    /// Get mutable peer connection
    pub fn get_connection_mut(&mut self, id: PeerConnectionId) -> Option<&mut PeerConnection> {
        self.connections.get_mut(&id)
    }

    /// Close peer connection
    pub fn close_connection(&mut self, id: PeerConnectionId) {
        if let Some(connection) = self.connections.get_mut(&id) {
            connection.close();
        }
    }

    /// Request media devices
    pub fn get_user_media(&mut self, constraints: MediaTrackConstraints) -> WebRtcResult<MediaStream> {
        if !self.media_devices_available {
            return Err(WebRtcError::DeviceNotFound);
        }

        // Check permissions
        if constraints.video_enabled && !self.camera_permission {
            return Err(WebRtcError::PermissionDenied);
        }
        if constraints.audio_enabled && !self.microphone_permission {
            return Err(WebRtcError::PermissionDenied);
        }

        let stream_id = MediaStreamId::new(self.next_stream_id);
        self.next_stream_id += 1;

        let mut stream = MediaStream::new(stream_id);

        // Add video track
        if constraints.video_enabled {
            let track_id = TrackId::new(self.next_track_id);
            self.next_track_id += 1;

            let mut track = MediaStreamTrack::new(track_id, MediaKind::Video, "video0");
            track.constraints = constraints.clone();
            stream.add_track(track);
        }

        // Add audio track
        if constraints.audio_enabled {
            let track_id = TrackId::new(self.next_track_id);
            self.next_track_id += 1;

            let mut track = MediaStreamTrack::new(track_id, MediaKind::Audio, "audio0");
            track.constraints = constraints.clone();
            stream.add_track(track);
        }

        self.media_streams.insert(stream_id, stream.clone());
        Ok(stream)
    }

    /// Request display media (screen sharing)
    pub fn get_display_media(&mut self) -> WebRtcResult<MediaStream> {
        let stream_id = MediaStreamId::new(self.next_stream_id);
        self.next_stream_id += 1;

        let mut stream = MediaStream::new(stream_id);

        let track_id = TrackId::new(self.next_track_id);
        self.next_track_id += 1;

        let track = MediaStreamTrack::new(track_id, MediaKind::Video, "screen0");
        stream.add_track(track);

        self.media_streams.insert(stream_id, stream.clone());
        Ok(stream)
    }

    /// Get media stream
    pub fn get_stream(&self, id: MediaStreamId) -> Option<&MediaStream> {
        self.media_streams.get(&id)
    }

    /// Stop media stream
    pub fn stop_stream(&mut self, id: MediaStreamId) {
        if let Some(stream) = self.media_streams.get_mut(&id) {
            stream.stop();
        }
    }

    /// Create data channel
    pub fn create_data_channel(&mut self, connection_id: PeerConnectionId, label: &str) -> WebRtcResult<DataChannel> {
        let connection = self.connections.get_mut(&connection_id)
            .ok_or(WebRtcError::InvalidState)?;

        let channel_id = DataChannelId::new(self.next_channel_id);
        self.next_channel_id += 1;

        let channel = connection.create_data_channel(label, channel_id);
        Ok(channel)
    }

    /// Grant camera permission
    pub fn grant_camera_permission(&mut self) {
        self.camera_permission = true;
    }

    /// Grant microphone permission
    pub fn grant_microphone_permission(&mut self) {
        self.microphone_permission = true;
    }

    /// Revoke permissions
    pub fn revoke_permissions(&mut self) {
        self.camera_permission = false;
        self.microphone_permission = false;
    }

    /// Get connection count
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Get active connections
    pub fn active_connections(&self) -> Vec<&PeerConnection> {
        self.connections.values()
            .filter(|c| c.is_connected())
            .collect()
    }

    /// Set current time
    pub fn set_current_time(&mut self, time: u64) {
        self.current_time = time;
    }

    /// Add sample data for demo
    pub fn add_sample_data(&mut self) {
        self.current_time = 1705600000;

        // Grant permissions
        self.camera_permission = true;
        self.microphone_permission = true;

        // Create a demo connection
        let config = RtcConfiguration::default();
        let conn_id = self.create_peer_connection(config);

        if let Some(conn) = self.connections.get_mut(&conn_id) {
            // Create offer
            let _offer = conn.create_offer(true, true);

            // Add an ICE candidate
            let mut candidate = IceCandidate::new("candidate:1 1 UDP 2130706431 192.168.1.100 50000 typ host");
            candidate.candidate_type = IceCandidateType::Host;
            candidate.protocol = IceProtocol::Udp;
            candidate.address = String::from("192.168.1.100");
            candidate.port = 50000;
            candidate.priority = 2130706431;
            candidate.foundation = String::from("1");

            conn.local_candidates.push(candidate);
        }
    }
}

impl Default for WebRtcManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize WebRTC module
pub fn init() -> WebRtcManager {
    let mut manager = WebRtcManager::new();
    manager.add_sample_data();
    manager
}
