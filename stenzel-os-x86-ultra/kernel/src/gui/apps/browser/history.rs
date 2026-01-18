//! Browser History
//!
//! Browsing history management with search, filtering, and privacy controls.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;

/// Unique history entry identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HistoryEntryId(u64);

impl HistoryEntryId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Visit type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisitType {
    /// Direct URL entry or link click
    Link,
    /// Typed in address bar
    Typed,
    /// Bookmark click
    Bookmark,
    /// From history or autocomplete
    AutoComplete,
    /// Embedded resource (iframe, frame)
    Embed,
    /// Redirect
    Redirect,
    /// Download link
    Download,
    /// Form submission
    FormSubmit,
    /// Page reload
    Reload,
}

impl VisitType {
    pub fn name(&self) -> &'static str {
        match self {
            VisitType::Link => "Link",
            VisitType::Typed => "Typed",
            VisitType::Bookmark => "Bookmark",
            VisitType::AutoComplete => "Auto-complete",
            VisitType::Embed => "Embedded",
            VisitType::Redirect => "Redirect",
            VisitType::Download => "Download",
            VisitType::FormSubmit => "Form Submit",
            VisitType::Reload => "Reload",
        }
    }

    pub fn is_user_initiated(&self) -> bool {
        matches!(self,
            VisitType::Link |
            VisitType::Typed |
            VisitType::Bookmark |
            VisitType::AutoComplete |
            VisitType::FormSubmit
        )
    }
}

/// Single visit to a page
#[derive(Debug, Clone)]
pub struct Visit {
    pub timestamp: u64,
    pub visit_type: VisitType,
    pub referrer_url: Option<String>,
    pub transition_type: TransitionType,
    pub duration_ms: Option<u64>,
}

/// Page transition type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionType {
    /// Normal navigation
    Normal,
    /// New tab/window
    NewTab,
    /// Forward/back button
    ForwardBack,
    /// Address bar entry
    AddressBar,
    /// From another site
    External,
    /// From same site
    Internal,
}

/// History entry for a URL
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub id: HistoryEntryId,
    pub url: String,
    pub title: String,
    pub favicon_url: Option<String>,
    pub first_visit: u64,
    pub last_visit: u64,
    pub visit_count: u32,
    pub typed_count: u32,
    pub visits: Vec<Visit>,
    pub is_hidden: bool,
}

impl HistoryEntry {
    pub fn new(id: HistoryEntryId, url: &str, title: &str) -> Self {
        Self {
            id,
            url: String::from(url),
            title: String::from(title),
            favicon_url: None,
            first_visit: 0,
            last_visit: 0,
            visit_count: 0,
            typed_count: 0,
            visits: Vec::new(),
            is_hidden: false,
        }
    }

    pub fn domain(&self) -> &str {
        if let Some(start) = self.url.find("://") {
            let after_proto = &self.url[start + 3..];
            if let Some(end) = after_proto.find('/') {
                return &after_proto[..end];
            }
            return after_proto;
        }
        &self.url
    }

    pub fn display_title(&self) -> &str {
        if self.title.is_empty() {
            self.domain()
        } else {
            &self.title
        }
    }

    pub fn record_visit(&mut self, timestamp: u64, visit_type: VisitType) {
        let visit = Visit {
            timestamp,
            visit_type,
            referrer_url: None,
            transition_type: TransitionType::Normal,
            duration_ms: None,
        };

        if self.first_visit == 0 || timestamp < self.first_visit {
            self.first_visit = timestamp;
        }
        if timestamp > self.last_visit {
            self.last_visit = timestamp;
        }

        self.visit_count += 1;
        if visit_type == VisitType::Typed {
            self.typed_count += 1;
        }

        self.visits.push(visit);

        // Keep only last 100 visits per entry
        if self.visits.len() > 100 {
            self.visits.remove(0);
        }
    }

