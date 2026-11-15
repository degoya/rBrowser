//! Where the viewport lies in a view's window image (RB-080-77, RB-090-19).
//!
//! The window's image (`fromSurface: false`) is what the view shows: the viewport drawn at the
//! scale of its cell - canvas zoom, the emulation's scale, the display's DPR -, and **in the
//! display's colours**: on Windows with a colour profile the page's `#ff00ff` is `(231, 70, 251)`
//! and `#00ff00` `(124, 248, 59)` (diag9, 2026-11-15).
//!
//! **An image that is the viewport at its DPR** (give or take the pixel Chromium rounds) maps it
//! without any marker ([`direct`]): from its corner, at the DPR. That is every view shown at
//! 100 % - every image of diag11 (2026-11-15), where the markers' pairing failed or took a box of
//! the page for one and measured 0.5538 px per CSS px for an image of exactly 1.5.
//!
//! **A view shown scaled** (canvas zoom, a cell smaller than the viewport) is measured by the
//! page script's [`MARKER`]s in the viewport's four corners, any two of them enough: a run of
//! magenta pixels next to a run of green ones, told apart by which channels dominate
//! ([`DOMINANCE`]), over rows that form a box of the marker's shape ([`Finder`]), framed in black
//! so a page's colour beside it never runs into it. A pair of boxes gives a scale and an offset
//! ([`calibrate`]) that is kept only when it is near the scale the image's size gives, both boxes
//! are a marker's size at it, and the most corners agree. Whether the image's colours are the
//! page's is told by the markers too ([`EXACT`]).

use crate::pixels::Rgb;

/// The marker's two colours, left and right half (`page/picker.js` draws them).
pub const MARKER: [Rgb; 2] = [[0xff, 0x00, 0xff], [0x00, 0xff, 0x00]];
/// A marker's coloured part in CSS px: two squares of 12 side by side.
pub const MARKER_SIZE: (f64, f64) = (24.0, 12.0);
/// The black frame around a marker's squares in CSS px (`page/picker.js`): the page script
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
/// The shortest CSS distance between the markers on an axis that measures the scale.
const MIN_SPAN: f64 = 40.0;
/// How far the scales of the two axes may differ (the viewport is drawn at one scale).
const AXES_AGREE: f64 = 0.03;
/// How close a measured scale must be to the DPR or the image's width over the viewport's to be
/// taken as that one.
const SNAP: f64 = 0.01;
/// How far an image's side may be from the viewport's at the DPR for it to map without markers.
const DIRECT_SLACK: f64 = 1.0;
/// How far a measured scale may be from the one the image's size gives (its share).
const SCALE_AGREES: f64 = 0.15;
/// How far a box may be from a marker's size at the measured scale: pixels, plus a share.
const SIZE_SLACK: (f64, f64) = (2.0, 0.2);

/// The CSS px of the viewport of the markers' top left corners (their frames'), as the page
/// script measured them: top left, top right, bottom left, bottom right.
pub type Marker = Vec<(f64, f64)>;

/// Where the viewport lies in a window image of `width` x `height` pixels: its CSS point 0, 0 at
/// the image pixel `left`, `top`, `scale` image pixels per CSS px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Map {
    pub left: i64,
    pub top: i64,
    pub scale: f64,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Magenta,
    Green,
    Other,
}

