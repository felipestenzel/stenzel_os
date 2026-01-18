//! Browser Password Manager
//!
//! Secure password storage and autofill for the web browser.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;

/// Unique credential identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CredentialId(u64);

impl CredentialId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Credential type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialType {
    /// Website login
    Login,
    /// Credit card
    CreditCard,
    /// Address/identity
    Address,
    /// Secure note
    SecureNote,
    /// Custom field
    Custom,
}

impl CredentialType {
    pub fn name(&self) -> &'static str {
        match self {
            CredentialType::Login => "Login",
            CredentialType::CreditCard => "Credit Card",
            CredentialType::Address => "Address",
            CredentialType::SecureNote => "Secure Note",
            CredentialType::Custom => "Custom",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            CredentialType::Login => "🔑",
            CredentialType::CreditCard => "💳",
            CredentialType::Address => "📍",
            CredentialType::SecureNote => "📝",
            CredentialType::Custom => "📦",
        }
    }
}

/// Password strength level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PasswordStrength {
    VeryWeak,
    Weak,
    Fair,
    Strong,
    VeryStrong,
}

impl PasswordStrength {
    pub fn name(&self) -> &'static str {
        match self {
            PasswordStrength::VeryWeak => "Very Weak",
            PasswordStrength::Weak => "Weak",
            PasswordStrength::Fair => "Fair",
            PasswordStrength::Strong => "Strong",
            PasswordStrength::VeryStrong => "Very Strong",
        }
    }

    pub fn color(&self) -> u32 {
        match self {
            PasswordStrength::VeryWeak => 0xFF0000,  // Red
            PasswordStrength::Weak => 0xFF6600,     // Orange
            PasswordStrength::Fair => 0xFFCC00,     // Yellow
            PasswordStrength::Strong => 0x99CC00,   // Light green
            PasswordStrength::VeryStrong => 0x00CC00, // Green
        }
    }

    /// Calculate password strength
    pub fn calculate(password: &str) -> Self {
        let len = password.len();
        let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
        let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
        let has_digit = password.chars().any(|c| c.is_ascii_digit());
        let has_special = password.chars().any(|c| !c.is_alphanumeric());

        let mut score = 0;

        // Length score
        if len >= 8 { score += 1; }
        if len >= 12 { score += 1; }
        if len >= 16 { score += 1; }
        if len >= 20 { score += 1; }

        // Character variety score
        if has_upper { score += 1; }
        if has_lower { score += 1; }
        if has_digit { score += 1; }
        if has_special { score += 1; }

        match score {
            0..=2 => PasswordStrength::VeryWeak,
            3..=4 => PasswordStrength::Weak,
            5..=6 => PasswordStrength::Fair,
            7 => PasswordStrength::Strong,
            _ => PasswordStrength::VeryStrong,
        }
    }
}

/// Stored login credential
#[derive(Debug, Clone)]
pub struct LoginCredential {
    pub id: CredentialId,
    pub site_name: String,
    pub url: String,
    pub username: String,
    pub password: String,
    pub totp_secret: Option<String>,
    pub notes: Option<String>,
    pub created: u64,
    pub modified: u64,
    pub last_used: Option<u64>,
    pub use_count: u32,
    pub favorite: bool,
    pub folder_id: Option<FolderId>,
}

