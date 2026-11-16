//! What a design file is: its type by extension and content, its size, the hints its name gives
//! (`home-1440@2x.png`: 1440 CSS px wide, exported at 2x), its fingerprint and its bytes as
//! base64 for the page script.
//!
//! Designs are images, never code, so they need no workspace trust; but every file is held to
//! the type its extension names (PNG, JPEG, WebP - no SVG, which can carry script), to
//! [`MAX_IMAGE_BYTES`] and to [`MAX_SIDE`] and [`MAX_PIXELS`], before a byte reaches a view.

/// The largest design file: the host's `project.read` limit.
pub const MAX_IMAGE_BYTES: usize = rb_plugin_api::MAX_IO_BYTES;
/// The longest side of a design in image pixels.
pub const MAX_SIDE: u32 = 16_384;
/// The most pixels of a design: 16 Mpx, 64 MiB decoded as RGBA in every view that shows it (and
/// as much again for the canvas), so eight views stay near 1 GiB. A tall full-page design fits
/// at 1x (2880 x 11 000 at 2x does not; Charge 8 review M3 lowered it from 32 Mpx).
pub const MAX_PIXELS: u64 = 16 * 1024 * 1024;
/// The largest export scale a name may give (`@4x`).
pub const MAX_SCALE: u32 = 4;

/// The image types a design may be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Png,
    Jpeg,
    Webp,
}

impl Kind {
    /// The type a file name's extension names, in any case; none for any other extension.
    #[must_use]
    pub fn of(name: &str) -> Option<Self> {
        let (_, extension) = name.rsplit_once('.')?;
        match extension.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "webp" => Some(Self::Webp),
            _ => None,
        }
    }

    /// The MIME type the page script decodes the bytes as.
    #[must_use]
    pub fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
        }
    }
}

/// A design's type and size in image pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageInfo {
    pub kind: Kind,
    pub width: u32,
    pub height: u32,
}

/// Why a file is no usable design.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Not an image of the type its extension names, or no size could be read.
    BadType,
    /// Over [`MAX_IMAGE_BYTES`].
    TooLarge,
    /// A side over [`MAX_SIDE`] or more than [`MAX_PIXELS`].
    TooManyPixels,
}

/// The type and size of `bytes`, which must be an image of `kind`.
///
/// # Errors
///
/// A [`Refusal`] when the bytes are no such image or beyond the limits.
pub fn sniff(bytes: &[u8], kind: Kind) -> Result<ImageInfo, Refusal> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(Refusal::TooLarge);
    }
    let (width, height) = match kind {
        Kind::Png => png_size(bytes),
        Kind::Jpeg => jpeg_size(bytes),
        Kind::Webp => webp_size(bytes),
    }
    .ok_or(Refusal::BadType)?;
    if width == 0 || height == 0 {
        return Err(Refusal::BadType);
    }
    if width > MAX_SIDE || height > MAX_SIDE || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(Refusal::TooManyPixels);
    }
    Ok(ImageInfo {
        kind,
        width,
        height,
    })
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at + 4)?;
    Some(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn be16(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at + 2)?;
    Some(u32::from(u16::from_be_bytes([slice[0], slice[1]])))
}

fn le(bytes: &[u8], at: usize, count: usize) -> Option<u32> {
    let slice = bytes.get(at..at + count)?;
    Some(
        slice
            .iter()
            .rev()
            .fold(0u32, |value, byte| (value << 8) | u32::from(*byte)),
    )
}

/// The signature, then the IHDR chunk first: width and height big-endian.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.get(..8)? != b"\x89PNG\r\n\x1a\n" || bytes.get(12..16)? != b"IHDR" {
        return None;
    }
    Some((be32(bytes, 16)?, be32(bytes, 20)?))
}

/// Start of image, then the markers up to the first frame header (SOF0-SOF15 but DHT, JPG, DAC).
fn jpeg_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.get(..3)? != [0xFF, 0xD8, 0xFF] {
        return None;
    }
    let mut at = 2;
    loop {
        if *bytes.get(at)? != 0xFF {
            return None;
        }
        let marker = *bytes.get(at + 1)?;
        match marker {
            // Fill bytes before a marker.
            0xFF => at += 1,
            // Markers without a length.
            0x01 | 0xD0..=0xD7 => at += 2,
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                return Some((be16(bytes, at + 7)?, be16(bytes, at + 5)?));
            }
            // End of image or start of scan before a frame header.
            0xD9 | 0xDA => return None,
            _ => at += 2 + usize::try_from(be16(bytes, at + 2)?).ok()?,
        }
    }
}

