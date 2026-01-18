//! Thumbnail Generator and Cache
//!
//! Generates, caches, and manages thumbnails for images, videos, and documents.
//! Follows the freedesktop.org thumbnail specification.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;

use spin::Mutex;

/// Thumbnail sizes following freedesktop.org specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThumbnailSize {
    /// 128x128 pixels
    Normal,
    /// 256x256 pixels
    Large,
    /// 512x512 pixels (freedesktop.org extension)
    XLarge,
    /// 1024x1024 pixels (freedesktop.org extension)
    XXLarge,
}

impl ThumbnailSize {
    pub fn pixels(&self) -> usize {
        match self {
            ThumbnailSize::Normal => 128,
            ThumbnailSize::Large => 256,
            ThumbnailSize::XLarge => 512,
            ThumbnailSize::XXLarge => 1024,
        }
    }

    pub fn directory_name(&self) -> &'static str {
        match self {
            ThumbnailSize::Normal => "normal",
            ThumbnailSize::Large => "large",
            ThumbnailSize::XLarge => "x-large",
            ThumbnailSize::XXLarge => "xx-large",
        }
    }
}

/// Supported file types for thumbnail generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailableType {
    Image,
    Video,
    Pdf,
    Document,
    Font,
    Archive,
    Unknown,
}

impl ThumbnailableType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            // Images
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tiff" | "tif" |
            "ico" | "svg" | "heic" | "heif" | "avif" | "jxl" => ThumbnailableType::Image,

            // Videos
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" |
            "m4v" | "mpeg" | "mpg" | "3gp" | "ogv" => ThumbnailableType::Video,

            // PDF
            "pdf" => ThumbnailableType::Pdf,

            // Documents
            "doc" | "docx" | "odt" | "rtf" | "txt" | "xls" | "xlsx" |
            "ppt" | "pptx" | "odp" | "ods" => ThumbnailableType::Document,

            // Fonts
            "ttf" | "otf" | "woff" | "woff2" => ThumbnailableType::Font,

            // Archives
            "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => ThumbnailableType::Archive,

            _ => ThumbnailableType::Unknown,
        }
    }

    pub fn can_generate_thumbnail(&self) -> bool {
        !matches!(self, ThumbnailableType::Unknown)
    }

    pub fn priority(&self) -> u8 {
        // Lower number = higher priority for generation
        match self {
            ThumbnailableType::Image => 0,
            ThumbnailableType::Video => 2,
            ThumbnailableType::Pdf => 1,
            ThumbnailableType::Document => 3,
            ThumbnailableType::Font => 4,
            ThumbnailableType::Archive => 5,
            ThumbnailableType::Unknown => 255,
        }
    }
}

/// Thumbnail generation status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailStatus {
    /// Thumbnail not yet generated
    Pending,
    /// Currently being generated
    Generating,
    /// Successfully generated and cached
    Ready,
    /// Generation failed
    Failed,
    /// File type not supported
    Unsupported,
    /// File too large for thumbnail
    TooLarge,
    /// File has changed since thumbnail was generated
    Stale,
}

/// Thumbnail metadata stored with cached thumbnails
#[derive(Debug, Clone)]
pub struct ThumbnailMetadata {
    /// Original file URI
    pub uri: String,
    /// Original file modification time
    pub mtime: u64,
    /// Original file size
    pub file_size: u64,
    /// Thumbnail width
    pub width: u32,
    /// Thumbnail height: u32,
    pub height: u32,
    /// MIME type of original
    pub mime_type: String,
    /// MD5 hash of URI (used for filename)
    pub uri_hash: String,
    /// Thumbnail generation time
    pub thumbnail_mtime: u64,
    /// Software that generated it
    pub software: String,
}

