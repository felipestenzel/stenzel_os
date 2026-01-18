//! COMCTL32 Emulation
//!
//! Emulates Windows comctl32.dll - Common Controls library
//! Provides common GUI controls:
//! - ListView, TreeView, TabControl
//! - StatusBar, ToolBar, Rebar
//! - Progress bar, Trackbar, UpDown
//! - Tooltip, HotKey, Animation
//! - MonthCalendar, DateTime picker
//! - ImageList, PropertySheet
//! - Header, Pager, ComboBoxEx

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use core::sync::atomic::{AtomicU64, Ordering};

/// Handle types
pub type HWND = u64;
pub type HINSTANCE = u64;
pub type HIMAGELIST = u64;
pub type HTREEITEM = u64;
pub type HBITMAP = u64;
pub type HICON = u64;

/// Invalid handle
pub const INVALID_HANDLE: u64 = u64::MAX;

// =============================================================================
// Common Control Classes
// =============================================================================

/// Window class names for common controls
pub mod class {
    pub const LISTVIEW: &str = "SysListView32";
    pub const TREEVIEW: &str = "SysTreeView32";
    pub const TABCONTROL: &str = "SysTabControl32";
    pub const HEADER: &str = "SysHeader32";
    pub const HOTKEY: &str = "msctls_hotkey32";
    pub const PROGRESS: &str = "msctls_progress32";
    pub const STATUSBAR: &str = "msctls_statusbar32";
    pub const TRACKBAR: &str = "msctls_trackbar32";
    pub const UPDOWN: &str = "msctls_updown32";
    pub const ANIMATE: &str = "SysAnimate32";
    pub const MONTHCAL: &str = "SysMonthCal32";
    pub const DATETIMEPICK: &str = "SysDateTimePick32";
    pub const REBAR: &str = "ReBarWindow32";
    pub const TOOLBAR: &str = "ToolbarWindow32";
    pub const TOOLTIP: &str = "tooltips_class32";
    pub const LINK: &str = "SysLink";
    pub const PAGER: &str = "SysPager";
    pub const COMBOBOXEX: &str = "ComboBoxEx32";
    pub const IPADDRESS: &str = "SysIPAddress32";
}

// =============================================================================
// InitCommonControlsEx flags
// =============================================================================

pub mod icc {
    pub const LISTVIEW_CLASSES: u32 = 0x00000001;
    pub const TREEVIEW_CLASSES: u32 = 0x00000002;
    pub const BAR_CLASSES: u32 = 0x00000004;      // Trackbar, StatusBar, ToolBar
    pub const TAB_CLASSES: u32 = 0x00000008;
    pub const UPDOWN_CLASS: u32 = 0x00000010;
    pub const PROGRESS_CLASS: u32 = 0x00000020;
    pub const HOTKEY_CLASS: u32 = 0x00000040;
    pub const ANIMATE_CLASS: u32 = 0x00000080;
    pub const WIN95_CLASSES: u32 = 0x000000FF;
    pub const DATE_CLASSES: u32 = 0x00000100;     // DateTimePicker, MonthCal
    pub const USEREX_CLASSES: u32 = 0x00000200;   // ComboBoxEx
    pub const COOL_CLASSES: u32 = 0x00000400;     // Rebar
    pub const INTERNET_CLASSES: u32 = 0x00000800; // IP Address
    pub const PAGESCROLLER_CLASS: u32 = 0x00001000;
    pub const NATIVEFNTCTL_CLASS: u32 = 0x00002000;
    pub const STANDARD_CLASSES: u32 = 0x00004000;
    pub const LINK_CLASS: u32 = 0x00008000;
}

// =============================================================================
// ListView styles and messages
// =============================================================================

pub mod lvs {
    pub const ICON: u32 = 0x0000;
    pub const REPORT: u32 = 0x0001;
    pub const SMALLICON: u32 = 0x0002;
    pub const LIST: u32 = 0x0003;
    pub const TYPEMASK: u32 = 0x0003;
    pub const SINGLESEL: u32 = 0x0004;
    pub const SHOWSELALWAYS: u32 = 0x0008;
    pub const SORTASCENDING: u32 = 0x0010;
    pub const SORTDESCENDING: u32 = 0x0020;
    pub const SHAREIMAGELISTS: u32 = 0x0040;
    pub const NOLABELWRAP: u32 = 0x0080;
    pub const AUTOARRANGE: u32 = 0x0100;
    pub const EDITLABELS: u32 = 0x0200;
    pub const OWNERDATA: u32 = 0x1000;
    pub const NOSCROLL: u32 = 0x2000;
    pub const ALIGNTOP: u32 = 0x0000;
    pub const ALIGNLEFT: u32 = 0x0800;
    pub const OWNERDRAWFIXED: u32 = 0x0400;
    pub const NOCOLUMNHEADER: u32 = 0x4000;
    pub const NOSORTHEADER: u32 = 0x8000;
}

pub mod lvm {
    pub const FIRST: u32 = 0x1000;
    pub const GETITEMCOUNT: u32 = FIRST + 4;
    pub const GETITEM: u32 = FIRST + 75;  // LVM_GETITEMW
    pub const SETITEM: u32 = FIRST + 76;  // LVM_SETITEMW
    pub const INSERTITEM: u32 = FIRST + 77; // LVM_INSERTITEMW
    pub const DELETEITEM: u32 = FIRST + 8;
    pub const DELETEALLITEMS: u32 = FIRST + 9;
    pub const GETNEXTITEM: u32 = FIRST + 12;
    pub const FINDITEM: u32 = FIRST + 83; // LVM_FINDITEMW
    pub const GETITEMRECT: u32 = FIRST + 14;
    pub const SETITEMPOSITION: u32 = FIRST + 15;
    pub const GETITEMPOSITION: u32 = FIRST + 16;
    pub const GETSTRINGWIDTH: u32 = FIRST + 87; // LVM_GETSTRINGWIDTHW
    pub const HITTEST: u32 = FIRST + 18;
    pub const ENSUREVISIBLE: u32 = FIRST + 19;
    pub const SCROLL: u32 = FIRST + 20;
    pub const REDRAWITEMS: u32 = FIRST + 21;
    pub const ARRANGE: u32 = FIRST + 22;
    pub const EDITLABEL: u32 = FIRST + 118; // LVM_EDITLABELW
    pub const GETEDITCONTROL: u32 = FIRST + 24;
    pub const GETCOLUMN: u32 = FIRST + 95; // LVM_GETCOLUMNW
    pub const SETCOLUMN: u32 = FIRST + 96; // LVM_SETCOLUMNW
    pub const INSERTCOLUMN: u32 = FIRST + 97; // LVM_INSERTCOLUMNW
    pub const DELETECOLUMN: u32 = FIRST + 28;
    pub const GETCOLUMNWIDTH: u32 = FIRST + 29;
    pub const SETCOLUMNWIDTH: u32 = FIRST + 30;
    pub const GETHEADER: u32 = FIRST + 31;
    pub const SETIMAGELIST: u32 = FIRST + 3;
    pub const GETIMAGELIST: u32 = FIRST + 2;
    pub const GETITEMCOUNT_: u32 = FIRST + 4;
    pub const SORTITEMS: u32 = FIRST + 48;
    pub const SETBKCOLOR: u32 = FIRST + 1;
    pub const GETBKCOLOR: u32 = FIRST + 0;
    pub const SETTEXTCOLOR: u32 = FIRST + 36;
    pub const GETTEXTCOLOR: u32 = FIRST + 35;
    pub const SETTEXTBKCOLOR: u32 = FIRST + 38;
    pub const GETTEXTBKCOLOR: u32 = FIRST + 37;
    pub const GETTOPINDEX: u32 = FIRST + 39;
    pub const GETCOUNTPERPAGE: u32 = FIRST + 40;
    pub const GETORIGIN: u32 = FIRST + 41;
    pub const UPDATE: u32 = FIRST + 42;
    pub const SETITEMSTATE: u32 = FIRST + 43;
    pub const GETITEMSTATE: u32 = FIRST + 44;
    pub const GETITEMTEXT: u32 = FIRST + 115; // LVM_GETITEMTEXTW
    pub const SETITEMTEXT: u32 = FIRST + 116; // LVM_SETITEMTEXTW
    pub const SETITEMCOUNT: u32 = FIRST + 47;
    pub const GETSELECTEDCOUNT: u32 = FIRST + 50;
    pub const GETITEMSPACING: u32 = FIRST + 51;
    pub const SETICONSPACING: u32 = FIRST + 53;
    pub const SETEXTENDEDLISTVIEWSTYLE: u32 = FIRST + 54;
    pub const GETEXTENDEDLISTVIEWSTYLE: u32 = FIRST + 55;
    pub const GETSUBITEMRECT: u32 = FIRST + 56;
    pub const SUBITEMHITTEST: u32 = FIRST + 57;
    pub const SETCOLUMNORDERARRAY: u32 = FIRST + 58;
    pub const GETCOLUMNORDERARRAY: u32 = FIRST + 59;
    pub const SETHOTITEM: u32 = FIRST + 60;
    pub const GETHOTITEM: u32 = FIRST + 61;
    pub const SETHOTCURSOR: u32 = FIRST + 62;
    pub const GETHOTCURSOR: u32 = FIRST + 63;
    pub const APPROXIMATEVIEWRECT: u32 = FIRST + 64;
}

