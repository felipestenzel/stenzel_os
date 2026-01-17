//! USER32 Emulation
//!
//! Emulates Windows user32.dll - provides window management,
//! message handling, input processing, and basic GUI functions.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

/// Window handle type
pub type HWND = u64;
/// Device context handle
pub type HDC = u64;
/// Menu handle
pub type HMENU = u64;
/// Icon handle
pub type HICON = u64;
/// Cursor handle
pub type HCURSOR = u64;
/// Brush handle
pub type HBRUSH = u64;
/// Instance handle
pub type HINSTANCE = u64;

/// Invalid handle value
pub const INVALID_HANDLE: u64 = u64::MAX;

/// Window styles
pub mod ws {
    pub const OVERLAPPED: u32 = 0x00000000;
    pub const POPUP: u32 = 0x80000000;
    pub const CHILD: u32 = 0x40000000;
    pub const MINIMIZE: u32 = 0x20000000;
    pub const VISIBLE: u32 = 0x10000000;
    pub const DISABLED: u32 = 0x08000000;
    pub const CLIPSIBLINGS: u32 = 0x04000000;
    pub const CLIPCHILDREN: u32 = 0x02000000;
    pub const MAXIMIZE: u32 = 0x01000000;
    pub const CAPTION: u32 = 0x00C00000;
    pub const BORDER: u32 = 0x00800000;
    pub const DLGFRAME: u32 = 0x00400000;
    pub const VSCROLL: u32 = 0x00200000;
    pub const HSCROLL: u32 = 0x00100000;
    pub const SYSMENU: u32 = 0x00080000;
    pub const THICKFRAME: u32 = 0x00040000;
    pub const GROUP: u32 = 0x00020000;
    pub const TABSTOP: u32 = 0x00010000;
    pub const MINIMIZEBOX: u32 = 0x00020000;
    pub const MAXIMIZEBOX: u32 = 0x00010000;
    pub const OVERLAPPEDWINDOW: u32 = OVERLAPPED | CAPTION | SYSMENU | THICKFRAME | MINIMIZEBOX | MAXIMIZEBOX;
    pub const POPUPWINDOW: u32 = POPUP | BORDER | SYSMENU;
}

/// Extended window styles
pub mod ws_ex {
    pub const DLGMODALFRAME: u32 = 0x00000001;
    pub const NOPARENTNOTIFY: u32 = 0x00000004;
    pub const TOPMOST: u32 = 0x00000008;
    pub const ACCEPTFILES: u32 = 0x00000010;
    pub const TRANSPARENT: u32 = 0x00000020;
    pub const MDICHILD: u32 = 0x00000040;
    pub const TOOLWINDOW: u32 = 0x00000080;
    pub const WINDOWEDGE: u32 = 0x00000100;
    pub const CLIENTEDGE: u32 = 0x00000200;
    pub const CONTEXTHELP: u32 = 0x00000400;
    pub const RIGHT: u32 = 0x00001000;
    pub const RTLREADING: u32 = 0x00002000;
    pub const LEFTSCROLLBAR: u32 = 0x00004000;
    pub const CONTROLPARENT: u32 = 0x00010000;
    pub const STATICEDGE: u32 = 0x00020000;
    pub const APPWINDOW: u32 = 0x00040000;
    pub const LAYERED: u32 = 0x00080000;
    pub const COMPOSITED: u32 = 0x02000000;
    pub const NOACTIVATE: u32 = 0x08000000;
}

