//! One PNG of a page captured in sections (RB-099-11): a GPU texture holds at most 16384 device
//! pixels, so a long page is taken as several shots of at most that height and stacked here, top
//! to bottom, into one RGBA8 PNG.
//!
//! Memory stays bounded by rows: each section is decoded one row at a time and written straight
//! into the encoder's stream; no whole decoded image is ever held (the sections' compressed PNGs
//! are the caller's). Each section contributes exactly the rows its plan gave it: a renderer that
//! rounded one row more is cut back (no doubled row at the seam), one that gave up to
//! [`MAX_PAD_ROWS`] fewer repeats its last row (no gap); anything further off fails.
//!
//! A cut page (higher than the image may be) says so in a `tEXt` chunk ([`TRUNCATED_KEY`]),
//! which [`truncation`] reads back: the capture's answer names it without another channel.

use std::io::Cursor;

use crate::VisualError;
use crate::raster::{MAX_SIDE, Rgba, check_size, decode_png, png_size};

/// The most rows a section may lack against its plan; they repeat its last row.
pub const MAX_PAD_ROWS: u32 = 2;
/// The keyword of the `tEXt` chunk of a cut image: `<page height> <captured height>` in CSS px.
pub const TRUNCATED_KEY: &str = "rbrowser-truncated";

/// One section: its PNG and the rows it gives the image.
#[derive(Clone, Copy, Debug)]
pub struct Section<'a> {
    /// The section's PNG.
    pub png: &'a [u8],
    /// The rows of the image it fills.
    pub rows: u32,
}

/// A page cut at the image's limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Truncated {
    /// The whole page's height in CSS px.
    pub page_height: u32,
    /// The height in CSS px the image covers, from the top.
    pub captured_height: u32,
}

/// `sections` stacked top to bottom into one PNG, the cut recorded when `truncated`.
///
/// # Errors
///
/// No section, a section that is no PNG, interlaced, of another width than the first or more
/// than [`MAX_PAD_ROWS`] rows short, or an image over [`crate::raster::MAX_PIXELS`].
pub fn stitch(
    sections: &[Section<'_>],
    truncated: Option<Truncated>,
) -> Result<Vec<u8>, VisualError> {
    let first = sections
        .first()
        .ok_or_else(|| VisualError::Png("no section to stitch".to_owned()))?;
    let (width, _) =
        png_size(first.png).ok_or_else(|| VisualError::Png("a section is no PNG".to_owned()))?;
    let rows = sections
        .iter()
        .try_fold(0_u32, |sum, section| sum.checked_add(section.rows))
        .filter(|rows| *rows <= MAX_SIDE)
        .ok_or(VisualError::TooLarge {
            width: width.into(),
            height: u64::from(MAX_SIDE) + 1,
        })?;
    check_size(width, rows)?;
    let line = width as usize * 4;
    let failed = |error: png::EncodingError| VisualError::Png(error.to_string());
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, rows);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        if let Some(cut) = truncated {
            let text = format!("{} {}", cut.page_height, cut.captured_height);
            encoder
                .add_text_chunk(TRUNCATED_KEY.to_owned(), text)
                .map_err(failed)?;
        }
        let mut writer = encoder.write_header().map_err(failed)?;
        let mut stream = writer.stream_writer().map_err(failed)?;
        let mut row = Vec::with_capacity(line);
        for section in sections {
            copy_section(section, width, &mut row, &mut stream)?;
        }
        stream.finish().map_err(failed)?;
        writer.finish().map_err(failed)?;
    }
    Ok(bytes)
}

