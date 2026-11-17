//! The fields of the layout modes `horizontal`, `split` and `canvas` (RB-080-04), flattened into
//! a [`crate::LayoutPreset`] beside its `mode`:
//!
//! - `{ "mode": "horizontal", "height": "fit" | <px>, "gap": 8 }`
//! - `{ "mode": "split", "orientation": "vertical" | "horizontal", "ratio": 0.5,
//!   "primary": ["<breakpoint>"], "secondary": ["<breakpoint>", ...] }`
//! - `{ "mode": "canvas", "snap": 8, "items": [{ "breakpoint": "...", "x": 0, "y": 0,
//!   "zoom": 0.5 }] }`
//!
//! Every field is optional and written only when set, so a preset of 0.1 loads and saves as it
//! was: the config `version` stays 1 (no migration step). The rules a schema cannot say are
//! [`check_modes`] and, for the hidden selection of `layout.hidden` and a preset's `hidden`
//! (RB-080-41), [`check_hidden`].

use std::collections::HashSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ConfigIssue, EngineKind, Layout, LayoutMode, ProjectConfig};

/// The smallest and largest split ratio: neither side may vanish.
pub const SPLIT_RATIO_RANGE: (f64, f64) = (0.1, 0.9);
/// The range of a canvas item's zoom.
pub const CANVAS_ZOOM_RANGE: (f64, f64) = (0.1, 1.0);
/// The largest canvas snap and horizontal gap, in DIP.
pub const MAX_SNAP: u32 = 128;
/// The smallest and largest fixed row height of the horizontal mode, in DIP.
pub const STRIP_HEIGHT_RANGE: (u32, u32) = (100, 10_000);
/// The largest coordinate of a canvas item, in either direction.
pub const MAX_CANVAS_COORD: i32 = 100_000;

/// The mode fields of a preset; each applies to its mode and is ignored by the others.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModeFields {
    /// Horizontal: the row height, `"fit"` (the grid area's) or a height in DIP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<StripHeight>,
    /// Horizontal: the gap between the views in DIP; the grid's `gap` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<u32>,
    /// Split: a `vertical` divider (primary left) or a `horizontal` one (primary on top).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<SplitOrientation>,
    /// Split: the primary side's share of the area, `0.1`..=`0.9`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ratio: Option<f64>,
    /// Split: the breakpoints of the primary side; the active view's when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub primary: Vec<String>,
    /// Split: the breakpoints of the other side, stacked; every other one when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secondary: Vec<String>,
    /// Canvas: the grid a moved view snaps to in DIP, `0` for none; the user setting
    /// `layoutModes.snap` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snap: Option<u32>,
    /// Canvas: where each view sits; a breakpoint without an item is placed after the others.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<CanvasItem>,
}

/// `layout.modes`: the fields of the horizontal, split and canvas modes shown over the
/// top-level `layout` (or over a preset of another mode) - the divider's ratio, the canvas
/// positions, the strip's height. rBrowser writes them here instead of creating a preset per
/// mode (owner, 2026-11-12); a preset of the mode keeps its own fields.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LayoutModes {
    /// The horizontal mode's fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizontal: Option<ModeFields>,
    /// The split mode's fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split: Option<ModeFields>,
    /// The canvas mode's fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas: Option<ModeFields>,
}

impl LayoutModes {
    /// The fields of `mode`; `None` for grid and focus or when none are kept.
    pub fn get(&self, mode: LayoutMode) -> Option<&ModeFields> {
        match mode {
            LayoutMode::Horizontal => self.horizontal.as_ref(),
            LayoutMode::Split => self.split.as_ref(),
            LayoutMode::Canvas => self.canvas.as_ref(),
            LayoutMode::Grid | LayoutMode::Focus => None,
        }
    }

    /// Keeps `fields` for `mode`; `false` for grid and focus, which have none.
    pub fn set(&mut self, mode: LayoutMode, fields: ModeFields) -> bool {
        let slot = match mode {
            LayoutMode::Horizontal => &mut self.horizontal,
            LayoutMode::Split => &mut self.split,
            LayoutMode::Canvas => &mut self.canvas,
            LayoutMode::Grid | LayoutMode::Focus => return false,
        };
        *slot = Some(fields);
        true
    }

    /// Whether no mode keeps fields (then `layout.modes` is not written).
    pub fn is_empty(&self) -> bool {
        self.horizontal.is_none() && self.split.is_none() && self.canvas.is_none()
    }

    /// Each kept mode's fields with its config name.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &ModeFields)> {
        [
            ("horizontal", &self.horizontal),
            ("split", &self.split),
            ("canvas", &self.canvas),
        ]
        .into_iter()
        .filter_map(|(name, fields)| fields.as_ref().map(|fields| (name, fields)))
    }
}