/// Window messages
pub mod wm {
    pub const NULL: u32 = 0x0000;
    pub const CREATE: u32 = 0x0001;
    pub const DESTROY: u32 = 0x0002;
    pub const MOVE: u32 = 0x0003;
    pub const SIZE: u32 = 0x0005;
    pub const ACTIVATE: u32 = 0x0006;
    pub const SETFOCUS: u32 = 0x0007;
    pub const KILLFOCUS: u32 = 0x0008;
    pub const ENABLE: u32 = 0x000A;
    pub const SETTEXT: u32 = 0x000C;
    pub const GETTEXT: u32 = 0x000D;
    pub const GETTEXTLENGTH: u32 = 0x000E;
    pub const PAINT: u32 = 0x000F;
    pub const CLOSE: u32 = 0x0010;
    pub const QUIT: u32 = 0x0012;
    pub const ERASEBKGND: u32 = 0x0014;
    pub const SHOWWINDOW: u32 = 0x0018;
    pub const SETCURSOR: u32 = 0x0020;
    pub const MOUSEACTIVATE: u32 = 0x0021;
    pub const GETMINMAXINFO: u32 = 0x0024;
    pub const WINDOWPOSCHANGING: u32 = 0x0046;
    pub const WINDOWPOSCHANGED: u32 = 0x0047;
    pub const NCCREATE: u32 = 0x0081;
    pub const NCDESTROY: u32 = 0x0082;
    pub const NCCALCSIZE: u32 = 0x0083;
    pub const NCHITTEST: u32 = 0x0084;
    pub const NCPAINT: u32 = 0x0085;
    pub const NCACTIVATE: u32 = 0x0086;
    pub const KEYDOWN: u32 = 0x0100;
    pub const KEYUP: u32 = 0x0101;
    pub const CHAR: u32 = 0x0102;
    pub const SYSKEYDOWN: u32 = 0x0104;
    pub const SYSKEYUP: u32 = 0x0105;
    pub const SYSCHAR: u32 = 0x0106;
    pub const INITDIALOG: u32 = 0x0110;
    pub const COMMAND: u32 = 0x0111;
    pub const SYSCOMMAND: u32 = 0x0112;
    pub const TIMER: u32 = 0x0113;
    pub const HSCROLL: u32 = 0x0114;
    pub const VSCROLL: u32 = 0x0115;
    pub const INITMENU: u32 = 0x0116;
    pub const INITMENUPOPUP: u32 = 0x0117;
    pub const MENUSELECT: u32 = 0x011F;
    pub const MENUCHAR: u32 = 0x0120;
    pub const MOUSEMOVE: u32 = 0x0200;
    pub const LBUTTONDOWN: u32 = 0x0201;
    pub const LBUTTONUP: u32 = 0x0202;
    pub const LBUTTONDBLCLK: u32 = 0x0203;
    pub const RBUTTONDOWN: u32 = 0x0204;
    pub const RBUTTONUP: u32 = 0x0205;
    pub const RBUTTONDBLCLK: u32 = 0x0206;
    pub const MBUTTONDOWN: u32 = 0x0207;
    pub const MBUTTONUP: u32 = 0x0208;
    pub const MBUTTONDBLCLK: u32 = 0x0209;
    pub const MOUSEWHEEL: u32 = 0x020A;
    pub const USER: u32 = 0x0400;
    pub const APP: u32 = 0x8000;
}

/// Show window commands
pub mod sw {
    pub const HIDE: i32 = 0;
    pub const SHOWNORMAL: i32 = 1;
    pub const SHOWMINIMIZED: i32 = 2;
    pub const SHOWMAXIMIZED: i32 = 3;
    pub const MAXIMIZE: i32 = 3;
    pub const SHOWNOACTIVATE: i32 = 4;
    pub const SHOW: i32 = 5;
    pub const MINIMIZE: i32 = 6;
    pub const SHOWMINNOACTIVE: i32 = 7;
    pub const SHOWNA: i32 = 8;
    pub const RESTORE: i32 = 9;
    pub const SHOWDEFAULT: i32 = 10;
    pub const FORCEMINIMIZE: i32 = 11;
}

/// Message box flags
pub mod mb {
    pub const OK: u32 = 0x00000000;
    pub const OKCANCEL: u32 = 0x00000001;
    pub const ABORTRETRYIGNORE: u32 = 0x00000002;
    pub const YESNOCANCEL: u32 = 0x00000003;
    pub const YESNO: u32 = 0x00000004;
    pub const RETRYCANCEL: u32 = 0x00000005;

    pub const ICONHAND: u32 = 0x00000010;
    pub const ICONQUESTION: u32 = 0x00000020;
    pub const ICONEXCLAMATION: u32 = 0x00000030;
    pub const ICONASTERISK: u32 = 0x00000040;
    pub const ICONERROR: u32 = ICONHAND;
    pub const ICONWARNING: u32 = ICONEXCLAMATION;
    pub const ICONINFORMATION: u32 = ICONASTERISK;

