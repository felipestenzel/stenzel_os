//! SHELL32 Emulation
//!
//! Emulates Windows shell32.dll - provides shell functions including:
//! - Special folder paths (CSIDL/KNOWNFOLDERID)
//! - File operations (copy, move, delete, rename)
//! - Shell execution
//! - Drag and drop interfaces
//! - Shell namespace
//! - Icons and shell links
//! - System tray (notification area) icons
//! - Shell dialogs (browse for folder, etc.)

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use core::sync::atomic::{AtomicU64, Ordering};

use super::fs_translate;

/// Handle types
pub type HWND = u64;
pub type HINSTANCE = u64;
pub type HICON = u64;
pub type HKEY = u64;
pub type PIDLIST_ABSOLUTE = u64;
pub type LPITEMIDLIST = u64;
pub type PCIDLIST_ABSOLUTE = u64;

/// Invalid handle
pub const INVALID_HANDLE: u64 = u64::MAX;

/// CSIDL (Constant Special Item ID List) values
pub mod csidl {
    pub const DESKTOP: i32 = 0x0000;
    pub const INTERNET: i32 = 0x0001;
    pub const PROGRAMS: i32 = 0x0002;
    pub const CONTROLS: i32 = 0x0003;
    pub const PRINTERS: i32 = 0x0004;
    pub const PERSONAL: i32 = 0x0005;  // Documents
    pub const FAVORITES: i32 = 0x0006;
    pub const STARTUP: i32 = 0x0007;
    pub const RECENT: i32 = 0x0008;
    pub const SENDTO: i32 = 0x0009;
    pub const BITBUCKET: i32 = 0x000a;  // Recycle Bin
    pub const STARTMENU: i32 = 0x000b;
    pub const MYDOCUMENTS: i32 = 0x000c;
    pub const MYMUSIC: i32 = 0x000d;
    pub const MYVIDEO: i32 = 0x000e;
    pub const DESKTOPDIRECTORY: i32 = 0x0010;
    pub const DRIVES: i32 = 0x0011;  // My Computer
    pub const NETWORK: i32 = 0x0012;
    pub const NETHOOD: i32 = 0x0013;
    pub const FONTS: i32 = 0x0014;
    pub const TEMPLATES: i32 = 0x0015;
    pub const COMMON_STARTMENU: i32 = 0x0016;
    pub const COMMON_PROGRAMS: i32 = 0x0017;
    pub const COMMON_STARTUP: i32 = 0x0018;
    pub const COMMON_DESKTOPDIRECTORY: i32 = 0x0019;
    pub const APPDATA: i32 = 0x001a;
    pub const PRINTHOOD: i32 = 0x001b;
    pub const LOCAL_APPDATA: i32 = 0x001c;
    pub const ALTSTARTUP: i32 = 0x001d;
    pub const COMMON_ALTSTARTUP: i32 = 0x001e;
    pub const COMMON_FAVORITES: i32 = 0x001f;
    pub const INTERNET_CACHE: i32 = 0x0020;
    pub const COOKIES: i32 = 0x0021;
    pub const HISTORY: i32 = 0x0022;
    pub const COMMON_APPDATA: i32 = 0x0023;
    pub const WINDOWS: i32 = 0x0024;
    pub const SYSTEM: i32 = 0x0025;
    pub const PROGRAM_FILES: i32 = 0x0026;
    pub const MYPICTURES: i32 = 0x0027;
    pub const PROFILE: i32 = 0x0028;
    pub const SYSTEMX86: i32 = 0x0029;
    pub const PROGRAM_FILESX86: i32 = 0x002a;
    pub const PROGRAM_FILES_COMMON: i32 = 0x002b;
    pub const PROGRAM_FILES_COMMONX86: i32 = 0x002c;
    pub const COMMON_TEMPLATES: i32 = 0x002d;
    pub const COMMON_DOCUMENTS: i32 = 0x002e;
    pub const COMMON_ADMINTOOLS: i32 = 0x002f;
    pub const ADMINTOOLS: i32 = 0x0030;
    pub const CONNECTIONS: i32 = 0x0031;
    pub const COMMON_MUSIC: i32 = 0x0035;
    pub const COMMON_PICTURES: i32 = 0x0036;
    pub const COMMON_VIDEO: i32 = 0x0037;
    pub const RESOURCES: i32 = 0x0038;
    pub const RESOURCES_LOCALIZED: i32 = 0x0039;
    pub const COMMON_OEM_LINKS: i32 = 0x003a;
    pub const CDBURN_AREA: i32 = 0x003b;
    pub const COMPUTERSNEARME: i32 = 0x003d;

    // Flags
    pub const FLAG_CREATE: i32 = 0x8000;
    pub const FLAG_DONT_VERIFY: i32 = 0x4000;
    pub const FLAG_DONT_UNEXPAND: i32 = 0x2000;
    pub const FLAG_NO_ALIAS: i32 = 0x1000;
    pub const FLAG_PER_USER_INIT: i32 = 0x0800;
}

/// SHFileOperation operations
pub mod fo {
    pub const MOVE: u32 = 0x0001;
    pub const COPY: u32 = 0x0002;
    pub const DELETE: u32 = 0x0003;
    pub const RENAME: u32 = 0x0004;
}

/// SHFileOperation flags
pub mod fof {
    pub const MULTIDESTFILES: u16 = 0x0001;
    pub const CONFIRMMOUSE: u16 = 0x0002;
    pub const SILENT: u16 = 0x0004;
    pub const RENAMEONCOLLISION: u16 = 0x0008;
    pub const NOCONFIRMATION: u16 = 0x0010;
    pub const WANTMAPPINGHANDLE: u16 = 0x0020;
    pub const ALLOWUNDO: u16 = 0x0040;
    pub const FILESONLY: u16 = 0x0080;
    pub const SIMPLEPROGRESS: u16 = 0x0100;
    pub const NOCONFIRMMKDIR: u16 = 0x0200;
    pub const NOERRORUI: u16 = 0x0400;
    pub const NOCOPYSECURITYATTRIBS: u16 = 0x0800;
    pub const NORECURSION: u16 = 0x1000;
    pub const NO_CONNECTED_ELEMENTS: u16 = 0x2000;
    pub const WANTNUKEWARNING: u16 = 0x4000;
    pub const NORECURSEREPARSE: u16 = 0x8000;
}

