//! The edited image encoded band by band (RB-099-24): a PNG through the `png` crate's stream
//! writer, a JPEG through `image`'s encoder, which takes its 8 x 8 blocks one after the other
//! from an image view that renders the next band when a block reaches it. Pixels only, as
//! `crate::codec`: no text chunk, no time, no colour profile.
//!
//! The PNG is RGB when the source is opaque by its format, RGBA otherwise (drawing never makes a
//! pixel less opaque); the JPEG lays transparency over white, as before.

use std::cell::RefCell;
use std::io::Write;

use image::codecs::jpeg::JpegEncoder;
use image::{GenericImageView, ImageError, Rgb};
use rb_plugin_api::Failure;
use tiny_skia::PremultipliedColorU8;

use crate::band::Bands;
use crate::codec::encode_failed;
use crate::stream::host_failure;

fn png_failure(error: png::EncodingError) -> Failure {
    match error {
        png::EncodingError::IoError(io) => host_failure(&io).unwrap_or_else(encode_failed),
        _ => encode_failed(),
    }
}

/// Premultiplied RGBA `rows` as straight RGBA, or RGB when `opaque`, appended to `out`.
pub fn straight(rows: &[u8], opaque: bool, out: &mut Vec<u8>) {
    for pixel in rows.chunks_exact(4) {
        let color = PremultipliedColorU8::from_rgba(pixel[0], pixel[1], pixel[2], pixel[3])
            .map_or(tiny_skia::ColorU8::from_rgba(0, 0, 0, 0), |color| {
                color.demultiply()
            });
        out.extend_from_slice(&[color.red(), color.green(), color.blue()]);
        if !opaque {
            out.push(color.alpha());
        }
    }
}

/// Premultiplied RGBA `rows` as RGB over white (straight alpha over an opaque background, the
/// arithmetic of `codec::encode_jpeg`), appended to `out`.
pub fn over_white(rows: &[u8], out: &mut Vec<u8>) {
    let mut rgba = Vec::with_capacity(rows.len());
    straight(rows, false, &mut rgba);
    for pixel in rgba.chunks_exact(4) {
        let alpha = u32::from(pixel[3]);
        for channel in &pixel[..3] {
            let mixed = (u32::from(*channel) * alpha + 255 * (255 - alpha) + 127) / 255;
            out.push(u8::try_from(mixed).unwrap_or(u8::MAX));
        }
    }
}

/// Every band of `bands` as one PNG into `out`.
pub fn png(bands: &mut Bands<'_, '_>, out: impl Write) -> Result<(), Failure> {
    let (width, height) = bands.size();
    let opaque = bands.opaque();
    let mut encoder = png::Encoder::new(out, width, height);
    encoder.set_color(if opaque {
        png::ColorType::Rgb
    } else {
        png::ColorType::Rgba
    });
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header().map_err(png_failure)?;
    let mut stream = writer.stream_writer().map_err(png_failure)?;
    let mut line = Vec::new();
    while let Some((rows, _)) = bands.next_band()? {
        line.clear();
        straight(rows, opaque, &mut line);
        stream
            .write_all(&line)
            .map_err(|error| host_failure(&error).unwrap_or_else(encode_failed))?;
    }
    stream.finish().map_err(png_failure)?;
    writer.finish().map_err(png_failure)
}

/// What the JPEG encoder reads: the band holding the rows it is at, as RGB over white.
struct Lazy<'b, 'p, 'r> {
    width: u32,
    height: u32,
    state: RefCell<LazyState<'b, 'p, 'r>>,
}

struct LazyState<'b, 'p, 'r> {
    bands: &'b mut Bands<'p, 'r>,
    rgb: Vec<u8>,
    top: u32,
    rows: u32,
    failure: Option<Failure>,
}

impl GenericImageView for Lazy<'_, '_, '_> {
    type Pixel = Rgb<u8>;

    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        let mut guard = self.state.borrow_mut();
        let state = &mut *guard;
        // The encoder goes down block row by block row, and a band holds whole block rows.
        while y >= state.top + state.rows && state.failure.is_none() {
            let next = state.top + state.rows;
            match state.bands.next_band() {
                Ok(Some((rows, count))) => {
                    state.rgb.clear();
                    over_white(rows, &mut state.rgb);
                    state.top = next;
                    state.rows = count;
                }
                Ok(None) => state.failure = Some(encode_failed()),
                Err(failure) => state.failure = Some(failure),
            }
        }
        if state.failure.is_some() || y < state.top {
            return Rgb([255, 255, 255]);
        }
        let i = (((y - state.top) * self.width + x) * 3) as usize;
        match state.rgb.get(i..i + 3) {
            Some(&[r, g, b]) => Rgb([r, g, b]),
            _ => Rgb([255, 255, 255]),
        }
    }
}

/// Every band of `bands` as one baseline JPEG of `quality` (1-100) into `out`.
pub fn jpeg(bands: &mut Bands<'_, '_>, quality: u8, out: impl Write) -> Result<(), Failure> {
    let (width, height) = bands.size();
    let lazy = Lazy {
        width,
        height,
        state: RefCell::new(LazyState {
            bands,
            rgb: Vec::new(),
            top: 0,
            rows: 0,
            failure: None,
        }),
    };
    let mut encoder = JpegEncoder::new_with_quality(out, quality.clamp(1, 100));
    let encoded = encoder.encode_image(&lazy);
    if let Some(failure) = lazy.state.into_inner().failure {
        return Err(failure);
    }
    encoded.map_err(|error| match error {
        ImageError::IoError(io) => host_failure(&io).unwrap_or_else(encode_failed),
        _ => encode_failed(),
    })
}

#[cfg(test)]
#[path = "encode_tests.rs"]
mod tests;
