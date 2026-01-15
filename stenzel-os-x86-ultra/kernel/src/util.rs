    #![allow(dead_code)]

    use core::fmt;

    pub fn banner() {
        kprintln!("");
        kprintln!("============================================================");
        kprintln!("  Stenzel OS (x86_64) - kernel");
        kprintln!("  build: {} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        kprintln!("============================================================");
        kprintln!("");
    }

    #[doc(hidden)]
    pub fn _print(args: fmt::Arguments) {
        crate::serial::print(args);
    }

    #[macro_export]
    macro_rules! kprint {
        ($($arg:tt)*) => ({
            $crate::util::_print(format_args!($($arg)*));
        });
    }

    #[macro_export]
    macro_rules! kprintln {
        () => ($crate::kprint!("\n"));
        ($fmt:expr) => ($crate::kprint!(concat!($fmt, "\n")));
        ($fmt:expr, $($arg:tt)*) => ($crate::kprint!(concat!($fmt, "\n"), $($arg)*));
    }

    pub use crate::kprintln;

    /// Erros "kernel-level" genéricos para subsistemas (fs, storage, etc).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum KError {
        NotFound,
        AlreadyExists,
        Invalid,
        PermissionDenied,
        NoMemory,
        Busy,
        NotSupported,
        IO,
        WouldBlock,
        OutOfRange,
        Timeout,
        NoChild,
    }

    pub type KResult<T> = core::result::Result<T, KError>;