    pub const DEFBUTTON1: u32 = 0x00000000;
    pub const DEFBUTTON2: u32 = 0x00000100;
    pub const DEFBUTTON3: u32 = 0x00000200;
    pub const DEFBUTTON4: u32 = 0x00000300;

    pub const APPLMODAL: u32 = 0x00000000;
    pub const SYSTEMMODAL: u32 = 0x00001000;
    pub const TASKMODAL: u32 = 0x00002000;
}

/// Message box return values
pub mod id {
    pub const OK: i32 = 1;
    pub const CANCEL: i32 = 2;
    pub const ABORT: i32 = 3;
    pub const RETRY: i32 = 4;
    pub const IGNORE: i32 = 5;
    pub const YES: i32 = 6;
    pub const NO: i32 = 7;
    pub const CLOSE: i32 = 8;
    pub const HELP: i32 = 9;
    pub const TRYAGAIN: i32 = 10;
    pub const CONTINUE: i32 = 11;
}

/// Virtual key codes
pub mod vk {
    pub const LBUTTON: u32 = 0x01;
    pub const RBUTTON: u32 = 0x02;
    pub const CANCEL: u32 = 0x03;
    pub const MBUTTON: u32 = 0x04;
    pub const BACK: u32 = 0x08;
    pub const TAB: u32 = 0x09;
    pub const CLEAR: u32 = 0x0C;
    pub const RETURN: u32 = 0x0D;
    pub const SHIFT: u32 = 0x10;
    pub const CONTROL: u32 = 0x11;
    pub const MENU: u32 = 0x12;
    pub const PAUSE: u32 = 0x13;
    pub const CAPITAL: u32 = 0x14;
    pub const ESCAPE: u32 = 0x1B;
    pub const SPACE: u32 = 0x20;
    pub const PRIOR: u32 = 0x21;
    pub const NEXT: u32 = 0x22;
    pub const END: u32 = 0x23;
    pub const HOME: u32 = 0x24;
    pub const LEFT: u32 = 0x25;
    pub const UP: u32 = 0x26;
    pub const RIGHT: u32 = 0x27;
    pub const DOWN: u32 = 0x28;
    pub const SELECT: u32 = 0x29;
    pub const PRINT: u32 = 0x2A;
    pub const EXECUTE: u32 = 0x2B;
    pub const SNAPSHOT: u32 = 0x2C;
    pub const INSERT: u32 = 0x2D;
    pub const DELETE: u32 = 0x2E;
    pub const HELP: u32 = 0x2F;
    pub const LWIN: u32 = 0x5B;
    pub const RWIN: u32 = 0x5C;
    pub const APPS: u32 = 0x5D;
    pub const F1: u32 = 0x70;
    pub const F2: u32 = 0x71;
    pub const F3: u32 = 0x72;
    pub const F4: u32 = 0x73;
    pub const F5: u32 = 0x74;
    pub const F6: u32 = 0x75;
    pub const F7: u32 = 0x76;
    pub const F8: u32 = 0x77;
    pub const F9: u32 = 0x78;
    pub const F10: u32 = 0x79;
    pub const F11: u32 = 0x7A;
    pub const F12: u32 = 0x7B;
}

/// Point structure
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

/// Rectangle structure
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Message structure
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wparam: u64,
    pub lparam: i64,
    pub time: u32,
    pub pt: POINT,
}

/// Window class structure (simplified)
#[derive(Debug, Clone)]
pub struct WNDCLASS {
    pub style: u32,
    pub lpfn_wnd_proc: u64,
    pub cb_cls_extra: i32,
    pub cb_wnd_extra: i32,
    pub h_instance: HINSTANCE,
    pub h_icon: HICON,
    pub h_cursor: HCURSOR,
    pub hbr_background: HBRUSH,
    pub lpsz_menu_name: Option<String>,
    pub lpsz_class_name: String,
}

/// Window info
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub class_name: String,
    pub title: String,
    pub style: u32,
    pub ex_style: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub parent: HWND,
    pub menu: HMENU,
    pub instance: HINSTANCE,
    pub visible: bool,
    pub enabled: bool,
    pub wnd_proc: u64,
}

