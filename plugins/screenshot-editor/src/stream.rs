//! Files larger than one host call (RB-099-24): the image read in ranges (`project-range`) and the
//! edited file written in parts (`out-stream`), each at most `rb_plugin_api::MAX_IO_BYTES` a call.
//!
//! The decoder reads while the encoder writes, and both reach the host: they share it through a
//! [`Shared`] cell and borrow it for one call at a time, never across one.

use std::cell::RefCell;
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};

use rb_plugin_api::{Failure, MAX_IO_BYTES, PluginHost};

use crate::cache::{FNV_START, fnv};

/// The host, shared by the reader and the writer of one render.
pub type Shared<'a> = RefCell<&'a mut dyn PluginHost>;

/// The bytes one range read asks for.
pub const READ_BYTES: usize = 4 * 1024 * 1024;
/// The bytes the writer gathers before it appends them.
pub const PART_BYTES: usize = 4 * 1024 * 1024;

/// A host failure as an I/O error, so a decoder or an encoder hands it back unchanged.
fn io_error(failure: Failure) -> io::Error {
    io::Error::other(failure)
}

/// The host failure inside an I/O error of a decoder or an encoder, if it is one.
#[must_use]
pub fn host_failure(error: &io::Error) -> Option<Failure> {
    error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<Failure>())
        .cloned()
}

/// The project file `path`, read in ranges as a decoder reads it.
pub struct RangeFile<'s, 'h> {
    host: &'s Shared<'h>,
    path: String,
    size: u64,
    buffer: Vec<u8>,
    /// The file offset of `buffer[0]`.
    start: u64,
    /// The position within `buffer`.
    at: usize,
    /// The fingerprint of the bytes read from the start up to `hashed_to`, while they were read
    /// in order (review of RB-099-24: what was decoded is what was fingerprinted).
    hashed: Option<u64>,
    hashed_to: u64,
}

impl<'s, 'h> RangeFile<'s, 'h> {
    /// Opens `path` with its first range read.
    pub fn open(host: &'s Shared<'h>, path: &str) -> Result<Self, Failure> {
        let (buffer, size) = read_range(host, path, 0)?;
        Ok(Self {
            host,
            path: path.to_owned(),
            size,
            hashed: Some(fnv(FNV_START, &buffer)),
            hashed_to: buffer.len() as u64,
            buffer,
            start: 0,
            at: 0,
        })
    }

    /// The fingerprint of the file as this reader read it: what it has read, in order, and the
    /// rest read now; `None` when it read out of order or the file changed its length.
    pub fn finish_hash(&mut self) -> Result<Option<String>, Failure> {
        let Some(mut hash) = self.hashed else {
            return Ok(None);
        };
        let mut at = self.hashed_to;
        while at < self.size {
            let (bytes, size) = read_range(self.host, &self.path, at)?;
            if bytes.is_empty() || size != self.size {
                return Ok(None);
            }
            hash = fnv(hash, &bytes);
            at += bytes.len() as u64;
        }
        Ok(Some(format!("{hash:016x}")))
    }

    /// The file's length in bytes.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }
}

fn read_range(host: &Shared<'_>, path: &str, offset: u64) -> Result<(Vec<u8>, u64), Failure> {
    let length = u32::try_from(READ_BYTES.min(MAX_IO_BYTES)).unwrap_or(u32::MAX);
    host.borrow_mut().project_read_range(path, offset, length)
}

impl Read for RangeFile<'_, '_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let available = self.fill_buf()?;
        let count = available.len().min(out.len());
        out[..count].copy_from_slice(&available[..count]);
        self.consume(count);
        Ok(count)
    }
}

impl BufRead for RangeFile<'_, '_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.at >= self.buffer.len() {
            let position = self.start + self.at as u64;
            if position >= self.size {
                return Ok(&[]);
            }
            let (bytes, size) = read_range(self.host, &self.path, position).map_err(io_error)?;
            // A file that changed its length under the reader is no longer the one opened.
            if size != self.size {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "the file changed while it was read",
                ));
            }
            self.hashed = match self.hashed {
                Some(hash) if position == self.hashed_to => {
                    self.hashed_to += bytes.len() as u64;
                    Some(fnv(hash, &bytes))
                }
                _ => None,
            };
            self.buffer = bytes;
            self.start = position;
            self.at = 0;
        }
        Ok(&self.buffer[self.at..])
    }

    fn consume(&mut self, amount: usize) {
        self.at = (self.at + amount).min(self.buffer.len());
    }
}

impl Seek for RangeFile<'_, '_> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let position = self.start + self.at as u64;
        let target = match to {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::End(delta) => self.size.checked_add_signed(delta),
            SeekFrom::Current(delta) => position.checked_add_signed(delta),
        }
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before the start"))?;
        let end = self.start + self.buffer.len() as u64;
        if (self.start..=end).contains(&target) {
            self.at = usize::try_from(target - self.start).unwrap_or(self.buffer.len());
        } else {
            self.buffer.clear();
            self.start = target;
            self.at = 0;
        }
        Ok(target)
    }
}

/// The edited file `path` below the plugin's out folder, written in parts and committed at
/// [`OutFile::commit`]; dropped before, nothing of it shows (the host removes it when the call
/// ends).
pub struct OutFile<'s, 'h> {
    host: &'s Shared<'h>,
    buffer: Vec<u8>,
    written: usize,
}

impl<'s, 'h> OutFile<'s, 'h> {
    /// Begins `path`.
    pub fn begin(host: &'s Shared<'h>, path: &str) -> Result<Self, Failure> {
        host.borrow_mut().out_begin(path)?;
        Ok(Self {
            host,
            buffer: Vec::with_capacity(PART_BYTES),
            written: 0,
        })
    }

    fn append_buffer(&mut self) -> Result<(), Failure> {
        for part in self.buffer.chunks(MAX_IO_BYTES) {
            self.host.borrow_mut().out_append(part)?;
        }
        self.written += self.buffer.len();
        self.buffer.clear();
        Ok(())
    }

    /// Appends what is left and commits the file; its length in bytes.
    pub fn commit(mut self) -> Result<usize, Failure> {
        self.append_buffer()?;
        self.host.borrow_mut().out_commit()?;
        Ok(self.written)
    }
}

impl Write for OutFile<'_, '_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(bytes);
        if self.buffer.len() >= PART_BYTES {
            self.append_buffer().map_err(io_error)?;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // The parts go out at their size and at the commit; nothing is half-visible meanwhile.
        Ok(())
    }
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
