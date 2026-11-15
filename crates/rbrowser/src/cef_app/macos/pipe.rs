//! The CDP pipe on macOS (RB-090-15, ADR 0001 Decision 4; the Windows twin is `../pipe.rs`).
//! With `--remote-debugging-pipe` Chromium's pipe handler reads from descriptor 3 and writes to
//! descriptor 4 of the browser process (`--remote-debugging-io-pipes` is Windows' only, and is
//! ignored here). [`reserve`] makes both pipes first thing in `run_plain`, before anything
//! opens a file that could take those numbers: Chromium's ends are moved to 3 and 4, ours to
//! [`OURS_FROM`] and above; all four are close-on-exec, so no child process (the sidecar,
//! Firefox) inherits them. The router's NUL-framed JSON runs over them as on Windows. No TCP
//! port is opened.
//!
//! `libc` has no safe wrapper for `fcntl` and `dup2`, so this file carries the unsafe blocks,
//! each with its SAFETY note.

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::sync::{Arc, Mutex};

use rb_engine::EngineError;

use crate::cdp::{CdpRouter, CdpTransport, FrameDecoder, lock};

/// The descriptor Chromium reads the messages from.
const CHROMIUM_READ: RawFd = 3;
/// The descriptor Chromium writes its messages to.
const CHROMIUM_WRITE: RawFd = 4;
/// Our ends are moved to this descriptor or above, clear of Chromium's.
const OURS_FROM: RawFd = 10;

/// Our ends of the two pipes, as raw descriptors (handed to [`connect`] once).
#[derive(Clone, Copy, Debug)]
pub struct PipeEnds {
    /// What Chromium writes, we read.
    read: RawFd,
    /// What we write, Chromium reads.
    write: RawFd,
}

impl PipeEnds {
    /// The value of `--remote-debugging-io-pipes`, which Chromium reads on Windows only: the
    /// descriptors it uses here, for the log.
    pub fn switch_value(&self) -> String {
        format!("{CHROMIUM_READ},{CHROMIUM_WRITE}")
    }
}

/// The ends [`reserve`] made, until the launch takes them ([`create`]); the error otherwise.
static RESERVED: Mutex<Option<Result<PipeEnds, String>>> = Mutex::new(None);

/// Makes both pipes and puts Chromium's ends on descriptors 3 and 4. First thing in the app's
/// `run_plain`, before the log or anything else opens a file.
pub fn reserve() {
    *lock(&RESERVED) = Some(make().map_err(|error| error.to_string()));
}

/// The ends [`reserve`] made; `None` (logged) when it failed or did not run: then the app runs
/// without CDP.
pub fn create() -> Option<PipeEnds> {
    match lock(&RESERVED).take() {
        Some(Ok(ends)) => Some(ends),
        Some(Err(error)) => {
            tracing::error!(%error, "the CDP pipes could not be made; the app runs without CDP");
            None
        }
        None => {
            tracing::error!("the CDP pipes were not reserved; the app runs without CDP");
            None
        }
    }
}

fn make() -> std::io::Result<PipeEnds> {
    let (to_chromium_read, to_chromium_write) = std::io::pipe()?;
    let (from_chromium_read, from_chromium_write) = std::io::pipe()?;
    // Every end moved up first, so placing Chromium's on 3 and 4 never closes one of the others.
    let ours_read = move_up(&from_chromium_read)?;
    let ours_write = move_up(&to_chromium_write)?;
    let chromium_read = move_up(&to_chromium_read)?;
    let chromium_write = move_up(&from_chromium_write)?;
    drop((
        to_chromium_read,
        to_chromium_write,
        from_chromium_read,
        from_chromium_write,
    ));
    place(&chromium_read, CHROMIUM_READ)?;
    place(&chromium_write, CHROMIUM_WRITE)?;
    // The copies above 10 close here; 3 and 4 stay open for Chromium for the process's life.
    drop((chromium_read, chromium_write));
    Ok(PipeEnds {
        read: ours_read.into_raw_fd(),
        write: ours_write.into_raw_fd(),
    })
}

/// A close-on-exec copy of `fd` on [`OURS_FROM`] or the first free descriptor above.
#[allow(
    unsafe_code,
    reason = "fcntl(F_DUPFD_CLOEXEC) has no safe wrapper in std"
)]
fn move_up(fd: &impl AsRawFd) -> std::io::Result<OwnedFd> {
    // SAFETY: `fd` is an open descriptor we own; F_DUPFD_CLOEXEC only reads it and returns a
    // new descriptor (or -1), which nothing else owns.
    let copy = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, OURS_FROM) };
    if copy < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `copy` was just returned by fcntl and is owned by nobody else.
    Ok(unsafe { OwnedFd::from_raw_fd(copy) })
}

/// Makes `target` a close-on-exec copy of `fd`; whatever `target` held before is closed (the
/// pipes are made before anything else opens a file, so it held nothing of ours).
#[allow(
    unsafe_code,
    reason = "dup2 and fcntl(F_SETFD) have no safe wrapper in std"
)]
fn place(fd: &OwnedFd, target: RawFd) -> std::io::Result<()> {
    // SAFETY: `fd` is an open descriptor we own and `target` a descriptor number; dup2 only
    // makes `target` refer to the same pipe.
    if unsafe { libc::dup2(fd.as_raw_fd(), target) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `target` is open (dup2 succeeded); F_SETFD only sets its flags.
    if unsafe { libc::fcntl(target, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Writes the router's messages into the pipe.
struct PipeWriter {
    file: Mutex<File>,
}

impl CdpTransport for PipeWriter {
    fn send(&self, frame: Vec<u8>) -> Result<(), EngineError> {
        lock(&self.file)
            .write_all(&frame)
            .map_err(|_| EngineError::Closed)
    }
}

/// Connects `ends` to a new router: the writer goes into it, a reader thread feeds it every
/// message and closes it when Chromium closes its end.
#[allow(
    unsafe_code,
    reason = "the descriptors come as raw numbers through the launch; they are taken here once"
)]
pub fn connect(ends: PipeEnds) -> Arc<CdpRouter> {
    // SAFETY: both descriptors were made by `make` for this launch, are open, and are handed to
    // this one call (the launch has one pipe, `start` connects it once).
    let (read, write) = unsafe { (File::from_raw_fd(ends.read), File::from_raw_fd(ends.write)) };
    let router = CdpRouter::new(Arc::new(PipeWriter {
        file: Mutex::new(write),
    }));
    let reader = router.clone();
    let spawned = std::thread::Builder::new()
        .name("rb-cdp-pipe".to_owned())
        .spawn(move || read_loop(read, &reader));
    if let Err(error) = spawned {
        tracing::error!(%error, "the CDP reader could not start");
        router.on_closed();
    }
    router
}

fn read_loop(mut read: File, router: &CdpRouter) {
    let mut buffer = vec![0u8; 1 << 16];
    let mut decoder = FrameDecoder::default();
    loop {
        let count = match read.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                tracing::warn!(%error, "the CDP pipe failed");
                break;
            }
        };
        for message in decoder.push(&buffer[..count]) {
            match serde_json::from_slice(&message) {
                Ok(value) => router.on_message(value),
                Err(error) => tracing::warn!(%error, "a CDP message did not parse"),
            }
        }
    }
    tracing::info!("the CDP pipe closed");
    router.on_closed();
}
