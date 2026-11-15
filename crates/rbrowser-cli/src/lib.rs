//! `rbrowser.com`, the console twin of `rbrowser.exe` (RB-010-11).
//!
//! `rbrowser.exe` is CEF's `bootstrap.exe`, a GUI program (ADR 0001): PowerShell and cmd neither
//! wait for it nor show what it prints. This console program sits beside it as `rbrowser.com`,
//! which Windows finds first for a bare `rbrowser`, starts `rbrowser.exe` with the same
//! arguments, hands it the terminal's input, relays its output and ends with its exit code.
//!
//! Wait or not: the twin cannot know from the arguments whether a call ends in the terminal
//! (`init`, `diagnostics`, `--help`, `--version`, `--reload`, a hand-over to the running
//! instance, an error) or opens the window. `rbrowser.exe` tells it: right before the window
//! opens it closes the output it was given (`rbrowser::cli::console`). So the twin relays until
//! both pipes end, then waits [`GRACE`] for the exit: a program that ended returns its code, one
//! still running is the app, which runs on while the terminal is free again (exit code 0).

use std::ffi::OsStr;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use rb_process::NoConsoleWindow as _;

/// The GUI program the twin starts, in its own folder.
pub const GUI_EXE: &str = "rbrowser.exe";

/// How long the twin waits for the exit once the program closed its output: a program that
/// ends closes its pipes a moment before its exit can be read; one still running after this is
/// the app with its window.
pub const GRACE: Duration = Duration::from_millis(500);

/// The exit code when `rbrowser.exe` cannot be started (`cli::exit::FAILED` of the app).
const FAILED: i32 = 1;

/// How a call ended.
#[derive(Debug)]
pub enum Outcome {
    /// The program ended with this exit code.
    Exited(i32),
    /// The program let go of its output and runs on: the app opened its window.
    Detached(Child),
}

/// `rbrowser.exe` in the folder of `own` (the twin's own path); `None` when that would be the
/// twin itself or `own` has no folder.
pub fn gui_beside(own: &Path) -> Option<PathBuf> {
    let gui = own.parent()?.join(GUI_EXE);
    let own_name = own.file_name()?;
    if own_name.eq_ignore_ascii_case(GUI_EXE) || own.parent()?.as_os_str().is_empty() {
        return None;
    }
    Some(gui)
}

/// Starts `gui` with `args` and the terminal's input, relays its output to `out` and `err`
/// until it closes both, and returns how it ended ([`Outcome`]).
///
/// # Errors
///
/// The program could not be started, or its state could not be read.
pub fn run<I, S>(
    gui: &Path,
    args: I,
    out: impl Write + Send,
    err: impl Write + Send,
    grace: Duration,
) -> std::io::Result<Outcome>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    // rbrowser.exe is a GUI program: CREATE_NO_WINDOW changes nothing for it, and a console
    // program put there instead opens no window of its own either.
    let mut child = Command::new(gui)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .no_console_window()
        .spawn()?;
    let child_out = child.stdout.take();
    let child_err = child.stderr.take();
    std::thread::scope(|scope| {
        if let Some(mut pipe) = child_err {
            scope.spawn(move || relay(&mut pipe, err));
        }
        if let Some(mut pipe) = child_out {
            relay(&mut pipe, out);
        }
    });
    let deadline = Instant::now() + grace;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Outcome::Exited(status.code().unwrap_or(FAILED)));
        }
        if Instant::now() >= deadline {
            return Ok(Outcome::Detached(child));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Copies `from` to `to` as it comes, chunk by chunk, so a prompt without a line end shows at
/// once. A sink that fails (the terminal closed) is dropped, but `from` is read to its end, so
/// the program never blocks on a full pipe.
pub fn relay(from: &mut impl Read, mut to: impl Write) {
    let mut buffer = [0_u8; 8192];
    let mut sink = true;
    loop {
        let read = match from.read(&mut buffer) {
            Ok(0) => return,
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return,
        };
        if sink {
            let chunk = buffer.get(..read).unwrap_or_default();
            sink = to.write_all(chunk).and_then(|()| to.flush()).is_ok();
        }
    }
}

/// The twin's `main`: the exit code of `rbrowser.exe`, 0 once the app runs on its own.
pub fn main_code() -> i32 {
    let gui = match std::env::current_exe() {
        Ok(own) => gui_beside(&own),
        Err(error) => {
            eprintln!("rbrowser: the console program cannot find itself: {error}");
            return FAILED;
        }
    };
    let Some(gui) = gui else {
        eprintln!("rbrowser: the console program must sit beside {GUI_EXE} under another name");
        return FAILED;
    };
    match run(
        &gui,
        std::env::args_os().skip(1),
        std::io::stdout(),
        std::io::stderr(),
        GRACE,
    ) {
        Ok(Outcome::Exited(code)) => code,
        Ok(Outcome::Detached(_)) => 0,
        Err(error) => {
            eprintln!("rbrowser: {} cannot be started: {error}", gui.display());
            FAILED
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
