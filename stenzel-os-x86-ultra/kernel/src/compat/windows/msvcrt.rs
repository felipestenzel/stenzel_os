//! MSVCRT (Microsoft Visual C Runtime) Emulation
//!
//! Provides C runtime library functions for Windows applications.
//! This includes stdio, stdlib, string, memory, and math functions.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use core::ptr;

/// File stream structure
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct FILE {
    pub ptr: u64,        // Current buffer pointer
    pub cnt: i32,        // Characters remaining in buffer
    pub base: u64,       // Buffer base
    pub flag: i32,       // File flags
    pub file: i32,       // File descriptor
    pub charbuf: i32,    // Single char buffer
    pub bufsiz: i32,     // Buffer size
    pub tmpfname: u64,   // Temp file name pointer
}

/// File flags
pub mod file_flags {
    pub const _IOREAD: i32 = 0x0001;
    pub const _IOWRT: i32 = 0x0002;
    pub const _IOMYBUF: i32 = 0x0008;
    pub const _IOEOF: i32 = 0x0010;
    pub const _IOERR: i32 = 0x0020;
    pub const _IOSTRG: i32 = 0x0040;
    pub const _IORW: i32 = 0x0080;
}

/// Seek modes
pub mod seek {
    pub const SEEK_SET: i32 = 0;
    pub const SEEK_CUR: i32 = 1;
    pub const SEEK_END: i32 = 2;
}

/// Error codes (errno)
pub mod errno {
    pub const EPERM: i32 = 1;
    pub const ENOENT: i32 = 2;
    pub const ESRCH: i32 = 3;
    pub const EINTR: i32 = 4;
    pub const EIO: i32 = 5;
    pub const ENXIO: i32 = 6;
    pub const E2BIG: i32 = 7;
    pub const ENOEXEC: i32 = 8;
    pub const EBADF: i32 = 9;
    pub const ECHILD: i32 = 10;
    pub const EAGAIN: i32 = 11;
    pub const ENOMEM: i32 = 12;
    pub const EACCES: i32 = 13;
    pub const EFAULT: i32 = 14;
    pub const EBUSY: i32 = 16;
    pub const EEXIST: i32 = 17;
    pub const EXDEV: i32 = 18;
    pub const ENODEV: i32 = 19;
    pub const ENOTDIR: i32 = 20;
    pub const EISDIR: i32 = 21;
    pub const EINVAL: i32 = 22;
    pub const ENFILE: i32 = 23;
    pub const EMFILE: i32 = 24;
    pub const ENOTTY: i32 = 25;
    pub const EFBIG: i32 = 27;
    pub const ENOSPC: i32 = 28;
    pub const ESPIPE: i32 = 29;
    pub const EROFS: i32 = 30;
    pub const EMLINK: i32 = 31;
    pub const EPIPE: i32 = 32;
    pub const EDOM: i32 = 33;
    pub const ERANGE: i32 = 34;
    pub const EDEADLK: i32 = 36;
    pub const ENAMETOOLONG: i32 = 38;
    pub const ENOLCK: i32 = 39;
    pub const ENOSYS: i32 = 40;
    pub const ENOTEMPTY: i32 = 41;
    pub const EILSEQ: i32 = 42;
}

/// MSVCRT state
pub struct MsvcrtEmulator {
    /// errno value
    errno: i32,
    /// Standard file handles
    stdin: FILE,
    stdout: FILE,
    stderr: FILE,
    /// Open files
    files: BTreeMap<i32, FILE>,
    /// Next file descriptor
    next_fd: i32,
    /// Environment variables (for getenv)
    environment: BTreeMap<String, String>,
    /// Exit handlers (atexit)
    atexit_handlers: Vec<u64>,
    /// Random seed
    rand_seed: u32,
    /// Temp file counter
    tmpfile_counter: u32,
}

