//! The MCP tool `rulers_set` (and the panel's one call): sets guides, column setups, the baseline
//! grid and what is shown, and answers the state.
//!
//! Arguments, each optional - none answers the state unchanged:
//!
//! - `guides`: `[{axis: "x" | "y", unit?: "px" | "px-right" | "percent", value}]` replaces every
//!   guide (ids are given anew, from 1);
//! - `columns`: `{<breakpoint id> | "*": ColumnSpec | null}` - a setup per breakpoint, `null`
//!   switching one off - or one `ColumnSpec` (`preset`, `columns`, `gutter`, `margin`,
//!   `maxWidth`, `align`) for the breakpoint of `target` when it is a view, else for `*`;
//! - `baseline`: `{lineHeight, offset?}`;
//! - `visible`: `true` / `false` for the rulers, or `{rulers?, columns?, baseline?}`;
//! - `colors`: `{ruler?, ticks?, guide?, columnFill?, columnEdge?, baseline?}`, each `#rrggbb` or
//!   `#rrggbbaa` (RB-090-58); the others stay;
//! - `reset`: `true` drops the person's overrides first, back to the shared settings;
//! - `target`: `"all"` or a view id (the host's MCP layer resolves a breakpoint id to its views).
//!
//! The answer: `{state, saved?, breakpoints, breakpoint?, templates}` - the effective state,
//! whether the overrides file was written (after a change), the open views' breakpoints,
//! `target`'s, and the templates' values ([`crate::columns::templates`], the panel's
//! placeholders).

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, PluginHost};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::colors::ColorOverrides;
use crate::layout::{breakpoint_of, breakpoints};
use crate::model::{
    ANY_BREAKPOINT, Axis, Baseline, ColumnSpec, Guide, GuideUnit, MAX_GUIDES, Overrides,
    VisibleOverrides, check_guides,
};
use crate::{bad_args, changed, store};

/// The fields of a single column setup: an object with any of them is one setup, not a map.
const SPEC_FIELDS: [&str; 6] = ["preset", "columns", "gutter", "margin", "maxWidth", "align"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GuideInput {
    axis: Axis,
    #[serde(default)]
    unit: GuideUnit,
    value: f64,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum VisibleInput {
    Rulers(bool),
    Each(VisibleOverrides),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetArgs {
    #[serde(default)]
    guides: Option<Vec<GuideInput>>,
    #[serde(default)]
    columns: Option<Value>,
    #[serde(default)]
    baseline: Option<Baseline>,
    #[serde(default)]
    visible: Option<VisibleInput>,
    #[serde(default)]
    colors: Option<ColorOverrides>,
    #[serde(default)]
    reset: bool,
    #[serde(default)]
    target: Option<Value>,
}

/// The view `target` names, if it names one.
fn target_view(target: Option<&Value>) -> Result<Option<u32>, Failure> {
    match target {
        None => Ok(None),
        Some(Value::String(all)) if all == "all" => Ok(None),
        Some(Value::Number(view)) => view
            .as_u64()
            .and_then(|view| u32::try_from(view).ok())
            .map(Some)
            .ok_or_else(|| bad_args("target")),
        Some(_) => Err(bad_args("target")),
    }
}

fn column_setups(
    host: &mut dyn PluginHost,
    columns: Value,
    view: Option<u32>,
) -> Result<BTreeMap<String, Option<ColumnSpec>>, Failure> {
    let object = columns.as_object().ok_or_else(|| bad_args("columns"))?;
    if SPEC_FIELDS.iter().any(|field| object.contains_key(*field)) {
        let spec: ColumnSpec = serde_json::from_value(columns).map_err(|_| bad_args("columns"))?;
        let breakpoint = view
            .and_then(|view| breakpoint_of(host, view))
            .unwrap_or_else(|| ANY_BREAKPOINT.to_owned());
        return Ok(BTreeMap::from([(breakpoint, Some(spec))]));
    }
    serde_json::from_value(columns).map_err(|_| bad_args("columns"))
}

/// Runs `rulers_set`.
///
/// # Errors
///
/// Arguments that do not read or are out of range.
pub fn run(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args: SetArgs = serde_json::from_str(args).map_err(|_| bad_args("arguments"))?;
    let view = target_view(args.target.as_ref())?;
    let mut overrides = if args.reset {
        Overrides::default()
    } else {
        store::overrides(host, None)
    };
    let touched = args.reset
        || args.guides.is_some()
        || args.columns.is_some()
        || args.baseline.is_some()
        || args.visible.is_some()
        || args.colors.is_some();
    if let Some(inputs) = args.guides {
        if inputs.len() > MAX_GUIDES {
            return Err(bad_args("guides"));
        }
        let guides: Vec<Guide> = inputs
            .into_iter()
            .zip(1..)
            .map(|(input, id)| Guide {
                id,
                axis: input.axis,
                unit: input.unit,
                value: input.value,
            })
            .collect();
        check_guides(&guides).map_err(bad_args)?;
        overrides.guides = Some(guides);
    }
    if let Some(columns) = args.columns {
        let setups = column_setups(host, columns, view)?;
        overrides.columns.extend(setups);
    }
    if let Some(baseline) = args.baseline {
        overrides.baseline = Some(baseline);
    }
    if let Some(colors) = args.colors {
        overrides.colors.merge(colors);
    }
    match args.visible {
        Some(VisibleInput::Rulers(on)) => overrides.visible.rulers = Some(on),
        Some(VisibleInput::Each(each)) => {
            for (field, value) in [
                (&mut overrides.visible.rulers, each.rulers),
                (&mut overrides.visible.columns, each.columns),
                (&mut overrides.visible.baseline, each.baseline),
            ] {
                if value.is_some() {
                    *field = value;
                }
            }
        }
        None => {}
    }
    overrides.check().map_err(bad_args)?;
    let shared = store::shared(host);
    let mut extra = json!({
        "breakpoints": breakpoints(host),
        "templates": crate::columns::templates(),
    });
    if let Some(view) = view {
        extra["breakpoint"] = json!(breakpoint_of(host, view));
    }
    if touched {
        changed(host, &shared, &overrides, extra)
    } else {
        extra["state"] = json!(crate::model::effective(&shared, &overrides));
        Ok(extra.to_string())
    }
}

#[cfg(test)]
#[path = "set_tests.rs"]
mod tests;
