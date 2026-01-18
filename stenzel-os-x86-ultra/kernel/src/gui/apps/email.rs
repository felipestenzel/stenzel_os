//! Email Client Application
//!
//! Full-featured email client with IMAP/POP3/SMTP support.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton, theme};

/// Email protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailProtocol {
    Imap,
    Pop3,
    Exchange,
    Gmail,
    Outlook,
}

impl EmailProtocol {
    pub fn name(&self) -> &'static str {
        match self {
            EmailProtocol::Imap => "IMAP",
            EmailProtocol::Pop3 => "POP3",
            EmailProtocol::Exchange => "Exchange",
            EmailProtocol::Gmail => "Gmail",
            EmailProtocol::Outlook => "Outlook",
        }
    }

    pub fn default_port(&self, secure: bool) -> u16 {
        match self {
            EmailProtocol::Imap => if secure { 993 } else { 143 },
            EmailProtocol::Pop3 => if secure { 995 } else { 110 },
            EmailProtocol::Exchange | EmailProtocol::Gmail | EmailProtocol::Outlook => 993,
        }
    }
}

/// SMTP settings
#[derive(Debug, Clone)]
pub struct SmtpSettings {
    pub server: String,
    pub port: u16,
    pub use_tls: bool,
    pub use_starttls: bool,
    pub auth_method: AuthMethod,
}

impl SmtpSettings {
    pub fn new(server: &str) -> Self {
        Self {
            server: server.to_string(),
            port: 587,
            use_tls: false,
            use_starttls: true,
            auth_method: AuthMethod::Plain,
        }
    }
}

/// Authentication method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    Plain,
    Login,
    CramMd5,
    OAuth2,
    XOAuth2,
}

impl AuthMethod {
    pub fn name(&self) -> &'static str {
        match self {
            AuthMethod::Plain => "PLAIN",
            AuthMethod::Login => "LOGIN",
            AuthMethod::CramMd5 => "CRAM-MD5",
            AuthMethod::OAuth2 => "OAuth2",
            AuthMethod::XOAuth2 => "XOAUTH2",
        }
    }
}

/// Email account configuration
#[derive(Debug, Clone)]
pub struct EmailAccount {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub display_name: String,
    pub protocol: EmailProtocol,
    pub incoming_server: String,
    pub incoming_port: u16,
    pub use_ssl: bool,
    pub username: String,
    pub smtp: SmtpSettings,
    pub signature: Option<String>,
    pub is_default: bool,
    pub sync_interval_minutes: u32,
    pub last_sync: u64,
}

impl EmailAccount {
    pub fn new(name: &str, email: &str, protocol: EmailProtocol) -> Self {
        Self {
            id: 0,
            name: name.to_string(),
            email: email.to_string(),
            display_name: name.to_string(),
            protocol,
            incoming_server: String::new(),
            incoming_port: protocol.default_port(true),
            use_ssl: true,
            username: email.to_string(),
            smtp: SmtpSettings::new(""),
            signature: None,
            is_default: false,
            sync_interval_minutes: 15,
            last_sync: 0,
        }
    }
}

/// Mailbox/folder type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxType {
    Inbox,
    Sent,
    Drafts,
    Trash,
    Spam,
    Archive,
    Starred,
    Important,
    Custom,
}

impl MailboxType {
    pub fn name(&self) -> &'static str {
        match self {
            MailboxType::Inbox => "Inbox",
            MailboxType::Sent => "Sent",
            MailboxType::Drafts => "Drafts",
            MailboxType::Trash => "Trash",
            MailboxType::Spam => "Spam",
            MailboxType::Archive => "Archive",
            MailboxType::Starred => "Starred",
            MailboxType::Important => "Important",
            MailboxType::Custom => "Folder",
        }
    }

    pub fn icon(&self) -> char {
        match self {
            MailboxType::Inbox => 'I',
            MailboxType::Sent => 'S',
            MailboxType::Drafts => 'D',
            MailboxType::Trash => 'T',
            MailboxType::Spam => '!',
            MailboxType::Archive => 'A',
            MailboxType::Starred => '*',
            MailboxType::Important => '!',
            MailboxType::Custom => 'F',
        }
    }
}

/// Mailbox/folder
#[derive(Debug, Clone)]
pub struct Mailbox {
    pub id: u64,
    pub account_id: u64,
    pub name: String,
    pub mailbox_type: MailboxType,
    pub path: String,
    pub unread_count: u32,
    pub total_count: u32,
    pub parent_id: Option<u64>,
    pub children: Vec<u64>,
    pub is_subscribed: bool,
    pub is_selectable: bool,
}

impl Mailbox {
    pub fn new(name: &str, mailbox_type: MailboxType, account_id: u64) -> Self {
        Self {
            id: 0,
            account_id,
            name: name.to_string(),
            mailbox_type,
            path: name.to_string(),
            unread_count: 0,
            total_count: 0,
            parent_id: None,
            children: Vec::new(),
            is_subscribed: true,
            is_selectable: true,
        }
    }
}

/// Email address with optional display name
#[derive(Debug, Clone)]
pub struct EmailAddress {
    pub address: String,
    pub display_name: Option<String>,
}

impl EmailAddress {
    pub fn new(address: &str) -> Self {
        Self {
            address: address.to_string(),
            display_name: None,
        }
    }

    pub fn with_name(address: &str, name: &str) -> Self {
        Self {
            address: address.to_string(),
            display_name: Some(name.to_string()),
        }
    }

    pub fn format(&self) -> String {
        match &self.display_name {
            Some(name) => format!("{} <{}>", name, self.address),
            None => self.address.clone(),
        }
    }
}

/// Email message flags
#[derive(Debug, Clone, Copy, Default)]
pub struct MessageFlags {
    pub seen: bool,
    pub answered: bool,
    pub flagged: bool,
    pub deleted: bool,
    pub draft: bool,
    pub recent: bool,
}

impl MessageFlags {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read() -> Self {
        Self { seen: true, ..Default::default() }
    }
}

/// Email attachment
#[derive(Debug, Clone)]
pub struct Attachment {
    pub id: u64,
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
    pub content_id: Option<String>,
    pub is_inline: bool,
    pub data: Option<Vec<u8>>,
}

impl Attachment {
    pub fn new(filename: &str, mime_type: &str, size: u64) -> Self {
        Self {
            id: 0,
            filename: filename.to_string(),
            mime_type: mime_type.to_string(),
            size,
            content_id: None,
            is_inline: false,
            data: None,
        }
    }

