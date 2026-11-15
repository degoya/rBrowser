//! What an icon really is: its format sniffed from its first bytes and its size read from its
//! header - PNG, JPEG, GIF, WebP, ICO and SVG (scalable, no size) - so a manifest's `sizes` and
//! `type` can be held to the file. Headers only: nothing is decoded.

/// An image format an icon may have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
    Gif,
    Webp,
    Ico,
    Svg,
}

impl Format {
    /// The MIME type the format is served as.
    #[must_use]
    pub fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
            Self::Ico => "image/x-icon",
            Self::Svg => "image/svg+xml",
        }
    }

    /// Whether the declared `type` names this format (its usual aliases too).
    #[must_use]
    pub fn matches(self, declared: &str) -> bool {
        let declared = declared.trim().to_ascii_lowercase();
        let declared = declared.split(';').next().unwrap_or_default().trim();
        match self {
            Self::Jpeg => matches!(declared, "image/jpeg" | "image/jpg" | "image/pjpeg"),
            Self::Ico => matches!(declared, "image/x-icon" | "image/vnd.microsoft.icon"),
            other => declared == other.mime(),
        }
    }
}

/// A sniffed image: its format and, but for SVG, its size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Image {
    pub format: Format,
    pub size: Option<(u32, u32)>,
}

/// The `sizes` of a manifest icon: `any`, and the `<w>x<h>` pairs listed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sizes {
    pub any: bool,
    pub listed: Vec<(u32, u32)>,
}

/// `sizes` as the Web App Manifest reads it: space-separated `<w>x<h>` (either `x` case) or
/// `any`; anything else is skipped.
#[must_use]
pub fn parse_sizes(text: &str) -> Sizes {
    let mut sizes = Sizes::default();
    for token in text.split_ascii_whitespace() {
        if token.eq_ignore_ascii_case("any") {
            sizes.any = true;
            continue;
        }
        let lower = token.to_ascii_lowercase();
        let Some((width, height)) = lower.split_once('x') else {
            continue;
        };
        if let (Ok(width), Ok(height)) = (width.parse::<u32>(), height.parse::<u32>())
            && width > 0
            && height > 0
        {
            sizes.listed.push((width, height));
        }
    }
    sizes
}

impl Image {
    #[must_use]
    pub fn sized(format: Format, width: u32, height: u32) -> Self {
        Self {
            format,
            size: Some((width, height)),
        }
    }

    /// Whether the image is what `sizes` claims: a scalable image fits any size, a sized one
    /// must be one of the listed sizes (`any` alone claims scalability, which a bitmap lacks).
    #[must_use]
    pub fn fits(&self, sizes: &Sizes) -> bool {
        match self.size {
            None => true,
            Some(size) => sizes.listed.contains(&size),
        }
    }
}

fn be16(bytes: &[u8], at: usize) -> Option<u32> {
    let pair = bytes.get(at..at + 2)?;
    Some(u32::from(u16::from_be_bytes([pair[0], pair[1]])))
}

fn le16(bytes: &[u8], at: usize) -> Option<u32> {
    let pair = bytes.get(at..at + 2)?;
    Some(u32::from(u16::from_le_bytes([pair[0], pair[1]])))
}

fn le24(bytes: &[u8], at: usize) -> Option<u32> {
    let three = bytes.get(at..at + 3)?;
    Some(u32::from_le_bytes([three[0], three[1], three[2], 0]))
}

fn png(bytes: &[u8]) -> Option<Image> {
    let header = bytes.get(16..24)?;
    let width = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    let height = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
    Some(Image::sized(Format::Png, width, height))
}

/// The first start-of-frame segment's size; the markers before it are skipped by their length.
fn jpeg(bytes: &[u8]) -> Option<Image> {
    let mut at = 2;
    loop {
        if *bytes.get(at)? != 0xFF {
            return None;
        }
        let marker = *bytes.get(at + 1)?;
        if marker == 0xFF {
            at += 1;
            continue;
        }
        let length = usize::try_from(be16(bytes, at + 2)?).ok()?;
        let frame = matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
        if frame {
            let height = be16(bytes, at + 5)?;
            let width = be16(bytes, at + 7)?;
            return Some(Image::sized(Format::Jpeg, width, height));
        }
        if length < 2 {
            return None;
        }
        at += 2 + length;
    }
}

fn webp(bytes: &[u8]) -> Option<Image> {
    let size = match bytes.get(12..16)? {
        b"VP8X" => (1 + le24(bytes, 24)?, 1 + le24(bytes, 27)?),
        b"VP8 " => (le16(bytes, 26)? & 0x3FFF, le16(bytes, 28)? & 0x3FFF),
        b"VP8L" => {
            let packed = bytes.get(21..25)?;
            let bits = u32::from_le_bytes([packed[0], packed[1], packed[2], packed[3]]);
            (1 + (bits & 0x3FFF), 1 + ((bits >> 14) & 0x3FFF))
        }
        _ => return None,
    };
    Some(Image::sized(Format::Webp, size.0, size.1))
}

/// The largest entry of an icon directory (a byte of 0 is 256).
fn ico(bytes: &[u8]) -> Option<Image> {
    let count = usize::try_from(le16(bytes, 4)?).ok()?;
    let side = |byte: u8| if byte == 0 { 256 } else { u32::from(byte) };
    (0..count.min(64))
        .filter_map(|index| {
            let entry = bytes.get(6 + index * 16..8 + index * 16)?;
            Some((side(entry[0]), side(entry[1])))
        })
        .max_by_key(|(width, height)| width * height)
        .map(|(width, height)| Image::sized(Format::Ico, width, height))
}

fn svg(bytes: &[u8]) -> Option<Image> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
    let head = head.trim_start_matches('\u{feff}').trim_start();
    let svg = head.starts_with("<svg") || (head.starts_with("<?xml") && head.contains("<svg"));
    svg.then_some(Image {
        format: Format::Svg,
        size: None,
    })
}

/// The format and size of `bytes`; `None` for anything that is no image of the six formats.
#[must_use]
pub fn sniff(bytes: &[u8]) -> Option<Image> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        png(bytes)
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        jpeg(bytes)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(Image::sized(Format::Gif, le16(bytes, 6)?, le16(bytes, 8)?))
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP".as_slice()) {
        webp(bytes)
    } else if bytes.starts_with(&[0, 0, 1, 0]) {
        ico(bytes)
    } else {
        svg(bytes)
    }
}

#[cfg(test)]
#[path = "image_tests.rs"]
pub(crate) mod tests;