/// ShellExecute show commands
pub mod sw {
    pub const HIDE: i32 = 0;
    pub const SHOWNORMAL: i32 = 1;
    pub const SHOWMINIMIZED: i32 = 2;
    pub const SHOWMAXIMIZED: i32 = 3;
    pub const SHOWNOACTIVATE: i32 = 4;
    pub const SHOW: i32 = 5;
    pub const MINIMIZE: i32 = 6;
    pub const SHOWMINNOACTIVE: i32 = 7;
    pub const SHOWNA: i32 = 8;
    pub const RESTORE: i32 = 9;
    pub const SHOWDEFAULT: i32 = 10;
    pub const FORCEMINIMIZE: i32 = 11;
}

/// ShellExecute error codes
pub mod se_err {
    pub const FNF: u32 = 2;  // File not found
    pub const PNF: u32 = 3;  // Path not found
    pub const ACCESSDENIED: u32 = 5;
    pub const OOM: u32 = 8;  // Out of memory
    pub const SHARE: u32 = 26;
    pub const ASSOCINCOMPLETE: u32 = 27;
    pub const DDETIMEOUT: u32 = 28;
    pub const DDEFAIL: u32 = 29;
    pub const DDEBUSY: u32 = 30;
    pub const NOASSOC: u32 = 31;
    pub const DLLNOTFOUND: u32 = 32;
}

/// Notify icon message
pub mod nim {
    pub const ADD: u32 = 0x00000000;
    pub const MODIFY: u32 = 0x00000001;
    pub const DELETE: u32 = 0x00000002;
    pub const SETFOCUS: u32 = 0x00000003;
    pub const SETVERSION: u32 = 0x00000004;
}

/// Notify icon flags
pub mod nif {
    pub const MESSAGE: u32 = 0x00000001;
    pub const ICON: u32 = 0x00000002;
    pub const TIP: u32 = 0x00000004;
    pub const STATE: u32 = 0x00000008;
    pub const INFO: u32 = 0x00000010;
    pub const GUID: u32 = 0x00000020;
    pub const REALTIME: u32 = 0x00000040;
    pub const SHOWTIP: u32 = 0x00000080;
}

/// Notify icon info flags
pub mod niif {
    pub const NONE: u32 = 0x00000000;
    pub const INFO: u32 = 0x00000001;
    pub const WARNING: u32 = 0x00000002;
    pub const ERROR: u32 = 0x00000003;
    pub const USER: u32 = 0x00000004;
    pub const NOSOUND: u32 = 0x00000010;
    pub const LARGE_ICON: u32 = 0x00000020;
    pub const RESPECT_QUIET_TIME: u32 = 0x00000080;
}

/// Browse info flags
pub mod bif {
    pub const RETURNONLYFSDIRS: u32 = 0x00000001;
    pub const DONTGOBELOWDOMAIN: u32 = 0x00000002;
    pub const STATUSTEXT: u32 = 0x00000004;
    pub const RETURNFSANCESTORS: u32 = 0x00000008;
    pub const EDITBOX: u32 = 0x00000010;
    pub const VALIDATE: u32 = 0x00000020;
    pub const NEWDIALOGSTYLE: u32 = 0x00000040;
    pub const BROWSEINCLUDEURLS: u32 = 0x00000080;
    pub const USENEWUI: u32 = EDITBOX | NEWDIALOGSTYLE;
    pub const UAHINT: u32 = 0x00000100;
    pub const NONEWFOLDERBUTTON: u32 = 0x00000200;
    pub const NOTRANSLATETARGETS: u32 = 0x00000400;
    pub const BROWSEFORCOMPUTER: u32 = 0x00001000;
    pub const BROWSEFORPRINTER: u32 = 0x00002000;
    pub const BROWSEINCLUDEFILES: u32 = 0x00004000;
    pub const SHAREABLE: u32 = 0x00008000;
    pub const BROWSEFILEJUNCTIONS: u32 = 0x00010000;
}

/// SHFILEOPSTRUCT for SHFileOperation
#[repr(C)]
#[derive(Clone)]
pub struct SHFileOpStruct {
    pub hwnd: HWND,
    pub wfunc: u32,
    pub pfrom: u64,  // Pointer to source paths (double-null terminated)
    pub pto: u64,    // Pointer to destination paths
    pub fflags: u16,
    pub fany_operations_aborted: i32,
    pub hname_mappings: u64,
    pub lpszprogresstitle: u64,
}

/// SHELLEXECUTEINFO structure
#[repr(C)]
#[derive(Clone)]
pub struct ShellExecuteInfo {
    pub cbsize: u32,
    pub fmask: u32,
    pub hwnd: HWND,
    pub lpverb: u64,
    pub lpfile: u64,
    pub lpparameters: u64,
    pub lpdirectory: u64,
    pub nshow: i32,
    pub hinstapp: HINSTANCE,
    pub lpidlist: u64,
    pub lpclass: u64,
    pub hkeyclass: HKEY,
    pub dwhotkey: u32,
    pub hicon_or_hmonitor: u64,
    pub hprocess: u64,
}

/// NOTIFYICONDATA structure
#[repr(C)]
#[derive(Clone)]
pub struct NotifyIconData {
    pub cbsize: u32,
    pub hwnd: HWND,
    pub uid: u32,
    pub uflags: u32,
    pub ucallback_message: u32,
    pub hicon: HICON,
    pub sztip: [u8; 128],
    pub dwstate: u32,
    pub dwstate_mask: u32,
    pub szinfo: [u8; 256],
    pub utimeout_or_version: u32,
    pub szinfotitle: [u8; 64],
    pub dwinfo_flags: u32,
    // GUID would go here in newer versions
}

/// BROWSEINFO structure
#[repr(C)]
#[derive(Clone)]
pub struct BrowseInfo {
    pub hwndowner: HWND,
    pub pidlroot: PCIDLIST_ABSOLUTE,
    pub pszdisplayname: u64,
    pub lpsztitle: u64,
    pub ulflags: u32,
    pub lpfn: u64,
    pub lparam: u64,
    pub iimage: i32,
}

/// Shell link data (for .lnk files)
#[derive(Clone)]
pub struct ShellLinkData {
    pub target_path: String,
    pub arguments: String,
    pub working_dir: String,
    pub description: String,
    pub icon_path: String,
    pub icon_index: i32,
    pub show_cmd: i32,
    pub hotkey: u16,
}

