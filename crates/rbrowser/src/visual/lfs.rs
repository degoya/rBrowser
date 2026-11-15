//! The Git behind `rb_visual::lfs`: `git lfs version` for the check after a baseline is saved,
//! `git lfs install --local` for the person's own set-up command. Every spawn without a console
//! window. `git lfs version` runs outside the project, so a project's own Git configuration (an
//! alias named `lfs`) cannot turn the check into a command of its choosing; the install runs in
//! the project, as it must, and only on the person's command. No command touches history.

use std::path::Path;
use std::process::{Command, Stdio};

use rb_process::NoConsoleWindow as _;
use rb_visual::lfs::Git;

/// The most characters of Git's error a notice shows.
const MAX_ERROR_CHARS: usize = 200;

/// The installed `git`.
pub struct SystemGit;

fn git() -> Command {
    let mut command = Command::new("git");
    command
        .stdin(Stdio::null())
        .env("GIT_TERMINAL_PROMPT", "0")
        .no_console_window();
    command
}

impl Git for SystemGit {
    fn lfs_installed(&self, _root: &Path) -> bool {
        git()
            .args(["lfs", "version"])
            .current_dir(std::env::temp_dir())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    fn lfs_install_local(&self, root: &Path) -> Result<(), String> {
        let output = git()
            .args(["lfs", "install", "--local"])
            .current_dir(root)
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            return Ok(());
        }
        Err(first_line(&String::from_utf8_lossy(&output.stderr)))
    }
}

/// The first non-empty line of `text`, cut to [`MAX_ERROR_CHARS`].
pub fn first_line(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("git lfs install failed");
    line.chars().take(MAX_ERROR_CHARS).collect()
}

#[cfg(test)]
#[path = "lfs_tests.rs"]
mod tests;