    pub fn frecency_score(&self, current_time: u64) -> u32 {
        // Firefox-style frecency calculation
        let mut score = 0u32;

        // Base score from visit count
        score += self.visit_count * 10;

        // Bonus for typed visits
        score += self.typed_count * 20;

        // Recency bonus
        let age_hours = (current_time.saturating_sub(self.last_visit)) / 3600;
        if age_hours < 4 {
            score += 100;
        } else if age_hours < 24 {
            score += 70;
        } else if age_hours < 72 {
            score += 50;
        } else if age_hours < 168 {
            score += 30;
        } else if age_hours < 720 {
            score += 10;
        }

        score
    }
}

/// Time range filter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeRange {
    /// Last hour
    LastHour,
    /// Today
    Today,
    /// Yesterday
    Yesterday,
    /// Last 7 days
    LastWeek,
    /// Last 30 days
    LastMonth,
    /// Last 90 days
    Last3Months,
    /// Last 365 days
    LastYear,
    /// All time
    AllTime,
    /// Custom date range
    Custom,
}

impl TimeRange {
    pub fn name(&self) -> &'static str {
        match self {
            TimeRange::LastHour => "Last hour",
            TimeRange::Today => "Today",
            TimeRange::Yesterday => "Yesterday",
            TimeRange::LastWeek => "Last 7 days",
            TimeRange::LastMonth => "Last 30 days",
            TimeRange::Last3Months => "Last 3 months",
            TimeRange::LastYear => "Last year",
            TimeRange::AllTime => "All time",
            TimeRange::Custom => "Custom range",
        }
    }

    pub fn seconds(&self) -> u64 {
        match self {
            TimeRange::LastHour => 3600,
            TimeRange::Today => 86400,
            TimeRange::Yesterday => 86400,
            TimeRange::LastWeek => 7 * 86400,
            TimeRange::LastMonth => 30 * 86400,
            TimeRange::Last3Months => 90 * 86400,
            TimeRange::LastYear => 365 * 86400,
            TimeRange::AllTime | TimeRange::Custom => u64::MAX,
        }
    }
}

/// History search result
#[derive(Debug, Clone)]
pub struct HistorySearchResult {
    pub entry: HistoryEntry,
    pub match_score: u32,
    pub matched_in_title: bool,
    pub matched_in_url: bool,
}

/// History statistics
#[derive(Debug, Clone)]
pub struct HistoryStats {
    pub total_entries: usize,
    pub total_visits: u32,
    pub total_typed: u32,
    pub unique_domains: usize,
    pub oldest_visit: u64,
    pub newest_visit: u64,
    pub most_visited_domain: Option<String>,
    pub most_visited_count: u32,
}

/// History grouped by date
#[derive(Debug, Clone)]
pub struct HistoryByDate {
    pub date_label: String,
    pub timestamp_start: u64,
    pub timestamp_end: u64,
    pub entries: Vec<HistoryEntry>,
}

/// History grouped by domain
#[derive(Debug, Clone)]
pub struct HistoryByDomain {
    pub domain: String,
    pub visit_count: u32,
    pub entries: Vec<HistoryEntry>,
}

/// History sort order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistorySortOrder {
    /// Most recent first
    DateDesc,
    /// Oldest first
    DateAsc,
    /// Most visited first
    VisitCountDesc,
    /// By frecency (Firefox-style)
    Frecency,
    /// Title A-Z
    TitleAsc,
    /// Title Z-A
    TitleDesc,
}

impl HistorySortOrder {
    pub fn name(&self) -> &'static str {
        match self {
            HistorySortOrder::DateDesc => "Date (Newest first)",
            HistorySortOrder::DateAsc => "Date (Oldest first)",
            HistorySortOrder::VisitCountDesc => "Most Visited",
            HistorySortOrder::Frecency => "Frecency",
            HistorySortOrder::TitleAsc => "Title A-Z",
            HistorySortOrder::TitleDesc => "Title Z-A",
        }
    }
}

