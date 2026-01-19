//! SIM Card Manager.
//!
//! Provides:
//! - SIM card detection and initialization
//! - PIN/PUK management
//! - SIM contacts (phonebook)
//! - SIM file system access (EF files)
//! - Multi-SIM support (dual SIM)
//! - eSIM support

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// Maximum PIN length
const MAX_PIN_LENGTH: usize = 8;
/// Minimum PIN length
const MIN_PIN_LENGTH: usize = 4;
/// Maximum PUK length
const MAX_PUK_LENGTH: usize = 8;
/// Maximum PIN attempts
const MAX_PIN_ATTEMPTS: u8 = 3;
/// Maximum PUK attempts
const MAX_PUK_ATTEMPTS: u8 = 10;
/// Maximum contacts per SIM
const MAX_CONTACTS: usize = 250;
/// Maximum SMS storage
const MAX_SMS_STORAGE: usize = 50;

/// SIM slot identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimSlot {
    Slot1,
    Slot2,
    ESim,
}

impl SimSlot {
    pub fn as_str(&self) -> &'static str {
        match self {
            SimSlot::Slot1 => "SIM 1",
            SimSlot::Slot2 => "SIM 2",
            SimSlot::ESim => "eSIM",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            SimSlot::Slot1 => 0,
            SimSlot::Slot2 => 1,
            SimSlot::ESim => 2,
        }
    }
}

/// SIM card state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SimState {
    #[default]
    Unknown,
    Absent,
    Present,
    PinRequired,
    PukRequired,
    NetworkLocked,
    Ready,
    Error,
}

impl SimState {
    pub fn description(&self) -> &'static str {
        match self {
            SimState::Unknown => "Unknown state",
            SimState::Absent => "No SIM card inserted",
            SimState::Present => "SIM card detected",
            SimState::PinRequired => "PIN required",
            SimState::PukRequired => "PUK required (PIN blocked)",
            SimState::NetworkLocked => "Network locked",
            SimState::Ready => "SIM ready",
            SimState::Error => "SIM error",
        }
    }

    pub fn needs_unlock(&self) -> bool {
        matches!(self, SimState::PinRequired | SimState::PukRequired | SimState::NetworkLocked)
    }
}

/// PIN type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinType {
    Pin1,
    Pin2,
    Puk1,
    Puk2,
    NetworkPin,
    NetworkSubsetPin,
    ServiceProviderPin,
    CorporatePin,
}

impl PinType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PinType::Pin1 => "PIN",
            PinType::Pin2 => "PIN2",
            PinType::Puk1 => "PUK",
            PinType::Puk2 => "PUK2",
            PinType::NetworkPin => "Network PIN",
            PinType::NetworkSubsetPin => "Network Subset PIN",
            PinType::ServiceProviderPin => "Service Provider PIN",
            PinType::CorporatePin => "Corporate PIN",
        }
    }

    pub fn max_attempts(&self) -> u8 {
        match self {
            PinType::Pin1 | PinType::Pin2 => MAX_PIN_ATTEMPTS,
            PinType::Puk1 | PinType::Puk2 => MAX_PUK_ATTEMPTS,
            _ => MAX_PIN_ATTEMPTS,
        }
    }
}

/// SIM card type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SimType {
    #[default]
    Unknown,
    /// 2G SIM (GSM)
    Sim,
    /// 3G USIM (UMTS)
    Usim,
    /// 4G ISIM (IMS)
    Isim,
    /// eSIM (embedded)
    ESim,
}

impl SimType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SimType::Unknown => "Unknown",
            SimType::Sim => "SIM (2G)",
            SimType::Usim => "USIM (3G/4G)",
            SimType::Isim => "ISIM (IMS)",
            SimType::ESim => "eSIM",
        }
    }
}