impl ThumbnailMetadata {
    pub fn new(uri: &str, mtime: u64, file_size: u64) -> Self {
        let uri_hash = simple_md5_hex(uri);

        ThumbnailMetadata {
            uri: uri.to_string(),
            mtime,
            file_size,
            width: 0,
            height: 0,
            mime_type: String::new(),
            uri_hash,
            thumbnail_mtime: 0,
            software: "Stenzel OS Thumbnail Generator".to_string(),
        }
    }

    pub fn thumbnail_filename(&self, size: ThumbnailSize) -> String {
        format!("{}/{}.png", size.directory_name(), self.uri_hash)
    }

    pub fn is_valid(&self, current_mtime: u64) -> bool {
        self.mtime == current_mtime
    }
}

/// A cached thumbnail entry
#[derive(Debug, Clone)]
pub struct CachedThumbnail {
    /// Thumbnail metadata
    pub metadata: ThumbnailMetadata,
    /// Thumbnail size category
    pub size: ThumbnailSize,
    /// Status
    pub status: ThumbnailStatus,
    /// Pixel data (RGBA format)
    pub pixels: Option<Vec<u8>>,
    /// Actual dimensions after scaling
    pub actual_width: u32,
    pub actual_height: u32,
    /// Last access time for LRU eviction
    pub last_accessed: u64,
}

impl CachedThumbnail {
    pub fn new(metadata: ThumbnailMetadata, size: ThumbnailSize) -> Self {
        CachedThumbnail {
            metadata,
            size,
            status: ThumbnailStatus::Pending,
            pixels: None,
            actual_width: 0,
            actual_height: 0,
            last_accessed: 0,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.status == ThumbnailStatus::Ready && self.pixels.is_some()
    }
}

/// Request for thumbnail generation
#[derive(Debug, Clone)]
pub struct ThumbnailRequest {
    /// File path
    pub path: String,
    /// File URI (file:// format)
    pub uri: String,
    /// Requested size
    pub size: ThumbnailSize,
    /// File modification time
    pub mtime: u64,
    /// File size
    pub file_size: u64,
    /// File type
    pub file_type: ThumbnailableType,
    /// Priority (lower = higher priority)
    pub priority: u8,
    /// Request ID
    pub request_id: u64,
}

impl ThumbnailRequest {
    pub fn new(path: &str, size: ThumbnailSize, mtime: u64, file_size: u64) -> Self {
        let uri = format!("file://{}", path);
        let extension = path.rsplit('.').next().unwrap_or("");
        let file_type = ThumbnailableType::from_extension(extension);

        static mut REQUEST_COUNTER: u64 = 0;
        let request_id = unsafe {
            REQUEST_COUNTER += 1;
            REQUEST_COUNTER
        };

        ThumbnailRequest {
            path: path.to_string(),
            uri,
            size,
            mtime,
            file_size,
            file_type,
            priority: file_type.priority(),
            request_id,
        }
    }
}

/// Thumbnail generation result
#[derive(Debug, Clone)]
pub struct ThumbnailResult {
    /// Request that was processed
    pub request_id: u64,
    /// Path of original file
    pub path: String,
    /// Whether generation succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Generated thumbnail (if successful)
    pub thumbnail: Option<CachedThumbnail>,
}

/// Configuration for thumbnail generator
#[derive(Debug, Clone)]
pub struct ThumbnailConfig {
    /// Cache directory (e.g., ~/.cache/thumbnails)
    pub cache_dir: String,
    /// Maximum cache size in bytes
    pub max_cache_size: u64,
    /// Maximum file size to thumbnail (in bytes)
    pub max_file_size: u64,
    /// Number of concurrent generation workers
    pub worker_count: usize,
    /// Generate thumbnails for hidden files
    pub thumbnail_hidden: bool,
    /// Default thumbnail size
    pub default_size: ThumbnailSize,
    /// Enable video thumbnails
    pub enable_video_thumbnails: bool,
    /// Video thumbnail position (percentage, 0-100)
    pub video_thumbnail_position: u8,
}

impl Default for ThumbnailConfig {
    fn default() -> Self {
        ThumbnailConfig {
            cache_dir: "/home/user/.cache/thumbnails".to_string(),
            max_cache_size: 256 * 1024 * 1024, // 256 MB
            max_file_size: 100 * 1024 * 1024,  // 100 MB
            worker_count: 2,
            thumbnail_hidden: false,
            default_size: ThumbnailSize::Normal,
            enable_video_thumbnails: true,
            video_thumbnail_position: 10, // 10% into video
        }
    }
}

/// Statistics about thumbnail cache
#[derive(Debug, Clone, Default)]
pub struct ThumbnailStats {
    /// Total thumbnails cached
    pub cached_count: usize,
    /// Cache size in bytes
    pub cache_size: u64,
    /// Pending requests
    pub pending_requests: usize,
    /// Thumbnails generated this session
    pub generated_count: u64,
    /// Generation failures this session
    pub failed_count: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// By type breakdown
    pub by_type: BTreeMap<String, usize>,
    /// By size breakdown
    pub by_size: BTreeMap<String, usize>,
}

impl ThumbnailStats {
    pub fn hit_rate(&self) -> f32 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f32 / total as f32 * 100.0
        }
    }
}

