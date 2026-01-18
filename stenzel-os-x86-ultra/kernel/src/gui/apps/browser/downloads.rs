//! Downloads Manager
//!
//! Download management for the browser.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use super::network::{HttpClient, HttpRequest, Url, NetworkError};

/// Download ID
pub type DownloadId = u32;

/// Download manager
pub struct DownloadManager {
    /// Downloads
    downloads: BTreeMap<DownloadId, Download>,
    /// Next download ID
    next_id: DownloadId,
    /// Download directory
    download_dir: String,
    /// Max concurrent downloads
    max_concurrent: usize,
    /// Active downloads count
    active_count: usize,
    /// HTTP client
    http_client: HttpClient,
    /// Download callbacks
    on_progress: Option<fn(DownloadId, u64, u64)>,
    on_complete: Option<fn(DownloadId)>,
    on_error: Option<fn(DownloadId, DownloadError)>,
}

impl DownloadManager {
    /// Create new download manager
    pub fn new() -> Self {
        Self {
            downloads: BTreeMap::new(),
            next_id: 1,
            download_dir: String::from("/home/Downloads"),
            max_concurrent: 5,
            active_count: 0,
            http_client: HttpClient::new(),
            on_progress: None,
            on_complete: None,
            on_error: None,
        }
    }

    /// Set download directory
    pub fn set_download_dir(&mut self, dir: &str) {
        self.download_dir = String::from(dir);
    }

    /// Get download directory
    pub fn download_dir(&self) -> &str {
        &self.download_dir
    }

    /// Start a download
    pub fn download(&mut self, url: &str) -> Result<DownloadId, DownloadError> {
        self.download_with_options(url, DownloadOptions::default())
    }

    /// Start a download with options
    pub fn download_with_options(&mut self, url: &str, options: DownloadOptions) -> Result<DownloadId, DownloadError> {
        // Parse URL
        let parsed_url = Url::parse(url).map_err(|_| DownloadError::InvalidUrl)?;

        // Generate filename
        let filename = options.filename.unwrap_or_else(|| {
            // Extract from URL
            let path = &parsed_url.path;
            if let Some(pos) = path.rfind('/') {
                let name = &path[pos + 1..];
                if !name.is_empty() && !name.contains('?') {
                    return String::from(name);
                }
            }
            // Default filename
            alloc::format!("download_{}", self.next_id)
        });

        let file_path = if options.save_path.is_some() {
            options.save_path.unwrap()
        } else {
            alloc::format!("{}/{}", self.download_dir, filename)
        };

        // Create download
        let id = self.next_id;
        self.next_id += 1;

        let download = Download {
            id,
            url: String::from(url),
            filename,
            file_path,
            state: DownloadState::Queued,
            bytes_downloaded: 0,
            total_bytes: 0,
            progress: 0.0,
            speed: 0,
            eta: 0,
            error: None,
            mime_type: None,
            started_at: 0,
            completed_at: None,
            resumable: false,
            data: Vec::new(),
        };

        self.downloads.insert(id, download);

        // Start if under concurrent limit
        if self.active_count < self.max_concurrent {
            self.start_download(id)?;
        }

        Ok(id)
    }

    /// Start a queued download
    fn start_download(&mut self, id: DownloadId) -> Result<(), DownloadError> {
        let download = self.downloads.get_mut(&id).ok_or(DownloadError::NotFound)?;

        if download.state != DownloadState::Queued {
            return Ok(());
        }

        download.state = DownloadState::Downloading;
        download.started_at = crate::time::uptime_secs();
        self.active_count += 1;

        // Make HTTP request
        let url = download.url.clone();
        let response = self.http_client.get(&url).map_err(|e| DownloadError::Network(e))?;

        if !response.is_success() {
            let download = self.downloads.get_mut(&id).unwrap();
            download.state = DownloadState::Failed;
            download.error = Some(alloc::format!("HTTP {}", response.status));
            self.active_count = self.active_count.saturating_sub(1);
            return Err(DownloadError::HttpError(response.status));
        }

        // Get content info
        let total_bytes = response.content_length().unwrap_or(0) as u64;
        let mime_type = response.content_type().map(String::from);

        // Check for resume support
        let resumable = response.header("accept-ranges") == Some("bytes");

        // Store data
        let data = response.body.clone();

        let download = self.downloads.get_mut(&id).unwrap();
        download.total_bytes = total_bytes;
        download.mime_type = mime_type;
        download.resumable = resumable;
        download.bytes_downloaded = data.len() as u64;
        download.data = data;

        // Calculate progress
        if total_bytes > 0 {
            download.progress = (download.bytes_downloaded as f32 / total_bytes as f32) * 100.0;
        } else {
            download.progress = 100.0;
        }

        // Mark complete
        download.state = DownloadState::Completed;
        download.completed_at = Some(crate::time::uptime_secs());
        self.active_count = self.active_count.saturating_sub(1);

        // Save to file
        self.save_download(id)?;

        // Call callback
        if let Some(callback) = self.on_complete {
            callback(id);
        }

        // Start next queued download
        self.process_queue();

        Ok(())
    }

