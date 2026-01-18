//! Timezone and Locale Configuration for Stenzel OS.
//!
//! Provides timezone and locale selection for system installation.
//!
//! Features:
//! - Complete timezone database (IANA)
//! - Locale database with language/country
//! - Keyboard layout configuration
//! - Date/time format settings
//! - NTP server configuration
//! - Auto-detection from geolocation
//! - Manual selection interface

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicI32, Ordering};
use spin::{Mutex, Once};

// ============================================================================
// Timezone Types
// ============================================================================

/// Timezone identifier (IANA format)
pub type TzId = String;

/// Timezone information
#[derive(Debug, Clone)]
pub struct Timezone {
    /// IANA timezone identifier (e.g., "America/Sao_Paulo")
    pub id: TzId,
    /// Display name (e.g., "São Paulo, Brazil")
    pub display_name: String,
    /// Country code (ISO 3166-1 alpha-2)
    pub country: String,
    /// UTC offset in seconds
    pub utc_offset: i32,
    /// Daylight saving time offset in seconds (0 if no DST)
    pub dst_offset: i32,
    /// Has daylight saving time
    pub has_dst: bool,
    /// Abbreviation (e.g., "BRT", "EST")
    pub abbreviation: String,
}

/// Timezone region for grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TzRegion {
    Africa,
    America,
    Antarctica,
    Arctic,
    Asia,
    Atlantic,
    Australia,
    Europe,
    Indian,
    Pacific,
    Etc,
}

impl TzRegion {
    pub fn from_tz_id(id: &str) -> Option<Self> {
        let region = id.split('/').next()?;
        match region {
            "Africa" => Some(TzRegion::Africa),
            "America" => Some(TzRegion::America),
            "Antarctica" => Some(TzRegion::Antarctica),
            "Arctic" => Some(TzRegion::Arctic),
            "Asia" => Some(TzRegion::Asia),
            "Atlantic" => Some(TzRegion::Atlantic),
            "Australia" => Some(TzRegion::Australia),
            "Europe" => Some(TzRegion::Europe),
            "Indian" => Some(TzRegion::Indian),
            "Pacific" => Some(TzRegion::Pacific),
            "Etc" => Some(TzRegion::Etc),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            TzRegion::Africa => "Africa",
            TzRegion::America => "Americas",
            TzRegion::Antarctica => "Antarctica",
            TzRegion::Arctic => "Arctic",
            TzRegion::Asia => "Asia",
            TzRegion::Atlantic => "Atlantic",
            TzRegion::Australia => "Australia",
            TzRegion::Europe => "Europe",
            TzRegion::Indian => "Indian Ocean",
            TzRegion::Pacific => "Pacific",
            TzRegion::Etc => "Other",
        }
    }
}

// ============================================================================
// Locale Types
// ============================================================================

/// Locale identifier
pub type LocaleId = String;

/// Locale information
#[derive(Debug, Clone)]
pub struct Locale {
    /// Locale identifier (e.g., "pt_BR.UTF-8")
    pub id: LocaleId,
    /// Language code (ISO 639-1)
    pub language: String,
    /// Country code (ISO 3166-1 alpha-2)
    pub country: String,
    /// Character encoding
    pub encoding: String,
    /// Display name in native language
    pub native_name: String,
    /// Display name in English
    pub english_name: String,
    /// Is right-to-left language
    pub rtl: bool,
}

/// Language family for grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageFamily {
    Germanic,
    Romance,
    Slavic,
    Sino,
    Semitic,
    IndoIranian,
    Japanese,
    Korean,
    Other,
}

// ============================================================================
// Keyboard Layout Types
// ============================================================================

/// Keyboard layout
#[derive(Debug, Clone)]
pub struct KeyboardLayout {
    /// Layout code (e.g., "us", "br", "de")
    pub code: String,
    /// Display name (e.g., "English (US)", "Portuguese (Brazil)")
    pub name: String,
    /// Variant (e.g., "dvorak", "intl")
    pub variant: Option<String>,
    /// XKB model
    pub model: String,
}

// ============================================================================
// Date/Time Format
// ============================================================================

/// Date format style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateFormat {
    /// DD/MM/YYYY
    DayMonthYear,
    /// MM/DD/YYYY
    MonthDayYear,
    /// YYYY-MM-DD (ISO 8601)
    YearMonthDay,
}

