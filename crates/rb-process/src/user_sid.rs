//! The current user's security identifier (RB-090-29): the one reader of the token's user SID,
//! for the private folder's DACL ([`crate::create_private_dir`]) and the instance pipe's
//! (`crates/rbrowser/src/instance/channel_windows.rs`).

use std::io;

#[cfg(windows)]
#[path = "user_sid_windows.rs"]
mod platform;

#[cfg(not(windows))]
mod platform {
    //! Outside Windows there is no SID.

    use std::io;

    pub fn current_user_sid() -> io::Result<String> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "a user SID exists on Windows only",
        ))
    }
}

/// The current user's SID as text (`S-1-5-21-...`), read from this process's token.
///
/// # Errors
///
/// The token cannot be read, or the platform has no SID (`ErrorKind::Unsupported` outside
/// Windows).
pub fn current_user_sid() -> io::Result<String> {
    platform::current_user_sid()
}

#[cfg(test)]
#[path = "user_sid_tests.rs"]
mod tests;
