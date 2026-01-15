    use core::ops::{Deref, DerefMut};
    use spin::{Mutex, MutexGuard};

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
            crate::arch::interrupts::restore(self.irq_was_enabled);
        }
    }
