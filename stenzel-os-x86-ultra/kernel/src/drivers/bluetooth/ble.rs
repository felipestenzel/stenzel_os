//! Bluetooth Low Energy (BLE) subsystem.
//!
//! Provides:
//! - GATT client and server
//! - Advertising and scanning
//! - Connection management
//! - Pairing and bonding

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU64, AtomicU32, AtomicBool, Ordering};
use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// BLE address type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressType {
    Public,
    RandomStatic,
    RandomPrivateResolvable,
    RandomPrivateNonResolvable,
}

impl AddressType {
    pub fn from_u8(val: u8) -> Self {
        match val & 0x03 {
            0 => AddressType::Public,
            1 => AddressType::RandomStatic,
            2 => AddressType::RandomPrivateResolvable,
            _ => AddressType::RandomPrivateNonResolvable,
        }
    }

    pub fn as_u8(&self) -> u8 {
        match self {
            AddressType::Public => 0,
            AddressType::RandomStatic => 1,
            AddressType::RandomPrivateResolvable => 2,
            AddressType::RandomPrivateNonResolvable => 3,
        }
    }

    pub fn is_random(&self) -> bool {
        !matches!(self, AddressType::Public)
    }
}

/// BLE address
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BleAddr {
    pub addr: [u8; 6],
    pub addr_type: AddressType,
}

impl BleAddr {
    pub fn new(addr: [u8; 6], addr_type: AddressType) -> Self {
        Self { addr, addr_type }
    }

    pub fn public(addr: [u8; 6]) -> Self {
        Self::new(addr, AddressType::Public)
    }

    pub fn random(addr: [u8; 6]) -> Self {
        Self::new(addr, AddressType::RandomStatic)
    }

    pub fn is_zero(&self) -> bool {
        self.addr == [0u8; 6]
    }

    pub fn to_string(&self) -> String {
        use core::fmt::Write;
        let mut s = String::with_capacity(20);
        for (i, b) in self.addr.iter().rev().enumerate() {
            if i > 0 { s.push(':'); }
            let _ = write!(s, "{:02X}", b);
        }
        s
    }
}

impl Default for AddressType {
    fn default() -> Self {
        AddressType::Public
    }
}

/// UUID type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Uuid {
    Uuid16(u16),
    Uuid32(u32),
    Uuid128([u8; 16]),
}

impl Uuid {
    pub fn uuid16(val: u16) -> Self {
        Uuid::Uuid16(val)
    }

    pub fn uuid32(val: u32) -> Self {
        Uuid::Uuid32(val)
    }

    pub fn uuid128(val: [u8; 16]) -> Self {
        Uuid::Uuid128(val)
    }

