//! The model: guides, column setups per breakpoint, the baseline grid and what is shown.
//!
//! Two layers make the state the views draw ([`State`]): the **shared** one from the plugin's
//! settings ([`Shared`], set in Settings > DevTools & Debugging > Rulers, at project scope checked
//! in for the team) and the person's **overrides** ([`Overrides`]): what they dragged, typed or
//! set over MCP, kept in `.rbrowser/out/rulers-guides/state.json` (git-ignored). An override wins
//! per field - per breakpoint for the column setups.
//!
//! Every value that comes in (settings, the state file, a page, MCP) is held to the limits here
//! before it is used: finite numbers in range, at most [`MAX_GUIDES`] guides and
//! [`MAX_COLUMN_SETUPS`] setups, breakpoint ids of the config's alphabet, colours of
//! [`crate::colors::is_color`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::colors::{ColorOverrides, Colors};

/// The most guides the state holds.
pub const MAX_GUIDES: usize = 200;
/// The most column setups (breakpoints and `*`) the state holds.
pub const MAX_COLUMN_SETUPS: usize = 32;
/// The most columns one setup draws.
pub const MAX_COLUMNS: u32 = 48;
/// The largest length in CSS px a guide, gutter, margin or width may have.
pub const MAX_LENGTH: f64 = 100_000.0;
/// The smallest and largest baseline step in CSS px.
pub const BASELINE_STEP: (f64, f64) = (2.0, 1_000.0);
/// The setup for every breakpoint without its own.
pub const ANY_BREAKPOINT: &str = "*";

/// Which coordinate a guide fixes: `x` a vertical line, `y` a horizontal one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    X,
    Y,
}

/// What a guide's value counts: CSS px from the left (from the top for `y`), CSS px from the
/// right, or per cent of the page's width - for fluid layouts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GuideUnit {
    #[default]
    Px,
    PxRight,
    Percent,
}

/// One guide.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Guide {
    pub id: u32,
    pub axis: Axis,
    #[serde(default)]
    pub unit: GuideUnit,
    pub value: f64,
}

/// A column setup's template: the framework's own widths, gutter and margin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    Bootstrap,
    Tailwind,
}

/// Where the column box sits when the page is wider than its `maxWidth`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Left,
    #[default]
    Center,
    Right,
}

/// A column setup; a field left out comes from the preset, else the defaults
/// (`crate::columns::resolve`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColumnSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<Preset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gutter: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
}

/// The baseline grid: a line every `lineHeight` CSS px, starting at `offset`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Baseline {
    pub line_height: f64,
    #[serde(default)]
    pub offset: f64,
}

impl Default for Baseline {
    fn default() -> Self {
        Self {
            line_height: 8.0,
            offset: 0.0,
        }
    }
}

/// What the views show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Visible {
    pub rulers: bool,
    pub columns: bool,
    pub baseline: bool,
}

/// The person's choice of what to show; `None` leaves the shared value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VisibleOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rulers: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<bool>,
}

/// The shared layer, from the plugin's settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shared {
    pub visible: Visible,
    pub guides: Vec<Guide>,
    pub columns: BTreeMap<String, ColumnSpec>,
    pub baseline: Baseline,
    pub colors: Colors,
}

/// The person's layer: what they changed. A column setup of `null` switches the shared one of
/// that breakpoint off.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Overrides {
    pub visible: VisibleOverrides,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guides: Option<Vec<Guide>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub columns: BTreeMap<String, Option<ColumnSpec>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<Baseline>,
    #[serde(skip_serializing_if = "ColorOverrides::is_empty")]
    pub colors: ColorOverrides,
}

/// What the views draw: the shared layer with the overrides over it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub visible: Visible,
    pub guides: Vec<Guide>,
    pub columns: BTreeMap<String, ColumnSpec>,
    pub baseline: Baseline,
    #[serde(default)]
    pub colors: Colors,
}

/// Why a value is refused: the field, for the error's `reason`.
pub type Invalid = &'static str;

fn length(value: f64, field: Invalid) -> Result<(), Invalid> {
    if value.is_finite() && (0.0..=MAX_LENGTH).contains(&value) {
        Ok(())
    } else {
        Err(field)
    }
}