/// History error types
#[derive(Debug, Clone)]
pub enum HistoryError {
    NotFound,
    DatabaseError(String),
    ImportFailed(String),
    ExportFailed(String),
}

pub type HistoryResult<T> = Result<T, HistoryError>;

/// History manager
pub struct HistoryManager {
    entries: BTreeMap<HistoryEntryId, HistoryEntry>,
    url_to_id: BTreeMap<String, HistoryEntryId>,

    next_id: u64,
    max_entries: usize,
    max_age_days: u32,

    // Privacy settings
    enable_history: bool,
    remember_search_history: bool,
    remember_form_data: bool,
    clear_on_exit: bool,

    // Current filter state
    current_time: u64,
}

impl HistoryManager {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            url_to_id: BTreeMap::new(),
            next_id: 1,
            max_entries: 100000,
            max_age_days: 90,
            enable_history: true,
            remember_search_history: true,
            remember_form_data: true,
            clear_on_exit: false,
            current_time: 0,
        }
    }

    /// Record a page visit
    pub fn record_visit(&mut self, url: &str, title: &str, visit_type: VisitType) -> HistoryEntryId {
        if !self.enable_history {
            return HistoryEntryId::new(0);
        }

        // Check for existing entry
        if let Some(&id) = self.url_to_id.get(url) {
            if let Some(entry) = self.entries.get_mut(&id) {
                entry.record_visit(self.current_time, visit_type);
                if !title.is_empty() && entry.title.is_empty() {
                    entry.title = String::from(title);
                }
                return id;
            }
        }

        // Create new entry
        let id = HistoryEntryId::new(self.next_id);
        self.next_id += 1;

        let mut entry = HistoryEntry::new(id, url, title);
        entry.record_visit(self.current_time, visit_type);

        self.url_to_id.insert(String::from(url), id);
        self.entries.insert(id, entry);

        // Enforce max entries limit
        self.enforce_limits();

        id
    }

    /// Update page title
    pub fn update_title(&mut self, url: &str, title: &str) {
        if let Some(&id) = self.url_to_id.get(url) {
            if let Some(entry) = self.entries.get_mut(&id) {
                entry.title = String::from(title);
            }
        }
    }

    /// Get entry by URL
    pub fn get_by_url(&self, url: &str) -> Option<&HistoryEntry> {
        self.url_to_id.get(url)
            .and_then(|id| self.entries.get(id))
    }

    /// Get entry by ID
    pub fn get_by_id(&self, id: HistoryEntryId) -> Option<&HistoryEntry> {
        self.entries.get(&id)
    }

    /// Delete a single entry
    pub fn delete_entry(&mut self, id: HistoryEntryId) -> HistoryResult<()> {
        if let Some(entry) = self.entries.remove(&id) {
            self.url_to_id.remove(&entry.url);
            Ok(())
        } else {
            Err(HistoryError::NotFound)
        }
    }

    /// Delete by URL
    pub fn delete_by_url(&mut self, url: &str) -> HistoryResult<()> {
        if let Some(id) = self.url_to_id.remove(url) {
            self.entries.remove(&id);
            Ok(())
        } else {
            Err(HistoryError::NotFound)
        }
    }

    /// Delete entries in time range
    pub fn delete_range(&mut self, time_range: TimeRange) {
        let cutoff = self.current_time.saturating_sub(time_range.seconds());

        let ids_to_remove: Vec<_> = self.entries.iter()
            .filter(|(_, entry)| entry.last_visit >= cutoff)
            .map(|(id, _)| *id)
            .collect();

        for id in ids_to_remove {
            if let Some(entry) = self.entries.remove(&id) {
                self.url_to_id.remove(&entry.url);
            }
        }
    }

    /// Delete entries for a domain
    pub fn delete_domain(&mut self, domain: &str) {
        let ids_to_remove: Vec<_> = self.entries.iter()
            .filter(|(_, entry)| entry.domain() == domain)
            .map(|(id, _)| *id)
            .collect();

        for id in ids_to_remove {
            if let Some(entry) = self.entries.remove(&id) {
                self.url_to_id.remove(&entry.url);
            }
        }
    }

    /// Clear all history
    pub fn clear_all(&mut self) {
        self.entries.clear();
        self.url_to_id.clear();
    }

    /// Search history
    pub fn search(&self, query: &str) -> Vec<HistorySearchResult> {
        let query_lower = query.to_ascii_lowercase();
        let mut results = Vec::new();

        for entry in self.entries.values() {
            if entry.is_hidden {
                continue;
            }

            let mut score = 0u32;
            let mut matched_in_title = false;
            let mut matched_in_url = false;

            // Check title
            let title_lower = entry.title.to_ascii_lowercase();
            if title_lower.contains(&query_lower) {
                score += 100;
                matched_in_title = true;
                if title_lower.starts_with(&query_lower) {
                    score += 50;
                }
            }

            // Check URL
            let url_lower = entry.url.to_ascii_lowercase();
            if url_lower.contains(&query_lower) {
                score += 50;
                matched_in_url = true;
            }

            // Add frecency bonus
            score += entry.frecency_score(self.current_time) / 10;

            if score > 0 {
                results.push(HistorySearchResult {
                    entry: entry.clone(),
                    match_score: score,
                    matched_in_title,
                    matched_in_url,
                });
            }
        }

        // Sort by score
        results.sort_by(|a, b| b.match_score.cmp(&a.match_score));
        results
    }

    /// Get recent history
    pub fn get_recent(&self, limit: usize) -> Vec<&HistoryEntry> {
        let mut entries: Vec<_> = self.entries.values()
            .filter(|e| !e.is_hidden)
            .collect();

        entries.sort_by(|a, b| b.last_visit.cmp(&a.last_visit));
        entries.truncate(limit);
        entries
    }

    /// Get most visited
    pub fn get_most_visited(&self, limit: usize) -> Vec<&HistoryEntry> {
        let mut entries: Vec<_> = self.entries.values()
            .filter(|e| !e.is_hidden)
            .collect();

        entries.sort_by(|a, b| b.visit_count.cmp(&a.visit_count));
        entries.truncate(limit);
        entries
    }

    /// Get by frecency (Firefox-style)
    pub fn get_by_frecency(&self, limit: usize) -> Vec<&HistoryEntry> {
        let current_time = self.current_time;
        let mut entries: Vec<_> = self.entries.values()
            .filter(|e| !e.is_hidden)
            .collect();

        entries.sort_by(|a, b|
            b.frecency_score(current_time).cmp(&a.frecency_score(current_time))
        );
        entries.truncate(limit);
        entries
    }

    /// Get history for time range
    pub fn get_for_range(&self, time_range: TimeRange) -> Vec<&HistoryEntry> {
        let cutoff = self.current_time.saturating_sub(time_range.seconds());

        let mut entries: Vec<_> = self.entries.values()
            .filter(|e| !e.is_hidden && e.last_visit >= cutoff)
            .collect();

        entries.sort_by(|a, b| b.last_visit.cmp(&a.last_visit));
        entries
    }

    /// Group history by date
    pub fn group_by_date(&self, time_range: TimeRange) -> Vec<HistoryByDate> {
        let entries = self.get_for_range(time_range);
        let mut groups: BTreeMap<u64, Vec<HistoryEntry>> = BTreeMap::new();

        for entry in entries {
            // Group by day (truncate to start of day)
            let day_start = (entry.last_visit / 86400) * 86400;
            groups.entry(day_start)
                .or_insert_with(Vec::new)
                .push(entry.clone());
        }

        let mut result: Vec<_> = groups.into_iter()
            .map(|(day_start, mut entries)| {
                entries.sort_by(|a, b| b.last_visit.cmp(&a.last_visit));

                let date_label = self.format_date_label(day_start);

                HistoryByDate {
                    date_label,
                    timestamp_start: day_start,
                    timestamp_end: day_start + 86400 - 1,
                    entries,
                }
            })
            .collect();

        result.sort_by(|a, b| b.timestamp_start.cmp(&a.timestamp_start));
        result
    }

    /// Group history by domain
    pub fn group_by_domain(&self) -> Vec<HistoryByDomain> {
        let mut domains: BTreeMap<String, Vec<HistoryEntry>> = BTreeMap::new();

        for entry in self.entries.values() {
            if entry.is_hidden {
                continue;
            }
            let domain = String::from(entry.domain());
            domains.entry(domain)
                .or_insert_with(Vec::new)
                .push(entry.clone());
        }

        let mut result: Vec<_> = domains.into_iter()
            .map(|(domain, entries)| {
                let visit_count: u32 = entries.iter().map(|e| e.visit_count).sum();
                HistoryByDomain {
                    domain,
                    visit_count,
                    entries,
                }
            })
            .collect();

        result.sort_by(|a, b| b.visit_count.cmp(&a.visit_count));
        result
    }

    fn format_date_label(&self, timestamp: u64) -> String {
        let age_days = (self.current_time.saturating_sub(timestamp)) / 86400;

        if age_days == 0 {
            String::from("Today")
        } else if age_days == 1 {
            String::from("Yesterday")
        } else if age_days < 7 {
            format!("{} days ago", age_days)
        } else if age_days < 30 {
            format!("{} weeks ago", age_days / 7)
        } else if age_days < 365 {
            format!("{} months ago", age_days / 30)
        } else {
            format!("{} years ago", age_days / 365)
        }
    }

    /// Get statistics
    pub fn get_stats(&self) -> HistoryStats {
        let mut total_visits = 0u32;
        let mut total_typed = 0u32;
        let mut oldest = u64::MAX;
        let mut newest = 0u64;
        let mut domain_counts: BTreeMap<String, u32> = BTreeMap::new();

        for entry in self.entries.values() {
            total_visits += entry.visit_count;
            total_typed += entry.typed_count;

            if entry.first_visit < oldest {
                oldest = entry.first_visit;
            }
            if entry.last_visit > newest {
                newest = entry.last_visit;
            }

            let domain = String::from(entry.domain());
            *domain_counts.entry(domain).or_insert(0) += entry.visit_count;
        }

        let (most_visited_domain, most_visited_count) = domain_counts.iter()
            .max_by_key(|(_, &count)| count)
            .map(|(domain, &count)| (Some(domain.clone()), count))
            .unwrap_or((None, 0));

        HistoryStats {
            total_entries: self.entries.len(),
            total_visits,
            total_typed,
            unique_domains: domain_counts.len(),
            oldest_visit: if oldest == u64::MAX { 0 } else { oldest },
            newest_visit: newest,
            most_visited_domain,
            most_visited_count,
        }
    }

    fn enforce_limits(&mut self) {
        // Remove entries over max count
        if self.entries.len() > self.max_entries {
            // Find oldest entries to remove
            let mut entries_by_age: Vec<_> = self.entries.iter()
                .map(|(id, entry)| (*id, entry.last_visit))
                .collect();
            entries_by_age.sort_by_key(|(_, visit)| *visit);

            let to_remove = self.entries.len() - self.max_entries;
            for (id, _) in entries_by_age.into_iter().take(to_remove) {
                if let Some(entry) = self.entries.remove(&id) {
                    self.url_to_id.remove(&entry.url);
                }
            }
        }

        // Remove entries older than max age
        let age_cutoff = self.current_time.saturating_sub(self.max_age_days as u64 * 86400);
        let old_ids: Vec<_> = self.entries.iter()
            .filter(|(_, entry)| entry.last_visit < age_cutoff)
            .map(|(id, _)| *id)
            .collect();

        for id in old_ids {
            if let Some(entry) = self.entries.remove(&id) {
                self.url_to_id.remove(&entry.url);
            }
        }
    }

    /// Set current time (for timestamping visits)
    pub fn set_current_time(&mut self, time: u64) {
        self.current_time = time;
    }

    /// Settings
    pub fn is_enabled(&self) -> bool {
        self.enable_history
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enable_history = enabled;
    }

    pub fn clear_on_exit(&self) -> bool {
        self.clear_on_exit
    }

    pub fn set_clear_on_exit(&mut self, clear: bool) {
        self.clear_on_exit = clear;
    }

    pub fn max_age_days(&self) -> u32 {
        self.max_age_days
    }

    pub fn set_max_age_days(&mut self, days: u32) {
        self.max_age_days = days;
        self.enforce_limits();
    }

    /// Autocomplete suggestions based on history
    pub fn get_suggestions(&self, input: &str, limit: usize) -> Vec<&HistoryEntry> {
        let input_lower = input.to_ascii_lowercase();
        let current_time = self.current_time;

        let mut candidates: Vec<_> = self.entries.values()
            .filter(|e| {
                !e.is_hidden &&
                (e.url.to_ascii_lowercase().contains(&input_lower) ||
                 e.title.to_ascii_lowercase().contains(&input_lower))
            })
            .map(|e| (e, e.frecency_score(current_time)))
            .collect();

        // Prioritize matches that start with the input
        candidates.sort_by(|(a, score_a), (b, score_b)| {
            let a_starts = a.url.to_ascii_lowercase().starts_with(&input_lower) ||
                          a.domain().to_ascii_lowercase().starts_with(&input_lower);
            let b_starts = b.url.to_ascii_lowercase().starts_with(&input_lower) ||
                          b.domain().to_ascii_lowercase().starts_with(&input_lower);

            match (a_starts, b_starts) {
                (true, false) => core::cmp::Ordering::Less,
                (false, true) => core::cmp::Ordering::Greater,
                _ => score_b.cmp(score_a),
            }
        });

        candidates.into_iter()
            .take(limit)
            .map(|(e, _)| e)
            .collect()
    }

    /// Total entries count
    pub fn total_entries(&self) -> usize {
        self.entries.len()
    }

    /// Add sample data for demo
    pub fn add_sample_data(&mut self) {
        self.current_time = 1705600000; // Some timestamp

        // Add some sample history
        self.record_visit("https://www.google.com", "Google", VisitType::Typed);
        self.record_visit("https://github.com", "GitHub", VisitType::Typed);
        self.record_visit("https://stackoverflow.com/questions/12345", "How to fix rust borrow checker - Stack Overflow", VisitType::Link);
        self.record_visit("https://doc.rust-lang.org/book/", "The Rust Programming Language", VisitType::Link);
        self.record_visit("https://crates.io", "crates.io: Rust Package Registry", VisitType::Typed);
        self.record_visit("https://news.ycombinator.com", "Hacker News", VisitType::Typed);
        self.record_visit("https://www.reddit.com/r/rust", "r/rust", VisitType::Link);
        self.record_visit("https://en.wikipedia.org/wiki/Rust", "Rust (programming language) - Wikipedia", VisitType::Link);

        // Simulate some revisits
        self.current_time += 3600;
        self.record_visit("https://github.com", "GitHub", VisitType::Typed);
        self.record_visit("https://www.google.com", "Google", VisitType::AutoComplete);

        self.current_time += 7200;
        self.record_visit("https://github.com", "GitHub", VisitType::Bookmark);
        self.record_visit("https://doc.rust-lang.org/std/", "std - Rust", VisitType::Link);
    }
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize history module
pub fn init() -> HistoryManager {
    let mut manager = HistoryManager::new();
    manager.add_sample_data();
    manager
}