    /// Save download to file
    fn save_download(&self, id: DownloadId) -> Result<(), DownloadError> {
        let download = self.downloads.get(&id).ok_or(DownloadError::NotFound)?;

        let cred = crate::security::Cred::root();
        let mode = crate::fs::vfs::Mode::from_bits_truncate(0o644);

        crate::fs::write_file(&download.file_path, &cred, mode, &download.data)
            .map_err(|_| DownloadError::IoError)?;

        Ok(())
    }

    /// Process download queue
    fn process_queue(&mut self) {
        if self.active_count >= self.max_concurrent {
            return;
        }

        // Find next queued download
        let queued: Vec<DownloadId> = self.downloads
            .iter()
            .filter(|(_, d)| d.state == DownloadState::Queued)
            .map(|(&id, _)| id)
            .collect();

        for id in queued {
            if self.active_count >= self.max_concurrent {
                break;
            }
            let _ = self.start_download(id);
        }
    }

    /// Pause a download
    pub fn pause(&mut self, id: DownloadId) -> Result<(), DownloadError> {
        let download = self.downloads.get_mut(&id).ok_or(DownloadError::NotFound)?;

        if download.state != DownloadState::Downloading {
            return Ok(());
        }

        download.state = DownloadState::Paused;
        self.active_count = self.active_count.saturating_sub(1);

        Ok(())
    }

    /// Resume a download
    pub fn resume(&mut self, id: DownloadId) -> Result<(), DownloadError> {
        let download = self.downloads.get_mut(&id).ok_or(DownloadError::NotFound)?;

        if download.state != DownloadState::Paused {
            return Ok(());
        }

        if !download.resumable {
            // Restart from beginning
            download.bytes_downloaded = 0;
            download.data.clear();
        }

        download.state = DownloadState::Queued;
        self.process_queue();

        Ok(())
    }

    /// Cancel a download
    pub fn cancel(&mut self, id: DownloadId) -> Result<(), DownloadError> {
        let download = self.downloads.get_mut(&id).ok_or(DownloadError::NotFound)?;

        if download.state == DownloadState::Downloading {
            self.active_count = self.active_count.saturating_sub(1);
        }

        download.state = DownloadState::Cancelled;
        download.data.clear();

        self.process_queue();

        Ok(())
    }

    /// Remove a download
    pub fn remove(&mut self, id: DownloadId) -> Result<(), DownloadError> {
        let download = self.downloads.get(&id).ok_or(DownloadError::NotFound)?;

        if download.state == DownloadState::Downloading {
            self.active_count = self.active_count.saturating_sub(1);
        }

        self.downloads.remove(&id);
        self.process_queue();

        Ok(())
    }

    /// Retry a failed download
    pub fn retry(&mut self, id: DownloadId) -> Result<(), DownloadError> {
        let download = self.downloads.get_mut(&id).ok_or(DownloadError::NotFound)?;

        if download.state != DownloadState::Failed {
            return Ok(());
        }

        download.state = DownloadState::Queued;
        download.bytes_downloaded = 0;
        download.data.clear();
        download.error = None;

        self.process_queue();

        Ok(())
    }

    /// Get download
    pub fn get(&self, id: DownloadId) -> Option<&Download> {
        self.downloads.get(&id)
    }

    /// Get all downloads
    pub fn all(&self) -> Vec<&Download> {
        self.downloads.values().collect()
    }

    /// Get active downloads
    pub fn active(&self) -> Vec<&Download> {
        self.downloads.values()
            .filter(|d| d.state == DownloadState::Downloading)
            .collect()
    }

