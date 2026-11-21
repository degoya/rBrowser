//! The CDP pipe (ADR 0001, Decision 4): two anonymous pipes created before CEF starts; their
//! Chromium ends go to `--remote-debugging-io-pipes=<read>,<write>` (valid in-process, no
//! inheritance), our ends carry the NUL-framed JSON of the router. No TCP port is opened.

use std::sync::Arc;

use rb_engine::EngineError;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows_sys::Win32::System::Pipes::CreatePipe;

use crate::cdp::{CdpRouter, FrameDecoder, ThreadWriter};

/// The pipe buffer size.
const BUFFER: u32 = 1 << 20;

/// Our ends of the two pipes, as raw handle values (a `HANDLE` is not `Send`).
#[derive(Clone, Copy, Debug)]
pub struct PipeEnds {
    /// What Chromium writes, we read.
    read: usize,
    /// What we write, Chromium reads.
    write: usize,
    /// The value of `--remote-debugging-io-pipes`.
    chromium: (usize, usize),
}

impl PipeEnds {
    /// The value of `--remote-debugging-io-pipes`: Chromium's read end, then its write end.
    pub fn switch_value(&self) -> String {
        format!("{},{}", self.chromium.0, self.chromium.1)
    }
}

/// Creates both pipes. `None` when Windows refuses (then the app runs without CDP).
#[allow(
    unsafe_code,
    reason = "CreatePipe has no safe wrapper; it only writes the four handles"
)]
pub fn create() -> Option<PipeEnds> {
    let mut ends: [HANDLE; 4] = [std::ptr::null_mut(); 4];
    let [
        to_chromium_read,
        to_chromium_write,
        from_chromium_read,
        from_chromium_write,
    ] = &mut ends;
    // SAFETY: valid out pointers, no security attributes (not inheritable), a fixed size.
    let created = unsafe {
        CreatePipe(
            to_chromium_read,
            to_chromium_write,
            std::ptr::null(),
            BUFFER,
        ) != 0
            && CreatePipe(
                from_chromium_read,
                from_chromium_write,
                std::ptr::null(),
                BUFFER,
            ) != 0
    };
    if !created {
        tracing::error!(error = %std::io::Error::last_os_error(), "the CDP pipes could not be created");
        return None;
    }
    Some(PipeEnds {
        read: ends[2] as usize,
        write: ends[1] as usize,
        chromium: (ends[0] as usize, ends[3] as usize),
    })
}

/// Writes one frame into the pipe `handle`, on the writer thread ([`ThreadWriter`]): a full
/// pipe blocks that thread, never a tokio worker.
#[allow(
    unsafe_code,
    reason = "WriteFile has no safe wrapper; the buffer outlives the call"
)]
fn write_frame(handle: usize, frame: &[u8]) -> Result<(), EngineError> {
    let mut offset = 0;
    while offset < frame.len() {
        let rest = &frame[offset..];
        let length = u32::try_from(rest.len()).unwrap_or(u32::MAX);
        let mut written = 0u32;
        // SAFETY: `rest` is valid for `length` bytes; synchronous write, no OVERLAPPED.
        let ok = unsafe {
            WriteFile(
                handle as HANDLE,
                rest.as_ptr(),
                length,
                &mut written,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 || written == 0 {
            return Err(EngineError::Closed);
        }
        offset += written as usize;
    }
    Ok(())
}

/// Connects `ends` to a new router: a writer thread takes its messages, a reader thread feeds
/// it every message and closes it when Chromium closes its end.
pub fn connect(ends: PipeEnds) -> Arc<CdpRouter> {
    let write = ends.write;
    let writer = ThreadWriter::spawn("rb-cdp-pipe-write", move |frame| write_frame(write, frame));
    let router = CdpRouter::new(Arc::new(writer));
    let reader = router.clone();
    let spawned = std::thread::Builder::new()
        .name("rb-cdp-pipe".to_owned())
        .spawn(move || read_loop(ends.read, &reader));
    if let Err(error) = spawned {
        tracing::error!(%error, "the CDP reader could not start");
        router.on_closed();
    }
    router
}

#[allow(unsafe_code, reason = "ReadFile and CloseHandle have no safe wrapper")]
fn read_loop(read: usize, router: &CdpRouter) {
    let mut buffer = vec![0u8; 1 << 16];
    let mut decoder = FrameDecoder::default();
    loop {
        let mut count = 0u32;
        let length = u32::try_from(buffer.len()).unwrap_or(u32::MAX);
        // SAFETY: `buffer` is valid for `length` bytes; synchronous read, no OVERLAPPED.
        let ok = unsafe {
            ReadFile(
                read as HANDLE,
                buffer.as_mut_ptr(),
                length,
                &mut count,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 || count == 0 {
            break;
        }
        for message in decoder.push(&buffer[..count as usize]) {
            match serde_json::from_slice(&message) {
                Ok(value) => router.on_message(value),
                Err(error) => tracing::warn!(%error, "a CDP message did not parse"),
            }
        }
    }
    tracing::info!("the CDP pipe closed");
    router.on_closed();
    // SAFETY: our own handle, closed once, after its last use.
    unsafe {
        CloseHandle(read as HANDLE);
    }
}
