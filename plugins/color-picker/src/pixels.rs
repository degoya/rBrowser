//! The view's image and the pixels around a point in it, DPR-aware.
//!
//! The component reads the view as its window shows it - one `Page.captureScreenshot` without a
//! clip ([`crate::sample`], RB-080-64) - and keeps only the [`Region`] around the point it needs:
//! the PNG is decoded row by row and stops after the region's last row, so a viewport of
//! millions of pixels costs one row of memory besides the region. Where the window's image
//! cannot be had, a [`Clip`] of the document around the point is screenshot instead (RB-080-56's
//! way, which makes the view flicker for a frame).
//!
//! A point in CSS px maps to the image pixel in the middle of its CSS pixel ([`image_point`]):
//! with 3 image pixels per CSS px, the CSS pixel (200, 120) is the image pixel (601, 361) of the
//! viewport's image. The loupe is the `loupe` x `loupe` image pixels around it.

use base64::Engine as _;
use rb_plugin_api::Failure;

use crate::color::Rgba;
use crate::{bad_data, invalid};

/// The most pixels an image of a view may have: 8 megapixels, a 4K screen (review C2 of
/// RB-080-64). The PNG is decoded row by row, so this bounds the work - inflating a larger one
/// could use up the call's fuel, a trap the clip's fallback cannot catch -, not the memory; a
/// larger window image is refused and a clip read instead.
pub const MAX_PIXELS: u64 = 1 << 23;

/// Half the side of the pipette's tile in image pixels. The page script keeps the tile and draws
/// the loupe from it while the pointer stays inside, so a pointer move costs no screenshot
/// (RB-080-56). The tile's [`MAX_TILE_BYTES`] as base64 stay under the 64 KiB a message to the
/// page may have.
pub const TILE_HALF: u32 = 58;
/// The most RGB bytes of a tile: 126 x 126 pixels - a window tile is `2 * TILE_HALF + 1` image
/// pixels, a clipped one ([`tile_around`]) at most `2 * TILE_HALF + dpr`, 124 at DPR 8.
pub const MAX_TILE_BYTES: usize = 126 * 126 * 3;

/// A screenshot's clip in CSS pixels of the document.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clip {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

fn usable_dpr(dpr: f64) -> f64 {
    if dpr.is_finite() && dpr >= 0.25 {
        dpr
    } else {
        1.0
    }
}

/// The clip around the CSS pixel at `x`, `y` (document coordinates) wide enough for a loupe of
/// `loupe` device pixels at `dpr`.
#[must_use]
pub fn clip_around(x: f64, y: f64, dpr: f64, loupe: u32) -> Clip {
    let half = (f64::from(loupe / 2) / usable_dpr(dpr)).ceil().max(1.0);
    square(x, y, half)
}

/// The clip of the pipette's tile around the CSS pixel at `x`, `y` (document coordinates):
/// [`TILE_HALF`] device pixels on each side at `dpr`.
#[must_use]
pub fn tile_around(x: f64, y: f64, dpr: f64) -> Clip {
    let half = (f64::from(TILE_HALF) / usable_dpr(dpr)).floor().max(1.0);
    square(x, y, half)
}

fn square(x: f64, y: f64, half: f64) -> Clip {
    let side = 2.0 * half + 1.0;
    Clip {
        x: (x.floor() - half).max(0.0),
        y: (y.floor() - half).max(0.0),
        width: side,
        height: side,
    }
}

/// A rectangle of an image, in its pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    /// The `2 * half + 1` pixels square around the pixel `x`, `y` of an image of `width` x
    /// `height`, cut at its edges; empty when the pixel lies outside.
    #[must_use]
    pub fn around(x: i64, y: i64, half: u32, width: u32, height: u32) -> Self {
        let span = |at: i64, size: u32| -> (u32, u32) {
            let start = (at - i64::from(half)).clamp(0, i64::from(size));
            let end = (at + i64::from(half) + 1).clamp(0, i64::from(size));
            // Both clamped into 0..=size, a u32.
            (
                u32::try_from(start).unwrap_or(0),
                u32::try_from(end - start).unwrap_or(0),
            )
        };
        let (left, columns) = span(x, width);
        let (top, rows) = span(y, height);
        Self {
            left,
            top,
            width: columns,
            height: rows,
        }
    }

    /// The region cut to an image of `width` x `height`.
    #[must_use]
    pub fn within(self, width: u32, height: u32) -> Self {
        let left = self.left.min(width);
        let top = self.top.min(height);
        Self {
            left,
            top,
            width: self.width.min(width - left),
            height: self.height.min(height - top),
        }
    }

    fn holds(&self, x: u32, y: u32) -> bool {
        x >= self.left && y >= self.top && x - self.left < self.width && y - self.top < self.height
    }
}

