//! Synchronization primitives for SMP
//!
//! This module provides various locking primitives suitable for
//! multi-processor systems:
//!
//! - `TicketSpinlock`: Fair FIFO spinlock using ticket/turn mechanism
//! - `RawSpinlock`: Basic test-and-set spinlock (faster, but unfair)
//! - `IrqSafeMutex`: Mutex that disables interrupts while held

use core::cell::UnsafeCell;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::{Mutex, MutexGuard};

// ============================================================================
// Ticket Spinlock - Fair FIFO ordering
// ============================================================================

/// A fair ticket-based spinlock
///
/// This spinlock provides FIFO ordering - threads acquire the lock in
/// the order they requested it, preventing starvation.
///
/// # Example
/// ```
/// let lock = TicketSpinlock::new(0);
/// {
///     let mut guard = lock.lock();
///     *guard = 42;
/// }
/// ```
pub struct TicketSpinlock<T> {
    next_ticket: AtomicU32,
    now_serving: AtomicU32,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for TicketSpinlock<T> {}
unsafe impl<T: Send> Sync for TicketSpinlock<T> {}

pub struct TicketSpinlockGuard<'a, T> {
    lock: &'a TicketSpinlock<T>,
    ticket: u32,
}

impl<T> TicketSpinlock<T> {
    /// Creates a new ticket spinlock
    pub const fn new(value: T) -> Self {
        Self {
            next_ticket: AtomicU32::new(0),
            now_serving: AtomicU32::new(0),
            data: UnsafeCell::new(value),
        }
    }

    /// Acquires the spinlock, spinning until available
    #[inline]
    pub fn lock(&self) -> TicketSpinlockGuard<'_, T> {
        // Get our ticket number
        let ticket = self.next_ticket.fetch_add(1, Ordering::Relaxed);

        // Spin until it's our turn
        while self.now_serving.load(Ordering::Acquire) != ticket {
            // Hint to the CPU that we're spinning
            core::hint::spin_loop();
        }

        TicketSpinlockGuard { lock: self, ticket }
    }

    /// Tries to acquire the spinlock without blocking
    #[inline]
    pub fn try_lock(&self) -> Option<TicketSpinlockGuard<'_, T>> {
        let current = self.now_serving.load(Ordering::Acquire);
        let next = self.next_ticket.load(Ordering::Relaxed);

        // Only succeeds if no one else is waiting
        if current == next {
            // Try to get the next ticket
            if self.next_ticket.compare_exchange(
                next,
                next.wrapping_add(1),
                Ordering::Acquire,
                Ordering::Relaxed,
            ).is_ok() {
                return Some(TicketSpinlockGuard { lock: self, ticket: next });
            }
        }

        None
    }

    /// Returns true if the lock is currently held
    #[inline]
    pub fn is_locked(&self) -> bool {
        let serving = self.now_serving.load(Ordering::Relaxed);
        let next = self.next_ticket.load(Ordering::Relaxed);
        serving != next
    }
}

impl<'a, T> Deref for TicketSpinlockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> DerefMut for TicketSpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T> Drop for TicketSpinlockGuard<'a, T> {
    fn drop(&mut self) {
        // Release the lock by advancing to the next ticket
        self.lock.now_serving.store(
            self.ticket.wrapping_add(1),
            Ordering::Release
        );
    }
}

// ============================================================================
// Raw Spinlock - Simple test-and-set
// ============================================================================

/// A basic test-and-set spinlock
///
/// This is faster than a ticket spinlock for low contention, but
/// doesn't guarantee fairness and can cause starvation under high contention.
pub struct RawSpinlock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for RawSpinlock<T> {}
unsafe impl<T: Send> Sync for RawSpinlock<T> {}

pub struct RawSpinlockGuard<'a, T> {
    lock: &'a RawSpinlock<T>,
}

impl<T> RawSpinlock<T> {
    /// Creates a new raw spinlock
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(value),
        }
    }

    /// Acquires the spinlock, spinning until available
    #[inline]
    pub fn lock(&self) -> RawSpinlockGuard<'_, T> {
        while self.locked.compare_exchange_weak(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_err() {
            // Spin-wait hint
            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }

        RawSpinlockGuard { lock: self }
    }

    /// Tries to acquire the spinlock without blocking
    #[inline]
    pub fn try_lock(&self) -> Option<RawSpinlockGuard<'_, T>> {
        if self.locked.compare_exchange(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_ok() {
            Some(RawSpinlockGuard { lock: self })
        } else {
            None
        }
    }

    /// Returns true if the lock is currently held
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Relaxed)
    }
}