/// Thumbnail cache manager
pub struct ThumbnailCache {
    /// Cached thumbnails indexed by URI hash + size
    cache: BTreeMap<String, CachedThumbnail>,
    /// Cache size in bytes
    cache_size: u64,
    /// Configuration
    config: ThumbnailConfig,
    /// Statistics
    stats: ThumbnailStats,
    /// Pending requests
    pending: Vec<ThumbnailRequest>,
    /// Failed URIs (to avoid retrying)
    failed_uris: Vec<String>,
    /// Current simulated time
    current_time: u64,
}

impl ThumbnailCache {
    pub fn new(config: ThumbnailConfig) -> Self {
        ThumbnailCache {
            cache: BTreeMap::new(),
            cache_size: 0,
            config,
            stats: ThumbnailStats::default(),
            pending: Vec::new(),
            failed_uris: Vec::new(),
            current_time: 1737216000,
        }
    }

    /// Get a cached thumbnail if available
    pub fn get(&mut self, path: &str, size: ThumbnailSize) -> Option<&CachedThumbnail> {
        let uri = format!("file://{}", path);
        let uri_hash = simple_md5_hex(&uri);
        let key = format!("{}:{}", uri_hash, size.directory_name());

        if self.cache.contains_key(&key) {
            self.stats.cache_hits += 1;
            // Update last accessed time
            if let Some(thumb) = self.cache.get_mut(&key) {
                thumb.last_accessed = self.current_time;
            }
            self.cache.get(&key)
        } else {
            self.stats.cache_misses += 1;
            None
        }
    }

    /// Request thumbnail generation
    pub fn request(&mut self, path: &str, size: ThumbnailSize, mtime: u64, file_size: u64) -> u64 {
        let request = ThumbnailRequest::new(path, size, mtime, file_size);
        let request_id = request.request_id;

        // Check if already cached
        let uri_hash = simple_md5_hex(&request.uri);
        let key = format!("{}:{}", uri_hash, size.directory_name());

        if let Some(cached) = self.cache.get(&key) {
            if cached.metadata.is_valid(mtime) && cached.is_ready() {
                return request_id; // Already have valid thumbnail
            }
        }

        // Check if file type is supported
        if !request.file_type.can_generate_thumbnail() {
            return request_id;
        }

        // Check if file is too large
        if file_size > self.config.max_file_size {
            return request_id;
        }

        // Check if already pending
        if self.pending.iter().any(|r| r.path == path && r.size == size) {
            return request_id;
        }

        // Check if previously failed
        if self.failed_uris.contains(&request.uri) {
            return request_id;
        }

        // Add to pending queue
        self.pending.push(request);
        self.stats.pending_requests = self.pending.len();

        // Sort by priority
        self.pending.sort_by(|a, b| a.priority.cmp(&b.priority));

        request_id
    }