impl LoginCredential {
    pub fn new(id: CredentialId, site_name: &str, url: &str, username: &str, password: &str) -> Self {
        Self {
            id,
            site_name: String::from(site_name),
            url: String::from(url),
            username: String::from(username),
            password: String::from(password),
            totp_secret: None,
            notes: None,
            created: 0,
            modified: 0,
            last_used: None,
            use_count: 0,
            favorite: false,
            folder_id: None,
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

    pub fn display_name(&self) -> &str {
        if self.site_name.is_empty() {
            self.domain()
        } else {
            &self.site_name
        }
    }

    pub fn password_strength(&self) -> PasswordStrength {
        PasswordStrength::calculate(&self.password)
    }

    pub fn record_use(&mut self, timestamp: u64) {
        self.use_count += 1;
        self.last_used = Some(timestamp);
    }
}

/// Credit card credential
#[derive(Debug, Clone)]
pub struct CreditCardCredential {
    pub id: CredentialId,
    pub name: String,
    pub cardholder_name: String,
    pub card_number: String,
    pub expiry_month: u8,
    pub expiry_year: u16,
    pub cvv: String,
    pub billing_address: Option<AddressInfo>,
    pub notes: Option<String>,
    pub created: u64,
    pub modified: u64,
    pub favorite: bool,
    pub folder_id: Option<FolderId>,
}

impl CreditCardCredential {
    pub fn new(id: CredentialId, name: &str, card_number: &str) -> Self {
        Self {
            id,
            name: String::from(name),
            cardholder_name: String::new(),
            card_number: String::from(card_number),
            expiry_month: 0,
            expiry_year: 0,
            cvv: String::new(),
            billing_address: None,
            notes: None,
            created: 0,
            modified: 0,
            favorite: false,
            folder_id: None,
        }
    }

    pub fn masked_number(&self) -> String {
        let len = self.card_number.len();
        if len <= 4 {
            return String::from(&self.card_number);
        }
        let visible = &self.card_number[len - 4..];
        format!("**** **** **** {}", visible)
    }

    pub fn card_type(&self) -> CardType {
        CardType::detect(&self.card_number)
    }

    pub fn is_expired(&self, current_month: u8, current_year: u16) -> bool {
        if self.expiry_year < current_year {
            return true;
        }
        if self.expiry_year == current_year && self.expiry_month < current_month {
            return true;
        }
        false
    }
}

/// Credit card type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardType {
    Visa,
    Mastercard,
    Amex,
    Discover,
    DinersClub,
    Jcb,
    UnionPay,
    Unknown,
}

impl CardType {
    pub fn name(&self) -> &'static str {
        match self {
            CardType::Visa => "Visa",
            CardType::Mastercard => "Mastercard",
            CardType::Amex => "American Express",
            CardType::Discover => "Discover",
            CardType::DinersClub => "Diners Club",
            CardType::Jcb => "JCB",
            CardType::UnionPay => "UnionPay",
            CardType::Unknown => "Unknown",
        }
    }

    pub fn detect(number: &str) -> Self {
        let digits: String = number.chars().filter(|c| c.is_ascii_digit()).collect();

        if digits.starts_with('4') {
            CardType::Visa
        } else if digits.starts_with("51") || digits.starts_with("52") ||
                  digits.starts_with("53") || digits.starts_with("54") ||
                  digits.starts_with("55") || digits.starts_with("22") {
            CardType::Mastercard
        } else if digits.starts_with("34") || digits.starts_with("37") {
            CardType::Amex
        } else if digits.starts_with("6011") || digits.starts_with("65") {
            CardType::Discover
        } else if digits.starts_with("36") || digits.starts_with("38") {
            CardType::DinersClub
        } else if digits.starts_with("35") {
            CardType::Jcb
        } else if digits.starts_with("62") {
            CardType::UnionPay
        } else {
            CardType::Unknown
        }
    }
}

/// Address information
#[derive(Debug, Clone)]
pub struct AddressInfo {
    pub id: CredentialId,
    pub name: String,
    pub full_name: String,
    pub organization: Option<String>,
    pub street_address: String,
    pub street_address_2: Option<String>,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub country: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub created: u64,
    pub modified: u64,
    pub favorite: bool,
    pub folder_id: Option<FolderId>,
}

impl AddressInfo {
    pub fn new(id: CredentialId, name: &str) -> Self {
        Self {
            id,
            name: String::from(name),
            full_name: String::new(),
            organization: None,
            street_address: String::new(),
            street_address_2: None,
            city: String::new(),
            state: String::new(),
            postal_code: String::new(),
            country: String::new(),
            phone: None,
            email: None,
            created: 0,
            modified: 0,
            favorite: false,
            folder_id: None,
        }
    }

    pub fn format_address(&self) -> String {
        let mut parts = Vec::new();
        parts.push(self.street_address.clone());
        if let Some(ref addr2) = self.street_address_2 {
            if !addr2.is_empty() {
                parts.push(addr2.clone());
            }
        }
        parts.push(format!("{}, {} {}", self.city, self.state, self.postal_code));
        parts.push(self.country.clone());
        parts.join("\n")
    }
}

/// Secure note
#[derive(Debug, Clone)]
pub struct SecureNote {
    pub id: CredentialId,
    pub title: String,
    pub content: String,
    pub created: u64,
    pub modified: u64,
    pub favorite: bool,
    pub folder_id: Option<FolderId>,
}

impl SecureNote {
    pub fn new(id: CredentialId, title: &str) -> Self {
        Self {
            id,
            title: String::from(title),
            content: String::new(),
            created: 0,
            modified: 0,
            favorite: false,
            folder_id: None,
        }
    }
}