/// A decoded image of `width` x `height` pixels of which [`Image::region`] is kept: 8-bit RGB,
/// row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    region: Region,
    rgb: Vec<u8>,
}

impl Image {
    /// The pixels kept.
    #[must_use]
    pub fn region(&self) -> Region {
        self.region
    }

    /// The region's pixels: three bytes each, row by row.
    #[must_use]
    pub fn rgb(&self) -> &[u8] {
        &self.rgb
    }

    /// The opaque colour of the image's pixel at `x`, `y`; `None` outside the region kept.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<Rgba> {
        if !self.region.holds(x, y) {
            return None;
        }
        let row = usize::try_from(y - self.region.top).ok()?;
        let column = usize::try_from(x - self.region.left).ok()?;
        let at = (row * usize::try_from(self.region.width).ok()? + column) * 3;
        let rgb = self.rgb.get(at..at + 3)?;
        Some(Rgba {
            r: f64::from(rgb[0]),
            g: f64::from(rgb[1]),
            b: f64::from(rgb[2]),
            a: 1.0,
        })
    }
}

/// The whole screenshot CDP answered (`data`, base64 PNG).
///
/// # Errors
///
/// As [`decode_region`].
pub fn decode(data: &str) -> Result<Image, Failure> {
    decode_region(data, |width, height| Region {
        left: 0,
        top: 0,
        width,
        height,
    })
}

/// The region `pick` names for the image's size of the screenshot CDP answered (`data`, base64
/// PNG), as RGB pixels; any alpha is dropped (a screenshot of a page is opaque). Rows below the
/// region are not decoded.
///
/// # Errors
///
/// As [`decode_scan`].
pub fn decode_region(data: &str, pick: impl FnOnce(u32, u32) -> Region) -> Result<Image, Failure> {
    decode_scan(data, pick, None)
}

/// The width and height of the screenshot CDP answered (`data`, base64 PNG), read from its header
/// alone: nothing is inflated (RB-090-19).
///
/// # Errors
///
/// `bad_data` for no base64 PNG.
pub fn image_size(data: &str) -> Result<(u32, u32), Failure> {
    let reader = png_reader(data)?;
    Ok((reader.info().width, reader.info().height))
}

/// The PNG of `data` (base64), its header read.
fn png_reader(data: &str) -> Result<png::Reader<std::io::Cursor<Vec<u8>>>, Failure> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| bad_data("screenshot"))?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    decoder.read_info().map_err(|_| bad_data("screenshot"))
}

/// One pixel's 8-bit red, green and blue.
pub type Rgb = [u8; 3];

/// What sees every row of an image: its index and its pixels.
pub type RowVisitor<'a> = &'a mut dyn FnMut(u32, &[Rgb]);

/// [`decode_region`]; with `scan`, every row of the image is decoded and handed to it as well -
/// the window image's markers ([`crate::calibrate`], RB-080-77). Without, rows below the region
/// are not decoded.
///
/// # Errors
///
/// `bad_data` for no base64 PNG, an interlaced one or more than [`MAX_PIXELS`] pixels.
pub fn decode_scan(
    data: &str,
    pick: impl FnOnce(u32, u32) -> Region,
    mut scan: Option<RowVisitor<'_>>,
) -> Result<Image, Failure> {
    let mut reader = png_reader(data)?;
    let (width, height) = (reader.info().width, reader.info().height);
    if u64::from(width) * u64::from(height) > MAX_PIXELS || width == 0 || height == 0 {
        return Err(bad_data("screenshot_size"));
    }
    // Chromium writes no interlaced PNG; its rows would come in passes.
    if reader.info().interlaced {
        return Err(bad_data("screenshot"));
    }
    let channels = match reader.output_color_type().0 {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err(bad_data("screenshot")),
    };
    let region = pick(width, height).within(width, height);
    let last = if scan.is_some() {
        height
    } else {
        region.top + region.height
    };
    let colour = |pixel: &[u8]| match channels {
        1 | 2 => [pixel[0]; 3],
        _ => [pixel[0], pixel[1], pixel[2]],
    };
    let (from, to) = (
        usize::try_from(region.left).unwrap_or(0),
        usize::try_from(region.left + region.width).unwrap_or(0),
    );
    let mut rgb = Vec::with_capacity(region.width as usize * region.height as usize * 3);
    let mut line: Vec<Rgb> = Vec::new();
    for y in 0..last {
        let row = reader
            .next_row()
            .map_err(|_| bad_data("screenshot"))?
            .ok_or_else(|| bad_data("screenshot"))?;
        let pixels = row.data();
        if let Some(visit) = scan.as_mut() {
            line.clear();
            line.extend(pixels.chunks_exact(channels).map(colour));
            visit(y, &line);
        }
        if y < region.top || y >= region.top + region.height {
            continue;
        }
        for pixel in pixels.chunks_exact(channels).skip(from).take(to - from) {
            rgb.extend_from_slice(&colour(pixel));
        }
    }
    if rgb.len() != region.width as usize * region.height as usize * 3 {
        return Err(bad_data("screenshot"));
    }
    Ok(Image {
        width,
        height,
        region,
        rgb,
    })
}

