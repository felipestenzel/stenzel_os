//! Contacts Application
//!
//! Contact management with vCard support, groups, and search.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton, theme};

/// Phone number type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhoneType {
    Mobile,
    Home,
    Work,
    Fax,
    Pager,
    Other,
}

impl PhoneType {
    pub fn name(&self) -> &'static str {
        match self {
            PhoneType::Mobile => "Mobile",
            PhoneType::Home => "Home",
            PhoneType::Work => "Work",
            PhoneType::Fax => "Fax",
            PhoneType::Pager => "Pager",
            PhoneType::Other => "Other",
        }
    }

    pub fn icon(&self) -> char {
        match self {
            PhoneType::Mobile => 'M',
            PhoneType::Home => 'H',
            PhoneType::Work => 'W',
            PhoneType::Fax => 'F',
            PhoneType::Pager => 'P',
            PhoneType::Other => 'O',
        }
    }
}

/// Phone number
#[derive(Debug, Clone)]
pub struct PhoneNumber {
    pub number: String,
    pub phone_type: PhoneType,
    pub is_primary: bool,
}

impl PhoneNumber {
    pub fn new(number: &str, phone_type: PhoneType) -> Self {
        Self {
            number: number.to_string(),
            phone_type,
            is_primary: false,
        }
    }

    pub fn format(&self) -> String {
        format!("{}: {}", self.phone_type.name(), self.number)
    }
}

/// Email type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailType {
    Personal,
    Work,
    Other,
}

impl EmailType {
    pub fn name(&self) -> &'static str {
        match self {
            EmailType::Personal => "Personal",
            EmailType::Work => "Work",
            EmailType::Other => "Other",
        }
    }
}

/// Email address
#[derive(Debug, Clone)]
pub struct Email {
    pub address: String,
    pub email_type: EmailType,
    pub is_primary: bool,
}

impl Email {
    pub fn new(address: &str, email_type: EmailType) -> Self {
        Self {
            address: address.to_string(),
            email_type,
            is_primary: false,
        }
    }

    pub fn format(&self) -> String {
        format!("{}: {}", self.email_type.name(), self.address)
    }
}

/// Address type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressType {
    Home,
    Work,
    Other,
}

impl AddressType {
    pub fn name(&self) -> &'static str {
        match self {
            AddressType::Home => "Home",
            AddressType::Work => "Work",
            AddressType::Other => "Other",
        }
    }
}

/// Physical address
#[derive(Debug, Clone)]
pub struct Address {
    pub street: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub country: String,
    pub address_type: AddressType,
    pub is_primary: bool,
}

impl Address {
    pub fn new(address_type: AddressType) -> Self {
        Self {
            street: String::new(),
            city: String::new(),
            state: String::new(),
            postal_code: String::new(),
            country: String::new(),
            address_type,
            is_primary: false,
        }
    }

    pub fn format_single_line(&self) -> String {
        let parts: Vec<&str> = [
            self.street.as_str(),
            self.city.as_str(),
            self.state.as_str(),
            self.postal_code.as_str(),
            self.country.as_str(),
        ].iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect();
        parts.join(", ")
    }

    pub fn is_empty(&self) -> bool {
        self.street.is_empty() &&
        self.city.is_empty() &&
        self.state.is_empty() &&
        self.postal_code.is_empty() &&
        self.country.is_empty()
    }
}

/// Social media profile
#[derive(Debug, Clone)]
pub struct SocialProfile {
    pub platform: String,
    pub username: String,
    pub url: Option<String>,
}

impl SocialProfile {
    pub fn new(platform: &str, username: &str) -> Self {
        Self {
            platform: platform.to_string(),
            username: username.to_string(),
            url: None,
        }
    }
}

/// Important date
#[derive(Debug, Clone)]
pub struct ImportantDate {
    pub label: String,
    pub month: u8,
    pub day: u8,
    pub year: Option<u16>,
}

impl ImportantDate {
    pub fn birthday(month: u8, day: u8, year: Option<u16>) -> Self {
        Self {
            label: "Birthday".to_string(),
            month,
            day,
            year,
        }
    }

    pub fn anniversary(month: u8, day: u8, year: Option<u16>) -> Self {
        Self {
            label: "Anniversary".to_string(),
            month,
            day,
            year,
        }
    }