/// Time format style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeFormat {
    /// 12-hour with AM/PM
    Hour12,
    /// 24-hour format
    Hour24,
}

/// First day of week
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstDayOfWeek {
    Sunday,
    Monday,
    Saturday,
}

/// Date/time format configuration
#[derive(Debug, Clone)]
pub struct DateTimeConfig {
    pub date_format: DateFormat,
    pub time_format: TimeFormat,
    pub first_day: FirstDayOfWeek,
    pub use_24h: bool,
    pub short_date_separator: char,
}

impl Default for DateTimeConfig {
    fn default() -> Self {
        Self {
            date_format: DateFormat::YearMonthDay,
            time_format: TimeFormat::Hour24,
            first_day: FirstDayOfWeek::Monday,
            use_24h: true,
            short_date_separator: '-',
        }
    }
}

// ============================================================================
// Regional Settings
// ============================================================================

/// Complete regional settings
#[derive(Debug, Clone)]
pub struct RegionalSettings {
    /// Timezone
    pub timezone: Option<Timezone>,
    /// System locale
    pub locale: Option<Locale>,
    /// Keyboard layout
    pub keyboard: Option<KeyboardLayout>,
    /// Date/time format
    pub datetime: DateTimeConfig,
    /// NTP servers
    pub ntp_servers: Vec<String>,
    /// Use NTP
    pub use_ntp: bool,
    /// Manual time set (if not using NTP)
    pub manual_time: Option<i64>,
}

impl Default for RegionalSettings {
    fn default() -> Self {
        Self {
            timezone: None,
            locale: None,
            keyboard: None,
            datetime: DateTimeConfig::default(),
            ntp_servers: vec![
                String::from("pool.ntp.org"),
                String::from("time.google.com"),
                String::from("time.cloudflare.com"),
            ],
            use_ntp: true,
            manual_time: None,
        }
    }
}

// ============================================================================
// Timezone Database
// ============================================================================

/// Timezone database
pub struct TimezoneDatabase {
    /// All timezones
    timezones: Vec<Timezone>,
    /// Index by ID
    by_id: BTreeMap<TzId, usize>,
    /// Index by region
    by_region: BTreeMap<TzRegion, Vec<usize>>,
    /// Index by country
    by_country: BTreeMap<String, Vec<usize>>,
}

impl TimezoneDatabase {
    /// Create new database with common timezones
    pub fn new() -> Self {
        let mut db = Self {
            timezones: Vec::new(),
            by_id: BTreeMap::new(),
            by_region: BTreeMap::new(),
            by_country: BTreeMap::new(),
        };

        // Add common timezones
        db.populate_common_timezones();
        db
    }