/// Password folder ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FolderId(u64);

impl FolderId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Password folder
#[derive(Debug, Clone)]
pub struct PasswordFolder {
    pub id: FolderId,
    pub name: String,
    pub parent_id: Option<FolderId>,
    pub created: u64,
    pub modified: u64,
}

impl PasswordFolder {
    pub fn new(id: FolderId, name: &str) -> Self {
        Self {
            id,
            name: String::from(name),
            parent_id: None,
            created: 0,
            modified: 0,
        }
    }
}

/// Breached password info
#[derive(Debug, Clone)]
pub struct BreachInfo {
    pub credential_id: CredentialId,
    pub breach_name: String,
    pub breach_date: u64,
    pub compromised_data: Vec<String>,
    pub severity: BreachSeverity,
}

/// Breach severity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreachSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl BreachSeverity {
    pub fn name(&self) -> &'static str {
        match self {
            BreachSeverity::Low => "Low",
            BreachSeverity::Medium => "Medium",
            BreachSeverity::High => "High",
            BreachSeverity::Critical => "Critical",
        }
    }

    pub fn color(&self) -> u32 {
        match self {
            BreachSeverity::Low => 0x00CC00,
            BreachSeverity::Medium => 0xFFCC00,
            BreachSeverity::High => 0xFF6600,
            BreachSeverity::Critical => 0xFF0000,
        }
    }
}

/// Password generation options
#[derive(Debug, Clone)]
pub struct PasswordGeneratorOptions {
    pub length: usize,
    pub use_uppercase: bool,
    pub use_lowercase: bool,
    pub use_digits: bool,
    pub use_symbols: bool,
    pub avoid_ambiguous: bool,
    pub min_uppercase: usize,
    pub min_lowercase: usize,
    pub min_digits: usize,
    pub min_symbols: usize,
}

impl Default for PasswordGeneratorOptions {
    fn default() -> Self {
        Self {
            length: 16,
            use_uppercase: true,
            use_lowercase: true,
            use_digits: true,
            use_symbols: true,
            avoid_ambiguous: true,
            min_uppercase: 1,
            min_lowercase: 1,
            min_digits: 1,
            min_symbols: 1,
        }
    }
}

/// Password sort order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordSortOrder {
    NameAsc,
    NameDesc,
    DateCreatedDesc,
    DateCreatedAsc,
    DateModifiedDesc,
    DateUsedDesc,
    UsageCountDesc,
}

impl PasswordSortOrder {
    pub fn name(&self) -> &'static str {
        match self {
            PasswordSortOrder::NameAsc => "Name A-Z",
            PasswordSortOrder::NameDesc => "Name Z-A",
            PasswordSortOrder::DateCreatedDesc => "Newest First",
            PasswordSortOrder::DateCreatedAsc => "Oldest First",
            PasswordSortOrder::DateModifiedDesc => "Recently Modified",
            PasswordSortOrder::DateUsedDesc => "Recently Used",
            PasswordSortOrder::UsageCountDesc => "Most Used",
        }
    }
}

/// Password manager error
#[derive(Debug, Clone)]
pub enum PasswordError {
    NotFound,
    DuplicateEntry,
    InvalidFolder,
    EncryptionFailed,
    DecryptionFailed,
    WeakPassword,
    ImportFailed(String),
    ExportFailed(String),
    Locked,
}

pub type PasswordResult<T> = Result<T, PasswordError>;

/// Master password status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultStatus {
    Locked,
    Unlocked,
    NoMasterPassword,
}

/// Password manager
pub struct PasswordManager {
    logins: BTreeMap<CredentialId, LoginCredential>,
    cards: BTreeMap<CredentialId, CreditCardCredential>,
    addresses: BTreeMap<CredentialId, AddressInfo>,
    notes: BTreeMap<CredentialId, SecureNote>,
    folders: BTreeMap<FolderId, PasswordFolder>,

    next_credential_id: u64,
    next_folder_id: u64,

    // Security state
    vault_status: VaultStatus,
    master_password_hash: Option<[u8; 32]>,
    auto_lock_minutes: u32,
    last_activity: u64,

    // Settings
    auto_save: bool,
    auto_fill: bool,
    remember_generator_settings: bool,
    generator_options: PasswordGeneratorOptions,

    current_time: u64,
}

