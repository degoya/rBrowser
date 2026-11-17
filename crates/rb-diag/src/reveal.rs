//! Shows a file in the file manager: Explorer with the file selected on Windows (its folder when
//! the file is gone, an error when the folder is gone too - never Explorer's default folder), the
//! folder through `xdg-open` elsewhere. Never with a console window (`rb-process`).

use std::io;
use std::path::Path;

/// Opens the folder of `path` with `path` selected where the platform can. Waits for the
/// launcher, which hands over and exits at once (Explorer exits with 1 even on success, so the
/// status is not read).
pub fn reveal(path: &Path) -> io::Result<()> {
    platform::reveal(path)
}

/// What Explorer is asked to show.
#[cfg(any(windows, test))]
#[derive(Debug, PartialEq, Eq)]
enum Explorer {
    /// The folder of an existing file, with the file selected.
    Select(String),
    /// The folder of a file that does not exist (any more).
    Open(String),
}

/// What Explorer shows for the Windows path `path`, with `exists` asked of the path Explorer
/// gets: the file selected when it exists, else its folder when that exists, else an error.
/// The path loses a `\\?\UNC\` (back to `\\`) or `\\?\` prefix, which Explorer does not read,
/// and has backslashes only.
#[cfg(any(windows, test))]
fn explorer_target(path: &str, exists: impl Fn(&str) -> bool) -> io::Result<Explorer> {
    let path = explorer_path(path);
    if exists(&path) {
        return Ok(Explorer::Select(path));
    }
    match folder_of(&path) {
        Some(folder) if exists(&folder) => Ok(Explorer::Open(folder)),
        _ => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "neither the file nor its folder exists",
        )),
    }
}

/// `path` with backslashes and without the long-path prefix.
#[cfg(any(windows, test))]
fn explorer_path(path: &str) -> String {
    let path = path.replace('/', "\\");
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    match path.strip_prefix(r"\\?\") {
        Some(rest) => rest.to_owned(),
        None => path,
    }
}

/// The folder of the Windows path `path`: `C:\` for a file at a drive's root, nothing above a
/// share's root `\\server\share`.
#[cfg(any(windows, test))]
fn folder_of(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('\\');
    let head = &trimmed[..trimmed.rfind('\\')?];
    if let Some(share) = head.strip_prefix(r"\\") {
        let parts = share.split('\\').filter(|part| !part.is_empty()).count();
        return (parts >= 2).then(|| head.to_owned());
    }
    if head.len() == 2 && head.ends_with(':') {
        return Some(format!("{head}\\"));
    }
    (!head.is_empty()).then(|| head.to_owned())
}

#[cfg(windows)]
mod platform {
    use std::io;
    use std::os::windows::process::CommandExt as _;
    use std::path::Path;
    use std::process::Command;

    use rb_process::NoConsoleWindow as _;

    use super::{Explorer, explorer_target};

    pub(super) fn reveal(path: &Path) -> io::Result<()> {
        // Explorer parses its own command line: `/select,"<path>"` or `"<folder>"` as one raw
        // argument.
        let argument =
            match explorer_target(&path.to_string_lossy(), |path| Path::new(path).exists())? {
                Explorer::Select(file) => format!("/select,\"{file}\""),
                Explorer::Open(folder) => format!("\"{folder}\""),
            };
        Command::new("explorer.exe")
            .raw_arg(argument)
            .no_console_window()
            .status()
            .map(drop)
    }
}

#[cfg(not(windows))]
mod platform {
    use std::io;
    use std::path::Path;
    use std::process::Command;

    use rb_process::NoConsoleWindow as _;

    pub(super) fn reveal(path: &Path) -> io::Result<()> {
        let folder = path.parent().unwrap_or(path);
        Command::new("xdg-open")
            .arg(folder)
            .no_console_window()
            .status()
            .map(drop)
    }
}

#[cfg(test)]
#[path = "reveal_tests.rs"]
mod tests;
