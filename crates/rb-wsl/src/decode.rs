//! What `wsl.exe` prints, as text.
//!
//! `wsl.exe` writes UTF-16LE even into a pipe - with a byte order mark on some versions, padded
//! with NUL characters on others, with CRLF or LF line ends. Versions since 0.64 write UTF-8
//! when `WSL_UTF8=1` is set, which [`crate::query`] does; both forms are read here. Adapted from
//! rCommander's `rc-vfs::wsl::decode`.

const UTF16LE_BOM: [u8; 2] = [0xFF, 0xFE];
const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// The text of `bytes`: UTF-16LE with or without its byte order mark, otherwise UTF-8; invalid
/// sequences replaced, NUL characters removed.
#[must_use]
pub fn decode(bytes: &[u8]) -> String {
    let text = if let Some(rest) = bytes.strip_prefix(&UTF16LE_BOM) {
        utf16le(rest)
    } else if let Some(rest) = bytes.strip_prefix(&UTF8_BOM) {
        String::from_utf8_lossy(rest).into_owned()
    } else if looks_utf16le(bytes) {
        utf16le(bytes)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    text.replace('\0', "")
}

/// ASCII in UTF-16LE has a zero in every second byte; UTF-8 text has none at all. Half of the
/// odd bytes zero leaves room for a few characters past ASCII in a message.
fn looks_utf16le(bytes: &[u8]) -> bool {
    let odd = bytes.iter().skip(1).step_by(2);
    let total = odd.clone().count();
    let zeros = odd.filter(|byte| **byte == 0).count();
    total > 0 && zeros * 2 >= total
}

/// UTF-16LE code units to text; a trailing odd byte is dropped.
fn utf16le(bytes: &[u8]) -> String {
    let units = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair));
    char::decode_utf16(units)
        .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

#[cfg(test)]
#[path = "decode_tests.rs"]
mod tests;