impl Default for ShellLinkData {
    fn default() -> Self {
        Self {
            target_path: String::new(),
            arguments: String::new(),
            working_dir: String::new(),
            description: String::new(),
            icon_path: String::new(),
            icon_index: 0,
            show_cmd: sw::SHOWNORMAL,
            hotkey: 0,
        }
    }
}

/// Notify icon entry
#[derive(Clone)]
pub struct NotifyIcon {
    pub hwnd: HWND,
    pub uid: u32,
    pub flags: u32,
    pub callback_message: u32,
    pub icon: HICON,
    pub tip: String,
    pub visible: bool,
}

/// Shell32 emulator state
pub struct Shell32Emulator {
    /// Next icon handle
    next_icon_id: AtomicU64,
    /// Registered notify icons
    notify_icons: BTreeMap<(HWND, u32), NotifyIcon>,
    /// Shell links
    shell_links: BTreeMap<u64, ShellLinkData>,
    /// Next link handle
    next_link_id: AtomicU64,
    /// Drag-drop data
    drag_data: Option<Vec<String>>,
    /// Exported functions
    exports: BTreeMap<String, u64>,
}

impl Shell32Emulator {
    pub fn new() -> Self {
        let mut emu = Self {
            next_icon_id: AtomicU64::new(1),
            notify_icons: BTreeMap::new(),
            shell_links: BTreeMap::new(),
            next_link_id: AtomicU64::new(1),
            drag_data: None,
            exports: BTreeMap::new(),
        };
        emu.register_exports();
        emu
    }