/// SIM card information
#[derive(Debug, Clone, Default)]
pub struct SimInfo {
    /// International Mobile Subscriber Identity
    pub imsi: String,
    /// Integrated Circuit Card Identifier
    pub iccid: String,
    /// Mobile Subscriber ISDN Number (phone number)
    pub msisdn: String,
    /// Service Provider Name
    pub spn: String,
    /// SIM card type
    pub sim_type: SimType,
    /// Mobile Country Code
    pub mcc: u16,
    /// Mobile Network Code
    pub mnc: u16,
    /// Available storage for contacts
    pub phonebook_capacity: u16,
    /// Used storage for contacts
    pub phonebook_used: u16,
    /// Available SMS storage
    pub sms_capacity: u16,
    /// Used SMS storage
    pub sms_used: u16,
}

impl SimInfo {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get PLMN (MCC + MNC)
    pub fn plmn(&self) -> String {
        use core::fmt::Write;
        let mut s = String::new();
        let _ = write!(s, "{:03}{:02}", self.mcc, self.mnc);
        s
    }

    /// Get home network name
    pub fn network_name(&self) -> &str {
        if !self.spn.is_empty() {
            &self.spn
        } else {
            "Unknown"
        }
    }
}

/// PIN status
#[derive(Debug, Clone)]
pub struct PinStatus {
    /// PIN type
    pub pin_type: PinType,
    /// Whether PIN is enabled
    pub enabled: bool,
    /// Remaining attempts
    pub attempts_remaining: u8,
    /// Whether PIN is blocked (need PUK)
    pub blocked: bool,
}

impl PinStatus {
    pub fn new(pin_type: PinType) -> Self {
        Self {
            pin_type,
            enabled: true,
            attempts_remaining: pin_type.max_attempts(),
            blocked: false,
        }
    }
}

/// SIM contact
#[derive(Debug, Clone)]
pub struct SimContact {
    /// Index in SIM phonebook
    pub index: u16,
    /// Contact name
    pub name: String,
    /// Phone number
    pub number: String,
    /// Number type (national/international)
    pub number_type: NumberType,
    /// Email (if supported)
    pub email: Option<String>,
}

impl SimContact {
    pub fn new(index: u16, name: String, number: String) -> Self {
        let number_type = if number.starts_with('+') {
            NumberType::International
        } else {
            NumberType::National
        };

        Self {
            index,
            name,
            number,
            number_type,
            email: None,
        }
    }
}

/// Phone number type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumberType {
    #[default]
    Unknown,
    National,
    International,
    NetworkSpecific,
    Subscriber,
    Abbreviated,
}

impl NumberType {
    pub fn ton_npi(&self) -> u8 {
        match self {
            NumberType::Unknown => 0x81,
            NumberType::National => 0xA1,
            NumberType::International => 0x91,
            NumberType::NetworkSpecific => 0xB1,
            NumberType::Subscriber => 0xC1,
            NumberType::Abbreviated => 0xD1,
        }
    }
}

/// SIM Elementary File identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimFile {
    /// ICCID
    EfIccid,
    /// IMSI
    EfImsi,
    /// ADN (Abbreviated Dialing Numbers - phonebook)
    EfAdn,
    /// FDN (Fixed Dialing Numbers)
    EfFdn,
    /// SMS
    EfSms,
    /// MSISDN (own number)
    EfMsisdn,
    /// SPN (Service Provider Name)
    EfSpn,
    /// PLMN selector
    EfPlmnSel,
    /// Language preference
    EfLi,
    /// Administrative data
    EfAd,
}

impl SimFile {
    /// Get file ID
    pub fn file_id(&self) -> u16 {
        match self {
            SimFile::EfIccid => 0x2FE2,
            SimFile::EfImsi => 0x6F07,
            SimFile::EfAdn => 0x6F3A,
            SimFile::EfFdn => 0x6F3B,
            SimFile::EfSms => 0x6F3C,
            SimFile::EfMsisdn => 0x6F40,
            SimFile::EfSpn => 0x6F46,
            SimFile::EfPlmnSel => 0x6F30,
            SimFile::EfLi => 0x6F05,
            SimFile::EfAd => 0x6FAD,
        }
    }

    /// Get parent directory
    pub fn parent_df(&self) -> u16 {
        match self {
            SimFile::EfIccid => 0x3F00, // MF
            _ => 0x7F20, // DF GSM
        }
    }
}

