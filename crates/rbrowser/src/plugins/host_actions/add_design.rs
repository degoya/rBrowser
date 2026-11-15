//! The file half of the image overlay's `add_design` (RB-080-74): the picked file is checked as
//! the overlay checks a design and copied into the project's `.rbrowser/designs/`.
//!
//! The overlay's component cannot run here, so its rules are kept a second time, small and
//! tested (`plugins/image-overlay/src/image.rs`, `path.rs`): PNG, JPEG or WebP by the extension
//! and by the content's header, at most [`MAX_BYTES`], [`MAX_SIDE`] px a side and
//! [`MAX_PIXELS`]; a name of plain characters ([`design_name`]). A copy never overwrites: the
//! same bytes under the name are the design already, other bytes get the next free name
//! `name (2).png` ([`numbered`]) - the Windows form, before the hints of width and scale so
//! `home-1440@2x.png` keeps them.

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;

/// The folder below the project root the designs live in.
pub const DESIGNS_DIR: &str = ".rbrowser/designs";
/// The largest design file: the plugins' `project.read` limit.
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
/// The longest side of a design in image pixels.
pub const MAX_SIDE: u32 = 16_384;
/// The most pixels of a design.
pub const MAX_PIXELS: u64 = 16 * 1024 * 1024;
/// The longest name before the extension, in characters: with the number, the extension and
/// `designs/` it stays well below the overlay's 255.
pub const MAX_STEM_CHARS: usize = 120;
/// The highest number a copy of a taken name gets.
pub const MAX_COPIES: u32 = 999;

/// The image types a design may be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Png,
    Jpeg,
    Webp,
}

impl Kind {
    /// The type a file name's extension names, in any case.
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
}

/// Why a picked file is not added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No PNG, JPEG or WebP name, or the content is not the image the name says.
    BadType,
    /// Over [`MAX_BYTES`].
    TooLarge,
    /// A side over [`MAX_SIDE`] or more than [`MAX_PIXELS`].
    TooManyPixels,
    /// The picked file could not be read.
    Unreadable,
    /// The copy could not be written (or `.rbrowser/designs/` is a link).
    NotWritten,
}

/// A copy in `.rbrowser/designs/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Copied {
    /// Its file name.
    pub name: String,
    /// The same bytes were there under the name already; nothing was written.
    pub reused: bool,
}

/// A design added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Added {
    /// Its file name in `.rbrowser/designs/`.
    pub name: String,
    /// The picked file's name.
    pub original: String,
    /// The same bytes were in `.rbrowser/designs/` already; nothing was written.
    pub reused: bool,
}

impl Added {
    /// Whether it carries another name than the picked file.
    #[must_use]
    pub fn renamed(&self) -> bool {
        self.name != self.original
    }
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

fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.get(..8)? != b"\x89PNG\r\n\x1a\n" || bytes.get(12..16)? != b"IHDR" {
        return None;
    }
    Some((be32(bytes, 16)?, be32(bytes, 20)?))
}

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
            0xFF => at += 1,
            0x01 | 0xD0..=0xD7 => at += 2,
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                return Some((be16(bytes, at + 7)?, be16(bytes, at + 5)?));
            }
            0xD9 | 0xDA => return None,
            _ => at += 2 + usize::try_from(be16(bytes, at + 2)?).ok()?,
        }
    }
}

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

/// The size of `bytes`, which must be an image of `kind` within the limits.
///
/// # Errors
///
/// [`Refusal::TooLarge`], [`Refusal::BadType`] or [`Refusal::TooManyPixels`].
pub fn check(bytes: &[u8], kind: Kind) -> Result<(u32, u32), Refusal> {
    if bytes.len() > MAX_BYTES {
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
    Ok((width, height))
}

/// Whether the overlay takes `c` in a design's name.
fn plain(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-' | '@' | '+' | '(' | ')')
}

/// The name the copy of `original` gets: its own, each character the overlay does not take
/// (and `~`) as `-`, no leading dot, the part before the extension at most [`MAX_STEM_CHARS`];
/// `design` when nothing is left; never a device name Windows reserves ([`not_reserved`]).
/// `None` for a name without a PNG, JPEG or WebP extension.
#[must_use]
pub fn design_name(original: &str) -> Option<String> {
    Kind::of(original)?;
    let (stem, extension) = original.rsplit_once('.')?;
    let stem: String = stem
        .chars()
        .map(|c| if plain(c) { c } else { '-' })
        .collect();
    let stem: String = stem
        .trim_start_matches(['.', ' '])
        .trim_end()
        .chars()
        .take(MAX_STEM_CHARS)
        .collect();
    let stem = if stem.is_empty() {
        "design".to_owned()
    } else {
        stem
    };
    Some(format!("{}.{extension}", not_reserved(&stem)))
}

/// `stem` with `_` after its first part when Windows reserves that part as a device (`CON`,
/// `NUL`, `AUX`, `PRN`, `COM1`-`COM9`, `LPT1`-`LPT9`, in any case and with any extension).
fn not_reserved(stem: &str) -> String {
    let head = stem.split('.').next().unwrap_or(stem).trim_end();
    let upper = head.to_ascii_uppercase();
    let numbered_device = ["COM", "LPT"].iter().any(|device| {
        upper
            .strip_prefix(device)
            .is_some_and(|digit| matches!(digit.as_bytes(), [b'1'..=b'9']))
    });
    if matches!(upper.as_str(), "CON" | "NUL" | "AUX" | "PRN") || numbered_device {
        format!("{head}_{}", stem[head.len()..].trim_start())
    } else {
        stem.to_owned()
    }
}

/// `name` as copy `number`: ` (n)` before the width (`-1440`) and the scale (`@2x`) the overlay
/// reads from a name.
#[must_use]
pub fn numbered(name: &str, number: u32) -> String {
    let (stem, extension) = name.rsplit_once('.').unwrap_or((name, ""));
    let (rest, scale) = match stem.rsplit_once('@') {
        Some((before, suffix))
            if suffix.strip_suffix('x').is_some_and(|digits| {
                !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
            }) =>
        {
            (before, &stem[before.len()..])
        }
        _ => (stem, ""),
    };
    let (base, width) = match rest.rsplit_once('-') {
        Some((before, digits))
            if (2..=5).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit()) =>
        {
            (before, &rest[before.len()..])
        }
        _ => (rest, ""),
    };
    if extension.is_empty() {
        format!("{base} ({number}){width}{scale}")
    } else {
        format!("{base} ({number}){width}{scale}.{extension}")
    }
}

