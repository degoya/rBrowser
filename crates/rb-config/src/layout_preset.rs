//! A named layout preset (`layouts.presets.<id>`). Its `mode` is the canvas when absent
//! (RB-099-26) - except for a preset with a grid of its own (`layout`), which was a grid preset
//! by the built-in mode before and stores a grid in effect: it stays one (owner: a project that
//! stores a mode keeps it). rBrowser writes `mode` with every preset.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{Layout, LayoutMode, ModeFields};

/// A named layout preset.
#[derive(Clone, Debug, Default, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LayoutPreset {
    /// Grid, focus, horizontal, split or canvas; when absent the grid for a preset with its own
    /// `layout`, else the canvas (RB-099-26).
    #[serde(default)]
    pub mode: LayoutMode,
    /// The grid of this preset; the top-level `layout` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<Layout>,
    /// Focus mode: the breakpoint shown first; the first breakpoint when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
    /// The fields of the horizontal, split and canvas modes (RB-080-04).
    #[serde(flatten)]
    pub modes: ModeFields,
    /// The breakpoints whose views are not open while this preset is shown (RB-080-41):
    /// choosing the preset writes it as `layout.hidden`; absent, the preset leaves the
    /// selection as it is. `[]` opens every view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<Vec<String>>,
}

/// [`LayoutPreset`] as a file has it: `mode` may be absent.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PresetFile {
    #[serde(default)]
    mode: Option<LayoutMode>,
    #[serde(default)]
    layout: Option<Layout>,
    #[serde(default)]
    focus: Option<String>,
    #[serde(flatten)]
    modes: ModeFields,
    #[serde(default)]
    hidden: Option<Vec<String>>,
}

impl<'de> Deserialize<'de> for LayoutPreset {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let file = PresetFile::deserialize(deserializer)?;
        let mode = file.mode.unwrap_or(if file.layout.is_some() {
            LayoutMode::Grid
        } else {
            LayoutMode::default()
        });
        Ok(Self {
            mode,
            layout: file.layout,
            focus: file.focus,
            modes: file.modes,
            hidden: file.hidden,
        })
    }
}

#[cfg(test)]
#[path = "layout_preset_tests.rs"]
mod tests;