/// eSIM profile
#[derive(Debug, Clone)]
pub struct EsimProfile {
    /// Profile ICCID
    pub iccid: String,
    /// Profile name
    pub name: String,
    /// Service provider
    pub provider: String,
    /// Profile state
    pub state: EsimProfileState,
    /// Profile type
    pub profile_type: EsimProfileType,
    /// Icon (base64 encoded)
    pub icon: Option<String>,
}

impl EsimProfile {
    pub fn new(iccid: String, name: String, provider: String) -> Self {
        Self {
            iccid,
            name,
            provider,
            state: EsimProfileState::Disabled,
            profile_type: EsimProfileType::Operational,
            icon: None,
        }
    }
}

/// eSIM profile state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EsimProfileState {
    #[default]
    Disabled,
    Enabled,
    Deleting,
    Error,
}

/// eSIM profile type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EsimProfileType {
    #[default]
    Operational,
    Test,
    Provisioning,
    Bootstrap,
}

/// SIM card handle
pub struct SimCard {
    /// SIM slot
    slot: SimSlot,
    /// Current state
    state: SimState,
    /// SIM information
    info: SimInfo,
    /// PIN1 status
    pin1_status: PinStatus,
    /// PIN2 status
    pin2_status: PinStatus,
    /// Contacts cache
    contacts: Vec<SimContact>,
    /// eSIM profiles (if eSIM)
    esim_profiles: Vec<EsimProfile>,
    /// Whether initialized
    initialized: bool,
}

impl SimCard {
    fn new(slot: SimSlot) -> Self {
        Self {
            slot,
            state: SimState::Unknown,
            info: SimInfo::new(),
            pin1_status: PinStatus::new(PinType::Pin1),
            pin2_status: PinStatus::new(PinType::Pin2),
            contacts: Vec::new(),
            esim_profiles: Vec::new(),
            initialized: false,
        }
    }

    /// Initialize SIM card
    pub fn init(&mut self) -> KResult<()> {
        // Detect SIM presence
        self.state = self.detect_sim()?;

        if self.state == SimState::Absent {
            return Ok(());
        }

        // If PIN required, wait for unlock
        if self.state == SimState::PinRequired {
            return Ok(());
        }

        // Read SIM info
        if self.state == SimState::Ready || self.state == SimState::Present {
            self.read_sim_info()?;
            self.state = SimState::Ready;
        }

        self.initialized = true;
        Ok(())
    }

    /// Detect SIM presence
    fn detect_sim(&self) -> KResult<SimState> {
        // In real implementation, would send AT+CPIN? command
        // Placeholder: assume SIM present
        Ok(SimState::Present)
    }

    /// Read SIM information
    fn read_sim_info(&mut self) -> KResult<()> {
        // Would read IMSI, ICCID, SPN, etc. from SIM
        // Placeholder implementation
        Ok(())
    }

    /// Verify PIN
    pub fn verify_pin(&mut self, pin_type: PinType, pin: &str) -> KResult<()> {
        // Validate PIN length
        if pin.len() < MIN_PIN_LENGTH || pin.len() > MAX_PIN_LENGTH {
            return Err(KError::Invalid);
        }

        // Validate PIN is numeric
        if !pin.chars().all(|c| c.is_ascii_digit()) {
            return Err(KError::Invalid);
        }

        let status = match pin_type {
            PinType::Pin1 | PinType::Puk1 => &mut self.pin1_status,
            PinType::Pin2 | PinType::Puk2 => &mut self.pin2_status,
            _ => return Err(KError::NotSupported),
        };

        if status.blocked {
            return Err(KError::IO);
        }

        // In real implementation, would send AT+CPIN command
        // Placeholder: accept any PIN
        status.attempts_remaining = status.pin_type.max_attempts();

        if pin_type == PinType::Pin1 {
            self.state = SimState::Ready;
            self.read_sim_info()?;
        }

        Ok(())
    }

