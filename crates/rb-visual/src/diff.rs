//! The pixel diff of a baseline and a fresh capture (RB-050-04), after pixelmatch
//! (<https://github.com/mapbox/pixelmatch>, ISC): a perceptual colour distance in YIQ space per
//! pixel, compared with `visual.threshold` (0 exact, 1 anything goes, 0.1 the default), and an
//! anti-aliasing test, so a font or an edge smoothed a little differently is no regression.
//!
//! The answer: the pixels that differ and their share, the diff image (the baseline faded to
//! grey, a differing pixel red, an anti-aliased one yellow) and the bounding boxes of the
//! differing areas ([`crate::regions`]). Two images of different sizes are no diff but a
//! [`DiffOutcome::SizeMismatch`] naming both sizes.

use serde::{Deserialize, Serialize};

use crate::regions::{Region, regions};
use crate::{Rgba, VisualError};

/// `visual.threshold` when not set.
pub const DEFAULT_THRESHOLD: f64 = 0.1;
/// A pixel that differs, in the diff image.
pub const DIFF_COLOR: [u8; 4] = [255, 0, 0, 255];
/// A pixel that differs only by anti-aliasing, in the diff image.
pub const AA_COLOR: [u8; 4] = [255, 255, 0, 255];
/// The largest YIQ distance two RGB colours can have (black and white).
const MAX_YIQ_DELTA: f64 = 35_215.0;

/// How a diff decides.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiffOptions {
    /// The colour distance below which two pixels count as equal, 0 to 1; outside or not a
    /// number: [`DEFAULT_THRESHOLD`].
    pub threshold: f64,
    /// Count anti-aliased pixels as differences too.
    pub include_aa: bool,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_THRESHOLD,
            include_aa: false,
        }
    }
}

impl DiffOptions {
    /// The threshold in effect: `threshold` when it is a number from 0 to 1.
    pub fn threshold(&self) -> f64 {
        if (0.0..=1.0).contains(&self.threshold) {
            self.threshold
        } else {
            DEFAULT_THRESHOLD
        }
    }
}

/// A width and a height in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    /// Pixels across.
    pub width: u32,
    /// Pixels down.
    pub height: u32,
}

impl Size {
    /// The size of `image`.
    pub fn of(image: &Rgba) -> Self {
        Self {
            width: image.width(),
            height: image.height(),
        }
    }
}

/// Two images of the same size, compared.
#[derive(Clone, Debug, PartialEq)]
pub struct PixelDiff {
    /// The pixels that differ.
    pub different: u64,
    /// The pixels that differ only by anti-aliasing (not counted in `different`).
    pub antialiased: u64,
    /// Every pixel.
    pub total: u64,
    /// The diff image, the size of both.
    pub image: Rgba,
    /// The differing areas, largest first.
    pub regions: Vec<Region>,
    /// More areas than [`crate::regions::MAX_REGIONS`]: the smallest were left out.
    pub regions_truncated: bool,
}

impl PixelDiff {
    /// The share of differing pixels, 0 to 1.
    pub fn ratio(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            // Pixel counts stay far below 2^52, so the conversion is exact.
            self.different as f64 / self.total as f64
        }
    }
}

/// What a diff found.
#[derive(Clone, Debug, PartialEq)]
pub enum DiffOutcome {
    /// Same size: compared pixel by pixel.
    Compared(PixelDiff),
    /// Different sizes: nothing compared.
    SizeMismatch {
        /// The baseline's size.
        baseline: Size,
        /// The capture's size.
        actual: Size,
    },
}

/// Compares `actual` with `baseline`.
pub fn diff(
    baseline: &Rgba,
    actual: &Rgba,
    options: &DiffOptions,
) -> Result<DiffOutcome, VisualError> {
    let (width, height) = (baseline.width(), baseline.height());
    if (width, height) != (actual.width(), actual.height()) {
        return Ok(DiffOutcome::SizeMismatch {
            baseline: Size::of(baseline),
            actual: Size::of(actual),
        });
    }
    let max_delta = MAX_YIQ_DELTA * options.threshold() * options.threshold();
    let a = Pixels::of(baseline);
    let b = Pixels::of(actual);
    let mut out = Vec::with_capacity(baseline.pixels().len());
    let mut mask = vec![false; a.pixels.len()];
    let (mut different, mut antialiased) = (0_u64, 0_u64);
    for y in 0..height {
        for x in 0..width {
            let at = a.index(x, y);
            let delta = color_delta(a.pixels[at], b.pixels[at], false);
            if delta.abs() <= max_delta {
                out.extend_from_slice(&gray(a.pixels[at]));
            } else if !options.include_aa
                && (antialiased_at(&a, &b, x, y) || antialiased_at(&b, &a, x, y))
            {
                antialiased += 1;
                out.extend_from_slice(&AA_COLOR);
            } else {
                different += 1;
                mask[at] = true;
                out.extend_from_slice(&DIFF_COLOR);
            }
        }
    }
    let (regions, regions_truncated) = regions(&mask, width, height);
    Ok(DiffOutcome::Compared(PixelDiff {
        different,
        antialiased,
        total: u64::from(width) * u64::from(height),
        image: Rgba::from_raw(width, height, out)?,
        regions,
        regions_truncated,
    }))
}

