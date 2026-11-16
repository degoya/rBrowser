//! gzip sitemaps (`sitemap.xml.gz`, RFC 1952): the frame read here, the deflate stream by
//! `miniz_oxide`, never more than the caller's limit unpacked - a small file that would unpack
//! to gigabytes stops at the limit.

/// The two bytes every gzip member starts with.
const MAGIC: [u8; 2] = [0x1f, 0x8b];
/// Deflate, the only compression method of gzip.
const DEFLATE: u8 = 8;
const FHCRC: u8 = 0x02;
const FEXTRA: u8 = 0x04;
const FNAME: u8 = 0x08;
const FCOMMENT: u8 = 0x10;

/// Why a gzip body could not be unpacked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GzipError {
    /// The frame is broken or uses another method.
    Malformed,
    /// It would unpack to more than the limit.
    TooLarge,
}

/// `bytes` start like a gzip member.
#[must_use]
pub fn is_gzip(bytes: &[u8]) -> bool {
    bytes.starts_with(&MAGIC)
}

/// The offset after a zero-terminated field starting at `at`.
fn skip_zero_terminated(bytes: &[u8], at: usize) -> Option<usize> {
    let rest = bytes.get(at..)?;
    let end = rest.iter().position(|byte| *byte == 0)?;
    Some(at + end + 1)
}

/// The first member of `bytes` unpacked, at most `limit` bytes of it.
///
/// # Errors
///
/// [`GzipError::Malformed`] for a broken frame or stream, [`GzipError::TooLarge`] when the data
/// is larger than `limit`.
pub fn gunzip(bytes: &[u8], limit: usize) -> Result<Vec<u8>, GzipError> {
    let header = bytes.get(..10).ok_or(GzipError::Malformed)?;
    if header[..2] != MAGIC || header[2] != DEFLATE {
        return Err(GzipError::Malformed);
    }
    let flags = header[3];
    let mut at = 10;
    if flags & FEXTRA != 0 {
        let len = bytes.get(at..at + 2).ok_or(GzipError::Malformed)?;
        at += 2 + usize::from(u16::from_le_bytes([len[0], len[1]]));
    }
    if flags & FNAME != 0 {
        at = skip_zero_terminated(bytes, at).ok_or(GzipError::Malformed)?;
    }
    if flags & FCOMMENT != 0 {
        at = skip_zero_terminated(bytes, at).ok_or(GzipError::Malformed)?;
    }
    if flags & FHCRC != 0 {
        at += 2;
    }
    let stream = bytes.get(at..).ok_or(GzipError::Malformed)?;
    miniz_oxide::inflate::decompress_to_vec_with_limit(stream, limit).map_err(|error| {
        if error.status == miniz_oxide::inflate::TINFLStatus::HasMoreOutput {
            GzipError::TooLarge
        } else {
            GzipError::Malformed
        }
    })
}

#[cfg(test)]
#[path = "gzip_tests.rs"]
mod tests;
