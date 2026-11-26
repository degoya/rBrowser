//! PNG in, PNG or JPEG out. The size is checked before a pixel is decoded, so a crafted header
//! cannot make the component allocate more than its memory; the encoders write the pixels and
//! nothing else - no text chunk, no time, no colour profile - so the edited file carries none of
//! the original's metadata.
//!
//! The editor opens every image the capture writes (RB-099-24): up to [`MAX_SIDE`] a side and
//! [`MAX_PIXELS`] in all, `rb_visual::raster`'s limits. Such an image is rendered in bands
//! (`crate::band`, `crate::encode`); the whole-image functions here ([`decode`], [`encode_png`],
//! [`encode_jpeg`]) take at most [`WHOLE_PIXELS`] and serve the tests and an interlaced PNG.

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder, ImageFormat, ImageReader, Limits};
use rb_plugin_api::{Failure, FailureKind};
use tiny_skia::{ColorU8, IntSize, Pixmap};

use crate::PLUGIN_ID;

/// The longest side the editor opens: the capture's (`rb_visual::raster::MAX_SIDE`).
pub const MAX_SIDE: u32 = 65_535;
/// The most pixels the editor opens: the capture's (`rb_visual::raster::MAX_PIXELS`, 256
/// megapixels, 1 GiB as RGBA - never in memory whole, the bands take a few MiB).
pub const MAX_PIXELS: u64 = 256 * 1024 * 1024;
/// The most pixels decoded whole (100 MB as RGBA).
pub const WHOLE_PIXELS: u64 = 25_000_000;

/// `plugin.screenshot-editor.errors.not_png`.
pub(crate) fn not_png() -> Failure {
    Failure::coded(
        FailureKind::BadData,
        &format!("plugin.{PLUGIN_ID}.errors.not_png"),
    )
}

/// `plugin.screenshot-editor.errors.image_too_large` naming the size and the limits.
fn too_large(width: u32, height: u32) -> Failure {
    Failure::coded(
        FailureKind::Limit,
        &format!("plugin.{PLUGIN_ID}.errors.image_too_large"),
    )
    .with_param("width", width)
    .with_param("height", height)
    .with_param("max_side", MAX_SIDE)
    .with_param("max_pixels", MAX_PIXELS)
}

fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    // The decoded RGBA image and the decoder's own buffers.
    limits.max_alloc = Some(WHOLE_PIXELS * 4 + 16 * 1024 * 1024);
    limits
}

fn reader(bytes: &[u8]) -> ImageReader<Cursor<&[u8]>> {
    ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png)
}

/// `width` x `height` held to the capture's limits: `image_too_large` naming them otherwise.
pub fn check_size(width: u32, height: u32) -> Result<(), Failure> {
    if width == 0
        || height == 0
        || width > MAX_SIDE
        || height > MAX_SIDE
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err(too_large(width, height));
    }
    Ok(())
}

/// The width and height of the PNG `bytes` (its signature and header are enough), held to the
/// limits.
pub fn dimensions(bytes: &[u8]) -> Result<(u32, u32), Failure> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    let header = decoder.read_header_info().map_err(|_| not_png())?;
    let (width, height) = (header.width, header.height);
    check_size(width, height)?;
    Ok((width, height))
}

/// The PNG `bytes` as a pixmap (premultiplied RGBA), at most [`WHOLE_PIXELS`].
pub fn decode(bytes: &[u8]) -> Result<Pixmap, Failure> {
    let (width, height) = dimensions(bytes)?;
    if u64::from(width) * u64::from(height) > WHOLE_PIXELS {
        return Err(too_large(width, height));
    }
    let mut reader = reader(bytes);
    reader.limits(limits());
    let image = reader.decode().map_err(|_| not_png())?.into_rgba8();
    let mut data = image.into_raw();
    for pixel in data.chunks_exact_mut(4) {
        let premultiplied =
            ColorU8::from_rgba(pixel[0], pixel[1], pixel[2], pixel[3]).premultiply();
        pixel.copy_from_slice(&[
            premultiplied.red(),
            premultiplied.green(),
            premultiplied.blue(),
            premultiplied.alpha(),
        ]);
    }
    let size = IntSize::from_wh(width, height).ok_or_else(not_png)?;
    Pixmap::from_vec(data, size).ok_or_else(not_png)
}

