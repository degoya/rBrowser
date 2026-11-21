//! The pointer's way of a step (RB-090-87): the recorder keeps the way that led to a menu's hover
//! or a click (`automation.recorder.recordPointer`), in CSS px of the viewport of the view it was
//! recorded in. Before the step's action the pointer moves along it with `page.mouse.move` - at
//! the `recorded` pace with the recorded times between the points, at `fast` through the points
//! without waits -, then the locator's hover or click goes to the element as before. Only in a
//! view of the recording's size (`view.width`/`view.height` in the flow, `page.viewportSize()`
//! in the spec): elsewhere the points lie somewhere else, and the step goes straight to its
//! element.
//!
//! The pause rule stays one ([`crate::pace::pauses`]): the way's time is taken from the step's
//! pause - the step waits the rest of it, then moves -, so the run takes no longer and its budget
//! is the same; a way longer than the pause moves its last part without waits.

use crate::model::{PathPoint, RecordedBreakpoint, Step};
use crate::pace::Pace;
use crate::steps::Guard;

/// Points per line of a written way.
const POINTS_PER_LINE: usize = 8;

/// The most points of a way written: its last ones (the recorder keeps no more).
pub const MAX_POINTS: usize = 64;

/// A step's way as written: the pause left before it, and its lines as (depth below the step's
/// statement, text).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Way {
    /// The part of the step's pause waited before the way, in ms.
    pub before_ms: u64,
    /// The lines.
    pub lines: Vec<(usize, String)>,
}

/// The wait before each point of `points` - the recorded time since the point before -, the
/// running sum at most `budget_ms`.
#[must_use]
pub fn waits(points: &[PathPoint], budget_ms: u64) -> Vec<u64> {
    let mut left = budget_ms;
    let mut previous: Option<u32> = None;
    points
        .iter()
        .map(|point| {
            let gap = previous.map_or(0, |before| u64::from(point.t.saturating_sub(before)));
            previous = Some(point.t);
            let wait = gap.min(left);
            left -= wait;
            wait
        })
        .collect()
}

/// `step`'s way with the pause `pause_ms` before it; `None` without a way or a size to compare
/// with (a breakpoint the recording does not name).
#[must_use]
pub fn way(
    step: &Step,
    sizes: &[RecordedBreakpoint],
    guard: Guard,
    pace: Pace,
    pause_ms: u64,
) -> Option<Way> {
    let points = &step.path[step.path.len().saturating_sub(MAX_POINTS)..];
    if points.len() < 2 {
        return None;
    }
    let size = sizes.iter().find(|breakpoint| breakpoint.id == step.view)?;
    let (width, height) = (size.width, size.height);
    let check = match guard {
        Guard::Breakpoint => format!("view.width === {width} && view.height === {height}"),
        Guard::Project => format!(
            "page.viewportSize()?.width === {width} && page.viewportSize()?.height === {height}"
        ),
    };
    let timed = pace == Pace::Recorded;
    let budget = if timed { pause_ms } else { 0 };
    let waits = waits(points, budget);
    let used: u64 = waits.iter().sum();
    let items: Vec<String> = points
        .iter()
        .zip(&waits)
        .map(|(point, wait)| {
            if timed {
                format!("[{}, {}, {wait}]", point.x, point.y)
            } else {
                format!("[{}, {}]", point.x, point.y)
            }
        })
        .collect();
    let pattern = if timed { "[x, y, wait]" } else { "[x, y]" };
    let mut lines = vec![
        (
            0,
            format!(
                "// Pointer path: {} points, recorded at {width}x{height}.",
                points.len()
            ),
        ),
        (0, format!("if ({check}) {{")),
    ];
    let head = format!("for (const {pattern} of ");
    let tail = if timed {
        ") {".to_owned()
    } else {
        ") await page.mouse.move(x, y);".to_owned()
    };
    if items.len() <= POINTS_PER_LINE {
        lines.push((1, format!("{head}[{}]{tail}", items.join(", "))));
    } else {
        lines.push((1, format!("{head}[")));
        for chunk in items.chunks(POINTS_PER_LINE) {
            lines.push((2, format!("{},", chunk.join(", "))));
        }
        lines.push((1, format!("]{tail}")));
    }
    if timed {
        lines.push((
            2,
            "if (wait > 0) await page.waitForTimeout(wait);".to_owned(),
        ));
        lines.push((2, "await page.mouse.move(x, y);".to_owned()));
        lines.push((1, "}".to_owned()));
    }
    lines.push((0, "}".to_owned()));
    Some(Way {
        before_ms: pause_ms.saturating_sub(used),
        lines,
    })
}

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
