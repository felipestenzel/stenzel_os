//! OLE32 Emulation
//!
//! Emulates Windows ole32.dll - COM/OLE library
//! Provides:
//! - COM (Component Object Model) runtime
//! - IUnknown interface
//! - Class factories and object creation
//! - Reference counting
//! - Apartment model (STA/MTA)
//! - Marshaling and proxies
//! - Structured storage (compound files)
//! - Monikers
//! - Clipboard and drag-drop OLE
//! - Data objects

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Handle types
pub type HRESULT = i32;
pub type ULONG = u32;
pub type LPVOID = u64;
pub type REFCLSID = u64;  // Pointer to CLSID
pub type REFIID = u64;    // Pointer to IID
pub type LPUNKNOWN = u64; // Pointer to IUnknown

// =============================================================================
// HRESULT values
// =============================================================================

pub mod hr {
    use super::HRESULT;

    pub const S_OK: HRESULT = 0;
    pub const S_FALSE: HRESULT = 1;
    pub const E_UNEXPECTED: HRESULT = 0x8000FFFF_u32 as i32;
    pub const E_NOTIMPL: HRESULT = 0x80004001_u32 as i32;
    pub const E_OUTOFMEMORY: HRESULT = 0x8007000E_u32 as i32;
    pub const E_INVALIDARG: HRESULT = 0x80070057_u32 as i32;
    pub const E_NOINTERFACE: HRESULT = 0x80004002_u32 as i32;
    pub const E_POINTER: HRESULT = 0x80004003_u32 as i32;
    pub const E_HANDLE: HRESULT = 0x80070006_u32 as i32;
    pub const E_ABORT: HRESULT = 0x80004004_u32 as i32;
    pub const E_FAIL: HRESULT = 0x80004005_u32 as i32;
    pub const E_ACCESSDENIED: HRESULT = 0x80070005_u32 as i32;
    pub const E_PENDING: HRESULT = 0x8000000A_u32 as i32;

    pub const CLASS_E_NOAGGREGATION: HRESULT = 0x80040110_u32 as i32;
    pub const CLASS_E_CLASSNOTAVAILABLE: HRESULT = 0x80040111_u32 as i32;
    pub const REGDB_E_CLASSNOTREG: HRESULT = 0x80040154_u32 as i32;
    pub const CO_E_NOTINITIALIZED: HRESULT = 0x800401F0_u32 as i32;
    pub const CO_E_ALREADYINITIALIZED: HRESULT = 0x800401F1_u32 as i32;
    pub const RPC_E_CHANGED_MODE: HRESULT = 0x80010106_u32 as i32;

    pub const STG_E_INVALIDFUNCTION: HRESULT = 0x80030001_u32 as i32;
    pub const STG_E_FILENOTFOUND: HRESULT = 0x80030002_u32 as i32;
    pub const STG_E_ACCESSDENIED: HRESULT = 0x80030005_u32 as i32;
    pub const STG_E_INVALIDHANDLE: HRESULT = 0x80030006_u32 as i32;
    pub const STG_E_INSUFFICIENTMEMORY: HRESULT = 0x80030008_u32 as i32;
    pub const STG_E_INVALIDPOINTER: HRESULT = 0x80030009_u32 as i32;
    pub const STG_E_NOMOREFILES: HRESULT = 0x80030012_u32 as i32;
    pub const STG_E_FILEALREADYEXISTS: HRESULT = 0x80030050_u32 as i32;
    pub const STG_E_INVALIDNAME: HRESULT = 0x800300FC_u32 as i32;
    pub const STG_E_UNKNOWN: HRESULT = 0x800300FD_u32 as i32;
    pub const STG_E_UNIMPLEMENTEDFUNCTION: HRESULT = 0x800300FE_u32 as i32;
    pub const STG_E_INVALIDFLAG: HRESULT = 0x800300FF_u32 as i32;

    pub const OLE_E_BLANK: HRESULT = 0x80040007_u32 as i32;
    pub const DV_E_FORMATETC: HRESULT = 0x80040064_u32 as i32;
    pub const DV_E_TYMED: HRESULT = 0x80040069_u32 as i32;

    pub const MK_E_NOOBJECT: HRESULT = 0x800401E5_u32 as i32;
    pub const MK_E_UNAVAILABLE: HRESULT = 0x800401E3_u32 as i32;
    pub const MK_E_SYNTAX: HRESULT = 0x800401E4_u32 as i32;

    /// Check if HRESULT indicates success
    pub fn succeeded(hr: HRESULT) -> bool {
        hr >= 0
    }

    /// Check if HRESULT indicates failure
    pub fn failed(hr: HRESULT) -> bool {
        hr < 0
    }
}

// =============================================================================
// GUID / CLSID / IID
// =============================================================================

/// GUID structure
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl Guid {
    pub const fn new(d1: u32, d2: u16, d3: u16, d4: [u8; 8]) -> Self {
        Self { data1: d1, data2: d2, data3: d3, data4: d4 }
    }

    pub const ZERO: Guid = Guid::new(0, 0, 0, [0; 8]);

    /// Parse GUID from string like "{00000000-0000-0000-0000-000000000000}"
    pub fn from_str(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('{').trim_end_matches('}');
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 5 {
            return None;
        }

        let d1 = u32::from_str_radix(parts[0], 16).ok()?;
        let d2 = u16::from_str_radix(parts[1], 16).ok()?;
        let d3 = u16::from_str_radix(parts[2], 16).ok()?;
        let d4_hi = u16::from_str_radix(parts[3], 16).ok()?;
        let d4_lo = u64::from_str_radix(parts[4], 16).ok()?;

        let mut d4 = [0u8; 8];
        d4[0] = (d4_hi >> 8) as u8;
        d4[1] = d4_hi as u8;
        for i in 0..6 {
            d4[2 + i] = (d4_lo >> (40 - i * 8)) as u8;
        }

        Some(Self { data1: d1, data2: d2, data3: d3, data4: d4 })
    }
}