pub mod lvif {
    pub const TEXT: u32 = 0x0001;
    pub const IMAGE: u32 = 0x0002;
    pub const PARAM: u32 = 0x0004;
    pub const STATE: u32 = 0x0008;
    pub const INDENT: u32 = 0x0010;
    pub const NORECOMPUTE: u32 = 0x0800;
    pub const GROUPID: u32 = 0x0100;
    pub const COLUMNS: u32 = 0x0200;
    pub const COLFMT: u32 = 0x00010000;
}

// =============================================================================
// TreeView styles and messages
// =============================================================================

pub mod tvs {
    pub const HASBUTTONS: u32 = 0x0001;
    pub const HASLINES: u32 = 0x0002;
    pub const LINESATROOT: u32 = 0x0004;
    pub const EDITLABELS: u32 = 0x0008;
    pub const DISABLEDRAGDROP: u32 = 0x0010;
    pub const SHOWSELALWAYS: u32 = 0x0020;
    pub const RTLREADING: u32 = 0x0040;
    pub const NOTOOLTIPS: u32 = 0x0080;
    pub const CHECKBOXES: u32 = 0x0100;
    pub const TRACKSELECT: u32 = 0x0200;
    pub const SINGLEEXPAND: u32 = 0x0400;
    pub const INFOTIP: u32 = 0x0800;
    pub const FULLROWSELECT: u32 = 0x1000;
    pub const NOSCROLL: u32 = 0x2000;
    pub const NONEVENHEIGHT: u32 = 0x4000;
    pub const NOHSCROLL: u32 = 0x8000;
}

pub mod tvm {
    pub const FIRST: u32 = 0x1100;
    pub const INSERTITEM: u32 = FIRST + 50; // TVM_INSERTITEMW
    pub const DELETEITEM: u32 = FIRST + 1;
    pub const EXPAND: u32 = FIRST + 2;
    pub const GETITEMRECT: u32 = FIRST + 4;
    pub const GETCOUNT: u32 = FIRST + 5;
    pub const GETINDENT: u32 = FIRST + 6;
    pub const SETINDENT: u32 = FIRST + 7;
    pub const GETIMAGELIST: u32 = FIRST + 8;
    pub const SETIMAGELIST: u32 = FIRST + 9;
    pub const GETNEXTITEM: u32 = FIRST + 10;
    pub const SELECTITEM: u32 = FIRST + 11;
    pub const GETITEM: u32 = FIRST + 62; // TVM_GETITEMW
    pub const SETITEM: u32 = FIRST + 63; // TVM_SETITEMW
    pub const EDITLABEL: u32 = FIRST + 65; // TVM_EDITLABELW
    pub const GETEDITCONTROL: u32 = FIRST + 15;
    pub const GETVISIBLECOUNT: u32 = FIRST + 16;
    pub const HITTEST: u32 = FIRST + 17;
    pub const CREATEDRAGIMAGE: u32 = FIRST + 18;
    pub const SORTCHILDREN: u32 = FIRST + 19;
    pub const ENSUREVISIBLE: u32 = FIRST + 20;
    pub const SORTCHILDRENCB: u32 = FIRST + 21;
    pub const ENDEDITLABELNOW: u32 = FIRST + 22;
    pub const GETISEARCHSTRING: u32 = FIRST + 64; // TVM_GETISEARCHSTRINGW
    pub const SETTOOLTIPS: u32 = FIRST + 24;
    pub const GETTOOLTIPS: u32 = FIRST + 25;
    pub const SETINSERTMARK: u32 = FIRST + 26;
    pub const SETITEMHEIGHT: u32 = FIRST + 27;
    pub const GETITEMHEIGHT: u32 = FIRST + 28;
    pub const SETBKCOLOR: u32 = FIRST + 29;
    pub const SETTEXTCOLOR: u32 = FIRST + 30;
    pub const GETBKCOLOR: u32 = FIRST + 31;
    pub const GETTEXTCOLOR: u32 = FIRST + 32;
    pub const SETSCROLLTIME: u32 = FIRST + 33;
    pub const GETSCROLLTIME: u32 = FIRST + 34;
}

pub mod tvi {
    pub const ROOT: u64 = 0xFFFF0000;
    pub const FIRST: u64 = 0xFFFF0001;
    pub const LAST: u64 = 0xFFFF0002;
    pub const SORT: u64 = 0xFFFF0003;
}

pub mod tvif {
    pub const TEXT: u32 = 0x0001;
    pub const IMAGE: u32 = 0x0002;
    pub const PARAM: u32 = 0x0004;
    pub const STATE: u32 = 0x0008;
    pub const HANDLE: u32 = 0x0010;
    pub const SELECTEDIMAGE: u32 = 0x0020;
    pub const CHILDREN: u32 = 0x0040;
    pub const INTEGRAL: u32 = 0x0080;
    pub const STATEEX: u32 = 0x0100;
    pub const EXPANDEDIMAGE: u32 = 0x0200;
}

// =============================================================================
// Tab Control
// =============================================================================

