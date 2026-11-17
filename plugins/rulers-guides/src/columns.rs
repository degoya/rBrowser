//! The column overlay: a breakpoint's setup resolved against the page's width.
//!
//! The model is a framework's container and row: a box of the page's width, at most `maxWidth`
//! wide and placed by `align`; inside it `margin` on either side; across the rest `columns`
//! columns with `gutter` between them. Bootstrap's `.container` (padding 12 px) with a `.row` of
//! `.col-*` (gutter 24 px) is exactly `{columns: 12, gutter: 24, margin: 12, maxWidth: <the
//! container's width>}`: the columns drawn are the columns' content boxes.
//!
//! The presets carry the frameworks' default container widths, which follow the viewport width
//! (the media queries' width, scroll bar included); the box follows the page's client width.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Value, json};

use crate::model::{ANY_BREAKPOINT, Align, ColumnSpec, Preset};

/// Columns of a setup that names neither a preset nor its own number.
pub const DEFAULT_COLUMNS: u32 = 12;
/// The gutter of a setup that names neither a preset nor its own gutter.
pub const DEFAULT_GUTTER: f64 = 24.0;

/// Bootstrap 5's container widths: (from viewport width, container width).
const BOOTSTRAP_CONTAINERS: [(f64, f64); 5] = [
    (576.0, 540.0),
    (768.0, 720.0),
    (992.0, 960.0),
    (1200.0, 1140.0),
    (1400.0, 1320.0),
];
/// Tailwind's `container` widths: the breakpoint is the width.
const TAILWIND_CONTAINERS: [(f64, f64); 5] = [
    (640.0, 640.0),
    (768.0, 768.0),
    (1024.0, 1024.0),
    (1280.0, 1280.0),
    (1536.0, 1536.0),
];

/// A setup with every field decided.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolved {
    pub columns: u32,
    pub gutter: f64,
    pub margin: f64,
    pub max_width: Option<f64>,
    pub align: Align,
}

/// One column in page CSS px.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Column {
    pub x: f64,
    pub width: f64,
}

/// The column box and its columns in page CSS px.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ColumnLayout {
    pub left: f64,
    pub width: f64,
    pub columns: Vec<Column>,
}

/// The preset's container width at `viewport_width`; `None` below its first breakpoint.
fn container(table: &[(f64, f64)], viewport_width: f64) -> Option<f64> {
    table
        .iter()
        .rev()
        .find(|(from, _)| viewport_width >= *from)
        .map(|(_, width)| *width)
}

/// A template's columns, gutter, margin and container table (none without a template).
fn template(preset: Option<Preset>) -> (u32, f64, f64, Option<&'static [(f64, f64)]>) {
    match preset {
        Some(Preset::Bootstrap) => (12, 24.0, 12.0, Some(&BOOTSTRAP_CONTAINERS)),
        Some(Preset::Tailwind) => (12, 24.0, 16.0, Some(&TAILWIND_CONTAINERS)),
        None => (DEFAULT_COLUMNS, DEFAULT_GUTTER, 0.0, None),
    }
}

/// The templates' values for the panel's placeholders (RB-080-66), keyed by the preset's name
/// (`""` without one): `columns`, `gutter`, `margin` and `maxWidth` - the smallest and largest
/// container width, `null` for the full width.
#[must_use]
pub fn templates() -> Value {
    let entry = |preset: Option<Preset>| {
        let (columns, gutter, margin, table) = template(preset);
        let max_width = table.and_then(|table| Some((table.first()?.1, table.last()?.1)));
        json!({
            "columns": columns,
            "gutter": gutter,
            "margin": margin,
            "maxWidth": max_width.map(|(min, max)| json!({ "min": min, "max": max })),
        })
    };
    json!({
        "": entry(None),
        "bootstrap": entry(Some(Preset::Bootstrap)),
        "tailwind": entry(Some(Preset::Tailwind)),
    })
}

/// `spec` with every field decided at `viewport_width`: its own fields, then the preset's, then
/// the defaults.
#[must_use]
pub fn resolve(spec: &ColumnSpec, viewport_width: f64) -> Resolved {
    let (columns, gutter, margin, table) = template(spec.preset);
    let max_width = table.and_then(|table| container(table, viewport_width));
    Resolved {
        columns: spec.columns.unwrap_or(columns),
        gutter: spec.gutter.unwrap_or(gutter),
        margin: spec.margin.unwrap_or(margin),
        max_width: spec.max_width.or(max_width),
        align: spec.align.unwrap_or_default(),
    }
}

/// Rounded to a thousandth of a CSS px: exact for the halves and quarters layouts use, and
/// short in JSON.
fn round(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// The columns of `resolved` in a page `client_width` CSS px wide.
#[must_use]
pub fn layout(resolved: &Resolved, client_width: f64) -> ColumnLayout {
    let client_width = client_width.max(0.0);
    let width = resolved
        .max_width
        .map_or(client_width, |max| max.min(client_width));
    let left = match resolved.align {
        Align::Left => 0.0,
        Align::Center => (client_width - width) / 2.0,
        Align::Right => client_width - width,
    };
    let count = resolved.columns.max(1);
    let inner = (width - 2.0 * resolved.margin).max(0.0);
    let gutters = f64::from(count - 1) * resolved.gutter;
    let column_width = ((inner - gutters) / f64::from(count)).max(0.0);
    let columns = (0..count)
        .map(|index| Column {
            x: round(left + resolved.margin + f64::from(index) * (column_width + resolved.gutter)),
            width: round(column_width),
        })
        .collect();
    ColumnLayout {
        left: round(left),
        width: round(width),
        columns,
    }
}

/// The setup of `breakpoint`, else the one for every breakpoint (`*`).
#[must_use]
pub fn spec_for<'a>(
    setups: &'a BTreeMap<String, ColumnSpec>,
    breakpoint: Option<&str>,
) -> Option<&'a ColumnSpec> {
    breakpoint
        .and_then(|breakpoint| setups.get(breakpoint))
        .or_else(|| setups.get(ANY_BREAKPOINT))
}

#[cfg(test)]
#[path = "columns_tests.rs"]
mod tests;