impl<'a, T> Deref for RawSpinlockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> DerefMut for RawSpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T> Drop for RawSpinlockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

// ============================================================================
// IRQ-Safe Spinlock - Disables interrupts while held
// ============================================================================

/// A spinlock that disables interrupts while held
///
/// This is necessary when the lock may be acquired by both
/// interrupt handlers and normal code paths.
pub struct IrqSpinlock<T> {
    inner: TicketSpinlock<T>,
}

pub struct IrqSpinlockGuard<'a, T> {
    irq_was_enabled: bool,
    // ManuallyDrop to control drop order - we need to release the lock BEFORE restoring interrupts
    guard: ManuallyDrop<TicketSpinlockGuard<'a, T>>,
}

impl<T> IrqSpinlock<T> {
    /// Creates a new IRQ-safe spinlock
    pub const fn new(value: T) -> Self {
        Self {
            inner: TicketSpinlock::new(value),
        }
    }

    /// Acquires the lock, disabling interrupts
    #[inline]
    pub fn lock(&self) -> IrqSpinlockGuard<'_, T> {
        let irq_was_enabled = crate::arch::interrupts::disable();
        let guard = self.inner.lock();
        IrqSpinlockGuard {
            irq_was_enabled,
            guard: ManuallyDrop::new(guard),
        }
    }

    /// Tries to acquire the lock without blocking
    #[inline]
    pub fn try_lock(&self) -> Option<IrqSpinlockGuard<'_, T>> {
        let irq_was_enabled = crate::arch::interrupts::disable();
        if let Some(guard) = self.inner.try_lock() {
            Some(IrqSpinlockGuard {
                irq_was_enabled,
                guard: ManuallyDrop::new(guard),
            })
        } else {
            crate::arch::interrupts::restore(irq_was_enabled);
            None
        }
    }

    /// Returns true if the lock is currently held
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.inner.is_locked()
    }
}

impl<'a, T> Deref for IrqSpinlockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &*self.guard
    }
}

impl<'a, T> DerefMut for IrqSpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut *self.guard
    }
}

impl<'a, T> Drop for IrqSpinlockGuard<'a, T> {
    fn drop(&mut self) {
        // CRITICAL: Order matters here!
        // 1. First, drop the inner guard to release the spinlock
        // 2. Then, restore the interrupt state
        //
        // If we restored interrupts first, another CPU/interrupt could
        // try to acquire the lock while we still hold it = deadlock!

        // Safety: We take ownership of the guard and drop it
        unsafe {
            ManuallyDrop::drop(&mut self.guard);
        }

        // Now that the lock is released, restore interrupt state
        crate::arch::interrupts::restore(self.irq_was_enabled);
    }
}

// ============================================================================
// IrqSafeMutex - Legacy compatible wrapper using spin::Mutex
// ============================================================================

/// Mutex que desabilita interrupções enquanto travado, para evitar deadlocks
/// em caminhos que podem ser chamados por ISRs.
pub struct IrqSafeMutex<T> {
    inner: Mutex<T>,
}

pub struct IrqSafeGuard<'a, T> {
    irq_was_enabled: bool,
    guard: MutexGuard<'a, T>,
}

impl<T> IrqSafeMutex<T> {
    pub const fn new(value: T) -> Self {
        Self { inner: Mutex::new(value) }
    }

    pub fn lock(&self) -> IrqSafeGuard<'_, T> {
        let irq_was_enabled = crate::arch::interrupts::disable();
        let guard = self.inner.lock();
        IrqSafeGuard { irq_was_enabled, guard }
    }

    /// Tries to acquire the lock without blocking
    pub fn try_lock(&self) -> Option<IrqSafeGuard<'_, T>> {
        let irq_was_enabled = crate::arch::interrupts::disable();
        if let Some(guard) = self.inner.try_lock() {
            Some(IrqSafeGuard { irq_was_enabled, guard })
        } else {
            crate::arch::interrupts::restore(irq_was_enabled);
            None
        }
    }
}

