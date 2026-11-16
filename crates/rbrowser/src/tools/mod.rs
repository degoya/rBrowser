//! Managed external tools (RB-080-02), after rDownloader's `rd-tools`: programs rBrowser does
//! not ship but downloads on first use, after the person agreed - in phase 1 only `ffmpeg`, the
//! LGPL video encoder of [`crate::video`].
//!
//! - [`manifest`]: the built-in, pinned manifest (`tools.json`): version, https address,
//!   SHA-256, size, members, licence and source address per tool and platform.
//! - [`fetch`]: the https download, replaceable in the tests.
//! - [`store`]: one install per tool across processes (a lock file), staging, hash check,
//!   unpacking, activation by rename and pointer, pruning and removal under
//!   `%LOCALAPPDATA%\rBrowser\tools`.
//!
//! Nothing here asks the person: the consent is the caller's (`crate::video::ffmpeg`), and no
//! MCP tool reaches an install.

pub mod fetch;
pub mod manifest;
pub mod store;

pub use manifest::{FFMPEG, Manifest, ToolEntry};
pub use store::{Installed, ToolStore};

/// Why a managed tool could not be had.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    /// The manifest breaks a rule (a pin not set, plain http, a GPL licence).
    #[error("the tool manifest is invalid: {0}")]
    Manifest(String),
    /// No build is pinned for this platform.
    #[error("no {tool} build is pinned for {platform}")]
    NoBuild {
        /// The tool.
        tool: String,
        /// The platform.
        platform: String,
    },
    /// The download failed or had another size.
    #[error("the download of {tool} failed: {reason}")]
    Download {
        /// The tool.
        tool: String,
        /// Why.
        reason: String,
    },
    /// The archive is not the pinned one.
    #[error(
        "{tool} {version} was not installed: its SHA-256 is {actual}, the pinned one is {expected}"
    )]
    HashMismatch {
        /// The tool.
        tool: String,
        /// The version.
        version: String,
        /// The manifest's digest.
        expected: String,
        /// The received bytes' digest.
        actual: String,
    },
    /// The archive could not be unpacked or lacks the program.
    #[error("unpacking {tool} failed: {reason}")]
    Unpack {
        /// The tool.
        tool: String,
        /// Why.
        reason: String,
    },
    /// Another install of the tool runs (this or another rBrowser process).
    #[error("another download of {tool} is running")]
    Busy {
        /// The tool.
        tool: String,
    },
    /// A file operation failed.
    #[error("{0}")]
    Io(String),
}

/// The pinned ffmpeg of this platform.
///
/// # Errors
///
/// The manifest is invalid, or no build is pinned for this platform.
pub fn ffmpeg_entry() -> Result<ToolEntry, ToolError> {
    let platform = manifest::current_platform().unwrap_or(std::env::consts::OS);
    Manifest::embedded()?.entry(FFMPEG, platform).cloned()
}