/// An image as one RGBA value per pixel, borrowed: the diff copies no image.
struct Pixels<'a> {
    width: u32,
    height: u32,
    pixels: &'a [[u8; 4]],
}

impl<'a> Pixels<'a> {
    fn of(image: &'a Rgba) -> Self {
        Self {
            width: image.width(),
            height: image.height(),
            pixels: image.pixels().as_chunks::<4>().0,
        }
    }

    fn index(&self, x: u32, y: u32) -> usize {
        y as usize * self.width as usize + x as usize
    }

    /// The 3 x 3 neighbourhood of `x`, `y` clipped to the image, column by column (pixelmatch's
    /// order), without the centre; and whether the centre lies on the image's border.
    fn neighbours(&self, x: u32, y: u32) -> (Vec<(u32, u32)>, bool) {
        let (x0, y0) = (x.saturating_sub(1), y.saturating_sub(1));
        let (x2, y2) = ((x + 1).min(self.width - 1), (y + 1).min(self.height - 1));
        let border = x == x0 || x == x2 || y == y0 || y == y2;
        let mut around = Vec::with_capacity(8);
        for nx in x0..=x2 {
            for ny in y0..=y2 {
                if (nx, ny) != (x, y) {
                    around.push((nx, ny));
                }
            }
        }
        (around, border)
    }

    /// Whether the pixel at `x`, `y` has three or more neighbours of exactly its value.
    fn has_many_siblings(&self, x: u32, y: u32) -> bool {
        let value = self.pixels[self.index(x, y)];
        let (around, border) = self.neighbours(x, y);
        let mut equal = u32::from(border);
        for (nx, ny) in around {
            equal += u32::from(self.pixels[self.index(nx, ny)] == value);
            if equal > 2 {
                return true;
            }
        }
        false
    }
}

/// Whether the pixel at `x`, `y` of `image` looks anti-aliased: it has both a darker and a
/// brighter neighbour, not more than two equal ones, and the darkest or the brightest neighbour
/// is part of a flat area in both images.
fn antialiased_at(image: &Pixels<'_>, other: &Pixels<'_>, x: u32, y: u32) -> bool {
    let center = image.pixels[image.index(x, y)];
    let (around, border) = image.neighbours(x, y);
    let mut zeroes = u32::from(border);
    let (mut min, mut max) = (0.0_f64, 0.0_f64);
    let (mut darkest, mut brightest) = ((0, 0), (0, 0));
    for (nx, ny) in around {
        let delta = color_delta(center, image.pixels[image.index(nx, ny)], true);
        if delta == 0.0 {
            zeroes += 1;
            if zeroes > 2 {
                return false;
            }
        } else if delta < min {
            min = delta;
            darkest = (nx, ny);
        } else if delta > max {
            max = delta;
            brightest = (nx, ny);
        }
    }
    if min == 0.0 || max == 0.0 {
        return false;
    }
    (image.has_many_siblings(darkest.0, darkest.1) && other.has_many_siblings(darkest.0, darkest.1))
        || (image.has_many_siblings(brightest.0, brightest.1)
            && other.has_many_siblings(brightest.0, brightest.1))
}

/// A colour blended with white by its alpha.
fn on_white(pixel: [u8; 4]) -> (f64, f64, f64) {
    let [r, g, b, a] = pixel.map(f64::from);
    if pixel[3] == 255 {
        return (r, g, b);
    }
    let alpha = a / 255.0;
    let blend = |c: f64| 255.0 + (c - 255.0) * alpha;
    (blend(r), blend(g), blend(b))
}

fn rgb2y(r: f64, g: f64, b: f64) -> f64 {
    r * 0.298_895_31 + g * 0.586_622_47 + b * 0.114_482_23
}

fn rgb2i(r: f64, g: f64, b: f64) -> f64 {
    r * 0.595_977_99 - g * 0.274_176_10 - b * 0.321_801_89
}

fn rgb2q(r: f64, g: f64, b: f64) -> f64 {
    r * 0.211_470_17 - g * 0.522_617_11 + b * 0.311_146_94
}

/// The squared YIQ distance of two pixels, negative when the second is brighter; with
/// `y_only` the plain brightness difference.
fn color_delta(first: [u8; 4], second: [u8; 4], y_only: bool) -> f64 {
    if first == second {
        return 0.0;
    }
    let (r1, g1, b1) = on_white(first);
    let (r2, g2, b2) = on_white(second);
    let (y1, y2) = (rgb2y(r1, g1, b1), rgb2y(r2, g2, b2));
    let y = y1 - y2;
    if y_only {
        return y;
    }
    let i = rgb2i(r1, g1, b1) - rgb2i(r2, g2, b2);
    let q = rgb2q(r1, g1, b1) - rgb2q(r2, g2, b2);
    let delta = 0.5053 * y * y + 0.299 * i * i + 0.1957 * q * q;
    if y1 > y2 { -delta } else { delta }
}

/// An unchanged pixel in the diff image: its brightness, faded towards white.
fn gray(pixel: [u8; 4]) -> [u8; 4] {
    let [r, g, b, a] = pixel.map(u32::from);
    let luma = (r * 299 + g * 587 + b * 114) / 1000;
    // Transparent parts read as white, as in the comparison.
    let luma = 255 - (255 - luma) * a / 255;
    let faded = u8::try_from(255 - (255 - luma) / 10).unwrap_or(u8::MAX);
    [faded, faded, faded, 255]
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;