/// The row height of the horizontal mode: `"fit"` or a height in DIP.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(from = "StripHeightRepr", into = "StripHeightRepr")]
pub enum StripHeight {
    /// The grid area's height.
    #[default]
    Fit,
    /// A fixed height in DIP (the area's at most).
    Px(u32),
}

#[derive(Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
enum StripHeightRepr {
    Keyword(FitKeyword),
    Px(u32),
}

#[derive(Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum FitKeyword {
    Fit,
}

impl From<StripHeightRepr> for StripHeight {
    fn from(repr: StripHeightRepr) -> Self {
        match repr {
            StripHeightRepr::Keyword(FitKeyword::Fit) => StripHeight::Fit,
            StripHeightRepr::Px(px) => StripHeight::Px(px),
        }
    }
}

impl From<StripHeight> for StripHeightRepr {
    fn from(height: StripHeight) -> Self {
        match height {
            StripHeight::Fit => StripHeightRepr::Keyword(FitKeyword::Fit),
            StripHeight::Px(px) => StripHeightRepr::Px(px),
        }
    }
}

/// The divider of the split mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SplitOrientation {
    /// A vertical divider: the primary side left, the others stacked on the right.
    #[default]
    Vertical,
    /// A horizontal divider: the primary side on top, the others side by side below.
    Horizontal,
}

/// One view of the canvas: its breakpoint, the top-left corner of the view (its label strip
/// sits in the gap above) in canvas DIP, and its zoom.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CanvasItem {
    /// The breakpoint id.
    pub breakpoint: String,
    /// Left edge in canvas DIP.
    pub x: i32,
    /// Top edge in canvas DIP.
    pub y: i32,
    /// The view's zoom, `0.1`..=`1`.
    #[serde(default = "default_item_zoom")]
    pub zoom: f64,
}

/// The zoom of a canvas item without one.
pub const fn default_item_zoom() -> f64 {
    0.5
}

/// The rules of the mode fields of the preset at `at` (a JSON pointer), against the breakpoint
/// ids `ids`.
pub fn check_modes(fields: &ModeFields, at: &str, ids: &HashSet<&str>) -> Vec<ConfigIssue> {
    let mut issues = Vec::new();
    if let Some(StripHeight::Px(px)) = fields.height
        && !(STRIP_HEIGHT_RANGE.0..=STRIP_HEIGHT_RANGE.1).contains(&px)
    {
        issues.push(issue(
            format!("{at}/height"),
            "config.error.height_range",
            format!(
                "height {px} is outside {} to {}",
                STRIP_HEIGHT_RANGE.0, STRIP_HEIGHT_RANGE.1
            ),
        ));
    }
    for (field, value) in [("gap", fields.gap), ("snap", fields.snap)] {
        if let Some(value) = value
            && value > MAX_SNAP
        {
            issues.push(issue(
                format!("{at}/{field}"),
                "config.error.snap_range",
                format!("{field} {value} is larger than {MAX_SNAP}"),
            ));
        }
    }
    if let Some(ratio) = fields.ratio
        && !(SPLIT_RATIO_RANGE.0..=SPLIT_RATIO_RANGE.1).contains(&ratio)
    {
        issues.push(issue(
            format!("{at}/ratio"),
            "config.error.ratio_range",
            format!(
                "ratio {ratio} is outside {} to {}",
                SPLIT_RATIO_RANGE.0, SPLIT_RATIO_RANGE.1
            ),
        ));
    }
    let mut placed = HashSet::new();
    for (side, list) in [
        ("primary", &fields.primary),
        ("secondary", &fields.secondary),
    ] {
        for (index, breakpoint) in list.iter().enumerate() {
            let path = format!("{at}/{side}/{index}");
            known(breakpoint, &path, ids, &mut issues);
            if !placed.insert(breakpoint.as_str()) {
                issues.push(duplicate(path, breakpoint));
            }
        }
    }
    let mut on_canvas = HashSet::new();
    for (index, item) in fields.items.iter().enumerate() {
        let path = format!("{at}/items/{index}");
        known(
            &item.breakpoint,
            &format!("{path}/breakpoint"),
            ids,
            &mut issues,
        );
        if !on_canvas.insert(item.breakpoint.as_str()) {
            issues.push(duplicate(format!("{path}/breakpoint"), &item.breakpoint));
        }
        if !(CANVAS_ZOOM_RANGE.0..=CANVAS_ZOOM_RANGE.1).contains(&item.zoom) {
            issues.push(issue(
                format!("{path}/zoom"),
                "config.error.zoom_range",
                format!(
                    "zoom {} is outside {} to {}",
                    item.zoom, CANVAS_ZOOM_RANGE.0, CANVAS_ZOOM_RANGE.1
                ),
            ));
        }
        if item.x.abs() > MAX_CANVAS_COORD || item.y.abs() > MAX_CANVAS_COORD {
            issues.push(issue(
                path,
                "config.error.canvas_range",
                format!("x and y lie within -{MAX_CANVAS_COORD} to {MAX_CANVAS_COORD}"),
            ));
        }
    }
    issues
}