    /// Process pending requests (call periodically)
    pub fn process_pending(&mut self, max_count: usize) -> Vec<ThumbnailResult> {
        let mut results = Vec::new();
        let count = max_count.min(self.pending.len());

        for _ in 0..count {
            if let Some(request) = self.pending.pop() {
                let result = self.generate_thumbnail(&request);
                results.push(result);
            }
        }

        self.stats.pending_requests = self.pending.len();
        results
    }

    /// Generate a thumbnail for a request
    fn generate_thumbnail(&mut self, request: &ThumbnailRequest) -> ThumbnailResult {
        let metadata = ThumbnailMetadata::new(&request.uri, request.mtime, request.file_size);
        let mut cached = CachedThumbnail::new(metadata, request.size);
        cached.status = ThumbnailStatus::Generating;

        // Simulate thumbnail generation based on file type
        let result = match request.file_type {
            ThumbnailableType::Image => self.generate_image_thumbnail(request, &mut cached),
            ThumbnailableType::Video => self.generate_video_thumbnail(request, &mut cached),
            ThumbnailableType::Pdf => self.generate_pdf_thumbnail(request, &mut cached),
            ThumbnailableType::Document => self.generate_document_thumbnail(request, &mut cached),
            ThumbnailableType::Font => self.generate_font_thumbnail(request, &mut cached),
            ThumbnailableType::Archive => self.generate_archive_thumbnail(request, &mut cached),
            ThumbnailableType::Unknown => Err("Unsupported file type".to_string()),
        };

        match result {
            Ok(()) => {
                cached.status = ThumbnailStatus::Ready;
                cached.last_accessed = self.current_time;

                // Add to cache
                let key = format!("{}:{}", cached.metadata.uri_hash, request.size.directory_name());

                // Update cache size
                if let Some(pixels) = &cached.pixels {
                    self.cache_size += pixels.len() as u64;
                }

                self.cache.insert(key.clone(), cached.clone());
                self.stats.generated_count += 1;

                // Update type stats
                let type_name = format!("{:?}", request.file_type);
                *self.stats.by_type.entry(type_name).or_insert(0) += 1;

                // Update size stats
                let size_name = request.size.directory_name().to_string();
                *self.stats.by_size.entry(size_name).or_insert(0) += 1;

                self.stats.cached_count = self.cache.len();

                // Evict if cache too large
                self.evict_if_needed();

                ThumbnailResult {
                    request_id: request.request_id,
                    path: request.path.clone(),
                    success: true,
                    error: None,
                    thumbnail: Some(cached),
                }
            }
            Err(error) => {
                cached.status = ThumbnailStatus::Failed;
                self.failed_uris.push(request.uri.clone());
                self.stats.failed_count += 1;

                ThumbnailResult {
                    request_id: request.request_id,
                    path: request.path.clone(),
                    success: false,
                    error: Some(error),
                    thumbnail: None,
                }
            }
        }
    }