    pub fn format(&self) -> String {
        if let Some(year) = self.year {
            format!("{}/{}/{}", self.month, self.day, year)
        } else {
            format!("{}/{}", self.month, self.day)
        }
    }
}

/// Contact record
#[derive(Debug, Clone)]
pub struct Contact {
    pub id: u64,
    pub prefix: Option<String>,
    pub first_name: String,
    pub middle_name: Option<String>,
    pub last_name: String,
    pub suffix: Option<String>,
    pub nickname: Option<String>,
    pub company: Option<String>,
    pub job_title: Option<String>,
    pub department: Option<String>,
    pub phones: Vec<PhoneNumber>,
    pub emails: Vec<Email>,
    pub addresses: Vec<Address>,
    pub website: Option<String>,
    pub social_profiles: Vec<SocialProfile>,
    pub dates: Vec<ImportantDate>,
    pub notes: String,
    pub photo: Option<Vec<u8>>,
    pub groups: Vec<u64>,
    pub is_favorite: bool,
    pub created: u64,
    pub modified: u64,
}

impl Contact {
    pub fn new(first_name: &str, last_name: &str) -> Self {
        Self {
            id: 0,
            prefix: None,
            first_name: first_name.to_string(),
            middle_name: None,
            last_name: last_name.to_string(),
            suffix: None,
            nickname: None,
            company: None,
            job_title: None,
            department: None,
            phones: Vec::new(),
            emails: Vec::new(),
            addresses: Vec::new(),
            website: None,
            social_profiles: Vec::new(),
            dates: Vec::new(),
            notes: String::new(),
            photo: None,
            groups: Vec::new(),
            is_favorite: false,
            created: 0,
            modified: 0,
        }
    }

    pub fn display_name(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref prefix) = self.prefix {
            parts.push(prefix.as_str());
        }
        if !self.first_name.is_empty() {
            parts.push(&self.first_name);
        }
        if let Some(ref middle) = self.middle_name {
            parts.push(middle.as_str());
        }
        if !self.last_name.is_empty() {
            parts.push(&self.last_name);
        }
        if let Some(ref suffix) = self.suffix {
            parts.push(suffix.as_str());
        }
        parts.join(" ")
    }

    pub fn initials(&self) -> String {
        let first = self.first_name.chars().next().unwrap_or(' ');
        let last = self.last_name.chars().next().unwrap_or(' ');
        format!("{}{}", first.to_uppercase(), last.to_uppercase())
    }

    pub fn sort_name(&self) -> String {
        if self.last_name.is_empty() {
            self.first_name.to_lowercase()
        } else {
            format!("{} {}", self.last_name.to_lowercase(), self.first_name.to_lowercase())
        }
    }

    pub fn primary_phone(&self) -> Option<&PhoneNumber> {
        self.phones.iter().find(|p| p.is_primary).or(self.phones.first())
    }

    pub fn primary_email(&self) -> Option<&Email> {
        self.emails.iter().find(|e| e.is_primary).or(self.emails.first())
    }

    pub fn primary_address(&self) -> Option<&Address> {
        self.addresses.iter().find(|a| a.is_primary).or(self.addresses.first())
    }

    pub fn birthday(&self) -> Option<&ImportantDate> {
        self.dates.iter().find(|d| d.label == "Birthday")
    }

    pub fn matches_search(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.first_name.to_lowercase().contains(&q) ||
        self.last_name.to_lowercase().contains(&q) ||
        self.nickname.as_ref().map(|n| n.to_lowercase().contains(&q)).unwrap_or(false) ||
        self.company.as_ref().map(|c| c.to_lowercase().contains(&q)).unwrap_or(false) ||
        self.phones.iter().any(|p| p.number.contains(&q)) ||
        self.emails.iter().any(|e| e.address.to_lowercase().contains(&q))
    }

    /// Export to vCard format
    pub fn to_vcard(&self) -> String {
        let mut vcard = String::new();
        vcard.push_str("BEGIN:VCARD\n");
        vcard.push_str("VERSION:3.0\n");
        vcard.push_str(&format!("N:{};{};;;\n", self.last_name, self.first_name));
        vcard.push_str(&format!("FN:{}\n", self.display_name()));

        if let Some(ref company) = self.company {
            vcard.push_str(&format!("ORG:{}\n", company));
        }
        if let Some(ref title) = self.job_title {
            vcard.push_str(&format!("TITLE:{}\n", title));
        }

        for phone in &self.phones {
            let ptype = match phone.phone_type {
                PhoneType::Mobile => "CELL",
                PhoneType::Home => "HOME",
                PhoneType::Work => "WORK",
                PhoneType::Fax => "FAX",
                _ => "VOICE",
            };
            vcard.push_str(&format!("TEL;TYPE={}:{}\n", ptype, phone.number));
        }

        for email in &self.emails {
            let etype = match email.email_type {
                EmailType::Personal => "HOME",
                EmailType::Work => "WORK",
                _ => "OTHER",
            };
            vcard.push_str(&format!("EMAIL;TYPE={}:{}\n", etype, email.address));
        }

        if !self.notes.is_empty() {
            vcard.push_str(&format!("NOTE:{}\n", self.notes.replace('\n', "\\n")));
        }

        vcard.push_str("END:VCARD\n");
        vcard
    }
}

