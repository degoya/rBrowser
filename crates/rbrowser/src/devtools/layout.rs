//! Where DevTools go: the dock side of each mode, the order `devtools.dock_cycle` walks, and
//! the dock's size (the settings `devtools.dockHeight` and `devtools.dockWidth`, set by the
//! splitter), held between a minimum and the part of the window the grid can give up.

use rb_config::DevtoolsMode;
use serde::{Deserialize, Serialize};

use crate::host::ui::DockSide;

/// The smallest dock in DIP: the frontend's toolbar and one row of panels.
pub const MIN_DOCK: u32 = 120;
/// The largest dock in DIP, whatever the window.
pub const MAX_DOCK: u32 = 4000;
/// The share of the window the grid keeps at least, in percent.
pub const GRID_KEEPS_PERCENT: u32 = 25;

/// The dock's size on each side, in DIP (`devtools.dockHeight`, `devtools.dockWidth`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DockSizes {
    /// The height below the grid.
    pub bottom: u32,
    /// The width beside the grid.
    pub right: u32,
}

impl Default for DockSizes {
    fn default() -> Self {
        Self {
            bottom: 320,
            right: 480,
        }
    }
}

impl DockSizes {
    /// The size kept for `side`, held between [`MIN_DOCK`] and [`MAX_DOCK`].
    pub fn size_for(&self, side: DockSide) -> u32 {
        let size = match side {
            DockSide::Bottom => self.bottom,
            DockSide::Right => self.right,
        };
        size.clamp(MIN_DOCK, MAX_DOCK)
    }
}

/// The dock side of `mode`; `None` for the window of its own.
pub fn side_of(mode: DevtoolsMode) -> Option<DockSide> {
    match mode {
        DevtoolsMode::DockedBottom => Some(DockSide::Bottom),
        DevtoolsMode::DockedRight => Some(DockSide::Right),
        DevtoolsMode::Window => None,
    }
}

/// The mode after `mode` for `devtools.dock_cycle`: bottom, right, window, bottom again.
pub fn next_mode(mode: DevtoolsMode) -> DevtoolsMode {
    match mode {
        DevtoolsMode::DockedBottom => DevtoolsMode::DockedRight,
        DevtoolsMode::DockedRight => DevtoolsMode::Window,
        DevtoolsMode::Window => DevtoolsMode::DockedBottom,
    }
}

/// The dock's size along its axis when the content area (grid plus dock) measures `available`
/// DIP on that axis: `wanted`, but never under [`MIN_DOCK`] and never so large that the grid
/// keeps less than [`GRID_KEEPS_PERCENT`] of the area. An area too small for both gives the
/// grid its share and the dock the rest.
pub fn dock_extent(wanted: u32, available: u32) -> u32 {
    let grid_floor = available.saturating_mul(GRID_KEEPS_PERCENT) / 100;
    let ceiling = available.saturating_sub(grid_floor);
    wanted.max(MIN_DOCK).min(ceiling)
}

/// The grid's extent along the dock's axis once the dock takes its share of `available`.
pub fn grid_extent(wanted: u32, available: u32) -> u32 {
    available.saturating_sub(dock_extent(wanted, available))
}

/// What a dock beside the views wants: its axis and its size along it, in DIP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DockWant {
    /// Beside the views (its size is a width), else below them (a height).
    pub horizontal: bool,
    /// The size it is set to.
    pub wanted: u32,
    /// A fixed size - the folded plugin panels' strip (RB-080-61) - taken as it is, below
    /// [`MIN_DOCK`] too, and counted before a dock beside it on the same axis gets its share.
    pub fixed: bool,
}

/// The sizes of the docked plugin panels and docked DevTools in a content area of `width` x
/// `height` DIP - the views and both docks (RB-080-57); 0 for a dock not shown. One dock alone,
/// or two on different axes, are each [`dock_extent`]. Two on the same axis never leave the
/// views less than [`GRID_KEEPS_PERCENT`] of the whole area: when together they want more than
/// it can give, that room is shared in the ratio of what each wants; a fixed want (the folded
/// panels' strip, RB-080-61) keeps its size and the other dock gets the rest.
pub fn shared_extents(
    width: u32,
    height: u32,
    panels: Option<DockWant>,
    devtools: Option<DockWant>,
) -> (u32, u32) {
    let axis = |want: DockWant| if want.horizontal { width } else { height };
    let ceiling_of = |available: u32| {
        available.saturating_sub(available.saturating_mul(GRID_KEEPS_PERCENT) / 100)
    };
    let alone = |want: Option<DockWant>| {
        want.map_or(0, |want| {
            if want.fixed {
                want.wanted.min(ceiling_of(axis(want)))
            } else {
                dock_extent(want.wanted, axis(want))
            }
        })
    };
    let floor = |want: DockWant| {
        if want.fixed {
            want.wanted
        } else {
            want.wanted.max(MIN_DOCK)
        }
    };
    match (panels, devtools) {
        (Some(first), Some(second)) if first.horizontal == second.horizontal => {
            let ceiling = ceiling_of(axis(first));
            let (a, b) = (floor(first), floor(second));
            if a.saturating_add(b) <= ceiling {
                return (a, b);
            }
            // A fixed strip keeps its size; the other dock gets the rest of the room.
            if first.fixed {
                let a = a.min(ceiling);
                return (a, ceiling - a);
            }
            if second.fixed {
                let b = b.min(ceiling);
                return (ceiling - b, b);
            }
            let share = u64::from(ceiling) * u64::from(a) / (u64::from(a) + u64::from(b));
            let share = u32::try_from(share).unwrap_or(ceiling);
            (share, ceiling - share)
        }
        _ => (alone(panels), alone(devtools)),
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