    fn register_exports(&mut self) {
        let mut addr = 0x7FF80000_u64;

        // Shell folder functions
        self.exports.insert("SHGetSpecialFolderPathA".into(), addr); addr += 0x100;
        self.exports.insert("SHGetSpecialFolderPathW".into(), addr); addr += 0x100;
        self.exports.insert("SHGetFolderPathA".into(), addr); addr += 0x100;
        self.exports.insert("SHGetFolderPathW".into(), addr); addr += 0x100;
        self.exports.insert("SHGetKnownFolderPath".into(), addr); addr += 0x100;
        self.exports.insert("SHGetSpecialFolderLocation".into(), addr); addr += 0x100;
        self.exports.insert("SHGetFolderLocation".into(), addr); addr += 0x100;

        // File operations
        self.exports.insert("SHFileOperationA".into(), addr); addr += 0x100;
        self.exports.insert("SHFileOperationW".into(), addr); addr += 0x100;
        self.exports.insert("SHCreateDirectoryExA".into(), addr); addr += 0x100;
        self.exports.insert("SHCreateDirectoryExW".into(), addr); addr += 0x100;

        // Shell execution
        self.exports.insert("ShellExecuteA".into(), addr); addr += 0x100;
        self.exports.insert("ShellExecuteW".into(), addr); addr += 0x100;
        self.exports.insert("ShellExecuteExA".into(), addr); addr += 0x100;
        self.exports.insert("ShellExecuteExW".into(), addr); addr += 0x100;

        // Shell links (shortcuts)
        self.exports.insert("SHCreateShortcut".into(), addr); addr += 0x100;
        self.exports.insert("SHGetShortcutTarget".into(), addr); addr += 0x100;

        // Icons
        self.exports.insert("ExtractIconA".into(), addr); addr += 0x100;
        self.exports.insert("ExtractIconW".into(), addr); addr += 0x100;
        self.exports.insert("ExtractIconExA".into(), addr); addr += 0x100;
        self.exports.insert("ExtractIconExW".into(), addr); addr += 0x100;
        self.exports.insert("SHGetFileInfoA".into(), addr); addr += 0x100;
        self.exports.insert("SHGetFileInfoW".into(), addr); addr += 0x100;

        // Notify icons (system tray)
        self.exports.insert("Shell_NotifyIconA".into(), addr); addr += 0x100;
        self.exports.insert("Shell_NotifyIconW".into(), addr); addr += 0x100;

        // Browse dialogs
        self.exports.insert("SHBrowseForFolderA".into(), addr); addr += 0x100;
        self.exports.insert("SHBrowseForFolderW".into(), addr); addr += 0x100;
        self.exports.insert("SHGetPathFromIDListA".into(), addr); addr += 0x100;
        self.exports.insert("SHGetPathFromIDListW".into(), addr); addr += 0x100;

        // Item ID list functions
        self.exports.insert("ILCreateFromPathA".into(), addr); addr += 0x100;
        self.exports.insert("ILCreateFromPathW".into(), addr); addr += 0x100;
        self.exports.insert("ILClone".into(), addr); addr += 0x100;
        self.exports.insert("ILFree".into(), addr); addr += 0x100;
        self.exports.insert("ILCombine".into(), addr); addr += 0x100;
        self.exports.insert("ILIsEqual".into(), addr); addr += 0x100;
        self.exports.insert("ILGetSize".into(), addr); addr += 0x100;

        // Drag and drop
        self.exports.insert("DragAcceptFiles".into(), addr); addr += 0x100;
        self.exports.insert("DragQueryFileA".into(), addr); addr += 0x100;
        self.exports.insert("DragQueryFileW".into(), addr); addr += 0x100;
        self.exports.insert("DragQueryPoint".into(), addr); addr += 0x100;
        self.exports.insert("DragFinish".into(), addr); addr += 0x100;

        // Path functions
        self.exports.insert("PathAddBackslashA".into(), addr); addr += 0x100;
        self.exports.insert("PathAddBackslashW".into(), addr); addr += 0x100;
        self.exports.insert("PathAddExtensionA".into(), addr); addr += 0x100;
        self.exports.insert("PathAddExtensionW".into(), addr); addr += 0x100;
        self.exports.insert("PathAppendA".into(), addr); addr += 0x100;
        self.exports.insert("PathAppendW".into(), addr); addr += 0x100;
        self.exports.insert("PathCanonicalizeA".into(), addr); addr += 0x100;
        self.exports.insert("PathCanonicalizeW".into(), addr); addr += 0x100;
        self.exports.insert("PathCombineA".into(), addr); addr += 0x100;
        self.exports.insert("PathCombineW".into(), addr); addr += 0x100;
        self.exports.insert("PathCompactPathA".into(), addr); addr += 0x100;
        self.exports.insert("PathCompactPathW".into(), addr); addr += 0x100;
        self.exports.insert("PathFileExistsA".into(), addr); addr += 0x100;
        self.exports.insert("PathFileExistsW".into(), addr); addr += 0x100;
        self.exports.insert("PathFindExtensionA".into(), addr); addr += 0x100;
        self.exports.insert("PathFindExtensionW".into(), addr); addr += 0x100;
        self.exports.insert("PathFindFileNameA".into(), addr); addr += 0x100;
        self.exports.insert("PathFindFileNameW".into(), addr); addr += 0x100;
        self.exports.insert("PathGetArgsA".into(), addr); addr += 0x100;
        self.exports.insert("PathGetArgsW".into(), addr); addr += 0x100;
        self.exports.insert("PathIsDirectoryA".into(), addr); addr += 0x100;
        self.exports.insert("PathIsDirectoryW".into(), addr); addr += 0x100;
        self.exports.insert("PathIsRelativeA".into(), addr); addr += 0x100;
        self.exports.insert("PathIsRelativeW".into(), addr); addr += 0x100;
        self.exports.insert("PathIsRootA".into(), addr); addr += 0x100;
        self.exports.insert("PathIsRootW".into(), addr); addr += 0x100;
        self.exports.insert("PathIsUNCA".into(), addr); addr += 0x100;
        self.exports.insert("PathIsUNCW".into(), addr); addr += 0x100;
        self.exports.insert("PathMakePrettyA".into(), addr); addr += 0x100;
        self.exports.insert("PathMakePrettyW".into(), addr); addr += 0x100;
        self.exports.insert("PathMatchSpecA".into(), addr); addr += 0x100;
        self.exports.insert("PathMatchSpecW".into(), addr); addr += 0x100;
        self.exports.insert("PathQuoteSpacesA".into(), addr); addr += 0x100;
        self.exports.insert("PathQuoteSpacesW".into(), addr); addr += 0x100;
        self.exports.insert("PathRemoveArgsA".into(), addr); addr += 0x100;
        self.exports.insert("PathRemoveArgsW".into(), addr); addr += 0x100;
        self.exports.insert("PathRemoveBackslashA".into(), addr); addr += 0x100;
        self.exports.insert("PathRemoveBackslashW".into(), addr); addr += 0x100;
        self.exports.insert("PathRemoveBlanksA".into(), addr); addr += 0x100;
        self.exports.insert("PathRemoveBlanksW".into(), addr); addr += 0x100;
        self.exports.insert("PathRemoveExtensionA".into(), addr); addr += 0x100;
        self.exports.insert("PathRemoveExtensionW".into(), addr); addr += 0x100;
        self.exports.insert("PathRemoveFileSpecA".into(), addr); addr += 0x100;
        self.exports.insert("PathRemoveFileSpecW".into(), addr); addr += 0x100;
        self.exports.insert("PathRenameExtensionA".into(), addr); addr += 0x100;
        self.exports.insert("PathRenameExtensionW".into(), addr); addr += 0x100;
        self.exports.insert("PathSearchAndQualifyA".into(), addr); addr += 0x100;
        self.exports.insert("PathSearchAndQualifyW".into(), addr); addr += 0x100;
        self.exports.insert("PathSetDlgItemPathA".into(), addr); addr += 0x100;
        self.exports.insert("PathSetDlgItemPathW".into(), addr); addr += 0x100;
        self.exports.insert("PathStripPathA".into(), addr); addr += 0x100;
        self.exports.insert("PathStripPathW".into(), addr); addr += 0x100;
        self.exports.insert("PathStripToRootA".into(), addr); addr += 0x100;
        self.exports.insert("PathStripToRootW".into(), addr); addr += 0x100;
        self.exports.insert("PathUnquoteSpacesA".into(), addr); addr += 0x100;
        self.exports.insert("PathUnquoteSpacesW".into(), addr); addr += 0x100;

        // String functions
        self.exports.insert("StrCatW".into(), addr); addr += 0x100;
        self.exports.insert("StrChrA".into(), addr); addr += 0x100;
        self.exports.insert("StrChrW".into(), addr); addr += 0x100;
        self.exports.insert("StrCmpNA".into(), addr); addr += 0x100;
        self.exports.insert("StrCmpNW".into(), addr); addr += 0x100;
        self.exports.insert("StrCmpNIA".into(), addr); addr += 0x100;
        self.exports.insert("StrCmpNIW".into(), addr); addr += 0x100;
        self.exports.insert("StrCpyNA".into(), addr); addr += 0x100;
        self.exports.insert("StrCpyNW".into(), addr); addr += 0x100;
        self.exports.insert("StrDupA".into(), addr); addr += 0x100;
        self.exports.insert("StrDupW".into(), addr); addr += 0x100;
        self.exports.insert("StrFormatByteSizeA".into(), addr); addr += 0x100;
        self.exports.insert("StrFormatByteSizeW".into(), addr); addr += 0x100;
        self.exports.insert("StrRChrA".into(), addr); addr += 0x100;
        self.exports.insert("StrRChrW".into(), addr); addr += 0x100;
        self.exports.insert("StrStrA".into(), addr); addr += 0x100;
        self.exports.insert("StrStrW".into(), addr); addr += 0x100;
        self.exports.insert("StrStrIA".into(), addr); addr += 0x100;
        self.exports.insert("StrStrIW".into(), addr); addr += 0x100;
        self.exports.insert("StrToIntA".into(), addr); addr += 0x100;
        self.exports.insert("StrToIntW".into(), addr); addr += 0x100;
        self.exports.insert("StrTrimA".into(), addr); addr += 0x100;
        self.exports.insert("StrTrimW".into(), addr); addr += 0x100;

        // Miscellaneous
        self.exports.insert("SHChangeNotify".into(), addr); addr += 0x100;
        self.exports.insert("SHEmptyRecycleBinA".into(), addr); addr += 0x100;
        self.exports.insert("SHEmptyRecycleBinW".into(), addr); addr += 0x100;
        self.exports.insert("SHQueryRecycleBinA".into(), addr); addr += 0x100;
        self.exports.insert("SHQueryRecycleBinW".into(), addr); addr += 0x100;
        self.exports.insert("SHFormatDrive".into(), addr); addr += 0x100;
        self.exports.insert("SHGetDiskFreeSpaceExA".into(), addr); addr += 0x100;
        self.exports.insert("SHGetDiskFreeSpaceExW".into(), addr); addr += 0x100;
        self.exports.insert("SHGetNewLinkInfoA".into(), addr); addr += 0x100;
        self.exports.insert("SHGetNewLinkInfoW".into(), addr); addr += 0x100;
        self.exports.insert("SHInvokePrinterCommandA".into(), addr); addr += 0x100;
        self.exports.insert("SHInvokePrinterCommandW".into(), addr); addr += 0x100;
        self.exports.insert("SHLoadNonloadedIconOverlayIdentifiers".into(), addr); addr += 0x100;
        self.exports.insert("SHIsFileAvailableOffline".into(), addr); addr += 0x100;
        self.exports.insert("SHSetLocalizedName".into(), addr); addr += 0x100;
        self.exports.insert("SHRemoveLocalizedName".into(), addr); addr += 0x100;
        self.exports.insert("SHGetLocalizedName".into(), addr); addr += 0x100;

        // CommandLineToArgv
        self.exports.insert("CommandLineToArgvW".into(), addr); addr += 0x100;

        // Run dialog
        self.exports.insert("SHRunFileDialog".into(), addr); addr += 0x100;

        // Shell about
        self.exports.insert("ShellAboutA".into(), addr); addr += 0x100;
        self.exports.insert("ShellAboutW".into(), addr); addr += 0x100;

        // FindExecutable
        self.exports.insert("FindExecutableA".into(), addr); addr += 0x100;
        self.exports.insert("FindExecutableW".into(), addr); addr += 0x100;

        // Association
        self.exports.insert("AssocCreate".into(), addr); addr += 0x100;
        self.exports.insert("AssocQueryKeyA".into(), addr); addr += 0x100;
        self.exports.insert("AssocQueryKeyW".into(), addr); addr += 0x100;
        self.exports.insert("AssocQueryStringA".into(), addr); addr += 0x100;
        self.exports.insert("AssocQueryStringW".into(), addr); addr += 0x100;
        self.exports.insert("AssocQueryStringByKeyA".into(), addr); addr += 0x100;
        self.exports.insert("AssocQueryStringByKeyW".into(), addr);
    }