    pub fn format_size(&self) -> String {
        if self.size < 1024 {
            format!("{} B", self.size)
        } else if self.size < 1024 * 1024 {
            format!("{:.1} KB", self.size as f64 / 1024.0)
        } else if self.size < 1024 * 1024 * 1024 {
            format!("{:.1} MB", self.size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", self.size as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

/// Email message priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessagePriority {
    Highest,
    High,
    #[default]
    Normal,
    Low,
    Lowest,
}

impl MessagePriority {
    pub fn name(&self) -> &'static str {
        match self {
            MessagePriority::Highest => "Highest",
            MessagePriority::High => "High",
            MessagePriority::Normal => "Normal",
            MessagePriority::Low => "Low",
            MessagePriority::Lowest => "Lowest",
        }
    }
}

/// Email message
#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub id: u64,
    pub uid: u64,
    pub account_id: u64,
    pub mailbox_id: u64,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub from: Vec<EmailAddress>,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub bcc: Vec<EmailAddress>,
    pub reply_to: Vec<EmailAddress>,
    pub subject: String,
    pub date: u64,
    pub received_date: u64,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub attachments: Vec<Attachment>,
    pub flags: MessageFlags,
    pub priority: MessagePriority,
    pub size: u64,
    pub thread_id: Option<u64>,
    pub labels: Vec<String>,
}

impl EmailMessage {
    pub fn new() -> Self {
        Self {
            id: 0,
            uid: 0,
            account_id: 0,
            mailbox_id: 0,
            message_id: None,
            in_reply_to: None,
            references: Vec::new(),
            from: Vec::new(),
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            reply_to: Vec::new(),
            subject: String::new(),
            date: 0,
            received_date: 0,
            body_text: None,
            body_html: None,
            attachments: Vec::new(),
            flags: MessageFlags::new(),
            priority: MessagePriority::Normal,
            size: 0,
            thread_id: None,
            labels: Vec::new(),
        }
    }

    pub fn is_unread(&self) -> bool {
        !self.flags.seen
    }

    pub fn has_attachments(&self) -> bool {
        !self.attachments.is_empty()
    }

    pub fn from_display(&self) -> String {
        if let Some(first) = self.from.first() {
            first.display_name.clone().unwrap_or_else(|| first.address.clone())
        } else {
            "(Unknown)".to_string()
        }
    }

    pub fn to_display(&self) -> String {
        self.to.iter()
            .map(|a| a.display_name.clone().unwrap_or_else(|| a.address.clone()))
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn format_date(&self) -> String {
        let hours = (self.date / 3600) % 24;
        let minutes = (self.date / 60) % 60;
        let days = self.date / 86400;

        if days == 0 {
            format!("{:02}:{:02}", hours, minutes)
        } else if days < 7 {
            let weekdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
            weekdays[(days % 7) as usize].to_string()
        } else {
            let month = (days / 30) % 12 + 1;
            let day = (days % 30) + 1;
            format!("{}/{}", month, day)
        }
    }

    pub fn preview(&self, max_len: usize) -> String {
        let text = self.body_text.as_deref().unwrap_or("");
        let clean: String = text.chars()
            .filter(|c| !c.is_control())
            .take(max_len)
            .collect();
        if clean.len() < text.len() {
            format!("{}...", clean)
        } else {
            clean
        }
    }
}

/// Draft message for composing
#[derive(Debug, Clone)]
pub struct DraftMessage {
    pub id: Option<u64>,
    pub account_id: u64,
    pub to: String,
    pub cc: String,
    pub bcc: String,
    pub subject: String,
    pub body: String,
    pub is_html: bool,
    pub attachments: Vec<Attachment>,
    pub in_reply_to: Option<u64>,
    pub is_forward: bool,
    pub priority: MessagePriority,
    pub request_read_receipt: bool,
    pub created: u64,
    pub modified: u64,
}

impl DraftMessage {
    pub fn new(account_id: u64) -> Self {
        Self {
            id: None,
            account_id,
            to: String::new(),
            cc: String::new(),
            bcc: String::new(),
            subject: String::new(),
            body: String::new(),
            is_html: false,
            attachments: Vec::new(),
            in_reply_to: None,
            is_forward: false,
            priority: MessagePriority::Normal,
            request_read_receipt: false,
            created: 0,
            modified: 0,
        }
    }

    pub fn reply_to(message: &EmailMessage, account: &EmailAccount, reply_all: bool) -> Self {
        let mut draft = Self::new(account.id);
        draft.in_reply_to = Some(message.id);
        draft.subject = if message.subject.starts_with("Re:") {
            message.subject.clone()
        } else {
            format!("Re: {}", message.subject)
        };
        draft.to = message.reply_to.first()
            .or(message.from.first())
            .map(|a| a.address.clone())
            .unwrap_or_default();

        if reply_all {
            let others: Vec<_> = message.to.iter()
                .chain(message.cc.iter())
                .filter(|a| a.address != account.email)
                .map(|a| a.address.clone())
                .collect();
            draft.cc = others.join(", ");
        }

        // Quote original message
        let quoted = message.body_text.as_deref()
            .unwrap_or("")
            .lines()
            .map(|l| format!("> {}", l))
            .collect::<Vec<_>>()
            .join("\n");
        draft.body = format!("\n\nOn {}, {} wrote:\n{}",
            message.format_date(),
            message.from_display(),
            quoted
        );

        draft
    }

    pub fn forward(message: &EmailMessage, account: &EmailAccount) -> Self {
        let mut draft = Self::new(account.id);
        draft.is_forward = true;
        draft.subject = if message.subject.starts_with("Fwd:") {
            message.subject.clone()
        } else {
            format!("Fwd: {}", message.subject)
        };

        let body = message.body_text.as_deref().unwrap_or("");
        draft.body = format!(
            "\n\n---------- Forwarded message ----------\n\
             From: {}\n\
             Date: {}\n\
             Subject: {}\n\
             To: {}\n\n\
             {}",
            message.from_display(),
            message.format_date(),
            message.subject,
            message.to_display(),
            body
        );

        // Copy attachments
        draft.attachments = message.attachments.clone();

        draft
    }
}

/// Search filter for messages
#[derive(Debug, Clone)]
pub struct SearchFilter {
    pub query: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub subject: Option<String>,
    pub has_attachment: Option<bool>,
    pub is_unread: Option<bool>,
    pub is_flagged: Option<bool>,
    pub date_from: Option<u64>,
    pub date_to: Option<u64>,
    pub mailbox_id: Option<u64>,
    pub account_id: Option<u64>,
}

impl SearchFilter {
    pub fn new(query: &str) -> Self {
        Self {
            query: query.to_string(),
            from: None,
            to: None,
            subject: None,
            has_attachment: None,
            is_unread: None,
            is_flagged: None,
            date_from: None,
            date_to: None,
            mailbox_id: None,
            account_id: None,
        }
    }