/// USER32 emulator state
pub struct User32Emulator {
    /// Registered window classes
    window_classes: BTreeMap<String, WNDCLASS>,
    /// Created windows
    windows: BTreeMap<HWND, WindowInfo>,
    /// Next window handle
    next_hwnd: HWND,
    /// Message queue
    message_queue: Vec<MSG>,
    /// Active window
    active_window: HWND,
    /// Focused window
    focus_window: HWND,
    /// Captured window (mouse capture)
    capture_window: HWND,
    /// Desktop window handle
    desktop_hwnd: HWND,
    /// Cursor position
    cursor_pos: POINT,
    /// Timers
    timers: BTreeMap<(HWND, u64), u32>,  // (hwnd, id) -> interval
    /// Clipboard content
    clipboard: Option<Vec<u8>>,
    /// Clipboard format
    clipboard_format: u32,
}

impl User32Emulator {
    pub fn new() -> Self {
        Self {
            window_classes: BTreeMap::new(),
            windows: BTreeMap::new(),
            next_hwnd: 0x10000,
            message_queue: Vec::new(),
            active_window: 0,
            focus_window: 0,
            capture_window: 0,
            desktop_hwnd: 0x10000,
            cursor_pos: POINT::default(),
            timers: BTreeMap::new(),
            clipboard: None,
            clipboard_format: 0,
        }
    }

    // ========== Window Class Functions ==========

    /// RegisterClassA
    pub fn register_class(&mut self, wc: &WNDCLASS) -> u16 {
        crate::kprintln!("user32: RegisterClass(\"{}\")", wc.lpsz_class_name);

        let class_name = wc.lpsz_class_name.to_lowercase();
        if self.window_classes.contains_key(&class_name) {
            // Class already exists
            return 0;
        }

        self.window_classes.insert(class_name, wc.clone());

        // Return a non-zero atom
        (self.window_classes.len() as u16) + 0xC000
    }

    /// UnregisterClassA
    pub fn unregister_class(&mut self, class_name: &str) -> bool {
        self.window_classes.remove(&class_name.to_lowercase()).is_some()
    }

    // ========== Window Functions ==========

    /// CreateWindowExA
    pub fn create_window_ex(
        &mut self,
        ex_style: u32,
        class_name: &str,
        window_name: &str,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: HWND,
        menu: HMENU,
        instance: HINSTANCE,
        _param: u64,
    ) -> HWND {
        crate::kprintln!("user32: CreateWindowEx(\"{}\", \"{}\")", class_name, window_name);

        // Look up window class
        let class_lower = class_name.to_lowercase();
        let wnd_proc = self.window_classes.get(&class_lower)
            .map(|wc| wc.lpfn_wnd_proc)
            .unwrap_or(0);

        // Create window
        let hwnd = self.next_hwnd;
        self.next_hwnd += 1;

        let info = WindowInfo {
            class_name: String::from(class_name),
            title: String::from(window_name),
            style,
            ex_style,
            x: if x == 0x80000000u32 as i32 { 0 } else { x },  // CW_USEDEFAULT
            y: if y == 0x80000000u32 as i32 { 0 } else { y },
            width: if width == 0x80000000u32 as i32 { 640 } else { width },
            height: if height == 0x80000000u32 as i32 { 480 } else { height },
            parent,
            menu,
            instance,
            visible: (style & ws::VISIBLE) != 0,
            enabled: (style & ws::DISABLED) == 0,
            wnd_proc,
        };

        self.windows.insert(hwnd, info);

        // Post WM_CREATE
        self.post_message(hwnd, wm::CREATE, 0, 0);

        hwnd
    }

    /// DestroyWindow
    pub fn destroy_window(&mut self, hwnd: HWND) -> bool {
        crate::kprintln!("user32: DestroyWindow({:#x})", hwnd);

        if self.windows.remove(&hwnd).is_some() {
            // Post WM_DESTROY
            self.post_message(hwnd, wm::DESTROY, 0, 0);

            // Clean up focus/active
            if self.active_window == hwnd {
                self.active_window = 0;
            }
            if self.focus_window == hwnd {
                self.focus_window = 0;
            }
            if self.capture_window == hwnd {
                self.capture_window = 0;
            }

            true
        } else {
            false
        }
    }