impl PasswordManager {
    pub fn new() -> Self {
        Self {
            logins: BTreeMap::new(),
            cards: BTreeMap::new(),
            addresses: BTreeMap::new(),
            notes: BTreeMap::new(),
            folders: BTreeMap::new(),
            next_credential_id: 1,
            next_folder_id: 1,
            vault_status: VaultStatus::NoMasterPassword,
            master_password_hash: None,
            auto_lock_minutes: 15,
            last_activity: 0,
            auto_save: true,
            auto_fill: true,
            remember_generator_settings: true,
            generator_options: PasswordGeneratorOptions::default(),
            current_time: 0,
        }
    }

    /// Set master password
    pub fn set_master_password(&mut self, password: &str) {
        // In a real implementation, this would use a proper KDF like Argon2
        let hash = self.simple_hash(password);
        self.master_password_hash = Some(hash);
        self.vault_status = VaultStatus::Unlocked;
    }

    /// Verify master password
    pub fn verify_master_password(&self, password: &str) -> bool {
        if let Some(ref stored_hash) = self.master_password_hash {
            let hash = self.simple_hash(password);
            hash == *stored_hash
        } else {
            false
        }
    }

    /// Simple hash for demo (use proper crypto in production)
    fn simple_hash(&self, input: &str) -> [u8; 32] {
        let mut hash = [0u8; 32];
        for (i, byte) in input.bytes().enumerate() {
            hash[i % 32] ^= byte;
            hash[(i + 1) % 32] = hash[(i + 1) % 32].wrapping_add(byte);
        }
        hash
    }

    /// Unlock vault
    pub fn unlock(&mut self, password: &str) -> PasswordResult<()> {
        if self.verify_master_password(password) {
            self.vault_status = VaultStatus::Unlocked;
            self.last_activity = self.current_time;
            Ok(())
        } else {
            Err(PasswordError::DecryptionFailed)
        }
    }

    /// Lock vault
    pub fn lock(&mut self) {
        self.vault_status = VaultStatus::Locked;
    }

    /// Check if vault is unlocked
    pub fn is_unlocked(&self) -> bool {
        self.vault_status == VaultStatus::Unlocked
    }

    /// Check for auto-lock timeout
    pub fn check_auto_lock(&mut self) {
        if self.vault_status == VaultStatus::Unlocked {
            let timeout_seconds = self.auto_lock_minutes as u64 * 60;
            if self.current_time > self.last_activity + timeout_seconds {
                self.lock();
            }
        }
    }

    /// Record activity (reset auto-lock timer)
    pub fn record_activity(&mut self) {
        self.last_activity = self.current_time;
    }

    /// Add login credential
    pub fn add_login(&mut self, site_name: &str, url: &str, username: &str, password: &str) -> PasswordResult<CredentialId> {
        if !self.is_unlocked() {
            return Err(PasswordError::Locked);
        }

        let id = CredentialId::new(self.next_credential_id);
        self.next_credential_id += 1;

        let mut cred = LoginCredential::new(id, site_name, url, username, password);
        cred.created = self.current_time;
        cred.modified = self.current_time;

        self.logins.insert(id, cred);
        Ok(id)
    }

    /// Update login credential
    pub fn update_login(&mut self, id: CredentialId, username: &str, password: &str) -> PasswordResult<()> {
        if !self.is_unlocked() {
            return Err(PasswordError::Locked);
        }

        let login = self.logins.get_mut(&id).ok_or(PasswordError::NotFound)?;
        login.username = String::from(username);
        login.password = String::from(password);
        login.modified = self.current_time;
        Ok(())
    }

    /// Delete login
    pub fn delete_login(&mut self, id: CredentialId) -> PasswordResult<()> {
        if !self.is_unlocked() {
            return Err(PasswordError::Locked);
        }

        self.logins.remove(&id).ok_or(PasswordError::NotFound)?;
        Ok(())
    }

    /// Get login by ID
    pub fn get_login(&self, id: CredentialId) -> Option<&LoginCredential> {
        if !self.is_unlocked() {
            return None;
        }
        self.logins.get(&id)
    }

    /// Find logins for URL
    pub fn find_logins_for_url(&self, url: &str) -> Vec<&LoginCredential> {
        if !self.is_unlocked() {
            return Vec::new();
        }

        let domain = self.extract_domain(url);

        self.logins.values()
            .filter(|login| {
                let login_domain = self.extract_domain(&login.url);
                login_domain == domain
            })
            .collect()
    }

    fn extract_domain(&self, url: &str) -> String {
        if let Some(start) = url.find("://") {
            let after_proto = &url[start + 3..];
            if let Some(end) = after_proto.find('/') {
                return String::from(&after_proto[..end]);
            }
            return String::from(after_proto);
        }
        String::from(url)
    }