    pub fn matches(&self, message: &EmailMessage) -> bool {
        let query_lower = self.query.to_lowercase();

        if !self.query.is_empty() {
            let subject_match = message.subject.to_lowercase().contains(&query_lower);
            let from_match = message.from.iter()
                .any(|a| a.address.to_lowercase().contains(&query_lower) ||
                     a.display_name.as_ref().map(|n| n.to_lowercase().contains(&query_lower)).unwrap_or(false));
            let body_match = message.body_text.as_ref()
                .map(|b| b.to_lowercase().contains(&query_lower))
                .unwrap_or(false);

            if !subject_match && !from_match && !body_match {
                return false;
            }
        }

        if let Some(ref from) = self.from {
            if !message.from.iter().any(|a| a.address.contains(from)) {
                return false;
            }
        }

        if let Some(ref to) = self.to {
            if !message.to.iter().any(|a| a.address.contains(to)) {
                return false;
            }
        }

        if let Some(ref subj) = self.subject {
            if !message.subject.to_lowercase().contains(&subj.to_lowercase()) {
                return false;
            }
        }

        if let Some(has_att) = self.has_attachment {
            if message.has_attachments() != has_att {
                return false;
            }
        }

        if let Some(unread) = self.is_unread {
            if message.is_unread() != unread {
                return false;
            }
        }

        if let Some(flagged) = self.is_flagged {
            if message.flags.flagged != flagged {
                return false;
            }
        }

        true
    }
}

/// Sort order for message list
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    DateDesc,
    DateAsc,
    FromAsc,
    FromDesc,
    SubjectAsc,
    SubjectDesc,
    SizeAsc,
    SizeDesc,
}

impl SortOrder {
    pub fn name(&self) -> &'static str {
        match self {
            SortOrder::DateDesc => "Newest First",
            SortOrder::DateAsc => "Oldest First",
            SortOrder::FromAsc => "From A-Z",
            SortOrder::FromDesc => "From Z-A",
            SortOrder::SubjectAsc => "Subject A-Z",
            SortOrder::SubjectDesc => "Subject Z-A",
            SortOrder::SizeAsc => "Smallest First",
            SortOrder::SizeDesc => "Largest First",
        }
    }
}

/// View mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    MessageList,
    MessageView,
    Compose,
    Settings,
    AccountSetup,
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Syncing,
    Error,
}

impl ConnectionState {
    pub fn name(&self) -> &'static str {
        match self {
            ConnectionState::Disconnected => "Disconnected",
            ConnectionState::Connecting => "Connecting...",
            ConnectionState::Connected => "Connected",
            ConnectionState::Syncing => "Syncing...",
            ConnectionState::Error => "Error",
        }
    }
}

/// Email error type
#[derive(Debug, Clone)]
pub enum EmailError {
    ConnectionFailed(String),
    AuthenticationFailed,
    NetworkError(String),
    ServerError(String),
    InvalidMessage,
    MailboxNotFound,
    MessageNotFound,
    SendFailed(String),
    AttachmentTooLarge,
    StorageError,
}

impl EmailError {
    pub fn message(&self) -> String {
        match self {
            EmailError::ConnectionFailed(s) => format!("Connection failed: {}", s),
            EmailError::AuthenticationFailed => "Authentication failed".to_string(),
            EmailError::NetworkError(s) => format!("Network error: {}", s),
            EmailError::ServerError(s) => format!("Server error: {}", s),
            EmailError::InvalidMessage => "Invalid message".to_string(),
            EmailError::MailboxNotFound => "Mailbox not found".to_string(),
            EmailError::MessageNotFound => "Message not found".to_string(),
            EmailError::SendFailed(s) => format!("Send failed: {}", s),
            EmailError::AttachmentTooLarge => "Attachment too large".to_string(),
            EmailError::StorageError => "Storage error".to_string(),
        }
    }
}

/// Email client widget
pub struct EmailClient {
    id: WidgetId,
    bounds: Bounds,
    enabled: bool,
    visible: bool,

    // Accounts
    accounts: Vec<EmailAccount>,
    current_account_id: Option<u64>,
    next_account_id: u64,

    // Mailboxes
    mailboxes: Vec<Mailbox>,
    current_mailbox_id: Option<u64>,
    next_mailbox_id: u64,

    // Messages
    messages: Vec<EmailMessage>,
    selected_message_id: Option<u64>,
    next_message_id: u64,

    // UI state
    view_mode: ViewMode,
    connection_state: ConnectionState,
    search_query: String,
    sort_order: SortOrder,
    scroll_offset: usize,
    sidebar_width: usize,
    preview_height: usize,

    // Compose
    draft: Option<DraftMessage>,
    compose_field: usize, // 0=to, 1=cc, 2=bcc, 3=subject, 4=body

    // Selection
    selected_indices: Vec<usize>,
    hovered_index: Option<usize>,

    // Error state
    last_error: Option<EmailError>,
}

impl EmailClient {
    pub fn new(id: WidgetId) -> Self {
        let mut client = Self {
            id,
            bounds: Bounds { x: 0, y: 0, width: 800, height: 600 },
            enabled: true,
            visible: true,
            accounts: Vec::new(),
            current_account_id: None,
            next_account_id: 1,
            mailboxes: Vec::new(),
            current_mailbox_id: None,
            next_mailbox_id: 1,
            messages: Vec::new(),
            selected_message_id: None,
            next_message_id: 1,
            view_mode: ViewMode::MessageList,
            connection_state: ConnectionState::Disconnected,
            search_query: String::new(),
            sort_order: SortOrder::DateDesc,
            scroll_offset: 0,
            sidebar_width: 200,
            preview_height: 200,
            draft: None,
            compose_field: 0,
            selected_indices: Vec::new(),
            hovered_index: None,
            last_error: None,
        };

        client.add_sample_data();
        client
    }