/// RIFF, WEBP, then the first chunk: `VP8 ` (lossy), `VP8L` (lossless) or `VP8X` (extended).
fn webp_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.get(..4)? != b"RIFF" || bytes.get(8..12)? != b"WEBP" {
        return None;
    }
    match bytes.get(12..16)? {
        b"VP8 " => {
            if bytes.get(23..26)? != [0x9D, 0x01, 0x2A] {
                return None;
            }
            Some((le(bytes, 26, 2)? & 0x3FFF, le(bytes, 28, 2)? & 0x3FFF))
        }
        b"VP8L" => {
            if *bytes.get(20)? != 0x2F {
                return None;
            }
            let bits = le(bytes, 21, 4)?;
            Some(((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1))
        }
        b"VP8X" => Some((le(bytes, 24, 3)? + 1, le(bytes, 27, 3)? + 1)),
        _ => None,
    }
}

/// What a file name says about the design: the CSS width after a last `-` (`home-1440`, two to
/// five digits, at most [`MAX_SIDE`]) and the export scale of an `@<n>x` suffix (1 to
/// [`MAX_SCALE`], else 1).
#[must_use]
pub fn name_hints(path: &str) -> (Option<u32>, u32) {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let (stem, scale) = match stem.rsplit_once('@') {
        Some((before, suffix)) => {
            let scale = suffix
                .strip_suffix('x')
                .and_then(|digits| digits.parse::<u32>().ok())
                .filter(|scale| (1..=MAX_SCALE).contains(scale));
            match scale {
                Some(scale) => (before, scale),
                None => (stem, 1),
            }
        }
        None => (stem, 1),
    };
    let width = stem
        .rsplit_once('-')
        .map(|(_, digits)| digits)
        .filter(|digits| {
            (2..=5).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit())
        })
        .and_then(|digits| digits.parse::<u32>().ok())
        .filter(|width| (1..=MAX_SIDE).contains(width));
    (width, scale)
}

/// FNV-1a (64 bit) of `bytes` as 16 hex digits: tells the page script that the file it is
/// fetching in chunks has not changed in between.
#[must_use]
pub fn fingerprint(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!("{hash:016x}")
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `bytes` as standard base64 with padding (what the page's `atob` reads).
#[must_use]
pub fn base64(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for group in bytes.chunks(3) {
        let value = group.iter().enumerate().fold(0u32, |value, (index, byte)| {
            value | (u32::from(*byte) << (16 - 8 * index))
        });
        for index in 0..4 {
            if index <= group.len() {
                let sextet = (value >> (18 - 6 * index)) & 0x3F;
                text.push(char::from(ALPHABET[sextet as usize]));
            } else {
                text.push('=');
            }
        }
    }
    text
}

/// The bytes of standard base64 with padding (what [`base64`] writes and a canvas's
/// `toDataURL` gives); `None` for any other text.
#[must_use]
pub fn unbase64(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if bytes.len() % 4 != 0 {
        return None;
    }
    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    if padding > 2 || (padding > 0 && bytes.len() < 4) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for (index, group) in bytes.chunks(4).enumerate() {
        let last = index + 1 == bytes.len() / 4;
        let pads = if last { padding } else { 0 };
        let mut value = 0u32;
        for (at, byte) in group.iter().enumerate() {
            let sextet = if at >= 4 - pads {
                0
            } else {
                let found = ALPHABET.iter().position(|letter| letter == byte)?;
                u32::try_from(found).ok()?
            };
            value = (value << 6) | sextet;
        }
        let [_, first, second, third] = value.to_be_bytes();
        out.extend_from_slice(&[first, second, third][..3 - pads]);
    }
    Some(out)
}

#[cfg(test)]
#[path = "image_tests.rs"]
pub(crate) mod tests;