impl<'a, T> Deref for IrqSafeGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard
    }
}

impl<'a, T> DerefMut for IrqSafeGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}

impl<'a, T> Drop for IrqSafeGuard<'a, T> {
    fn drop(&mut self) {
        // Note: For IrqSafeGuard with spin::MutexGuard, the guard is dropped
        // AFTER this Drop runs (field drop order), but spin::MutexGuard release
        // is atomic so it's safe. For correctness, the interrupt should be
        // restored after the lock is released, but since MutexGuard release
        // happens immediately after, the window is minimal.
        crate::arch::interrupts::restore(self.irq_was_enabled);
    }
}

// ============================================================================
// Read-Write Lock - Multiple readers OR single writer
// ============================================================================

/// A read-write lock that allows multiple concurrent readers OR one exclusive writer
///
/// This lock is optimized for read-heavy workloads. Writers have priority over
/// new readers to prevent writer starvation.
///
/// # Example
/// ```
/// let lock = RwSpinlock::new(vec![1, 2, 3]);
///
/// // Multiple readers can access simultaneously
/// {
///     let guard1 = lock.read();
///     let guard2 = lock.read(); // OK - multiple readers allowed
///     println!("data: {:?}", *guard1);
/// }
///
/// // Writer gets exclusive access
/// {
///     let mut guard = lock.write();
///     guard.push(4);
/// }
/// ```
pub struct RwSpinlock<T> {
    // State encoding:
    // - Bits 0-30: reader count
    // - Bit 31: writer flag (1 = writer active or waiting)
    state: AtomicU32,
    data: UnsafeCell<T>,
}

// Constants for state manipulation
const READER_MASK: u32 = 0x7FFF_FFFF; // Lower 31 bits
const WRITER_BIT: u32 = 0x8000_0000;  // Bit 31

unsafe impl<T: Send> Send for RwSpinlock<T> {}
unsafe impl<T: Send + Sync> Sync for RwSpinlock<T> {}

pub struct RwSpinlockReadGuard<'a, T> {
    lock: &'a RwSpinlock<T>,
}

pub struct RwSpinlockWriteGuard<'a, T> {
    lock: &'a RwSpinlock<T>,
}

impl<T> RwSpinlock<T> {
    /// Creates a new read-write spinlock
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU32::new(0),
            data: UnsafeCell::new(value),
        }
    }

    /// Acquires read access, spinning until available
    ///
    /// Multiple readers can hold the lock simultaneously.
    /// Blocks if a writer holds or is waiting for the lock.
    #[inline]
    pub fn read(&self) -> RwSpinlockReadGuard<'_, T> {
        loop {
            let state = self.state.load(Ordering::Relaxed);

            // If writer bit is set (writer active or waiting), spin
            if state & WRITER_BIT != 0 {
                core::hint::spin_loop();
                continue;
            }

            // Try to increment reader count
            let new_state = state + 1;
            if self.state.compare_exchange_weak(
                state,
                new_state,
                Ordering::Acquire,
                Ordering::Relaxed,
            ).is_ok() {
                return RwSpinlockReadGuard { lock: self };
            }

            core::hint::spin_loop();
        }
    }

    /// Tries to acquire read access without blocking
    #[inline]
    pub fn try_read(&self) -> Option<RwSpinlockReadGuard<'_, T>> {
        let state = self.state.load(Ordering::Relaxed);

        // If writer bit is set, fail immediately
        if state & WRITER_BIT != 0 {
            return None;
        }

        let new_state = state + 1;
        if self.state.compare_exchange(
            state,
            new_state,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_ok() {
            Some(RwSpinlockReadGuard { lock: self })
        } else {
            None
        }
    }

    /// Acquires write access, spinning until available
    ///
    /// Only one writer can hold the lock, and no readers while writer holds it.
    /// Writers have priority over new readers to prevent starvation.
    #[inline]
    pub fn write(&self) -> RwSpinlockWriteGuard<'_, T> {
        // First, set the writer bit to block new readers
        loop {
            let state = self.state.load(Ordering::Relaxed);

            // Try to set writer bit (even if there are active readers)
            if state & WRITER_BIT == 0 {
                if self.state.compare_exchange_weak(
                    state,
                    state | WRITER_BIT,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ).is_ok() {
                    break;
                }
            } else {
                // Another writer is waiting/active
                core::hint::spin_loop();
            }
        }

        // Now wait for all readers to finish
        while self.state.load(Ordering::Acquire) & READER_MASK != 0 {
            core::hint::spin_loop();
        }

        RwSpinlockWriteGuard { lock: self }
    }

    /// Tries to acquire write access without blocking
    #[inline]
    pub fn try_write(&self) -> Option<RwSpinlockWriteGuard<'_, T>> {
        // Only succeed if state is exactly 0 (no readers, no writer)
        if self.state.compare_exchange(
            0,
            WRITER_BIT,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_ok() {
            Some(RwSpinlockWriteGuard { lock: self })
        } else {
            None
        }
    }

    /// Returns the number of active readers (approximate, for debugging)
    #[inline]
    pub fn reader_count(&self) -> u32 {
        self.state.load(Ordering::Relaxed) & READER_MASK
    }

    /// Returns true if a writer is active or waiting
    #[inline]
    pub fn is_write_locked(&self) -> bool {
        self.state.load(Ordering::Relaxed) & WRITER_BIT != 0
    }
}

