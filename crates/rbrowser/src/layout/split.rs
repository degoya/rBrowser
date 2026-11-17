//! The split mode: two sides with a divider between them. With a vertical divider the primary
//! side is on the left and the views of each side are stacked; with a horizontal one the primary
//! side is on top and the views of each side sit side by side. `ratio` is the primary side's
//! share of the space; the divider is the gap between the sides. A side without views gives its
//! space to the other.

use rb_config::{SPLIT_RATIO_RANGE, SplitOrientation};
use rb_engine::CellRect;

use super::{Slot, split_label, to_dip};

/// The split's parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Split {
    /// Where the divider runs.
    pub orientation: SplitOrientation,
    /// The primary side's share, held to [`SPLIT_RATIO_RANGE`].
    pub ratio: f64,
    /// The gap around and between the views, in DIP; the divider is one gap wide.
    pub gap: u32,
    /// The label strip above each view, in DIP.
    pub label: u32,
}

/// `ratio` held to [`SPLIT_RATIO_RANGE`]; a ratio that is no number is the middle.
pub fn clamp_ratio(ratio: f64) -> f64 {
    if ratio.is_finite() {
        ratio.clamp(SPLIT_RATIO_RANGE.0, SPLIT_RATIO_RANGE.1)
    } else {
        0.5
    }
}

/// The ratio after one keyboard step of `step` towards the secondary side (`grow`) or back.
pub fn step_ratio(ratio: f64, step: f64, grow: bool) -> f64 {
    let delta = if grow { step } else { -step };
    // Three decimals, so repeated steps stay on the grid of the step.
    (clamp_ratio(ratio + delta) * 1000.0).round() / 1000.0
}

/// Where the `primary` and then the `secondary` views go in `area`, in that order.
pub fn arrange(primary: usize, secondary: usize, area: CellRect, split: &Split) -> Vec<Slot> {
    let vertical = split.orientation == SplitOrientation::Vertical;
    let (inner, first_size, second_size) = sides(primary, secondary, area, split);
    let second_offset = if first_size > 0 && second_size > 0 {
        first_size + split.gap
    } else {
        0
    };
    let mut slots = stack(
        side_rect(inner, vertical, 0, first_size),
        primary,
        vertical,
        split,
    );
    slots.extend(stack(
        side_rect(inner, vertical, second_offset, second_size),
        secondary,
        vertical,
        split,
    ));
    slots
}

/// The divider between the two sides: the gap between them, across the whole area; `None`
/// when one side is empty.
pub fn divider(
    primary: usize,
    secondary: usize,
    area: CellRect,
    split: &Split,
) -> Option<CellRect> {
    let (inner, first_size, second_size) = sides(primary, secondary, area, split);
    if first_size == 0 || second_size == 0 {
        return None;
    }
    let at = first_size as i32;
    Some(if split.orientation == SplitOrientation::Vertical {
        CellRect {
            x: inner.x + at,
            y: area.y,
            width: split.gap,
            height: area.height,
        }
    } else {
        CellRect {
            x: area.x,
            y: inner.y + at,
            width: area.width,
            height: split.gap,
        }
    })
}

/// The area inside the outer gaps and the primary and secondary sides' sizes along the main
/// axis; a side without views gets none.
fn sides(primary: usize, secondary: usize, area: CellRect, split: &Split) -> (CellRect, u32, u32) {
    let gap = split.gap;
    let inner = CellRect {
        x: area.x + gap as i32,
        y: area.y + gap as i32,
        width: area.width.saturating_sub(2 * gap),
        height: area.height.saturating_sub(2 * gap),
    };
    let along = if split.orientation == SplitOrientation::Vertical {
        inner.width
    } else {
        inner.height
    };
    let (first, second) = match (primary, secondary) {
        (0, 0) => (0, 0),
        (_, 0) => (along, 0),
        (0, _) => (0, along),
        _ => {
            let main = along.saturating_sub(gap);
            let first = to_dip(f64::from(main) * clamp_ratio(split.ratio)).min(main);
            (first, main - first)
        }
    };
    (inner, first, second)
}

/// One side of `inner`: `size` along the main axis from `offset`, the whole cross axis.
fn side_rect(inner: CellRect, vertical: bool, offset: u32, size: u32) -> CellRect {
    if vertical {
        CellRect {
            x: inner.x + offset as i32,
            width: size,
            ..inner
        }
    } else {
        CellRect {
            y: inner.y + offset as i32,
            height: size,
            ..inner
        }
    }
}

/// `count` views across `side`: stacked when the divider is vertical, side by side when it is
/// horizontal, sharing the space after the gaps, spread one DIP at a time.
fn stack(side: CellRect, count: usize, vertical: bool, split: &Split) -> Vec<Slot> {
    if count == 0 {
        return Vec::new();
    }
    let gap = u64::from(split.gap);
    let across = u64::from(if vertical { side.height } else { side.width });
    let free = across.saturating_sub(gap * (count as u64 - 1));
    let mut slots = Vec::with_capacity(count);
    let mut start = 0u64;
    for index in 0..count as u64 {
        let end = free * (index + 1) / count as u64;
        let size = u32::try_from(end - free * index / count as u64).unwrap_or(0);
        let offset = i32::try_from(start).unwrap_or(i32::MAX);
        let outer = if vertical {
            CellRect {
                y: side.y + offset,
                height: size,
                ..side
            }
        } else {
            CellRect {
                x: side.x + offset,
                width: size,
                ..side
            }
        };
        let visible = outer.width > 0 && outer.height > split.label;
        slots.push(split_label(outer, split.label, visible));
        start += u64::from(size) + gap;
    }
    slots
}

#[cfg(test)]
#[path = "split_tests.rs"]
mod tests;
