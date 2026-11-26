//! An RGBA8 image in memory and the PNG codec around it (the `png` crate): what the views'
//! captures are decoded into and the overview is drawn on.

use std::io::Cursor;

use crate::VisualError;

/// The longest side an image may have (PNG's own limit is far beyond what a browser renders).
pub const MAX_SIDE: u32 = 65_535;
/// The most pixels an image may have: 256 Mi, 1 GiB of RGBA.
pub const MAX_PIXELS: u64 = 256 * 1024 * 1024;

/// An 8-bit RGBA image, rows top to bottom.
#[derive(Clone, PartialEq, Eq)]
pub struct Rgba {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl std::fmt::Debug for Rgba {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Rgba")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

pub(crate) fn check_size(width: u32, height: u32) -> Result<usize, VisualError> {
    let pixels = u64::from(width) * u64::from(height);
    if width > MAX_SIDE || height > MAX_SIDE || pixels > MAX_PIXELS {
        return Err(VisualError::TooLarge {
            width: width.into(),
            height: height.into(),
        });
    }
    usize::try_from(pixels * 4).map_err(|_| VisualError::TooLarge {
        width: width.into(),
        height: height.into(),
    })
}

impl Rgba {
    /// A `width` x `height` image filled with `color`.
    pub fn new(width: u32, height: u32, color: [u8; 4]) -> Result<Self, VisualError> {
        let bytes = check_size(width, height)?;
        Ok(Self {
            width,
            height,
            pixels: color.iter().copied().cycle().take(bytes).collect(),
        })
    }

    /// A `width` x `height` image of the RGBA bytes `pixels`, which must be exactly that many.
    pub(crate) fn from_raw(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, VisualError> {
        if pixels.len() != check_size(width, height)? {
            return Err(VisualError::Png(format!(
                "{} bytes are no {width} x {height} RGBA image",
                pixels.len()
            )));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// The width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The raw RGBA bytes.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    fn offset(&self, x: u32, y: u32) -> Option<usize> {
        (x < self.width && y < self.height)
            .then(|| (y as usize * self.width as usize + x as usize) * 4)
    }

    /// The pixel at `x`, `y`; `None` outside.
    pub fn get(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        let at = self.offset(x, y)?;
        let mut pixel = [0; 4];
        pixel.copy_from_slice(&self.pixels[at..at + 4]);
        Some(pixel)
    }

    /// Sets the pixel at `x`, `y`; nothing outside.
    pub fn put(&mut self, x: u32, y: u32, color: [u8; 4]) {
        if let Some(at) = self.offset(x, y) {
            self.pixels[at..at + 4].copy_from_slice(&color);
        }
    }

    /// Fills the rectangle, clipped to the image.
    pub fn fill(&mut self, x: u32, y: u32, width: u32, height: u32, color: [u8; 4]) {
        let right = x.saturating_add(width).min(self.width);
        let bottom = y.saturating_add(height).min(self.height);
        for row in y..bottom {
            for column in x..right {
                self.put(column, row, color);
            }
        }
    }

    /// Draws `source` with its top left at `x`, `y`, clipped to the image, blending by the
    /// source's alpha.
    pub fn blit(&mut self, source: &Rgba, x: u32, y: u32) {
        for row in 0..source.height {
            for column in 0..source.width {
                let (Some(over), Some(under)) = (
                    source.get(column, row),
                    self.get(x.saturating_add(column), y.saturating_add(row)),
                ) else {
                    continue;
                };
                self.put(x + column, y + row, blend(over, under));
            }
        }
    }
}

/// `over` on `under` (source-over, straight alpha).
fn blend(over: [u8; 4], under: [u8; 4]) -> [u8; 4] {
    match over[3] {
        255 => over,
        0 => under,
        alpha => {
            let a = u32::from(alpha);
            let mix = |o: u8, u: u8| {
                u8::try_from((u32::from(o) * a + u32::from(u) * (255 - a)) / 255).unwrap_or(255)
            };
            [
                mix(over[0], under[0]),
                mix(over[1], under[1]),
                mix(over[2], under[2]),
                under[3].max(alpha),
            ]
        }
    }
}

/// Decodes a PNG (any colour type, 8 or 16 bits) into RGBA8.
pub fn decode_png(bytes: &[u8]) -> Result<Rgba, VisualError> {
    let failed = |error: png::DecodingError| VisualError::Png(error.to_string());
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(failed)?;
    let (width, height) = {
        let info = reader.info();
        (info.width, info.height)
    };
    check_size(width, height)?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| VisualError::Png("the image is too large to decode".to_owned()))?;
    let mut buffer = vec![0; size];
    let frame = reader.next_frame(&mut buffer).map_err(failed)?;
    buffer.truncate(frame.buffer_size());
    let pixels = match frame.color_type {
        png::ColorType::Rgba => buffer,
        png::ColorType::Rgb => buffer
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|[r, g, b]| [*r, *g, *b, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buffer
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|[grey, alpha]| [*grey, *grey, *grey, *alpha])
            .collect(),
        png::ColorType::Grayscale => buffer.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::Indexed => {
            return Err(VisualError::Png(
                "an indexed PNG was not expanded".to_owned(),
            ));
        }
    };
    Ok(Rgba {
        width,
        height,
        pixels,
    })
}

/// The width and height in a PNG's header, without decoding it; `None` for anything but a PNG.
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let read =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    Some((read(16), read(20)))
}

/// Encodes `image` as an RGBA8 PNG.
pub fn encode_png(image: &Rgba) -> Result<Vec<u8>, VisualError> {
    let failed = |error: png::EncodingError| VisualError::Png(error.to_string());
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, image.width, image.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(failed)?;
        writer.write_image_data(&image.pixels).map_err(failed)?;
        writer.finish().map_err(failed)?;
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "raster_tests.rs"]
mod tests;