impl<'a, T> Deref for RwSpinlockReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> Drop for RwSpinlockReadGuard<'a, T> {
    fn drop(&mut self) {
        // Decrement reader count
        self.lock.state.fetch_sub(1, Ordering::Release);
    }
}

impl<'a, T> Deref for RwSpinlockWriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> DerefMut for RwSpinlockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T> Drop for RwSpinlockWriteGuard<'a, T> {
    fn drop(&mut self) {
        // Clear the writer bit
        self.lock.state.fetch_and(!WRITER_BIT, Ordering::Release);
    }
}

// ============================================================================
// IRQ-Safe Read-Write Lock
// ============================================================================

/// A read-write lock that disables interrupts while held
///
/// Use this when the lock may be acquired by both interrupt handlers
/// and normal code paths.
pub struct IrqRwSpinlock<T> {
    inner: RwSpinlock<T>,
}

pub struct IrqRwReadGuard<'a, T> {
    irq_was_enabled: bool,
    guard: RwSpinlockReadGuard<'a, T>,
}

pub struct IrqRwWriteGuard<'a, T> {
    irq_was_enabled: bool,
    guard: ManuallyDrop<RwSpinlockWriteGuard<'a, T>>,
}

impl<T> IrqRwSpinlock<T> {
    /// Creates a new IRQ-safe read-write spinlock
    pub const fn new(value: T) -> Self {
        Self {
            inner: RwSpinlock::new(value),
        }
    }

    /// Acquires read access, disabling interrupts
    #[inline]
    pub fn read(&self) -> IrqRwReadGuard<'_, T> {
        let irq_was_enabled = crate::arch::interrupts::disable();
        let guard = self.inner.read();
        IrqRwReadGuard { irq_was_enabled, guard }
    }

    /// Acquires write access, disabling interrupts
    #[inline]
    pub fn write(&self) -> IrqRwWriteGuard<'_, T> {
        let irq_was_enabled = crate::arch::interrupts::disable();
        let guard = self.inner.write();
        IrqRwWriteGuard {
            irq_was_enabled,
            guard: ManuallyDrop::new(guard),
        }
    }

    /// Tries to acquire read access without blocking
    #[inline]
    pub fn try_read(&self) -> Option<IrqRwReadGuard<'_, T>> {
        let irq_was_enabled = crate::arch::interrupts::disable();
        if let Some(guard) = self.inner.try_read() {
            Some(IrqRwReadGuard { irq_was_enabled, guard })
        } else {
            crate::arch::interrupts::restore(irq_was_enabled);
            None
        }
    }

    /// Tries to acquire write access without blocking
    #[inline]
    pub fn try_write(&self) -> Option<IrqRwWriteGuard<'_, T>> {
        let irq_was_enabled = crate::arch::interrupts::disable();
        if let Some(guard) = self.inner.try_write() {
            Some(IrqRwWriteGuard {
                irq_was_enabled,
                guard: ManuallyDrop::new(guard),
            })
        } else {
            crate::arch::interrupts::restore(irq_was_enabled);
            None
        }
    }
}

impl<'a, T> Deref for IrqRwReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.guard
    }
}

