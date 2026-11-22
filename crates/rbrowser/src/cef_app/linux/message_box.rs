//! The message for what the user must see before any window exists on Linux (RB-097-05): CEF
//! did not start (`crate::start_failure`) or no Chromium sandbox can (`super::linux`). Started
//! from the desktop menu there is no terminal to read stderr in, so with a display the text also
//! shows in the desktop's own dialog - `zenity` (GNOME and most others), else `kdialog` (KDE),
//! found on `PATH` and started without a shell, the text one argument. Without either the text
//! goes to stderr only; the caller has logged it already, so the log and the diagnostics ZIP
//! have it too.

use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use rb_process::NoConsoleWindow as _;

/// Writes `title` and `text` to stderr and, with a display, shows them in an error dialog and
/// waits until it is closed.
pub fn error(title: &str, text: &str) {
    eprintln!("{title}: {text}");
    let display = ["DISPLAY", "WAYLAND_DISPLAY"]
        .iter()
        .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()));
    let path = std::env::var_os("PATH");
    let Some((program, args)) = dialog(title, text, display, path.as_deref()) else {
        return;
    };
    let shown = std::process::Command::new(&program)
        .args(&args)
        .no_console_window()
        .status();
    if let Err(error) = shown {
        tracing::warn!(%error, program = %program.display(), "the error dialog did not open");
    }
}

/// The dialog program on `path` and its arguments for `title` and `text`; `None` without a
/// display or without zenity and kdialog.
pub fn dialog(
    title: &str,
    text: &str,
    display: bool,
    path: Option<&OsStr>,
) -> Option<(PathBuf, Vec<String>)> {
    if !display {
        return None;
    }
    let path = path?;
    let found = |name: &str| {
        std::env::split_paths(path)
            .map(|dir| dir.join(name))
            .find(|candidate| is_executable(candidate))
    };
    if let Some(zenity) = found("zenity") {
        // `--name=value`: a text can never be read as an option; no markup, so `<` stays text.
        let args = vec![
            "--error".to_owned(),
            "--no-markup".to_owned(),
            format!("--title={title}"),
            format!("--text={text}"),
        ];
        return Some((zenity, args));
    }
    found("kdialog").map(|kdialog| {
        let args = vec![
            "--title".to_owned(),
            title.to_owned(),
            "--error".to_owned(),
            text.to_owned(),
        ];
        (kdialog, args)
    })
}

/// Whether `path` is a file someone may run.
fn is_executable(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
#[path = "message_box_tests.rs"]
mod tests;
