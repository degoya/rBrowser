//! A mutex lock that survives a panic of another holder.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks `mutex`, taking over the data of a thread that panicked while holding it: the state
/// stays usable for the rest, as every holder keeps it consistent at each point it can panic.
pub fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "lock_tests.rs"]
mod tests;