    /// Search credentials
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        if !self.is_unlocked() {
            return Vec::new();
        }

        let query_lower = query.to_ascii_lowercase();
        let mut results = Vec::new();

        // Search logins
        for login in self.logins.values() {
            let name_lower = login.site_name.to_ascii_lowercase();
            let url_lower = login.url.to_ascii_lowercase();
            let username_lower = login.username.to_ascii_lowercase();

            if name_lower.contains(&query_lower) ||
               url_lower.contains(&query_lower) ||
               username_lower.contains(&query_lower) {
                results.push(SearchResult {
                    credential_type: CredentialType::Login,
                    id: login.id,
                    name: login.display_name().into(),
                    detail: login.username.clone(),
                    favorite: login.favorite,
                });
            }
        }

        // Search cards
        for card in self.cards.values() {
            let name_lower = card.name.to_ascii_lowercase();
            if name_lower.contains(&query_lower) {
                results.push(SearchResult {
                    credential_type: CredentialType::CreditCard,
                    id: card.id,
                    name: card.name.clone(),
                    detail: card.masked_number(),
                    favorite: card.favorite,
                });
            }
        }

        // Search addresses
        for addr in self.addresses.values() {
            let name_lower = addr.name.to_ascii_lowercase();
            if name_lower.contains(&query_lower) {
                results.push(SearchResult {
                    credential_type: CredentialType::Address,
                    id: addr.id,
                    name: addr.name.clone(),
                    detail: addr.city.clone(),
                    favorite: addr.favorite,
                });
            }
        }

        // Search notes
        for note in self.notes.values() {
            let title_lower = note.title.to_ascii_lowercase();
            if title_lower.contains(&query_lower) {
                results.push(SearchResult {
                    credential_type: CredentialType::SecureNote,
                    id: note.id,
                    name: note.title.clone(),
                    detail: String::new(),
                    favorite: note.favorite,
                });
            }
        }