impl MsvcrtEmulator {
    pub fn new() -> Self {
        let mut emulator = Self {
            errno: 0,
            stdin: FILE {
                file: 0,
                flag: file_flags::_IOREAD,
                ..Default::default()
            },
            stdout: FILE {
                file: 1,
                flag: file_flags::_IOWRT,
                ..Default::default()
            },
            stderr: FILE {
                file: 2,
                flag: file_flags::_IOWRT,
                ..Default::default()
            },
            files: BTreeMap::new(),
            next_fd: 3,
            environment: BTreeMap::new(),
            atexit_handlers: Vec::new(),
            rand_seed: 1,
            tmpfile_counter: 0,
        };

        // Initialize environment
        emulator.environment.insert(String::from("PATH"), String::from("C:\\Windows;C:\\Windows\\System32"));
        emulator.environment.insert(String::from("TEMP"), String::from("C:\\Windows\\Temp"));
        emulator.environment.insert(String::from("TMP"), String::from("C:\\Windows\\Temp"));

        emulator
    }

    /// Get/set errno
    pub fn get_errno(&self) -> i32 {
        self.errno
    }

    pub fn set_errno(&mut self, e: i32) {
        self.errno = e;
    }

    // ========== stdio functions ==========

    /// fopen - Open file
    pub fn fopen(&mut self, filename: &str, mode: &str) -> Option<&mut FILE> {
        crate::kprintln!("msvcrt: fopen(\"{}\", \"{}\")", filename, mode);

        // Parse mode
        let _flags = match mode {
            "r" | "rb" => 0,      // Read only
            "w" | "wb" => 1,      // Write only, truncate
            "a" | "ab" => 2,      // Append
            "r+" | "rb+" | "r+b" => 3,  // Read/write
            "w+" | "wb+" | "w+b" => 4,  // Read/write, truncate
            "a+" | "ab+" | "a+b" => 5,  // Read/append
            _ => {
                self.errno = errno::EINVAL;
                return None;
            }
        };

        // TODO: Actually open file
        self.errno = errno::ENOENT;
        None
    }

    /// fclose - Close file
    pub fn fclose(&mut self, fd: i32) -> i32 {
        if fd < 3 {
            self.errno = errno::EBADF;
            return -1;
        }

        if self.files.remove(&fd).is_some() {
            0
        } else {
            self.errno = errno::EBADF;
            -1
        }
    }

    /// fread - Read from file
    pub fn fread(&mut self, _buffer: u64, size: usize, count: usize, fd: i32) -> usize {
        crate::kprintln!("msvcrt: fread(size={}, count={}, fd={})", size, count, fd);

        // TODO: Implement actual file reading
        self.errno = errno::ENOSYS;
        0
    }

    /// fwrite - Write to file
    pub fn fwrite(&mut self, buffer: &[u8], size: usize, count: usize, fd: i32) -> usize {
        let total = size * count;
        let to_write = core::cmp::min(total, buffer.len());

        if fd == 1 || fd == 2 {
            // stdout/stderr
            for &b in &buffer[..to_write] {
                crate::console::write_byte(b);
            }
            return count;
        }

        // TODO: Implement file writing
        self.errno = errno::ENOSYS;
        0
    }

    /// fprintf - Formatted output to file
    pub fn fprintf(&mut self, fd: i32, format: &str, args: &[u64]) -> i32 {
        let output = self.format_string(format, args);
        let bytes = output.as_bytes();
        self.fwrite(bytes, 1, bytes.len(), fd) as i32
    }

    /// printf - Formatted output to stdout
    pub fn printf(&mut self, format: &str, args: &[u64]) -> i32 {
        self.fprintf(1, format, args)
    }

    /// sprintf - Formatted output to string
    pub fn sprintf(&self, format: &str, args: &[u64]) -> String {
        self.format_string(format, args)
    }

