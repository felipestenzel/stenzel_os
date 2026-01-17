//! eventfd - Event notification file descriptor
//!
//! eventfd provides a mechanism for event notification between processes
//! or threads using a file descriptor interface.

use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;

/// eventfd flags
pub const EFD_CLOEXEC: i32 = 0o2000000;   // Close-on-exec
pub const EFD_NONBLOCK: i32 = 0o0004000;  // Non-blocking
pub const EFD_SEMAPHORE: i32 = 0o0000001; // Provide semaphore-like semantics

/// Maximum value for eventfd counter
const EFD_MAX: u64 = u64::MAX - 1;

/// Internal eventfd state
pub struct EventFd {
    /// Counter value
    counter: AtomicU64,
    /// Flags (EFD_SEMAPHORE, etc.)
    flags: i32,
    /// Waiters blocking on read
    read_waiters: IrqSafeMutex<u32>,
    /// Waiters blocking on write
    write_waiters: IrqSafeMutex<u32>,
}

impl EventFd {
    /// Create a new eventfd with initial value and flags
    pub fn new(initval: u32, flags: i32) -> Self {
        Self {
            counter: AtomicU64::new(initval as u64),
            flags,
            read_waiters: IrqSafeMutex::new(0),
            write_waiters: IrqSafeMutex::new(0),
        }
    }

    /// Check if this eventfd is in semaphore mode
    pub fn is_semaphore(&self) -> bool {
        self.flags & EFD_SEMAPHORE != 0
    }

    /// Check if this eventfd is non-blocking
    pub fn is_nonblock(&self) -> bool {
        self.flags & EFD_NONBLOCK != 0
    }

    /// Read from eventfd
    ///
    /// Returns the counter value (or 1 in semaphore mode) and resets/decrements
    /// Returns None if counter is 0 and would block
    pub fn read(&self) -> Option<u64> {
        loop {
            let current = self.counter.load(Ordering::SeqCst);

            if current == 0 {
                // Would block
                return None;
            }

            let (new_val, return_val) = if self.is_semaphore() {
                // Semaphore mode: decrement by 1, return 1
                (current - 1, 1u64)
            } else {
                // Normal mode: reset to 0, return current value
                (0, current)
            };

            // Try to atomically update
            if self.counter.compare_exchange(
                current,
                new_val,
                Ordering::SeqCst,
                Ordering::SeqCst
            ).is_ok() {
                return Some(return_val);
            }
            // CAS failed, retry
        }
    }

    /// Write to eventfd
    ///
    /// Adds value to counter
    /// Returns None if would overflow and block
    pub fn write(&self, value: u64) -> Option<()> {
        if value == 0 {
            return Some(()); // Writing 0 is always ok
        }

        loop {
            let current = self.counter.load(Ordering::SeqCst);

            // Check for overflow
            if current > EFD_MAX - value {
                // Would overflow
                return None;
            }

            let new_val = current + value;

            // Try to atomically update
            if self.counter.compare_exchange(
                current,
                new_val,
                Ordering::SeqCst,
                Ordering::SeqCst
            ).is_ok() {
                return Some(());
            }
            // CAS failed, retry
        }
    }

    /// Check if readable (counter > 0)
    pub fn is_readable(&self) -> bool {
        self.counter.load(Ordering::SeqCst) > 0
    }

    /// Check if writable (counter < max)
    pub fn is_writable(&self) -> bool {
        self.counter.load(Ordering::SeqCst) < EFD_MAX
    }

    /// Get current counter value (for poll/select)
    pub fn poll(&self) -> (bool, bool) {
        let val = self.counter.load(Ordering::SeqCst);
        (val > 0, val < EFD_MAX)  // (readable, writable)
    }
}

/// Wrapper for eventfd as a file descriptor
#[derive(Clone)]
pub struct EventFdFile {
    pub inner: Arc<EventFd>,
    pub flags: i32,
}

impl EventFdFile {
    pub fn new(initval: u32, flags: i32) -> Self {
        Self {
            inner: Arc::new(EventFd::new(initval, flags)),
            flags,
        }
    }

    /// Read from eventfd (blocking if necessary unless nonblock)
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, i32> {
        if buf.len() < 8 {
            return Err(-22); // EINVAL - buffer too small
        }

        // Try to read
        loop {
            if let Some(value) = self.inner.read() {
                // Copy 8-byte value to buffer
                let bytes = value.to_ne_bytes();
                buf[..8].copy_from_slice(&bytes);
                return Ok(8);
            }

            // Counter is 0
            if self.inner.is_nonblock() {
                return Err(-11); // EAGAIN
            }

            // Block and wait (simplified - just yield and retry)
            // In a real implementation, we'd add to a wait queue
            crate::task::yield_now();
        }
    }

    /// Write to eventfd (blocking if necessary unless nonblock)
    pub fn write(&self, buf: &[u8]) -> Result<usize, i32> {
        if buf.len() < 8 {
            return Err(-22); // EINVAL - buffer too small
        }

        // Parse 8-byte value from buffer
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&buf[..8]);
        let value = u64::from_ne_bytes(bytes);

        if value == u64::MAX {
            return Err(-22); // EINVAL - special value not allowed
        }

        // Try to write
        loop {
            if self.inner.write(value).is_some() {
                return Ok(8);
            }

            // Would overflow
            if self.inner.is_nonblock() {
                return Err(-11); // EAGAIN
            }

            // Block and wait (simplified - just yield and retry)
            crate::task::yield_now();
        }
    }

    /// Poll for readability/writability
    pub fn poll(&self) -> (bool, bool) {
        self.inner.poll()
    }
}

/// Create eventfd syscall
///
/// initval: Initial value for counter
/// flags: EFD_CLOEXEC, EFD_NONBLOCK, EFD_SEMAPHORE
///
/// Returns: file descriptor on success, negative errno on error
pub fn sys_eventfd(initval: u32, flags: i32) -> i64 {
    // Validate flags
    let valid_flags = EFD_CLOEXEC | EFD_NONBLOCK | EFD_SEMAPHORE;
    if flags & !valid_flags != 0 {
        return -22; // EINVAL
    }

    // Create eventfd file
    let eventfd_file = EventFdFile::new(initval, flags);

    // Allocate file descriptor
    match crate::syscall::alloc_fd_for_eventfd(eventfd_file) {
        Some(fd) => fd as i64,
        None => -24, // EMFILE - too many open files
    }
}

/// Simpler version (eventfd without flags - for compatibility)
pub fn sys_eventfd2(initval: u32, flags: i32) -> i64 {
    sys_eventfd(initval, flags)
}