/// Contact group
#[derive(Debug, Clone)]
pub struct ContactGroup {
    pub id: u64,
    pub name: String,
    pub color: Color,
    pub member_count: usize,
}

impl ContactGroup {
    pub fn new(name: &str) -> Self {
        Self {
            id: 0,
            name: name.to_string(),
            color: Color::new(100, 100, 100),
            member_count: 0,
        }
    }
}

/// Sort order for contacts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    FirstNameAsc,
    FirstNameDesc,
    LastNameAsc,
    LastNameDesc,
    CompanyAsc,
    CompanyDesc,
    RecentFirst,
}

impl SortOrder {
    pub fn name(&self) -> &'static str {
        match self {
            SortOrder::FirstNameAsc => "First Name A-Z",
            SortOrder::FirstNameDesc => "First Name Z-A",
            SortOrder::LastNameAsc => "Last Name A-Z",
            SortOrder::LastNameDesc => "Last Name Z-A",
            SortOrder::CompanyAsc => "Company A-Z",
            SortOrder::CompanyDesc => "Company Z-A",
            SortOrder::RecentFirst => "Recently Added",
        }
    }
}

/// View mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Detail,
    Edit,
    Create,
}

/// Filter type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    All,
    Favorites,
    Group(u64),
    Recent,
}

/// Contacts manager widget
pub struct ContactsManager {
    id: WidgetId,
    bounds: Bounds,
    enabled: bool,
    visible: bool,

    // Data
    contacts: Vec<Contact>,
    groups: Vec<ContactGroup>,
    next_contact_id: u64,
    next_group_id: u64,

    // View state
    view_mode: ViewMode,
    selected_contact_id: Option<u64>,
    filter: FilterType,
    search_query: String,
    sort_order: SortOrder,

    // UI state
    sidebar_width: usize,
    scroll_offset: usize,
    hovered_index: Option<usize>,
    editing_contact: Option<Contact>,
    edit_field: usize,
}

impl ContactsManager {
    pub fn new(id: WidgetId) -> Self {
        let mut manager = Self {
            id,
            bounds: Bounds { x: 0, y: 0, width: 800, height: 600 },
            enabled: true,
            visible: true,
            contacts: Vec::new(),
            groups: Vec::new(),
            next_contact_id: 1,
            next_group_id: 1,
            view_mode: ViewMode::List,
            selected_contact_id: None,
            filter: FilterType::All,
            search_query: String::new(),
            sort_order: SortOrder::LastNameAsc,
            sidebar_width: 200,
            scroll_offset: 0,
            hovered_index: None,
            editing_contact: None,
            edit_field: 0,
        };

        manager.add_sample_data();
        manager
    }

