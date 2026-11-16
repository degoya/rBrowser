//! The terminal of a command line call (RB-010-11). `rbrowser.exe` is CEF's `bootstrap.exe`, a
//! GUI program (ADR 0001), so it starts without a console:
//!
//! - From `rbrowser.com`, the console twin (`crates/rbrowser-cli`), it gets the terminal's input
//!   and a pipe each for its output and errors; [`attach`] joins the twin's console too, so the
//!   input works as a terminal (`init` asks there).
//! - Started straight from a shell, it gets no handles at all; [`attach`] joins the shell's
//!   console and writes there (`CONOUT$`), but never reads it: the shell reads the same console
//!   and does not wait.
//!
//! Before the window opens, [`release`] closes every handle and leaves the console: the twin
//! sees its pipes end and frees the terminal, and closing that terminal later does not end the
//! app. Both are called on the main thread before CEF starts, and need no runtime. Elsewhere than
//! Windows they do nothing.

#[cfg(windows)]
#[path = "console_windows.rs"]
mod win32;

/// What [`attach`] does with one standard handle (input, output, errors).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// The caller handed one over (the twin's pipe, a redirect): kept.
    Keep,
    /// None was given and a console is attached: its screen, `CONOUT$`.
    Console,
    /// None: reading finds the end, writing goes nowhere.
    Empty,
}

/// The handles after [`attach`], from whether the caller `given` input, output and errors and
/// whether a console is `attached`. An input that was not given stays empty: the console then
/// belongs to a shell that reads it itself.
pub fn slots(given: [bool; 3], attached: bool) -> [Slot; 3] {
    let [input, output, errors] = given;
    let write = |given: bool| match (given, attached) {
        (true, _) => Slot::Keep,
        (false, true) => Slot::Console,
        (false, false) => Slot::Empty,
    };
    [
        if input { Slot::Keep } else { Slot::Empty },
        write(output),
        write(errors),
    ]
}

/// Joins the console of the program that started this one, so a command's output reaches the
/// terminal ([`slots`]). First thing in the browser process; a program with a console of its own
/// (`rbrowser-nosandbox.exe`) keeps it untouched.
pub fn attach() {
    #[cfg(windows)]
    win32::attach();
}

/// Lets go of the terminal before the window opens: flushes, closes the standard handles and
/// leaves the console [`attach`] joined. Does nothing when [`attach`] did nothing.
pub fn release() {
    use std::io::Write as _;
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    #[cfg(windows)]
    win32::release();
}

#[cfg(test)]
#[path = "console_tests.rs"]
mod tests;
