//! What every program rBrowser starts has in common.
//!
//! Window: on Windows a console program started by a process without a console (the GUI binary)
//! opens a console window of its own. [`NoConsoleWindow`] is the one way every spawn in the
//! workspace avoids it; `crates/rbrowser/tests/repo_lints/no_console_window.rs` fails on a
//! `Command::new` that does not use it.
//!
//! Tree: [`ProcessJob`] ties a child's whole process tree to its owner on Windows (RB-080-67).
//!
//! Files: [`create_private_dir`] makes a folder only the current user can open, for what a
//! process of another account must not read or plant (RB-090-21), and [`current_user_sid`] reads
//! the token's user SID such a folder and the instance pipe grant (RB-090-29).
//!
//! Output: [`output_capped`] reads a child's output with a cap ([`MAX_OUTPUT`], RB-095-05).
//!
//! Priority: [`low_priority_command`] starts a background program (a video encoder, RB-090-52)
//! below the app's priority, so it never takes the CPU from the browser views.

pub mod background;
pub mod capped;
/// A DACL read back from Windows, for the tests of the private folder and the instance pipe
/// (RB-099-17); never in a shipped build.
#[cfg(all(windows, any(test, feature = "test-support")))]
#[path = "dacl_windows.rs"]
pub mod dacl;
pub mod job;
pub mod private_dir;
pub mod user_sid;

pub use background::{BELOW_NORMAL_PRIORITY_CLASS, NICE_INCREMENT};
pub use capped::{MAX_OUTPUT, output_capped};
pub use job::{ProcessJob, adopted_pids, spawn_in_job, untied_spawns};
pub use private_dir::create_private_dir;
pub use user_sid::current_user_sid;

/// `CREATE_NO_WINDOW`: a console program started with it gets a console of its own that is
/// never shown, and the console programs it starts in turn share that hidden console.
///
/// Public for a caller that has to combine it with other creation flags; every other spawn calls
/// [`NoConsoleWindow::no_console_window`], because `creation_flags` replaces the flags set before
/// rather than adding to them.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Starts a program without a console window on Windows; nothing changes elsewhere.
///
/// Spawns that must show a window on purpose (an external editor, a terminal the user asked
/// for) do not use this and carry an allow comment instead (see the repo lint).
pub trait NoConsoleWindow {
    /// Sets [`CREATE_NO_WINDOW`] as the creation flags on Windows; a no-op elsewhere.
    fn no_console_window(&mut self) -> &mut Self;
}

impl NoConsoleWindow for tokio::process::Command {
    fn no_console_window(&mut self) -> &mut Self {
        #[cfg(windows)]
        self.creation_flags(CREATE_NO_WINDOW);
        self
    }
}

impl NoConsoleWindow for std::process::Command {
    fn no_console_window(&mut self) -> &mut Self {
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(self, CREATE_NO_WINDOW);
        self
    }
}

/// A command that starts `program` as a background program without a console window: on
/// Windows in the [`BELOW_NORMAL_PRIORITY_CLASS`] (with [`CREATE_NO_WINDOW`]), on Unix through
/// `nice -n` [`NICE_INCREMENT`] where the system has `nice` (`background`). Add the program's
/// own arguments to it as to any command.
#[must_use]
pub fn low_priority_command(program: &std::path::Path) -> tokio::process::Command {
    use tokio::process::Command;
    let (program, leading) = background::low_priority_argv(program, background::nice_program());
    let mut command = Command::new(program);
    command.args(leading).no_console_window();
    // `creation_flags` replaces the flags `no_console_window` set: both again, with the class.
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    command
}

/// `CREATE_NEW_PROCESS_GROUP`: a Ctrl-C meant for the starter does not reach the program.
pub const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
/// `CREATE_BREAKAWAY_FROM_JOB`: the program leaves the starter's job, so it outlives it.
pub const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

/// Starts a program that outlives the one that started it (the self-updater of rb-update and the
/// version it starts): on Windows [`CREATE_NO_WINDOW`] with a process group of its own and, with
/// `breakaway`, outside the starter's job; on Unix a process group of its own. Here because only
/// this file sets creation flags (`repo_lints/no_console_window.rs`).
pub trait Detached {
    /// Sets the flags; a job that allows no breakaway refuses the spawn with error 5, and the
    /// caller tries again with `breakaway` off.
    fn detached(&mut self, breakaway: bool) -> &mut Self;
}

impl Detached for std::process::Command {
    fn detached(&mut self, breakaway: bool) -> &mut Self {
        #[cfg(windows)]
        {
            let job = if breakaway {
                CREATE_BREAKAWAY_FROM_JOB
            } else {
                0
            };
            std::os::windows::process::CommandExt::creation_flags(
                self,
                CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP | job,
            );
        }
        #[cfg(unix)]
        {
            let _ = breakaway;
            std::os::unix::process::CommandExt::process_group(self, 0);
        }
        self
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