    /// Simple format string implementation
    fn format_string(&self, format: &str, args: &[u64]) -> String {
        let mut result = String::new();
        let mut chars = format.chars().peekable();
        let mut arg_index = 0;

        while let Some(c) = chars.next() {
            if c == '%' {
                if let Some(&next) = chars.peek() {
                    match next {
                        '%' => {
                            result.push('%');
                            chars.next();
                        }
                        'd' | 'i' => {
                            chars.next();
                            if arg_index < args.len() {
                                result.push_str(&(args[arg_index] as i64).to_string());
                                arg_index += 1;
                            }
                        }
                        'u' => {
                            chars.next();
                            if arg_index < args.len() {
                                result.push_str(&args[arg_index].to_string());
                                arg_index += 1;
                            }
                        }
                        'x' => {
                            chars.next();
                            if arg_index < args.len() {
                                result.push_str(&alloc::format!("{:x}", args[arg_index]));
                                arg_index += 1;
                            }
                        }
                        'X' => {
                            chars.next();
                            if arg_index < args.len() {
                                result.push_str(&alloc::format!("{:X}", args[arg_index]));
                                arg_index += 1;
                            }
                        }
                        'p' => {
                            chars.next();
                            if arg_index < args.len() {
                                result.push_str(&alloc::format!("0x{:x}", args[arg_index]));
                                arg_index += 1;
                            }
                        }
                        's' => {
                            chars.next();
                            if arg_index < args.len() {
                                // In a real implementation, would read string from address
                                result.push_str("<string>");
                                arg_index += 1;
                            }
                        }
                        'c' => {
                            chars.next();
                            if arg_index < args.len() {
                                let c = (args[arg_index] & 0xFF) as u8 as char;
                                result.push(c);
                                arg_index += 1;
                            }
                        }
                        'f' | 'g' | 'e' => {
                            chars.next();
                            if arg_index < args.len() {
                                // Simplified float handling
                                result.push_str("<float>");
                                arg_index += 1;
                            }
                        }
                        _ => {
                            // Skip unknown format specifier
                            result.push('%');
                        }
                    }
                } else {
                    result.push('%');
                }
            } else {
                result.push(c);
            }
        }

        result
    }

    /// fgetc - Get character from file
    pub fn fgetc(&mut self, fd: i32) -> i32 {
        if fd == 0 {
            // stdin - would need to read from console
            self.errno = errno::ENOSYS;
            return -1;
        }
        self.errno = errno::ENOSYS;
        -1
    }

    /// fputc - Put character to file
    pub fn fputc(&mut self, c: i32, fd: i32) -> i32 {
        if fd == 1 || fd == 2 {
            crate::console::write_byte(c as u8);
            return c;
        }
        self.errno = errno::ENOSYS;
        -1
    }

    /// fputs - Put string to file
    pub fn fputs(&mut self, s: &str, fd: i32) -> i32 {
        if fd == 1 || fd == 2 {
            for b in s.bytes() {
                crate::console::write_byte(b);
            }
            return 0;
        }
        self.errno = errno::ENOSYS;
        -1
    }

    /// puts - Put string to stdout with newline
    pub fn puts(&mut self, s: &str) -> i32 {
        self.fputs(s, 1);
        self.fputc('\n' as i32, 1);
        0
    }

    /// fseek - Seek in file
    pub fn fseek(&mut self, fd: i32, offset: i64, whence: i32) -> i32 {
        crate::kprintln!("msvcrt: fseek(fd={}, offset={}, whence={})", fd, offset, whence);
        self.errno = errno::ENOSYS;
        -1
    }

    /// ftell - Get file position
    pub fn ftell(&mut self, fd: i32) -> i64 {
        crate::kprintln!("msvcrt: ftell(fd={})", fd);
        self.errno = errno::ENOSYS;
        -1
    }

    /// fflush - Flush file
    pub fn fflush(&mut self, fd: i32) -> i32 {
        // For stdout/stderr, nothing to do
        if fd == 1 || fd == 2 {
            return 0;
        }
        self.errno = errno::ENOSYS;
        -1
    }

    /// feof - Check end of file
    pub fn feof(&self, fd: i32) -> i32 {
        if fd < 3 {
            return 0;
        }
        if let Some(f) = self.files.get(&fd) {
            if (f.flag & file_flags::_IOEOF) != 0 { 1 } else { 0 }
        } else {
            0
        }
    }

    /// ferror - Check file error
    pub fn ferror(&self, fd: i32) -> i32 {
        if fd < 3 {
            return 0;
        }
        if let Some(f) = self.files.get(&fd) {
            if (f.flag & file_flags::_IOERR) != 0 { 1 } else { 0 }
        } else {
            0
        }
    }

    // ========== stdlib functions ==========