/// The rules of a hidden selection (`layout.hidden`, a preset's `hidden`, RB-080-41) at `at`:
/// every id names a breakpoint of `ids`, none twice.
pub fn check_hidden(hidden: &[String], at: &str, ids: &HashSet<&str>) -> Vec<ConfigIssue> {
    let mut issues = Vec::new();
    let mut seen = HashSet::new();
    for (index, breakpoint) in hidden.iter().enumerate() {
        let path = format!("{at}/{index}");
        known(breakpoint, &path, ids, &mut issues);
        if !seen.insert(breakpoint.as_str()) {
            issues.push(duplicate(path, breakpoint));
        }
    }
    issues
}

/// The ids of `hidden` that name a breakpoint of `ids`, each once, in order: the selection a
/// config with a stale entry (a breakpoint since removed) still yields.
pub fn hidden_of(hidden: &[String], ids: &HashSet<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in hidden {
        if ids.contains(id.as_str()) && !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}

/// The ids of `hidden` that name a breakpoint of `config`, each once, in order ([`hidden_of`]).
pub fn known_hidden(config: &ProjectConfig, hidden: &[String]) -> Vec<String> {
    let ids: HashSet<&str> = config
        .breakpoints
        .iter()
        .map(|breakpoint| breakpoint.id.as_str())
        .collect();
    hidden_of(hidden, &ids)
}

/// Whether `hidden` leaves a Chromium breakpoint of `config` open in the config's own grid
/// ([`keeps_a_chromium_view_in`] with `config.layout`).
pub fn keeps_a_chromium_view(config: &ProjectConfig, hidden: &[String]) -> bool {
    keeps_a_chromium_view_in(config, &config.layout, hidden)
}

/// Whether `hidden` leaves a Chromium breakpoint of `config` open in the grid `layout`: a grid
/// with cells shows only those, a computed one (no cells) every Chromium breakpoint.
pub fn keeps_a_chromium_view_in(
    config: &ProjectConfig,
    layout: &Layout,
    hidden: &[String],
) -> bool {
    let open = |id: &str| {
        config
            .breakpoints
            .iter()
            .any(|breakpoint| breakpoint.id == id && breakpoint.engine == EngineKind::Chromium)
            && !hidden.iter().any(|entry| entry == id)
    };
    if layout.cells.is_empty() {
        config
            .breakpoints
            .iter()
            .any(|breakpoint| open(&breakpoint.id))
    } else {
        layout.cells.iter().any(|cell| open(&cell.breakpoint))
    }
}

/// The selection in effect for `config` in its own grid ([`hidden_selection_in`] with
/// `config.layout`).
pub fn hidden_selection(config: &ProjectConfig) -> Vec<String> {
    hidden_selection_in(config, &config.layout)
}

/// The selection in effect for `config` in the grid `layout` (RB-080-41): [`known_hidden`] of
/// its `layout.hidden`, unless that would leave the grid without a Chromium view - then nothing
/// is hidden, so a selection edited by hand never shows an empty window.
pub fn hidden_selection_in(config: &ProjectConfig, layout: &Layout) -> Vec<String> {
    let hidden = known_hidden(config, &config.layout.hidden);
    if keeps_a_chromium_view_in(config, layout, &hidden) || config.breakpoints.is_empty() {
        hidden
    } else {
        Vec::new()
    }
}

fn known(breakpoint: &str, path: &str, ids: &HashSet<&str>, issues: &mut Vec<ConfigIssue>) {
    if !ids.contains(breakpoint) {
        issues.push(issue(
            path.to_owned(),
            "config.error.unknown_breakpoint",
            format!("breakpoint `{breakpoint}` does not exist"),
        ));
    }
}

fn duplicate(path: String, breakpoint: &str) -> ConfigIssue {
    issue(
        path,
        "config.error.duplicate_breakpoint",
        format!("breakpoint `{breakpoint}` is listed more than once"),
    )
}

fn issue(path: String, key: &str, message: String) -> ConfigIssue {
    ConfigIssue {
        path,
        key: key.to_owned(),
        message,
    }
}

#[cfg(test)]
#[path = "layout_modes_tests.rs"]
mod tests;
