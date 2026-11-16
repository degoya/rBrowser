//! Guides: where one lies in a page, the value a dragged position gives, snapping, and adding,
//! moving and removing by id.
//!
//! An `x` guide counts from the page's left edge (`px`), from its right edge (`px-right`) or in
//! per cent of its client width (`percent`); a `y` guide counts px from the page's top. Positions
//! are page CSS px, so a guide stays on the content when the page scrolls.

use serde::Serialize;

use crate::model::{Axis, Guide, GuideUnit, Invalid, MAX_GUIDES, MAX_LENGTH};

/// A guide placed in a page.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Placed {
    pub id: u32,
    pub axis: Axis,
    pub pos: f64,
}

/// Where `guide` lies in a page `client_width` CSS px wide, in page CSS px.
#[must_use]
pub fn position(guide: &Guide, client_width: f64) -> f64 {
    let width = client_width.max(0.0);
    match (guide.axis, guide.unit) {
        (Axis::X, GuideUnit::PxRight) => width - guide.value,
        (Axis::X, GuideUnit::Percent) => guide.value / 100.0 * width,
        _ => guide.value,
    }
}

/// Every guide placed in a page `client_width` CSS px wide.
#[must_use]
pub fn place(guides: &[Guide], client_width: f64) -> Vec<Placed> {
    guides
        .iter()
        .map(|guide| Placed {
            id: guide.id,
            axis: guide.axis,
            pos: position(guide, client_width),
        })
        .collect()
}

/// The value in `unit` of a guide at page position `pos`, held to the page.
#[must_use]
pub fn value_at(axis: Axis, unit: GuideUnit, pos: f64, client_width: f64) -> f64 {
    let width = client_width.max(0.0);
    let limit = if axis == Axis::X { width } else { MAX_LENGTH };
    let pos = if pos.is_finite() {
        pos.clamp(0.0, limit)
    } else {
        0.0
    };
    match (axis, unit) {
        (Axis::X, GuideUnit::PxRight) => width - pos,
        (Axis::X, GuideUnit::Percent) if width > 0.0 => {
            ((pos / width * 100.0) * 1000.0).round() / 1000.0
        }
        (Axis::X, GuideUnit::Percent) => 0.0,
        _ => pos,
    }
}

/// `pos` on the nearest multiple of `step` px; unchanged for a step of 0.
#[must_use]
pub fn snap(pos: f64, step: u32) -> f64 {
    if step == 0 {
        return pos;
    }
    let step = f64::from(step);
    (pos / step).round() * step
}

/// Adds a guide; answers its id, one above the highest.
///
/// # Errors
///
/// The guide is out of range or the list full.
pub fn add(
    guides: &mut Vec<Guide>,
    axis: Axis,
    unit: GuideUnit,
    value: f64,
) -> Result<u32, Invalid> {
    if guides.len() >= MAX_GUIDES {
        return Err("guides");
    }
    let id = guides
        .iter()
        .map(|guide| guide.id)
        .max()
        .map_or(1, |id| id.saturating_add(1));
    let guide = Guide {
        id,
        axis,
        unit,
        value,
    };
    guide.check()?;
    guides.push(guide);
    Ok(id)
}

/// Moves guide `id` to `value` (in its own unit).
///
/// # Errors
///
/// No such guide, or the value is out of range.
pub fn move_to(guides: &mut [Guide], id: u32, value: f64) -> Result<(), Invalid> {
    let guide = guides
        .iter_mut()
        .find(|guide| guide.id == id)
        .ok_or("guide.id")?;
    let moved = Guide {
        value,
        ..guide.clone()
    };
    moved.check()?;
    *guide = moved;
    Ok(())
}

/// Removes guide `id`; answers whether there was one.
pub fn remove(guides: &mut Vec<Guide>, id: u32) -> bool {
    let before = guides.len();
    guides.retain(|guide| guide.id != id);
    guides.len() != before
}

#[cfg(test)]
#[path = "guides_tests.rs"]
mod tests;