    /// ShowWindow
    pub fn show_window(&mut self, hwnd: HWND, cmd_show: i32) -> bool {
        crate::kprintln!("user32: ShowWindow({:#x}, {})", hwnd, cmd_show);

        let (was_visible, is_visible) = if let Some(info) = self.windows.get_mut(&hwnd) {
            let was_visible = info.visible;

            match cmd_show {
                sw::HIDE => info.visible = false,
                sw::SHOW | sw::SHOWNORMAL | sw::SHOWDEFAULT => info.visible = true,
                sw::SHOWMINIMIZED | sw::MINIMIZE | sw::SHOWMINNOACTIVE => {
                    info.visible = true;
                    info.style |= ws::MINIMIZE;
                }
                sw::SHOWMAXIMIZED | sw::MAXIMIZE => {
                    info.visible = true;
                    info.style |= ws::MAXIMIZE;
                    info.style &= !ws::MINIMIZE;
                }
                sw::RESTORE => {
                    info.visible = true;
                    info.style &= !(ws::MINIMIZE | ws::MAXIMIZE);
                }
                _ => {}
            }

            (was_visible, info.visible)
        } else {
            return false;
        };

        // Post WM_SHOWWINDOW
        self.post_message(hwnd, wm::SHOWWINDOW, if is_visible { 1 } else { 0 }, 0);

        was_visible
    }

    /// UpdateWindow
    pub fn update_window(&mut self, hwnd: HWND) -> bool {
        if self.windows.contains_key(&hwnd) {
            self.post_message(hwnd, wm::PAINT, 0, 0);
            true
        } else {
            false
        }
    }

    /// MoveWindow
    pub fn move_window(&mut self, hwnd: HWND, x: i32, y: i32, width: i32, height: i32, repaint: bool) -> bool {
        let success = if let Some(info) = self.windows.get_mut(&hwnd) {
            info.x = x;
            info.y = y;
            info.width = width;
            info.height = height;
            true
        } else {
            false
        };

        if success {
            self.post_message(hwnd, wm::MOVE, 0, ((y as u32) << 16 | (x as u32)) as i64);
            self.post_message(hwnd, wm::SIZE, 0, ((height as u32) << 16 | (width as u32)) as i64);

            if repaint {
                self.post_message(hwnd, wm::PAINT, 0, 0);
            }
        }

        success
    }

    /// GetWindowRect
    pub fn get_window_rect(&self, hwnd: HWND) -> Option<RECT> {
        self.windows.get(&hwnd).map(|info| RECT {
            left: info.x,
            top: info.y,
            right: info.x + info.width,
            bottom: info.y + info.height,
        })
    }

    /// GetClientRect
    pub fn get_client_rect(&self, hwnd: HWND) -> Option<RECT> {
        self.windows.get(&hwnd).map(|info| {
            // Subtract border/title bar (simplified)
            let border = if (info.style & ws::BORDER) != 0 { 2 } else { 0 };
            let caption = if (info.style & ws::CAPTION) != 0 { 20 } else { 0 };
            RECT {
                left: 0,
                top: 0,
                right: info.width - border * 2,
                bottom: info.height - caption - border * 2,
            }
        })
    }

    /// SetWindowTextA
    pub fn set_window_text(&mut self, hwnd: HWND, text: &str) -> bool {
        let success = if let Some(info) = self.windows.get_mut(&hwnd) {
            info.title = String::from(text);
            true
        } else {
            false
        };

        if success {
            self.post_message(hwnd, wm::SETTEXT, 0, 0);
        }

        success
    }

    /// GetWindowTextA
    pub fn get_window_text(&self, hwnd: HWND) -> Option<&str> {
        self.windows.get(&hwnd).map(|info| info.title.as_str())
    }

    /// IsWindow
    pub fn is_window(&self, hwnd: HWND) -> bool {
        self.windows.contains_key(&hwnd)
    }

    /// IsWindowVisible
    pub fn is_window_visible(&self, hwnd: HWND) -> bool {
        self.windows.get(&hwnd).map(|i| i.visible).unwrap_or(false)
    }

    /// IsWindowEnabled
    pub fn is_window_enabled(&self, hwnd: HWND) -> bool {
        self.windows.get(&hwnd).map(|i| i.enabled).unwrap_or(false)
    }

