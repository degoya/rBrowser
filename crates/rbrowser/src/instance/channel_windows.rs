//! The channel on Windows: a named pipe `\\.\pipe\rbrowser-<key>-<random>` with a protected
//! DACL that grants the current user (the token's user SID, `rb_process::current_user_sid`) and nobody else, local clients only
//! (`PIPE_REJECT_REMOTE_CLIENTS`), the first instance claimed by us
//! (`FILE_FLAG_FIRST_PIPE_INSTANCE`). Win32 has no safe wrapper for the security descriptor, so
//! this file carries the unsafe blocks, each with its SAFETY note.

use std::ffi::c_void;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, PipeMode, ServerOptions};
use windows_sys::Win32::Foundation::{ERROR_PIPE_BUSY, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;

use super::Conn;

/// The prefix of every pipe of ours.
const PREFIX: &str = "\\\\.\\pipe\\rbrowser-";
/// How long a client waits while every pipe instance is busy.
const BUSY_WAIT: Duration = Duration::from_secs(2);

/// The pipe of the project with `key`; a random part keeps the name from being guessed.
pub fn endpoint_for(_dir: &Path, key: &str) -> String {
    // The nonce authenticates a caller; this part only keeps the name from being guessed, so a
    // missing random source (the nonce fails first) leaves it out.
    let random = crate::secret::random_hex(8).unwrap_or_default();
    format!("{PREFIX}{key}-{random}")
}

/// Whether `endpoint` is a pipe of ours: the prefix, then letters, digits and `-`.
pub fn valid_endpoint(endpoint: &str, _dir: &Path) -> bool {
    endpoint.len() <= 128
        && endpoint.strip_prefix(PREFIX).is_some_and(|rest| {
            !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

/// A listening pipe: the instance a client connects to next.
pub struct Listener {
    name: String,
    next: NamedPipeServer,
}

/// Listens at `endpoint`; fails when another process holds the name. Inside a tokio runtime.
pub fn bind(endpoint: &str) -> io::Result<Listener> {
    Ok(Listener {
        name: endpoint.to_owned(),
        next: create(endpoint, true)?,
    })
}

impl Listener {
    /// The next connection; a new pipe instance takes its place.
    pub async fn accept(&mut self) -> io::Result<Box<dyn Conn>> {
        self.next.connect().await?;
        let next = create(&self.name, false)?;
        Ok(Box::new(std::mem::replace(&mut self.next, next)))
    }
}

/// Connects to the instance at `endpoint`, waiting briefly while the pipe is busy.
pub async fn connect(endpoint: &str) -> io::Result<Box<dyn Conn>> {
    let deadline = Instant::now() + BUSY_WAIT;
    loop {
        match ClientOptions::new().open(endpoint) {
            Ok(client) => return Ok(Box::new(client)),
            Err(error)
                if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32)
                    && Instant::now() < deadline =>
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Lets the instance `pid` bring its window to the front: Windows gives the focus only to the
/// process the user is working in, which is this one.
#[allow(
    unsafe_code,
    reason = "AllowSetForegroundWindow has no safe wrapper; it takes a process id only"
)]
pub fn allow_foreground(pid: u32) {
    // SAFETY: a plain value argument; a failure only means the window stays where it is.
    if unsafe { AllowSetForegroundWindow(pid) } == 0 {
        tracing::debug!(error = %io::Error::last_os_error(), "the instance may not take the focus");
    }
}

/// One pipe instance with the user-only DACL.
#[allow(
    unsafe_code,
    reason = "tokio takes the security attributes as a raw pointer only"
)]
fn create(name: &str, first: bool) -> io::Result<NamedPipeServer> {
    let descriptor = UserOnly::new()?;
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(u32::MAX),
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let mut options = ServerOptions::new();
    options
        .first_pipe_instance(first)
        .reject_remote_clients(true)
        .pipe_mode(PipeMode::Byte);
    // SAFETY: `attributes` is a valid SECURITY_ATTRIBUTES whose descriptor (`descriptor`) lives
    // until this call has returned; CreateNamedPipeW copies what it needs.
    unsafe {
        options.create_with_security_attributes_raw(name, (&raw mut attributes).cast::<c_void>())
    }
}

/// A security descriptor `D:P(A;;GA;;;<user SID>)`, freed on drop.
struct UserOnly(PSECURITY_DESCRIPTOR);

impl UserOnly {
    #[allow(
        unsafe_code,
        reason = "ConvertStringSecurityDescriptorToSecurityDescriptorW has no safe wrapper"
    )]
    fn new() -> io::Result<Self> {
        let sddl = format!("D:P(A;;GA;;;{})", rb_process::current_user_sid()?);
        let wide: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
        let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: a NUL-terminated wide string and a valid out pointer; the size is not wanted.
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                std::ptr::null_mut(),
            )
        };
        if converted == 0 || descriptor.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(descriptor))
    }
}

impl Drop for UserOnly {
    #[allow(
        unsafe_code,
        reason = "LocalFree releases what ConvertStringSecurityDescriptorToSecurityDescriptorW allocated"
    )]
    fn drop(&mut self) {
        // SAFETY: allocated by the conversion above with LocalAlloc, freed once.
        unsafe { LocalFree(self.0) };
    }
}

#[cfg(test)]
#[path = "channel_windows_tests.rs"]
mod tests;