impl core::fmt::Debug for Guid {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
               self.data1, self.data2, self.data3,
               self.data4[0], self.data4[1],
               self.data4[2], self.data4[3], self.data4[4],
               self.data4[5], self.data4[6], self.data4[7])
    }
}

pub type Clsid = Guid;
pub type Iid = Guid;

// =============================================================================
// Well-known IIDs and CLSIDs
// =============================================================================

pub mod iid {
    use super::Guid;

    pub const IID_IUNKNOWN: Guid = Guid::new(
        0x00000000, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_ICLASSFACTORY: Guid = Guid::new(
        0x00000001, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IDISPATCH: Guid = Guid::new(
        0x00020400, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_ITYPEINFO: Guid = Guid::new(
        0x00020401, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IPERSIST: Guid = Guid::new(
        0x0000010C, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IPERSISTSTREAM: Guid = Guid::new(
        0x00000109, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IPERSISTFILE: Guid = Guid::new(
        0x0000010B, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_ISTREAM: Guid = Guid::new(
        0x0000000C, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_ISTORAGE: Guid = Guid::new(
        0x0000000B, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IDATAOBJECT: Guid = Guid::new(
        0x0000010E, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IDROPSOURCE: Guid = Guid::new(
        0x00000121, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IDROPTARGET: Guid = Guid::new(
        0x00000122, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IENUMVARIANT: Guid = Guid::new(
        0x00020404, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IMONIKER: Guid = Guid::new(
        0x0000000F, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IBINDCTX: Guid = Guid::new(
        0x0000000E, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IRUNNINGOBJECTTABLE: Guid = Guid::new(
        0x00000010, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IMALLOC: Guid = Guid::new(
        0x00000002, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IMALLOCSPY: Guid = Guid::new(
        0x0000001D, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_ILOCKBYTES: Guid = Guid::new(
        0x0000000A, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IENUMSTATSTG: Guid = Guid::new(
        0x0000000D, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IOLEOBJECT: Guid = Guid::new(
        0x00000112, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );

    pub const IID_IOLECLIENTSITE: Guid = Guid::new(
        0x00000118, 0x0000, 0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]
    );
}

// =============================================================================
// CoInit flags
// =============================================================================

pub mod coinit {
    pub const APARTMENTTHREADED: u32 = 0x2;
    pub const MULTITHREADED: u32 = 0x0;
    pub const DISABLE_OLE1DDE: u32 = 0x4;
    pub const SPEED_OVER_MEMORY: u32 = 0x8;
}

// =============================================================================
// CLSCTX flags
// =============================================================================

pub mod clsctx {
    pub const INPROC_SERVER: u32 = 0x1;
    pub const INPROC_HANDLER: u32 = 0x2;
    pub const LOCAL_SERVER: u32 = 0x4;
    pub const INPROC_SERVER16: u32 = 0x8;
    pub const REMOTE_SERVER: u32 = 0x10;
    pub const INPROC_HANDLER16: u32 = 0x20;
    pub const NO_CODE_DOWNLOAD: u32 = 0x400;
    pub const NO_CUSTOM_MARSHAL: u32 = 0x1000;
    pub const ENABLE_CODE_DOWNLOAD: u32 = 0x2000;
    pub const NO_FAILURE_LOG: u32 = 0x4000;
    pub const DISABLE_AAA: u32 = 0x8000;
    pub const ENABLE_AAA: u32 = 0x10000;
    pub const FROM_DEFAULT_CONTEXT: u32 = 0x20000;
    pub const ACTIVATE_32_BIT_SERVER: u32 = 0x40000;
    pub const ACTIVATE_64_BIT_SERVER: u32 = 0x80000;
    pub const ENABLE_CLOAKING: u32 = 0x100000;
    pub const APPCONTAINER: u32 = 0x400000;
    pub const ACTIVATE_AAA_AS_IU: u32 = 0x800000;
    pub const PS_DLL: u32 = 0x80000000;

    pub const ALL: u32 = INPROC_SERVER | INPROC_HANDLER | LOCAL_SERVER | REMOTE_SERVER;
    pub const SERVER: u32 = INPROC_SERVER | LOCAL_SERVER | REMOTE_SERVER;
}

// =============================================================================
// STGM (Storage Mode) flags
// =============================================================================

pub mod stgm {
    pub const READ: u32 = 0x00000000;
    pub const WRITE: u32 = 0x00000001;
    pub const READWRITE: u32 = 0x00000002;
    pub const SHARE_DENY_NONE: u32 = 0x00000040;
    pub const SHARE_DENY_READ: u32 = 0x00000030;
    pub const SHARE_DENY_WRITE: u32 = 0x00000020;
    pub const SHARE_EXCLUSIVE: u32 = 0x00000010;
    pub const PRIORITY: u32 = 0x00040000;
    pub const DELETE_ON_RELEASE: u32 = 0x04000000;
    pub const NOSCRATCH: u32 = 0x00100000;
    pub const CREATE: u32 = 0x00001000;
    pub const CONVERT: u32 = 0x00020000;
    pub const FAILIFTHERE: u32 = 0x00000000;
    pub const NOSNAPSHOT: u32 = 0x00200000;
    pub const DIRECT_SWMR: u32 = 0x00400000;
    pub const TRANSACTED: u32 = 0x00010000;
    pub const SIMPLE: u32 = 0x08000000;
    pub const DIRECT: u32 = 0x00000000;
}

// =============================================================================
// REGCLS (Register Class) flags
// =============================================================================

pub mod regcls {
    pub const SINGLEUSE: u32 = 0;
    pub const MULTIPLEUSE: u32 = 1;
    pub const MULTI_SEPARATE: u32 = 2;
    pub const SUSPENDED: u32 = 4;
    pub const SURROGATE: u32 = 8;
    pub const AGILE: u32 = 0x10;
}

// =============================================================================
// TYMED (Transfer Medium) values
// =============================================================================

pub mod tymed {
    pub const HGLOBAL: u32 = 1;
    pub const FILE: u32 = 2;
    pub const ISTREAM: u32 = 4;
    pub const ISTORAGE: u32 = 8;
    pub const GDI: u32 = 16;
    pub const MFPICT: u32 = 32;
    pub const ENHMF: u32 = 64;
    pub const NULL: u32 = 0;
}

// =============================================================================
// Clipboard format constants
// =============================================================================

pub mod cf {
    pub const TEXT: u32 = 1;
    pub const BITMAP: u32 = 2;
    pub const METAFILEPICT: u32 = 3;
    pub const SYLK: u32 = 4;
    pub const DIF: u32 = 5;
    pub const TIFF: u32 = 6;
    pub const OEMTEXT: u32 = 7;
    pub const DIB: u32 = 8;
    pub const PALETTE: u32 = 9;
    pub const PENDATA: u32 = 10;
    pub const RIFF: u32 = 11;
    pub const WAVE: u32 = 12;
    pub const UNICODETEXT: u32 = 13;
    pub const ENHMETAFILE: u32 = 14;
    pub const HDROP: u32 = 15;
    pub const LOCALE: u32 = 16;
    pub const DIBV5: u32 = 17;
    pub const OWNERDISPLAY: u32 = 0x0080;
    pub const DSPTEXT: u32 = 0x0081;
    pub const DSPBITMAP: u32 = 0x0082;
    pub const DSPMETAFILEPICT: u32 = 0x0083;
    pub const DSPENHMETAFILE: u32 = 0x008E;
    pub const PRIVATEFIRST: u32 = 0x0200;
    pub const PRIVATELAST: u32 = 0x02FF;
    pub const GDIOBJFIRST: u32 = 0x0300;
    pub const GDIOBJLAST: u32 = 0x03FF;
}

// =============================================================================
// DVASPECT values
// =============================================================================

pub mod dvaspect {
    pub const CONTENT: u32 = 1;
    pub const THUMBNAIL: u32 = 2;
    pub const ICON: u32 = 4;
    pub const DOCPRINT: u32 = 8;
}

// =============================================================================
// Structures
// =============================================================================

/// FORMATETC structure
#[repr(C)]
#[derive(Clone)]
pub struct FormatEtc {
    pub cf_format: u16,
    pub ptd: u64,  // DVTARGETDEVICE*
    pub dw_aspect: u32,
    pub lindex: i32,
    pub tymed: u32,
}

/// STGMEDIUM structure
#[repr(C)]
#[derive(Clone)]
pub struct StgMedium {
    pub tymed: u32,
    pub union_member: u64,  // hBitmap, hMetaFilePict, hEnhMetaFile, hGlobal, lpszFileName, pstm, pstg
    pub punk_for_release: u64,  // IUnknown*
}

/// STATSTG structure
#[repr(C)]
#[derive(Clone)]
pub struct StatStg {
    pub pwcs_name: u64,  // LPOLESTR
    pub type_: u32,
    pub cb_size: u64,
    pub mtime: u64,  // FILETIME
    pub ctime: u64,  // FILETIME
    pub atime: u64,  // FILETIME
    pub grfmode: u32,
    pub grflocksupported: u32,
    pub clsid: Guid,
    pub grfstatesbits: u32,
    pub reserved: u32,
}

/// COM object info
#[derive(Clone)]
pub struct ComObject {
    pub clsid: Clsid,
    pub ref_count: u32,
    pub interfaces: Vec<Iid>,
    pub vtable: u64,
}

/// Registered class factory
#[derive(Clone)]
pub struct ClassFactory {
    pub clsid: Clsid,
    pub factory_ptr: u64,
    pub flags: u32,
    pub cookie: u32,
}

/// Apartment model
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ApartmentModel {
    None,
    Sta,  // Single-Threaded Apartment
    Mta,  // Multi-Threaded Apartment
}

/// Storage stream
#[derive(Clone)]
pub struct OleStream {
    pub name: String,
    pub data: Vec<u8>,
    pub position: u64,
}

/// Storage (compound file)
#[derive(Clone)]
pub struct OleStorage {
    pub name: String,
    pub streams: BTreeMap<String, OleStream>,
    pub sub_storages: BTreeMap<String, OleStorage>,
    pub clsid: Guid,
    pub mode: u32,
}

/// Running object entry
#[derive(Clone)]
pub struct RunningObject {
    pub moniker: u64,
    pub object: u64,
    pub time: u64,
    pub cookie: u32,
}

// =============================================================================
// OLE32 Emulator
// =============================================================================

pub struct Ole32Emulator {
    /// Next handle ID
    next_id: AtomicU64,
    /// COM apartment model
    apartment: ApartmentModel,
    /// COM initialization count
    com_init_count: AtomicU32,
    /// Registered class factories
    class_factories: BTreeMap<Clsid, ClassFactory>,
    /// Next class factory cookie
    next_cookie: AtomicU32,
    /// COM objects
    objects: BTreeMap<u64, ComObject>,
    /// Storages
    storages: BTreeMap<u64, OleStorage>,
    /// Running object table
    running_objects: BTreeMap<u32, RunningObject>,
    /// Registered clipboard formats
    clipboard_formats: BTreeMap<String, u32>,
    /// Next clipboard format ID
    next_cf: u32,
    /// Exported functions
    exports: BTreeMap<String, u64>,
}

impl Ole32Emulator {
    pub fn new() -> Self {
        let mut emu = Self {
            next_id: AtomicU64::new(1),
            apartment: ApartmentModel::None,
            com_init_count: AtomicU32::new(0),
            class_factories: BTreeMap::new(),
            next_cookie: AtomicU32::new(1),
            objects: BTreeMap::new(),
            storages: BTreeMap::new(),
            running_objects: BTreeMap::new(),
            clipboard_formats: BTreeMap::new(),
            next_cf: 0xC000,  // Private format range starts at 0xC000
            exports: BTreeMap::new(),
        };
        emu.register_exports();
        emu
    }

    fn alloc_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    fn alloc_cookie(&self) -> u32 {
        self.next_cookie.fetch_add(1, Ordering::Relaxed)
    }

    fn register_exports(&mut self) {
        let mut addr = 0x7FFA_0000_u64;

        // COM initialization
        self.exports.insert("CoInitialize".into(), addr); addr += 0x100;
        self.exports.insert("CoInitializeEx".into(), addr); addr += 0x100;
        self.exports.insert("CoUninitialize".into(), addr); addr += 0x100;
        self.exports.insert("OleInitialize".into(), addr); addr += 0x100;
        self.exports.insert("OleUninitialize".into(), addr); addr += 0x100;

        // Object creation
        self.exports.insert("CoCreateInstance".into(), addr); addr += 0x100;
        self.exports.insert("CoCreateInstanceEx".into(), addr); addr += 0x100;
        self.exports.insert("CoGetClassObject".into(), addr); addr += 0x100;
        self.exports.insert("CoGetInstanceFromFile".into(), addr); addr += 0x100;
        self.exports.insert("CoGetInstanceFromIStorage".into(), addr); addr += 0x100;

        // Class registration
        self.exports.insert("CoRegisterClassObject".into(), addr); addr += 0x100;
        self.exports.insert("CoRevokeClassObject".into(), addr); addr += 0x100;
        self.exports.insert("CoSuspendClassObjects".into(), addr); addr += 0x100;
        self.exports.insert("CoResumeClassObjects".into(), addr); addr += 0x100;
        self.exports.insert("CoAddRefServerProcess".into(), addr); addr += 0x100;
        self.exports.insert("CoReleaseServerProcess".into(), addr); addr += 0x100;

        // Memory management
        self.exports.insert("CoTaskMemAlloc".into(), addr); addr += 0x100;
        self.exports.insert("CoTaskMemRealloc".into(), addr); addr += 0x100;
        self.exports.insert("CoTaskMemFree".into(), addr); addr += 0x100;
        self.exports.insert("CoGetMalloc".into(), addr); addr += 0x100;

        // Interface marshaling
        self.exports.insert("CoMarshalInterface".into(), addr); addr += 0x100;
        self.exports.insert("CoUnmarshalInterface".into(), addr); addr += 0x100;
        self.exports.insert("CoMarshalInterThreadInterfaceInStream".into(), addr); addr += 0x100;
        self.exports.insert("CoGetInterfaceAndReleaseStream".into(), addr); addr += 0x100;
        self.exports.insert("CoReleaseMarshalData".into(), addr); addr += 0x100;
        self.exports.insert("CoDisconnectObject".into(), addr); addr += 0x100;
        self.exports.insert("CoLockObjectExternal".into(), addr); addr += 0x100;
        self.exports.insert("CoGetStandardMarshal".into(), addr); addr += 0x100;
        self.exports.insert("CoGetMarshalSizeMax".into(), addr); addr += 0x100;

        // Thread/apartment
        self.exports.insert("CoGetCurrentProcess".into(), addr); addr += 0x100;
        self.exports.insert("CoGetApartmentType".into(), addr); addr += 0x100;
        self.exports.insert("CoGetContextToken".into(), addr); addr += 0x100;
        self.exports.insert("CoGetObjectContext".into(), addr); addr += 0x100;
        self.exports.insert("CoSwitchCallContext".into(), addr); addr += 0x100;

        // GUID functions
        self.exports.insert("CLSIDFromString".into(), addr); addr += 0x100;
        self.exports.insert("StringFromCLSID".into(), addr); addr += 0x100;
        self.exports.insert("StringFromGUID2".into(), addr); addr += 0x100;
        self.exports.insert("IIDFromString".into(), addr); addr += 0x100;
        self.exports.insert("StringFromIID".into(), addr); addr += 0x100;
        self.exports.insert("CLSIDFromProgID".into(), addr); addr += 0x100;
        self.exports.insert("CLSIDFromProgIDEx".into(), addr); addr += 0x100;
        self.exports.insert("ProgIDFromCLSID".into(), addr); addr += 0x100;
        self.exports.insert("CoCreateGuid".into(), addr); addr += 0x100;
        self.exports.insert("IsEqualGUID".into(), addr); addr += 0x100;
        self.exports.insert("IsEqualIID".into(), addr); addr += 0x100;
        self.exports.insert("IsEqualCLSID".into(), addr); addr += 0x100;

        // Storage functions
        self.exports.insert("StgCreateDocfile".into(), addr); addr += 0x100;
        self.exports.insert("StgCreateDocfileOnILockBytes".into(), addr); addr += 0x100;
        self.exports.insert("StgOpenStorage".into(), addr); addr += 0x100;
        self.exports.insert("StgOpenStorageOnILockBytes".into(), addr); addr += 0x100;
        self.exports.insert("StgIsStorageFile".into(), addr); addr += 0x100;
        self.exports.insert("StgIsStorageILockBytes".into(), addr); addr += 0x100;
        self.exports.insert("StgSetTimes".into(), addr); addr += 0x100;
        self.exports.insert("StgCreateStorageEx".into(), addr); addr += 0x100;
        self.exports.insert("StgOpenStorageEx".into(), addr); addr += 0x100;
        self.exports.insert("WriteClassStg".into(), addr); addr += 0x100;
        self.exports.insert("ReadClassStg".into(), addr); addr += 0x100;
        self.exports.insert("WriteClassStm".into(), addr); addr += 0x100;
        self.exports.insert("ReadClassStm".into(), addr); addr += 0x100;
        self.exports.insert("WriteFmtUserTypeStg".into(), addr); addr += 0x100;
        self.exports.insert("ReadFmtUserTypeStg".into(), addr); addr += 0x100;
        self.exports.insert("GetHGlobalFromILockBytes".into(), addr); addr += 0x100;
        self.exports.insert("CreateILockBytesOnHGlobal".into(), addr); addr += 0x100;
        self.exports.insert("GetHGlobalFromStream".into(), addr); addr += 0x100;
        self.exports.insert("CreateStreamOnHGlobal".into(), addr); addr += 0x100;

        // Moniker functions
        self.exports.insert("CreateFileMoniker".into(), addr); addr += 0x100;
        self.exports.insert("CreateItemMoniker".into(), addr); addr += 0x100;
        self.exports.insert("CreateAntiMoniker".into(), addr); addr += 0x100;
        self.exports.insert("CreatePointerMoniker".into(), addr); addr += 0x100;
        self.exports.insert("CreateGenericComposite".into(), addr); addr += 0x100;
        self.exports.insert("CreateClassMoniker".into(), addr); addr += 0x100;
        self.exports.insert("CreateObjrefMoniker".into(), addr); addr += 0x100;
        self.exports.insert("GetClassFile".into(), addr); addr += 0x100;
        self.exports.insert("MkParseDisplayName".into(), addr); addr += 0x100;
        self.exports.insert("MonikerRelativePathTo".into(), addr); addr += 0x100;
        self.exports.insert("MonikerCommonPrefixWith".into(), addr); addr += 0x100;
        self.exports.insert("BindMoniker".into(), addr); addr += 0x100;
        self.exports.insert("CreateBindCtx".into(), addr); addr += 0x100;
        self.exports.insert("GetRunningObjectTable".into(), addr); addr += 0x100;

        // OLE clipboard
        self.exports.insert("OleSetClipboard".into(), addr); addr += 0x100;
        self.exports.insert("OleGetClipboard".into(), addr); addr += 0x100;
        self.exports.insert("OleFlushClipboard".into(), addr); addr += 0x100;
        self.exports.insert("OleIsCurrentClipboard".into(), addr); addr += 0x100;

        // Drag and drop
        self.exports.insert("RegisterDragDrop".into(), addr); addr += 0x100;
        self.exports.insert("RevokeDragDrop".into(), addr); addr += 0x100;
        self.exports.insert("DoDragDrop".into(), addr); addr += 0x100;

        // Data transfer
        self.exports.insert("OleQueryLinkFromData".into(), addr); addr += 0x100;
        self.exports.insert("OleQueryCreateFromData".into(), addr); addr += 0x100;
        self.exports.insert("OleCreateFromData".into(), addr); addr += 0x100;
        self.exports.insert("OleCreateLinkFromData".into(), addr); addr += 0x100;
        self.exports.insert("OleCreateFromFile".into(), addr); addr += 0x100;
        self.exports.insert("OleCreateLinkToFile".into(), addr); addr += 0x100;
        self.exports.insert("OleCreate".into(), addr); addr += 0x100;
        self.exports.insert("OleCreateLink".into(), addr); addr += 0x100;
        self.exports.insert("OleLoad".into(), addr); addr += 0x100;
        self.exports.insert("OleSave".into(), addr); addr += 0x100;
        self.exports.insert("OleRun".into(), addr); addr += 0x100;

        // OLE verbs
        self.exports.insert("OleDoAutoConvert".into(), addr); addr += 0x100;
        self.exports.insert("OleGetAutoConvert".into(), addr); addr += 0x100;
        self.exports.insert("OleSetAutoConvert".into(), addr); addr += 0x100;
        self.exports.insert("OleRegGetUserType".into(), addr); addr += 0x100;
        self.exports.insert("OleRegGetMiscStatus".into(), addr); addr += 0x100;
        self.exports.insert("OleRegEnumVerbs".into(), addr); addr += 0x100;
        self.exports.insert("OleRegEnumFormatEtc".into(), addr); addr += 0x100;

        // OLE embedding
        self.exports.insert("OleSetContainedObject".into(), addr); addr += 0x100;
        self.exports.insert("OleNoteObjectVisible".into(), addr); addr += 0x100;
        self.exports.insert("OleLockRunning".into(), addr); addr += 0x100;
        self.exports.insert("OleIsRunning".into(), addr); addr += 0x100;
        self.exports.insert("OleGetIconOfFile".into(), addr); addr += 0x100;
        self.exports.insert("OleGetIconOfClass".into(), addr); addr += 0x100;
        self.exports.insert("OleMetafilePictFromIconAndLabel".into(), addr); addr += 0x100;

        // Clipboard format
        self.exports.insert("RegisterClipboardFormatA".into(), addr); addr += 0x100;
        self.exports.insert("RegisterClipboardFormatW".into(), addr); addr += 0x100;
        self.exports.insert("GetClipboardFormatNameA".into(), addr); addr += 0x100;
        self.exports.insert("GetClipboardFormatNameW".into(), addr); addr += 0x100;

        // Free/release functions
        self.exports.insert("ReleaseStgMedium".into(), addr); addr += 0x100;

        // Property functions
        self.exports.insert("PropVariantClear".into(), addr); addr += 0x100;
        self.exports.insert("PropVariantCopy".into(), addr); addr += 0x100;
        self.exports.insert("FreePropVariantArray".into(), addr); addr += 0x100;
        self.exports.insert("PropVariantInit".into(), addr); addr += 0x100;
        self.exports.insert("StgCreatePropSetStg".into(), addr); addr += 0x100;
        self.exports.insert("StgCreatePropStg".into(), addr); addr += 0x100;
        self.exports.insert("StgOpenPropStg".into(), addr); addr += 0x100;
        self.exports.insert("FmtIdToPropStgName".into(), addr); addr += 0x100;
        self.exports.insert("PropStgNameToFmtId".into(), addr); addr += 0x100;

        // Security
        self.exports.insert("CoInitializeSecurity".into(), addr); addr += 0x100;
        self.exports.insert("CoSetProxyBlanket".into(), addr); addr += 0x100;
        self.exports.insert("CoQueryProxyBlanket".into(), addr); addr += 0x100;
        self.exports.insert("CoCopyProxy".into(), addr); addr += 0x100;
        self.exports.insert("CoQueryClientBlanket".into(), addr); addr += 0x100;
        self.exports.insert("CoImpersonateClient".into(), addr); addr += 0x100;
        self.exports.insert("CoRevertToSelf".into(), addr); addr += 0x100;

        // Error handling
        self.exports.insert("CoGetCallContext".into(), addr); addr += 0x100;
        self.exports.insert("CoGetTreatAsClass".into(), addr); addr += 0x100;
        self.exports.insert("CoTreatAsClass".into(), addr); addr += 0x100;

        // Misc
        self.exports.insert("CoFreeUnusedLibraries".into(), addr); addr += 0x100;
        self.exports.insert("CoFreeUnusedLibrariesEx".into(), addr); addr += 0x100;
        self.exports.insert("CoFreeAllLibraries".into(), addr); addr += 0x100;
        self.exports.insert("CoLoadLibrary".into(), addr); addr += 0x100;
        self.exports.insert("CoFreeLibrary".into(), addr); addr += 0x100;
        self.exports.insert("CoGetState".into(), addr); addr += 0x100;
        self.exports.insert("CoSetState".into(), addr); addr += 0x100;
        self.exports.insert("DllGetClassObject".into(), addr); addr += 0x100;
        self.exports.insert("DllCanUnloadNow".into(), addr); addr += 0x100;
        self.exports.insert("DllRegisterServer".into(), addr); addr += 0x100;
        self.exports.insert("DllUnregisterServer".into(), addr);
    }

    /// Get export address
    pub fn get_proc_address(&self, name: &str) -> Option<u64> {
        self.exports.get(name).copied()
    }

    /// Get all exports
    pub fn get_exports(&self) -> &BTreeMap<String, u64> {
        &self.exports
    }

    // ==================== COM Initialization ====================

    /// CoInitialize
    pub fn co_initialize(&mut self) -> HRESULT {
        self.co_initialize_ex(coinit::APARTMENTTHREADED)
    }

    /// CoInitializeEx
    pub fn co_initialize_ex(&mut self, coinit_flags: u32) -> HRESULT {
        let count = self.com_init_count.fetch_add(1, Ordering::SeqCst);

        if count == 0 {
            // First initialization
            if coinit_flags & coinit::APARTMENTTHREADED != 0 {
                self.apartment = ApartmentModel::Sta;
            } else {
                self.apartment = ApartmentModel::Mta;
            }
            hr::S_OK
        } else {
            // Already initialized
            let expected = if coinit_flags & coinit::APARTMENTTHREADED != 0 {
                ApartmentModel::Sta
            } else {
                ApartmentModel::Mta
            };

            if self.apartment == expected {
                hr::S_FALSE  // Already initialized, same mode
            } else {
                hr::RPC_E_CHANGED_MODE  // Different mode
            }
        }
    }

    /// CoUninitialize
    pub fn co_uninitialize(&mut self) {
        let count = self.com_init_count.load(Ordering::SeqCst);
        if count > 0 {
            if self.com_init_count.fetch_sub(1, Ordering::SeqCst) == 1 {
                // Last uninitialization
                self.apartment = ApartmentModel::None;
            }
        }
    }

    /// OleInitialize
    pub fn ole_initialize(&mut self) -> HRESULT {
        self.co_initialize()
    }

    /// OleUninitialize
    pub fn ole_uninitialize(&mut self) {
        self.co_uninitialize()
    }

    /// Check if COM is initialized
    pub fn is_initialized(&self) -> bool {
        self.com_init_count.load(Ordering::SeqCst) > 0
    }

    // ==================== Object Creation ====================

    /// CoCreateInstance (simplified)
    pub fn co_create_instance(
        &mut self,
        rclsid: &Clsid,
        _punk_outer: LPUNKNOWN,
        _cls_context: u32,
        riid: &Iid,
    ) -> Result<u64, HRESULT> {
        if !self.is_initialized() {
            return Err(hr::CO_E_NOTINITIALIZED);
        }

        // Check if class is registered
        if !self.class_factories.contains_key(rclsid) {
            return Err(hr::REGDB_E_CLASSNOTREG);
        }

        // Create object
        let obj_id = self.alloc_id();
        let obj = ComObject {
            clsid: *rclsid,
            ref_count: 1,
            interfaces: vec![iid::IID_IUNKNOWN, *riid],
            vtable: obj_id * 0x1000,  // Pseudo vtable address
        };

        self.objects.insert(obj_id, obj);
        Ok(obj_id)
    }

    /// CoGetClassObject (simplified)
    pub fn co_get_class_object(
        &self,
        rclsid: &Clsid,
        _cls_context: u32,
        _riid: &Iid,
    ) -> Result<u64, HRESULT> {
        if !self.is_initialized() {
            return Err(hr::CO_E_NOTINITIALIZED);
        }

        if let Some(factory) = self.class_factories.get(rclsid) {
            Ok(factory.factory_ptr)
        } else {
            Err(hr::REGDB_E_CLASSNOTREG)
        }
    }

    // ==================== Class Registration ====================

    /// CoRegisterClassObject
    pub fn co_register_class_object(
        &mut self,
        rclsid: &Clsid,
        factory_ptr: u64,
        cls_context: u32,
        flags: u32,
    ) -> Result<u32, HRESULT> {
        if !self.is_initialized() {
            return Err(hr::CO_E_NOTINITIALIZED);
        }

        let cookie = self.alloc_cookie();
        let factory = ClassFactory {
            clsid: *rclsid,
            factory_ptr,
            flags: flags | cls_context,
            cookie,
        };

        self.class_factories.insert(*rclsid, factory);
        Ok(cookie)
    }

    /// CoRevokeClassObject
    pub fn co_revoke_class_object(&mut self, cookie: u32) -> HRESULT {
        // Find and remove by cookie
        let clsid_to_remove = self.class_factories
            .iter()
            .find(|(_, f)| f.cookie == cookie)
            .map(|(c, _)| *c);

        if let Some(clsid) = clsid_to_remove {
            self.class_factories.remove(&clsid);
            hr::S_OK
        } else {
            hr::E_INVALIDARG
        }
    }

    // ==================== Memory Management ====================

    /// CoTaskMemAlloc (returns pseudo address)
    pub fn co_task_mem_alloc(&self, size: usize) -> u64 {
        if size == 0 {
            0
        } else {
            // Return pseudo address (in real implementation would allocate)
            0x10000000 + self.alloc_id() * 0x1000
        }
    }

    /// CoTaskMemFree
    pub fn co_task_mem_free(&self, _ptr: u64) {
        // In real implementation would free memory
    }

    // ==================== GUID Functions ====================

    /// CoCreateGuid (pseudo GUID generation)
    pub fn co_create_guid(&self) -> Guid {
        let id1 = self.alloc_id();
        let id2 = self.alloc_id();
        Guid {
            data1: id1 as u32,
            data2: (id1 >> 32) as u16,
            data3: id2 as u16,
            data4: [
                (id2 >> 16) as u8,
                (id2 >> 24) as u8,
                (id2 >> 32) as u8,
                (id2 >> 40) as u8,
                (id2 >> 48) as u8,
                (id2 >> 56) as u8,
                0x40,  // Version 4
                0x80,  // Variant
            ],
        }
    }

    /// CLSIDFromString
    pub fn clsid_from_string(&self, str_clsid: &str) -> Result<Clsid, HRESULT> {
        Guid::from_str(str_clsid).ok_or(hr::E_INVALIDARG)
    }

    // ==================== Storage ====================

    /// StgCreateDocfile
    pub fn stg_create_docfile(&mut self, name: &str, mode: u32, _reserved: u32) -> Result<u64, HRESULT> {
        let id = self.alloc_id();
        let storage = OleStorage {
            name: name.into(),
            streams: BTreeMap::new(),
            sub_storages: BTreeMap::new(),
            clsid: Guid::ZERO,
            mode,
        };
        self.storages.insert(id, storage);
        Ok(id)
    }

    /// StgOpenStorage
    pub fn stg_open_storage(&mut self, name: &str, mode: u32) -> Result<u64, HRESULT> {
        // In real implementation would open existing file
        // For now, create if doesn't exist
        self.stg_create_docfile(name, mode, 0)
    }

    /// StgIsStorageFile
    pub fn stg_is_storage_file(&self, _name: &str) -> HRESULT {
        // Would check if file is a structured storage file
        hr::S_FALSE
    }

    /// Get storage by handle
    pub fn get_storage(&self, handle: u64) -> Option<&OleStorage> {
        self.storages.get(&handle)
    }

    /// Get storage by handle (mutable)
    pub fn get_storage_mut(&mut self, handle: u64) -> Option<&mut OleStorage> {
        self.storages.get_mut(&handle)
    }

    /// Create stream in storage
    pub fn storage_create_stream(&mut self, storage_handle: u64, name: &str) -> Result<u64, HRESULT> {
        if let Some(storage) = self.storages.get_mut(&storage_handle) {
            if storage.streams.contains_key(name) {
                return Err(hr::STG_E_FILEALREADYEXISTS);
            }

            let stream = OleStream {
                name: name.into(),
                data: Vec::new(),
                position: 0,
            };
            storage.streams.insert(name.into(), stream);

            // Return pseudo stream handle
            Ok(self.alloc_id())
        } else {
            Err(hr::STG_E_INVALIDHANDLE)
        }
    }

    /// Release storage
    pub fn release_storage(&mut self, handle: u64) {
        self.storages.remove(&handle);
    }

    // ==================== Clipboard Format ====================

    /// RegisterClipboardFormat
    pub fn register_clipboard_format(&mut self, format_name: &str) -> u32 {
        if let Some(&cf) = self.clipboard_formats.get(format_name) {
            cf
        } else {
            let cf = self.next_cf;
            self.next_cf += 1;
            self.clipboard_formats.insert(format_name.into(), cf);
            cf
        }
    }

    /// GetClipboardFormatName
    pub fn get_clipboard_format_name(&self, format: u32) -> Option<&str> {
        for (name, &cf) in &self.clipboard_formats {
            if cf == format {
                return Some(name);
            }
        }
        None
    }

    // ==================== Running Object Table ====================

    /// Register in ROT
    pub fn running_object_register(&mut self, object: u64, moniker: u64) -> u32 {
        let cookie = self.alloc_cookie();
        let entry = RunningObject {
            moniker,
            object,
            time: 0,  // Would be current time
            cookie,
        };
        self.running_objects.insert(cookie, entry);
        cookie
    }

    /// Revoke from ROT
    pub fn running_object_revoke(&mut self, cookie: u32) -> HRESULT {
        if self.running_objects.remove(&cookie).is_some() {
            hr::S_OK
        } else {
            hr::E_INVALIDARG
        }
    }

    /// Get from ROT
    pub fn running_object_get(&self, moniker: u64) -> Option<u64> {
        for entry in self.running_objects.values() {
            if entry.moniker == moniker {
                return Some(entry.object);
            }
        }
        None
    }

    // ==================== Object Reference Counting ====================

    /// AddRef
    pub fn add_ref(&mut self, obj_id: u64) -> u32 {
        if let Some(obj) = self.objects.get_mut(&obj_id) {
            obj.ref_count += 1;
            obj.ref_count
        } else {
            0
        }
    }

    /// Release
    pub fn release(&mut self, obj_id: u64) -> u32 {
        let should_remove = if let Some(obj) = self.objects.get_mut(&obj_id) {
            if obj.ref_count > 0 {
                obj.ref_count -= 1;
            }
            obj.ref_count == 0
        } else {
            false
        };

        if should_remove {
            self.objects.remove(&obj_id);
            0
        } else {
            self.objects.get(&obj_id).map(|o| o.ref_count).unwrap_or(0)
        }
    }

    /// QueryInterface
    pub fn query_interface(&self, obj_id: u64, riid: &Iid) -> Result<u64, HRESULT> {
        if let Some(obj) = self.objects.get(&obj_id) {
            // Check if object supports the interface
            if obj.interfaces.contains(riid) || *riid == iid::IID_IUNKNOWN {
                Ok(obj_id)
            } else {
                Err(hr::E_NOINTERFACE)
            }
        } else {
            Err(hr::E_INVALIDARG)
        }
    }
}

// =============================================================================
// Global Instance
// =============================================================================

use spin::Mutex;
static OLE32: Mutex<Option<Ole32Emulator>> = Mutex::new(None);

/// Initialize OLE32 emulation
pub fn init() {
    let mut ole32 = OLE32.lock();
    *ole32 = Some(Ole32Emulator::new());
    crate::kprintln!("ole32: initialized (~150 exports)");
}

/// Get OLE32 emulator
pub fn with_ole32<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&Ole32Emulator) -> R,
{
    OLE32.lock().as_ref().map(f)
}

/// Get OLE32 emulator (mutable)
pub fn with_ole32_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Ole32Emulator) -> R,
{
    OLE32.lock().as_mut().map(f)
}

/// Get export address
pub fn get_proc_address(name: &str) -> Option<u64> {
    with_ole32(|o| o.get_proc_address(name)).flatten()
}

/// Get all exports
pub fn get_exports() -> BTreeMap<String, u64> {
    with_ole32(|o| o.get_exports().clone()).unwrap_or_default()
}