pub mod tcs {
    pub const SCROLLOPPOSITE: u32 = 0x0001;
    pub const BOTTOM: u32 = 0x0002;
    pub const RIGHT: u32 = 0x0002;
    pub const MULTISELECT: u32 = 0x0004;
    pub const FLATBUTTONS: u32 = 0x0008;
    pub const FORCEICONLEFT: u32 = 0x0010;
    pub const FORCELABELLEFT: u32 = 0x0020;
    pub const HOTTRACK: u32 = 0x0040;
    pub const VERTICAL: u32 = 0x0080;
    pub const TABS: u32 = 0x0000;
    pub const BUTTONS: u32 = 0x0100;
    pub const SINGLELINE: u32 = 0x0000;
    pub const MULTILINE: u32 = 0x0200;
    pub const RIGHTJUSTIFY: u32 = 0x0000;
    pub const FIXEDWIDTH: u32 = 0x0400;
    pub const RAGGEDRIGHT: u32 = 0x0800;
    pub const FOCUSONBUTTONDOWN: u32 = 0x1000;
    pub const OWNERDRAWFIXED: u32 = 0x2000;
    pub const TOOLTIPS: u32 = 0x4000;
    pub const FOCUSNEVER: u32 = 0x8000;
}

pub mod tcm {
    pub const FIRST: u32 = 0x1300;
    pub const GETIMAGELIST: u32 = FIRST + 2;
    pub const SETIMAGELIST: u32 = FIRST + 3;
    pub const GETITEMCOUNT: u32 = FIRST + 4;
    pub const GETITEM: u32 = FIRST + 60; // TCM_GETITEMW
    pub const SETITEM: u32 = FIRST + 61; // TCM_SETITEMW
    pub const INSERTITEM: u32 = FIRST + 62; // TCM_INSERTITEMW
    pub const DELETEITEM: u32 = FIRST + 8;
    pub const DELETEALLITEMS: u32 = FIRST + 9;
    pub const GETITEMRECT: u32 = FIRST + 10;
    pub const GETCURSEL: u32 = FIRST + 11;
    pub const SETCURSEL: u32 = FIRST + 12;
    pub const HITTEST: u32 = FIRST + 13;
    pub const SETITEMEXTRA: u32 = FIRST + 14;
    pub const ADJUSTRECT: u32 = FIRST + 40;
    pub const SETITEMSIZE: u32 = FIRST + 41;
    pub const REMOVEIMAGE: u32 = FIRST + 42;
    pub const SETPADDING: u32 = FIRST + 43;
    pub const GETROWCOUNT: u32 = FIRST + 44;
    pub const GETTOOLTIPS: u32 = FIRST + 45;
    pub const SETTOOLTIPS: u32 = FIRST + 46;
    pub const GETCURFOCUS: u32 = FIRST + 47;
    pub const SETCURFOCUS: u32 = FIRST + 48;
    pub const SETMINTABWIDTH: u32 = FIRST + 49;
    pub const DESELECTALL: u32 = FIRST + 50;
    pub const HIGHLIGHTITEM: u32 = FIRST + 51;
    pub const SETEXTENDEDSTYLE: u32 = FIRST + 52;
    pub const GETEXTENDEDSTYLE: u32 = FIRST + 53;
}

// =============================================================================
// StatusBar
// =============================================================================

pub mod sbars {
    pub const SIZEGRIP: u32 = 0x0100;
    pub const TOOLTIPS: u32 = 0x0800;
}

pub mod sb {
    pub const SETTEXT: u32 = 0x0400 + 11;  // SB_SETTEXTW
    pub const GETTEXT: u32 = 0x0400 + 13;  // SB_GETTEXTW
    pub const GETTEXTLENGTH: u32 = 0x0400 + 12; // SB_GETTEXTLENGTHW
    pub const SETPARTS: u32 = 0x0400 + 4;
    pub const GETPARTS: u32 = 0x0400 + 6;
    pub const GETBORDERS: u32 = 0x0400 + 7;
    pub const SETMINHEIGHT: u32 = 0x0400 + 8;
    pub const SIMPLE: u32 = 0x0400 + 9;
    pub const GETRECT: u32 = 0x0400 + 10;
    pub const ISSIMPLE: u32 = 0x0400 + 14;
    pub const SETICON: u32 = 0x0400 + 15;
    pub const SETTIPTEXT: u32 = 0x0400 + 17; // SB_SETTIPTEXTW
    pub const GETTIPTEXT: u32 = 0x0400 + 19; // SB_GETTIPTEXTW
    pub const GETICON: u32 = 0x0400 + 20;
    pub const SETUNICODEFORMAT: u32 = 0x2005;
    pub const GETUNICODEFORMAT: u32 = 0x2006;
    pub const SETBKCOLOR: u32 = 0x2001;
}

// =============================================================================
// Toolbar
// =============================================================================

pub mod tbstyle {
    pub const BUTTON: u32 = 0x0000;
    pub const SEP: u32 = 0x0001;
    pub const CHECK: u32 = 0x0002;
    pub const GROUP: u32 = 0x0004;
    pub const CHECKGROUP: u32 = CHECK | GROUP;
    pub const DROPDOWN: u32 = 0x0008;
    pub const AUTOSIZE: u32 = 0x0010;
    pub const NOPREFIX: u32 = 0x0020;

    // Toolbar styles
    pub const TOOLTIPS: u32 = 0x0100;
    pub const WRAPABLE: u32 = 0x0200;
    pub const ALTDRAG: u32 = 0x0400;
    pub const FLAT: u32 = 0x0800;
    pub const LIST: u32 = 0x1000;
    pub const CUSTOMERASE: u32 = 0x2000;
    pub const REGISTERDROP: u32 = 0x4000;
    pub const TRANSPARENT: u32 = 0x8000;
}