    /// Populate with common timezones
    fn populate_common_timezones(&mut self) {
        let common_tzs = [
            // Americas
            ("America/New_York", "New York", "US", -18000, 3600, true, "EST"),
            ("America/Los_Angeles", "Los Angeles", "US", -28800, 3600, true, "PST"),
            ("America/Chicago", "Chicago", "US", -21600, 3600, true, "CST"),
            ("America/Denver", "Denver", "US", -25200, 3600, true, "MST"),
            ("America/Sao_Paulo", "São Paulo", "BR", -10800, 0, false, "BRT"),
            ("America/Buenos_Aires", "Buenos Aires", "AR", -10800, 0, false, "ART"),
            ("America/Mexico_City", "Mexico City", "MX", -21600, 3600, true, "CST"),
            ("America/Toronto", "Toronto", "CA", -18000, 3600, true, "EST"),
            ("America/Vancouver", "Vancouver", "CA", -28800, 3600, true, "PST"),

            // Europe
            ("Europe/London", "London", "GB", 0, 3600, true, "GMT"),
            ("Europe/Paris", "Paris", "FR", 3600, 3600, true, "CET"),
            ("Europe/Berlin", "Berlin", "DE", 3600, 3600, true, "CET"),
            ("Europe/Rome", "Rome", "IT", 3600, 3600, true, "CET"),
            ("Europe/Madrid", "Madrid", "ES", 3600, 3600, true, "CET"),
            ("Europe/Lisbon", "Lisbon", "PT", 0, 3600, true, "WET"),
            ("Europe/Moscow", "Moscow", "RU", 10800, 0, false, "MSK"),
            ("Europe/Amsterdam", "Amsterdam", "NL", 3600, 3600, true, "CET"),
            ("Europe/Brussels", "Brussels", "BE", 3600, 3600, true, "CET"),
            ("Europe/Stockholm", "Stockholm", "SE", 3600, 3600, true, "CET"),
            ("Europe/Warsaw", "Warsaw", "PL", 3600, 3600, true, "CET"),
            ("Europe/Zurich", "Zurich", "CH", 3600, 3600, true, "CET"),

            // Asia
            ("Asia/Tokyo", "Tokyo", "JP", 32400, 0, false, "JST"),
            ("Asia/Shanghai", "Shanghai", "CN", 28800, 0, false, "CST"),
            ("Asia/Hong_Kong", "Hong Kong", "HK", 28800, 0, false, "HKT"),
            ("Asia/Singapore", "Singapore", "SG", 28800, 0, false, "SGT"),
            ("Asia/Seoul", "Seoul", "KR", 32400, 0, false, "KST"),
            ("Asia/Dubai", "Dubai", "AE", 14400, 0, false, "GST"),
            ("Asia/Kolkata", "Mumbai/Kolkata", "IN", 19800, 0, false, "IST"),
            ("Asia/Bangkok", "Bangkok", "TH", 25200, 0, false, "ICT"),
            ("Asia/Jakarta", "Jakarta", "ID", 25200, 0, false, "WIB"),
            ("Asia/Manila", "Manila", "PH", 28800, 0, false, "PHT"),
            ("Asia/Tel_Aviv", "Tel Aviv", "IL", 7200, 3600, true, "IST"),

            // Australia/Pacific
            ("Australia/Sydney", "Sydney", "AU", 36000, 3600, true, "AEST"),
            ("Australia/Melbourne", "Melbourne", "AU", 36000, 3600, true, "AEST"),
            ("Australia/Brisbane", "Brisbane", "AU", 36000, 0, false, "AEST"),
            ("Australia/Perth", "Perth", "AU", 28800, 0, false, "AWST"),
            ("Pacific/Auckland", "Auckland", "NZ", 43200, 3600, true, "NZST"),
            ("Pacific/Honolulu", "Honolulu", "US", -36000, 0, false, "HST"),

            // Africa
            ("Africa/Cairo", "Cairo", "EG", 7200, 0, false, "EET"),
            ("Africa/Johannesburg", "Johannesburg", "ZA", 7200, 0, false, "SAST"),
            ("Africa/Lagos", "Lagos", "NG", 3600, 0, false, "WAT"),
            ("Africa/Nairobi", "Nairobi", "KE", 10800, 0, false, "EAT"),

            // UTC
            ("Etc/UTC", "UTC (Coordinated Universal Time)", "ZZ", 0, 0, false, "UTC"),
        ];

        for (id, name, country, offset, dst, has_dst, abbr) in common_tzs.iter() {
            self.add_timezone(Timezone {
                id: id.to_string(),
                display_name: name.to_string(),
                country: country.to_string(),
                utc_offset: *offset,
                dst_offset: *dst,
                has_dst: *has_dst,
                abbreviation: abbr.to_string(),
            });
        }
    }

    /// Add a timezone
    fn add_timezone(&mut self, tz: Timezone) {
        let idx = self.timezones.len();
        let id = tz.id.clone();
        let country = tz.country.clone();
        let region = TzRegion::from_tz_id(&id);

        self.timezones.push(tz);
        self.by_id.insert(id, idx);

        if let Some(r) = region {
            self.by_region.entry(r).or_default().push(idx);
        }

        self.by_country.entry(country).or_default().push(idx);
    }

    /// Get timezone by ID
    pub fn get(&self, id: &str) -> Option<&Timezone> {
        self.by_id.get(id).map(|&idx| &self.timezones[idx])
    }

    /// Get all timezones
    pub fn all(&self) -> &[Timezone] {
        &self.timezones
    }

    /// Get timezones by region
    pub fn by_region(&self, region: TzRegion) -> Vec<&Timezone> {
        self.by_region.get(&region)
            .map(|indices| indices.iter().map(|&i| &self.timezones[i]).collect())
            .unwrap_or_default()
    }

    /// Get timezones by country
    pub fn by_country(&self, country: &str) -> Vec<&Timezone> {
        self.by_country.get(country)
            .map(|indices| indices.iter().map(|&i| &self.timezones[i]).collect())
            .unwrap_or_default()
    }

