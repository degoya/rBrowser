//! A folder only the current user can open (RB-090-21): the MCP instance file carries the
//! server's token, the plugin compile cache machine code that runs in rBrowser. On Unix the
//! folder gets mode 0700. On Windows it gets a protected DACL that grants the current user (the
//! token's user SID) and nobody else, inherited by every file and folder inside
//! ([`user_only_sddl`], the pipe's `UserOnly` of `crates/rbrowser/src/instance/channel_windows.rs`
//! made inheritable); it is set over what the folder already holds, so a data folder under a
//! self-made `C:\` path no longer inherits Modify for other local accounts (security review
//! RB-090-06, finding 3).

use std::io;
use std::path::Path;

#[cfg(windows)]
#[path = "private_dir_windows.rs"]
mod platform;

#[cfg(not(windows))]
mod platform {
    //! Unix: mode 0700; elsewhere nothing to set.

    use std::io;
    use std::path::Path;

    pub fn restrict(dir: &Path) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
        #[cfg(not(unix))]
        let _ = dir;
        Ok(())
    }
}

/// Creates `dir` and its parents and restricts `dir` to the current user; an existing folder is
/// restricted too.
///
/// # Errors
///
/// The folder cannot be created or its permissions cannot be set.
pub fn create_private_dir(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    platform::restrict(dir)
}

/// The Windows security descriptor (SDDL) of a private folder for the user with `sid`:
/// `P` protects the DACL from the parent's entries, and the one allow entry gives full file
/// access (`FA`) to the user, inherited by objects and containers inside (`OICI`).
pub fn user_only_sddl(sid: &str) -> String {
    format!("D:P(A;OICI;FA;;;{sid})")
}

#[cfg(test)]
#[path = "private_dir_tests.rs"]
mod tests;