    /// malloc - Allocate memory
    pub fn malloc(&mut self, size: usize) -> u64 {
        crate::kprintln!("msvcrt: malloc({})", size);
        // TODO: Actually allocate
        0
    }

    /// calloc - Allocate and zero memory
    pub fn calloc(&mut self, count: usize, size: usize) -> u64 {
        crate::kprintln!("msvcrt: calloc({}, {})", count, size);
        // TODO: Actually allocate
        0
    }

    /// realloc - Reallocate memory
    pub fn realloc(&mut self, ptr: u64, size: usize) -> u64 {
        crate::kprintln!("msvcrt: realloc({:#x}, {})", ptr, size);
        // TODO: Actually reallocate
        0
    }

    /// free - Free memory
    pub fn free(&mut self, ptr: u64) {
        crate::kprintln!("msvcrt: free({:#x})", ptr);
        // TODO: Actually free
    }

    /// atoi - String to integer
    pub fn atoi(&self, s: &str) -> i32 {
        let s = s.trim();
        let mut result: i64 = 0;
        let mut negative = false;
        let mut chars = s.chars();

        // Handle sign
        if let Some(first) = chars.next() {
            if first == '-' {
                negative = true;
            } else if first == '+' {
                // Continue
            } else if first.is_ascii_digit() {
                result = (first as i64) - ('0' as i64);
            } else {
                return 0;
            }
        }

        // Parse digits
        for c in chars {
            if c.is_ascii_digit() {
                result = result * 10 + (c as i64) - ('0' as i64);
            } else {
                break;
            }
        }

        if negative {
            (-result) as i32
        } else {
            result as i32
        }
    }

    /// atol - String to long
    pub fn atol(&self, s: &str) -> i64 {
        self.atoi(s) as i64
    }

    /// rand - Random number
    pub fn rand(&mut self) -> i32 {
        // Linear congruential generator
        self.rand_seed = self.rand_seed.wrapping_mul(1103515245).wrapping_add(12345);
        ((self.rand_seed >> 16) & 0x7fff) as i32
    }

    /// srand - Seed random number generator
    pub fn srand(&mut self, seed: u32) {
        self.rand_seed = seed;
    }

    /// exit - Exit process
    pub fn exit(&self, status: i32) -> ! {
        crate::kprintln!("msvcrt: exit({})", status);
        // Run atexit handlers in reverse order (would need function pointers)
        crate::syscall::sys_exit(status as u64);
    }

    /// abort - Abort process
    pub fn abort(&self) -> ! {
        crate::kprintln!("msvcrt: abort()");
        crate::syscall::sys_exit(3);
    }

    /// atexit - Register exit handler
    pub fn atexit(&mut self, func: u64) -> i32 {
        if self.atexit_handlers.len() >= 32 {
            return -1;
        }
        self.atexit_handlers.push(func);
        0
    }

    /// getenv - Get environment variable
    pub fn getenv(&self, name: &str) -> Option<&String> {
        self.environment.get(name)
    }

    /// system - Execute command
    pub fn system(&mut self, command: &str) -> i32 {
        crate::kprintln!("msvcrt: system(\"{}\")", command);
        // TODO: Would need to spawn a shell
        -1
    }

    // ========== string functions ==========

    /// strlen - String length
    pub fn strlen(&self, s: &[u8]) -> usize {
        s.iter().position(|&b| b == 0).unwrap_or(s.len())
    }

    /// strcpy - Copy string
    pub fn strcpy(&self, dest: &mut [u8], src: &[u8]) -> usize {
        let len = self.strlen(src);
        let copy_len = core::cmp::min(len + 1, dest.len());
        dest[..copy_len].copy_from_slice(&src[..copy_len]);
        copy_len
    }

    /// strncpy - Copy string with limit
    pub fn strncpy(&self, dest: &mut [u8], src: &[u8], n: usize) -> usize {
        let len = self.strlen(src);
        let copy_len = core::cmp::min(core::cmp::min(len, n), dest.len());
        dest[..copy_len].copy_from_slice(&src[..copy_len]);
        // Pad with zeros
        if copy_len < n && copy_len < dest.len() {
            let pad_end = core::cmp::min(n, dest.len());
            for byte in &mut dest[copy_len..pad_end] {
                *byte = 0;
            }
        }
        copy_len
    }