/// Writes `section.rows` RGBA rows of `section` into `stream`; `row` is the scratch line.
fn copy_section(
    section: &Section<'_>,
    width: u32,
    row: &mut Vec<u8>,
    stream: &mut impl std::io::Write,
) -> Result<(), VisualError> {
    let failed = |error: png::DecodingError| VisualError::Png(error.to_string());
    let mut decoder = png::Decoder::new(Cursor::new(section.png));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(failed)?;
    let info = reader.info();
    if info.width != width {
        return Err(VisualError::Png(format!(
            "a section is {} px wide, the first {width} px: no width to stitch",
            info.width
        )));
    }
    if info.interlaced {
        return Err(VisualError::Png("an interlaced section".to_owned()));
    }
    let (color, _) = reader.output_color_type();
    let written = |error: std::io::Error| VisualError::Png(error.to_string());
    let mut taken = 0;
    while taken < section.rows {
        let Some(next) = reader.next_row().map_err(failed)? else {
            break;
        };
        to_rgba(color, next.data(), row)?;
        stream.write_all(row).map_err(written)?;
        taken += 1;
    }
    let missing = section.rows - taken;
    if missing > MAX_PAD_ROWS || (missing > 0 && taken == 0) {
        return Err(VisualError::Png(format!(
            "a section gave {taken} rows of the {} planned",
            section.rows
        )));
    }
    for _ in 0..missing {
        stream.write_all(row).map_err(written)?;
    }
    Ok(())
}

/// `data`, one row of 8-bit `color` samples, as RGBA into `row`.
fn to_rgba(color: png::ColorType, data: &[u8], row: &mut Vec<u8>) -> Result<(), VisualError> {
    row.clear();
    match color {
        png::ColorType::Rgba => row.extend_from_slice(data),
        png::ColorType::Rgb => row.extend(
            data.as_chunks::<3>()
                .0
                .iter()
                .flat_map(|[r, g, b]| [*r, *g, *b, 255]),
        ),
        png::ColorType::GrayscaleAlpha => row.extend(
            data.as_chunks::<2>()
                .0
                .iter()
                .flat_map(|[grey, alpha]| [*grey, *grey, *grey, *alpha]),
        ),
        png::ColorType::Grayscale => row.extend(data.iter().flat_map(|g| [*g, *g, *g, 255])),
        png::ColorType::Indexed => {
            return Err(VisualError::Png(
                "an indexed PNG was not expanded".to_owned(),
            ));
        }
    }
    Ok(())
}

/// The top `rows` rows of `png` as RGBA (all of it when shorter) and the PNG's whole height: a
/// tall image read without decoding the rest (the overview's tile of a long page).
///
/// # Errors
///
/// `png` is no PNG, ends early, or its top is over [`crate::raster::MAX_PIXELS`].
pub fn decode_top(png: &[u8], rows: u32) -> Result<(Rgba, u32), VisualError> {
    let failed = |error: png::DecodingError| VisualError::Png(error.to_string());
    let mut decoder = png::Decoder::new(Cursor::new(png));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(failed)?;
    let (width, height, interlaced) = {
        let info = reader.info();
        (info.width, info.height, info.interlaced)
    };
    let kept = height.min(rows);
    let bytes = check_size(width, kept)?;
    if interlaced {
        let whole = decode_png(png)?;
        let top = whole.pixels().get(..bytes).unwrap_or_default().to_vec();
        return Ok((Rgba::from_raw(width, kept, top)?, height));
    }
    let (color, _) = reader.output_color_type();
    let mut pixels = Vec::with_capacity(bytes);
    let mut row = Vec::with_capacity(width as usize * 4);
    while pixels.len() < bytes {
        let Some(next) = reader.next_row().map_err(failed)? else {
            break;
        };
        to_rgba(color, next.data(), &mut row)?;
        pixels.extend_from_slice(&row);
    }
    Ok((Rgba::from_raw(width, kept, pixels)?, height))
}

/// The cut a stitched PNG records; `None` for a whole page or anything but such a PNG.
pub fn truncation(png: &[u8]) -> Option<Truncated> {
    let reader = png::Decoder::new(Cursor::new(png)).read_info().ok()?;
    let chunk = reader
        .info()
        .uncompressed_latin1_text
        .iter()
        .find(|chunk| chunk.keyword == TRUNCATED_KEY)?;
    let (page, captured) = chunk.text.split_once(' ')?;
    Some(Truncated {
        page_height: page.parse().ok()?,
        captured_height: captured.parse().ok()?,
    })
}

#[cfg(test)]
#[path = "stitch_tests.rs"]
mod tests;