    /// Unblock PIN with PUK
    pub fn unblock_pin(&mut self, puk: &str, new_pin: &str) -> KResult<()> {
        // Validate PUK
        if puk.len() != MAX_PUK_LENGTH {
            return Err(KError::Invalid);
        }
        if !puk.chars().all(|c| c.is_ascii_digit()) {
            return Err(KError::Invalid);
        }

        // Validate new PIN
        if new_pin.len() < MIN_PIN_LENGTH || new_pin.len() > MAX_PIN_LENGTH {
            return Err(KError::Invalid);
        }
        if !new_pin.chars().all(|c| c.is_ascii_digit()) {
            return Err(KError::Invalid);
        }

        // In real implementation, would send AT+CPIN=PUK,newPIN
        self.pin1_status.blocked = false;
        self.pin1_status.attempts_remaining = MAX_PIN_ATTEMPTS;
        self.state = SimState::Ready;

        Ok(())
    }

    /// Change PIN
    pub fn change_pin(&mut self, pin_type: PinType, old_pin: &str, new_pin: &str) -> KResult<()> {
        // Validate old PIN
        if old_pin.len() < MIN_PIN_LENGTH || old_pin.len() > MAX_PIN_LENGTH {
            return Err(KError::Invalid);
        }

        // Validate new PIN
        if new_pin.len() < MIN_PIN_LENGTH || new_pin.len() > MAX_PIN_LENGTH {
            return Err(KError::Invalid);
        }

        // Validate both are numeric
        if !old_pin.chars().all(|c| c.is_ascii_digit()) ||
           !new_pin.chars().all(|c| c.is_ascii_digit()) {
            return Err(KError::Invalid);
        }

        // In real implementation, would send AT+CPWD command
        Ok(())
    }

    /// Enable/disable PIN
    pub fn set_pin_enabled(&mut self, enabled: bool, pin: &str) -> KResult<()> {
        // Validate PIN
        if pin.len() < MIN_PIN_LENGTH || pin.len() > MAX_PIN_LENGTH {
            return Err(KError::Invalid);
        }

        // In real implementation, would send AT+CLCK command
        self.pin1_status.enabled = enabled;
        Ok(())
    }

    /// Read contacts from SIM
    pub fn read_contacts(&mut self) -> KResult<&[SimContact]> {
        if self.state != SimState::Ready {
            return Err(KError::IO);
        }

        // In real implementation, would read from EF_ADN
        // Placeholder: return cached contacts
        Ok(&self.contacts)
    }

    /// Add contact to SIM
    pub fn add_contact(&mut self, name: &str, number: &str) -> KResult<u16> {
        if self.state != SimState::Ready {
            return Err(KError::IO);
        }

        if self.contacts.len() >= MAX_CONTACTS {
            return Err(KError::NoMemory);
        }

        // Find next available index
        let index = self.contacts.len() as u16 + 1;

        let contact = SimContact::new(
            index,
            String::from(name),
            String::from(number),
        );

        self.contacts.push(contact);
        self.info.phonebook_used += 1;

        Ok(index)
    }

