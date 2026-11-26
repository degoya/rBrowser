//! The panel's view of an image too large for a canvas (RB-099-24). Chromium draws a canvas of
//! at most 32767 px a side and some 268 megapixels, and two of them (the image and the drawing)
//! of a 256-megapixel capture would take gigabytes; so an image over [`PREVIEW_SIDE`] a side or
//! [`PREVIEW_PIXELS`] in all reaches the panel scaled down by a whole factor ([`scale`]): each
//! preview pixel the average of `scale` x `scale` image pixels, decoded row by row like the
//! render. The panel draws the annotations in the image's own pixels over it; the saved file is
//! always rendered from the original at full resolution.

use std::cell::RefCell;
use std::io::Write;

use rb_plugin_api::{Failure, PluginHost};

use crate::codec::encode_failed;
use crate::encode::straight;
use crate::rows::{PngRows, RowSource};
use crate::stream::{RangeFile, Shared, host_failure};

/// The longest side the panel shows as it is (the editor's limit before RB-099-24).
pub const PREVIEW_SIDE: u32 = 16_384;
/// The most pixels the panel shows as they are.
pub const PREVIEW_PIXELS: u64 = 25_000_000;

/// The smallest whole factor that brings a `width` x `height` image within the preview's limits
/// and its PNG (`bytes` long, `0` when unknown) within the panel's ([`crate::cache::MAX_BYTES`]);
/// 1 for an image the panel shows as it is.
#[must_use]
pub fn scale(width: u32, height: u32, bytes: usize) -> u32 {
    let fits = |factor: u32| {
        let (w, h) = (width.div_ceil(factor), height.div_ceil(factor));
        w <= PREVIEW_SIDE && h <= PREVIEW_SIDE && u64::from(w) * u64::from(h) <= PREVIEW_PIXELS
    };
    let mut factor = 1;
    while !fits(factor) {
        factor += 1;
    }
    if factor == 1 && bytes > crate::cache::MAX_BYTES {
        factor = 2;
    }
    factor
}

fn write_failed(error: &std::io::Error) -> Failure {
    host_failure(error).unwrap_or_else(encode_failed)
}

/// A preview and the version it was made from.
#[derive(Debug)]
pub struct Preview {
    /// The preview as a PNG (pixels only).
    pub png: Vec<u8>,
    /// The fingerprint of the very bytes decoded for it; `None` when they were not read in order.
    pub hash: Option<String>,
}

/// The preview of the PNG `image` at `scale`, and the fingerprint of what it was made from.
pub fn encode(host: &mut dyn PluginHost, image: &str, scale: u32) -> Result<Preview, Failure> {
    let shared: Shared<'_> = RefCell::new(host);
    let mut file = RangeFile::open(&shared, image)?;
    let mut png = Vec::new();
    {
        let mut rows = PngRows::new(&mut file)?;
        downscale(&mut rows, scale, &mut png)?;
    }
    let hash = file.finish_hash()?;
    Ok(Preview { png, hash })
}

/// The rows of `source` averaged `scale` x `scale` into a PNG written to `out`.
pub fn downscale(source: &mut dyn RowSource, scale: u32, out: impl Write) -> Result<(), Failure> {
    let scale = scale.max(1);
    let (width, height) = source.size();
    let (wide, high) = (width.div_ceil(scale), height.div_ceil(scale));
    let opaque = source.opaque();
    let mut encoder = png::Encoder::new(out, wide, high);
    encoder.set_color(if opaque {
        png::ColorType::Rgb
    } else {
        png::ColorType::Rgba
    });
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header().map_err(|_| encode_failed())?;
    let mut stream = writer.stream_writer().map_err(|_| encode_failed())?;
    let mut row = vec![0u8; width as usize * 4];
    let mut sums = vec![0u32; wide as usize * 4];
    let mut averaged = vec![0u8; wide as usize * 4];
    let mut line = Vec::with_capacity(wide as usize * 4);
    for band in 0..high {
        sums.fill(0);
        let rows = scale.min(height - band * scale);
        for _ in 0..rows {
            source.next_row(&mut row)?;
            for (x, pixel) in row.chunks_exact(4).enumerate() {
                let at = x / scale as usize * 4;
                for (sum, value) in sums[at..at + 4].iter_mut().zip(pixel) {
                    *sum += u32::from(*value);
                }
            }
        }
        for (column, (sum, average)) in sums
            .chunks_exact(4)
            .zip(averaged.chunks_exact_mut(4))
            .enumerate()
        {
            let columns = scale.min(width - column as u32 * scale);
            let count = columns * rows;
            for (total, value) in sum.iter().zip(average.iter_mut()) {
                *value = u8::try_from((total + count / 2) / count).unwrap_or(u8::MAX);
            }
        }
        line.clear();
        straight(&averaged, opaque, &mut line);
        stream
            .write_all(&line)
            .map_err(|error| write_failed(&error))?;
    }
    stream.finish().map_err(|_| encode_failed())?;
    writer.finish().map_err(|_| encode_failed())
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod tests;