    /// Get export address
    pub fn get_proc_address(&self, name: &str) -> Option<u64> {
        self.exports.get(name).copied()
    }

    /// Get all exports
    pub fn get_exports(&self) -> &BTreeMap<String, u64> {
        &self.exports
    }

    // ==================== Special Folder Functions ====================

    /// Get special folder path (CSIDL)
    pub fn sh_get_special_folder_path(&self, csidl: i32) -> Option<String> {
        let folder = csidl & 0xFF;  // Remove flags

        match folder {
            csidl::DESKTOP => Some("C:\\Users\\User\\Desktop".into()),
            csidl::PERSONAL | csidl::MYDOCUMENTS => Some("C:\\Users\\User\\Documents".into()),
            csidl::FAVORITES => Some("C:\\Users\\User\\Favorites".into()),
            csidl::MYMUSIC => Some("C:\\Users\\User\\Music".into()),
            csidl::MYVIDEO => Some("C:\\Users\\User\\Videos".into()),
            csidl::MYPICTURES => Some("C:\\Users\\User\\Pictures".into()),
            csidl::PROGRAMS => Some("C:\\Users\\User\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs".into()),
            csidl::STARTUP => Some("C:\\Users\\User\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup".into()),
            csidl::RECENT => Some("C:\\Users\\User\\AppData\\Roaming\\Microsoft\\Windows\\Recent".into()),
            csidl::SENDTO => Some("C:\\Users\\User\\AppData\\Roaming\\Microsoft\\Windows\\SendTo".into()),
            csidl::STARTMENU => Some("C:\\Users\\User\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu".into()),
            csidl::DESKTOPDIRECTORY => Some("C:\\Users\\User\\Desktop".into()),
            csidl::APPDATA => Some("C:\\Users\\User\\AppData\\Roaming".into()),
            csidl::LOCAL_APPDATA => Some("C:\\Users\\User\\AppData\\Local".into()),
            csidl::INTERNET_CACHE => Some("C:\\Users\\User\\AppData\\Local\\Microsoft\\Windows\\INetCache".into()),
            csidl::COOKIES => Some("C:\\Users\\User\\AppData\\Local\\Microsoft\\Windows\\INetCookies".into()),
            csidl::HISTORY => Some("C:\\Users\\User\\AppData\\Local\\Microsoft\\Windows\\History".into()),
            csidl::TEMPLATES => Some("C:\\Users\\User\\AppData\\Roaming\\Microsoft\\Windows\\Templates".into()),
            csidl::PROFILE => Some("C:\\Users\\User".into()),
            csidl::FONTS => Some("C:\\Windows\\Fonts".into()),
            csidl::WINDOWS => Some("C:\\Windows".into()),
            csidl::SYSTEM => Some("C:\\Windows\\System32".into()),
            csidl::SYSTEMX86 => Some("C:\\Windows\\SysWOW64".into()),
            csidl::PROGRAM_FILES => Some("C:\\Program Files".into()),
            csidl::PROGRAM_FILESX86 => Some("C:\\Program Files (x86)".into()),
            csidl::PROGRAM_FILES_COMMON => Some("C:\\Program Files\\Common Files".into()),
            csidl::PROGRAM_FILES_COMMONX86 => Some("C:\\Program Files (x86)\\Common Files".into()),
            csidl::COMMON_APPDATA => Some("C:\\ProgramData".into()),
            csidl::COMMON_DOCUMENTS => Some("C:\\Users\\Public\\Documents".into()),
            csidl::COMMON_MUSIC => Some("C:\\Users\\Public\\Music".into()),
            csidl::COMMON_PICTURES => Some("C:\\Users\\Public\\Pictures".into()),
            csidl::COMMON_VIDEO => Some("C:\\Users\\Public\\Videos".into()),
            csidl::COMMON_TEMPLATES => Some("C:\\ProgramData\\Microsoft\\Windows\\Templates".into()),
            csidl::COMMON_STARTMENU => Some("C:\\ProgramData\\Microsoft\\Windows\\Start Menu".into()),
            csidl::COMMON_PROGRAMS => Some("C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs".into()),
            csidl::COMMON_STARTUP => Some("C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Startup".into()),
            csidl::COMMON_DESKTOPDIRECTORY => Some("C:\\Users\\Public\\Desktop".into()),
            csidl::ADMINTOOLS => Some("C:\\Users\\User\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Administrative Tools".into()),
            csidl::COMMON_ADMINTOOLS => Some("C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Administrative Tools".into()),
            csidl::CDBURN_AREA => Some("C:\\Users\\User\\AppData\\Local\\Microsoft\\Windows\\Burn\\Burn".into()),
            csidl::NETHOOD => Some("C:\\Users\\User\\AppData\\Roaming\\Microsoft\\Windows\\Network Shortcuts".into()),
            csidl::PRINTHOOD => Some("C:\\Users\\User\\AppData\\Roaming\\Microsoft\\Windows\\Printer Shortcuts".into()),
            _ => None,
        }
    }