    fn add_sample_data(&mut self) {
        // Add groups
        let groups = ["Family", "Friends", "Work", "Favorites"];
        for name in groups {
            let mut group = ContactGroup::new(name);
            group.id = self.next_group_id;
            self.next_group_id += 1;
            self.groups.push(group);
        }

        // Add sample contacts
        let sample_contacts = [
            ("John", "Doe", "Acme Corp", "john.doe@example.com", "555-1234", true),
            ("Jane", "Smith", "Tech Inc", "jane.smith@tech.com", "555-5678", false),
            ("Bob", "Johnson", "Startup LLC", "bob@startup.io", "555-9012", false),
            ("Alice", "Williams", "Big Corp", "alice.w@bigcorp.com", "555-3456", true),
            ("Charlie", "Brown", "Small Biz", "charlie@smallbiz.net", "555-7890", false),
            ("Diana", "Miller", "Creative Co", "diana@creative.co", "555-2345", false),
            ("Edward", "Davis", "Finance Ltd", "edward.d@finance.com", "555-6789", true),
            ("Fiona", "Wilson", "Health Inc", "fiona.w@health.org", "555-0123", false),
        ];

        for (first, last, company, email, phone, is_fav) in sample_contacts {
            let mut contact = Contact::new(first, last);
            contact.id = self.next_contact_id;
            self.next_contact_id += 1;
            contact.company = Some(company.to_string());

            let mut email_addr = Email::new(email, EmailType::Work);
            email_addr.is_primary = true;
            contact.emails.push(email_addr);

            let mut phone_num = PhoneNumber::new(phone, PhoneType::Mobile);
            phone_num.is_primary = true;
            contact.phones.push(phone_num);

            contact.is_favorite = is_fav;

            self.contacts.push(contact);
        }

        // Update group counts
        self.update_group_counts();
    }

    fn update_group_counts(&mut self) {
        for group in &mut self.groups {
            group.member_count = self.contacts.iter()
                .filter(|c| c.groups.contains(&group.id))
                .count();
        }
    }

    /// Add a new contact
    pub fn add_contact(&mut self, mut contact: Contact) {
        contact.id = self.next_contact_id;
        self.next_contact_id += 1;
        self.contacts.push(contact);
        self.update_group_counts();
    }

    /// Remove a contact
    pub fn remove_contact(&mut self, contact_id: u64) {
        self.contacts.retain(|c| c.id != contact_id);
        if self.selected_contact_id == Some(contact_id) {
            self.selected_contact_id = None;
        }
        self.update_group_counts();
    }

    /// Update a contact
    pub fn update_contact(&mut self, contact: Contact) {
        if let Some(existing) = self.contacts.iter_mut().find(|c| c.id == contact.id) {
            *existing = contact;
        }
        self.update_group_counts();
    }

    /// Add a new group
    pub fn add_group(&mut self, mut group: ContactGroup) {
        group.id = self.next_group_id;
        self.next_group_id += 1;
        self.groups.push(group);
    }

    /// Remove a group
    pub fn remove_group(&mut self, group_id: u64) {
        self.groups.retain(|g| g.id != group_id);
        for contact in &mut self.contacts {
            contact.groups.retain(|gid| *gid != group_id);
        }
    }

    /// Get filtered and sorted contacts
    pub fn filtered_contacts(&self) -> Vec<&Contact> {
        let mut contacts: Vec<_> = self.contacts.iter()
            .filter(|c| {
                // Apply search filter
                if !self.search_query.is_empty() && !c.matches_search(&self.search_query) {
                    return false;
                }

                // Apply type filter
                match self.filter {
                    FilterType::All => true,
                    FilterType::Favorites => c.is_favorite,
                    FilterType::Group(gid) => c.groups.contains(&gid),
                    FilterType::Recent => true, // Would check created date
                }
            })
            .collect();

        // Sort
        match self.sort_order {
            SortOrder::FirstNameAsc => contacts.sort_by(|a, b| a.first_name.cmp(&b.first_name)),
            SortOrder::FirstNameDesc => contacts.sort_by(|a, b| b.first_name.cmp(&a.first_name)),
            SortOrder::LastNameAsc => contacts.sort_by(|a, b| a.last_name.cmp(&b.last_name)),
            SortOrder::LastNameDesc => contacts.sort_by(|a, b| b.last_name.cmp(&a.last_name)),
            SortOrder::CompanyAsc => contacts.sort_by(|a, b| a.company.cmp(&b.company)),
            SortOrder::CompanyDesc => contacts.sort_by(|a, b| b.company.cmp(&a.company)),
            SortOrder::RecentFirst => contacts.sort_by(|a, b| b.created.cmp(&a.created)),
        }

        contacts
    }

    /// Get selected contact
    pub fn selected_contact(&self) -> Option<&Contact> {
        self.selected_contact_id.and_then(|id| self.contacts.iter().find(|c| c.id == id))
    }

    /// Select a contact
    pub fn select_contact(&mut self, contact_id: u64) {
        self.selected_contact_id = Some(contact_id);
        self.view_mode = ViewMode::Detail;
    }