    pub fn to_uuid128(&self) -> [u8; 16] {
        // Bluetooth Base UUID: 00000000-0000-1000-8000-00805F9B34FB
        let mut uuid128 = [
            0xfb, 0x34, 0x9b, 0x5f, 0x80, 0x00, 0x00, 0x80,
            0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        match self {
            Uuid::Uuid16(val) => {
                uuid128[12] = (*val & 0xff) as u8;
                uuid128[13] = ((*val >> 8) & 0xff) as u8;
            }
            Uuid::Uuid32(val) => {
                uuid128[12] = (*val & 0xff) as u8;
                uuid128[13] = ((*val >> 8) & 0xff) as u8;
                uuid128[14] = ((*val >> 16) & 0xff) as u8;
                uuid128[15] = ((*val >> 24) & 0xff) as u8;
            }
            Uuid::Uuid128(val) => {
                uuid128.copy_from_slice(val);
            }
        }

        uuid128
    }

    pub fn to_string(&self) -> String {
        use core::fmt::Write;
        match self {
            Uuid::Uuid16(val) => {
                let mut s = String::new();
                let _ = write!(s, "{:04X}", val);
                s
            }
            Uuid::Uuid32(val) => {
                let mut s = String::new();
                let _ = write!(s, "{:08X}", val);
                s
            }
            Uuid::Uuid128(val) => {
                let mut s = String::with_capacity(36);
                for (i, b) in val.iter().rev().enumerate() {
                    if i == 4 || i == 6 || i == 8 || i == 10 {
                        s.push('-');
                    }
                    let _ = write!(s, "{:02X}", b);
                }
                s
            }
        }
    }
}

/// Standard GATT service UUIDs
pub mod gatt_services {
    pub const GENERIC_ACCESS: u16 = 0x1800;
    pub const GENERIC_ATTRIBUTE: u16 = 0x1801;
    pub const DEVICE_INFORMATION: u16 = 0x180A;
    pub const BATTERY_SERVICE: u16 = 0x180F;
    pub const HEART_RATE: u16 = 0x180D;
    pub const HEALTH_THERMOMETER: u16 = 0x1809;
    pub const BLOOD_PRESSURE: u16 = 0x1810;
    pub const CURRENT_TIME: u16 = 0x1805;
    pub const CYCLING_POWER: u16 = 0x1818;
    pub const CYCLING_SPEED_AND_CADENCE: u16 = 0x1816;
    pub const RUNNING_SPEED_AND_CADENCE: u16 = 0x1814;
    pub const ENVIRONMENTAL_SENSING: u16 = 0x181A;
    pub const HUMAN_INTERFACE_DEVICE: u16 = 0x1812;
}

/// Standard GATT characteristic UUIDs
pub mod gatt_characteristics {
    pub const DEVICE_NAME: u16 = 0x2A00;
    pub const APPEARANCE: u16 = 0x2A01;
    pub const PERIPHERAL_PRIVACY_FLAG: u16 = 0x2A02;
    pub const RECONNECTION_ADDRESS: u16 = 0x2A03;
    pub const PPCP: u16 = 0x2A04; // Peripheral Preferred Connection Parameters
    pub const SERVICE_CHANGED: u16 = 0x2A05;
    pub const BATTERY_LEVEL: u16 = 0x2A19;
    pub const MANUFACTURER_NAME: u16 = 0x2A29;
    pub const MODEL_NUMBER: u16 = 0x2A24;
    pub const SERIAL_NUMBER: u16 = 0x2A25;
    pub const HARDWARE_REVISION: u16 = 0x2A27;
    pub const FIRMWARE_REVISION: u16 = 0x2A26;
    pub const SOFTWARE_REVISION: u16 = 0x2A28;
    pub const SYSTEM_ID: u16 = 0x2A23;
    pub const PNP_ID: u16 = 0x2A50;
}

/// Characteristic properties
#[derive(Debug, Clone, Copy, Default)]
pub struct CharacteristicProperties {
    pub broadcast: bool,
    pub read: bool,
    pub write_without_response: bool,
    pub write: bool,
    pub notify: bool,
    pub indicate: bool,
    pub authenticated_signed_writes: bool,
    pub extended_properties: bool,
}

impl CharacteristicProperties {
    pub fn from_u8(val: u8) -> Self {
        Self {
            broadcast: (val & 0x01) != 0,
            read: (val & 0x02) != 0,
            write_without_response: (val & 0x04) != 0,
            write: (val & 0x08) != 0,
            notify: (val & 0x10) != 0,
            indicate: (val & 0x20) != 0,
            authenticated_signed_writes: (val & 0x40) != 0,
            extended_properties: (val & 0x80) != 0,
        }
    }

    pub fn as_u8(&self) -> u8 {
        let mut val = 0u8;
        if self.broadcast { val |= 0x01; }
        if self.read { val |= 0x02; }
        if self.write_without_response { val |= 0x04; }
        if self.write { val |= 0x08; }
        if self.notify { val |= 0x10; }
        if self.indicate { val |= 0x20; }
        if self.authenticated_signed_writes { val |= 0x40; }
        if self.extended_properties { val |= 0x80; }
        val
    }

    pub fn readable() -> Self {
        Self { read: true, ..Default::default() }
    }

    pub fn writable() -> Self {
        Self { write: true, ..Default::default() }
    }

    pub fn read_write() -> Self {
        Self { read: true, write: true, ..Default::default() }
    }

    pub fn notifiable() -> Self {
        Self { read: true, notify: true, ..Default::default() }
    }
}

/// GATT descriptor
#[derive(Debug, Clone)]
pub struct GattDescriptor {
    pub handle: u16,
    pub uuid: Uuid,
    pub value: Vec<u8>,
}

/// GATT characteristic
#[derive(Debug, Clone)]
pub struct GattCharacteristic {
    pub handle: u16,
    pub value_handle: u16,
    pub uuid: Uuid,
    pub properties: CharacteristicProperties,
    pub value: Vec<u8>,
    pub descriptors: Vec<GattDescriptor>,
}

impl GattCharacteristic {
    pub fn new(handle: u16, value_handle: u16, uuid: Uuid, properties: CharacteristicProperties) -> Self {
        Self {
            handle,
            value_handle,
            uuid,
            properties,
            value: Vec::new(),
            descriptors: Vec::new(),
        }
    }