    /// Get completed downloads
    pub fn completed(&self) -> Vec<&Download> {
        self.downloads.values()
            .filter(|d| d.state == DownloadState::Completed)
            .collect()
    }

    /// Get failed downloads
    pub fn failed(&self) -> Vec<&Download> {
        self.downloads.values()
            .filter(|d| d.state == DownloadState::Failed)
            .collect()
    }

    /// Clear completed downloads
    pub fn clear_completed(&mut self) {
        self.downloads.retain(|_, d| d.state != DownloadState::Completed);
    }

    /// Clear all downloads
    pub fn clear_all(&mut self) {
        // Cancel active downloads
        let active: Vec<DownloadId> = self.downloads
            .iter()
            .filter(|(_, d)| d.state == DownloadState::Downloading)
            .map(|(&id, _)| id)
            .collect();

        for id in active {
            let _ = self.cancel(id);
        }

        self.downloads.clear();
    }

    /// Set callbacks
    pub fn on_progress(&mut self, callback: fn(DownloadId, u64, u64)) {
        self.on_progress = Some(callback);
    }

    pub fn on_complete(&mut self, callback: fn(DownloadId)) {
        self.on_complete = Some(callback);
    }

    pub fn on_error(&mut self, callback: fn(DownloadId, DownloadError)) {
        self.on_error = Some(callback);
    }

    /// Get total active download count
    pub fn active_count(&self) -> usize {
        self.active_count
    }

    /// Get total download count
    pub fn download_count(&self) -> usize {
        self.downloads.len()
    }

    /// Open download folder
    pub fn open_folder(&self) {
        // This would integrate with the file manager
        crate::kprintln!("Open folder: {}", self.download_dir);
    }

    /// Open completed download file
    pub fn open_file(&self, id: DownloadId) -> Result<(), DownloadError> {
        let download = self.downloads.get(&id).ok_or(DownloadError::NotFound)?;

        if download.state != DownloadState::Completed {
            return Err(DownloadError::NotCompleted);
        }

        // This would open the file with the default application
        crate::kprintln!("Open file: {}", download.file_path);

        Ok(())
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Download
#[derive(Debug, Clone)]
pub struct Download {
    /// Download ID
    pub id: DownloadId,
    /// Source URL
    pub url: String,
    /// Filename
    pub filename: String,
    /// Full file path
    pub file_path: String,
    /// Download state
    pub state: DownloadState,
    /// Bytes downloaded
    pub bytes_downloaded: u64,
    /// Total bytes
    pub total_bytes: u64,
    /// Progress (0-100)
    pub progress: f32,
    /// Download speed (bytes/sec)
    pub speed: u64,
    /// Estimated time remaining (seconds)
    pub eta: u64,
    /// Error message if failed
    pub error: Option<String>,
    /// MIME type
    pub mime_type: Option<String>,
    /// Started at timestamp
    pub started_at: u64,
    /// Completed at timestamp
    pub completed_at: Option<u64>,
    /// Is resumable
    pub resumable: bool,
    /// Downloaded data
    data: Vec<u8>,
}

impl Download {
    /// Get formatted size
    pub fn size_str(&self) -> String {
        format_bytes(self.total_bytes)
    }

    /// Get formatted downloaded size
    pub fn downloaded_str(&self) -> String {
        format_bytes(self.bytes_downloaded)
    }

    /// Get formatted speed
    pub fn speed_str(&self) -> String {
        alloc::format!("{}/s", format_bytes(self.speed))
    }

    /// Get formatted ETA
    pub fn eta_str(&self) -> String {
        if self.eta == 0 {
            return String::from("--");
        }

        let hours = self.eta / 3600;
        let minutes = (self.eta % 3600) / 60;
        let seconds = self.eta % 60;

        if hours > 0 {
            alloc::format!("{}h {}m", hours, minutes)
        } else if minutes > 0 {
            alloc::format!("{}m {}s", minutes, seconds)
        } else {
            alloc::format!("{}s", seconds)
        }
    }

    /// Get file extension
    pub fn extension(&self) -> Option<&str> {
        self.filename.rfind('.').map(|pos| &self.filename[pos + 1..])
    }

    /// Get icon for file type
    pub fn icon(&self) -> &'static str {
        match self.extension() {
            Some("pdf") => "document-pdf",
            Some("doc") | Some("docx") => "document-word",
            Some("xls") | Some("xlsx") => "document-excel",
            Some("ppt") | Some("pptx") => "document-powerpoint",
            Some("txt") | Some("md") => "document-text",
            Some("zip") | Some("rar") | Some("7z") | Some("tar") | Some("gz") => "archive",
            Some("jpg") | Some("jpeg") | Some("png") | Some("gif") | Some("bmp") | Some("webp") => "image",
            Some("mp3") | Some("wav") | Some("flac") | Some("aac") | Some("ogg") => "audio",
            Some("mp4") | Some("mkv") | Some("avi") | Some("webm") | Some("mov") => "video",
            Some("exe") | Some("msi") | Some("dmg") | Some("deb") | Some("rpm") => "application",
            Some("html") | Some("htm") | Some("css") | Some("js") => "code",
            _ => "file",
        }
    }

    /// Is download complete?
    pub fn is_complete(&self) -> bool {
        self.state == DownloadState::Completed
    }

    /// Is download in progress?
    pub fn is_active(&self) -> bool {
        self.state == DownloadState::Downloading
    }

    /// Is download paused?
    pub fn is_paused(&self) -> bool {
        self.state == DownloadState::Paused
    }

    /// Is download failed?
    pub fn is_failed(&self) -> bool {
        self.state == DownloadState::Failed
    }
}

/// Download state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DownloadState {
    /// Queued, waiting to start
    Queued,
    /// Currently downloading
    Downloading,
    /// Paused
    Paused,
    /// Completed successfully
    Completed,
    /// Failed
    Failed,
    /// Cancelled
    Cancelled,
}