    fn generate_image_thumbnail(&self, request: &ThumbnailRequest, cached: &mut CachedThumbnail) -> Result<(), String> {
        let size = request.size.pixels();

        // Simulate generating a placeholder thumbnail
        // In reality, this would load the image, scale it, and save to PNG
        let pixel_count = size * size * 4; // RGBA
        let mut pixels = vec![0u8; pixel_count];

        // Create a simple gradient pattern as placeholder
        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;
                pixels[idx] = (x * 255 / size) as u8;     // R
                pixels[idx + 1] = (y * 255 / size) as u8; // G
                pixels[idx + 2] = 128;                     // B
                pixels[idx + 3] = 255;                     // A
            }
        }

        cached.pixels = Some(pixels);
        cached.actual_width = size as u32;
        cached.actual_height = size as u32;
        cached.metadata.width = size as u32;
        cached.metadata.height = size as u32;
        cached.metadata.mime_type = "image/png".to_string();
        cached.metadata.thumbnail_mtime = self.current_time;

        Ok(())
    }

    fn generate_video_thumbnail(&self, request: &ThumbnailRequest, cached: &mut CachedThumbnail) -> Result<(), String> {
        if !self.config.enable_video_thumbnails {
            return Err("Video thumbnails disabled".to_string());
        }

        let size = request.size.pixels();

        // Simulate video frame extraction
        // In reality, this would use ffmpeg or similar to extract a frame
        let pixel_count = size * size * 4;
        let mut pixels = vec![0u8; pixel_count];

        // Create a film-strip pattern as placeholder
        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;
                let is_border = y < 8 || y >= size - 8;
                let is_sprocket = is_border && (x / 16) % 2 == 0;

                if is_sprocket {
                    pixels[idx] = 50;      // Dark
                    pixels[idx + 1] = 50;
                    pixels[idx + 2] = 50;
                } else if is_border {
                    pixels[idx] = 30;
                    pixels[idx + 1] = 30;
                    pixels[idx + 2] = 30;
                } else {
                    // Video frame area
                    pixels[idx] = 80 + ((x + y) % 50) as u8;
                    pixels[idx + 1] = 80 + ((x * 2) % 50) as u8;
                    pixels[idx + 2] = 100;
                }
                pixels[idx + 3] = 255;
            }
        }

        cached.pixels = Some(pixels);
        cached.actual_width = size as u32;
        cached.actual_height = size as u32;
        cached.metadata.width = size as u32;
        cached.metadata.height = size as u32;
        cached.metadata.mime_type = "image/png".to_string();
        cached.metadata.thumbnail_mtime = self.current_time;

        Ok(())
    }

    fn generate_pdf_thumbnail(&self, request: &ThumbnailRequest, cached: &mut CachedThumbnail) -> Result<(), String> {
        let size = request.size.pixels();

        // Simulate PDF first page rendering
        let pixel_count = size * size * 4;
        let mut pixels = vec![0u8; pixel_count];

        // Create a document-like pattern
        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;
                let margin = size / 10;
                let in_content = x > margin && x < size - margin && y > margin && y < size - margin;

                if in_content {
                    // Text lines simulation
                    let line_y = (y - margin) % 12;
                    if line_y < 8 && x < size - margin * 2 {
                        pixels[idx] = 60;
                        pixels[idx + 1] = 60;
                        pixels[idx + 2] = 60;
                    } else {
                        pixels[idx] = 250;
                        pixels[idx + 1] = 250;
                        pixels[idx + 2] = 250;
                    }
                } else {
                    // Page background
                    pixels[idx] = 240;
                    pixels[idx + 1] = 240;
                    pixels[idx + 2] = 240;
                }
                pixels[idx + 3] = 255;
            }
        }

        cached.pixels = Some(pixels);
        cached.actual_width = size as u32;
        cached.actual_height = size as u32;
        cached.metadata.mime_type = "image/png".to_string();
        cached.metadata.thumbnail_mtime = self.current_time;

        Ok(())
    }

    fn generate_document_thumbnail(&self, request: &ThumbnailRequest, cached: &mut CachedThumbnail) -> Result<(), String> {
        // Similar to PDF but with different icon
        self.generate_pdf_thumbnail(request, cached)
    }

    fn generate_font_thumbnail(&self, request: &ThumbnailRequest, cached: &mut CachedThumbnail) -> Result<(), String> {
        let size = request.size.pixels();
        let pixel_count = size * size * 4;
        let mut pixels = vec![0u8; pixel_count];

        // Create an "Aa" pattern for fonts
        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;
                // White background
                pixels[idx] = 255;
                pixels[idx + 1] = 255;
                pixels[idx + 2] = 255;
                pixels[idx + 3] = 255;
            }
        }

        // Draw a simple "A" shape in the center
        let center = size / 2;
        let letter_height = size / 2;
        let letter_width = size / 3;
        let start_x = center - letter_width / 2;
        let start_y = center - letter_height / 2;

        for dy in 0..letter_height {
            let y = start_y + dy;
            let progress = dy as f32 / letter_height as f32;
            let width_at_y = (letter_width as f32 * progress).max(2.0) as usize;

            for dx in 0..width_at_y.min(letter_width) {
                let x1 = start_x + (letter_width - width_at_y) / 2 + dx;
                let x2 = start_x + letter_width - 1 - (letter_width - width_at_y) / 2 - dx;

                if x1 < size && y < size {
                    let idx = (y * size + x1) * 4;
                    pixels[idx] = 40;
                    pixels[idx + 1] = 40;
                    pixels[idx + 2] = 40;
                }
                if x2 < size && y < size && x2 != x1 {
                    let idx = (y * size + x2) * 4;
                    pixels[idx] = 40;
                    pixels[idx + 1] = 40;
                    pixels[idx + 2] = 40;
                }
            }
        }

        cached.pixels = Some(pixels);
        cached.actual_width = size as u32;
        cached.actual_height = size as u32;
        cached.metadata.mime_type = "image/png".to_string();
        cached.metadata.thumbnail_mtime = self.current_time;

        Ok(())
    }

    fn generate_archive_thumbnail(&self, request: &ThumbnailRequest, cached: &mut CachedThumbnail) -> Result<(), String> {
        let size = request.size.pixels();
        let pixel_count = size * size * 4;
        let mut pixels = vec![0u8; pixel_count];

        // Create a folder/archive icon pattern
        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;

                let margin = size / 8;
                let tab_height = size / 6;
                let tab_width = size / 3;

                let is_in_folder = x >= margin && x < size - margin &&
                                   y >= margin + tab_height && y < size - margin;
                let is_in_tab = x >= margin && x < margin + tab_width &&
                               y >= margin && y < margin + tab_height;

                if is_in_folder || is_in_tab {
                    // Folder color
                    pixels[idx] = 230;
                    pixels[idx + 1] = 180;
                    pixels[idx + 2] = 80;
                } else {
                    // Background
                    pixels[idx] = 245;
                    pixels[idx + 1] = 245;
                    pixels[idx + 2] = 245;
                }
                pixels[idx + 3] = 255;
            }
        }

        cached.pixels = Some(pixels);
        cached.actual_width = size as u32;
        cached.actual_height = size as u32;
        cached.metadata.mime_type = "image/png".to_string();
        cached.metadata.thumbnail_mtime = self.current_time;

        Ok(())
    }

    /// Evict old thumbnails if cache is too large
    fn evict_if_needed(&mut self) {
        while self.cache_size > self.config.max_cache_size && !self.cache.is_empty() {
            // Find oldest accessed thumbnail
            let oldest_key = self.cache
                .iter()
                .min_by_key(|(_, v)| v.last_accessed)
                .map(|(k, _)| k.clone());

            if let Some(key) = oldest_key {
                if let Some(removed) = self.cache.remove(&key) {
                    if let Some(pixels) = &removed.pixels {
                        self.cache_size = self.cache_size.saturating_sub(pixels.len() as u64);
                    }
                }
            }
        }
        self.stats.cached_count = self.cache.len();
        self.stats.cache_size = self.cache_size;
    }

    /// Clear all cached thumbnails
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.cache_size = 0;
        self.stats.cached_count = 0;
        self.stats.cache_size = 0;
    }

    /// Clear failed URIs to allow retry
    pub fn clear_failures(&mut self) {
        self.failed_uris.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> &ThumbnailStats {
        &self.stats
    }

    /// Get configuration
    pub fn config(&self) -> &ThumbnailConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: ThumbnailConfig) {
        self.config = config;
    }

    /// Check if a thumbnail is cached
    pub fn is_cached(&self, path: &str, size: ThumbnailSize) -> bool {
        let uri = format!("file://{}", path);
        let uri_hash = simple_md5_hex(&uri);
        let key = format!("{}:{}", uri_hash, size.directory_name());
        self.cache.contains_key(&key)
    }

    /// Get pending request count
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Cancel all pending requests
    pub fn cancel_pending(&mut self) {
        self.pending.clear();
        self.stats.pending_requests = 0;
    }

    /// Invalidate thumbnail for a specific path
    pub fn invalidate(&mut self, path: &str) {
        let uri = format!("file://{}", path);
        let uri_hash = simple_md5_hex(&uri);

        // Remove all sizes
        let keys_to_remove: Vec<String> = self.cache
            .keys()
            .filter(|k| k.starts_with(&uri_hash))
            .cloned()
            .collect();

        for key in keys_to_remove {
            if let Some(removed) = self.cache.remove(&key) {
                if let Some(pixels) = &removed.pixels {
                    self.cache_size = self.cache_size.saturating_sub(pixels.len() as u64);
                }
            }
        }
        self.stats.cached_count = self.cache.len();
        self.stats.cache_size = self.cache_size;
    }
}

