//! GUI Applications
//!
//! Built-in graphical applications for Stenzel OS.

pub mod archive;
pub mod browser;
pub mod diskutil;
pub mod calculator;
pub mod recentfiles;
pub mod thumbnails;
pub mod musicplayer;
pub mod screenrecorder;
pub mod email;
pub mod calendar;
pub mod contacts;
pub mod filemanager;
pub mod notes;
pub mod webcam;
pub mod networkshares;
pub mod printersettings;
pub mod terminaltabs;
pub mod syntax;
pub mod git;
pub mod firewall;
pub mod imageviewer;
pub mod pdfviewer;
pub mod screenshot;
pub mod search;
pub mod settings;
pub mod softwarecenter;
pub mod taskmanager;
pub mod terminal;
pub mod texteditor;
pub mod trash;
pub mod videoplayer;

pub use browser::{
    BrowserEngine, RenderTree, RenderNode,
    HtmlParser, HtmlDocument, HtmlElement, HtmlNode,
    CssParser, StyleSheet, CssRule, CssSelector, CssDeclaration,
    LayoutEngine, LayoutBox, LayoutMode, BoxDimensions,
    Renderer, PaintCommand,
    Dom, DomNode, DomNodeType, DomElement, DomText,
    JsEngine, JsValue, JsContext, JsError,
    TabManager, Tab, TabId, TabState,
    DownloadManager, Download, DownloadState, DownloadError,
};
pub use calculator::Calculator;
pub use filemanager::FileManager;
pub use imageviewer::{ImageViewer, Image, ImageFormat, ZoomMode, create_test_image};
pub use pdfviewer::{PdfViewer, PdfDocument, PdfPage};
pub use screenshot::{ScreenshotWidget, CaptureMode, ImageFormat as ScreenshotFormat};
pub use search::{SearchWidget, SearchFilter, SearchResult};
pub use settings::Settings;
pub use taskmanager::TaskManager;
pub use terminal::Terminal;
pub use texteditor::TextEditor;
pub use trash::{TrashViewer, TrashedItem, TrashFileType};
pub use videoplayer::{VideoPlayer, MediaInfo, PlaybackState};
pub use softwarecenter::{SoftwareCenter, SoftwareCenterView, AppCategory, AppEntry, UpdateNotificationService};
pub use archive::{ArchiveManager, ArchiveFormat, ArchiveEntry, ArchiveInfo, ArchiveError, CompressionLevel, ExtractOptions, CreateOptions};
pub use diskutil::{DiskUtility, DiskInfo, PartitionInfo, DiskType, PartitionTable, FilesystemType, HealthStatus, FormatOptions, DiskError};
pub use recentfiles::{RecentFilesWidget, RecentFilesManager, RecentFile, RecentFilesStats, FileCategory, TimeGroup, SortOrder};
pub use thumbnails::{ThumbnailCache, ThumbnailSize, ThumbnailableType, ThumbnailStatus, ThumbnailMetadata, CachedThumbnail, ThumbnailRequest, ThumbnailResult, ThumbnailConfig, ThumbnailStats};
pub use musicplayer::{MusicPlayer, AudioFormat, PlayerState, RepeatMode, ShuffleMode, TrackMetadata, Track, Playlist, EqualizerPreset, ViewMode as MusicViewMode, LibraryView};
pub use screenrecorder::{ScreenRecorder, RecordingState, RegionType, VideoFormat, QualityPreset, FrameRate, AudioSource, RecordingSettings, RecordingStats};
pub use email::{EmailClient, EmailProtocol, EmailAccount, Mailbox, MailboxType, EmailMessage, EmailAddress, MessageFlags, Attachment, DraftMessage, SearchFilter as EmailSearchFilter, SortOrder as EmailSortOrder, ViewMode as EmailViewMode, ConnectionState, EmailError};
pub use calendar::{CalendarWidget, Date, Time, DateTime, Weekday, Month, CalendarEvent, Calendar as CalendarData, CalendarView, RecurrenceRule, ReminderTime, EventColor, CalendarSettings, WeekStart};
pub use contacts::{ContactsManager, Contact, ContactGroup, PhoneNumber, PhoneType, Email as ContactEmail, EmailType as ContactEmailType, Address, AddressType, SocialProfile, ImportantDate, SortOrder as ContactsSortOrder, ViewMode as ContactsViewMode, FilterType};
pub use notes::{NotesApp, Note, Notebook, Tag, TextBlock, TextStyle, ListType, HeadingLevel, NoteColor, NoteAttachment, SortOrder as NotesSortOrder, ViewMode as NotesViewMode, FilterType as NotesFilterType, ExportFormat};
pub use webcam::{WebcamApp, VideoDevice, DeviceCapabilities, Resolution, PixelFormat, CaptureMode as WebcamCaptureMode, CameraState, CameraSettings, PhotoQuality, VideoQuality, VideoCodec, FlashMode, WhiteBalance, TimerSetting, MediaItem, RecordingStats as WebcamRecordingStats};
pub use networkshares::{NetworkSharesBrowser, NetworkServer, NetworkShare, ShareProtocol, ShareType, SharePermissions, ShareCredentials, AuthMethod, ConnectionState as ShareConnectionState, RemoteFile, MountPoint, SavedConnection, ViewMode as SharesViewMode, ShareError};
pub use printersettings::{PrinterSettingsApp, Printer, PrinterState, PrinterType, ConnectionType as PrinterConnectionType, PrintJob, JobStatus, PrintSettings, PrinterCapabilities, PaperSize, PaperType, PrintQuality, ColorMode, DuplexMode, PaperTray, InkCartridge, CartridgeColor, ViewMode as PrinterViewMode};
pub use terminaltabs::{TerminalTabs, TerminalTab, TabId as TerminalTabId, TabState as TerminalTabState, TerminalProfile, TerminalPane, SplitDirection, SessionState, TabViewMode};
pub use syntax::{SyntaxHighlighter, Language, TokenType, Token, ColorScheme, HighlighterState, HighlightedLine};
pub use git::{GitPanel, Repository, Branch, Commit as GitCommit, Tag as GitTag, Remote, FileStatus, FileChange, FileDiff, DiffHunk, DiffLine, GitError, GitResult, RepoState, GitViewMode, StashEntry, MergeConflict, ObjectType};
pub use firewall::{FirewallApp, FirewallView, RuleFilter, LogEntry, QuickAction};