/// Download options
#[derive(Debug, Clone)]
pub struct DownloadOptions {
    /// Custom filename
    pub filename: Option<String>,
    /// Custom save path
    pub save_path: Option<String>,
    /// Whether to overwrite existing files
    pub overwrite: bool,
    /// Whether to open when complete
    pub open_when_complete: bool,
    /// Priority (higher = higher priority)
    pub priority: i32,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            filename: None,
            save_path: None,
            overwrite: false,
            open_when_complete: false,
            priority: 0,
        }
    }
}

/// Download error
#[derive(Debug, Clone)]
pub enum DownloadError {
    /// Invalid URL
    InvalidUrl,
    /// Network error
    Network(NetworkError),
    /// HTTP error
    HttpError(u16),
    /// IO error
    IoError,
    /// Download not found
    NotFound,
    /// Download not completed
    NotCompleted,
    /// Disk full
    DiskFull,
    /// Permission denied
    PermissionDenied,
    /// File already exists
    FileExists,
    /// Cancelled
    Cancelled,
}

/// Format bytes to human-readable string
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        alloc::format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        alloc::format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        alloc::format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        alloc::format!("{} B", bytes)
    }
}

/// Download history entry
#[derive(Debug, Clone)]
pub struct DownloadHistoryEntry {
    /// URL
    pub url: String,
    /// Filename
    pub filename: String,
    /// File path
    pub file_path: String,
    /// Size
    pub size: u64,
    /// Downloaded at
    pub downloaded_at: u64,
    /// MIME type
    pub mime_type: Option<String>,
}

/// Download history
pub struct DownloadHistory {
    /// History entries
    entries: Vec<DownloadHistoryEntry>,
    /// Max entries to keep
    max_entries: usize,
}

impl DownloadHistory {
    /// Create new history
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 1000,
        }
    }

    /// Add entry
    pub fn add(&mut self, entry: DownloadHistoryEntry) {
        self.entries.push(entry);

        // Trim if over limit
        while self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    /// Get all entries
    pub fn all(&self) -> &[DownloadHistoryEntry] {
        &self.entries
    }

    /// Search entries
    pub fn search(&self, query: &str) -> Vec<&DownloadHistoryEntry> {
        let query = query.to_lowercase();
        self.entries.iter()
            .filter(|e| {
                e.filename.to_lowercase().contains(&query) ||
                e.url.to_lowercase().contains(&query)
            })
            .collect()
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get entries from last N days
    pub fn recent(&self, days: u64) -> Vec<&DownloadHistoryEntry> {
        let cutoff = crate::time::uptime_secs().saturating_sub(days * 24 * 60 * 60);
        self.entries.iter()
            .filter(|e| e.downloaded_at >= cutoff)
            .collect()
    }
}

impl Default for DownloadHistory {
    fn default() -> Self {
        Self::new()
    }
}
