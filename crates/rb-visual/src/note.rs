//! A note of rBrowser's own in a PNG (RB-099-28): one `tEXt` chunk put in right after the
//! header, without decoding the image or encoding it again, so a capture's PNG - Chromium's own
//! or the stitcher's - carries what the capture learned to every reader of the image (the
//! screenshot answer, a saved file), as the cut of a long page does ([`crate::stitch`]).
//!
//! [`IMAGES_PENDING_KEY`] marks a full page whose lazy-loading pre-pass ran out of its deadline:
//! images may be missing from it.

use std::io::Cursor;

use png::text_metadata::{EncodableTextChunk, TEXtChunk};

/// The keyword of a full page captured with images still loading; the text is the deadline in
/// ms.
pub const IMAGES_PENDING_KEY: &str = "rbrowser-images-pending";

/// The PNG signature.
const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
/// The end of the header chunk: the signature, then `IHDR`'s length, type, 13 bytes and CRC.
const HEADER_END: usize = 8 + 4 + 4 + 13 + 4;

/// `png` with the note `key` = `text` after its header; `None` when `png` does not start with a
/// PNG header or the note cannot be written (a key of more than 79 bytes, text outside
/// Latin-1).
pub fn with_note(png: &[u8], key: &str, text: &str) -> Option<Vec<u8>> {
    let header = png.get(..HEADER_END)?;
    let is_header = header.starts_with(SIGNATURE)
        && header.get(8..12) == Some(&[0, 0, 0, 13][..])
        && header.get(12..16) == Some(&b"IHDR"[..]);
    if !is_header {
        return None;
    }
    let mut chunk = Vec::new();
    TEXtChunk::new(key, text).encode(&mut chunk).ok()?;
    let mut noted = Vec::with_capacity(png.len() + chunk.len());
    noted.extend_from_slice(header);
    noted.extend_from_slice(&chunk);
    noted.extend_from_slice(png.get(HEADER_END..)?);
    Some(noted)
}

/// The text of the note `key` in `png`; `None` without it or for anything but a PNG.
pub fn note(png: &[u8], key: &str) -> Option<String> {
    let reader = png::Decoder::new(Cursor::new(png)).read_info().ok()?;
    reader
        .info()
        .uncompressed_latin1_text
        .iter()
        .find(|chunk| chunk.keyword == key)
        .map(|chunk| chunk.text.clone())
}

/// Whether `png` is a full page captured with images still loading.
pub fn images_pending(png: &[u8]) -> bool {
    note(png, IMAGES_PENDING_KEY).is_some()
}

#[cfg(test)]
#[path = "note_tests.rs"]
mod tests;