impl Default for ThumbnailCache {
    fn default() -> Self {
        Self::new(ThumbnailConfig::default())
    }
}

/// Simple MD5-like hash for URI (not cryptographically secure)
/// In reality, this should be proper MD5
fn simple_md5_hex(input: &str) -> String {
    let mut hash: u64 = 0;
    for (i, byte) in input.bytes().enumerate() {
        hash = hash.wrapping_add(byte as u64);
        hash = hash.wrapping_mul(31);
        hash ^= (i as u64).wrapping_mul(17);
    }
    format!("{:016x}", hash)
}

/// Global thumbnail cache singleton
static THUMBNAIL_CACHE: Mutex<Option<ThumbnailCache>> = Mutex::new(None);

/// Initialize the thumbnail cache
pub fn init() {
    let mut cache = THUMBNAIL_CACHE.lock();
    if cache.is_none() {
        *cache = Some(ThumbnailCache::default());
    }
}

/// Initialize with custom configuration
pub fn init_with_config(config: ThumbnailConfig) {
    let mut cache = THUMBNAIL_CACHE.lock();
    *cache = Some(ThumbnailCache::new(config));
}

/// Get thumbnail for a file
pub fn get_thumbnail(path: &str, size: ThumbnailSize) -> Option<CachedThumbnail> {
    let mut cache = THUMBNAIL_CACHE.lock();
    if let Some(ref mut c) = *cache {
        c.get(path, size).cloned()
    } else {
        None
    }
}

