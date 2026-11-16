//! The video route of the Videos panel (RB-110-12): `rbrowser://video/<recording>/<file>`, the
//! bytes of one recorded file of the open project, with HTTP range requests so the player can
//! seek. A video cannot pass the panels' 64 KiB message bridge, and a page of the shell plays it
//! from here instead.
//!
//! - **What:** only a file rBrowser recorded itself and nobody changed since
//!   ([`super::registry`]), a regular file below the open project's `.rbrowser/out/video/` with
//!   the extension `mp4` or `webm`, or a video's poster `<breakpoint>.jpg` ([`super::poster`],
//!   served as `image/jpeg`), reached and opened without a link
//!   ([`super::library::open_video`]); the URL
//!   names a recording and a file - never a root, an absolute path or `..` ([`parse_url`]). No
//!   listing: a folder is no answer.
//! - **Who:** only a page of the shell in its main frame ([`requester_allowed`]): a page in a
//!   view, a frame in it or a plugin panel gets nothing - CEF hands the scheme handler the frame
//!   that asks (`cef_app/video_scheme.rs`), and a view never loads a shell page
//!   (`crate::shell::navigation_refused`). The pages window's CSP lets `media-src`, `img-src`
//!   (the posters) and `connect-src` (the panel's whole-file fallback) reach `rbrowser://video`,
//!   and `media-src` the panel's own `blob:` copies; nothing else new.
//! - **How:** one range per request (RFC 9110; several ranges or a header that does not read
//!   get the whole file), `206` with `Content-Range`, `416` for a range the file does not have. CEF's loader skips to the first
//!   byte itself; [`Served`] is at the range's start already and absorbs that skip, so the answer
//!   is right whether CEF skips or not. The file is opened with Rust's sharing, which lets a
//!   recording be deleted on Windows while a player still has it open. Every answer offers
//!   ranges, and lets the shell's page read it (`Access-Control-Allow-Origin: rbrowser://ui`):
//!   the panel's fallback fetches a video whole when the player cannot stream it
//!   (`ui/src/videos/blobs.ts`). The tests in `serve_loader_tests.rs` run the route through a
//!   model of CEF's loader and Chromium's media checks.

use std::fs::File;
use std::io::{self, Read as _, Seek as _, SeekFrom};
use std::path::Path;

use super::library::{self, LibraryError, is_recording_id, is_served_name};
use super::registry::Recorded;
use crate::shell::{is_shell_url, percent_decode};

/// The route's host under the app's scheme.
pub const ROUTE_HOST: &str = "video";
/// Every video URL starts with this.
pub const ROUTE_ORIGIN: &str = "rbrowser://video/";
/// The origin of the shell's pages (`crate::shell::SHELL_ORIGIN` without its slash), which may
/// read an answer with `fetch`.
pub const SHELL_PAGE_ORIGIN: &str = "rbrowser://ui";
/// The most bytes handed to the loader at once.
pub const CHUNK: usize = 256 * 1024;
/// The most characters of a request's `Range` header the log keeps.
pub const LOGGED_RANGE_CHARS: usize = 64;

/// A request's `Range` header for the log: `-` without one, control characters as `?`, cut to
/// [`LOGGED_RANGE_CHARS`]. A range is numbers, never a path.
#[must_use]
pub fn logged_range(header: Option<&str>) -> String {
    match header {
        None => "-".to_owned(),
        Some(header) => header
            .chars()
            .take(LOGGED_RANGE_CHARS)
            .map(|c| if c.is_control() { '?' } else { c })
            .collect(),
    }
}

/// The URL of the video file `name` of recording `id`.
#[must_use]
pub fn url_of(id: &str, name: &str) -> String {
    format!("{ROUTE_ORIGIN}{id}/{name}")
}

/// The recording and the file `url` names - a video or a video's poster - decoded and checked;
/// `None` for anything else.
#[must_use]
pub fn parse_url(url: &str) -> Option<(String, String)> {
    let path = url.split(['?', '#']).next()?;
    let prefix = path.get(..ROUTE_ORIGIN.len())?;
    if !prefix.eq_ignore_ascii_case(ROUTE_ORIGIN) {
        return None;
    }
    let (id, name) = path[ROUTE_ORIGIN.len()..].split_once('/')?;
    if name.contains('/') {
        return None;
    }
    let id = percent_decode(id)?;
    let name = percent_decode(name)?;
    (is_recording_id(&id) && is_served_name(&name)).then_some((id, name))
}