fn class(rgb: Rgb) -> Class {
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
    fn sized(&self, scale: f64) -> bool {
        let near = |pixels: u32, css: f64| {
            (f64::from(pixels) - css * scale).abs() <= SIZE_SLACK.0 + SIZE_SLACK.1 * css * scale
        };
        near(self.right - self.left, MARKER_SIZE.0) && near(self.bottom - self.top, MARKER_SIZE.1)
    }

    /// Whether its colours are the marker's own.
    fn exact(&self) -> bool {
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

/// The CSS middle of a marker whose frame's top left corner is `at`.
fn css_middle(at: (f64, f64)) -> (f64, f64) {
    (
        at.0 + MARKER_FRAME + MARKER_SIZE.0 / 2.0,
        at.1 + MARKER_FRAME + MARKER_SIZE.1 / 2.0,
    )
}

/// The scale the two middles measure, if they hold: on each axis the markers are far enough apart
/// on, the image's distance over the CSS one; both axes agreeing.
fn measured_scale(found: [(f64, f64); 2], css: [(f64, f64); 2]) -> Option<f64> {
    let axis = |image: f64, css: f64| (css.abs() >= MIN_SPAN).then(|| image / css);
    let scales: Vec<f64> = [
        axis(found[1].0 - found[0].0, css[1].0 - css[0].0),
        axis(found[1].1 - found[0].1, css[1].1 - css[0].1),
    ]
    .into_iter()
    .flatten()
    .collect();
    let (low, high) = scales
        .iter()
        .fold((f64::INFINITY, 0.0_f64), |(low, high), scale| {
            (low.min(*scale), high.max(*scale))
        });
    if scales.is_empty() || low <= 0.05 || high > 16.0 || high - low > AXES_AGREE * high {
        return None;
    }
    Some(scales.iter().sum::<f64>() / scales.len() as f64)
}

/// The map of an image that is the viewport `width` x `height` CSS px at `dpr`, give or take a
/// pixel: from its corner, at the DPR - a view shown at 100 %, no marker needed.
#[must_use]
pub fn direct((width, height, dpr): (f64, f64, f64), image: (u32, u32)) -> Option<Map> {
    let fits = |pixels: u32, css: f64| (f64::from(pixels) - css * dpr).abs() <= DIRECT_SLACK;
    (fits(image.0, width) && fits(image.1, height)).then_some(Map {
        left: 0,
        top: 0,
        scale: dpr,
        width: image.0,
        height: image.1,
    })
}

/// The CSS middles of the markers of `boxes` that lie inside the viewport `width` x `height`: one
/// the page drew elsewhere (a `transform` or `filter` on `<html>` makes `fixed` relative to the
/// document) is not looked for.
fn corners(boxes: &[(f64, f64)], (width, height): (f64, f64)) -> Vec<(f64, f64)> {
    let outer = (
        MARKER_SIZE.0 + 2.0 * MARKER_FRAME,
        MARKER_SIZE.1 + 2.0 * MARKER_FRAME,
    );
    boxes
        .iter()
        .filter(|(x, y)| x + outer.0 <= width + 1.0 && y + outer.1 <= height + 1.0)
        .map(|at| css_middle(*at))
        .collect()
}

/// The boxes of `bands` that sit where `map` puts the markers of `corners` and are their size,
/// with their corner: one per corner at most.
fn agreeing<'a>(
    map: &Map,
    bands: &'a [Band],
    corners: &[(f64, f64)],
) -> Vec<(&'a Band, (f64, f64))> {
    let reach = SIZE_SLACK.0 + SIZE_SLACK.1 * MARKER_SIZE.1 * map.scale;
    corners
        .iter()
        .filter_map(|(x, y)| {
            let want = (
                map.left as f64 + x * map.scale,
                map.top as f64 + y * map.scale,
            );
            bands
                .iter()
                .find(|band| {
                    let (bx, by) = band.middle();
                    band.sized(map.scale)
                        && (bx - want.0).abs() <= reach
                        && (by - want.1).abs() <= reach
                })
                .map(|band| (band, (*x, *y)))
        })
        .collect()
}

/// `map` with its offset the mean of what the `shown` markers give: a scaled marker's edges blend
/// into its frame, so one box alone may be half a pixel off.
fn settled(map: Map, shown: &[(&Band, (f64, f64))]) -> Map {
    let count = shown.len() as f64;
    let mean = |axis: fn((f64, f64)) -> f64| {
        shown
            .iter()
            .map(|(band, corner)| axis(band.middle()) - axis(*corner) * map.scale)
            .sum::<f64>()
            / count
    };
    Map {
        left: mean(|at| at.0).round() as i64,
        top: mean(|at| at.1).round() as i64,
        ..map
    }
}

/// The map two boxes give as the markers of the corners `css`, if it holds: a scale near
/// `expected` (the image's size over the viewport's), both boxes a marker's size at it, the
/// viewport inside the image.
fn pair_map(
    found: [&Band; 2],
    css: [(f64, f64); 2],
    (width, height, dpr): (f64, f64, f64),
    image: (u32, u32),
    expected: f64,
) -> Option<Map> {
    let middles = [found[0].middle(), found[1].middle()];
    let mut scale = measured_scale(middles, css)?;
    if (scale - expected).abs() > SCALE_AGREES * expected
        || !found.iter().all(|band| band.sized(scale))
    {
        return None;
    }
    // The DPR, or the image's width over the viewport's, where the markers measure it: no drift
    // of a pixel across the image.
    if let Some(exact) = [dpr, f64::from(image.0) / width]
        .into_iter()
        .find(|candidate| (scale - candidate).abs() <= SNAP * candidate)
    {
        scale = exact;
    }
    // The viewport starts at a whole image pixel.
    let left = (middles[0].0 - css[0].0 * scale).round() as i64;
    let top = (middles[0].1 - css[0].1 * scale).round() as i64;
    let fits = left >= -1
        && top >= -1
        && left as f64 + width * scale <= f64::from(image.0) + 1.5
        && top as f64 + height * scale <= f64::from(image.1) + 1.5;
    fits.then_some(Map {
        left,
        top,
        scale,
        width: image.0,
        height: image.1,
    })
}

/// Where the viewport `width` x `height` CSS px at `dpr` lies in an image of `image` pixels whose
/// marker-like boxes are `bands`, the page script having drawn its markers at `boxes`; whether
/// the image's colours are the page's (a marker where the map puts it shows them). An image that
/// is the viewport at its DPR is mapped [`direct`]ly, whatever boxes it shows; any other by the
/// pair of boxes whose map the most corners agree with, two at least.
///
/// # Errors
///
/// Why the image does not hold the viewport, for the log.
pub fn calibrate(
    bands: &[Band],
    boxes: &[(f64, f64)],
    (width, height, dpr): (f64, f64, f64),
    image: (u32, u32),
) -> Result<(Map, bool), String> {
    let corners = corners(boxes, (width, height));
    if let Some(map) = direct((width, height, dpr), image) {
        let shown = agreeing(&map, bands, &corners);
        let exact = !shown.is_empty() && shown.iter().all(|(band, _)| band.exact());
        return Ok((map, exact));
    }
    if corners.len() < 2 {
        return Err(format!(
            "has {} of the page's markers inside the viewport",
            corners.len()
        ));
    }
    if bands.len() < 2 {
        return Err(format!(
            "shows {} of the {} markers",
            bands.len(),
            corners.len()
        ));
    }
    let expected = (f64::from(image.0) / width).min(f64::from(image.1) / height);
    let mut best: Option<(usize, Map, bool)> = None;
    'pairs: for (index, first) in bands.iter().enumerate() {
        for second in &bands[index + 1..] {
            for (i, one) in corners.iter().enumerate() {
                for (j, other) in corners.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    let found = [first, second];
                    let port = (width, height, dpr);
                    let Some(map) = pair_map(found, [*one, *other], port, image, expected) else {
                        continue;
                    };
                    let shown = agreeing(&map, bands, &corners);
                    if shown.len() >= 2
                        && best.as_ref().is_none_or(|(most, ..)| shown.len() > *most)
                    {
                        let exact = shown.iter().all(|(band, _)| band.exact());
                        best = Some((shown.len(), settled(map, &shown), exact));
                        if shown.len() == corners.len() {
                            break 'pairs;
                        }
                    }
                }
            }
        }
    }
    best.map(|(_, map, exact)| (map, exact)).ok_or_else(|| {
        format!(
            "shows {} marker-like boxes but no two of them map the viewport into it at about {expected:.4} px per CSS px",
            bands.len()
        )
    })
}

#[cfg(test)]
#[path = "calibrate_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "calibrate_diag11_tests.rs"]
mod diag11_tests;