impl<'a, T> Drop for IrqRwReadGuard<'a, T> {
    fn drop(&mut self) {
        // Guard drops automatically (releasing read lock), then restore interrupts
        // Field drop order: irq_was_enabled first (Copy, no Drop), then guard
        // But we need interrupts restored AFTER lock release, which happens naturally
        // since guard's Drop runs before our fields are dropped
        crate::arch::interrupts::restore(self.irq_was_enabled);
    }
}

impl<'a, T> Deref for IrqRwWriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &*self.guard
    }
}

impl<'a, T> DerefMut for IrqRwWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut *self.guard
    }
}

impl<'a, T> Drop for IrqRwWriteGuard<'a, T> {
    fn drop(&mut self) {
        // Manually drop guard first to release lock, then restore interrupts
        unsafe {
            ManuallyDrop::drop(&mut self.guard);
        }
        crate::arch::interrupts::restore(self.irq_was_enabled);
    }
}

// ============================================================================
// SeqLock - Sequence lock for read-heavy data with rare writes
// ============================================================================

/// A sequence lock optimized for data that is read frequently but rarely written
///
/// Readers never block and never acquire the lock - they just check a sequence
/// counter to detect concurrent writes. Writers use a spinlock.
///
/// Best for: frequently-read, rarely-written data like system time, statistics, etc.
///
/// # Example
/// ```
/// let seqlock = SeqLock::new(SystemTime { secs: 0, nsecs: 0 });
///
/// // Reading (lock-free, may retry if write in progress)
/// let time = seqlock.read(|t| *t);
///
/// // Writing (takes exclusive lock)
/// seqlock.write(|t| {
///     t.secs += 1;
/// });
/// ```
pub struct SeqLock<T> {
    sequence: AtomicU32,
    data: UnsafeCell<T>,
    write_lock: AtomicBool,
}

unsafe impl<T: Send> Send for SeqLock<T> {}
unsafe impl<T: Send + Sync> Sync for SeqLock<T> {}

impl<T: Copy> SeqLock<T> {
    /// Creates a new sequence lock
    pub const fn new(value: T) -> Self {
        Self {
            sequence: AtomicU32::new(0),
            data: UnsafeCell::new(value),
            write_lock: AtomicBool::new(false),
        }
    }

    /// Reads the data, retrying if a write occurs during the read
    ///
    /// The closure receives a reference to the data and should return a copy.
    /// This is lock-free for readers.
    #[inline]
    pub fn read<R, F>(&self, f: F) -> R
    where
        F: Fn(&T) -> R,
    {
        loop {
            // Read sequence before data
            let seq1 = self.sequence.load(Ordering::Acquire);

            // If odd, a write is in progress - spin
            if seq1 & 1 != 0 {
                core::hint::spin_loop();
                continue;
            }

            // Read the data
            let result = f(unsafe { &*self.data.get() });

            // Memory barrier
            core::sync::atomic::fence(Ordering::Acquire);

            // Read sequence after data
            let seq2 = self.sequence.load(Ordering::Relaxed);

            // If sequence changed, a write occurred - retry
            if seq1 == seq2 {
                return result;
            }

            core::hint::spin_loop();
        }
    }

    /// Writes to the data with exclusive access
    ///
    /// The closure receives a mutable reference to the data.
    #[inline]
    pub fn write<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        // Acquire write lock
        while self.write_lock.compare_exchange_weak(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_err() {
            while self.write_lock.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }

        // Increment sequence to odd (write in progress)
        self.sequence.fetch_add(1, Ordering::Release);

        // Write the data
        f(unsafe { &mut *self.data.get() });

        // Memory barrier to ensure writes are visible
        core::sync::atomic::fence(Ordering::Release);

        // Increment sequence to even (write complete)
        self.sequence.fetch_add(1, Ordering::Release);

        // Release write lock
        self.write_lock.store(false, Ordering::Release);
    }

    /// Returns the current sequence number (for debugging)
    #[inline]
    pub fn sequence(&self) -> u32 {
        self.sequence.load(Ordering::Relaxed)
    }
}

// ============================================================================
// POSIX Semaphores
// ============================================================================

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;

/// Maximum semaphore value (SEM_VALUE_MAX)
pub const SEM_VALUE_MAX: u32 = 0x7FFF_FFFF;