pub mod tb {
    pub const ENABLEBUTTON: u32 = 0x0400 + 1;
    pub const CHECKBUTTON: u32 = 0x0400 + 2;
    pub const PRESSBUTTON: u32 = 0x0400 + 3;
    pub const HIDEBUTTON: u32 = 0x0400 + 4;
    pub const INDETERMINATE: u32 = 0x0400 + 5;
    pub const MARKBUTTON: u32 = 0x0400 + 6;
    pub const ISBUTTONENABLED: u32 = 0x0400 + 9;
    pub const ISBUTTONCHECKED: u32 = 0x0400 + 10;
    pub const ISBUTTONPRESSED: u32 = 0x0400 + 11;
    pub const ISBUTTONHIDDEN: u32 = 0x0400 + 12;
    pub const ISBUTTONINDETERMINATE: u32 = 0x0400 + 13;
    pub const ISBUTTONHIGHLIGHTED: u32 = 0x0400 + 14;
    pub const SETSTATE: u32 = 0x0400 + 17;
    pub const GETSTATE: u32 = 0x0400 + 18;
    pub const ADDBITMAP: u32 = 0x0400 + 19;
    pub const ADDBUTTONS: u32 = 0x0400 + 68; // TB_ADDBUTTONSW
    pub const INSERTBUTTON: u32 = 0x0400 + 67; // TB_INSERTBUTTONW
    pub const DELETEBUTTON: u32 = 0x0400 + 22;
    pub const GETBUTTON: u32 = 0x0400 + 23;
    pub const BUTTONCOUNT: u32 = 0x0400 + 24;
    pub const COMMANDTOINDEX: u32 = 0x0400 + 25;
    pub const SAVERESTOREW: u32 = 0x0400 + 76;
    pub const CUSTOMIZE: u32 = 0x0400 + 27;
    pub const ADDSTRING: u32 = 0x0400 + 77; // TB_ADDSTRINGW
    pub const GETITEMRECT: u32 = 0x0400 + 29;
    pub const BUTTONSTRUCTSIZE: u32 = 0x0400 + 30;
    pub const SETBUTTONSIZE: u32 = 0x0400 + 31;
    pub const SETBITMAPSIZE: u32 = 0x0400 + 32;
    pub const AUTOSIZE: u32 = 0x0400 + 33;
    pub const GETTOOLTIPS: u32 = 0x0400 + 35;
    pub const SETTOOLTIPS: u32 = 0x0400 + 36;
    pub const SETPARENT: u32 = 0x0400 + 37;
    pub const SETROWS: u32 = 0x0400 + 39;
    pub const GETROWS: u32 = 0x0400 + 40;
    pub const GETBITMAPFLAGS: u32 = 0x0400 + 41;
    pub const SETCMDID: u32 = 0x0400 + 42;
    pub const CHANGEBITMAP: u32 = 0x0400 + 43;
    pub const GETBITMAP: u32 = 0x0400 + 44;
    pub const GETBUTTONTEXT: u32 = 0x0400 + 75; // TB_GETBUTTONTEXTW
    pub const REPLACEBITMAP: u32 = 0x0400 + 46;
    pub const SETINDENT: u32 = 0x0400 + 47;
    pub const SETIMAGELIST: u32 = 0x0400 + 48;
    pub const GETIMAGELIST: u32 = 0x0400 + 49;
    pub const LOADIMAGES: u32 = 0x0400 + 50;
    pub const GETRECT: u32 = 0x0400 + 51;
    pub const SETHOTIMAGELIST: u32 = 0x0400 + 52;
    pub const GETHOTIMAGELIST: u32 = 0x0400 + 53;
    pub const SETDISABLEDIMAGELIST: u32 = 0x0400 + 54;
    pub const GETDISABLEDIMAGELIST: u32 = 0x0400 + 55;
    pub const SETSTYLE: u32 = 0x0400 + 56;
    pub const GETSTYLE: u32 = 0x0400 + 57;
    pub const GETBUTTONSIZE: u32 = 0x0400 + 58;
    pub const SETBUTTONWIDTH: u32 = 0x0400 + 59;
    pub const SETMAXTEXTROWS: u32 = 0x0400 + 60;
    pub const GETTEXTROWS: u32 = 0x0400 + 61;
    pub const GETOBJECT: u32 = 0x0400 + 62;
    pub const GETHOTITEM: u32 = 0x0400 + 71;
    pub const SETHOTITEM: u32 = 0x0400 + 72;
    pub const SETANCHORHIGHLIGHT: u32 = 0x0400 + 73;
    pub const GETANCHORHIGHLIGHT: u32 = 0x0400 + 74;
    pub const MAPACCELERATOR: u32 = 0x0400 + 90; // TB_MAPACCELERATORW
    pub const GETINSERTMARK: u32 = 0x0400 + 79;
    pub const SETINSERTMARK: u32 = 0x0400 + 80;
    pub const INSERTMARKHITTEST: u32 = 0x0400 + 81;
    pub const MOVEBUTTON: u32 = 0x0400 + 82;
    pub const GETMAXSIZE: u32 = 0x0400 + 83;
    pub const SETEXTENDEDSTYLE: u32 = 0x0400 + 84;
    pub const GETEXTENDEDSTYLE: u32 = 0x0400 + 85;
    pub const GETPADDING: u32 = 0x0400 + 86;
    pub const SETPADDING: u32 = 0x0400 + 87;
}

// =============================================================================
// Progress bar
// =============================================================================

pub mod pbs {
    pub const SMOOTH: u32 = 0x01;
    pub const VERTICAL: u32 = 0x04;
    pub const MARQUEE: u32 = 0x08;
    pub const SMOOTHREVERSE: u32 = 0x10;
}

pub mod pbm {
    pub const SETRANGE: u32 = 0x0400 + 1;
    pub const SETPOS: u32 = 0x0400 + 2;
    pub const DELTAPOS: u32 = 0x0400 + 3;
    pub const SETSTEP: u32 = 0x0400 + 4;
    pub const STEPIT: u32 = 0x0400 + 5;
    pub const SETRANGE32: u32 = 0x0400 + 6;
    pub const GETRANGE: u32 = 0x0400 + 7;
    pub const GETPOS: u32 = 0x0400 + 8;
    pub const SETBARCOLOR: u32 = 0x0400 + 9;
    pub const SETBKCOLOR: u32 = 0x2001;
    pub const SETMARQUEE: u32 = 0x0400 + 10;
    pub const GETSTEP: u32 = 0x0400 + 13;
    pub const GETBKCOLOR: u32 = 0x0400 + 14;
    pub const GETBARCOLOR: u32 = 0x0400 + 15;
    pub const SETSTATE: u32 = 0x0400 + 16;
    pub const GETSTATE: u32 = 0x0400 + 17;
}

// =============================================================================
// Trackbar (Slider)
// =============================================================================

pub mod tbs {
    pub const AUTOTICKS: u32 = 0x0001;
    pub const VERT: u32 = 0x0002;
    pub const HORZ: u32 = 0x0000;
    pub const TOP: u32 = 0x0004;
    pub const BOTTOM: u32 = 0x0000;
    pub const LEFT: u32 = 0x0004;
    pub const RIGHT: u32 = 0x0000;
    pub const BOTH: u32 = 0x0008;
    pub const NOTICKS: u32 = 0x0010;
    pub const ENABLESELRANGE: u32 = 0x0020;
    pub const FIXEDLENGTH: u32 = 0x0040;
    pub const NOTHUMB: u32 = 0x0080;
    pub const TOOLTIPS: u32 = 0x0100;
    pub const REVERSED: u32 = 0x0200;
    pub const DOWNISLEFT: u32 = 0x0400;
    pub const NOTIFYBEFOREMOVE: u32 = 0x0800;
    pub const TRANSPARENTBKGND: u32 = 0x1000;
}

pub mod tbm {
    pub const GETPOS: u32 = 0x0400;
    pub const GETRANGEMIN: u32 = 0x0400 + 1;
    pub const GETRANGEMAX: u32 = 0x0400 + 2;
    pub const GETTIC: u32 = 0x0400 + 3;
    pub const SETTIC: u32 = 0x0400 + 4;
    pub const SETPOS: u32 = 0x0400 + 5;
    pub const SETRANGE: u32 = 0x0400 + 6;
    pub const SETRANGEMIN: u32 = 0x0400 + 7;
    pub const SETRANGEMAX: u32 = 0x0400 + 8;
    pub const CLEARTICS: u32 = 0x0400 + 9;
    pub const SETSEL: u32 = 0x0400 + 10;
    pub const SETSELSTART: u32 = 0x0400 + 11;
    pub const SETSELEND: u32 = 0x0400 + 12;
    pub const GETPTICS: u32 = 0x0400 + 14;
    pub const GETTICPOS: u32 = 0x0400 + 15;
    pub const GETNUMTICS: u32 = 0x0400 + 16;
    pub const GETSELSTART: u32 = 0x0400 + 17;
    pub const GETSELEND: u32 = 0x0400 + 18;
    pub const CLEARSEL: u32 = 0x0400 + 19;
    pub const SETTICFREQ: u32 = 0x0400 + 20;
    pub const SETPAGESIZE: u32 = 0x0400 + 21;
    pub const GETPAGESIZE: u32 = 0x0400 + 22;
    pub const SETLINESIZE: u32 = 0x0400 + 23;
    pub const GETLINESIZE: u32 = 0x0400 + 24;
    pub const GETTHUMBRECT: u32 = 0x0400 + 25;
    pub const GETCHANNELRECT: u32 = 0x0400 + 26;
    pub const SETTHUMBLENGTH: u32 = 0x0400 + 27;
    pub const GETTHUMBLENGTH: u32 = 0x0400 + 28;
    pub const SETTOOLTIPS: u32 = 0x0400 + 29;
    pub const GETTOOLTIPS: u32 = 0x0400 + 30;
    pub const SETTIPSIDE: u32 = 0x0400 + 31;
    pub const SETBUDDY: u32 = 0x0400 + 32;
    pub const GETBUDDY: u32 = 0x0400 + 33;
    pub const SETPOSNOTIFY: u32 = 0x0400 + 34;
    pub const SETUNICODEFORMAT: u32 = 0x2005;
    pub const GETUNICODEFORMAT: u32 = 0x2006;
}