    fn add_sample_data(&mut self) {
        // Add sample account
        let mut account = EmailAccount::new("John Doe", "john.doe@example.com", EmailProtocol::Imap);
        account.id = self.next_account_id;
        account.incoming_server = "imap.example.com".to_string();
        account.smtp.server = "smtp.example.com".to_string();
        account.is_default = true;
        self.next_account_id += 1;
        let account_id = account.id;
        self.accounts.push(account);
        self.current_account_id = Some(account_id);

        // Add mailboxes
        let mailbox_types = [
            ("Inbox", MailboxType::Inbox),
            ("Sent", MailboxType::Sent),
            ("Drafts", MailboxType::Drafts),
            ("Trash", MailboxType::Trash),
            ("Spam", MailboxType::Spam),
            ("Archive", MailboxType::Archive),
        ];

        for (name, mtype) in mailbox_types.iter() {
            let mut mb = Mailbox::new(name, *mtype, account_id);
            mb.id = self.next_mailbox_id;
            self.next_mailbox_id += 1;
            self.mailboxes.push(mb);
        }

        // Set inbox as current
        if let Some(inbox) = self.mailboxes.iter().find(|m| m.mailbox_type == MailboxType::Inbox) {
            self.current_mailbox_id = Some(inbox.id);
        }

        // Add sample messages to inbox
        let inbox_id = self.mailboxes.iter()
            .find(|m| m.mailbox_type == MailboxType::Inbox)
            .map(|m| m.id)
            .unwrap_or(1);

        let sample_messages = [
            ("alice@example.com", "Alice Smith", "Meeting tomorrow", "Hi John,\n\nJust a reminder about our meeting tomorrow at 10 AM.\n\nBest,\nAlice", false),
            ("bob@company.com", "Bob Johnson", "Project Update", "Hello team,\n\nThe project is progressing well. We've completed phase 1.\n\nRegards,\nBob", true),
            ("newsletter@tech.com", "Tech Weekly", "This Week in Tech", "Top stories:\n1. New AI breakthrough\n2. Cloud computing trends\n3. Security updates", true),
            ("support@service.com", "Support Team", "Your ticket has been resolved", "Dear Customer,\n\nYour support ticket #12345 has been resolved.\n\nThank you for your patience.", true),
            ("jane@example.com", "Jane Wilson", "Re: Vacation plans", "That sounds great! Count me in for the trip.\n\nJane", false),
        ];

        for (i, (email, name, subject, body, read)) in sample_messages.iter().enumerate() {
            let mut msg = EmailMessage::new();
            msg.id = self.next_message_id;
            msg.uid = self.next_message_id;
            self.next_message_id += 1;
            msg.account_id = account_id;
            msg.mailbox_id = inbox_id;
            msg.from.push(EmailAddress::with_name(email, name));
            msg.to.push(EmailAddress::new("john.doe@example.com"));
            msg.subject = subject.to_string();
            msg.body_text = Some(body.to_string());
            msg.date = 1705600000 - (i as u64 * 3600);
            msg.flags.seen = *read;
            msg.size = body.len() as u64 + 500;

            // Add attachment to some messages
            if i == 1 {
                msg.attachments.push(Attachment::new("report.pdf", "application/pdf", 1500000));
            }

            self.messages.push(msg);
        }

        // Update unread count
        if let Some(inbox) = self.mailboxes.iter_mut().find(|m| m.id == inbox_id) {
            inbox.unread_count = self.messages.iter()
                .filter(|m| m.mailbox_id == inbox_id && !m.flags.seen)
                .count() as u32;
            inbox.total_count = self.messages.iter()
                .filter(|m| m.mailbox_id == inbox_id)
                .count() as u32;
        }

        self.connection_state = ConnectionState::Connected;
    }

    /// Add a new account
    pub fn add_account(&mut self, mut account: EmailAccount) {
        account.id = self.next_account_id;
        self.next_account_id += 1;

        if self.accounts.is_empty() {
            account.is_default = true;
            self.current_account_id = Some(account.id);
        }

        self.accounts.push(account);
    }

    /// Remove an account
    pub fn remove_account(&mut self, account_id: u64) {
        self.accounts.retain(|a| a.id != account_id);
        self.mailboxes.retain(|m| m.account_id != account_id);
        self.messages.retain(|m| m.account_id != account_id);

        if self.current_account_id == Some(account_id) {
            self.current_account_id = self.accounts.first().map(|a| a.id);
        }
    }

    /// Get current account
    pub fn current_account(&self) -> Option<&EmailAccount> {
        self.current_account_id.and_then(|id| self.accounts.iter().find(|a| a.id == id))
    }

    /// Select a mailbox
    pub fn select_mailbox(&mut self, mailbox_id: u64) {
        self.current_mailbox_id = Some(mailbox_id);
        self.selected_message_id = None;
        self.scroll_offset = 0;
        self.view_mode = ViewMode::MessageList;
    }

    /// Get current mailbox
    pub fn current_mailbox(&self) -> Option<&Mailbox> {
        self.current_mailbox_id.and_then(|id| self.mailboxes.iter().find(|m| m.id == id))
    }

    /// Get messages for current mailbox
    pub fn current_messages(&self) -> Vec<&EmailMessage> {
        let mailbox_id = self.current_mailbox_id;
        let mut msgs: Vec<_> = self.messages.iter()
            .filter(|m| mailbox_id.map(|id| m.mailbox_id == id).unwrap_or(false))
            .collect();

        // Apply search filter
        if !self.search_query.is_empty() {
            let filter = SearchFilter::new(&self.search_query);
            msgs.retain(|m| filter.matches(m));
        }

        // Sort
        match self.sort_order {
            SortOrder::DateDesc => msgs.sort_by(|a, b| b.date.cmp(&a.date)),
            SortOrder::DateAsc => msgs.sort_by(|a, b| a.date.cmp(&b.date)),
            SortOrder::FromAsc => msgs.sort_by(|a, b| a.from_display().cmp(&b.from_display())),
            SortOrder::FromDesc => msgs.sort_by(|a, b| b.from_display().cmp(&a.from_display())),
            SortOrder::SubjectAsc => msgs.sort_by(|a, b| a.subject.cmp(&b.subject)),
            SortOrder::SubjectDesc => msgs.sort_by(|a, b| b.subject.cmp(&a.subject)),
            SortOrder::SizeAsc => msgs.sort_by(|a, b| a.size.cmp(&b.size)),
            SortOrder::SizeDesc => msgs.sort_by(|a, b| b.size.cmp(&a.size)),
        }

        msgs
    }