    /// EnableWindow
    pub fn enable_window(&mut self, hwnd: HWND, enable: bool) -> bool {
        let (found, was_enabled) = if let Some(info) = self.windows.get_mut(&hwnd) {
            let was = info.enabled;
            info.enabled = enable;
            (true, was)
        } else {
            (false, false)
        };

        if found {
            self.post_message(hwnd, wm::ENABLE, if enable { 1 } else { 0 }, 0);
        }

        was_enabled
    }

    /// GetDesktopWindow
    pub fn get_desktop_window(&self) -> HWND {
        self.desktop_hwnd
    }

    /// GetForegroundWindow
    pub fn get_foreground_window(&self) -> HWND {
        self.active_window
    }

    /// SetForegroundWindow
    pub fn set_foreground_window(&mut self, hwnd: HWND) -> bool {
        if self.windows.contains_key(&hwnd) {
            self.active_window = hwnd;
            self.focus_window = hwnd;
            self.post_message(hwnd, wm::ACTIVATE, 1, 0);
            self.post_message(hwnd, wm::SETFOCUS, 0, 0);
            true
        } else {
            false
        }
    }

    /// GetFocus
    pub fn get_focus(&self) -> HWND {
        self.focus_window
    }

    /// SetFocus
    pub fn set_focus(&mut self, hwnd: HWND) -> HWND {
        let old_focus = self.focus_window;
        if self.windows.contains_key(&hwnd) {
            if old_focus != 0 {
                self.post_message(old_focus, wm::KILLFOCUS, hwnd, 0);
            }
            self.focus_window = hwnd;
            self.post_message(hwnd, wm::SETFOCUS, old_focus, 0);
        }
        old_focus
    }

    // ========== Message Functions ==========

    /// PostMessageA
    pub fn post_message(&mut self, hwnd: HWND, msg: u32, wparam: u64, lparam: i64) -> bool {
        let message = MSG {
            hwnd,
            message: msg,
            wparam,
            lparam,
            time: crate::time::ticks() as u32,
            pt: self.cursor_pos,
        };
        self.message_queue.push(message);
        true
    }

    /// SendMessageA (simplified - just calls window proc or returns 0)
    pub fn send_message(&mut self, hwnd: HWND, msg: u32, wparam: u64, lparam: i64) -> i64 {
        crate::kprintln!("user32: SendMessage({:#x}, {:#x}, {}, {})", hwnd, msg, wparam, lparam);

        // In a real implementation, this would call the window procedure
        match msg {
            wm::GETTEXTLENGTH => {
                self.windows.get(&hwnd).map(|i| i.title.len() as i64).unwrap_or(0)
            }
            wm::CLOSE => {
                self.destroy_window(hwnd);
                0
            }
            _ => 0,
        }
    }

    /// GetMessageA
    pub fn get_message(&mut self, msg: &mut MSG, hwnd: HWND, msg_filter_min: u32, msg_filter_max: u32) -> i32 {
        // Find message matching filter
        let pos = self.message_queue.iter().position(|m| {
            (hwnd == 0 || m.hwnd == hwnd) &&
            (msg_filter_min == 0 && msg_filter_max == 0 ||
             (m.message >= msg_filter_min && m.message <= msg_filter_max))
        });

        if let Some(pos) = pos {
            *msg = self.message_queue.remove(pos);
            if msg.message == wm::QUIT {
                0
            } else {
                1
            }
        } else {
            // No message - would block in real implementation
            -1
        }
    }

    /// PeekMessageA
    pub fn peek_message(&mut self, msg: &mut MSG, hwnd: HWND, msg_filter_min: u32, msg_filter_max: u32, remove: bool) -> bool {
        let pos = self.message_queue.iter().position(|m| {
            (hwnd == 0 || m.hwnd == hwnd) &&
            (msg_filter_min == 0 && msg_filter_max == 0 ||
             (m.message >= msg_filter_min && m.message <= msg_filter_max))
        });

        if let Some(pos) = pos {
            if remove {
                *msg = self.message_queue.remove(pos);
            } else {
                *msg = self.message_queue[pos];
            }
            true
        } else {
            false
        }
    }