// =============================================================================
// UpDown (Spinner)
// =============================================================================

pub mod uds {
    pub const WRAP: u32 = 0x0001;
    pub const SETBUDDYINT: u32 = 0x0002;
    pub const ALIGNRIGHT: u32 = 0x0004;
    pub const ALIGNLEFT: u32 = 0x0008;
    pub const AUTOBUDDY: u32 = 0x0010;
    pub const ARROWKEYS: u32 = 0x0020;
    pub const HORZ: u32 = 0x0040;
    pub const NOTHOUSANDS: u32 = 0x0080;
    pub const HOTTRACK: u32 = 0x0100;
}

pub mod udm {
    pub const SETRANGE: u32 = 0x0400 + 101;
    pub const GETRANGE: u32 = 0x0400 + 102;
    pub const SETPOS: u32 = 0x0400 + 103;
    pub const GETPOS: u32 = 0x0400 + 104;
    pub const SETBUDDY: u32 = 0x0400 + 105;
    pub const GETBUDDY: u32 = 0x0400 + 106;
    pub const SETACCEL: u32 = 0x0400 + 107;
    pub const GETACCEL: u32 = 0x0400 + 108;
    pub const SETBASE: u32 = 0x0400 + 109;
    pub const GETBASE: u32 = 0x0400 + 110;
    pub const SETRANGE32: u32 = 0x0400 + 111;
    pub const GETRANGE32: u32 = 0x0400 + 112;
    pub const SETUNICODEFORMAT: u32 = 0x2005;
    pub const GETUNICODEFORMAT: u32 = 0x2006;
    pub const SETPOS32: u32 = 0x0400 + 113;
    pub const GETPOS32: u32 = 0x0400 + 114;
}

// =============================================================================
// ImageList
// =============================================================================

pub mod ilc {
    pub const MASK: u32 = 0x00000001;
    pub const COLOR: u32 = 0x00000000;
    pub const COLORDDB: u32 = 0x000000FE;
    pub const COLOR4: u32 = 0x00000004;
    pub const COLOR8: u32 = 0x00000008;
    pub const COLOR16: u32 = 0x00000010;
    pub const COLOR24: u32 = 0x00000018;
    pub const COLOR32: u32 = 0x00000020;
    pub const PALETTE: u32 = 0x00000800;
    pub const MIRROR: u32 = 0x00002000;
    pub const PERITEMMIRROR: u32 = 0x00008000;
    pub const ORIGINALSIZE: u32 = 0x00010000;
    pub const HIGHQUALITYSCALE: u32 = 0x00020000;
}

pub mod ild {
    pub const NORMAL: u32 = 0x00000000;
    pub const TRANSPARENT: u32 = 0x00000001;
    pub const MASK: u32 = 0x00000010;
    pub const IMAGE: u32 = 0x00000020;
    pub const ROP: u32 = 0x00000040;
    pub const BLEND25: u32 = 0x00000002;
    pub const BLEND50: u32 = 0x00000004;
    pub const OVERLAYMASK: u32 = 0x00000F00;
    pub const PRESERVEALPHA: u32 = 0x00001000;
    pub const SCALE: u32 = 0x00002000;
    pub const DPISCALE: u32 = 0x00004000;
    pub const ASYNC: u32 = 0x00008000;
    pub const SELECTED: u32 = BLEND50;
    pub const FOCUS: u32 = BLEND25;
    pub const BLEND: u32 = BLEND50;
}

// =============================================================================
// Tooltip
// =============================================================================

pub mod tts {
    pub const ALWAYSTIP: u32 = 0x01;
    pub const NOPREFIX: u32 = 0x02;
    pub const NOANIMATE: u32 = 0x10;
    pub const NOFADE: u32 = 0x20;
    pub const BALLOON: u32 = 0x40;
    pub const CLOSE: u32 = 0x80;
    pub const USEVISUALSTYLE: u32 = 0x100;
}

pub mod ttm {
    pub const ACTIVATE: u32 = 0x0400 + 1;
    pub const SETDELAYTIME: u32 = 0x0400 + 3;
    pub const ADDTOOL: u32 = 0x0400 + 50; // TTM_ADDTOOLW
    pub const DELTOOL: u32 = 0x0400 + 51; // TTM_DELTOOLW
    pub const NEWTOOLRECT: u32 = 0x0400 + 52; // TTM_NEWTOOLRECTW
    pub const RELAYEVENT: u32 = 0x0400 + 7;
    pub const GETTOOLINFO: u32 = 0x0400 + 53; // TTM_GETTOOLINFOW
    pub const SETTOOLINFO: u32 = 0x0400 + 54; // TTM_SETTOOLINFOW
    pub const HITTEST: u32 = 0x0400 + 55; // TTM_HITTESTW
    pub const GETTEXT: u32 = 0x0400 + 56; // TTM_GETTEXTW
    pub const UPDATETIPTEXT: u32 = 0x0400 + 57; // TTM_UPDATETIPTEXTW
    pub const GETTOOLCOUNT: u32 = 0x0400 + 13;
    pub const ENUMTOOLS: u32 = 0x0400 + 58; // TTM_ENUMTOOLSW
    pub const GETCURRENTTOOL: u32 = 0x0400 + 59; // TTM_GETCURRENTTOOLW
    pub const WINDOWFROMPOINT: u32 = 0x0400 + 16;
    pub const TRACKACTIVATE: u32 = 0x0400 + 17;
    pub const TRACKPOSITION: u32 = 0x0400 + 18;
    pub const SETTIPBKCOLOR: u32 = 0x0400 + 19;
    pub const SETTIPTEXTCOLOR: u32 = 0x0400 + 20;
    pub const GETDELAYTIME: u32 = 0x0400 + 21;
    pub const GETTIPBKCOLOR: u32 = 0x0400 + 22;
    pub const GETTIPTEXTCOLOR: u32 = 0x0400 + 23;
    pub const SETMAXTIPWIDTH: u32 = 0x0400 + 24;
    pub const GETMAXTIPWIDTH: u32 = 0x0400 + 25;
    pub const SETMARGIN: u32 = 0x0400 + 26;
    pub const GETMARGIN: u32 = 0x0400 + 27;
    pub const POP: u32 = 0x0400 + 28;
    pub const UPDATE: u32 = 0x0400 + 29;
    pub const GETBUBBLESIZE: u32 = 0x0400 + 30;
    pub const ADJUSTRECT: u32 = 0x0400 + 31;
    pub const SETTITLE: u32 = 0x0400 + 33; // TTM_SETTITLEW
    pub const POPUP: u32 = 0x0400 + 34;
    pub const GETTITLE: u32 = 0x0400 + 35;
}