/// Whether `id` is a breakpoint id of the config (letters, digits, `_`, `-`, at most 64) or `*`.
#[must_use]
pub fn is_breakpoint_key(id: &str) -> bool {
    id == ANY_BREAKPOINT
        || (!id.is_empty()
            && id.len() <= 64
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
}

impl Guide {
    /// Holds the guide to its limits: a `y` guide counts px from the top only, a per cent lies
    /// in 0..=100, a length in 0..=[`MAX_LENGTH`].
    ///
    /// # Errors
    ///
    /// The field that is out of range.
    pub fn check(&self) -> Result<(), Invalid> {
        if self.axis == Axis::Y && self.unit != GuideUnit::Px {
            return Err("guide.unit");
        }
        match self.unit {
            GuideUnit::Percent
                if !(self.value.is_finite() && (0.0..=100.0).contains(&self.value)) =>
            {
                Err("guide.value")
            }
            GuideUnit::Percent => Ok(()),
            _ => length(self.value, "guide.value"),
        }
    }
}

impl ColumnSpec {
    /// # Errors
    ///
    /// The field that is out of range.
    pub fn check(&self) -> Result<(), Invalid> {
        if let Some(columns) = self.columns
            && !(1..=MAX_COLUMNS).contains(&columns)
        {
            return Err("columns.columns");
        }
        if let Some(gutter) = self.gutter {
            length(gutter, "columns.gutter")?;
        }
        if let Some(margin) = self.margin {
            length(margin, "columns.margin")?;
        }
        if let Some(width) = self.max_width {
            length(width, "columns.maxWidth")?;
            if width <= 0.0 {
                return Err("columns.maxWidth");
            }
        }
        Ok(())
    }
}

impl Baseline {
    /// # Errors
    ///
    /// The field that is out of range.
    pub fn check(&self) -> Result<(), Invalid> {
        let (low, high) = BASELINE_STEP;
        if !(self.line_height.is_finite() && (low..=high).contains(&self.line_height)) {
            return Err("baseline.lineHeight");
        }
        length(self.offset, "baseline.offset")
    }
}

/// Checks a list of guides: each one, their number and that no id repeats.
///
/// # Errors
///
/// The field that is out of range.
pub fn check_guides(guides: &[Guide]) -> Result<(), Invalid> {
    if guides.len() > MAX_GUIDES {
        return Err("guides");
    }
    let mut ids = std::collections::BTreeSet::new();
    for guide in guides {
        guide.check()?;
        if !ids.insert(guide.id) {
            return Err("guide.id");
        }
    }
    Ok(())
}

fn check_setups<'a>(
    setups: impl Iterator<Item = (&'a String, Option<&'a ColumnSpec>)>,
) -> Result<(), Invalid> {
    let mut count = 0;
    for (breakpoint, spec) in setups {
        count += 1;
        if count > MAX_COLUMN_SETUPS {
            return Err("columns");
        }
        if !is_breakpoint_key(breakpoint) {
            return Err("columns.breakpoint");
        }
        if let Some(spec) = spec {
            spec.check()?;
        }
    }
    Ok(())
}

impl Overrides {
    /// # Errors
    ///
    /// The field that is out of range.
    pub fn check(&self) -> Result<(), Invalid> {
        if let Some(guides) = &self.guides {
            check_guides(guides)?;
        }
        if let Some(baseline) = &self.baseline {
            baseline.check()?;
        }
        self.colors.check()?;
        check_setups(
            self.columns
                .iter()
                .map(|(breakpoint, spec)| (breakpoint, spec.as_ref())),
        )
    }
}

/// Checks the column setups of the shared layer.
///
/// # Errors
///
/// The field that is out of range.
pub fn check_columns(columns: &BTreeMap<String, ColumnSpec>) -> Result<(), Invalid> {
    check_setups(
        columns
            .iter()
            .map(|(breakpoint, spec)| (breakpoint, Some(spec))),
    )
}

/// The state the views draw: `shared` with `overrides` over it.
#[must_use]
pub fn effective(shared: &Shared, overrides: &Overrides) -> State {
    let mut columns = shared.columns.clone();
    for (breakpoint, spec) in &overrides.columns {
        match spec {
            Some(spec) => {
                columns.insert(breakpoint.clone(), spec.clone());
            }
            None => {
                columns.remove(breakpoint);
            }
        }
    }
    let visible = Visible {
        rulers: overrides.visible.rulers.unwrap_or(shared.visible.rulers),
        columns: overrides.visible.columns.unwrap_or(shared.visible.columns),
        baseline: overrides
            .visible
            .baseline
            .unwrap_or(shared.visible.baseline),
    };
    State {
        visible,
        guides: overrides
            .guides
            .clone()
            .unwrap_or_else(|| shared.guides.clone()),
        columns,
        baseline: overrides.baseline.unwrap_or(shared.baseline),
        colors: overrides.colors.over(&shared.colors),
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
