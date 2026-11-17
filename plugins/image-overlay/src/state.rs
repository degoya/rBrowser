//! The overlay's state: one for every view (each view shows its own image with it).
//!
//! Two layers, as the rulers keep theirs: the **shared** layer is the plugin's settings
//! (`image-overlay.opacity`, `blend`, `invert`, `anchor`, `autoScaleDpr`, `designs`), set at
//! project scope for the team; the person's **overrides** - what the panel, the page's split
//! handle, the command and MCP changed - lie in `.rbrowser/out/image-overlay/state.json` and win.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::path::{DesignPath, breakpoint_id_ok};

/// The largest offset either way, in CSS px.
pub const MAX_OFFSET: f64 = 100_000.0;
/// The most breakpoints the design mapping names.
pub const MAX_DESIGNS: usize = 64;
/// The opacity without the setting, in per cent.
pub const DEFAULT_OPACITY: u8 = 50;

/// How the design mixes with the page (CSS `mix-blend-mode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Blend {
    #[default]
    Normal,
    Difference,
    Multiply,
    Screen,
    Overlay,
    Exclusion,
}

/// Whether the design scrolls with the page or stays in the viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Anchor {
    #[default]
    Document,
    Viewport,
}

/// The design's offset from the page's (or the viewport's) top left corner, in CSS px.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offset {
    pub x: f64,
    pub y: f64,
}

/// The split view: the design left of `position` (per cent of the view's width), the page right.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Split {
    pub on: bool,
    pub position: f64,
}

impl Default for Split {
    fn default() -> Self {
        Self {
            on: false,
            position: 50.0,
        }
    }
}

/// The effective state, as the page scripts and the panel get it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub enabled: bool,
    pub opacity: u8,
    pub blend: Blend,
    pub invert: bool,
    pub anchor: Anchor,
    pub offset: Offset,
    pub split: Split,
    pub auto_scale_dpr: bool,
    /// Breakpoint id -> design path relative to `.rbrowser/` (checked where it is used).
    pub designs: BTreeMap<String, String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            enabled: false,
            opacity: DEFAULT_OPACITY,
            blend: Blend::Normal,
            invert: false,
            anchor: Anchor::Document,
            offset: Offset::default(),
            split: Split::default(),
            auto_scale_dpr: true,
            designs: BTreeMap::new(),
        }
    }
}

/// The shared layer: the plugin's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Shared {
    pub opacity: u8,
    pub blend: Blend,
    pub invert: bool,
    pub anchor: Anchor,
    pub auto_scale_dpr: bool,
    pub designs: BTreeMap<String, String>,
}

impl Default for Shared {
    fn default() -> Self {
        let state = State::default();
        Self {
            opacity: state.opacity,
            blend: state.blend,
            invert: state.invert,
            anchor: state.anchor,
            auto_scale_dpr: state.auto_scale_dpr,
            designs: state.designs,
        }
    }
}

/// The person's overrides; `None` leaves the shared value. A design set to `null` drops the
/// shared mapping of that breakpoint (the nearest image is taken).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Overrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blend: Option<Blend>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invert: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<Anchor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<Offset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split: Option<Split>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub designs: BTreeMap<String, Option<String>>,
}

/// Whether `value` is a usable offset coordinate.
#[must_use]
pub fn offset_ok(value: f64) -> bool {
    value.is_finite() && value.abs() <= MAX_OFFSET
}

/// Whether `value` is a usable split position.
#[must_use]
pub fn position_ok(value: f64) -> bool {
    value.is_finite() && (0.0..=100.0).contains(&value)
}

impl Overrides {
    /// Every value in range.
    ///
    /// # Errors
    ///
    /// The name of the first field out of range.
    pub fn check(&self) -> Result<(), &'static str> {
        if self.opacity.is_some_and(|opacity| opacity > 100) {
            return Err("opacity");
        }
        if self
            .offset
            .is_some_and(|offset| !offset_ok(offset.x) || !offset_ok(offset.y))
        {
            return Err("offset");
        }
        if self.split.is_some_and(|split| !position_ok(split.position)) {
            return Err("split");
        }
        let designs_ok = self.designs.len() <= MAX_DESIGNS
            && self.designs.iter().all(|(id, path)| {
                breakpoint_id_ok(id)
                    && path
                        .as_deref()
                        .is_none_or(|path| DesignPath::parse(path).is_ok())
            });
        if designs_ok { Ok(()) } else { Err("designs") }
    }
}

/// The state the views show: `shared` with `overrides` over it.
#[must_use]
pub fn effective(shared: &Shared, overrides: &Overrides) -> State {
    let mut designs = shared.designs.clone();
    for (id, path) in &overrides.designs {
        match path {
            Some(path) => designs.insert(id.clone(), path.clone()),
            None => designs.remove(id),
        };
    }
    State {
        enabled: overrides.enabled.unwrap_or(false),
        opacity: overrides.opacity.unwrap_or(shared.opacity),
        blend: overrides.blend.unwrap_or(shared.blend),
        invert: overrides.invert.unwrap_or(shared.invert),
        anchor: overrides.anchor.unwrap_or(shared.anchor),
        offset: overrides.offset.unwrap_or_default(),
        split: overrides.split.unwrap_or_default(),
        auto_scale_dpr: shared.auto_scale_dpr,
        designs,
    }
}

/// Whether a change from `before` to `after` makes the views ask for their image again: the
/// overlay switched on (the file may have changed meanwhile), the mapping or the scaling changed.
#[must_use]
pub fn reload(before: &State, after: &State) -> bool {
    (after.enabled && !before.enabled)
        || before.designs != after.designs
        || before.auto_scale_dpr != after.auto_scale_dpr
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