    /// Get selected message
    pub fn selected_message(&self) -> Option<&EmailMessage> {
        self.selected_message_id.and_then(|id| self.messages.iter().find(|m| m.id == id))
    }

    /// Select a message
    pub fn select_message(&mut self, message_id: u64) {
        self.selected_message_id = Some(message_id);

        // Mark as read
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message_id) {
            if !msg.flags.seen {
                msg.flags.seen = true;
                // Update unread count
                if let Some(mb) = self.mailboxes.iter_mut().find(|m| m.id == msg.mailbox_id) {
                    if mb.unread_count > 0 {
                        mb.unread_count -= 1;
                    }
                }
            }
        }
    }

    /// Open message in full view
    pub fn open_message(&mut self, message_id: u64) {
        self.select_message(message_id);
        self.view_mode = ViewMode::MessageView;
    }

    /// Start composing new message
    pub fn compose_new(&mut self) {
        if let Some(account_id) = self.current_account_id {
            self.draft = Some(DraftMessage::new(account_id));
            self.compose_field = 0;
            self.view_mode = ViewMode::Compose;
        }
    }

    /// Reply to current message
    pub fn reply(&mut self, reply_all: bool) {
        if let (Some(msg), Some(account)) = (self.selected_message(), self.current_account()) {
            let msg_clone = msg.clone();
            let account_clone = account.clone();
            self.draft = Some(DraftMessage::reply_to(&msg_clone, &account_clone, reply_all));
            self.compose_field = 4; // Start in body
            self.view_mode = ViewMode::Compose;
        }
    }

    /// Forward current message
    pub fn forward(&mut self) {
        if let (Some(msg), Some(account)) = (self.selected_message(), self.current_account()) {
            let msg_clone = msg.clone();
            let account_clone = account.clone();
            self.draft = Some(DraftMessage::forward(&msg_clone, &account_clone));
            self.compose_field = 0; // Start at To field
            self.view_mode = ViewMode::Compose;
        }
    }

    /// Send current draft
    pub fn send(&mut self) -> Result<(), EmailError> {
        if self.draft.is_none() {
            return Err(EmailError::InvalidMessage);
        }

        // In a real implementation, this would send via SMTP
        // For now, we just clear the draft
        self.draft = None;
        self.view_mode = ViewMode::MessageList;
        Ok(())
    }

    /// Save draft
    pub fn save_draft(&mut self) {
        if let Some(ref draft) = self.draft {
            // Create message from draft and save to Drafts mailbox
            let drafts_id = self.mailboxes.iter()
                .find(|m| m.mailbox_type == MailboxType::Drafts &&
                      m.account_id == draft.account_id)
                .map(|m| m.id);

            if let Some(mailbox_id) = drafts_id {
                let mut msg = EmailMessage::new();
                msg.id = self.next_message_id;
                self.next_message_id += 1;
                msg.account_id = draft.account_id;
                msg.mailbox_id = mailbox_id;
                msg.subject = draft.subject.clone();
                msg.body_text = Some(draft.body.clone());
                msg.flags.draft = true;

                self.messages.push(msg);
            }
        }

        self.draft = None;
        self.view_mode = ViewMode::MessageList;
    }

    /// Delete selected messages
    pub fn delete_selected(&mut self) {
        let trash_id = self.current_account_id.and_then(|aid| {
            self.mailboxes.iter()
                .find(|m| m.mailbox_type == MailboxType::Trash && m.account_id == aid)
                .map(|m| m.id)
        });

        if let Some(trash_id) = trash_id {
            if let Some(msg_id) = self.selected_message_id {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.id == msg_id) {
                    if msg.mailbox_id == trash_id {
                        // Already in trash, delete permanently
                        self.messages.retain(|m| m.id != msg_id);
                    } else {
                        // Move to trash
                        msg.mailbox_id = trash_id;
                        msg.flags.deleted = true;
                    }
                }
                self.selected_message_id = None;
            }
        }
    }

    /// Toggle flag on selected message
    pub fn toggle_flag(&mut self) {
        if let Some(msg_id) = self.selected_message_id {
            if let Some(msg) = self.messages.iter_mut().find(|m| m.id == msg_id) {
                msg.flags.flagged = !msg.flags.flagged;
            }
        }
    }

    /// Mark selected as read/unread
    pub fn toggle_read(&mut self) {
        if let Some(msg_id) = self.selected_message_id {
            if let Some(msg) = self.messages.iter_mut().find(|m| m.id == msg_id) {
                msg.flags.seen = !msg.flags.seen;
            }
        }
    }

    /// Set search query
    pub fn set_search(&mut self, query: &str) {
        self.search_query = query.to_string();
        self.scroll_offset = 0;
    }

    /// Clear search
    pub fn clear_search(&mut self) {
        self.search_query.clear();
    }

    /// Set sort order
    pub fn set_sort_order(&mut self, order: SortOrder) {
        self.sort_order = order;
    }

    fn get_visible_count(&self) -> usize {
        let list_height = self.bounds.height.saturating_sub(60);
        list_height / 60
    }

    fn message_at_point(&self, x: isize, y: isize) -> Option<usize> {
        let list_x = self.bounds.x + self.sidebar_width as isize;
        let list_y = self.bounds.y + 40;
        let list_width = self.bounds.width.saturating_sub(self.sidebar_width);
        let list_height = self.bounds.height.saturating_sub(60);

        if x >= list_x && x < list_x + list_width as isize &&
           y >= list_y && y < list_y + list_height as isize {
            let row = ((y - list_y) / 60) as usize;
            let index = self.scroll_offset + row;
            let messages = self.current_messages();
            if index < messages.len() {
                return Some(index);
            }
        }
        None
    }

    fn mailbox_at_point(&self, x: isize, y: isize) -> Option<u64> {
        let sidebar_x = self.bounds.x;
        let sidebar_y = self.bounds.y + 60;

        if x >= sidebar_x && x < sidebar_x + self.sidebar_width as isize &&
           y >= sidebar_y {
            let row = ((y - sidebar_y) / 28) as usize;
            let account_mailboxes: Vec<_> = self.mailboxes.iter()
                .filter(|m| self.current_account_id.map(|id| m.account_id == id).unwrap_or(false))
                .collect();
            if row < account_mailboxes.len() {
                return Some(account_mailboxes[row].id);
            }
        }
        None
    }
}