    /// Start creating a new contact
    pub fn create_contact(&mut self) {
        self.editing_contact = Some(Contact::new("", ""));
        self.view_mode = ViewMode::Create;
        self.edit_field = 0;
    }

    /// Start editing selected contact
    pub fn edit_selected(&mut self) {
        if let Some(contact) = self.selected_contact().cloned() {
            self.editing_contact = Some(contact);
            self.view_mode = ViewMode::Edit;
            self.edit_field = 0;
        }
    }

    /// Save editing contact
    pub fn save_editing(&mut self) {
        if let Some(contact) = self.editing_contact.take() {
            if contact.id == 0 {
                self.add_contact(contact);
            } else {
                self.update_contact(contact);
            }
        }
        self.view_mode = ViewMode::List;
    }

    /// Cancel editing
    pub fn cancel_editing(&mut self) {
        self.editing_contact = None;
        self.view_mode = if self.selected_contact_id.is_some() {
            ViewMode::Detail
        } else {
            ViewMode::List
        };
    }

    /// Delete selected contact
    pub fn delete_selected(&mut self) {
        if let Some(id) = self.selected_contact_id {
            self.remove_contact(id);
            self.view_mode = ViewMode::List;
        }
    }

    /// Toggle favorite status
    pub fn toggle_favorite(&mut self) {
        if let Some(id) = self.selected_contact_id {
            if let Some(contact) = self.contacts.iter_mut().find(|c| c.id == id) {
                contact.is_favorite = !contact.is_favorite;
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

    /// Set filter
    pub fn set_filter(&mut self, filter: FilterType) {
        self.filter = filter;
        self.scroll_offset = 0;
    }

    /// Set sort order
    pub fn set_sort_order(&mut self, order: SortOrder) {
        self.sort_order = order;
    }

    fn get_visible_count(&self) -> usize {
        let list_height = self.bounds.height.saturating_sub(60);
        list_height / 50
    }

    fn contact_at_point(&self, x: isize, y: isize) -> Option<usize> {
        let list_x = self.bounds.x + self.sidebar_width as isize;
        let list_y = self.bounds.y + 50;
        let list_width = self.bounds.width.saturating_sub(self.sidebar_width);
        let list_height = self.bounds.height.saturating_sub(70);

        if x >= list_x && x < list_x + list_width as isize &&
           y >= list_y && y < list_y + list_height as isize {
            let row = ((y - list_y) / 50) as usize;
            let index = self.scroll_offset + row;
            let contacts = self.filtered_contacts();
            if index < contacts.len() {
                return Some(index);
            }
        }
        None
    }

    fn group_at_point(&self, x: isize, y: isize) -> Option<FilterType> {
        let sidebar_x = self.bounds.x;
        let sidebar_y = self.bounds.y + 50;

        if x >= sidebar_x && x < sidebar_x + self.sidebar_width as isize {
            let row = ((y - sidebar_y) / 28) as usize;
            match row {
                0 => return Some(FilterType::All),
                1 => return Some(FilterType::Favorites),
                _ => {
                    let group_idx = row - 3;
                    if group_idx < self.groups.len() {
                        return Some(FilterType::Group(self.groups[group_idx].id));
                    }
                }
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

impl Widget for ContactsManager {
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
                    // Check for group/filter click in sidebar
                    if let Some(filter) = self.group_at_point(*x, *y) {
                        self.set_filter(filter);
                        return true;
                    }

                    // Check for contact click
                    if let Some(idx) = self.contact_at_point(*x, *y) {
                        let contacts = self.filtered_contacts();
                        if idx < contacts.len() {
                            let contact_id = contacts[idx].id;
                            self.select_contact(contact_id);
                            return true;
                        }
                    }

                    // Check toolbar buttons
                    let toolbar_y = self.bounds.y;
                    let toolbar_x = self.bounds.x + self.sidebar_width as isize;

                    if *y >= toolbar_y && *y < toolbar_y + 40 {
                        if *x >= toolbar_x && *x < toolbar_x + 80 {
                            self.create_contact();
                            return true;
                        }
                    }
                }
                false
            }

            WidgetEvent::MouseMove { x, y } => {
                self.hovered_index = self.contact_at_point(*x, *y);
                true
            }

            WidgetEvent::Scroll { delta_y, .. } => {
                let contacts = self.filtered_contacts();
                let visible = self.get_visible_count();
                let max_scroll = contacts.len().saturating_sub(visible);

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
                        let contacts = self.filtered_contacts();
                        if let Some(id) = self.selected_contact_id {
                            if let Some(pos) = contacts.iter().position(|c| c.id == id) {
                                if pos > 0 {
                                    self.select_contact(contacts[pos - 1].id);
                                    if pos.saturating_sub(1) < self.scroll_offset {
                                        self.scroll_offset = pos.saturating_sub(1);
                                    }
                                }
                            }
                        } else if !contacts.is_empty() {
                            self.select_contact(contacts[0].id);
                        }
                        true
                    }
                    0x50 => { // Down
                        let contacts = self.filtered_contacts();
                        if let Some(id) = self.selected_contact_id {
                            if let Some(pos) = contacts.iter().position(|c| c.id == id) {
                                if pos + 1 < contacts.len() {
                                    self.select_contact(contacts[pos + 1].id);
                                    let visible = self.get_visible_count();
                                    if pos + 1 >= self.scroll_offset + visible {
                                        self.scroll_offset = pos + 2 - visible;
                                    }
                                }
                            }
                        } else if !contacts.is_empty() {
                            self.select_contact(contacts[0].id);
                        }
                        true
                    }
                    0x53 | 0x7F => { // Delete
                        self.delete_selected();
                        true
                    }
                    0x1B => { // Escape
                        if self.view_mode == ViewMode::Edit || self.view_mode == ViewMode::Create {
                            self.cancel_editing();
                        } else if self.view_mode == ViewMode::Detail {
                            self.view_mode = ViewMode::List;
                            self.selected_contact_id = None;
                        }
                        true
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
        let border_color = Color::new(60, 60, 65);
        let favorite_color = Color::new(255, 180, 0);

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

        // Sidebar border
        for y in 0..self.bounds.height {
            let px = self.bounds.x + self.sidebar_width as isize;
            let py = self.bounds.y + y as isize;
            surface.set_pixel(px as usize, py as usize, border_color);
        }

        // Sidebar content
        draw_string(surface, self.bounds.x + 10, self.bounds.y + 15, "Contacts", text_color);

        // Filter options
        let filters = [
            ("All Contacts", FilterType::All),
            ("Favorites", FilterType::Favorites),
        ];

        for (i, (name, filter)) in filters.iter().enumerate() {
            let y = self.bounds.y + 50 + (i * 28) as isize;
            let selected = self.filter == *filter;

            if selected {
                for dy in 0..26 {
                    for dx in 0..self.sidebar_width {
                        surface.set_pixel(
                            (self.bounds.x + dx as isize) as usize,
                            (y + dy as isize) as usize,
                            accent_color
                        );
                    }
                }
            }

            let color = if selected { Color::new(255, 255, 255) } else { text_color };
            draw_string(surface, self.bounds.x + 10, y + 5, name, color);
        }

        // Groups header
        let groups_y = self.bounds.y + 130;
        draw_string(surface, self.bounds.x + 10, groups_y, "Groups", dim_text);

        for (i, group) in self.groups.iter().enumerate() {
            let y = groups_y + 25 + (i * 28) as isize;
            let selected = matches!(self.filter, FilterType::Group(gid) if gid == group.id);

            if selected {
                for dy in 0..26 {
                    for dx in 0..self.sidebar_width {
                        surface.set_pixel(
                            (self.bounds.x + dx as isize) as usize,
                            (y + dy as isize) as usize,
                            accent_color
                        );
                    }
                }
            }

            let color = if selected { Color::new(255, 255, 255) } else { text_color };
            draw_string(surface, self.bounds.x + 10, y + 5, &group.name, color);

            let count_str = format!("{}", group.member_count);
            draw_string(surface, self.bounds.x + (self.sidebar_width - 30) as isize, y + 5, &count_str, dim_text);
        }

        // Main content
        let content_x = self.bounds.x + self.sidebar_width as isize + 10;

        // Toolbar
        draw_string(surface, content_x, self.bounds.y + 12, "[+ New]", accent_color);

        // Toolbar separator
        for x in self.sidebar_width..self.bounds.width {
            let px = self.bounds.x + x as isize;
            let py = self.bounds.y + 39;
            surface.set_pixel(px as usize, py as usize, border_color);
        }

        match self.view_mode {
            ViewMode::List | ViewMode::Detail => self.render_contact_list(surface),
            ViewMode::Edit | ViewMode::Create => self.render_edit_form(surface),
        }
    }
}

impl ContactsManager {
    fn render_contact_list(&self, surface: &mut Surface) {
        let text_color = Color::new(220, 220, 220);
        let dim_text = Color::new(150, 150, 150);
        let hover_bg = Color::new(50, 50, 55);
        let selected_bg = Color::new(45, 85, 150);
        let border_color = Color::new(60, 60, 65);
        let accent_color = Color::new(66, 133, 244);
        let favorite_color = Color::new(255, 180, 0);

        let list_x = self.bounds.x + self.sidebar_width as isize + 1;
        let list_y = self.bounds.y + 50;
        let list_width = self.bounds.width.saturating_sub(self.sidebar_width + 1);

        // If detail view, split the view
        let contact_list_width = if self.view_mode == ViewMode::Detail {
            list_width / 3
        } else {
            list_width
        };

        let contacts = self.filtered_contacts();
        let visible_count = self.get_visible_count();
        let row_height = 50;

        for (i, contact) in contacts.iter()
            .skip(self.scroll_offset)
            .take(visible_count)
            .enumerate()
        {
            let row_y = list_y + (i * row_height) as isize;
            let actual_index = self.scroll_offset + i;

            // Row background
            let bg = if self.selected_contact_id == Some(contact.id) {
                selected_bg
            } else if self.hovered_index == Some(actual_index) {
                hover_bg
            } else {
                Color::new(30, 30, 35)
            };

            for y in 0..row_height {
                for x in 0..contact_list_width {
                    surface.set_pixel(
                        (list_x + x as isize) as usize,
                        (row_y + y as isize) as usize,
                        bg
                    );
                }
            }

            // Avatar circle with initials
            let avatar_x = list_x + 10;
            let avatar_y = row_y + 10;
            let avatar_color = accent_color;

            for dy in 0..30 {
                for dx in 0..30 {
                    let cx = dx as i32 - 15;
                    let cy = dy as i32 - 15;
                    if cx * cx + cy * cy <= 15 * 15 {
                        surface.set_pixel(
                            (avatar_x + dx) as usize,
                            (avatar_y + dy) as usize,
                            avatar_color
                        );
                    }
                }
            }

            let initials = contact.initials();
            draw_string(surface, avatar_x + 7, avatar_y + 8, &initials, Color::new(255, 255, 255));

            // Favorite star
            if contact.is_favorite {
                draw_char(surface, list_x + contact_list_width as isize - 25, row_y + 5, '*', favorite_color);
            }

            // Name
            draw_string(surface, list_x + 50, row_y + 8, &contact.display_name(), text_color);

            // Company or email
            let subtitle = contact.company.as_ref()
                .map(|c| c.as_str())
                .or_else(|| contact.primary_email().map(|e| e.address.as_str()))
                .unwrap_or("");
            let subtitle_truncated: String = subtitle.chars().take(30).collect();
            draw_string(surface, list_x + 50, row_y + 26, &subtitle_truncated, dim_text);

            // Row separator
            for x in 0..contact_list_width {
                surface.set_pixel(
                    (list_x + x as isize) as usize,
                    (row_y + row_height as isize - 1) as usize,
                    border_color
                );
            }
        }

        // Detail panel
        if self.view_mode == ViewMode::Detail {
            if let Some(contact) = self.selected_contact() {
                self.render_contact_detail(surface, contact, list_x + contact_list_width as isize + 10);
            }
        }

        // Empty state
        if contacts.is_empty() {
            let empty_msg = if self.search_query.is_empty() {
                "No contacts"
            } else {
                "No contacts match your search"
            };
            draw_string(surface, list_x + 50, list_y + 50, empty_msg, dim_text);
        }
    }

    fn render_contact_detail(&self, surface: &mut Surface, contact: &Contact, x: isize) {
        let text_color = Color::new(220, 220, 220);
        let dim_text = Color::new(150, 150, 150);
        let accent_color = Color::new(66, 133, 244);
        let border_color = Color::new(60, 60, 65);
        let favorite_color = Color::new(255, 180, 0);

        let y_start = self.bounds.y + 50;

        // Detail border
        for y in 0..self.bounds.height.saturating_sub(70) {
            surface.set_pixel(
                (x - 5) as usize,
                (y_start + y as isize) as usize,
                border_color
            );
        }

        // Name and actions
        draw_string(surface, x, y_start + 10, &contact.display_name(), text_color);

        // Favorite indicator
        if contact.is_favorite {
            draw_char(surface, x + (contact.display_name().len() * 8) as isize + 10, y_start + 10, '*', favorite_color);
        }

        // Action buttons
        draw_string(surface, x, y_start + 35, "[Edit]", accent_color);
        draw_string(surface, x + 60, y_start + 35, "[Delete]", dim_text);

        let mut y = y_start + 70;

        // Company
        if let Some(ref company) = contact.company {
            draw_string(surface, x, y, "Company:", dim_text);
            draw_string(surface, x + 80, y, company, text_color);
            y += 22;
        }

        // Job title
        if let Some(ref title) = contact.job_title {
            draw_string(surface, x, y, "Title:", dim_text);
            draw_string(surface, x + 80, y, title, text_color);
            y += 22;
        }

        // Phones
        if !contact.phones.is_empty() {
            y += 10;
            draw_string(surface, x, y, "Phone", dim_text);
            y += 20;
            for phone in &contact.phones {
                draw_string(surface, x + 10, y, &phone.format(), text_color);
                y += 18;
            }
        }

        // Emails
        if !contact.emails.is_empty() {
            y += 10;
            draw_string(surface, x, y, "Email", dim_text);
            y += 20;
            for email in &contact.emails {
                draw_string(surface, x + 10, y, &email.format(), text_color);
                y += 18;
            }
        }

        // Addresses
        if !contact.addresses.is_empty() {
            y += 10;
            draw_string(surface, x, y, "Address", dim_text);
            y += 20;
            for address in &contact.addresses {
                if !address.is_empty() {
                    draw_string(surface, x + 10, y, &address.format_single_line(), text_color);
                    y += 18;
                }
            }
        }

        // Notes
        if !contact.notes.is_empty() {
            y += 10;
            draw_string(surface, x, y, "Notes", dim_text);
            y += 20;
            for line in contact.notes.lines().take(3) {
                let truncated: String = line.chars().take(40).collect();
                draw_string(surface, x + 10, y, &truncated, text_color);
                y += 18;
            }
        }
    }

    fn render_edit_form(&self, surface: &mut Surface) {
        let text_color = Color::new(220, 220, 220);
        let dim_text = Color::new(150, 150, 150);
        let accent_color = Color::new(66, 133, 244);
        let field_bg = Color::new(40, 40, 45);

        let content_x = self.bounds.x + self.sidebar_width as isize + 10;
        let content_y = self.bounds.y + 50;
        let form_width = self.bounds.width.saturating_sub(self.sidebar_width + 20);

        let title = if self.view_mode == ViewMode::Create { "New Contact" } else { "Edit Contact" };
        draw_string(surface, content_x, content_y, title, text_color);
        draw_string(surface, content_x + 150, content_y, "[ESC] Cancel", dim_text);

        if let Some(ref contact) = self.editing_contact {
            let fields: [(&str, &str); 4] = [
                ("First Name:", contact.first_name.as_str()),
                ("Last Name:", contact.last_name.as_str()),
                ("Company:", contact.company.as_deref().unwrap_or("")),
                ("Job Title:", contact.job_title.as_deref().unwrap_or("")),
            ];

            let mut y = content_y + 40;
            for (i, (label, value)) in fields.iter().enumerate() {
                draw_string(surface, content_x, y, label, dim_text);

                let field_x = content_x + 100;
                let field_width = form_width - 120;

                // Field background
                for fy in 0..20 {
                    for fx in 0..field_width {
                        surface.set_pixel(
                            (field_x + fx as isize) as usize,
                            (y + fy as isize) as usize,
                            if self.edit_field == i { Color::new(50, 50, 55) } else { field_bg }
                        );
                    }
                }

                draw_string(surface, field_x + 5, y + 4, value, text_color);

                if self.edit_field == i {
                    let cursor_x = field_x + 5 + (value.len() * 8) as isize;
                    draw_char(surface, cursor_x, y + 4, '|', accent_color);
                }

                y += 35;
            }

            // Save button
            y += 20;
            draw_string(surface, content_x, y, "[Save]", accent_color);
            draw_string(surface, content_x + 60, y, "[Cancel]", dim_text);
        }
    }
}

/// Initialize contacts module
pub fn init() {
    // Initialization code
}