    /// TranslateMessage
    pub fn translate_message(&mut self, msg: &MSG) -> bool {
        // Convert WM_KEYDOWN to WM_CHAR for printable characters
        if msg.message == wm::KEYDOWN {
            let vk = msg.wparam as u32;
            if vk >= 0x30 && vk <= 0x5A {  // 0-9, A-Z
                self.post_message(msg.hwnd, wm::CHAR, msg.wparam, msg.lparam);
                return true;
            }
        }
        false
    }

    /// DispatchMessageA
    pub fn dispatch_message(&mut self, msg: &MSG) -> i64 {
        crate::kprintln!("user32: DispatchMessage({:#x}, {})", msg.hwnd, msg.message);

        // In a real implementation, this would call the window procedure
        // For now, just return 0
        0
    }

    /// PostQuitMessage
    pub fn post_quit_message(&mut self, exit_code: i32) {
        self.post_message(0, wm::QUIT, exit_code as u64, 0);
    }

    // ========== Dialog Functions ==========

    /// MessageBoxA
    pub fn message_box(&mut self, hwnd: HWND, text: &str, caption: &str, mb_type: u32) -> i32 {
        crate::kprintln!("user32: MessageBox(\"{}\", \"{}\")", caption, text);

        // In a real implementation, would show a dialog
        // For now, return default button

        let buttons = mb_type & 0x0F;
        match buttons {
            mb::OK => id::OK,
            mb::OKCANCEL => id::OK,
            mb::ABORTRETRYIGNORE => id::ABORT,
            mb::YESNOCANCEL => id::YES,
            mb::YESNO => id::YES,
            mb::RETRYCANCEL => id::RETRY,
            _ => id::OK,
        }
    }

    // ========== Input Functions ==========

    /// GetCursorPos
    pub fn get_cursor_pos(&self) -> POINT {
        self.cursor_pos
    }

    /// SetCursorPos
    pub fn set_cursor_pos(&mut self, x: i32, y: i32) -> bool {
        self.cursor_pos = POINT { x, y };
        true
    }

    /// GetAsyncKeyState
    pub fn get_async_key_state(&self, _vkey: u32) -> i16 {
        // Would need to check keyboard state
        0
    }

    /// SetCapture
    pub fn set_capture(&mut self, hwnd: HWND) -> HWND {
        let old = self.capture_window;
        self.capture_window = hwnd;
        old
    }

    /// ReleaseCapture
    pub fn release_capture(&mut self) -> bool {
        if self.capture_window != 0 {
            self.capture_window = 0;
            true
        } else {
            false
        }
    }

    /// GetCapture
    pub fn get_capture(&self) -> HWND {
        self.capture_window
    }

    // ========== Timer Functions ==========

    /// SetTimer
    pub fn set_timer(&mut self, hwnd: HWND, id: u64, elapse: u32) -> u64 {
        crate::kprintln!("user32: SetTimer({:#x}, {}, {}ms)", hwnd, id, elapse);
        self.timers.insert((hwnd, id), elapse);
        id
    }

    /// KillTimer
    pub fn kill_timer(&mut self, hwnd: HWND, id: u64) -> bool {
        self.timers.remove(&(hwnd, id)).is_some()
    }

    // ========== Clipboard Functions ==========

    /// OpenClipboard
    pub fn open_clipboard(&mut self, _hwnd: HWND) -> bool {
        true
    }

    /// CloseClipboard
    pub fn close_clipboard(&mut self) -> bool {
        true
    }

    /// EmptyClipboard
    pub fn empty_clipboard(&mut self) -> bool {
        self.clipboard = None;
        self.clipboard_format = 0;
        true
    }

    /// SetClipboardData
    pub fn set_clipboard_data(&mut self, format: u32, data: Vec<u8>) -> bool {
        self.clipboard = Some(data);
        self.clipboard_format = format;
        true
    }

    /// GetClipboardData
    pub fn get_clipboard_data(&self, format: u32) -> Option<&Vec<u8>> {
        if self.clipboard_format == format {
            self.clipboard.as_ref()
        } else {
            None
        }
    }

    /// IsClipboardFormatAvailable
    pub fn is_clipboard_format_available(&self, format: u32) -> bool {
        self.clipboard.is_some() && self.clipboard_format == format
    }
}