fn draw_char_at(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;
    if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
        for row in 0..DEFAULT_FONT.height {
            let byte = glyph[row];
            for col in 0..DEFAULT_FONT.width {
                if (byte >> (7 - col)) & 1 != 0 {
                    surface.set_pixel(x + col, y + row, color);
                }
            }
        }
    }
}

fn draw_char(surface: &mut Surface, x: isize, y: isize, c: char, color: Color) {
    if x >= 0 && y >= 0 {
        draw_char_at(surface, x as usize, y as usize, c, color);
    }
}

fn draw_string(surface: &mut Surface, x: isize, y: isize, s: &str, color: Color) {
    if x < 0 || y < 0 {
        return;
    }
    let mut px = x as usize;
    for c in s.chars() {
        draw_char_at(surface, px, y as usize, c, color);
        px += 8;
    }
}

impl Widget for EmailClient {
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
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::MouseDown { x, y, button } => {
                if *button == MouseButton::Left {
                    // Check for mailbox click
                    if let Some(mb_id) = self.mailbox_at_point(*x, *y) {
                        self.select_mailbox(mb_id);
                        return true;
                    }

                    // Check for message click
                    if let Some(idx) = self.message_at_point(*x, *y) {
                        let messages = self.current_messages();
                        if idx < messages.len() {
                            let msg_id = messages[idx].id;
                            self.select_message(msg_id);
                            return true;
                        }
                    }

                    // Check toolbar buttons
                    let toolbar_y = self.bounds.y;
                    if *y >= toolbar_y && *y < toolbar_y + 40 {
                        let bx = self.bounds.x + self.sidebar_width as isize;
                        if *x >= bx && *x < bx + 80 {
                            self.compose_new();
                            return true;
                        } else if *x >= bx + 90 && *x < bx + 170 {
                            self.reply(false);
                            return true;
                        } else if *x >= bx + 180 && *x < bx + 260 {
                            self.delete_selected();
                            return true;
                        }
                    }
                }
                false
            }

            WidgetEvent::MouseMove { x, y } => {
                self.hovered_index = self.message_at_point(*x, *y);
                true
            }

            WidgetEvent::Scroll { delta_y, .. } => {
                let messages = self.current_messages();
                let visible = self.get_visible_count();
                let max_scroll = messages.len().saturating_sub(visible);

                if *delta_y < 0 && self.scroll_offset > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                } else if *delta_y > 0 && self.scroll_offset < max_scroll {
                    self.scroll_offset += 1;
                }
                true
            }

            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x48 => { // Up
                        let messages = self.current_messages();
                        if let Some(msg_id) = self.selected_message_id {
                            if let Some(pos) = messages.iter().position(|m| m.id == msg_id) {
                                if pos > 0 {
                                    self.select_message(messages[pos - 1].id);
                                    if pos.saturating_sub(1) < self.scroll_offset {
                                        self.scroll_offset = pos.saturating_sub(1);
                                    }
                                }
                            }
                        } else if !messages.is_empty() {
                            self.select_message(messages[0].id);
                        }
                        true
                    }
                    0x50 => { // Down
                        let messages = self.current_messages();
                        if let Some(msg_id) = self.selected_message_id {
                            if let Some(pos) = messages.iter().position(|m| m.id == msg_id) {
                                if pos + 1 < messages.len() {
                                    self.select_message(messages[pos + 1].id);
                                    let visible = self.get_visible_count();
                                    if pos + 1 >= self.scroll_offset + visible {
                                        self.scroll_offset = pos + 2 - visible;
                                    }
                                }
                            }
                        } else if !messages.is_empty() {
                            self.select_message(messages[0].id);
                        }
                        true
                    }
                    0x1C => { // Enter - open message
                        if let Some(msg_id) = self.selected_message_id {
                            self.open_message(msg_id);
                        }
                        true
                    }
                    0x53 | 0x7F => { // Delete
                        self.delete_selected();
                        true
                    }
                    0x1B => { // Escape
                        if self.view_mode != ViewMode::MessageList {
                            self.view_mode = ViewMode::MessageList;
                            return true;
                        }
                        false
                    }
                    _ => false,
                }
            }

            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        let _theme = theme();
        let bg_color = Color::new(30, 30, 35);
        let sidebar_bg = Color::new(25, 25, 30);
        let text_color = Color::new(220, 220, 220);
        let dim_text = Color::new(150, 150, 150);
        let accent_color = Color::new(66, 133, 244);
        let unread_color = Color::new(255, 255, 255);
        let hover_bg = Color::new(50, 50, 55);
        let selected_bg = Color::new(66, 133, 244);
        let border_color = Color::new(60, 60, 65);

        // Clear background
        for y in 0..self.bounds.height {
            for x in 0..self.bounds.width {
                let px = self.bounds.x + x as isize;
                let py = self.bounds.y + y as isize;
                surface.set_pixel(px as usize, py as usize, bg_color);
            }
        }

        // Draw sidebar
        for y in 0..self.bounds.height {
            for x in 0..self.sidebar_width {
                let px = self.bounds.x + x as isize;
                let py = self.bounds.y + y as isize;
                surface.set_pixel(px as usize, py as usize, sidebar_bg);
            }
        }

        // Draw sidebar border
        for y in 0..self.bounds.height {
            let px = self.bounds.x + self.sidebar_width as isize;
            let py = self.bounds.y + y as isize;
            surface.set_pixel(px as usize, py as usize, border_color);
        }

        // Draw account name in sidebar
        if let Some(account) = self.current_account() {
            draw_string(surface, self.bounds.x + 10, self.bounds.y + 15, &account.name, text_color);
            draw_string(surface, self.bounds.x + 10, self.bounds.y + 35, &account.email, dim_text);
        }

        // Draw mailbox list
        let mailbox_y = self.bounds.y + 60;
        let account_mailboxes: Vec<_> = self.mailboxes.iter()
            .filter(|m| self.current_account_id.map(|id| m.account_id == id).unwrap_or(false))
            .collect();

        for (i, mailbox) in account_mailboxes.iter().enumerate() {
            let y = mailbox_y + (i * 28) as isize;

            // Highlight current mailbox
            if self.current_mailbox_id == Some(mailbox.id) {
                for dx in 0..self.sidebar_width {
                    for dy in 0..28 {
                        surface.set_pixel(
                            (self.bounds.x + dx as isize) as usize,
                            (y + dy as isize) as usize,
                            selected_bg
                        );
                    }
                }
            }

            // Icon
            let icon_color = if self.current_mailbox_id == Some(mailbox.id) {
                unread_color
            } else {
                dim_text
            };
            draw_char(surface, self.bounds.x + 10, y + 6, mailbox.mailbox_type.icon(), icon_color);

            // Name
            let name_color = if self.current_mailbox_id == Some(mailbox.id) {
                unread_color
            } else {
                text_color
            };
            draw_string(surface, self.bounds.x + 26, y + 6, &mailbox.name, name_color);

            // Unread count
            if mailbox.unread_count > 0 {
                let count_str = format!("{}", mailbox.unread_count);
                let count_x = self.bounds.x + (self.sidebar_width - 30) as isize;
                draw_string(surface, count_x, y + 6, &count_str, accent_color);
            }
        }

        // Draw toolbar
        let toolbar_y = self.bounds.y;
        let toolbar_x = self.bounds.x + self.sidebar_width as isize + 10;

        // Compose button
        draw_string(surface, toolbar_x, toolbar_y + 12, "[Compose]", accent_color);
        draw_string(surface, toolbar_x + 90, toolbar_y + 12, "[Reply]", text_color);
        draw_string(surface, toolbar_x + 180, toolbar_y + 12, "[Delete]", text_color);

        // Connection status
        let status_x = self.bounds.x + self.bounds.width as isize - 120;
        let status_color = match self.connection_state {
            ConnectionState::Connected => Color::new(100, 200, 100),
            ConnectionState::Syncing => accent_color,
            ConnectionState::Error => Color::new(200, 100, 100),
            _ => dim_text,
        };
        draw_string(surface, status_x, toolbar_y + 12, self.connection_state.name(), status_color);

        // Draw toolbar separator
        for x in self.sidebar_width..self.bounds.width {
            let px = self.bounds.x + x as isize;
            let py = self.bounds.y + 39;
            surface.set_pixel(px as usize, py as usize, border_color);
        }

        match self.view_mode {
            ViewMode::MessageList => self.render_message_list(surface),
            ViewMode::MessageView => self.render_message_view(surface),
            ViewMode::Compose => self.render_compose(surface),
            _ => {}
        }
    }
}

