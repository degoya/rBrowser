//! An image's rows, one at a time, as premultiplied RGBA (RB-099-24): decoded from the PNG as
//! it is read in ranges, so an image of the capture's size never lies in memory whole. An
//! interlaced PNG (no screenshot rBrowser writes) is decoded whole, within
//! [`codec::WHOLE_PIXELS`].

use std::io::{BufRead, Seek};

use png::{BitDepth, ColorType, Decoder, Transformations};
use rb_plugin_api::{Failure, FailureKind};
use tiny_skia::ColorU8;

use crate::PLUGIN_ID;
use crate::codec;
use crate::stream::{RangeFile, Shared, host_failure};

/// What hands the band renderer its rows: an image's size and its rows from the top.
pub trait RowSource {
    /// The image's width and height.
    fn size(&self) -> (u32, u32);
    /// Whether every pixel is opaque by the image's format (no alpha channel, no transparency).
    fn opaque(&self) -> bool;
    /// The next row, premultiplied RGBA, into `row` (`width * 4` bytes).
    fn next_row(&mut self, row: &mut [u8]) -> Result<(), Failure>;
}

/// `plugin.screenshot-editor.errors.interlaced_too_large` naming the size and the limit.
fn interlaced_too_large(width: u32, height: u32) -> Failure {
    Failure::coded(
        FailureKind::Limit,
        &format!("plugin.{PLUGIN_ID}.errors.interlaced_too_large"),
    )
    .with_param("width", width)
    .with_param("height", height)
    .with_param("max_pixels", codec::WHOLE_PIXELS)
}

/// A decoder's failure: the host's own when it is one, else `not_png`.
fn decoding(error: png::DecodingError) -> Failure {
    match error {
        png::DecodingError::IoError(io) => host_failure(&io).unwrap_or_else(codec::not_png),
        _ => codec::not_png(),
    }
}

enum Mode {
    /// Row by row from the decoder.
    Stream,
    /// An interlaced image, decoded whole; the next row to hand out.
    Whole {
        pixels: Vec<u8>,
        line: usize,
        next: usize,
    },
}

/// The rows of a PNG, from a file below the project read in ranges ([`PngRows::open`]) or a
/// reader the caller keeps ([`PngRows::new`] with `&mut RangeFile`, to fingerprint it after).
pub struct PngRows<R: BufRead + Seek> {
    reader: png::Reader<R>,
    width: u32,
    height: u32,
    color: ColorType,
    opaque: bool,
    mode: Mode,
}

impl<'s, 'h> PngRows<RangeFile<'s, 'h>> {
    /// Opens the PNG `path` and checks its header against the capture's limits before a pixel is
    /// decoded.
    pub fn open(host: &'s Shared<'h>, path: &str) -> Result<Self, Failure> {
        Self::new(RangeFile::open(host, path)?)
    }
}

impl<R: BufRead + Seek> PngRows<R> {
    /// The PNG `file`, its header checked against the capture's limits before a pixel is decoded.
    pub fn new(file: R) -> Result<Self, Failure> {
        let mut decoder = Decoder::new(file);
        decoder.set_transformations(Transformations::normalize_to_color8());
        decoder.set_ignore_text_chunk(true);
        let header = decoder.read_header_info().map_err(decoding)?;
        let (width, height) = (header.width, header.height);
        codec::check_size(width, height)?;
        let interlaced = header.interlaced;
        let alpha = matches!(
            header.color_type,
            ColorType::GrayscaleAlpha | ColorType::Rgba
        );
        let mut reader = decoder.read_info().map_err(decoding)?;
        // tRNS follows PLTE, so only the full info (not the header) knows it.
        let opaque = !alpha && reader.info().trns.is_none();
        let (color, depth) = reader.output_color_type();
        if depth != BitDepth::Eight {
            return Err(codec::not_png());
        }
        let mode = if interlaced {
            if u64::from(width) * u64::from(height) > codec::WHOLE_PIXELS {
                return Err(interlaced_too_large(width, height));
            }
            let size = reader.output_buffer_size().ok_or_else(codec::not_png)?;
            let mut pixels = vec![0; size];
            let info = reader.next_frame(&mut pixels).map_err(decoding)?;
            Mode::Whole {
                pixels,
                line: info.line_size,
                next: 0,
            }
        } else {
            Mode::Stream
        };
        Ok(Self {
            reader,
            width,
            height,
            color,
            opaque,
            mode,
        })
    }
}

impl<R: BufRead + Seek> RowSource for PngRows<R> {
    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn opaque(&self) -> bool {
        self.opaque
    }

    fn next_row(&mut self, row: &mut [u8]) -> Result<(), Failure> {
        let color = self.color;
        match &mut self.mode {
            Mode::Stream => {
                let decoded = self
                    .reader
                    .next_row()
                    .map_err(decoding)?
                    .ok_or_else(codec::not_png)?;
                premultiplied(decoded.data(), color, row)
            }
            Mode::Whole { pixels, line, next } => {
                let start = *next * *line;
                let data = pixels
                    .get(start..start + *line)
                    .ok_or_else(codec::not_png)?;
                *next += 1;
                premultiplied(data, color, row)
            }
        }
    }
}

/// One decoded row of 8-bit `color` samples as premultiplied RGBA into `row`.
pub fn premultiplied(data: &[u8], color: ColorType, row: &mut [u8]) -> Result<(), Failure> {
    let channels = match color {
        ColorType::Grayscale => 1,
        ColorType::GrayscaleAlpha => 2,
        ColorType::Rgb => 3,
        ColorType::Rgba => 4,
        ColorType::Indexed => return Err(codec::not_png()),
    };
    let pixels = row.len() / 4;
    if data.len() < pixels * channels {
        return Err(codec::not_png());
    }
    for (pixel, out) in data.chunks_exact(channels).zip(row.chunks_exact_mut(4)) {
        let [r, g, b, a] = match *pixel {
            [gray] => [gray, gray, gray, u8::MAX],
            [gray, alpha] => [gray, gray, gray, alpha],
            [r, g, b] => [r, g, b, u8::MAX],
            [r, g, b, a] => [r, g, b, a],
            _ => return Err(codec::not_png()),
        };
        let color = ColorU8::from_rgba(r, g, b, a).premultiply();
        out.copy_from_slice(&[color.red(), color.green(), color.blue(), color.alpha()]);
    }
    Ok(())
}

/// The rows of a pixmap in memory: the whole image the tests render and compare.
#[cfg(test)]
pub struct MemoryRows {
    pub pixmap: tiny_skia::Pixmap,
    pub next: u32,
}

#[cfg(test)]
impl RowSource for MemoryRows {
    fn size(&self) -> (u32, u32) {
        (self.pixmap.width(), self.pixmap.height())
    }

    fn opaque(&self) -> bool {
        self.pixmap
            .pixels()
            .iter()
            .all(|pixel| pixel.alpha() == u8::MAX)
    }

    fn next_row(&mut self, row: &mut [u8]) -> Result<(), Failure> {
        let stride = row.len();
        let start = self.next as usize * stride;
        row.copy_from_slice(&self.pixmap.data()[start..start + stride]);
        self.next += 1;
        Ok(())
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