    /// strcmp - Compare strings
    pub fn strcmp(&self, s1: &[u8], s2: &[u8]) -> i32 {
        let len1 = self.strlen(s1);
        let len2 = self.strlen(s2);
        let min_len = core::cmp::min(len1, len2);

        for i in 0..min_len {
            if s1[i] != s2[i] {
                return (s1[i] as i32) - (s2[i] as i32);
            }
        }

        if len1 < len2 {
            -(s2[len1] as i32)
        } else if len1 > len2 {
            s1[len2] as i32
        } else {
            0
        }
    }

    /// strncmp - Compare strings with limit
    pub fn strncmp(&self, s1: &[u8], s2: &[u8], n: usize) -> i32 {
        let len1 = core::cmp::min(self.strlen(s1), n);
        let len2 = core::cmp::min(self.strlen(s2), n);
        let min_len = core::cmp::min(len1, len2);

        for i in 0..min_len {
            if s1[i] != s2[i] {
                return (s1[i] as i32) - (s2[i] as i32);
            }
        }

        if len1 < len2 && len1 < n {
            -(s2[len1] as i32)
        } else if len1 > len2 && len2 < n {
            s1[len2] as i32
        } else {
            0
        }
    }

    /// strcat - Concatenate strings
    pub fn strcat(&self, dest: &mut [u8], src: &[u8]) -> usize {
        let dest_len = self.strlen(dest);
        let src_len = self.strlen(src);
        let copy_len = core::cmp::min(src_len + 1, dest.len() - dest_len);

        if copy_len > 0 && dest_len < dest.len() {
            dest[dest_len..dest_len + copy_len].copy_from_slice(&src[..copy_len]);
        }

        dest_len + copy_len
    }

    /// strncat - Concatenate strings with limit
    pub fn strncat(&self, dest: &mut [u8], src: &[u8], n: usize) -> usize {
        let dest_len = self.strlen(dest);
        let src_len = core::cmp::min(self.strlen(src), n);
        let copy_len = core::cmp::min(src_len, dest.len() - dest_len - 1);

        if copy_len > 0 && dest_len < dest.len() - 1 {
            dest[dest_len..dest_len + copy_len].copy_from_slice(&src[..copy_len]);
            dest[dest_len + copy_len] = 0;
        }

        dest_len + copy_len
    }

    // ========== memory functions ==========

    /// memcpy - Copy memory
    pub fn memcpy(&self, dest: &mut [u8], src: &[u8], n: usize) {
        let copy_len = core::cmp::min(core::cmp::min(n, dest.len()), src.len());
        dest[..copy_len].copy_from_slice(&src[..copy_len]);
    }

    /// memmove - Move memory (handles overlap)
    pub fn memmove(&self, dest: &mut [u8], src: &[u8], n: usize) {
        // In Rust, slices handle this correctly
        let copy_len = core::cmp::min(core::cmp::min(n, dest.len()), src.len());
        dest[..copy_len].copy_from_slice(&src[..copy_len]);
    }

    /// memset - Fill memory
    pub fn memset(&self, dest: &mut [u8], c: u8, n: usize) {
        let fill_len = core::cmp::min(n, dest.len());
        for byte in &mut dest[..fill_len] {
            *byte = c;
        }
    }

    /// memcmp - Compare memory
    pub fn memcmp(&self, s1: &[u8], s2: &[u8], n: usize) -> i32 {
        let cmp_len = core::cmp::min(core::cmp::min(n, s1.len()), s2.len());

        for i in 0..cmp_len {
            if s1[i] != s2[i] {
                return (s1[i] as i32) - (s2[i] as i32);
            }
        }

        0
    }

    // ========== character functions ==========

    /// isalpha
    pub fn isalpha(&self, c: i32) -> i32 {
        let c = (c & 0xFF) as u8 as char;
        if c.is_ascii_alphabetic() { 1 } else { 0 }
    }

    /// isdigit
    pub fn isdigit(&self, c: i32) -> i32 {
        let c = (c & 0xFF) as u8 as char;
        if c.is_ascii_digit() { 1 } else { 0 }
    }