    pub fn add_descriptor(&mut self, descriptor: GattDescriptor) {
        self.descriptors.push(descriptor);
    }
}

/// GATT service
#[derive(Debug, Clone)]
pub struct GattService {
    pub handle: u16,
    pub end_handle: u16,
    pub uuid: Uuid,
    pub primary: bool,
    pub characteristics: Vec<GattCharacteristic>,
}

impl GattService {
    pub fn new(handle: u16, end_handle: u16, uuid: Uuid, primary: bool) -> Self {
        Self {
            handle,
            end_handle,
            uuid,
            primary,
            characteristics: Vec::new(),
        }
    }

    pub fn add_characteristic(&mut self, characteristic: GattCharacteristic) {
        self.characteristics.push(characteristic);
    }
}

/// Advertisement type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvertisingType {
    ConnectableUndirected,
    ConnectableDirectedHighDuty,
    ScannableUndirected,
    NonConnectableUndirected,
    ConnectableDirectedLowDuty,
}

impl AdvertisingType {
    pub fn as_u8(&self) -> u8 {
        match self {
            AdvertisingType::ConnectableUndirected => 0x00,
            AdvertisingType::ConnectableDirectedHighDuty => 0x01,
            AdvertisingType::ScannableUndirected => 0x02,
            AdvertisingType::NonConnectableUndirected => 0x03,
            AdvertisingType::ConnectableDirectedLowDuty => 0x04,
        }
    }
}

/// Advertisement data type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdType {
    Flags = 0x01,
    IncompleteList16BitServiceUuids = 0x02,
    CompleteList16BitServiceUuids = 0x03,
    IncompleteList32BitServiceUuids = 0x04,
    CompleteList32BitServiceUuids = 0x05,
    IncompleteList128BitServiceUuids = 0x06,
    CompleteList128BitServiceUuids = 0x07,
    ShortenedLocalName = 0x08,
    CompleteLocalName = 0x09,
    TxPowerLevel = 0x0A,
    SlaveConnectionIntervalRange = 0x12,
    ServiceSolicitation16BitUuids = 0x14,
    ServiceSolicitation32BitUuids = 0x1F,
    ServiceSolicitation128BitUuids = 0x15,
    ServiceData16BitUuid = 0x16,
    ServiceData32BitUuid = 0x20,
    ServiceData128BitUuid = 0x21,
    Appearance = 0x19,
    ManufacturerSpecificData = 0xFF,
}

/// Advertisement flags
#[derive(Debug, Clone, Copy, Default)]
pub struct AdvFlags {
    pub le_limited_discoverable: bool,
    pub le_general_discoverable: bool,
    pub br_edr_not_supported: bool,
    pub le_br_edr_controller: bool,
    pub le_br_edr_host: bool,
}

impl AdvFlags {
    pub fn from_u8(val: u8) -> Self {
        Self {
            le_limited_discoverable: (val & 0x01) != 0,
            le_general_discoverable: (val & 0x02) != 0,
            br_edr_not_supported: (val & 0x04) != 0,
            le_br_edr_controller: (val & 0x08) != 0,
            le_br_edr_host: (val & 0x10) != 0,
        }
    }

    pub fn as_u8(&self) -> u8 {
        let mut val = 0u8;
        if self.le_limited_discoverable { val |= 0x01; }
        if self.le_general_discoverable { val |= 0x02; }
        if self.br_edr_not_supported { val |= 0x04; }
        if self.le_br_edr_controller { val |= 0x08; }
        if self.le_br_edr_host { val |= 0x10; }
        val
    }

    pub fn le_only_general() -> Self {
        Self {
            le_general_discoverable: true,
            br_edr_not_supported: true,
            ..Default::default()
        }
    }

    pub fn le_only_limited() -> Self {
        Self {
            le_limited_discoverable: true,
            br_edr_not_supported: true,
            ..Default::default()
        }
    }
}

/// Advertisement data builder
#[derive(Debug, Clone, Default)]
pub struct AdvertisementData {
    pub data: Vec<u8>,
}