/// The image pixel in the middle of the CSS pixel at `point`, in an image whose first pixel
/// shows the CSS coordinate `origin` at `scale` image pixels per CSS px (the viewport's image:
/// `origin` 0, `point` in the viewport; a clip: its origin and `point` in the document).
#[must_use]
pub fn image_point(point: f64, origin: f64, scale: f64) -> i64 {
    // At most MAX_COORDINATE times a scale of at most 8: far inside i64.
    ((point.floor() + 0.5 - origin) * scale).floor() as i64
}

/// The pixel under a point and the loupe around it.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub color: Rgba,
    /// The device pixel it was read from: of the viewport (the window's image) or of the
    /// document (a clip).
    pub device: (u32, u32),
    /// Image pixels per CSS pixel: the view's DPR when it is shown at it.
    pub scale: f64,
    /// `loupe` x `loupe` image pixels row by row, `None` outside the image.
    pub grid: Vec<Option<Rgba>>,
}

/// How CSS px map into an image: its first pixel shows the CSS point `origin` (a clip's, in the
/// document) or the CSS point 0, 0 lies at the image pixel `offset` (the viewport in the window's
/// image, RB-080-77), at `scale` image pixels per CSS px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mapping {
    pub origin: (f64, f64),
    pub offset: (i64, i64),
    pub scale: f64,
}

impl Mapping {
    /// A clip's or an image's whose first pixel shows `origin`.
    #[must_use]
    pub fn from_origin(origin: (f64, f64), scale: f64) -> Self {
        Self {
            origin,
            offset: (0, 0),
            scale,
        }
    }

    /// The image pixel in the middle of the CSS pixel at `x`, `y`.
    #[must_use]
    pub fn point(&self, (x, y): (f64, f64)) -> (i64, i64) {
        (
            self.offset.0 + image_point(x, self.origin.0, self.scale),
            self.offset.1 + image_point(y, self.origin.1, self.scale),
        )
    }
}

/// The sample of the point `x`, `y` (CSS px) in `image`, whose first pixel shows `origin` at
/// `scale` ([`image_point`]); `None` when the point's pixel is not in the region kept.
#[must_use]
pub fn sample(
    image: &Image,
    origin: (f64, f64),
    scale: f64,
    at: (f64, f64),
    loupe: u32,
) -> Option<Sample> {
    sample_mapped(image, &Mapping::from_origin(origin, scale), at, loupe)
}

/// [`sample`] by a [`Mapping`]; `device` is the pixel counted from the mapping's CSS 0, 0.
#[must_use]
pub fn sample_mapped(
    image: &Image,
    mapping: &Mapping,
    at: (f64, f64),
    loupe: u32,
) -> Option<Sample> {
    let (x, y) = mapping.point(at);
    let px = u32::try_from(x).ok()?;
    let py = u32::try_from(y).ok()?;
    let color = image.pixel(px, py)?;
    let half = i64::from(loupe / 2);
    let mut grid = Vec::new();
    for dy in -half..=half {
        for dx in -half..=half {
            let cell = (
                u32::try_from(i64::from(px) + dx).ok(),
                u32::try_from(i64::from(py) + dy).ok(),
            );
            grid.push(match cell {
                (Some(cx), Some(cy)) => image.pixel(cx, cy),
                _ => None,
            });
        }
    }
    // The origin's device pixel: a whole number of them before the image starts.
    let shift = |at: f64| (at * mapping.scale).round() as i64;
    let device = |origin: f64, offset: i64, pixel: u32| {
        u32::try_from(shift(origin) + i64::from(pixel) - offset).unwrap_or(0)
    };
    Some(Sample {
        color,
        device: (
            device(mapping.origin.0, mapping.offset.0, px),
            device(mapping.origin.1, mapping.offset.1, py),
        ),
        scale: mapping.scale,
        grid,
    })
}

/// Checks a loupe size: odd, 5 to 31.
///
/// # Errors
///
/// `invalid` naming `loupe`.
pub fn loupe_size(value: u64) -> Result<u32, Failure> {
    match u32::try_from(value) {
        Ok(size) if (5..=31).contains(&size) && size % 2 == 1 => Ok(size),
        _ => Err(invalid("loupe")),
    }
}

#[cfg(test)]
#[path = "pixels_tests.rs"]
pub(crate) mod tests;