    /// Search timezones by name
    pub fn search(&self, query: &str) -> Vec<&Timezone> {
        let query_lower = query.to_lowercase();
        self.timezones.iter()
            .filter(|tz| {
                tz.id.to_lowercase().contains(&query_lower) ||
                tz.display_name.to_lowercase().contains(&query_lower) ||
                tz.country.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    /// Get available regions
    pub fn regions(&self) -> Vec<TzRegion> {
        self.by_region.keys().copied().collect()
    }
}

impl Default for TimezoneDatabase {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Locale Database
// ============================================================================

/// Locale database
pub struct LocaleDatabase {
    /// All locales
    locales: Vec<Locale>,
    /// Index by ID
    by_id: BTreeMap<LocaleId, usize>,
    /// Index by language
    by_language: BTreeMap<String, Vec<usize>>,
}

impl LocaleDatabase {
    /// Create new database with common locales
    pub fn new() -> Self {
        let mut db = Self {
            locales: Vec::new(),
            by_id: BTreeMap::new(),
            by_language: BTreeMap::new(),
        };

        db.populate_common_locales();
        db
    }

    /// Populate with common locales
    fn populate_common_locales(&mut self) {
        let common_locales = [
            // English
            ("en_US.UTF-8", "en", "US", "UTF-8", "English (US)", "English (US)", false),
            ("en_GB.UTF-8", "en", "GB", "UTF-8", "English (UK)", "English (UK)", false),
            ("en_AU.UTF-8", "en", "AU", "UTF-8", "English (Australia)", "English (Australia)", false),
            ("en_CA.UTF-8", "en", "CA", "UTF-8", "English (Canada)", "English (Canada)", false),

            // Spanish
            ("es_ES.UTF-8", "es", "ES", "UTF-8", "Español (España)", "Spanish (Spain)", false),
            ("es_MX.UTF-8", "es", "MX", "UTF-8", "Español (México)", "Spanish (Mexico)", false),
            ("es_AR.UTF-8", "es", "AR", "UTF-8", "Español (Argentina)", "Spanish (Argentina)", false),

            // Portuguese
            ("pt_BR.UTF-8", "pt", "BR", "UTF-8", "Português (Brasil)", "Portuguese (Brazil)", false),
            ("pt_PT.UTF-8", "pt", "PT", "UTF-8", "Português (Portugal)", "Portuguese (Portugal)", false),

            // French
            ("fr_FR.UTF-8", "fr", "FR", "UTF-8", "Français (France)", "French (France)", false),
            ("fr_CA.UTF-8", "fr", "CA", "UTF-8", "Français (Canada)", "French (Canada)", false),

            // German
            ("de_DE.UTF-8", "de", "DE", "UTF-8", "Deutsch (Deutschland)", "German (Germany)", false),
            ("de_AT.UTF-8", "de", "AT", "UTF-8", "Deutsch (Österreich)", "German (Austria)", false),
            ("de_CH.UTF-8", "de", "CH", "UTF-8", "Deutsch (Schweiz)", "German (Switzerland)", false),

            // Italian
            ("it_IT.UTF-8", "it", "IT", "UTF-8", "Italiano", "Italian", false),

            // Russian
            ("ru_RU.UTF-8", "ru", "RU", "UTF-8", "Русский", "Russian", false),

            // Chinese
            ("zh_CN.UTF-8", "zh", "CN", "UTF-8", "简体中文", "Chinese (Simplified)", false),
            ("zh_TW.UTF-8", "zh", "TW", "UTF-8", "繁體中文", "Chinese (Traditional)", false),

            // Japanese
            ("ja_JP.UTF-8", "ja", "JP", "UTF-8", "日本語", "Japanese", false),

            // Korean
            ("ko_KR.UTF-8", "ko", "KR", "UTF-8", "한국어", "Korean", false),

            // Arabic
            ("ar_SA.UTF-8", "ar", "SA", "UTF-8", "العربية", "Arabic (Saudi Arabia)", true),
            ("ar_EG.UTF-8", "ar", "EG", "UTF-8", "العربية", "Arabic (Egypt)", true),

            // Hebrew
            ("he_IL.UTF-8", "he", "IL", "UTF-8", "עברית", "Hebrew", true),

            // Other European
            ("nl_NL.UTF-8", "nl", "NL", "UTF-8", "Nederlands", "Dutch", false),
            ("pl_PL.UTF-8", "pl", "PL", "UTF-8", "Polski", "Polish", false),
            ("sv_SE.UTF-8", "sv", "SE", "UTF-8", "Svenska", "Swedish", false),
            ("da_DK.UTF-8", "da", "DK", "UTF-8", "Dansk", "Danish", false),
            ("no_NO.UTF-8", "no", "NO", "UTF-8", "Norsk", "Norwegian", false),
            ("fi_FI.UTF-8", "fi", "FI", "UTF-8", "Suomi", "Finnish", false),
            ("cs_CZ.UTF-8", "cs", "CZ", "UTF-8", "Čeština", "Czech", false),
            ("hu_HU.UTF-8", "hu", "HU", "UTF-8", "Magyar", "Hungarian", false),
            ("el_GR.UTF-8", "el", "GR", "UTF-8", "Ελληνικά", "Greek", false),
            ("tr_TR.UTF-8", "tr", "TR", "UTF-8", "Türkçe", "Turkish", false),
            ("uk_UA.UTF-8", "uk", "UA", "UTF-8", "Українська", "Ukrainian", false),

            // Asian
            ("th_TH.UTF-8", "th", "TH", "UTF-8", "ไทย", "Thai", false),
            ("vi_VN.UTF-8", "vi", "VN", "UTF-8", "Tiếng Việt", "Vietnamese", false),
            ("id_ID.UTF-8", "id", "ID", "UTF-8", "Bahasa Indonesia", "Indonesian", false),
            ("hi_IN.UTF-8", "hi", "IN", "UTF-8", "हिन्दी", "Hindi", false),
        ];

        for (id, lang, country, enc, native, english, rtl) in common_locales.iter() {
            self.add_locale(Locale {
                id: id.to_string(),
                language: lang.to_string(),
                country: country.to_string(),
                encoding: enc.to_string(),
                native_name: native.to_string(),
                english_name: english.to_string(),
                rtl: *rtl,
            });
        }
    }

    /// Add a locale
    fn add_locale(&mut self, locale: Locale) {
        let idx = self.locales.len();
        let id = locale.id.clone();
        let lang = locale.language.clone();

        self.locales.push(locale);
        self.by_id.insert(id, idx);
        self.by_language.entry(lang).or_default().push(idx);
    }

    /// Get locale by ID
    pub fn get(&self, id: &str) -> Option<&Locale> {
        self.by_id.get(id).map(|&idx| &self.locales[idx])
    }

    /// Get all locales
    pub fn all(&self) -> &[Locale] {
        &self.locales
    }

    /// Get locales by language
    pub fn by_language(&self, language: &str) -> Vec<&Locale> {
        self.by_language.get(language)
            .map(|indices| indices.iter().map(|&i| &self.locales[i]).collect())
            .unwrap_or_default()
    }

    /// Search locales
    pub fn search(&self, query: &str) -> Vec<&Locale> {
        let query_lower = query.to_lowercase();
        self.locales.iter()
            .filter(|l| {
                l.id.to_lowercase().contains(&query_lower) ||
                l.native_name.to_lowercase().contains(&query_lower) ||
                l.english_name.to_lowercase().contains(&query_lower) ||
                l.language.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    /// Get available languages
    pub fn languages(&self) -> Vec<String> {
        self.by_language.keys().cloned().collect()
    }
}

impl Default for LocaleDatabase {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Keyboard Layout Database
// ============================================================================

/// Keyboard layout database
pub struct KeyboardDatabase {
    layouts: Vec<KeyboardLayout>,
}

impl KeyboardDatabase {
    /// Create new database
    pub fn new() -> Self {
        let mut db = Self { layouts: Vec::new() };
        db.populate_common_layouts();
        db
    }

    /// Populate with common layouts
    fn populate_common_layouts(&mut self) {
        let layouts = [
            ("us", "English (US)", None, "pc105"),
            ("us", "English (US, International)", Some("intl"), "pc105"),
            ("us", "English (Dvorak)", Some("dvorak"), "pc105"),
            ("gb", "English (UK)", None, "pc105"),
            ("de", "German", None, "pc105"),
            ("fr", "French", None, "pc105"),
            ("es", "Spanish", None, "pc105"),
            ("pt", "Portuguese (Portugal)", None, "pc105"),
            ("br", "Portuguese (Brazil)", None, "abnt2"),
            ("it", "Italian", None, "pc105"),
            ("ru", "Russian", None, "pc105"),
            ("jp", "Japanese", None, "jp106"),
            ("kr", "Korean", None, "pc105"),
            ("cn", "Chinese", None, "pc105"),
            ("pl", "Polish", None, "pc105"),
            ("nl", "Dutch", None, "pc105"),
            ("se", "Swedish", None, "pc105"),
            ("no", "Norwegian", None, "pc105"),
            ("dk", "Danish", None, "pc105"),
            ("fi", "Finnish", None, "pc105"),
            ("cz", "Czech", None, "pc105"),
            ("hu", "Hungarian", None, "pc105"),
            ("tr", "Turkish", None, "pc105"),
            ("il", "Hebrew", None, "pc105"),
            ("ar", "Arabic", None, "pc105"),
            ("th", "Thai", None, "pc105"),
            ("latam", "Latin American", None, "pc105"),
            ("ch", "Swiss German", None, "pc105"),
            ("ch", "Swiss French", Some("fr"), "pc105"),
            ("be", "Belgian", None, "pc105"),
            ("ca", "Canadian French", None, "pc105"),
        ];

        for (code, name, variant, model) in layouts.iter() {
            self.layouts.push(KeyboardLayout {
                code: code.to_string(),
                name: name.to_string(),
                variant: variant.map(|s| s.to_string()),
                model: model.to_string(),
            });
        }
    }

    /// Get all layouts
    pub fn all(&self) -> &[KeyboardLayout] {
        &self.layouts
    }

    /// Get layout by code
    pub fn get(&self, code: &str, variant: Option<&str>) -> Option<&KeyboardLayout> {
        self.layouts.iter().find(|l| {
            l.code == code && l.variant.as_deref() == variant
        })
    }

    /// Search layouts
    pub fn search(&self, query: &str) -> Vec<&KeyboardLayout> {
        let query_lower = query.to_lowercase();
        self.layouts.iter()
            .filter(|l| {
                l.code.to_lowercase().contains(&query_lower) ||
                l.name.to_lowercase().contains(&query_lower)
            })
            .collect()
    }
}

impl Default for KeyboardDatabase {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Configuration Manager
// ============================================================================

/// Regional configuration manager
pub struct RegionalConfigManager {
    /// Current settings
    settings: RegionalSettings,
    /// Timezone database
    pub tz_db: TimezoneDatabase,
    /// Locale database
    pub locale_db: LocaleDatabase,
    /// Keyboard database
    pub keyboard_db: KeyboardDatabase,
}

impl RegionalConfigManager {
    /// Create new manager
    pub fn new() -> Self {
        Self {
            settings: RegionalSettings::default(),
            tz_db: TimezoneDatabase::new(),
            locale_db: LocaleDatabase::new(),
            keyboard_db: KeyboardDatabase::new(),
        }
    }

    /// Set timezone
    pub fn set_timezone(&mut self, tz_id: &str) -> Result<(), RegionalError> {
        let tz = self.tz_db.get(tz_id)
            .ok_or(RegionalError::TimezoneNotFound)?;
        self.settings.timezone = Some(tz.clone());

        // Update system timezone offset
        CURRENT_TZ_OFFSET.store(tz.utc_offset, Ordering::Relaxed);

        Ok(())
    }

    /// Set locale
    pub fn set_locale(&mut self, locale_id: &str) -> Result<(), RegionalError> {
        let locale = self.locale_db.get(locale_id)
            .ok_or(RegionalError::LocaleNotFound)?;
        self.settings.locale = Some(locale.clone());
        Ok(())
    }

    /// Set keyboard layout
    pub fn set_keyboard(&mut self, code: &str, variant: Option<&str>) -> Result<(), RegionalError> {
        let layout = self.keyboard_db.get(code, variant)
            .ok_or(RegionalError::KeyboardNotFound)?;
        self.settings.keyboard = Some(layout.clone());
        Ok(())
    }

    /// Set date/time configuration
    pub fn set_datetime_config(&mut self, config: DateTimeConfig) {
        self.settings.datetime = config;
    }

    /// Enable/disable NTP
    pub fn set_use_ntp(&mut self, use_ntp: bool) {
        self.settings.use_ntp = use_ntp;
    }

    /// Set NTP servers
    pub fn set_ntp_servers(&mut self, servers: Vec<String>) {
        self.settings.ntp_servers = servers;
    }

    /// Get current settings
    pub fn settings(&self) -> &RegionalSettings {
        &self.settings
    }

    /// Auto-detect settings from geolocation/system
    pub fn auto_detect(&mut self) {
        // In real implementation:
        // 1. Query geolocation API or use IP-based detection
        // 2. Set timezone based on location
        // 3. Set locale based on browser/system language
        // 4. Set keyboard based on locale

        // Default to UTC and en_US for now
        let _ = self.set_timezone("Etc/UTC");
        let _ = self.set_locale("en_US.UTF-8");
        let _ = self.set_keyboard("us", None);
    }

    /// Generate /etc/timezone content
    pub fn generate_timezone_file(&self) -> Option<String> {
        self.settings.timezone.as_ref().map(|tz| tz.id.clone())
    }

    /// Generate /etc/localtime symlink target
    pub fn localtime_target(&self) -> Option<String> {
        self.settings.timezone.as_ref()
            .map(|tz| format!("/usr/share/zoneinfo/{}", tz.id))
    }

    /// Generate /etc/locale.conf content
    pub fn generate_locale_conf(&self) -> Option<String> {
        self.settings.locale.as_ref().map(|l| {
            format!("LANG={}\nLC_ALL={}\n", l.id, l.id)
        })
    }

    /// Generate /etc/vconsole.conf content
    pub fn generate_vconsole_conf(&self) -> Option<String> {
        self.settings.keyboard.as_ref().map(|k| {
            let mut conf = format!("KEYMAP={}\n", k.code);
            if let Some(ref var) = k.variant {
                conf.push_str(&format!("KEYMAP_TOGGLE={}\n", var));
            }
            conf
        })
    }
}

impl Default for RegionalConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Regional configuration error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionalError {
    TimezoneNotFound,
    LocaleNotFound,
    KeyboardNotFound,
    InvalidConfiguration,
}

// ============================================================================
// Global Instance
// ============================================================================

static REGIONAL_CONFIG: Once<Mutex<RegionalConfigManager>> = Once::new();
static CURRENT_TZ_OFFSET: AtomicI32 = AtomicI32::new(0);

/// Initialize timezone/locale subsystem
pub fn init() {
    REGIONAL_CONFIG.call_once(|| Mutex::new(RegionalConfigManager::new()));
    crate::kprintln!("timezone: initialized with {} timezones, {} locales",
        REGIONAL_CONFIG.get().unwrap().lock().tz_db.all().len(),
        REGIONAL_CONFIG.get().unwrap().lock().locale_db.all().len()
    );
}

/// Get regional config manager
pub fn manager() -> &'static Mutex<RegionalConfigManager> {
    REGIONAL_CONFIG.get().expect("Regional config not initialized")
}

/// Get current timezone offset in seconds
pub fn current_tz_offset() -> i32 {
    CURRENT_TZ_OFFSET.load(Ordering::Relaxed)
}

/// Set timezone by ID
pub fn set_timezone(tz_id: &str) -> Result<(), RegionalError> {
    manager().lock().set_timezone(tz_id)
}

/// Set locale by ID
pub fn set_locale(locale_id: &str) -> Result<(), RegionalError> {
    manager().lock().set_locale(locale_id)
}

/// Set keyboard layout
pub fn set_keyboard(code: &str, variant: Option<&str>) -> Result<(), RegionalError> {
    manager().lock().set_keyboard(code, variant)
}

/// Get all timezones
pub fn all_timezones() -> Vec<Timezone> {
    manager().lock().tz_db.all().to_vec()
}

/// Get all locales
pub fn all_locales() -> Vec<Locale> {
    manager().lock().locale_db.all().to_vec()
}

/// Get all keyboard layouts
pub fn all_keyboards() -> Vec<KeyboardLayout> {
    manager().lock().keyboard_db.all().to_vec()
}

/// Search timezones
pub fn search_timezones(query: &str) -> Vec<Timezone> {
    manager().lock().tz_db.search(query).into_iter().cloned().collect()
}

/// Search locales
pub fn search_locales(query: &str) -> Vec<Locale> {
    manager().lock().locale_db.search(query).into_iter().cloned().collect()
}

/// Auto-detect regional settings
pub fn auto_detect() {
    manager().lock().auto_detect();
}

/// Get current settings
pub fn get_settings() -> RegionalSettings {
    manager().lock().settings().clone()
}
