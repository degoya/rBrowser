//! Where an external window goes (`engines.external.placement`): `beside` the main window,
//! on `monitor:<n>`, or where it was last (`remember`), each further window cascaded. Pure
//! geometry in physical screen pixels; the engine moves the window (BiDi
//! `browser.setClientWindowState`, Win32 as the fallback; ADR 0001, Decision 7).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The offset of each further window of a cascade, in px.
pub const CASCADE_STEP: i32 = 32;
/// The gap between the main window and the first window beside it, in px.
pub const GAP: i32 = 8;
/// How much of a remembered window must lie on a monitor for it to be used again: its top-left
/// corner's title bar, this wide and high.
const GRIP: (u32, u32) = (120, 32);

/// A window's outer rectangle in physical screen pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowRect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

impl WindowRect {
    fn right(&self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    fn bottom(&self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }

    fn contains(&self, x: i64, y: i64) -> bool {
        x >= i64::from(self.x) && x < self.right() && y >= i64::from(self.y) && y < self.bottom()
    }

    /// Whether the area of `other` overlapping this one is at least `width` x `height`.
    fn overlaps_by(&self, other: &WindowRect, width: u32, height: u32) -> bool {
        let left = i64::from(self.x).max(i64::from(other.x));
        let top = i64::from(self.y).max(i64::from(other.y));
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        right - left >= i64::from(width) && bottom - top >= i64::from(height)
    }

    /// This rectangle moved and, if needed, shrunk so it lies inside `area`.
    fn clamped_to(&self, area: &WindowRect) -> WindowRect {
        let width = self.width.min(area.width);
        let height = self.height.min(area.height);
        let max_x = area.right() - i64::from(width);
        let max_y = area.bottom() - i64::from(height);
        let x = i64::from(self.x).clamp(i64::from(area.x), max_x);
        let y = i64::from(self.y).clamp(i64::from(area.y), max_y);
        WindowRect {
            x: i32::try_from(x).unwrap_or(area.x),
            y: i32::try_from(y).unwrap_or(area.y),
            width,
            height,
        }
    }
}

/// `engines.external.placement`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Placement {
    /// Right of the main window (the default).
    #[default]
    Beside,
    /// On monitor `n` (1-based, the order the OS lists them).
    Monitor(u32),
    /// Where the window was last, per project and view; `beside` the first time.
    Remember,
}

/// A placement that is not `beside`, `remember` or `monitor:<n>` with `n` from 1 to 99.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("placement must be \"beside\", \"remember\" or \"monitor:<n>\", not \"{0}\"")]
pub struct PlacementError(pub String);

impl FromStr for Placement {
    type Err = PlacementError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "beside" => Ok(Placement::Beside),
            "remember" => Ok(Placement::Remember),
            _ => text
                .strip_prefix("monitor:")
                .and_then(|n| n.parse::<u32>().ok())
                .filter(|n| (1..=99).contains(n))
                .map(Placement::Monitor)
                .ok_or_else(|| PlacementError(text.to_owned())),
        }
    }
}

impl fmt::Display for Placement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Placement::Beside => f.write_str("beside"),
            Placement::Monitor(n) => write!(f, "monitor:{n}"),
            Placement::Remember => f.write_str("remember"),
        }
    }
}

impl Serialize for Placement {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Placement {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// The screen as the placement sees it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Screen {
    /// The main window.
    pub main: WindowRect,
    /// The monitors' work areas, in the OS's order; the main window's monitor when empty.
    pub monitors: Vec<WindowRect>,
}

impl Screen {
    /// The work area that holds the point, else the main window's, else the first one, else
    /// the main window itself.
    fn area_at(&self, x: i64, y: i64) -> WindowRect {
        let main = &self.main;
        let (main_x, main_y) = (
            i64::from(main.x) + i64::from(main.width / 2),
            i64::from(main.y) + i64::from(main.height / 2),
        );
        self.monitors
            .iter()
            .find(|area| area.contains(x, y))
            .or_else(|| {
                self.monitors
                    .iter()
                    .find(|area| area.contains(main_x, main_y))
            })
            .or_else(|| self.monitors.first())
            .copied()
            .unwrap_or(self.main)
    }
}

/// Where window number `index` (0-based, in arrange order) of `size` goes under `placement`.
/// `remembered` is its last rectangle under `remember`; one that no longer lies on a monitor is
/// ignored. The result always lies inside a work area.
pub fn place(
    placement: Placement,
    screen: &Screen,
    size: (u32, u32),
    index: usize,
    remembered: Option<WindowRect>,
) -> WindowRect {
    let step = i32::try_from(index)
        .unwrap_or(i32::MAX / CASCADE_STEP)
        .saturating_mul(CASCADE_STEP);
    let (width, height) = size;
    match placement {
        Placement::Remember => match remembered {
            Some(rect)
                if screen
                    .monitors
                    .iter()
                    .any(|area| area.overlaps_by(&rect, GRIP.0, GRIP.1)) =>
            {
                rect
            }
            _ => place(Placement::Beside, screen, size, index, None),
        },
        Placement::Monitor(n) => {
            let Some(area) = usize::try_from(n)
                .ok()
                .and_then(|n| n.checked_sub(1))
                .and_then(|n| screen.monitors.get(n))
            else {
                return place(Placement::Beside, screen, size, index, None);
            };
            WindowRect {
                x: area.x.saturating_add(CASCADE_STEP).saturating_add(step),
                y: area.y.saturating_add(CASCADE_STEP).saturating_add(step),
                width,
                height,
            }
            .clamped_to(area)
        }
        Placement::Beside => {
            let main = screen.main;
            let x = main
                .x
                .saturating_add(i32::try_from(main.width).unwrap_or(i32::MAX))
                .saturating_add(GAP)
                .saturating_add(step);
            let y = main.y.saturating_add(step);
            let area = screen.area_at(i64::from(x), i64::from(y));
            WindowRect {
                x,
                y,
                width,
                height,
            }
            .clamped_to(&area)
        }
    }
}

#[cfg(test)]
#[path = "placement_tests.rs"]
mod tests;