        // Sort favorites first
        results.sort_by(|a, b| b.favorite.cmp(&a.favorite));
        results
    }

    /// Get all logins
    pub fn get_all_logins(&self) -> Vec<&LoginCredential> {
        if !self.is_unlocked() {
            return Vec::new();
        }
        self.logins.values().collect()
    }

    /// Get favorites
    pub fn get_favorites(&self) -> Vec<SearchResult> {
        if !self.is_unlocked() {
            return Vec::new();
        }

        let mut results = Vec::new();

        for login in self.logins.values().filter(|l| l.favorite) {
            results.push(SearchResult {
                credential_type: CredentialType::Login,
                id: login.id,
                name: login.display_name().into(),
                detail: login.username.clone(),
                favorite: true,
            });
        }

        for card in self.cards.values().filter(|c| c.favorite) {
            results.push(SearchResult {
                credential_type: CredentialType::CreditCard,
                id: card.id,
                name: card.name.clone(),
                detail: card.masked_number(),
                favorite: true,
            });
        }

        results
    }

    /// Check password health
    pub fn check_password_health(&self) -> PasswordHealthReport {
        if !self.is_unlocked() {
            return PasswordHealthReport::default();
        }

        let mut report = PasswordHealthReport::default();
        let mut password_counts: BTreeMap<String, u32> = BTreeMap::new();

        for login in self.logins.values() {
            report.total_passwords += 1;

            // Check strength
            let strength = login.password_strength();
            match strength {
                PasswordStrength::VeryWeak | PasswordStrength::Weak => {
                    report.weak_passwords.push(login.id);
                }
                _ => {}
            }

            // Check for reuse
            *password_counts.entry(login.password.clone()).or_insert(0) += 1;

            // Check for old passwords (not modified in 90 days)
            if self.current_time.saturating_sub(login.modified) > 90 * 86400 {
                report.old_passwords.push(login.id);
            }
        }

        // Find reused passwords
        for login in self.logins.values() {
            if let Some(&count) = password_counts.get(&login.password) {
                if count > 1 {
                    report.reused_passwords.push(login.id);
                }
            }
        }

        // Calculate score
        let weak_penalty = report.weak_passwords.len() as u32 * 10;
        let reused_penalty = report.reused_passwords.len() as u32 * 5;
        let old_penalty = report.old_passwords.len() as u32 * 2;

        report.overall_score = 100u32.saturating_sub(weak_penalty + reused_penalty + old_penalty);
        report
    }

    /// Generate password
    pub fn generate_password(&self, options: &PasswordGeneratorOptions) -> String {
        let mut chars = Vec::new();

        if options.use_uppercase {
            if options.avoid_ambiguous {
                chars.extend("ABCDEFGHJKLMNPQRSTUVWXYZ".chars());
            } else {
                chars.extend("ABCDEFGHIJKLMNOPQRSTUVWXYZ".chars());
            }
        }

        if options.use_lowercase {
            if options.avoid_ambiguous {
                chars.extend("abcdefghjkmnpqrstuvwxyz".chars());
            } else {
                chars.extend("abcdefghijklmnopqrstuvwxyz".chars());
            }
        }

        if options.use_digits {
            if options.avoid_ambiguous {
                chars.extend("23456789".chars());
            } else {
                chars.extend("0123456789".chars());
            }
        }

        if options.use_symbols {
            chars.extend("!@#$%^&*()_+-=[]{}|;:,.<>?".chars());
        }

        if chars.is_empty() {
            return String::new();
        }

        // Simple pseudo-random password generation (use proper CSPRNG in production)
        let mut password = String::new();
        let mut seed = self.current_time;

        for _ in 0..options.length {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let index = (seed as usize) % chars.len();
            password.push(chars[index]);
        }

        password
    }

    /// Create folder
    pub fn create_folder(&mut self, name: &str) -> PasswordResult<FolderId> {
        if !self.is_unlocked() {
            return Err(PasswordError::Locked);
        }

        let id = FolderId::new(self.next_folder_id);
        self.next_folder_id += 1;

        let mut folder = PasswordFolder::new(id, name);
        folder.created = self.current_time;
        folder.modified = self.current_time;

        self.folders.insert(id, folder);
        Ok(id)
    }

    /// Get all folders
    pub fn get_folders(&self) -> Vec<&PasswordFolder> {
        self.folders.values().collect()
    }

    /// Statistics
    pub fn get_stats(&self) -> PasswordStats {
        PasswordStats {
            total_logins: self.logins.len(),
            total_cards: self.cards.len(),
            total_addresses: self.addresses.len(),
            total_notes: self.notes.len(),
            total_folders: self.folders.len(),
            favorites_count: self.logins.values().filter(|l| l.favorite).count() +
                           self.cards.values().filter(|c| c.favorite).count(),
        }
    }

    /// Set current time
    pub fn set_current_time(&mut self, time: u64) {
        self.current_time = time;
    }

    /// Vault status
    pub fn vault_status(&self) -> VaultStatus {
        self.vault_status
    }

    /// Add sample data for demo
    pub fn add_sample_data(&mut self) {
        // Set a master password
        self.set_master_password("demo123");

        self.current_time = 1705600000;

        // Add some sample logins
        let _ = self.add_login("GitHub", "https://github.com", "user@example.com", "Gh1tHub$ecure!");
        let _ = self.add_login("Google", "https://accounts.google.com", "user@gmail.com", "G00gle#Pass");
        let _ = self.add_login("Amazon", "https://www.amazon.com", "user@example.com", "weak123"); // Weak password example
        let _ = self.add_login("Twitter", "https://twitter.com", "user@example.com", "G00gle#Pass"); // Reused password example
        let _ = self.add_login("Netflix", "https://www.netflix.com", "user@example.com", "N3tfl1x$ecure2024!");

        // Mark some as favorites
        if let Some(login) = self.logins.values_mut().find(|l| l.site_name == "GitHub") {
            login.favorite = true;
        }
    }
}

impl Default for PasswordManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub credential_type: CredentialType,
    pub id: CredentialId,
    pub name: String,
    pub detail: String,
    pub favorite: bool,
}

/// Password health report
#[derive(Debug, Clone, Default)]
pub struct PasswordHealthReport {
    pub total_passwords: usize,
    pub weak_passwords: Vec<CredentialId>,
    pub reused_passwords: Vec<CredentialId>,
    pub old_passwords: Vec<CredentialId>,
    pub breached_passwords: Vec<CredentialId>,
    pub overall_score: u32,
}

/// Password statistics
#[derive(Debug, Clone)]
pub struct PasswordStats {
    pub total_logins: usize,
    pub total_cards: usize,
    pub total_addresses: usize,
    pub total_notes: usize,
    pub total_folders: usize,
    pub favorites_count: usize,
}

/// Initialize password manager
pub fn init() -> PasswordManager {
    let mut manager = PasswordManager::new();
    manager.add_sample_data();
    manager
}