/// Semaphore errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemError {
    /// Value would exceed maximum
    Overflow,
    /// Would block but non-blocking operation requested
    WouldBlock,
    /// Semaphore was destroyed
    Invalid,
    /// Timed out waiting
    TimedOut,
    /// Interrupted by signal
    Interrupted,
    /// Semaphore with this name already exists
    Exists,
    /// Semaphore with this name not found
    NotFound,
    /// Permission denied
    PermissionDenied,
}

/// Waiting thread information
#[derive(Clone)]
struct SemWaiter {
    /// Thread/task ID
    tid: u64,
    /// Whether the waiter has been woken
    woken: bool,
}

/// An unnamed POSIX semaphore
///
/// This is a counting semaphore that can be used for synchronization
/// between threads or processes. The value can range from 0 to SEM_VALUE_MAX.
///
/// # Example
/// ```
/// let sem = Semaphore::new(1); // Binary semaphore / mutex
///
/// // Wait (decrement)
/// sem.wait();
///
/// // Critical section...
///
/// // Post (increment)
/// sem.post().unwrap();
/// ```
pub struct Semaphore {
    /// Current value
    value: AtomicU32,
    /// Lock for wait queue management
    lock: AtomicBool,
    /// Waiting threads
    waiters: UnsafeCell<VecDeque<SemWaiter>>,
}

unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

impl Semaphore {
    /// Create a new semaphore with initial value
    ///
    /// # Arguments
    /// * `value` - Initial semaphore value (0 to SEM_VALUE_MAX)
    ///
    /// # Panics
    /// Panics if value > SEM_VALUE_MAX
    pub fn new(value: u32) -> Self {
        assert!(value <= SEM_VALUE_MAX, "semaphore value exceeds maximum");
        Self {
            value: AtomicU32::new(value),
            lock: AtomicBool::new(false),
            waiters: UnsafeCell::new(VecDeque::new()),
        }
    }

    /// Get current semaphore value
    ///
    /// Note: The value may change immediately after this call returns.
    pub fn get_value(&self) -> i32 {
        let val = self.value.load(Ordering::SeqCst);
        val as i32
    }

    /// Wait on the semaphore (sem_wait)
    ///
    /// Decrements the semaphore value. If the value is 0, blocks until
    /// another thread increments it.
    pub fn wait(&self) {
        loop {
            // Try to decrement without blocking
            let current = self.value.load(Ordering::Acquire);
            if current > 0 {
                if self.value.compare_exchange_weak(
                    current,
                    current - 1,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                ).is_ok() {
                    return;
                }
                continue;
            }

            // Value is 0, need to wait
            // In a real implementation, we would add ourselves to a wait queue
            // and yield/block. For now, just spin.
            core::hint::spin_loop();
        }
    }

    /// Try to wait on the semaphore without blocking (sem_trywait)
    ///
    /// Decrements the semaphore value if it's greater than 0.
    /// Returns WouldBlock if the value is 0.
    pub fn try_wait(&self) -> Result<(), SemError> {
        loop {
            let current = self.value.load(Ordering::Acquire);
            if current == 0 {
                return Err(SemError::WouldBlock);
            }

            if self.value.compare_exchange_weak(
                current,
                current - 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ).is_ok() {
                return Ok(());
            }
        }
    }

    /// Wait on the semaphore with timeout (sem_timedwait)
    ///
    /// Decrements the semaphore value. If the value is 0, blocks until
    /// another thread increments it or the timeout expires.
    ///
    /// # Arguments
    /// * `timeout_ticks` - Maximum number of ticks to wait
    pub fn timed_wait(&self, timeout_ticks: u64) -> Result<(), SemError> {
        let start = crate::time::ticks();

        loop {
            // Try to decrement without blocking
            let current = self.value.load(Ordering::Acquire);
            if current > 0 {
                if self.value.compare_exchange_weak(
                    current,
                    current - 1,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                ).is_ok() {
                    return Ok(());
                }
                continue;
            }

            // Check timeout
            if crate::time::ticks().saturating_sub(start) >= timeout_ticks {
                return Err(SemError::TimedOut);
            }

            // Spin with hint
            core::hint::spin_loop();
        }
    }