// =============================================================================
// Structures
// =============================================================================

/// INITCOMMONCONTROLSEX structure
#[repr(C)]
#[derive(Clone, Copy)]
pub struct InitCommonControlsEx {
    pub dw_size: u32,
    pub dw_icc: u32,
}

/// ImageList item
#[derive(Clone)]
pub struct ImageListItem {
    pub index: i32,
    pub width: i32,
    pub height: i32,
}

/// ImageList
#[derive(Clone)]
pub struct ImageList {
    pub id: u64,
    pub cx: i32,
    pub cy: i32,
    pub flags: u32,
    pub count: i32,
    pub grow: i32,
    pub items: Vec<ImageListItem>,
}

/// TreeView item
#[derive(Clone)]
pub struct TreeViewItem {
    pub handle: HTREEITEM,
    pub parent: HTREEITEM,
    pub text: String,
    pub image: i32,
    pub selected_image: i32,
    pub state: u32,
    pub lparam: u64,
    pub children: Vec<HTREEITEM>,
}

/// ListView item
#[derive(Clone)]
pub struct ListViewItem {
    pub index: i32,
    pub subitem: i32,
    pub text: String,
    pub image: i32,
    pub state: u32,
    pub lparam: u64,
}

/// Tab item
#[derive(Clone)]
pub struct TabItem {
    pub index: i32,
    pub text: String,
    pub image: i32,
    pub lparam: u64,
}

// =============================================================================
// COMCTL32 Emulator
// =============================================================================

pub struct Comctl32Emulator {
    /// Next handle ID
    next_id: AtomicU64,
    /// Image lists
    image_lists: BTreeMap<HIMAGELIST, ImageList>,
    /// TreeView items
    treeview_items: BTreeMap<HWND, BTreeMap<HTREEITEM, TreeViewItem>>,
    /// ListView items
    listview_items: BTreeMap<HWND, Vec<ListViewItem>>,
    /// Tab items
    tab_items: BTreeMap<HWND, Vec<TabItem>>,
    /// Initialized control classes
    initialized_classes: u32,
    /// Exported functions
    exports: BTreeMap<String, u64>,
}

impl Comctl32Emulator {
    pub fn new() -> Self {
        let mut emu = Self {
            next_id: AtomicU64::new(1),
            image_lists: BTreeMap::new(),
            treeview_items: BTreeMap::new(),
            listview_items: BTreeMap::new(),
            tab_items: BTreeMap::new(),
            initialized_classes: 0,
            exports: BTreeMap::new(),
        };
        emu.register_exports();
        emu
    }