impl EmailClient {
    fn render_message_list(&self, surface: &mut Surface) {
        let text_color = Color::new(220, 220, 220);
        let dim_text = Color::new(150, 150, 150);
        let unread_color = Color::new(255, 255, 255);
        let accent_color = Color::new(66, 133, 244);
        let hover_bg = Color::new(50, 50, 55);
        let selected_bg = Color::new(45, 85, 150);
        let border_color = Color::new(60, 60, 65);
        let flag_color = Color::new(255, 180, 0);

        let list_x = self.bounds.x + self.sidebar_width as isize + 1;
        let list_y = self.bounds.y + 40;
        let list_width = self.bounds.width.saturating_sub(self.sidebar_width + 1);
        let list_height = self.bounds.height.saturating_sub(60);

        let messages = self.current_messages();
        let visible_count = self.get_visible_count();
        let row_height = 60;

        for (i, message) in messages.iter()
            .skip(self.scroll_offset)
            .take(visible_count)
            .enumerate()
        {
            let row_y = list_y + (i * row_height) as isize;
            let actual_index = self.scroll_offset + i;

            // Row background
            let bg = if self.selected_message_id == Some(message.id) {
                selected_bg
            } else if self.hovered_index == Some(actual_index) {
                hover_bg
            } else {
                Color::new(30, 30, 35)
            };

            for y in 0..row_height {
                for x in 0..list_width {
                    surface.set_pixel(
                        (list_x + x as isize) as usize,
                        (row_y + y as isize) as usize,
                        bg
                    );
                }
            }

            // Unread indicator
            if !message.flags.seen {
                for y in 10..20 {
                    for x in 5..9 {
                        surface.set_pixel(
                            (list_x + x as isize) as usize,
                            (row_y + y as isize) as usize,
                            accent_color
                        );
                    }
                }
            }

            // Flag indicator
            if message.flags.flagged {
                draw_char(surface, list_x + 15, row_y + 8, '*', flag_color);
            }

            // Attachment indicator
            if message.has_attachments() {
                draw_char(surface, list_x + 28, row_y + 8, '@', dim_text);
            }

            // From
            let from_color = if message.is_unread() { unread_color } else { text_color };
            let from = message.from_display();
            let from_truncated: String = from.chars().take(25).collect();
            draw_string(surface, list_x + 45, row_y + 8, &from_truncated, from_color);

            // Subject
            let subject_color = if message.is_unread() { unread_color } else { text_color };
            let subject_truncated: String = message.subject.chars().take(60).collect();
            draw_string(surface, list_x + 45, row_y + 24, &subject_truncated, subject_color);

            // Preview
            let preview = message.preview(80);
            draw_string(surface, list_x + 45, row_y + 40, &preview, dim_text);

            // Date
            let date_x = list_x + list_width as isize - 70;
            draw_string(surface, date_x, row_y + 8, &message.format_date(), dim_text);

            // Row separator
            for x in 0..list_width {
                surface.set_pixel(
                    (list_x + x as isize) as usize,
                    (row_y + row_height as isize - 1) as usize,
                    border_color
                );
            }
        }

        // Scrollbar
        if messages.len() > visible_count {
            let scrollbar_x = list_x + list_width as isize - 6;
            let scrollbar_height = list_height.saturating_sub(10);
            let thumb_height = (scrollbar_height * visible_count / messages.len()).max(20);
            let thumb_pos = scrollbar_height * self.scroll_offset / messages.len().max(1);

            // Track
            for y in 0..scrollbar_height {
                surface.set_pixel(
                    scrollbar_x as usize,
                    (list_y + 5 + y as isize) as usize,
                    Color::new(50, 50, 55)
                );
            }

            // Thumb
            for y in 0..thumb_height {
                surface.set_pixel(
                    scrollbar_x as usize,
                    (list_y + 5 + thumb_pos as isize + y as isize) as usize,
                    Color::new(100, 100, 105)
                );
            }
        }

        // Empty state
        if messages.is_empty() {
            let empty_msg = if self.search_query.is_empty() {
                "No messages"
            } else {
                "No messages match your search"
            };
            let center_x = list_x + (list_width / 2) as isize - (empty_msg.len() * 4) as isize;
            let center_y = list_y + (list_height / 2) as isize;
            draw_string(surface, center_x, center_y, empty_msg, dim_text);
        }
    }