fn opaque(rgba: &[u8]) -> bool {
    rgba.chunks_exact(4).all(|pixel| pixel[3] == u8::MAX)
}

fn rgb(rgba: &[u8], background: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgba.len() / 4 * 3);
    for pixel in rgba.chunks_exact(4) {
        let alpha = u32::from(pixel[3]);
        for channel in &pixel[..3] {
            // Straight alpha over an opaque background.
            let mixed =
                (u32::from(*channel) * alpha + u32::from(background) * (255 - alpha) + 127) / 255;
            out.push(u8::try_from(mixed).unwrap_or(u8::MAX));
        }
    }
    out
}

/// `plugin.screenshot-editor.errors.encode_failed`.
pub(crate) fn encode_failed() -> Failure {
    Failure::coded(
        FailureKind::Other,
        &format!("plugin.{PLUGIN_ID}.errors.encode_failed"),
    )
}

/// `pixmap` as a PNG within a plugin's write limit ([`encode_png_within`]).
pub fn encode_png(pixmap: &Pixmap) -> Result<Vec<u8>, Failure> {
    encode_png_within(pixmap, rb_plugin_api::MAX_IO_BYTES)
}

/// `pixmap` as a PNG of at most `limit` bytes: the fast compression first, the best one when that
/// is larger (a big full-page shot), else `plugin.screenshot-editor.errors.output_too_large`.
pub fn encode_png_within(pixmap: &Pixmap, limit: usize) -> Result<Vec<u8>, Failure> {
    let fast = encode_png_with(pixmap, CompressionType::Fast)?;
    if fast.len() <= limit {
        return Ok(fast);
    }
    let best = encode_png_with(pixmap, CompressionType::Best)?;
    if best.len() <= limit {
        return Ok(best);
    }
    Err(output_too_large(best.len(), limit))
}

/// `plugin.screenshot-editor.errors.output_too_large` naming the size and the limit.
pub fn output_too_large(bytes: usize, limit: usize) -> Failure {
    Failure::coded(
        FailureKind::Limit,
        &format!("plugin.{PLUGIN_ID}.errors.output_too_large"),
    )
    .with_param("bytes", bytes)
    .with_param("limit", limit)
}

/// `pixmap` as a PNG with `compression`: RGB when every pixel is opaque, else RGBA; pixels only.
pub(crate) fn encode_png_with(
    pixmap: &Pixmap,
    compression: CompressionType,
) -> Result<Vec<u8>, Failure> {
    let (width, height) = (pixmap.width(), pixmap.height());
    let rgba = pixmap.clone().take_demultiplied();
    let mut out = Vec::new();
    let encoder = PngEncoder::new_with_quality(&mut out, compression, FilterType::Adaptive);
    let written = if opaque(&rgba) {
        encoder.write_image(&rgb(&rgba, 255), width, height, ExtendedColorType::Rgb8)
    } else {
        encoder.write_image(&rgba, width, height, ExtendedColorType::Rgba8)
    };
    written.map_err(|_| encode_failed())?;
    Ok(out)
}

/// `pixmap` as a baseline JPEG of `quality` (1-100), transparency over white; pixels only.
pub fn encode_jpeg(pixmap: &Pixmap, quality: u8) -> Result<Vec<u8>, Failure> {
    let (width, height) = (pixmap.width(), pixmap.height());
    let rgba = pixmap.clone().take_demultiplied();
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100))
        .write_image(&rgb(&rgba, 255), width, height, ExtendedColorType::Rgb8)
        .map_err(|_| encode_failed())?;
    Ok(out)
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