    /// Delete contact from SIM
    pub fn delete_contact(&mut self, index: u16) -> KResult<()> {
        if self.state != SimState::Ready {
            return Err(KError::IO);
        }

        let pos = self.contacts.iter().position(|c| c.index == index);
        if let Some(pos) = pos {
            self.contacts.remove(pos);
            if self.info.phonebook_used > 0 {
                self.info.phonebook_used -= 1;
            }
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    /// Update contact on SIM
    pub fn update_contact(&mut self, index: u16, name: &str, number: &str) -> KResult<()> {
        if self.state != SimState::Ready {
            return Err(KError::IO);
        }

        let contact = self.contacts.iter_mut().find(|c| c.index == index);
        if let Some(contact) = contact {
            contact.name = String::from(name);
            contact.number = String::from(number);
            contact.number_type = if number.starts_with('+') {
                NumberType::International
            } else {
                NumberType::National
            };
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    /// Get eSIM profiles (if eSIM slot)
    pub fn esim_profiles(&self) -> &[EsimProfile] {
        &self.esim_profiles
    }

    /// Enable eSIM profile
    pub fn enable_esim_profile(&mut self, iccid: &str) -> KResult<()> {
        if self.slot != SimSlot::ESim {
            return Err(KError::NotSupported);
        }

        // Disable current profile
        for profile in &mut self.esim_profiles {
            if profile.state == EsimProfileState::Enabled {
                profile.state = EsimProfileState::Disabled;
            }
        }

        // Enable requested profile
        let profile = self.esim_profiles.iter_mut()
            .find(|p| p.iccid == iccid);

        if let Some(profile) = profile {
            profile.state = EsimProfileState::Enabled;
            self.state = SimState::Present;
            self.init()?;
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    /// Delete eSIM profile
    pub fn delete_esim_profile(&mut self, iccid: &str) -> KResult<()> {
        if self.slot != SimSlot::ESim {
            return Err(KError::NotSupported);
        }

        let pos = self.esim_profiles.iter().position(|p| p.iccid == iccid);
        if let Some(pos) = pos {
            let profile = &self.esim_profiles[pos];
            if profile.state == EsimProfileState::Enabled {
                return Err(KError::IO); // Can't delete active profile
            }
            self.esim_profiles.remove(pos);
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    /// Download eSIM profile (QR code / activation code)
    pub fn download_esim_profile(&mut self, activation_code: &str) -> KResult<String> {
        if self.slot != SimSlot::ESim {
            return Err(KError::NotSupported);
        }

        // Parse activation code (format: LPA:1$smdp.address$matchingId)
        let _parts: Vec<&str> = activation_code.split('$').collect();

        // In real implementation, would:
        // 1. Connect to SM-DP+ server
        // 2. Authenticate
        // 3. Download profile
        // 4. Install profile

        // Placeholder: create dummy profile
        let iccid = String::from("8901234567890123456");
        let profile = EsimProfile::new(
            iccid.clone(),
            String::from("Downloaded Profile"),
            String::from("Carrier"),
        );

        self.esim_profiles.push(profile);

        Ok(iccid)
    }
}

/// SIM Manager statistics
#[derive(Debug, Default)]
pub struct SimStats {
    /// PIN verifications attempted
    pub pin_attempts: AtomicU64,
    /// Successful PIN verifications
    pub pin_successes: AtomicU64,
    /// Failed PIN verifications
    pub pin_failures: AtomicU64,
    /// Contacts read
    pub contacts_read: AtomicU64,
    /// Contacts written
    pub contacts_written: AtomicU64,
    /// eSIM profile downloads
    pub esim_downloads: AtomicU64,
}

impl SimStats {
    pub const fn new() -> Self {
        Self {
            pin_attempts: AtomicU64::new(0),
            pin_successes: AtomicU64::new(0),
            pin_failures: AtomicU64::new(0),
            contacts_read: AtomicU64::new(0),
            contacts_written: AtomicU64::new(0),
            esim_downloads: AtomicU64::new(0),
        }
    }
}

/// SIM Manager
pub struct SimManager {
    /// SIM cards by slot
    sim_cards: [Option<SimCard>; 3],
    /// Active slot for data
    active_data_slot: SimSlot,
    /// Active slot for voice
    active_voice_slot: SimSlot,
    /// Statistics
    stats: SimStats,
    /// Initialized flag
    initialized: AtomicBool,
}

impl SimManager {
    pub const fn new() -> Self {
        Self {
            sim_cards: [None, None, None],
            active_data_slot: SimSlot::Slot1,
            active_voice_slot: SimSlot::Slot1,
            stats: SimStats::new(),
            initialized: AtomicBool::new(false),
        }
    }

    /// Initialize SIM manager
    pub fn init(&mut self) {
        // Initialize SIM slots
        self.sim_cards[0] = Some(SimCard::new(SimSlot::Slot1));
        self.sim_cards[1] = Some(SimCard::new(SimSlot::Slot2));
        self.sim_cards[2] = Some(SimCard::new(SimSlot::ESim));

        // Initialize each SIM
        for sim in self.sim_cards.iter_mut().flatten() {
            let _ = sim.init();
        }

        self.initialized.store(true, Ordering::SeqCst);
    }

    /// Get SIM card for slot
    pub fn get_sim(&self, slot: SimSlot) -> Option<&SimCard> {
        self.sim_cards[slot.index()].as_ref()
    }

    /// Get mutable SIM card for slot
    pub fn get_sim_mut(&mut self, slot: SimSlot) -> Option<&mut SimCard> {
        self.sim_cards[slot.index()].as_mut()
    }

    /// Get state of SIM in slot
    pub fn sim_state(&self, slot: SimSlot) -> SimState {
        self.sim_cards[slot.index()]
            .as_ref()
            .map(|s| s.state)
            .unwrap_or(SimState::Unknown)
    }

    /// Get SIM info for slot
    pub fn sim_info(&self, slot: SimSlot) -> Option<&SimInfo> {
        self.sim_cards[slot.index()]
            .as_ref()
            .map(|s| &s.info)
    }

    /// Verify PIN for slot
    pub fn verify_pin(&mut self, slot: SimSlot, pin: &str) -> KResult<()> {
        self.stats.pin_attempts.fetch_add(1, Ordering::Relaxed);

        let result = self.sim_cards[slot.index()]
            .as_mut()
            .ok_or(KError::NotFound)?
            .verify_pin(PinType::Pin1, pin);

        if result.is_ok() {
            self.stats.pin_successes.fetch_add(1, Ordering::Relaxed);
        } else {
            self.stats.pin_failures.fetch_add(1, Ordering::Relaxed);
        }

        result
    }

    /// Unblock PIN for slot
    pub fn unblock_pin(&mut self, slot: SimSlot, puk: &str, new_pin: &str) -> KResult<()> {
        self.sim_cards[slot.index()]
            .as_mut()
            .ok_or(KError::NotFound)?
            .unblock_pin(puk, new_pin)
    }

    /// Set active data slot
    pub fn set_data_slot(&mut self, slot: SimSlot) -> KResult<()> {
        if self.sim_state(slot) != SimState::Ready {
            return Err(KError::IO);
        }
        self.active_data_slot = slot;
        Ok(())
    }

    /// Set active voice slot
    pub fn set_voice_slot(&mut self, slot: SimSlot) -> KResult<()> {
        if self.sim_state(slot) != SimState::Ready {
            return Err(KError::IO);
        }
        self.active_voice_slot = slot;
        Ok(())
    }

    /// Get active data slot
    pub fn data_slot(&self) -> SimSlot {
        self.active_data_slot
    }

    /// Get active voice slot
    pub fn voice_slot(&self) -> SimSlot {
        self.active_voice_slot
    }

    /// Get contacts from slot
    pub fn get_contacts(&mut self, slot: SimSlot) -> KResult<Vec<SimContact>> {
        self.stats.contacts_read.fetch_add(1, Ordering::Relaxed);

        let sim = self.sim_cards[slot.index()]
            .as_mut()
            .ok_or(KError::NotFound)?;

        let contacts = sim.read_contacts()?;
        Ok(contacts.to_vec())
    }

    /// Add contact to slot
    pub fn add_contact(&mut self, slot: SimSlot, name: &str, number: &str) -> KResult<u16> {
        self.stats.contacts_written.fetch_add(1, Ordering::Relaxed);

        self.sim_cards[slot.index()]
            .as_mut()
            .ok_or(KError::NotFound)?
            .add_contact(name, number)
    }

    /// Get available SIM slots
    pub fn available_slots(&self) -> Vec<SimSlot> {
        let mut slots = Vec::new();

        for (i, sim) in self.sim_cards.iter().enumerate() {
            if let Some(sim) = sim {
                if sim.state == SimState::Ready {
                    slots.push(match i {
                        0 => SimSlot::Slot1,
                        1 => SimSlot::Slot2,
                        _ => SimSlot::ESim,
                    });
                }
            }
        }

        slots
    }

    /// Check if dual SIM active
    pub fn is_dual_sim(&self) -> bool {
        let ready_count = self.sim_cards.iter()
            .filter(|s| s.as_ref().map(|s| s.state == SimState::Ready).unwrap_or(false))
            .count();
        ready_count >= 2
    }

    /// Get statistics
    pub fn stats(&self) -> SimStatsSnapshot {
        SimStatsSnapshot {
            pin_attempts: self.stats.pin_attempts.load(Ordering::Relaxed),
            pin_successes: self.stats.pin_successes.load(Ordering::Relaxed),
            pin_failures: self.stats.pin_failures.load(Ordering::Relaxed),
            contacts_read: self.stats.contacts_read.load(Ordering::Relaxed),
            contacts_written: self.stats.contacts_written.load(Ordering::Relaxed),
            esim_downloads: self.stats.esim_downloads.load(Ordering::Relaxed),
        }
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        use core::fmt::Write;
        let mut s = String::new();

        let _ = writeln!(s, "SIM Manager Status:");

        for (i, sim) in self.sim_cards.iter().enumerate() {
            let slot = match i {
                0 => SimSlot::Slot1,
                1 => SimSlot::Slot2,
                _ => SimSlot::ESim,
            };

            if let Some(sim) = sim {
                let _ = writeln!(s, "  {}: {} - {}",
                    slot.as_str(),
                    sim.state.description(),
                    if sim.state == SimState::Ready {
                        sim.info.network_name()
                    } else {
                        ""
                    }
                );
            }
        }

        let _ = writeln!(s, "\nActive slots:");
        let _ = writeln!(s, "  Data: {}", self.active_data_slot.as_str());
        let _ = writeln!(s, "  Voice: {}", self.active_voice_slot.as_str());

        s
    }
}

/// Statistics snapshot
#[derive(Debug, Clone)]
pub struct SimStatsSnapshot {
    pub pin_attempts: u64,
    pub pin_successes: u64,
    pub pin_failures: u64,
    pub contacts_read: u64,
    pub contacts_written: u64,
    pub esim_downloads: u64,
}

/// Global SIM manager
static SIM_MANAGER: IrqSafeMutex<SimManager> = IrqSafeMutex::new(SimManager::new());

/// Initialize SIM manager
pub fn init() {
    SIM_MANAGER.lock().init();
}

/// Get SIM state for slot
pub fn sim_state(slot: SimSlot) -> SimState {
    SIM_MANAGER.lock().sim_state(slot)
}

/// Verify PIN for slot
pub fn verify_pin(slot: SimSlot, pin: &str) -> KResult<()> {
    SIM_MANAGER.lock().verify_pin(slot, pin)
}

/// Unblock PIN for slot
pub fn unblock_pin(slot: SimSlot, puk: &str, new_pin: &str) -> KResult<()> {
    SIM_MANAGER.lock().unblock_pin(slot, puk, new_pin)
}

/// Set active data slot
pub fn set_data_slot(slot: SimSlot) -> KResult<()> {
    SIM_MANAGER.lock().set_data_slot(slot)
}

/// Set active voice slot
pub fn set_voice_slot(slot: SimSlot) -> KResult<()> {
    SIM_MANAGER.lock().set_voice_slot(slot)
}

/// Get active data slot
pub fn data_slot() -> SimSlot {
    SIM_MANAGER.lock().data_slot()
}

/// Get active voice slot
pub fn voice_slot() -> SimSlot {
    SIM_MANAGER.lock().voice_slot()
}

/// Get contacts from slot
pub fn get_contacts(slot: SimSlot) -> KResult<Vec<SimContact>> {
    SIM_MANAGER.lock().get_contacts(slot)
}

/// Add contact to slot
pub fn add_contact(slot: SimSlot, name: &str, number: &str) -> KResult<u16> {
    SIM_MANAGER.lock().add_contact(slot, name, number)
}

/// Get available SIM slots
pub fn available_slots() -> Vec<SimSlot> {
    SIM_MANAGER.lock().available_slots()
}

/// Check if dual SIM active
pub fn is_dual_sim() -> bool {
    SIM_MANAGER.lock().is_dual_sim()
}

/// Get statistics
pub fn stats() -> SimStatsSnapshot {
    SIM_MANAGER.lock().stats()
}

/// Format status string
pub fn format_status() -> String {
    SIM_MANAGER.lock().format_status()
}