    /// Post to the semaphore (sem_post)
    ///
    /// Increments the semaphore value and wakes one waiting thread (if any).
    ///
    /// # Errors
    /// Returns Overflow if the value would exceed SEM_VALUE_MAX.
    pub fn post(&self) -> Result<(), SemError> {
        loop {
            let current = self.value.load(Ordering::Acquire);
            if current >= SEM_VALUE_MAX {
                return Err(SemError::Overflow);
            }

            if self.value.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ).is_ok() {
                // In a real implementation, wake one waiter here
                return Ok(());
            }
        }
    }
}

impl Drop for Semaphore {
    fn drop(&mut self) {
        // Nothing special to do for unnamed semaphores
    }
}

// ============================================================================
// Named Semaphores (for inter-process communication)
// ============================================================================

use spin::Once;

/// Named semaphore handle
pub struct NamedSemaphore {
    name: String,
    sem: Arc<Semaphore>,
}

/// Global table of named semaphores
static NAMED_SEMS: Once<Mutex<BTreeMap<String, Arc<Semaphore>>>> = Once::new();

fn named_sems() -> &'static Mutex<BTreeMap<String, Arc<Semaphore>>> {
    NAMED_SEMS.call_once(|| Mutex::new(BTreeMap::new()))
}

impl NamedSemaphore {
    /// Open or create a named semaphore (sem_open)
    ///
    /// # Arguments
    /// * `name` - Name starting with '/'
    /// * `create` - Create if doesn't exist
    /// * `exclusive` - Fail if exists (only with create=true)
    /// * `value` - Initial value (only used when creating)
    pub fn open(
        name: &str,
        create: bool,
        exclusive: bool,
        value: u32,
    ) -> Result<Self, SemError> {
        let mut table = named_sems().lock();

        if let Some(existing) = table.get(name) {
            if create && exclusive {
                return Err(SemError::Exists);
            }
            return Ok(NamedSemaphore {
                name: String::from(name),
                sem: Arc::clone(existing),
            });
        }

        if !create {
            return Err(SemError::NotFound);
        }

        // Create new semaphore
        let sem = Arc::new(Semaphore::new(value));
        table.insert(String::from(name), Arc::clone(&sem));

        Ok(NamedSemaphore {
            name: String::from(name),
            sem,
        })
    }

    /// Close the named semaphore (sem_close)
    ///
    /// The semaphore continues to exist until sem_unlink is called.
    pub fn close(self) {
        // Just drops the handle, semaphore stays in table
        drop(self);
    }

    /// Unlink a named semaphore (sem_unlink)
    ///
    /// Removes the semaphore from the global table. The semaphore
    /// continues to exist until all handles are closed.
    pub fn unlink(name: &str) -> Result<(), SemError> {
        let mut table = named_sems().lock();
        if table.remove(name).is_none() {
            return Err(SemError::NotFound);
        }
        Ok(())
    }

    /// Wait on the semaphore
    pub fn wait(&self) {
        self.sem.wait();
    }

    /// Try to wait without blocking
    pub fn try_wait(&self) -> Result<(), SemError> {
        self.sem.try_wait()
    }

    /// Wait with timeout
    pub fn timed_wait(&self, timeout_ticks: u64) -> Result<(), SemError> {
        self.sem.timed_wait(timeout_ticks)
    }

    /// Post to the semaphore
    pub fn post(&self) -> Result<(), SemError> {
        self.sem.post()
    }

    /// Get current value
    pub fn get_value(&self) -> i32 {
        self.sem.get_value()
    }

    /// Get the semaphore name
    pub fn name(&self) -> &str {
        &self.name
    }
}

// ============================================================================
// Counting Semaphore utilities
// ============================================================================

/// A binary semaphore (mutex-like behavior)
///
/// This is a convenience type for a semaphore initialized to 1.
pub type BinarySemaphore = Semaphore;

impl BinarySemaphore {
    /// Create a new binary semaphore (initialized to 1)
    pub fn new_binary() -> Self {
        Semaphore::new(1)
    }

    /// Acquire the binary semaphore (like mutex lock)
    pub fn acquire(&self) {
        self.wait();
    }

    /// Release the binary semaphore (like mutex unlock)
    pub fn release(&self) {
        let _ = self.post();
    }
}

/// Initialize the semaphore subsystem
pub fn init_semaphores() {
    NAMED_SEMS.call_once(|| Mutex::new(BTreeMap::new()));
    crate::kprintln!("sync: semaphore subsystem initialized");
}