impl Default for User32Emulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Get USER32 exports for the loader
pub fn get_exports() -> BTreeMap<String, u64> {
    let mut exports = BTreeMap::new();

    let funcs = [
        // Window class
        "RegisterClassA", "RegisterClassW", "RegisterClassExA", "RegisterClassExW",
        "UnregisterClassA", "UnregisterClassW",
        "GetClassNameA", "GetClassNameW",

        // Window creation/destruction
        "CreateWindowExA", "CreateWindowExW",
        "DestroyWindow",
        "ShowWindow", "UpdateWindow",
        "MoveWindow", "SetWindowPos",
        "GetWindowRect", "GetClientRect",
        "SetWindowTextA", "SetWindowTextW",
        "GetWindowTextA", "GetWindowTextW",
        "GetWindowTextLengthA", "GetWindowTextLengthW",
        "IsWindow", "IsWindowVisible", "IsWindowEnabled",
        "EnableWindow",
        "GetDesktopWindow", "GetParent", "GetWindow",
        "FindWindowA", "FindWindowW",
        "GetForegroundWindow", "SetForegroundWindow",
        "GetActiveWindow", "SetActiveWindow",
        "GetFocus", "SetFocus",
        "BringWindowToTop",

        // Message loop
        "GetMessageA", "GetMessageW",
        "PeekMessageA", "PeekMessageW",
        "TranslateMessage",
        "DispatchMessageA", "DispatchMessageW",
        "PostMessageA", "PostMessageW",
        "SendMessageA", "SendMessageW",
        "PostQuitMessage",
        "DefWindowProcA", "DefWindowProcW",
        "CallWindowProcA", "CallWindowProcW",

        // Dialog
        "MessageBoxA", "MessageBoxW",
        "DialogBoxParamA", "DialogBoxParamW",
        "CreateDialogParamA", "CreateDialogParamW",
        "EndDialog",
        "GetDlgItem", "GetDlgItemTextA", "GetDlgItemTextW",
        "SetDlgItemTextA", "SetDlgItemTextW",
        "GetDlgItemInt", "SetDlgItemInt",
        "SendDlgItemMessageA", "SendDlgItemMessageW",
        "CheckDlgButton", "IsDlgButtonChecked",

        // Input
        "GetCursorPos", "SetCursorPos",
        "SetCursor", "LoadCursorA", "LoadCursorW",
        "GetAsyncKeyState", "GetKeyState",
        "SetCapture", "ReleaseCapture", "GetCapture",
        "GetKeyboardState", "SetKeyboardState",
        "MapVirtualKeyA", "MapVirtualKeyW",

        // Timer
        "SetTimer", "KillTimer",

        // Clipboard
        "OpenClipboard", "CloseClipboard", "EmptyClipboard",
        "SetClipboardData", "GetClipboardData",
        "IsClipboardFormatAvailable", "GetClipboardFormatNameA", "GetClipboardFormatNameW",
        "RegisterClipboardFormatA", "RegisterClipboardFormatW",

        // Menu
        "CreateMenu", "CreatePopupMenu", "DestroyMenu",
        "AppendMenuA", "AppendMenuW",
        "InsertMenuA", "InsertMenuW",
        "RemoveMenu", "DeleteMenu",
        "GetMenu", "SetMenu",
        "GetSubMenu", "GetMenuItemCount", "GetMenuItemID",
        "CheckMenuItem", "EnableMenuItem",
        "TrackPopupMenu", "TrackPopupMenuEx",

        // Painting
        "BeginPaint", "EndPaint",
        "GetDC", "ReleaseDC", "GetWindowDC",
        "InvalidateRect", "ValidateRect",
        "RedrawWindow",

        // Resources
        "LoadIconA", "LoadIconW",
        "LoadImageA", "LoadImageW",
        "LoadStringA", "LoadStringW",

        // System
        "GetSystemMetrics",
        "SystemParametersInfoA", "SystemParametersInfoW",
        "GetDoubleClickTime", "SetDoubleClickTime",
        "ExitWindowsEx",
    ];

    let mut addr = 0x7FD0_0000u64;
    for func in funcs {
        exports.insert(String::from(func), addr);
        addr += 16;
    }

    exports
}