/// Request thumbnail generation
pub fn request_thumbnail(path: &str, size: ThumbnailSize, mtime: u64, file_size: u64) -> u64 {
    let mut cache = THUMBNAIL_CACHE.lock();
    if let Some(ref mut c) = *cache {
        c.request(path, size, mtime, file_size)
    } else {
        0
    }
}

/// Process pending thumbnail requests
pub fn process_thumbnails(max_count: usize) -> Vec<ThumbnailResult> {
    let mut cache = THUMBNAIL_CACHE.lock();
    if let Some(ref mut c) = *cache {
        c.process_pending(max_count)
    } else {
        Vec::new()
    }
}

/// Get thumbnail cache statistics
pub fn get_stats() -> Option<ThumbnailStats> {
    let cache = THUMBNAIL_CACHE.lock();
    if let Some(ref c) = *cache {
        Some(c.stats().clone())
    } else {
        None
    }
}

/// Clear the thumbnail cache
pub fn clear_cache() {
    let mut cache = THUMBNAIL_CACHE.lock();
    if let Some(ref mut c) = *cache {
        c.clear_cache();
    }
}

/// Invalidate thumbnail for a path
pub fn invalidate_thumbnail(path: &str) {
    let mut cache = THUMBNAIL_CACHE.lock();
    if let Some(ref mut c) = *cache {
        c.invalidate(path);
    }
}

/// Check if thumbnail is cached
pub fn is_thumbnail_cached(path: &str, size: ThumbnailSize) -> bool {
    let cache = THUMBNAIL_CACHE.lock();
    if let Some(ref c) = *cache {
        c.is_cached(path, size)
    } else {
        false
    }
}
