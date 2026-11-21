//! The clients' files on disk (RB-091-09): where each user file lies, reading one, and writing
//! one - a user file only after the person confirmed it and with a copy of the old file beside
//! it (`<name>.rbrowser-<seconds>.bak`), a project file without one (it is the project's, under
//! version control).

use std::io;
use std::path::{Path, PathBuf};

use rb_ipc::McpClient;

/// The system the user files are looked for on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    /// Windows: Claude Desktop under `%APPDATA%`.
    Windows,
    /// macOS: Claude Desktop under `~/Library/Application Support`.
    MacOs,
    /// Linux and the rest: Claude Desktop under `~/.config`.
    Other,
}

impl Os {
    /// This build's system.
    #[must_use]
    pub const fn here() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Other
        }
    }
}

/// Where the clients that are set up by file keep their user file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UserFiles {
    cursor: Option<PathBuf>,
    claude_desktop: Option<PathBuf>,
}

impl UserFiles {
    /// The files on `os` for the home folder `home` and, on Windows, `%APPDATA%`.
    #[must_use]
    pub fn locate(os: Os, home: Option<&Path>, app_data: Option<&Path>) -> Self {
        let desktop = match os {
            Os::Windows => app_data.map(Path::to_path_buf),
            Os::MacOs => home.map(|home| home.join("Library").join("Application Support")),
            Os::Other => home.map(|home| home.join(".config")),
        };
        Self {
            cursor: home.map(|home| home.join(".cursor").join("mcp.json")),
            claude_desktop: desktop
                .map(|folder| folder.join("Claude").join("claude_desktop_config.json")),
        }
    }

    /// The files of the person running rBrowser.
    #[must_use]
    pub fn here() -> Self {
        let dir = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
        let home = dir("USERPROFILE")
            .or_else(|| dir("HOME"))
            .map(PathBuf::from);
        let app_data = dir("APPDATA").map(PathBuf::from);
        Self::locate(Os::here(), home.as_deref(), app_data.as_deref())
    }

    /// `client`'s user file; `None` for a client set up by its command.
    #[must_use]
    pub fn of(&self, client: McpClient) -> Option<&Path> {
        match client {
            McpClient::Cursor => self.cursor.as_deref(),
            McpClient::ClaudeDesktop => self.claude_desktop.as_deref(),
            _ => None,
        }
    }
}

/// The text of `path`; `None` when there is no such file.
///
/// # Errors
///
/// The file exists and cannot be read.
pub fn read(path: &Path) -> io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// The copy of `path` made at `seconds` since the epoch.
#[must_use]
pub fn backup_path(path: &Path, seconds: u64) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".rbrowser-{seconds}.bak"));
    path.with_file_name(name)
}

/// Writes `text` to `path` (its folder made when missing). With `backup_at`, an existing file
/// is copied first to [`backup_path`] - never over an older copy - and the copy is returned.
///
/// # Errors
///
/// The folder, the copy or the file cannot be written; nothing is written when the copy fails.
pub fn write(path: &Path, text: &str, backup_at: Option<u64>) -> io::Result<Option<PathBuf>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let backup = match backup_at {
        Some(seconds) if path.is_file() => {
            let mut copy = backup_path(path, seconds);
            let mut next = 1_u32;
            while copy.exists() {
                copy = backup_path(path, seconds).with_extension(format!("{next}.bak"));
                next += 1;
            }
            std::fs::copy(path, &copy)?;
            Some(copy)
        }
        _ => None,
    };
    rb_util::fs::write_atomic(path, text.as_bytes())?;
    Ok(backup)
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