/// Whether a request of the frame at `frame_url` (`main`: the page's main frame) may have a
/// video: a page of the shell only.
#[must_use]
pub fn requester_allowed(frame_url: Option<&str>, main: bool) -> bool {
    main && frame_url.is_some_and(is_shell_url)
}

/// What a `Range` header asks of a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteRange {
    /// No range, another unit, or several ranges: the whole file.
    Full,
    /// Bytes `start` to `end`, both included, within the file.
    Part { start: u64, end: u64 },
    /// A range the file does not have, or one that does not read.
    Unsatisfiable,
}

fn digits(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// The range `header` asks of a file of `total` bytes (RFC 9110, 14.2): a header that does not
/// read as one byte range - another unit, several ranges, no `=`, no digits, a start after the
/// end, a number too large - is ignored and the whole file answered; a range that reads but
/// that the file does not have (a start at or past its end, an empty suffix, any range of an
/// empty file) is unsatisfiable.
#[must_use]
pub fn parse_range(header: Option<&str>, total: u64) -> ByteRange {
    let Some(header) = header.map(str::trim).filter(|header| !header.is_empty()) else {
        return ByteRange::Full;
    };
    let Some((unit, spec)) = header.split_once('=') else {
        return ByteRange::Full;
    };
    if !unit.trim().eq_ignore_ascii_case("bytes") {
        return ByteRange::Full;
    }
    let spec = spec.trim();
    if spec.contains(',') {
        return ByteRange::Full;
    }
    let Some((first, last)) = spec.split_once('-') else {
        return ByteRange::Full;
    };
    let (first, last) = (first.trim(), last.trim());
    let wanted = match (first.is_empty(), last.is_empty()) {
        // `-n`: the last n bytes.
        (true, false) => digits(last).map(|suffix| (None, Some(suffix))),
        // `n-`: from n to the end.
        (false, true) => digits(first).map(|start| (Some(start), None)),
        (false, false) => match (digits(first), digits(last)) {
            (Some(start), Some(end)) if start <= end => Some((Some(start), Some(end))),
            _ => None,
        },
        (true, true) => None,
    };
    let Some(wanted) = wanted else {
        return ByteRange::Full;
    };
    if total == 0 {
        return ByteRange::Unsatisfiable;
    }
    let part = |start: u64, end: u64| ByteRange::Part { start, end };
    match wanted {
        (None, Some(0)) => ByteRange::Unsatisfiable,
        (None, Some(suffix)) => part(total.saturating_sub(suffix), total - 1),
        (Some(start), _) if start >= total => ByteRange::Unsatisfiable,
        (Some(start), None) => part(start, total - 1),
        (Some(start), Some(end)) => part(start, end.min(total - 1)),
        (None, None) => ByteRange::Full,
    }
}

/// CEF's `ERR_FAILED`.
pub const ERR_FAILED: i32 = -2;

/// What the resource handler's `Skip` answers CEF (its return value and `bytes_skipped`) when
/// `requested` bytes were asked for and `skipped` were skipped (`None`: the file failed). A skip
/// of 0 asked and 0 done is done at once - Chromium's player starts with `Range: bytes=0-` - and
/// 0 skipped of more asked would read as "later" to CEF, so it fails instead.
#[must_use]
pub fn skip_answer(requested: i64, skipped: Option<u64>) -> (i32, i64) {
    let failed = (0, i64::from(ERR_FAILED));
    match skipped {
        _ if requested < 0 => failed,
        Some(0) if requested == 0 => (1, 0),
        Some(count) if count > 0 => (1, i64::try_from(count).unwrap_or(i64::MAX)),
        _ => failed,
    }
}

/// One video answer: the file, where it is read and where the answer ends.
#[derive(Debug)]
pub struct Served {
    file: File,
    total: u64,
    mime: &'static str,
    range: ByteRange,
    /// The file's read position.
    position: u64,
    /// Where the answer ends, exclusive.
    end: u64,
    /// The bytes of the range's start a skip of the loader may still take without moving.
    absorbable: u64,
}

impl Served {
    /// Opens the video `url` names in the project at `root` - one rBrowser wrote (`recorded`)
    /// - for an answer to `range` (the request's `Range` header).
    ///
    /// # Errors
    ///
    /// A URL that names no recording's video, a link on the way, nothing there, or the file
    /// cannot be opened.
    pub fn open(
        root: &Path,
        recorded: &Recorded,
        url: &str,
        range: Option<&str>,
    ) -> Result<Self, LibraryError> {
        let (id, name) = parse_url(url).ok_or_else(|| LibraryError::Refused(url.to_owned()))?;
        let mime = library::mime(&name).ok_or_else(|| LibraryError::Refused(name.clone()))?;
        let (mut file, metadata) = library::open_video(root, recorded, &id, &name)?;
        let io = |error: io::Error| LibraryError::Io(error.to_string());
        let total = metadata.len();
        let range = parse_range(range, total);
        let (position, end) = match range {
            ByteRange::Full => (0, total),
            ByteRange::Part { start, end } => (start, end + 1),
            ByteRange::Unsatisfiable => (0, 0),
        };
        if position > 0 {
            file.seek(SeekFrom::Start(position)).map_err(io)?;
        }
        Ok(Self {
            file,
            total,
            mime,
            range,
            position,
            end,
            absorbable: position,
        })
    }

    /// The status code and its text.
    #[must_use]
    pub fn status(&self) -> (i32, &'static str) {
        match self.range {
            ByteRange::Full => (200, "OK"),
            ByteRange::Part { .. } => (206, "Partial Content"),
            ByteRange::Unsatisfiable => (416, "Range Not Satisfiable"),
        }
    }

    /// The MIME type.
    #[must_use]
    pub fn mime(&self) -> &'static str {
        self.mime
    }

    /// The answer's length in bytes.
    #[must_use]
    pub fn length(&self) -> u64 {
        self.end.saturating_sub(match self.range {
            ByteRange::Part { start, .. } => start,
            _ => 0,
        })
    }

    /// The answer's headers besides its type and length.
    #[must_use]
    pub fn headers(&self) -> Vec<(&'static str, String)> {
        let mut headers = vec![
            ("Accept-Ranges", "bytes".to_owned()),
            ("Cache-Control", "no-store".to_owned()),
            ("X-Content-Type-Options", "nosniff".to_owned()),
            ("Access-Control-Allow-Origin", SHELL_PAGE_ORIGIN.to_owned()),
        ];
        if let Some(range) = self.content_range() {
            headers.push(("Content-Range", range));
        }
        headers
    }

    /// The answer's `Content-Range`: none for the whole file.
    #[must_use]
    pub fn content_range(&self) -> Option<String> {
        match self.range {
            ByteRange::Full => None,
            ByteRange::Part { start, end } => Some(format!("bytes {start}-{end}/{}", self.total)),
            ByteRange::Unsatisfiable => Some(format!("bytes */{}", self.total)),
        }
    }

    /// Skips `count` bytes for the loader: the range's start first (where the answer is
    /// already), then forward, never past the answer's end. Answers the bytes skipped.
    ///
    /// # Errors
    ///
    /// The file cannot seek.
    pub fn skip(&mut self, count: u64) -> io::Result<u64> {
        let absorbed = count.min(self.absorbable);
        self.absorbable -= absorbed;
        let moved = (count - absorbed).min(self.end.saturating_sub(self.position));
        if moved > 0 {
            self.position += moved;
            self.file.seek(SeekFrom::Start(self.position))?;
        }
        Ok(absorbed + moved)
    }

    /// Reads the next bytes of the answer into `buffer`; 0 at its end.
    ///
    /// # Errors
    ///
    /// The file cannot be read.
    pub fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let left = self.end.saturating_sub(self.position);
        let wanted = usize::try_from(left).map_or(buffer.len(), |left| left.min(buffer.len()));
        if wanted == 0 {
            return Ok(0);
        }
        let read = self.file.read(&mut buffer[..wanted])?;
        self.position += read as u64;
        // A later skip moves from here: the range's start is behind.
        self.absorbable = 0;
        Ok(read)
    }

    /// The next bytes for the loader's read of `bytes_to_read`: at most that many and at most
    /// [`CHUNK`]; none at the answer's end (or for a read of nothing).
    ///
    /// # Errors
    ///
    /// The file cannot be read.
    pub fn read_for_loader(&mut self, bytes_to_read: i32) -> io::Result<Vec<u8>> {
        let wanted = usize::try_from(bytes_to_read).unwrap_or(0).min(CHUNK);
        let mut buffer = vec![0_u8; wanted];
        let read = if wanted == 0 {
            0
        } else {
            self.read(&mut buffer)?
        };
        buffer.truncate(read);
        Ok(buffer)
    }
}

#[cfg(test)]
#[path = "serve_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "serve_loader_tests.rs"]
mod loader_tests;
