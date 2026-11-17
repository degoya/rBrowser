//! Where the viewport lies in a view's window image (RB-080-77, RB-090-19, RB-090-40).
//!
//! The window's image (`fromSurface: false`) is what the view shows: the viewport drawn at the
//! scale of its cell - canvas zoom, the emulation's scale, the display's DPR -, and **in the
//! display's colours**: on Windows with a colour profile the page's `#ff00ff` is `(231, 70, 251)`
//! and `#00ff00` `(124, 248, 59)` (diag9, 2026-11-15).
//!
//! **An image that is the viewport at its DPR** (give or take one CSS px the page rounds its
//! viewport to) maps it from its corner, at the DPR ([`direct`]), where a marker sits where that
//! puts it or no marker is seen at all - every image of diag11 (2026-11-15), where the markers'
//! pairing failed or took a box of the page for one and measured 0.5538 px per CSS px for an
//! image of exactly 1.5. Where boxes are seen and none of them there, the markers map it
//! ([`denied`]): on the canvas such an image held the page drawn at the canvas's scale in its top
//! left corner (diag12, 2026-11-16, RB-090-31), and its size put the loupe 2.2 times too far. Nor
//! where a pair of boxes maps it with more markers agreeing: at canvas zoom 75 % the corner box of
//! the page drawn at 0.75 of the DPR passes for the DPR's marker in place and size (diag13).
//!
//! **A view shown scaled** (canvas zoom, a cell smaller than the viewport) is measured by the
//! page script's markers ([`crate::finder`]) in the viewport's four corners and beside the
//! pointer, any two of them enough. A pair of boxes gives a scale and an offset ([`calibrate`])
//! that is kept only when it is not above the scale the image's size gives (the page may be drawn
//! smaller than the image, diag12), both boxes are a marker's size at it, and the most markers
//! agree. Whether the image's colours are the page's is told by the markers too ([`EXACT`]).
//!
//! **A view cut by the window's edge** (RB-090-40, diag12): the image is the window's pixels in
//! the view's rectangle, that rectangle moved to start at the window's top left corner where the
//! view begins above or left of it - view 6, 25 DIP above the window at DPR 1.5, showed its
//! bottom left marker 37 px higher than its size puts it. Such an image holds the viewport from
//! a point before its corner (a negative offset) - at the image's own scale or, drawn smaller on
//! the canvas, at the canvas's (diag13: 0.3 px per CSS px, 252 px before the corner) - and what
//! the window shows of other things where the view is cut off: the markers there are missing, the
//! two beside the pointer (which is where the view is seen) are not. Two readings that as many
//! markers agree with (two corners of one side or of the other) are refused, never guessed.

use crate::finder::SIZE_SLACK;
pub use crate::finder::{Band, DOMINANCE, EXACT, Finder, MARKER, MARKER_FRAME, MARKER_SIZE};
#[cfg(test)]
use crate::finder::{Class, class};
#[cfg(test)]
use crate::pixels::Rgb;

/// The shortest CSS distance between the markers on an axis that measures the scale.
const MIN_SPAN: f64 = 40.0;
/// How far the scales of the two axes may differ (the viewport is drawn at one scale).
const AXES_AGREE: f64 = 0.03;
/// How close a measured scale must be to the DPR or the image's width over the viewport's to be
/// taken as that one.
const SNAP: f64 = 0.01;
/// How far an image's side may be from the viewport's at the DPR for it to be its size, in CSS px
/// (at least one image pixel): the page reports whole CSS px, at a page zoom part of one off
/// (diag12: `1546` for 3480 px at 2.25).
const DIRECT_SLACK: f64 = 1.0;
/// How far a measured scale may be above the one the image's size gives (its share).
const SCALE_AGREES: f64 = 0.15;
/// How close a measured scale must be to the image's width over the viewport's to be taken as
/// that one where the DPR is not it: two markers beside the pointer are only 64 CSS px apart
/// (RB-090-40), a pixel's blend there is about 2 % of the distance.
const SNAP_IMAGE: f64 = 0.03;

/// The CSS px of the viewport of the markers' top left corners (their frames'), as the page
/// script measured them: the viewport's corners, then two beside the pointer (RB-090-40).
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

