//! The channel on Unix: a socket in the instance folder (0700), mode 0600, and every peer's
//! uid checked against the socket's owner. Phase 1 ships on Windows only; this side keeps the
//! hand-over tested on Linux and is the base for phase 2.

use std::io;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::Path;

use tokio::net::{UnixListener, UnixStream};

use super::Conn;

/// The socket of the project with `key` in the instance folder `dir`.
pub fn endpoint_for(dir: &Path, key: &str) -> String {
    dir.join(format!("{key}.sock"))
        .to_string_lossy()
        .into_owned()
}

/// Whether `endpoint` is a socket in the instance folder `dir`.
pub fn valid_endpoint(endpoint: &str, dir: &Path) -> bool {
    let path = Path::new(endpoint);
    path.parent() == Some(dir) && path.extension().is_some_and(|ext| ext == "sock")
}

/// A listening socket; accepts only peers of the same uid.
pub struct Listener {
    inner: UnixListener,
    uid: u32,
}

/// Listens at `endpoint` (a socket left by a crash is replaced). Inside a tokio runtime.
pub fn bind(endpoint: &str) -> io::Result<Listener> {
    let path = Path::new(endpoint);
    if let Some(dir) = path.parent() {
        crate::instance::file::create_private_dir(dir)?;
    }
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let inner = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    let uid = std::fs::metadata(path)?.uid();
    Ok(Listener { inner, uid })
}

impl Listener {
    /// The next connection of a peer with our uid; others are dropped unanswered.
    pub async fn accept(&mut self) -> io::Result<Box<dyn Conn>> {
        loop {
            let (stream, _) = self.inner.accept().await?;
            match stream.peer_cred() {
                Ok(cred) if cred.uid() == self.uid => return Ok(Box::new(stream)),
                _ => tracing::warn!("a connection from another user was refused"),
            }
        }
    }
}

/// Connects to the instance at `endpoint`.
pub async fn connect(endpoint: &str) -> io::Result<Box<dyn Conn>> {
    Ok(Box::new(UnixStream::connect(endpoint).await?))
}

/// Lets the instance `pid` take the focus (Windows only).
pub fn allow_foreground(pid: u32) {
    let _ = pid;
}