/// The bytes of the picked file, at most [`MAX_BYTES`].
///
/// # Errors
///
/// [`Refusal::TooLarge`] or [`Refusal::Unreadable`].
pub fn read_picked(path: &Path) -> Result<Vec<u8>, Refusal> {
    let file = File::open(path).map_err(|_| Refusal::Unreadable)?;
    let limit = u64::try_from(MAX_BYTES).unwrap_or(u64::MAX);
    if file.metadata().map_err(|_| Refusal::Unreadable)?.len() > limit {
        return Err(Refusal::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Refusal::Unreadable)?;
    if bytes.len() > MAX_BYTES {
        return Err(Refusal::TooLarge);
    }
    Ok(bytes)
}

/// Whether the file at `path` holds exactly `bytes`.
fn holds(path: &Path, bytes: &[u8]) -> bool {
    let same_length = fs::metadata(path).is_ok_and(|meta| {
        meta.is_file() && u64::try_from(bytes.len()).is_ok_and(|length| length == meta.len())
    });
    same_length && fs::read(path).is_ok_and(|there| there == bytes)
}

/// Writes `bytes` as `name` into `dir` (made when missing), never over a file: the same bytes
/// there already are the copy; other bytes take the next [`numbered`] name.
///
/// # Errors
///
/// [`Refusal::NotWritten`]: the folder or its parent is a link, the folder cannot be made, a
/// write failed, or every number up to [`MAX_COPIES`] is taken.
pub fn copy_into(dir: &Path, name: &str, bytes: &[u8]) -> Result<Copied, Refusal> {
    // Neither the folder nor `.rbrowser/` above it may lead elsewhere (a link, a junction).
    let leads_elsewhere = |path: &Path| fs::symlink_metadata(path).is_ok_and(|meta| !meta.is_dir());
    if leads_elsewhere(dir) || dir.parent().is_some_and(leads_elsewhere) {
        return Err(Refusal::NotWritten);
    }
    fs::create_dir_all(dir).map_err(|_| Refusal::NotWritten)?;
    for number in 1..=MAX_COPIES {
        let candidate = if number == 1 {
            name.to_owned()
        } else {
            numbered(name, number)
        };
        let path = dir.join(&candidate);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if file
                    .write_all(bytes)
                    .and_then(|()| file.sync_all())
                    .is_err()
                {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(Refusal::NotWritten);
                }
                return Ok(Copied {
                    name: candidate,
                    reused: false,
                });
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                if holds(&path, bytes) {
                    return Ok(Copied {
                        name: candidate,
                        reused: true,
                    });
                }
            }
            Err(_) => return Err(Refusal::NotWritten),
        }
    }
    Err(Refusal::NotWritten)
}

/// Checks the file `picked` and copies it into `root`'s `.rbrowser/designs/`.
///
/// # Errors
///
/// The picked file's name and why it was not added.
pub fn add(root: &Path, picked: &Path) -> Result<Added, (String, Refusal)> {
    let original = picked
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let refused = |refusal| (original.clone(), refusal);
    let name = design_name(&original).ok_or_else(|| refused(Refusal::BadType))?;
    let kind = Kind::of(&name).ok_or_else(|| refused(Refusal::BadType))?;
    let bytes = read_picked(picked).map_err(refused)?;
    check(&bytes, kind).map_err(refused)?;
    let copied = copy_into(&root.join(DESIGNS_DIR), &name, &bytes).map_err(refused)?;
    Ok(Added {
        name: copied.name,
        original,
        reused: copied.reused,
    })
}

#[cfg(test)]
#[path = "add_design_tests.rs"]
mod tests;