    /// Get folder path - translate to Unix path
    pub fn sh_get_folder_path_unix(&self, csidl: i32) -> Option<String> {
        self.sh_get_special_folder_path(csidl)
            .map(|p| fs_translate::windows_to_unix(&p))
    }

    // ==================== File Operations ====================

    /// Perform shell file operation (simplified)
    pub fn sh_file_operation(&self, op: &SHFileOpStruct) -> i32 {
        // In a real implementation, this would perform the file operation
        // For now, return success
        match op.wfunc {
            fo::COPY => {
                // Would copy files from pfrom to pto
                0
            }
            fo::MOVE => {
                // Would move files from pfrom to pto
                0
            }
            fo::DELETE => {
                // Would delete files in pfrom
                0
            }
            fo::RENAME => {
                // Would rename file from pfrom to pto
                0
            }
            _ => 1,  // Unknown operation
        }
    }

    /// Create directory with all intermediate directories
    pub fn sh_create_directory_ex(&self, _path: &str) -> i32 {
        // Would create directory structure
        // Return ERROR_SUCCESS (0) on success
        0
    }

    // ==================== Shell Execution ====================

    /// Execute a shell command
    pub fn shell_execute(
        &self,
        hwnd: HWND,
        operation: Option<&str>,
        file: &str,
        parameters: Option<&str>,
        directory: Option<&str>,
        show_cmd: i32,
    ) -> u64 {
        // Map operation to action
        let _action = operation.unwrap_or("open");

        // In a real implementation, this would:
        // 1. Find the executable or handler for the file type
        // 2. Execute it with the given parameters
        // 3. Return instance handle on success (> 32) or error code on failure

        let _ = (hwnd, file, parameters, directory, show_cmd);

        // Return success (value > 32)
        33
    }

    /// Execute a shell command (extended)
    pub fn shell_execute_ex(&self, info: &mut ShellExecuteInfo) -> bool {
        // Would perform extended shell execution
        info.hinstapp = 33;  // Success value
        true
    }

    // ==================== Icons ====================