/// The CSS middle of a marker whose frame's top left corner is `at`.
pub(crate) fn css_middle(at: (f64, f64)) -> (f64, f64) {
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

/// The map of an image that is the viewport `width` x `height` CSS px at `dpr`, give or take a CSS
/// px: from its corner, at the DPR - a view shown at 100 %, unless its markers deny it
/// ([`denied`]).
#[must_use]
pub fn direct((width, height, dpr): (f64, f64, f64), image: (u32, u32)) -> Option<Map> {
    let slack = DIRECT_SLACK * dpr.max(1.0);
    let fits = |pixels: u32, css: f64| (f64::from(pixels) - css * dpr).abs() <= slack;
    (fits(image.0, width) && fits(image.1, height)).then_some(Map {
        left: 0,
        top: 0,
        scale: dpr,
        width: image.0,
        height: image.1,
    })
}

/// Whether the marker-like `bands` of an image of the viewport's size at its DPR deny its
/// [`direct`] map: boxes are seen and none of them is a marker where that map puts one of the
/// page script's markers inside the viewport (diag12: the page drawn smaller in the image's
/// corner). `false` for an image of another size.
#[must_use]
pub fn denied(
    bands: &[Band],
    boxes: &[(f64, f64)],
    (width, height, dpr): (f64, f64, f64),
    image: (u32, u32),
) -> bool {
    let corners = corners(boxes, (width, height));
    direct((width, height, dpr), image).is_some_and(|map| {
        !bands.is_empty() && !corners.is_empty() && agreeing(&map, bands, &corners).is_empty()
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
pub(crate) fn agreeing<'a>(
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

/// The [`direct`] `map` moved by what its `shown` markers say where the window's edge cut a few
/// pixels off the view's top or left (RB-090-40): their box is near enough to agree, the pixels
/// would be read that far off. A pixel either way is the blend of a marker's edge.
fn cut(map: Map, shown: &[(&Band, (f64, f64))]) -> Map {
    if shown.is_empty() {
        return map;
    }
    let moved = settled(map, shown);
    let off = |at: i64| if at < -1 { at } else { 0 };
    Map {
        left: off(moved.left),
        top: off(moved.top),
        ..map
    }
}

/// The map two boxes give as the markers of the corners `css`, if it holds: a scale not above
/// `expected` (the image's size over the viewport's; the page may be drawn smaller than the image
/// holds it, diag12), both boxes a marker's size at it, the viewport ending inside the image -
/// and starting at its corner or before it, where the window's edge cut the view (RB-090-40), at
/// any such scale (diag13: a view drawn at 0.3 in an image of 1.2 began 252 px before it).
fn pair_map(
    found: [&Band; 2],
    css: [(f64, f64); 2],
    (width, height, dpr): (f64, f64, f64),
    image: (u32, u32),
    expected: f64,
) -> Option<Map> {
    let middles = [found[0].middle(), found[1].middle()];
    let mut scale = measured_scale(middles, css)?;
    if scale > (1.0 + SCALE_AGREES) * expected || !found.iter().all(|band| band.sized(scale)) {
        return None;
    }
    // The DPR, or the image's width over the viewport's, where the markers measure it: no drift
    // of a pixel across the image.
    let own = f64::from(image.0) / width;
    if let Some(exact) = [(dpr, SNAP), (own, SNAP_IMAGE)]
        .into_iter()
        .find(|(candidate, snap)| (scale - candidate).abs() <= snap * candidate)
    {
        scale = exact.0;
    }
    // The viewport starts at a whole image pixel.
    let left = (middles[0].0 - css[0].0 * scale).round() as i64;
    let top = (middles[0].1 - css[0].1 * scale).round() as i64;
    // Before the image's corner where the window's edge cut the view, whatever its scale; the end
    // of the viewport inside the image.
    let fits = left as f64 + width * scale <= f64::from(image.0) + 1.5
        && top as f64 + height * scale <= f64::from(image.1) + 1.5;
    fits.then_some(Map {
        left,
        top,
        scale,
        width: image.0,
        height: image.1,
    })
}

/// The markers a map finds: each box with the CSS middle of the marker it is taken for.
type Shown<'a> = Vec<(&'a Band, (f64, f64))>;

/// What the pairs of boxes say ([`paired`]).
enum Paired {
    /// No pair maps the viewport.
    Missing,
    /// The map the most markers agree with, how many, whether their colours are the page's.
    Found(Map, usize, bool),
    /// Two maps that read the boxes differently, as many markers agreeing with each: how many.
    Rivals(usize),
}

/// Whether two maps read the boxes differently: a box both see taken for another marker, or no
/// box in common - two places the viewport could be (diag13: two corners of one side, or of the
/// other side before the image's corner).
fn rivals(one: &[(&Band, (f64, f64))], other: &[(&Band, (f64, f64))]) -> bool {
    let mut common = false;
    for (band, corner) in one {
        if let Some((_, theirs)) = other.iter().find(|(seen, _)| std::ptr::eq(*seen, *band)) {
            if theirs.0.to_bits() != corner.0.to_bits() || theirs.1.to_bits() != corner.1.to_bits()
            {
                return true;
            }
            common = true;
        }
    }
    !common
}

/// The map of the pair of boxes the most `corners` agree with, two at least; of those, the pair
/// farthest apart measures the scale best (the two beside the pointer are only 64 CSS px apart,
/// RB-090-40). Another reading as many agree with is a guess either way: [`Paired::Rivals`].
fn paired(
    bands: &[Band],
    corners: &[(f64, f64)],
    port: (f64, f64, f64),
    image: (u32, u32),
) -> Paired {
    let expected = (f64::from(image.0) / port.0).min(f64::from(image.1) / port.1);
    let mut best: Option<((usize, f64), Map, bool, Shown<'_>)> = None;
    let mut rival = false;
    for (index, first) in bands.iter().enumerate() {
        for second in &bands[index + 1..] {
            for (i, one) in corners.iter().enumerate() {
                for (j, other) in corners.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    let found = [first, second];
                    let Some(map) = pair_map(found, [*one, *other], port, image, expected) else {
                        continue;
                    };
                    let shown = agreeing(&map, bands, corners);
                    if shown.len() < 2 {
                        continue;
                    }
                    let rank = (shown.len(), (one.0 - other.0).hypot(one.1 - other.1));
                    let better = match &best {
                        None => true,
                        Some((most, ..)) if shown.len() > most.0 => {
                            rival = false;
                            true
                        }
                        Some((most, .., kept)) if shown.len() == most.0 && rivals(kept, &shown) => {
                            rival = true;
                            false
                        }
                        Some((most, ..)) => rank > *most,
                    };
                    if better {
                        let exact = shown.iter().all(|(band, _)| band.exact());
                        best = Some((rank, settled(map, &shown), exact, shown));
                    }
                }
            }
        }
    }
    match best {
        None => Paired::Missing,
        Some(((count, _), ..)) if rival => Paired::Rivals(count),
        Some(((count, _), map, exact, _)) => Paired::Found(map, count, exact),
    }
}

/// The log's words for boxes two readings explain alike.
fn rivalled(boxes: usize, count: usize) -> String {
    format!(
        "shows {boxes} marker-like boxes that two places of the viewport explain alike, {count} markers each"
    )
}

/// Where the viewport `width` x `height` CSS px at `dpr` lies in an image of `image` pixels whose
/// marker-like boxes are `bands`, the page script having drawn its markers at `boxes`; whether
/// the image's colours are the page's (a marker where the map puts it shows them). An image that
/// is the viewport at its DPR is mapped [`direct`]ly unless its boxes deny it ([`denied`]) or a
/// pair of them maps it where more markers agree; any other by the pair of boxes whose map the
/// most corners agree with, two at least ([`paired`]) - two such readings are refused.
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
    let port = (width, height, dpr);
    let corners = corners(boxes, (width, height));
    let pairs = if bands.len() >= 2 && corners.len() >= 2 {
        paired(bands, &corners, port, image)
    } else {
        Paired::Missing
    };
    if let Some(map) = direct(port, image)
        && !denied(bands, boxes, port, image)
    {
        let shown = agreeing(&map, bands, &corners);
        // diag13: at canvas zoom 75 % the corner box of the page drawn at 1.125 is near enough
        // in place and size to the marker DPR 1.5 puts there - the image's size then read every
        // pick 1.33 times too far. Markers that agree with another map in greater number are
        // what the image holds.
        match pairs {
            Paired::Found(found, count, exact) if count > shown.len() => {
                return Ok((found, exact));
            }
            Paired::Rivals(count) if count > shown.len() => {
                return Err(rivalled(bands.len(), count));
            }
            _ => {}
        }
        let exact = !shown.is_empty() && shown.iter().all(|(band, _)| band.exact());
        return Ok((cut(map, &shown), exact));
    }
    // Of the viewport's size at its DPR, and its boxes deny it: the log says so.
    let (one, any) = if direct(port, image).is_some() {
        (
            ", not where the image's size puts one",
            ", none where the image's size puts one,",
        )
    } else {
        ("", "")
    };
    if corners.len() < 2 {
        return Err(format!(
            "has {} of the page's markers inside the viewport",
            corners.len()
        ));
    }
    if bands.len() < 2 {
        return Err(format!(
            "shows {} of the {} markers{one}",
            bands.len(),
            corners.len()
        ));
    }
    match pairs {
        Paired::Found(map, _, exact) => Ok((map, exact)),
        Paired::Rivals(count) => Err(rivalled(bands.len(), count)),
        Paired::Missing => {
            let expected = (f64::from(image.0) / width).min(f64::from(image.1) / height);
            Err(format!(
                "shows {} marker-like boxes{any} but no two of them map the viewport into it at up to {expected:.4} px per CSS px",
                bands.len()
            ))
        }
    }
}

#[cfg(test)]
#[path = "calibrate_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "calibrate_diag11_tests.rs"]
mod diag11_tests;

#[cfg(test)]
#[path = "calibrate_diag12_tests.rs"]
mod diag12_tests;

#[cfg(test)]
#[path = "calibrate_clipped_tests.rs"]
mod clipped_tests;

#[cfg(test)]
#[path = "calibrate_diag13_tests.rs"]
mod diag13_tests;