    fn alloc_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    fn register_exports(&mut self) {
        let mut addr = 0x7FF9_0000_u64;

        // Initialization
        self.exports.insert("InitCommonControls".into(), addr); addr += 0x100;
        self.exports.insert("InitCommonControlsEx".into(), addr); addr += 0x100;

        // ImageList functions
        self.exports.insert("ImageList_Create".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_Destroy".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_GetImageCount".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_SetImageCount".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_Add".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_ReplaceIcon".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_SetBkColor".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_GetBkColor".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_SetOverlayImage".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_Draw".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_DrawEx".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_DrawIndirect".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_Replace".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_AddMasked".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_Remove".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_GetIcon".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_LoadImageA".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_LoadImageW".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_Copy".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_BeginDrag".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_EndDrag".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_DragEnter".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_DragLeave".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_DragMove".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_SetDragCursorImage".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_DragShowNolock".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_GetDragImage".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_Read".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_Write".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_GetIconSize".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_SetIconSize".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_GetImageInfo".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_Merge".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_Duplicate".into(), addr); addr += 0x100;
        self.exports.insert("ImageList_CoCreateInstance".into(), addr); addr += 0x100;
        self.exports.insert("HIMAGELIST_QueryInterface".into(), addr); addr += 0x100;

        // Property sheet functions
        self.exports.insert("PropertySheetA".into(), addr); addr += 0x100;
        self.exports.insert("PropertySheetW".into(), addr); addr += 0x100;
        self.exports.insert("CreatePropertySheetPageA".into(), addr); addr += 0x100;
        self.exports.insert("CreatePropertySheetPageW".into(), addr); addr += 0x100;
        self.exports.insert("DestroyPropertySheetPage".into(), addr); addr += 0x100;

        // Control creation helpers
        self.exports.insert("CreateStatusWindowA".into(), addr); addr += 0x100;
        self.exports.insert("CreateStatusWindowW".into(), addr); addr += 0x100;
        self.exports.insert("CreateToolbarEx".into(), addr); addr += 0x100;
        self.exports.insert("CreateMappedBitmap".into(), addr); addr += 0x100;
        self.exports.insert("CreateUpDownControl".into(), addr); addr += 0x100;

        // Drawing functions
        self.exports.insert("DrawStatusTextA".into(), addr); addr += 0x100;
        self.exports.insert("DrawStatusTextW".into(), addr); addr += 0x100;
        self.exports.insert("DrawInsert".into(), addr); addr += 0x100;
        self.exports.insert("GetEffectiveClientRect".into(), addr); addr += 0x100;

        // Menu functions
        self.exports.insert("MenuHelp".into(), addr); addr += 0x100;
        self.exports.insert("ShowHideMenuCtl".into(), addr); addr += 0x100;

        // Flat scrollbar
        self.exports.insert("FlatSB_EnableScrollBar".into(), addr); addr += 0x100;
        self.exports.insert("FlatSB_ShowScrollBar".into(), addr); addr += 0x100;
        self.exports.insert("FlatSB_GetScrollRange".into(), addr); addr += 0x100;
        self.exports.insert("FlatSB_GetScrollInfo".into(), addr); addr += 0x100;
        self.exports.insert("FlatSB_GetScrollPos".into(), addr); addr += 0x100;
        self.exports.insert("FlatSB_GetScrollProp".into(), addr); addr += 0x100;
        self.exports.insert("FlatSB_SetScrollPos".into(), addr); addr += 0x100;
        self.exports.insert("FlatSB_SetScrollInfo".into(), addr); addr += 0x100;
        self.exports.insert("FlatSB_SetScrollRange".into(), addr); addr += 0x100;
        self.exports.insert("FlatSB_SetScrollProp".into(), addr); addr += 0x100;
        self.exports.insert("InitializeFlatSB".into(), addr); addr += 0x100;
        self.exports.insert("UninitializeFlatSB".into(), addr); addr += 0x100;

        // MRU (Most Recently Used) functions
        self.exports.insert("CreateMRUListA".into(), addr); addr += 0x100;
        self.exports.insert("CreateMRUListW".into(), addr); addr += 0x100;
        self.exports.insert("FreeMRUList".into(), addr); addr += 0x100;
        self.exports.insert("AddMRUStringA".into(), addr); addr += 0x100;
        self.exports.insert("AddMRUStringW".into(), addr); addr += 0x100;
        self.exports.insert("EnumMRUListA".into(), addr); addr += 0x100;
        self.exports.insert("EnumMRUListW".into(), addr); addr += 0x100;
        self.exports.insert("FindMRUStringA".into(), addr); addr += 0x100;
        self.exports.insert("FindMRUStringW".into(), addr); addr += 0x100;
        self.exports.insert("DelMRUString".into(), addr); addr += 0x100;

        // DPA (Dynamic Pointer Array) functions
        self.exports.insert("DPA_Create".into(), addr); addr += 0x100;
        self.exports.insert("DPA_CreateEx".into(), addr); addr += 0x100;
        self.exports.insert("DPA_Destroy".into(), addr); addr += 0x100;
        self.exports.insert("DPA_Clone".into(), addr); addr += 0x100;
        self.exports.insert("DPA_GetPtr".into(), addr); addr += 0x100;
        self.exports.insert("DPA_GetPtrIndex".into(), addr); addr += 0x100;
        self.exports.insert("DPA_InsertPtr".into(), addr); addr += 0x100;
        self.exports.insert("DPA_SetPtr".into(), addr); addr += 0x100;
        self.exports.insert("DPA_DeletePtr".into(), addr); addr += 0x100;
        self.exports.insert("DPA_DeleteAllPtrs".into(), addr); addr += 0x100;
        self.exports.insert("DPA_Sort".into(), addr); addr += 0x100;
        self.exports.insert("DPA_Search".into(), addr); addr += 0x100;
        self.exports.insert("DPA_EnumCallback".into(), addr); addr += 0x100;
        self.exports.insert("DPA_DestroyCallback".into(), addr); addr += 0x100;
        self.exports.insert("DPA_Grow".into(), addr); addr += 0x100;
        self.exports.insert("DPA_Merge".into(), addr); addr += 0x100;
        self.exports.insert("DPA_SaveStream".into(), addr); addr += 0x100;
        self.exports.insert("DPA_LoadStream".into(), addr); addr += 0x100;

        // DSA (Dynamic Structure Array) functions
        self.exports.insert("DSA_Create".into(), addr); addr += 0x100;
        self.exports.insert("DSA_Destroy".into(), addr); addr += 0x100;
        self.exports.insert("DSA_GetItem".into(), addr); addr += 0x100;
        self.exports.insert("DSA_GetItemPtr".into(), addr); addr += 0x100;
        self.exports.insert("DSA_SetItem".into(), addr); addr += 0x100;
        self.exports.insert("DSA_InsertItem".into(), addr); addr += 0x100;
        self.exports.insert("DSA_DeleteItem".into(), addr); addr += 0x100;
        self.exports.insert("DSA_DeleteAllItems".into(), addr); addr += 0x100;
        self.exports.insert("DSA_EnumCallback".into(), addr); addr += 0x100;
        self.exports.insert("DSA_DestroyCallback".into(), addr); addr += 0x100;
        self.exports.insert("DSA_Clone".into(), addr); addr += 0x100;
        self.exports.insert("DSA_Sort".into(), addr); addr += 0x100;

        // String functions
        self.exports.insert("Str_SetPtrA".into(), addr); addr += 0x100;
        self.exports.insert("Str_SetPtrW".into(), addr); addr += 0x100;

        // Subclassing
        self.exports.insert("SetWindowSubclass".into(), addr); addr += 0x100;
        self.exports.insert("GetWindowSubclass".into(), addr); addr += 0x100;
        self.exports.insert("RemoveWindowSubclass".into(), addr); addr += 0x100;
        self.exports.insert("DefSubclassProc".into(), addr); addr += 0x100;

        // Task dialog
        self.exports.insert("TaskDialogIndirect".into(), addr); addr += 0x100;
        self.exports.insert("TaskDialog".into(), addr); addr += 0x100;

        // Load icon metric
        self.exports.insert("LoadIconMetric".into(), addr); addr += 0x100;
        self.exports.insert("LoadIconWithScaleDown".into(), addr); addr += 0x100;

        // Misc
        self.exports.insert("GetMUILanguage".into(), addr); addr += 0x100;
        self.exports.insert("InitMUILanguage".into(), addr); addr += 0x100;
        self.exports.insert("_TrackMouseEvent".into(), addr); addr += 0x100;
        self.exports.insert("MakeDragList".into(), addr); addr += 0x100;
        self.exports.insert("LBItemFromPt".into(), addr); addr += 0x100;
        self.exports.insert("DrawShadowText".into(), addr);
    }

    /// Get export address
    pub fn get_proc_address(&self, name: &str) -> Option<u64> {
        self.exports.get(name).copied()
    }

    /// Get all exports
    pub fn get_exports(&self) -> &BTreeMap<String, u64> {
        &self.exports
    }

    // ==================== Initialization ====================

    /// Initialize common controls (old-style)
    pub fn init_common_controls(&mut self) {
        self.initialized_classes = icc::WIN95_CLASSES;
    }

    /// Initialize common controls (extended)
    pub fn init_common_controls_ex(&mut self, icex: &InitCommonControlsEx) -> bool {
        if icex.dw_size as usize != core::mem::size_of::<InitCommonControlsEx>() {
            return false;
        }
        self.initialized_classes |= icex.dw_icc;
        true
    }

    /// Check if a class is initialized
    pub fn is_class_initialized(&self, class_flag: u32) -> bool {
        (self.initialized_classes & class_flag) != 0
    }

    // ==================== ImageList ====================

    /// Create an image list
    pub fn imagelist_create(&mut self, cx: i32, cy: i32, flags: u32, initial: i32, grow: i32) -> HIMAGELIST {
        let id = self.alloc_id();
        let il = ImageList {
            id,
            cx,
            cy,
            flags,
            count: 0,
            grow: grow.max(1),
            items: Vec::with_capacity(initial.max(0) as usize),
        };
        self.image_lists.insert(id, il);
        id
    }

    /// Destroy an image list
    pub fn imagelist_destroy(&mut self, himl: HIMAGELIST) -> bool {
        self.image_lists.remove(&himl).is_some()
    }

    /// Get image count
    pub fn imagelist_get_image_count(&self, himl: HIMAGELIST) -> i32 {
        self.image_lists.get(&himl).map(|il| il.count).unwrap_or(0)
    }

    /// Set image count
    pub fn imagelist_set_image_count(&mut self, himl: HIMAGELIST, count: u32) -> bool {
        if let Some(il) = self.image_lists.get_mut(&himl) {
            il.count = count as i32;
            true
        } else {
            false
        }
    }

    /// Add image to image list
    pub fn imagelist_add(&mut self, himl: HIMAGELIST, _hbm: HBITMAP, _hbm_mask: HBITMAP) -> i32 {
        if let Some(il) = self.image_lists.get_mut(&himl) {
            let index = il.count;
            il.items.push(ImageListItem {
                index,
                width: il.cx,
                height: il.cy,
            });
            il.count += 1;
            index
        } else {
            -1
        }
    }

    /// Replace icon in image list
    pub fn imagelist_replace_icon(&mut self, himl: HIMAGELIST, index: i32, _hicon: HICON) -> i32 {
        if let Some(il) = self.image_lists.get_mut(&himl) {
            if index == -1 {
                // Add new
                let new_index = il.count;
                il.items.push(ImageListItem {
                    index: new_index,
                    width: il.cx,
                    height: il.cy,
                });
                il.count += 1;
                new_index
            } else if index >= 0 && index < il.count {
                // Replace existing
                index
            } else {
                -1
            }
        } else {
            -1
        }
    }

    /// Get icon size
    pub fn imagelist_get_icon_size(&self, himl: HIMAGELIST) -> Option<(i32, i32)> {
        self.image_lists.get(&himl).map(|il| (il.cx, il.cy))
    }

    /// Set icon size
    pub fn imagelist_set_icon_size(&mut self, himl: HIMAGELIST, cx: i32, cy: i32) -> bool {
        if let Some(il) = self.image_lists.get_mut(&himl) {
            il.cx = cx;
            il.cy = cy;
            true
        } else {
            false
        }
    }

    /// Remove image
    pub fn imagelist_remove(&mut self, himl: HIMAGELIST, index: i32) -> bool {
        if let Some(il) = self.image_lists.get_mut(&himl) {
            if index == -1 {
                // Remove all
                il.items.clear();
                il.count = 0;
                true
            } else if index >= 0 && (index as usize) < il.items.len() {
                il.items.remove(index as usize);
                il.count -= 1;
                // Re-index remaining items
                for (i, item) in il.items.iter_mut().enumerate() {
                    item.index = i as i32;
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Duplicate image list
    pub fn imagelist_duplicate(&mut self, himl: HIMAGELIST) -> HIMAGELIST {
        if let Some(il) = self.image_lists.get(&himl) {
            let new_id = self.alloc_id();
            let mut new_il = il.clone();
            new_il.id = new_id;
            self.image_lists.insert(new_id, new_il);
            new_id
        } else {
            0
        }
    }

    // ==================== TreeView helpers ====================

    /// Insert TreeView item
    pub fn treeview_insert_item(
        &mut self,
        hwnd: HWND,
        parent: HTREEITEM,
        insert_after: HTREEITEM,
        text: &str,
        image: i32,
        selected_image: i32,
    ) -> HTREEITEM {
        let handle = self.alloc_id();

        let item = TreeViewItem {
            handle,
            parent,
            text: text.into(),
            image,
            selected_image,
            state: 0,
            lparam: 0,
            children: Vec::new(),
        };

        let tree = self.treeview_items.entry(hwnd).or_insert_with(BTreeMap::new);
        tree.insert(handle, item);

        // Add to parent's children
        if parent != 0 && parent != tvi::ROOT {
            if let Some(parent_item) = tree.get_mut(&parent) {
                if insert_after == tvi::FIRST {
                    parent_item.children.insert(0, handle);
                } else if insert_after == tvi::LAST || insert_after == tvi::SORT {
                    parent_item.children.push(handle);
                } else {
                    // Insert after specific item
                    if let Some(pos) = parent_item.children.iter().position(|&h| h == insert_after) {
                        parent_item.children.insert(pos + 1, handle);
                    } else {
                        parent_item.children.push(handle);
                    }
                }
            }
        }

        handle
    }

    /// Delete TreeView item
    pub fn treeview_delete_item(&mut self, hwnd: HWND, hitem: HTREEITEM) -> bool {
        if let Some(tree) = self.treeview_items.get_mut(&hwnd) {
            if let Some(item) = tree.remove(&hitem) {
                // Remove from parent's children
                if item.parent != 0 && item.parent != tvi::ROOT {
                    if let Some(parent_item) = tree.get_mut(&item.parent) {
                        parent_item.children.retain(|&h| h != hitem);
                    }
                }
                // Recursively delete children
                for child in item.children {
                    self.treeview_delete_item(hwnd, child);
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Get TreeView item count
    pub fn treeview_get_count(&self, hwnd: HWND) -> i32 {
        self.treeview_items.get(&hwnd).map(|t| t.len() as i32).unwrap_or(0)
    }

    // ==================== ListView helpers ====================

    /// Insert ListView item
    pub fn listview_insert_item(&mut self, hwnd: HWND, index: i32, text: &str, image: i32) -> i32 {
        let items = self.listview_items.entry(hwnd).or_insert_with(Vec::new);

        let insert_index = if index < 0 || index as usize >= items.len() {
            items.len() as i32
        } else {
            index
        };

        let item = ListViewItem {
            index: insert_index,
            subitem: 0,
            text: text.into(),
            image,
            state: 0,
            lparam: 0,
        };

        items.insert(insert_index as usize, item);

        // Re-index items
        for (i, item) in items.iter_mut().enumerate() {
            item.index = i as i32;
        }

        insert_index
    }

    /// Delete ListView item
    pub fn listview_delete_item(&mut self, hwnd: HWND, index: i32) -> bool {
        if let Some(items) = self.listview_items.get_mut(&hwnd) {
            if index >= 0 && (index as usize) < items.len() {
                items.remove(index as usize);
                // Re-index
                for (i, item) in items.iter_mut().enumerate() {
                    item.index = i as i32;
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Delete all ListView items
    pub fn listview_delete_all_items(&mut self, hwnd: HWND) -> bool {
        if let Some(items) = self.listview_items.get_mut(&hwnd) {
            items.clear();
            true
        } else {
            false
        }
    }

    /// Get ListView item count
    pub fn listview_get_item_count(&self, hwnd: HWND) -> i32 {
        self.listview_items.get(&hwnd).map(|v| v.len() as i32).unwrap_or(0)
    }

    // ==================== Tab Control helpers ====================

    /// Insert tab
    pub fn tab_insert_item(&mut self, hwnd: HWND, index: i32, text: &str, image: i32) -> i32 {
        let tabs = self.tab_items.entry(hwnd).or_insert_with(Vec::new);

        let insert_index = if index < 0 || index as usize >= tabs.len() {
            tabs.len() as i32
        } else {
            index
        };

        let item = TabItem {
            index: insert_index,
            text: text.into(),
            image,
            lparam: 0,
        };

        tabs.insert(insert_index as usize, item);

        // Re-index
        for (i, tab) in tabs.iter_mut().enumerate() {
            tab.index = i as i32;
        }

        insert_index
    }

    /// Delete tab
    pub fn tab_delete_item(&mut self, hwnd: HWND, index: i32) -> bool {
        if let Some(tabs) = self.tab_items.get_mut(&hwnd) {
            if index >= 0 && (index as usize) < tabs.len() {
                tabs.remove(index as usize);
                // Re-index
                for (i, tab) in tabs.iter_mut().enumerate() {
                    tab.index = i as i32;
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Delete all tabs
    pub fn tab_delete_all_items(&mut self, hwnd: HWND) -> bool {
        if let Some(tabs) = self.tab_items.get_mut(&hwnd) {
            tabs.clear();
            true
        } else {
            false
        }
    }

    /// Get tab count
    pub fn tab_get_item_count(&self, hwnd: HWND) -> i32 {
        self.tab_items.get(&hwnd).map(|v| v.len() as i32).unwrap_or(0)
    }
}

// =============================================================================
// Global Instance
// =============================================================================

use spin::Mutex;
static COMCTL32: Mutex<Option<Comctl32Emulator>> = Mutex::new(None);

/// Initialize COMCTL32 emulation
pub fn init() {
    let mut comctl32 = COMCTL32.lock();
    *comctl32 = Some(Comctl32Emulator::new());
    crate::kprintln!("comctl32: initialized (~130 exports)");
}

/// Get COMCTL32 emulator
pub fn with_comctl32<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&Comctl32Emulator) -> R,
{
    COMCTL32.lock().as_ref().map(f)
}

/// Get COMCTL32 emulator (mutable)
pub fn with_comctl32_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Comctl32Emulator) -> R,
{
    COMCTL32.lock().as_mut().map(f)
}

/// Get export address
pub fn get_proc_address(name: &str) -> Option<u64> {
    with_comctl32(|c| c.get_proc_address(name)).flatten()
}

/// Get all exports
pub fn get_exports() -> BTreeMap<String, u64> {
    with_comctl32(|c| c.get_exports().clone()).unwrap_or_default()
}
