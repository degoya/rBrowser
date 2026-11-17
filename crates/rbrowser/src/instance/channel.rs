//! The local channel between a second call and the running instance, open to the same user
//! only: on Windows a named pipe whose DACL grants the current user alone and that refuses
//! remote clients (`channel_windows.rs`); elsewhere a Unix socket in the user's 0700 folder,
//! mode 0600, whose peers are checked for our uid (`channel_unix.rs`, what the tests run).

use tokio::io::{AsyncRead, AsyncWrite};

#[cfg(unix)]
#[path = "channel_unix.rs"]
mod imp;
#[cfg(windows)]
#[path = "channel_windows.rs"]
mod imp;

pub use imp::{Listener, allow_foreground, bind, connect, endpoint_for, valid_endpoint};

/// One connection, either side.
pub trait Conn: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> Conn for T {}