    fn render_message_view(&self, surface: &mut Surface) {
        let text_color = Color::new(220, 220, 220);
        let dim_text = Color::new(150, 150, 150);
        let accent_color = Color::new(66, 133, 244);
        let border_color = Color::new(60, 60, 65);

        let view_x = self.bounds.x + self.sidebar_width as isize + 10;
        let view_y = self.bounds.y + 50;
        let view_width = self.bounds.width.saturating_sub(self.sidebar_width + 20);

        if let Some(message) = self.selected_message() {
            // Back button hint
            draw_string(surface, view_x, view_y, "[ESC] Back to list", dim_text);

            // From
            let y = view_y + 30;
            draw_string(surface, view_x, y, "From:", dim_text);
            draw_string(surface, view_x + 50, y, &message.from_display(), text_color);

            // To
            let y = y + 20;
            draw_string(surface, view_x, y, "To:", dim_text);
            draw_string(surface, view_x + 50, y, &message.to_display(), text_color);

            // Subject
            let y = y + 20;
            draw_string(surface, view_x, y, "Subject:", dim_text);
            draw_string(surface, view_x + 70, y, &message.subject, text_color);

            // Date
            let y = y + 20;
            draw_string(surface, view_x, y, "Date:", dim_text);
            draw_string(surface, view_x + 50, y, &message.format_date(), text_color);

            // Attachments
            let y = y + 20;
            if !message.attachments.is_empty() {
                draw_string(surface, view_x, y, "Attachments:", dim_text);
                for (i, att) in message.attachments.iter().enumerate() {
                    let att_str = format!("{} ({})", att.filename, att.format_size());
                    draw_string(surface, view_x + 100 + (i * 150) as isize, y, &att_str, accent_color);
                }
            }

            // Header separator
            let y = y + 30;
            for x in 0..view_width {
                surface.set_pixel(
                    (view_x + x as isize) as usize,
                    y as usize,
                    border_color
                );
            }

            // Body
            let body_y = y + 20;
            if let Some(ref body) = message.body_text {
                for (i, line) in body.lines().enumerate() {
                    let line_y = body_y + (i * 16) as isize;
                    if line_y > self.bounds.y + self.bounds.height as isize - 20 {
                        break;
                    }
                    let display_line: String = line.chars().take((view_width / 8).max(40)).collect();
                    draw_string(surface, view_x, line_y, &display_line, text_color);
                }
            }

            // Action buttons at bottom
            let btn_y = self.bounds.y + self.bounds.height as isize - 30;
            draw_string(surface, view_x, btn_y, "[Reply]", accent_color);
            draw_string(surface, view_x + 80, btn_y, "[Reply All]", text_color);
            draw_string(surface, view_x + 180, btn_y, "[Forward]", text_color);
            draw_string(surface, view_x + 280, btn_y, "[Delete]", text_color);
        }
    }

    fn render_compose(&self, surface: &mut Surface) {
        let text_color = Color::new(220, 220, 220);
        let dim_text = Color::new(150, 150, 150);
        let accent_color = Color::new(66, 133, 244);
        let border_color = Color::new(60, 60, 65);
        let field_bg = Color::new(40, 40, 45);

        let compose_x = self.bounds.x + self.sidebar_width as isize + 10;
        let compose_y = self.bounds.y + 50;
        let compose_width = self.bounds.width.saturating_sub(self.sidebar_width + 20);

        draw_string(surface, compose_x, compose_y, "New Message", text_color);
        draw_string(surface, compose_x + 120, compose_y, "[ESC] Discard", dim_text);

        if let Some(ref draft) = self.draft {
            let fields = [
                ("To:", &draft.to),
                ("Cc:", &draft.cc),
                ("Bcc:", &draft.bcc),
                ("Subject:", &draft.subject),
            ];

            let mut y = compose_y + 30;
            for (i, (label, value)) in fields.iter().enumerate() {
                // Label
                draw_string(surface, compose_x, y, label, dim_text);

                // Field background
                let field_x = compose_x + 70;
                let field_width = compose_width - 80;
                for fy in 0..20 {
                    for fx in 0..field_width {
                        surface.set_pixel(
                            (field_x + fx as isize) as usize,
                            (y + fy as isize) as usize,
                            if self.compose_field == i { Color::new(50, 50, 55) } else { field_bg }
                        );
                    }
                }

                // Value
                let display_value: String = value.chars().take((field_width / 8).max(20)).collect();
                draw_string(surface, field_x + 5, y + 4, &display_value, text_color);

                // Cursor
                if self.compose_field == i {
                    let cursor_x = field_x + 5 + (value.len() * 8) as isize;
                    draw_char(surface, cursor_x, y + 4, '|', accent_color);
                }

                y += 28;
            }

            // Body area
            y += 10;
            draw_string(surface, compose_x, y, "Message:", dim_text);
            y += 20;

            let body_height = self.bounds.height.saturating_sub((y - self.bounds.y) as usize + 50);

            // Body background
            for by in 0..body_height {
                for bx in 0..compose_width {
                    surface.set_pixel(
                        (compose_x + bx as isize) as usize,
                        (y + by as isize) as usize,
                        if self.compose_field == 4 { Color::new(50, 50, 55) } else { field_bg }
                    );
                }
            }

            // Body text
            for (i, line) in draft.body.lines().enumerate() {
                let line_y = y + 5 + (i * 16) as isize;
                if line_y > y + body_height as isize - 20 {
                    break;
                }
                let display_line: String = line.chars().take((compose_width / 8).max(40)).collect();
                draw_string(surface, compose_x + 5, line_y, &display_line, text_color);
            }

            // Send button
            let btn_y = self.bounds.y + self.bounds.height as isize - 30;
            draw_string(surface, compose_x, btn_y, "[Send]", accent_color);
            draw_string(surface, compose_x + 70, btn_y, "[Save Draft]", text_color);
            draw_string(surface, compose_x + 180, btn_y, "[Attach]", text_color);
        }
    }
}

/// Initialize email client module
pub fn init() {
    // Initialization code
}
