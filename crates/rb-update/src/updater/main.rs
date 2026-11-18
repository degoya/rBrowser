//! `rbrowser-updater`: the program that switches an installed rBrowser to a verified update
//! (RB-090-14) - `rbrowser-updater apply-update --journal <data>/update/journal.json`.
//!
//! It ships beside `rbrowser.exe`, and the app starts a copy of it in `<data>/update/updater/`
//! ([`rb_update::install::process::launch_updater`]): a program cannot replace its own running
//! file on Windows, and the app's `rbrowser.exe` is CEF's sandbox bootstrap, which runs only
//! beside its `rbrowser.dll` and the CEF runtime - none of which a copy elsewhere would find.
//! This program links rb-update and nothing of the browser, so it runs from anywhere. What it
//! does is [`rb_update::install::apply::run`]; its exit code is that run's (`0` the new version
//! is in place, `2` the old one runs again, `1` anything else) and the journal says what
//! happened.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "rbrowser-updater",
    version,
    about = "Switch an installed rBrowser to a verified update"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Apply the update the journal hands over.
    #[command(name = "apply-update")]
    ApplyUpdate {
        /// `<data>/update/journal.json`.
        #[arg(long)]
        journal: PathBuf,
    },
}

/// The volume's free space is not measured here: `None` is "not refused"; a switch that runs out
/// of room fails and is taken back like any other.
fn unmeasured(_: &Path) -> Option<u64> {
    None
}

fn main() -> ExitCode {
    let Command::ApplyUpdate { journal } = Cli::parse().command;
    eprintln!(
        "rbrowser-updater {}: applying {}",
        env!("CARGO_PKG_VERSION"),
        journal.display()
    );
    match rb_update::install::apply::run(&journal, &unmeasured) {
        Ok(code) => {
            eprintln!("rbrowser-updater: ends with {code}");
            ExitCode::from(u8::try_from(code).unwrap_or(1))
        }
        Err(error) => {
            eprintln!("rbrowser-updater: {error:#}");
            ExitCode::FAILURE
        }
    }
}