    /// isalnum
    pub fn isalnum(&self, c: i32) -> i32 {
        let c = (c & 0xFF) as u8 as char;
        if c.is_ascii_alphanumeric() { 1 } else { 0 }
    }

    /// isspace
    pub fn isspace(&self, c: i32) -> i32 {
        let c = (c & 0xFF) as u8 as char;
        if c.is_ascii_whitespace() { 1 } else { 0 }
    }

    /// isupper
    pub fn isupper(&self, c: i32) -> i32 {
        let c = (c & 0xFF) as u8 as char;
        if c.is_ascii_uppercase() { 1 } else { 0 }
    }

    /// islower
    pub fn islower(&self, c: i32) -> i32 {
        let c = (c & 0xFF) as u8 as char;
        if c.is_ascii_lowercase() { 1 } else { 0 }
    }

    /// toupper
    pub fn toupper(&self, c: i32) -> i32 {
        let c = (c & 0xFF) as u8 as char;
        c.to_ascii_uppercase() as i32
    }

    /// tolower
    pub fn tolower(&self, c: i32) -> i32 {
        let c = (c & 0xFF) as u8 as char;
        c.to_ascii_lowercase() as i32
    }

    // ========== time functions ==========

    /// time - Get current time
    pub fn time(&self, _timer: Option<&mut i64>) -> i64 {
        // Return Unix timestamp
        crate::time::realtime().tv_sec
    }

    /// clock - Get processor time
    pub fn clock(&self) -> i64 {
        crate::time::ticks() as i64
    }
}

impl Default for MsvcrtEmulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Get MSVCRT exports for the loader
pub fn get_exports() -> BTreeMap<String, u64> {
    let mut exports = BTreeMap::new();

    let funcs = [
        // stdio
        "fopen", "fclose", "fread", "fwrite",
        "fprintf", "printf", "sprintf", "snprintf",
        "vfprintf", "vprintf", "vsprintf", "vsnprintf",
        "scanf", "fscanf", "sscanf",
        "fgetc", "fgets", "fputc", "fputs", "puts",
        "getchar", "putchar", "getc", "putc",
        "fseek", "ftell", "rewind", "fgetpos", "fsetpos",
        "fflush", "feof", "ferror", "clearerr", "perror",
        "__iob_func", "_iob",

        // stdlib
        "malloc", "calloc", "realloc", "free",
        "atoi", "atol", "atof", "strtol", "strtoul", "strtod",
        "rand", "srand",
        "exit", "_exit", "abort", "atexit",
        "getenv", "system",
        "abs", "labs", "div", "ldiv",
        "qsort", "bsearch",

        // string
        "strlen", "strcpy", "strncpy", "strcat", "strncat",
        "strcmp", "strncmp", "strchr", "strrchr", "strstr",
        "strtok", "strdup", "_strdup",
        "memcpy", "memmove", "memset", "memcmp", "memchr",

        // ctype
        "isalpha", "isdigit", "isalnum", "isspace",
        "isupper", "islower", "isprint", "iscntrl",
        "ispunct", "isxdigit", "isgraph",
        "toupper", "tolower",

        // time
        "time", "clock", "difftime", "mktime",
        "localtime", "gmtime", "asctime", "ctime", "strftime",

        // math
        "sin", "cos", "tan", "asin", "acos", "atan", "atan2",
        "sinh", "cosh", "tanh",
        "exp", "log", "log10", "pow", "sqrt",
        "ceil", "floor", "fabs", "fmod",
        "frexp", "ldexp", "modf",

        // process
        "_beginthread", "_beginthreadex", "_endthread", "_endthreadex",
        "getpid", "_getpid",

        // misc
        "setlocale", "localeconv",
        "_errno", "__dllonexit", "_onexit",
        "_initterm", "_initterm_e",
        "__CxxFrameHandler3", "_CxxThrowException",
        "__CppXcptFilter", "_XcptFilter",
    ];

    let mut addr = 0x7FE0_0000u64;
    for func in funcs {
        exports.insert(String::from(func), addr);
        addr += 16;
    }

    exports
}
