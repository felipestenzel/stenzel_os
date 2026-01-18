//! Browser Module
//!
//! Complete web browser implementation with HTML/CSS rendering engine.

pub mod engine;
pub mod html_parser;
pub mod css_parser;
pub mod layout;
pub mod render;
pub mod dom;
pub mod javascript;
pub mod tabs;
pub mod downloads;
pub mod network;
pub mod bookmarks;
pub mod history;
pub mod passwords;
pub mod webrtc;

pub use engine::BrowserEngine;
pub use render::{RenderTree, RenderNode};
pub use html_parser::{HtmlParser, HtmlDocument, HtmlElement, HtmlNode};
pub use css_parser::{CssParser, StyleSheet, CssRule, CssSelector, CssDeclaration};
pub use layout::{LayoutEngine, LayoutBox, LayoutMode, BoxDimensions};
pub use render::{Renderer, PaintCommand};
pub use dom::{Dom, DomNode, DomNodeType, DomElement, DomText};
pub use javascript::{JsEngine, JsValue, JsContext, JsError};
pub use tabs::{TabManager, Tab, TabId, TabState};
pub use downloads::{DownloadManager, Download, DownloadState, DownloadError};
pub use bookmarks::{BookmarkManager, Bookmark, BookmarkFolder, BookmarkTag, BookmarkId, FolderId, TagId, BookmarkType, SpecialFolder, SortOrder as BookmarkSortOrder, BookmarkFormat, BookmarkSearchResult, BookmarkError, BookmarkResult};
pub use history::{HistoryManager, HistoryEntry, HistoryEntryId, Visit, VisitType, TransitionType, TimeRange, HistorySearchResult as HistorySearch, HistoryStats, HistoryByDate, HistoryByDomain, HistorySortOrder, HistoryError, HistoryResult};
pub use passwords::{PasswordManager, LoginCredential, CreditCardCredential, AddressInfo, SecureNote, CredentialId, CredentialType, PasswordStrength, CardType, FolderId as PasswordFolderId, PasswordFolder, BreachInfo, BreachSeverity, PasswordGeneratorOptions, PasswordSortOrder, PasswordError, PasswordResult, VaultStatus, SearchResult as PasswordSearchResult, PasswordHealthReport, PasswordStats};
pub use webrtc::{WebRtcManager, PeerConnection, PeerConnectionId, MediaStream, MediaStreamId, MediaStreamTrack, TrackId, DataChannel, DataChannelId, IceCandidate, IceServer, SessionDescription, SdpType, IceConnectionState, IceGatheringState, SignalingState, PeerConnectionState, MediaKind, TrackState, DataChannelState, RtcConfiguration, MediaTrackConstraints, FacingMode, RtcStats, WebRtcError, WebRtcResult, IceCandidateType, IceProtocol, IceTransportPolicy, BundlePolicy, RtcpMuxPolicy, RtcCertificate};