impl AdvertisementData {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn add_flags(&mut self, flags: AdvFlags) {
        self.data.push(2); // length
        self.data.push(AdType::Flags as u8);
        self.data.push(flags.as_u8());
    }

    pub fn add_complete_local_name(&mut self, name: &str) {
        let name_bytes = name.as_bytes();
        self.data.push((name_bytes.len() + 1) as u8); // length
        self.data.push(AdType::CompleteLocalName as u8);
        self.data.extend_from_slice(name_bytes);
    }

    pub fn add_shortened_local_name(&mut self, name: &str) {
        let name_bytes = name.as_bytes();
        self.data.push((name_bytes.len() + 1) as u8);
        self.data.push(AdType::ShortenedLocalName as u8);
        self.data.extend_from_slice(name_bytes);
    }

    pub fn add_16bit_service_uuids(&mut self, uuids: &[u16]) {
        let len = uuids.len() * 2 + 1;
        self.data.push(len as u8);
        self.data.push(AdType::CompleteList16BitServiceUuids as u8);
        for uuid in uuids {
            self.data.push((*uuid & 0xff) as u8);
            self.data.push(((*uuid >> 8) & 0xff) as u8);
        }
    }

    pub fn add_appearance(&mut self, appearance: u16) {
        self.data.push(3);
        self.data.push(AdType::Appearance as u8);
        self.data.push((appearance & 0xff) as u8);
        self.data.push(((appearance >> 8) & 0xff) as u8);
    }

    pub fn add_tx_power_level(&mut self, tx_power: i8) {
        self.data.push(2);
        self.data.push(AdType::TxPowerLevel as u8);
        self.data.push(tx_power as u8);
    }

    pub fn add_manufacturer_data(&mut self, company_id: u16, data: &[u8]) {
        let len = data.len() + 3;
        self.data.push(len as u8);
        self.data.push(AdType::ManufacturerSpecificData as u8);
        self.data.push((company_id & 0xff) as u8);
        self.data.push(((company_id >> 8) & 0xff) as u8);
        self.data.extend_from_slice(data);
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_valid(&self) -> bool {
        self.data.len() <= 31
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
}

/// Scan result
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub addr: BleAddr,
    pub rssi: i8,
    pub adv_data: Vec<u8>,
    pub scan_response: Vec<u8>,
    pub connectable: bool,
    pub timestamp_ms: u64,
}

impl ScanResult {
    pub fn local_name(&self) -> Option<String> {
        self.parse_ad_field(AdType::CompleteLocalName as u8)
            .or_else(|| self.parse_ad_field(AdType::ShortenedLocalName as u8))
            .map(|data| String::from_utf8_lossy(data).into_owned())
    }

    pub fn service_uuids_16(&self) -> Vec<u16> {
        let mut uuids = Vec::new();
        if let Some(data) = self.parse_ad_field(AdType::CompleteList16BitServiceUuids as u8)
            .or_else(|| self.parse_ad_field(AdType::IncompleteList16BitServiceUuids as u8))
        {
            for chunk in data.chunks_exact(2) {
                uuids.push(u16::from_le_bytes([chunk[0], chunk[1]]));
            }
        }
        uuids
    }

    pub fn tx_power_level(&self) -> Option<i8> {
        self.parse_ad_field(AdType::TxPowerLevel as u8)
            .map(|data| data[0] as i8)
    }

