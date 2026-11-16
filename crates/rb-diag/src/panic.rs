//! The panic hook: every panic goes into the log with its backtrace before the hook that was
//! there runs - the GUI binary has no console, so a panic would otherwise leave no trace.

use std::any::Any;
use std::backtrace::Backtrace;
use std::fmt::Display;
use std::panic::Location;

/// The log target of a panic.
pub const PANIC_TARGET: &str = "panic";

/// Installs the hook; the previous hook (the default one prints to stderr) runs after it.
pub fn install_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = Backtrace::force_capture();
        tracing::error!(
            target: PANIC_TARGET,
            "{}",
            panic_report(info.payload(), info.location(), &backtrace)
        );
        previous(info);
    }));
}

/// A panic as text: where, on which thread, its message, then the backtrace.
pub fn panic_report(
    payload: &(dyn Any + Send),
    location: Option<&Location<'_>>,
    backtrace: &dyn Display,
) -> String {
    let message = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("a panic without a message");
    let place = location.map_or_else(
        || "an unknown place".to_owned(),
        |location| format!("{}:{}", location.file(), location.line()),
    );
    let thread = std::thread::current();
    let thread = thread.name().unwrap_or("an unnamed thread");
    format!("panicked at {place} on {thread}: {message}\nbacktrace:\n{backtrace}")
}

#[cfg(test)]
#[path = "panic_tests.rs"]
mod tests;
