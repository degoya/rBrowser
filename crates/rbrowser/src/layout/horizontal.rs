//! The horizontal mode: every view in one row at the same height, each as wide as its
//! breakpoint at the strip's scale, scrolled sideways one view at a time.
//!
//! The strip's scale is its zoom: `"fit"` makes the whole row fit the area's width (but no
//! smaller than [`MIN_SCALE`] and no larger than 1), a factor is taken as it is. A row wider
//! than the area scrolls: `first` is the view at the left edge, and every view that does not
//! lie wholly inside the area is hidden (and throttled).

use rb_config::{StripHeight, Zoom};
use rb_engine::CellRect;

use super::{Item, Slot, contains, split_label, to_dip};

/// The smallest scale `"fit"` goes to; a row that still does not fit scrolls.
pub const MIN_SCALE: f64 = 0.25;

/// The strip's parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strip {
    /// The views' height (label strip not included), `"fit"` for the area's.
    pub height: StripHeight,
    /// The gap between the views and around the row, in DIP.
    pub gap: u32,
    /// The label strip above each view, in DIP.
    pub label: u32,
    /// The scale: `"fit"` or a factor.
    pub zoom: Zoom,
    /// The view at the left edge.
    pub first: usize,
}

/// The scale of the strip's views in `area`.
pub fn scale(items: &[Item], area: CellRect, strip: &Strip) -> f64 {
    match strip.zoom {
        Zoom::Factor(factor) if factor.is_finite() && factor > 0.0 => factor.min(1.0),
        _ => {
            let total: u64 = items.iter().map(|item| u64::from(item.width)).sum();
            let gaps = u64::from(strip.gap) * (items.len() as u64 + 1);
            let free = u64::from(area.width).saturating_sub(gaps);
            if total == 0 {
                return 1.0;
            }
            (free as f64 / total as f64).clamp(MIN_SCALE, 1.0)
        }
    }
}

/// The views' widths at the strip's scale; none wider than the area's inside.
fn widths(items: &[Item], area: CellRect, strip: &Strip) -> Vec<u32> {
    let scale = scale(items, area, strip);
    let widest = area.width.saturating_sub(2 * strip.gap).max(1);
    items
        .iter()
        .map(|item| to_dip(f64::from(item.width) * scale).clamp(1, widest))
        .collect()
}

/// The largest useful `first`: from it on, the rest of the row fits the area.
pub fn last_first(items: &[Item], area: CellRect, strip: &Strip) -> usize {
    let widths = widths(items, area, strip);
    let mut used = u64::from(strip.gap);
    let mut first = widths.len();
    for (index, width) in widths.iter().enumerate().rev() {
        used += u64::from(*width) + u64::from(strip.gap);
        if used > u64::from(area.width) {
            break;
        }
        first = index;
    }
    first.min(widths.len().saturating_sub(1))
}

/// Where each view of `items` goes in `area`: in order, left to right from `strip.first`.
pub fn arrange(items: &[Item], area: CellRect, strip: &Strip) -> Vec<Slot> {
    let widths = widths(items, area, strip);
    let first = strip.first.min(last_first(items, area, strip));
    let gap = i64::from(strip.gap);
    let inside = area.height.saturating_sub(2 * strip.gap);
    let view_height = match strip.height {
        StripHeight::Fit => inside.saturating_sub(strip.label),
        StripHeight::Px(px) => px.min(inside.saturating_sub(strip.label)),
    };
    let top = area.y + strip.gap as i32;
    // The left edge of view `first` is `area.x + gap`; the others follow from it.
    let offset: i64 = widths[..first]
        .iter()
        .map(|width| i64::from(*width) + gap)
        .sum();
    let mut x = i64::from(area.x) + gap - offset;
    widths
        .iter()
        .map(|width| {
            let outer = CellRect {
                x: clamp_i32(x),
                y: top,
                width: *width,
                height: strip.label + view_height,
            };
            x += i64::from(*width) + gap;
            let visible = view_height > 0 && contains(area, outer);
            split_label(outer, strip.label, visible)
        })
        .collect()
}

/// `first` moved by one view, held to the useful range.
pub fn step(items: &[Item], area: CellRect, strip: &Strip, forward: bool) -> usize {
    let last = last_first(items, area, strip);
    let first = strip.first.min(last);
    if forward {
        (first + 1).min(last)
    } else {
        first.saturating_sub(1)
    }
}

/// The `first` that shows view `index` (it stays when the view is shown already).
pub fn reveal(items: &[Item], area: CellRect, strip: &Strip, index: usize) -> usize {
    let slots = arrange(items, area, strip);
    if slots.get(index).is_some_and(|slot| slot.visible) {
        return strip.first.min(last_first(items, area, strip));
    }
    index.min(last_first(items, area, strip))
}

fn clamp_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX })
}

#[cfg(test)]
#[path = "horizontal_tests.rs"]
mod tests;
