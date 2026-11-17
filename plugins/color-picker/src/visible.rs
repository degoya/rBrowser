//! The part of a view's viewport its window image is known to show (RB-090-40, diag13): where the
//! page script may put the loupe. A view the canvas area or the window cuts shows only part of
//! its viewport, and the loupe placed by the viewport's edges was cut off with it
//! (`owner-loupe-cut-right.png`: the canvas area's right edge through the loupe).
//!
//! What is known: the image's own bounds through the map (a view before the window's top left
//! edge starts there, RB-090-40) and the markers - one seen is where the view is seen, one drawn
//! inside the image and not seen is where it is cut off. On a side where a marker beyond the
//! farthest one seen is missing, the view is known to be seen up to that farthest one; where
//! none is missing, up to the image's bound. Without any marker seen nothing says where the view
//! is cut: the image bounds it.

use crate::calibrate::{Band, MARKER_FRAME, MARKER_SIZE, Map, agreeing, css_middle};

/// Half a marker's frame in CSS px: its middle to its edges.
const HALF: (f64, f64) = (
    MARKER_SIZE.0 / 2.0 + MARKER_FRAME,
    MARKER_SIZE.1 / 2.0 + MARKER_FRAME,
);
/// How far a missing marker must reach past the farthest one seen to bound a side (CSS px): the
/// corners on one side of the viewport lie at the same place.
const BEYOND: f64 = 1.0;

/// A rectangle of the viewport in CSS px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Area {
    /// The frame of the marker whose middle is `at`.
    fn around((x, y): (f64, f64)) -> Self {
        Self {
            left: x - HALF.0,
            top: y - HALF.1,
            right: x + HALF.0,
            bottom: y + HALF.1,
        }
    }

    fn inside(&self, outer: &Self) -> bool {
        self.left >= outer.left - BEYOND
            && self.top >= outer.top - BEYOND
            && self.right <= outer.right + BEYOND
            && self.bottom <= outer.bottom + BEYOND
    }
}

/// The part of the viewport `width` x `height` CSS px that the window image `map` reads is known
/// to show, by the markers drawn at `boxes` (their frames' top left corners) and the image's
/// marker-like `bands`.
#[must_use]
pub fn visible(
    map: &Map,
    bands: &[Band],
    boxes: &[(f64, f64)],
    (width, height): (f64, f64),
) -> Area {
    let image = Area {
        left: (-(map.left as f64) / map.scale).max(0.0),
        top: (-(map.top as f64) / map.scale).max(0.0),
        right: ((f64::from(map.width) - map.left as f64) / map.scale).min(width),
        bottom: ((f64::from(map.height) - map.top as f64) / map.scale).min(height),
    };
    let middles: Vec<(f64, f64)> = boxes.iter().map(|at| css_middle(*at)).collect();
    let shown: Vec<(f64, f64)> = agreeing(map, bands, &middles)
        .into_iter()
        .map(|(_, middle)| middle)
        .collect();
    let seen: Vec<Area> = shown.iter().map(|at| Area::around(*at)).collect();
    if seen.is_empty() {
        return image;
    }
    let missing: Vec<Area> = middles
        .iter()
        .filter(|at| {
            !shown
                .iter()
                .any(|one| one.0.to_bits() == at.0.to_bits() && one.1.to_bits() == at.1.to_bits())
        })
        .map(|at| Area::around(*at))
        .filter(|frame| frame.inside(&image))
        .collect();
    let lowest = |edge: fn(&Area) -> f64| seen.iter().map(edge).fold(f64::INFINITY, f64::min);
    let highest = |edge: fn(&Area) -> f64| seen.iter().map(edge).fold(f64::NEG_INFINITY, f64::max);
    let (left, top) = (lowest(|a| a.left), lowest(|a| a.top));
    let (right, bottom) = (highest(|a| a.right), highest(|a| a.bottom));
    let cut = |beyond: &dyn Fn(&Area) -> bool| missing.iter().any(beyond);
    Area {
        left: if cut(&|a: &Area| a.left < left - BEYOND) {
            left.max(image.left)
        } else {
            image.left
        },
        top: if cut(&|a: &Area| a.top < top - BEYOND) {
            top.max(image.top)
        } else {
            image.top
        },
        right: if cut(&|a: &Area| a.right > right + BEYOND) {
            right.min(image.right)
        } else {
            image.right
        },
        bottom: if cut(&|a: &Area| a.bottom > bottom + BEYOND) {
            bottom.min(image.bottom)
        } else {
            image.bottom
        },
    }
}

#[cfg(test)]
#[path = "visible_tests.rs"]
mod tests;
