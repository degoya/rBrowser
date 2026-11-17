//! The canvas zoom's presets as rows without a window (RB-090-18): Fit, then 25 to 200 %, the
//! row of the canvas's level checked. The status bar's level button asks for them
//! (`canvasZoomMenu`, `layout::view_menu_open`) and `cef_app/zoom_menu.rs` shows them as a
//! native menu above the bar - the `<select>` they were opened its list below the bar, off the
//! screen with the bar at its bottom. A row runs `layout.zoom_to_fit` or `canvas.zoom.reset
//! { level }` of the one registry, off the calling thread ([`run`]).

use serde_json::{Value, json};

use super::canvas_level::RESET;
use crate::control::Core;

/// Every view in sight: the one zoom-to-fit command (RB-090-63).
const ZOOM_TO_FIT: &str = "layout.zoom_to_fit";

/// The presets after Fit, in per cent; among `canvas_level::STEPS`.
pub const PRESETS: [u16; 6] = [25, 50, 75, 100, 150, 200];
/// The i18n keys of the rows: Fit, and a level (`{percent}`), as the status bar's.
pub const KEY_FIT: &str = "status.canvas_zoom.fit";
pub const KEY_LEVEL: &str = "status.canvas_zoom.level";

/// What a row does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ZoomAction {
    /// `layout.zoom_to_fit`: every view in sight.
    Fit,
    /// `canvas.zoom.reset { level }`, 1 for 100 %.
    Level(f64),
}

/// One row of the menu.
#[derive(Clone, Debug, PartialEq)]
pub struct ZoomRow {
    /// Its text.
    pub label: String,
    /// What it does.
    pub action: ZoomAction,
    /// The canvas stands at this level.
    pub checked: bool,
}

/// The rows, the one at `level` (the canvas's, 1 for 100 %) checked; Fit never is.
pub fn rows(level: Option<f64>) -> Vec<ZoomRow> {
    // `canvas_level::current` rounds to a tenth of a per cent; half of it decides.
    const SLACK_PERCENT: f64 = 0.05;
    let fit = ZoomRow {
        label: rb_i18n::t(KEY_FIT),
        action: ZoomAction::Fit,
        checked: false,
    };
    let presets = PRESETS.iter().map(|percent| {
        let percent = f64::from(*percent);
        ZoomRow {
            label: rb_i18n::t_with(KEY_LEVEL, &[("percent", &percent.to_string())]),
            action: ZoomAction::Level(percent / 100.0),
            checked: level.is_some_and(|level| (level * 100.0 - percent).abs() < SLACK_PERCENT),
        }
    });
    std::iter::once(fit).chain(presets).collect()
}

/// The command a row runs and its arguments.
pub fn command(action: ZoomAction) -> (&'static str, Value) {
    match action {
        ZoomAction::Fit => (ZOOM_TO_FIT, Value::Null),
        ZoomAction::Level(level) => (RESET, json!({ "level": level })),
    }
}

/// Runs the row's command off the calling thread (the menu's, the UI thread).
pub fn run(core: &Core, action: ZoomAction) {
    let (id, args) = command(action);
    core.run_command_async(id, args, move |result| {
        if let Err(error) = result {
            tracing::warn!(command = id, %error, "a zoom preset failed");
        }
    });
}

#[cfg(test)]
#[path = "zoom_menu_tests.rs"]
mod tests;