    fn parse_ad_field(&self, ad_type: u8) -> Option<&[u8]> {
        let mut offset = 0;
        while offset < self.adv_data.len() {
            let len = self.adv_data[offset] as usize;
            if len == 0 || offset + len >= self.adv_data.len() {
                break;
            }
            let field_type = self.adv_data[offset + 1];
            if field_type == ad_type {
                return Some(&self.adv_data[offset + 2..offset + 1 + len]);
            }
            offset += len + 1;
        }

        // Also check scan response
        offset = 0;
        while offset < self.scan_response.len() {
            let len = self.scan_response[offset] as usize;
            if len == 0 || offset + len >= self.scan_response.len() {
                break;
            }
            let field_type = self.scan_response[offset + 1];
            if field_type == ad_type {
                return Some(&self.scan_response[offset + 2..offset + 1 + len]);
            }
            offset += len + 1;
        }

        None
    }
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
}

/// BLE connection
#[derive(Debug, Clone)]
pub struct BleConnection {
    pub handle: u16,
    pub peer_addr: BleAddr,
    pub state: ConnectionState,
    pub conn_interval_ms: f32,
    pub slave_latency: u16,
    pub supervision_timeout_ms: u16,
    pub mtu: u16,
    pub services: Vec<GattService>,
}

impl BleConnection {
    pub fn new(handle: u16, peer_addr: BleAddr) -> Self {
        Self {
            handle,
            peer_addr,
            state: ConnectionState::Connecting,
            conn_interval_ms: 7.5,
            slave_latency: 0,
            supervision_timeout_ms: 4000,
            mtu: 23,
            services: Vec::new(),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }
}

/// BLE manager state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BleState {
    Off,
    Idle,
    Scanning,
    Advertising,
    Connecting,
}

/// BLE statistics
#[derive(Debug)]
pub struct BleStats {
    pub scans_started: AtomicU64,
    pub scan_results: AtomicU64,
    pub adverts_started: AtomicU64,
    pub connections_established: AtomicU64,
    pub connections_closed: AtomicU64,
    pub gatt_reads: AtomicU64,
    pub gatt_writes: AtomicU64,
    pub notifications_sent: AtomicU64,
    pub errors: AtomicU64,
}

impl BleStats {
    const fn new() -> Self {
        Self {
            scans_started: AtomicU64::new(0),
            scan_results: AtomicU64::new(0),
            adverts_started: AtomicU64::new(0),
            connections_established: AtomicU64::new(0),
            connections_closed: AtomicU64::new(0),
            gatt_reads: AtomicU64::new(0),
            gatt_writes: AtomicU64::new(0),
            notifications_sent: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> BleStatsSnapshot {
        BleStatsSnapshot {
            scans_started: self.scans_started.load(Ordering::Relaxed),
            scan_results: self.scan_results.load(Ordering::Relaxed),
            adverts_started: self.adverts_started.load(Ordering::Relaxed),
            connections_established: self.connections_established.load(Ordering::Relaxed),
            connections_closed: self.connections_closed.load(Ordering::Relaxed),
            gatt_reads: self.gatt_reads.load(Ordering::Relaxed),
            gatt_writes: self.gatt_writes.load(Ordering::Relaxed),
            notifications_sent: self.notifications_sent.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BleStatsSnapshot {
    pub scans_started: u64,
    pub scan_results: u64,
    pub adverts_started: u64,
    pub connections_established: u64,
    pub connections_closed: u64,
    pub gatt_reads: u64,
    pub gatt_writes: u64,
    pub notifications_sent: u64,
    pub errors: u64,
}

/// BLE manager
pub struct BleManager {
    state: BleState,
    local_addr: BleAddr,
    connections: Vec<BleConnection>,
    scan_results: Vec<ScanResult>,
    local_services: Vec<GattService>,
    stats: BleStats,
    next_handle: u16,
    initialized: bool,
}

impl BleManager {
    const fn new() -> Self {
        Self {
            state: BleState::Off,
            local_addr: BleAddr { addr: [0; 6], addr_type: AddressType::Public },
            connections: Vec::new(),
            scan_results: Vec::new(),
            local_services: Vec::new(),
            stats: BleStats::new(),
            next_handle: 1,
            initialized: false,
        }
    }

    /// Initialize BLE
    pub fn init(&mut self) {
        if self.initialized {
            return;
        }

        self.state = BleState::Idle;
        self.initialized = true;

        // Add standard GATT services
        self.add_generic_access_service();
        self.add_generic_attribute_service();
    }

    /// Add Generic Access service
    fn add_generic_access_service(&mut self) {
        let mut service = GattService::new(
            self.allocate_handle(),
            0, // Will be updated
            Uuid::uuid16(gatt_services::GENERIC_ACCESS),
            true,
        );

        // Device Name characteristic
        let mut char_device_name = GattCharacteristic::new(
            self.allocate_handle(),
            self.allocate_handle(),
            Uuid::uuid16(gatt_characteristics::DEVICE_NAME),
            CharacteristicProperties::readable(),
        );
        char_device_name.value = b"Stenzel OS".to_vec();
        service.add_characteristic(char_device_name);

        // Appearance characteristic
        let mut char_appearance = GattCharacteristic::new(
            self.allocate_handle(),
            self.allocate_handle(),
            Uuid::uuid16(gatt_characteristics::APPEARANCE),
            CharacteristicProperties::readable(),
        );
        char_appearance.value = vec![0x00, 0x00]; // Unknown
        service.add_characteristic(char_appearance);

        service.end_handle = self.next_handle - 1;
        self.local_services.push(service);
    }

    /// Add Generic Attribute service
    fn add_generic_attribute_service(&mut self) {
        let mut service = GattService::new(
            self.allocate_handle(),
            0,
            Uuid::uuid16(gatt_services::GENERIC_ATTRIBUTE),
            true,
        );

        // Service Changed characteristic
        let char_service_changed = GattCharacteristic::new(
            self.allocate_handle(),
            self.allocate_handle(),
            Uuid::uuid16(gatt_characteristics::SERVICE_CHANGED),
            CharacteristicProperties { indicate: true, ..Default::default() },
        );
        service.add_characteristic(char_service_changed);

        service.end_handle = self.next_handle - 1;
        self.local_services.push(service);
    }

    /// Allocate a handle
    fn allocate_handle(&mut self) -> u16 {
        let handle = self.next_handle;
        self.next_handle += 1;
        handle
    }

    /// Start scanning
    pub fn start_scan(&mut self, _active: bool) -> KResult<()> {
        if self.state != BleState::Idle {
            return Err(KError::Busy);
        }

        self.scan_results.clear();
        self.state = BleState::Scanning;
        self.stats.scans_started.fetch_add(1, Ordering::Relaxed);

        // In real implementation, would send HCI LE Set Scan Parameters
        // and HCI LE Set Scan Enable commands

        Ok(())
    }

    /// Stop scanning
    pub fn stop_scan(&mut self) -> KResult<()> {
        if self.state != BleState::Scanning {
            return Err(KError::Invalid);
        }

        self.state = BleState::Idle;

        // In real implementation, would send HCI LE Set Scan Enable (disable)

        Ok(())
    }

    /// Get scan results
    pub fn scan_results(&self) -> &[ScanResult] {
        &self.scan_results
    }

    /// Clear scan results
    pub fn clear_scan_results(&mut self) {
        self.scan_results.clear();
    }

    /// Start advertising
    pub fn start_advertising(
        &mut self,
        _adv_type: AdvertisingType,
        _adv_data: &AdvertisementData,
        _scan_rsp: Option<&AdvertisementData>,
    ) -> KResult<()> {
        if self.state != BleState::Idle {
            return Err(KError::Busy);
        }

        self.state = BleState::Advertising;
        self.stats.adverts_started.fetch_add(1, Ordering::Relaxed);

        // In real implementation, would send HCI commands:
        // LE Set Advertising Parameters
        // LE Set Advertising Data
        // LE Set Scan Response Data (if provided)
        // LE Set Advertising Enable

        Ok(())
    }

    /// Stop advertising
    pub fn stop_advertising(&mut self) -> KResult<()> {
        if self.state != BleState::Advertising {
            return Err(KError::Invalid);
        }

        self.state = BleState::Idle;

        // In real implementation, would send HCI LE Set Advertising Enable (disable)

        Ok(())
    }

    /// Connect to a device
    pub fn connect(&mut self, addr: &BleAddr) -> KResult<u16> {
        if self.state != BleState::Idle && self.state != BleState::Scanning {
            return Err(KError::Busy);
        }

        // If scanning, stop first
        if self.state == BleState::Scanning {
            self.stop_scan()?;
        }

        self.state = BleState::Connecting;

        // In real implementation, would send HCI LE Create Connection
        // For now, simulate connection
        let handle = self.allocate_handle();
        let conn = BleConnection::new(handle, *addr);
        self.connections.push(conn);
        self.stats.connections_established.fetch_add(1, Ordering::Relaxed);

        self.state = BleState::Idle;

        Ok(handle)
    }

    /// Disconnect
    pub fn disconnect(&mut self, handle: u16) -> KResult<()> {
        let idx = self.connections.iter().position(|c| c.handle == handle)
            .ok_or(KError::NotFound)?;

        self.connections.remove(idx);
        self.stats.connections_closed.fetch_add(1, Ordering::Relaxed);

        // In real implementation, would send HCI Disconnect

        Ok(())
    }

    /// Get connection
    pub fn get_connection(&self, handle: u16) -> Option<&BleConnection> {
        self.connections.iter().find(|c| c.handle == handle)
    }

    /// Get connections
    pub fn connections(&self) -> &[BleConnection] {
        &self.connections
    }

    /// Discover services
    pub fn discover_services(&mut self, _handle: u16) -> KResult<()> {
        // In real implementation, would perform GATT service discovery
        // using ATT Read By Group Type Request

        self.stats.gatt_reads.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Read characteristic
    pub fn read_characteristic(&mut self, _conn_handle: u16, _char_handle: u16) -> KResult<Vec<u8>> {
        self.stats.gatt_reads.fetch_add(1, Ordering::Relaxed);

        // In real implementation, would send ATT Read Request
        Ok(Vec::new())
    }

    /// Write characteristic
    pub fn write_characteristic(
        &mut self,
        _conn_handle: u16,
        _char_handle: u16,
        _data: &[u8],
        _write_with_response: bool,
    ) -> KResult<()> {
        self.stats.gatt_writes.fetch_add(1, Ordering::Relaxed);

        // In real implementation, would send ATT Write Request or Command
        Ok(())
    }

    /// Subscribe to notifications
    pub fn subscribe_notifications(&mut self, _conn_handle: u16, _char_handle: u16) -> KResult<()> {
        // In real implementation, would write to CCCD
        Ok(())
    }

    /// Add a service
    pub fn add_service(&mut self, uuid: Uuid, primary: bool) -> u16 {
        let handle = self.allocate_handle();
        let service = GattService::new(handle, handle, uuid, primary);
        self.local_services.push(service);
        handle
    }

    /// Get local services
    pub fn local_services(&self) -> &[GattService] {
        &self.local_services
    }

    /// Get state
    pub fn state(&self) -> BleState {
        self.state
    }

    /// Get local address
    pub fn local_addr(&self) -> BleAddr {
        self.local_addr
    }

    /// Set local address
    pub fn set_local_addr(&mut self, addr: BleAddr) {
        self.local_addr = addr;
    }

    /// Get stats
    pub fn stats(&self) -> BleStatsSnapshot {
        self.stats.snapshot()
    }

    /// Is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Is scanning
    pub fn is_scanning(&self) -> bool {
        self.state == BleState::Scanning
    }

    /// Is advertising
    pub fn is_advertising(&self) -> bool {
        self.state == BleState::Advertising
    }

    /// Connection count
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }
}

/// Global BLE manager
static BLE: IrqSafeMutex<BleManager> = IrqSafeMutex::new(BleManager::new());

/// Initialize BLE
pub fn init() {
    BLE.lock().init();
}

/// Start scanning
pub fn start_scan(active: bool) -> KResult<()> {
    BLE.lock().start_scan(active)
}

/// Stop scanning
pub fn stop_scan() -> KResult<()> {
    BLE.lock().stop_scan()
}

/// Get scan results
pub fn scan_results() -> Vec<ScanResult> {
    BLE.lock().scan_results().to_vec()
}

/// Start advertising
pub fn start_advertising(
    adv_type: AdvertisingType,
    adv_data: &AdvertisementData,
    scan_rsp: Option<&AdvertisementData>,
) -> KResult<()> {
    BLE.lock().start_advertising(adv_type, adv_data, scan_rsp)
}

/// Stop advertising
pub fn stop_advertising() -> KResult<()> {
    BLE.lock().stop_advertising()
}

/// Connect to device
pub fn connect(addr: &BleAddr) -> KResult<u16> {
    BLE.lock().connect(addr)
}

/// Disconnect
pub fn disconnect(handle: u16) -> KResult<()> {
    BLE.lock().disconnect(handle)
}

/// Is scanning
pub fn is_scanning() -> bool {
    BLE.lock().is_scanning()
}

/// Is advertising
pub fn is_advertising() -> bool {
    BLE.lock().is_advertising()
}

/// State
pub fn state() -> BleState {
    BLE.lock().state()
}

/// Stats
pub fn stats() -> BleStatsSnapshot {
    BLE.lock().stats()
}

/// Is initialized
pub fn is_initialized() -> bool {
    BLE.lock().is_initialized()
}

/// Connection count
pub fn connection_count() -> usize {
    BLE.lock().connection_count()
}

/// Driver name
pub fn driver_name() -> &'static str {
    "ble"
}