    /// Extract icon from file
    pub fn extract_icon(&self, _file: &str, _icon_index: u32) -> HICON {
        // Return a pseudo icon handle
        self.next_icon_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Extract multiple icons
    pub fn extract_icon_ex(
        &self,
        _file: &str,
        _icon_index: i32,
        large_icons: Option<&mut [HICON]>,
        small_icons: Option<&mut [HICON]>,
    ) -> u32 {
        // Fill in pseudo handles
        let mut count = 0u32;

        if let Some(large) = large_icons {
            for h in large.iter_mut() {
                *h = self.next_icon_id.fetch_add(1, Ordering::Relaxed);
                count += 1;
            }
        }

        if let Some(small) = small_icons {
            for h in small.iter_mut() {
                *h = self.next_icon_id.fetch_add(1, Ordering::Relaxed);
            }
        }

        count
    }

    // ==================== Notify Icons (System Tray) ====================

    /// Add, modify, or delete a notify icon
    pub fn shell_notify_icon(&mut self, message: u32, data: &NotifyIconData) -> bool {
        match message {
            nim::ADD => {
                let icon = NotifyIcon {
                    hwnd: data.hwnd,
                    uid: data.uid,
                    flags: data.uflags,
                    callback_message: data.ucallback_message,
                    icon: data.hicon,
                    tip: bytes_to_string(&data.sztip),
                    visible: true,
                };
                self.notify_icons.insert((data.hwnd, data.uid), icon);
                true
            }
            nim::MODIFY => {
                if let Some(icon) = self.notify_icons.get_mut(&(data.hwnd, data.uid)) {
                    if data.uflags & nif::ICON != 0 {
                        icon.icon = data.hicon;
                    }
                    if data.uflags & nif::TIP != 0 {
                        icon.tip = bytes_to_string(&data.sztip);
                    }
                    if data.uflags & nif::MESSAGE != 0 {
                        icon.callback_message = data.ucallback_message;
                    }
                    true
                } else {
                    false
                }
            }
            nim::DELETE => {
                self.notify_icons.remove(&(data.hwnd, data.uid)).is_some()
            }
            nim::SETFOCUS => true,
            nim::SETVERSION => true,
            _ => false,
        }
    }

    // ==================== Shell Links (Shortcuts) ====================

    /// Create a shell link handle
    pub fn create_shell_link(&mut self) -> u64 {
        let id = self.next_link_id.fetch_add(1, Ordering::Relaxed);
        self.shell_links.insert(id, ShellLinkData::default());
        id
    }

    /// Set shell link target
    pub fn set_link_path(&mut self, link: u64, path: &str) -> bool {
        if let Some(data) = self.shell_links.get_mut(&link) {
            data.target_path = path.into();
            true
        } else {
            false
        }
    }

    /// Set shell link arguments
    pub fn set_link_arguments(&mut self, link: u64, args: &str) -> bool {
        if let Some(data) = self.shell_links.get_mut(&link) {
            data.arguments = args.into();
            true
        } else {
            false
        }
    }

    /// Set shell link working directory
    pub fn set_link_working_dir(&mut self, link: u64, dir: &str) -> bool {
        if let Some(data) = self.shell_links.get_mut(&link) {
            data.working_dir = dir.into();
            true
        } else {
            false
        }
    }

    /// Set shell link description
    pub fn set_link_description(&mut self, link: u64, desc: &str) -> bool {
        if let Some(data) = self.shell_links.get_mut(&link) {
            data.description = desc.into();
            true
        } else {
            false
        }
    }

    /// Set shell link icon
    pub fn set_link_icon(&mut self, link: u64, path: &str, index: i32) -> bool {
        if let Some(data) = self.shell_links.get_mut(&link) {
            data.icon_path = path.into();
            data.icon_index = index;
            true
        } else {
            false
        }
    }

    /// Set shell link show command
    pub fn set_link_show_cmd(&mut self, link: u64, show_cmd: i32) -> bool {
        if let Some(data) = self.shell_links.get_mut(&link) {
            data.show_cmd = show_cmd;
            true
        } else {
            false
        }
    }

    /// Set shell link hotkey
    pub fn set_link_hotkey(&mut self, link: u64, hotkey: u16) -> bool {
        if let Some(data) = self.shell_links.get_mut(&link) {
            data.hotkey = hotkey;
            true
        } else {
            false
        }
    }

    /// Get shell link data
    pub fn get_link_data(&self, link: u64) -> Option<&ShellLinkData> {
        self.shell_links.get(&link)
    }

    /// Save shell link (would serialize to .lnk file format)
    pub fn save_shell_link(&self, _link: u64, _path: &str) -> bool {
        // In real implementation, would write to .lnk file
        true
    }

    /// Release shell link
    pub fn release_shell_link(&mut self, link: u64) {
        self.shell_links.remove(&link);
    }

    // ==================== Drag and Drop ====================

    /// Start accepting dropped files
    pub fn drag_accept_files(&self, _hwnd: HWND, _accept: bool) {
        // Would register window for drag-drop
    }

    /// Set drag data (internal)
    pub fn set_drag_data(&mut self, files: Vec<String>) {
        self.drag_data = Some(files);
    }

    /// Query number of dropped files
    pub fn drag_query_file_count(&self) -> u32 {
        self.drag_data.as_ref().map(|v| v.len() as u32).unwrap_or(0)
    }

    /// Query a dropped file by index
    pub fn drag_query_file(&self, index: u32) -> Option<&str> {
        self.drag_data.as_ref().and_then(|v| v.get(index as usize).map(|s| s.as_str()))
    }

    /// Finish drag operation
    pub fn drag_finish(&mut self) {
        self.drag_data = None;
    }

    // ==================== Path Functions ====================

    /// Add backslash to path if not present
    pub fn path_add_backslash(path: &str) -> String {
        if path.ends_with('\\') || path.ends_with('/') {
            path.into()
        } else {
            let mut p = path.to_string();
            p.push('\\');
            p
        }
    }

    /// Remove backslash from end of path
    pub fn path_remove_backslash(path: &str) -> String {
        let mut p = path.to_string();
        while p.ends_with('\\') || p.ends_with('/') {
            p.pop();
        }
        p
    }

    /// Get file extension
    pub fn path_find_extension(path: &str) -> Option<&str> {
        path.rfind('.').map(|i| &path[i..])
    }

    /// Get file name from path
    pub fn path_find_file_name(path: &str) -> &str {
        let sep = path.rfind(|c| c == '\\' || c == '/');
        match sep {
            Some(i) => &path[i + 1..],
            None => path,
        }
    }

    /// Remove file spec from path (keep directory only)
    pub fn path_remove_file_spec(path: &str) -> String {
        if let Some(i) = path.rfind(|c| c == '\\' || c == '/') {
            path[..i].to_string()
        } else {
            String::new()
        }
    }

    /// Strip path to just filename
    pub fn path_strip_path(path: &str) -> &str {
        Self::path_find_file_name(path)
    }

    /// Check if path is relative
    pub fn path_is_relative(path: &str) -> bool {
        !Self::path_is_root(path)
    }

    /// Check if path is a root (e.g., C:\ or \\server\share)
    pub fn path_is_root(path: &str) -> bool {
        // Check drive letter root
        if path.len() >= 2 && path.chars().nth(1) == Some(':') {
            let c = path.chars().next().unwrap();
            if c.is_ascii_alphabetic() {
                return path.len() == 2 || (path.len() == 3 && path.ends_with('\\'));
            }
        }

        // Check UNC root
        if path.starts_with("\\\\") {
            let rest = &path[2..];
            // Count backslashes after \\server\share
            let slashes = rest.chars().filter(|&c| c == '\\').count();
            return slashes <= 1;
        }

        false
    }

    /// Check if path is UNC
    pub fn path_is_unc(path: &str) -> bool {
        path.starts_with("\\\\")
    }

    /// Check if path is a directory (simplified - checks for trailing backslash)
    pub fn path_is_directory(path: &str) -> bool {
        path.ends_with('\\') || path.ends_with('/')
    }

    /// Combine two paths
    pub fn path_combine(dir: &str, file: &str) -> String {
        let dir = Self::path_add_backslash(dir);
        let file = file.trim_start_matches(|c| c == '\\' || c == '/');
        format!("{}{}", dir, file)
    }

    /// Append path component
    pub fn path_append(path: &str, more: &str) -> String {
        Self::path_combine(path, more)
    }

    /// Canonicalize path (resolve . and ..)
    pub fn path_canonicalize(path: &str) -> String {
        let mut components: Vec<&str> = Vec::new();

        for component in path.split(|c| c == '\\' || c == '/') {
            match component {
                "" | "." => {}
                ".." => {
                    if !components.is_empty() {
                        components.pop();
                    }
                }
                c => components.push(c),
            }
        }

        if path.starts_with("\\\\") {
            format!("\\\\{}", components.join("\\"))
        } else if path.len() >= 2 && path.chars().nth(1) == Some(':') {
            let drive = &path[..2];
            if components.is_empty() {
                format!("{}\\", drive)
            } else {
                format!("{}\\{}", drive, components.join("\\"))
            }
        } else {
            components.join("\\")
        }
    }

    /// Remove extension from path
    pub fn path_remove_extension(path: &str) -> String {
        if let Some(i) = path.rfind('.') {
            // Make sure the dot is in the filename part
            let after_sep = path.rfind(|c| c == '\\' || c == '/').unwrap_or(0);
            if i > after_sep {
                return path[..i].to_string();
            }
        }
        path.to_string()
    }

    /// Rename extension
    pub fn path_rename_extension(path: &str, new_ext: &str) -> String {
        let base = Self::path_remove_extension(path);
        let ext = if new_ext.starts_with('.') { new_ext.to_string() } else { format!(".{}", new_ext) };
        format!("{}{}", base, ext)
    }

    /// Quote spaces in path
    pub fn path_quote_spaces(path: &str) -> String {
        if path.contains(' ') && !path.starts_with('"') {
            format!("\"{}\"", path)
        } else {
            path.to_string()
        }
    }

    /// Unquote path
    pub fn path_unquote_spaces(path: &str) -> String {
        if path.starts_with('"') && path.ends_with('"') {
            path[1..path.len()-1].to_string()
        } else {
            path.to_string()
        }
    }

    /// Check if file exists (simplified)
    pub fn path_file_exists(&self, path: &str) -> bool {
        // Would check filesystem
        let _ = path;
        true  // Assume exists for now
    }

    /// Match path against spec pattern
    pub fn path_match_spec(path: &str, spec: &str) -> bool {
        // Simple wildcard matching
        let path_lower = path.to_lowercase();
        let spec_lower = spec.to_lowercase();

        if spec_lower == "*" || spec_lower == "*.*" {
            return true;
        }

        if let Some(ext) = spec_lower.strip_prefix("*.") {
            if let Some(path_ext) = Self::path_find_extension(&path_lower) {
                return path_ext.trim_start_matches('.') == ext;
            }
        }

        path_lower.contains(&spec_lower)
    }

    // ==================== String Functions ====================

    /// Format byte size as human-readable string
    pub fn str_format_byte_size(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        const TB: u64 = GB * 1024;

        if bytes >= TB {
            format!("{:.2} TB", bytes as f64 / TB as f64)
        } else if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} bytes", bytes)
        }
    }

    /// Parse string to int
    pub fn str_to_int(s: &str) -> i32 {
        s.trim().parse().unwrap_or(0)
    }

    /// Trim whitespace from string
    pub fn str_trim(s: &str) -> &str {
        s.trim()
    }

    // ==================== Recycle Bin ====================

    /// Empty the recycle bin
    pub fn sh_empty_recycle_bin(&self, _hwnd: HWND, _root: Option<&str>, _flags: u32) -> i32 {
        // Would empty recycle bin
        0  // S_OK
    }

    /// Query recycle bin size
    pub fn sh_query_recycle_bin(&self, _root: Option<&str>) -> (u64, u64) {
        // Return (size in bytes, number of items)
        (0, 0)
    }

    // ==================== Miscellaneous ====================

    /// Notify shell of changes
    pub fn sh_change_notify(&self, _event: u32, _flags: u32, _item1: u64, _item2: u64) {
        // Would notify shell of filesystem changes
    }

    /// Show shell about dialog
    pub fn shell_about(&self, _hwnd: HWND, _app_name: &str, _other_stuff: Option<&str>, _icon: HICON) -> bool {
        // Would show about dialog
        true
    }

    /// Find executable for a file
    pub fn find_executable(&self, file: &str, _directory: Option<&str>) -> Option<String> {
        // Map file extensions to executables
        let ext = Self::path_find_extension(file)?.to_lowercase();

        match ext.as_str() {
            ".txt" | ".log" | ".ini" | ".cfg" => Some("C:\\Windows\\System32\\notepad.exe".into()),
            ".doc" | ".docx" => Some("C:\\Program Files\\Microsoft Office\\WINWORD.EXE".into()),
            ".xls" | ".xlsx" => Some("C:\\Program Files\\Microsoft Office\\EXCEL.EXE".into()),
            ".ppt" | ".pptx" => Some("C:\\Program Files\\Microsoft Office\\POWERPNT.EXE".into()),
            ".htm" | ".html" => Some("C:\\Program Files\\Internet Explorer\\iexplore.exe".into()),
            ".pdf" => Some("C:\\Program Files\\Adobe\\Reader\\AcroRd32.exe".into()),
            ".jpg" | ".jpeg" | ".png" | ".gif" | ".bmp" => Some("C:\\Windows\\System32\\mspaint.exe".into()),
            ".mp3" | ".wav" | ".wma" => Some("C:\\Program Files\\Windows Media Player\\wmplayer.exe".into()),
            ".avi" | ".mp4" | ".wmv" | ".mkv" => Some("C:\\Program Files\\Windows Media Player\\wmplayer.exe".into()),
            ".zip" | ".rar" | ".7z" => Some("C:\\Program Files\\WinRAR\\WinRAR.exe".into()),
            ".exe" | ".com" | ".bat" | ".cmd" => Some(file.into()),
            _ => None,
        }
    }

    /// Parse command line to argv
    pub fn command_line_to_argv(cmdline: &str) -> Vec<String> {
        let mut args = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut chars = cmdline.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    in_quotes = !in_quotes;
                }
                ' ' | '\t' if !in_quotes => {
                    if !current.is_empty() {
                        args.push(core::mem::take(&mut current));
                    }
                }
                '\\' => {
                    // Handle backslash escapes
                    let mut backslashes = 1;
                    while chars.peek() == Some(&'\\') {
                        chars.next();
                        backslashes += 1;
                    }

                    if chars.peek() == Some(&'"') {
                        // Backslashes before quote
                        for _ in 0..backslashes / 2 {
                            current.push('\\');
                        }
                        if backslashes % 2 == 1 {
                            current.push(chars.next().unwrap());
                        }
                    } else {
                        for _ in 0..backslashes {
                            current.push('\\');
                        }
                    }
                }
                _ => {
                    current.push(c);
                }
            }
        }

        if !current.is_empty() {
            args.push(current);
        }

        args
    }

    /// Get disk free space
    pub fn sh_get_disk_free_space_ex(
        &self,
        _directory: &str,
    ) -> Option<(u64, u64, u64)> {
        // Return (free bytes available to user, total bytes, total free bytes)
        // Fake values for now
        Some((50 * 1024 * 1024 * 1024, 500 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024))
    }
}

/// Helper function to convert null-terminated byte array to String
fn bytes_to_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

// ==================== Global Instance ====================

use spin::Mutex;
static SHELL32: Mutex<Option<Shell32Emulator>> = Mutex::new(None);

/// Initialize shell32 emulation
pub fn init() {
    let mut shell32 = SHELL32.lock();
    *shell32 = Some(Shell32Emulator::new());
    crate::kprintln!("shell32: initialized (~120 exports)");
}

/// Get shell32 emulator
pub fn with_shell32<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&Shell32Emulator) -> R,
{
    SHELL32.lock().as_ref().map(f)
}

/// Get shell32 emulator (mutable)
pub fn with_shell32_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Shell32Emulator) -> R,
{
    SHELL32.lock().as_mut().map(f)
}

/// Get export address
pub fn get_proc_address(name: &str) -> Option<u64> {
    with_shell32(|s| s.get_proc_address(name)).flatten()
}
