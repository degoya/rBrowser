//! The calibration markers in a view's window image (RB-080-77, RB-090-19; split from
//! `calibrate.rs` in RB-090-40): what the page script draws ([`MARKER`]) and the boxes of rows
//! that show one ([`Finder`]) - a run of magenta pixels next to a run of green ones, told apart by
//! which channels dominate ([`DOMINANCE`]), over rows that form a box of the marker's shape,
//! framed in black so a page's colour beside it never runs into it. [`crate::calibrate`] maps the
//! viewport by them.

use crate::pixels::Rgb;

/// The marker's two colours, left and right half (`page/markers.js` draws them).
pub const MARKER: [Rgb; 2] = [[0xff, 0x00, 0xff], [0x00, 0xff, 0x00]];
/// A marker's coloured part in CSS px: two squares of 12 side by side.
pub const MARKER_SIZE: (f64, f64) = (24.0, 12.0);
/// The black frame around a marker's squares in CSS px (`page/markers.js`): the page script
/// reports the frame's top left corner.
pub const MARKER_FRAME: f64 = 2.0;
/// By how much magenta's red and blue (green's green) exceed the other channels at least: a
/// display's profile shifts the values, not which channels lead.
pub const DOMINANCE: u8 = 80;
/// How far a channel of the found marker may be from its own for the image's colours to be the
/// page's.
pub const EXACT: u8 = 3;
/// The most marker-like boxes kept of one image at once.
const MAX_BANDS: usize = 32;
/// How far a box may be from a marker's size at the measured scale: pixels, plus a share.
pub(crate) const SIZE_SLACK: (f64, f64) = (2.0, 0.2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Magenta,
    Green,
    Other,
}

pub(crate) fn class(rgb: Rgb) -> Class {
    class_by(rgb, DOMINANCE)
}

/// The class of a pixel whose leading channels exceed the others by `dominance`.
fn class_by([r, g, b]: Rgb, dominance: u8) -> Class {
    if r.saturating_sub(g) >= dominance && b.saturating_sub(g) >= dominance {
        Class::Magenta
    } else if g.saturating_sub(r) >= dominance && g.saturating_sub(b) >= dominance {
        Class::Green
    } else {
        Class::Other
    }
}

/// Whether the pixel at `at` of `row` leans to `kind`: a marker's edge blended half into its
/// black frame. Magenta keeps its lead there, green does not; taken on both ends alike, the box's
/// middle is the marker's.
fn leans(row: &[Rgb], at: Option<usize>, kind: Class) -> bool {
    at.and_then(|at| row.get(at))
        .is_some_and(|pixel| class_by(*pixel, DOMINANCE / 2) == kind)
}

/// A box of rows showing a marker: `left` to `right` (exclusive) image pixels, `top` to `bottom`
/// (exclusive) rows; `colours` its halves' middle pixels in its first row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band {
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
    pub colours: [Rgb; 2],
}

impl Band {
    /// The middle in image pixels.
    #[must_use]
    pub fn middle(&self) -> (f64, f64) {
        (
            f64::from(self.left + self.right) / 2.0,
            f64::from(self.top + self.bottom) / 2.0,
        )
    }

    /// Whether it has a marker's shape: twice as wide as high, give or take a blended edge.
    fn shaped(&self) -> bool {
        let (width, height) = (self.right - self.left, self.bottom - self.top);
        height >= 2 && (1.4..=3.0).contains(&(f64::from(width) / f64::from(height)))
    }

    /// Whether it is a marker's size at `scale` image px per CSS px.
    pub(crate) fn sized(&self, scale: f64) -> bool {
        let near = |pixels: u32, css: f64| {
            (f64::from(pixels) - css * scale).abs() <= SIZE_SLACK.0 + SIZE_SLACK.1 * css * scale
        };
        near(self.right - self.left, MARKER_SIZE.0) && near(self.bottom - self.top, MARKER_SIZE.1)
    }

    /// Whether its colours are the marker's own.
    pub(crate) fn exact(&self) -> bool {
        self.colours.iter().zip(MARKER).all(|(found, own)| {
            found
                .iter()
                .zip(own)
                .all(|(have, want)| have.abs_diff(want) <= EXACT)
        })
    }
}

/// One row's markers: a magenta run, at most two other pixels (a blended edge), a green run of a
/// like length; as `(start, end, colours)`.
fn hits(row: &[Rgb]) -> Vec<(u32, u32, [Rgb; 2])> {
    let mut runs: Vec<(Class, usize, usize)> = Vec::new();
    for (at, pixel) in row.iter().enumerate() {
        let kind = class(*pixel);
        match runs.last_mut() {
            Some((last, _, length)) if *last == kind => *length += 1,
            _ => runs.push((kind, at, 1)),
        }
    }
    let mut found = Vec::new();
    for (index, &(kind, start, length)) in runs.iter().enumerate() {
        if kind != Class::Magenta || length < 2 {
            continue;
        }
        let green = match runs.get(index + 1) {
            Some(&(Class::Other, _, gap)) if gap <= 2 => runs.get(index + 2),
            next => next,
        };
        let Some(&(Class::Green, green_start, green_length)) = green else {
            continue;
        };
        let (short, long) = (length.min(green_length), length.max(green_length));
        if short < 2 || long > 2 * short {
            continue;
        }
        let colours = [row[start + length / 2], row[green_start + green_length / 2]];
        let mut end = green_start + green_length;
        let mut start = start;
        if leans(row, start.checked_sub(1), Class::Magenta) {
            start -= 1;
        }
        if leans(row, Some(end), Class::Green) {
            end += 1;
        }
        if let (Ok(start), Ok(end)) = (u32::try_from(start), u32::try_from(end)) {
            found.push((start, end, colours));
        }
    }
    found
}

/// Collects the marker-like boxes of an image row by row ([`crate::pixels::decode_scan`]).
#[derive(Debug, Default)]
pub struct Finder {
    bands: Vec<Band>,
}

impl Finder {
    /// Row `y` of the image.
    pub fn row(&mut self, y: u32, row: &[Rgb]) {
        for (start, end, colours) in hits(row) {
            let open = self.bands.iter().position(|band| {
                band.bottom == y && band.left.abs_diff(start) <= 2 && band.right.abs_diff(end) <= 2
            });
            if let Some(band) = open.and_then(|index| self.bands.get_mut(index)) {
                band.bottom = y + 1;
                band.left = band.left.min(start);
                band.right = band.right.max(end);
                continue;
            }
            if self.bands.len() >= MAX_BANDS {
                // Rows come top to bottom: a box closed above without a marker's shape never gets
                // one. A colourful page's pictures leave many; the bottom markers come last.
                self.bands.retain(|band| band.bottom >= y || band.shaped());
            }
            if self.bands.len() < MAX_BANDS {
                self.bands.push(Band {
                    left: start,
                    right: end,
                    top: y,
                    bottom: y + 1,
                    colours,
                });
            }
        }
    }

    /// The boxes of a marker's shape, top to bottom.
    #[must_use]
    pub fn markers(&self) -> Vec<Band> {
        self.bands.iter().copied().filter(Band::shaped).collect()
    }
}
