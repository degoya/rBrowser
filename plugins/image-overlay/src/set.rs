//! The MCP tool `image_overlay_set`, which the panel calls as well.
//!
//! Arguments, every one optional:
//!
//! - `enabled` (bool), `opacity` (0-100 per cent), `blend` (`normal`, `difference`, `multiply`,
//!   `screen`, `overlay`, `exclusion`), `invert` (bool), `anchor` (`document`, `viewport`);
//! - `offset`: `{x?, y?}` in CSS px (a missing one keeps its value);
//! - `split`: `true`/`false` switches the split view, a number (0-100) puts it at that per cent;
//! - `designs`: `{"<breakpoint id>": "designs/<file>"}` - a design per breakpoint, checked to lie
//!   in `.rbrowser/designs/` (`plugin.image-overlay.errors.path_outside` otherwise); `null`
//!   returns the breakpoint to the nearest design;
//! - `reset`: drops the person's overrides (the settings apply again);
//! - `target`: `"all"` or a view id (the host resolves a breakpoint id to its views).
//!
//! The state is one for every view (each shows its own design with it); `target` only chooses
//! which views' designs the answer lists. The answer: `state`, `views` (`view`, `label`,
//! `breakpoint`, `image`, `problem`), `designs` (the files in `.rbrowser/designs/`) and, after a
//! change, `saved`. No argument answers it unchanged, without writing or posting.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, PluginHost};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::page::choice_json;
use crate::path::{DESIGNS_PROJECT_DIR, DesignPath, breakpoint_id_ok};
use crate::select::{Choice, ViewFacts, choose, design_path};
use crate::state::{
    Anchor, Blend, MAX_DESIGNS, Offset, Overrides, Split, State, effective, offset_ok, position_ok,
};
use crate::{bad_args, changed, store};

/// The most file names the answer lists.
const MAX_LISTED: usize = 256;

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct OffsetInput {
    #[serde(default)]
    x: Option<f64>,
    #[serde(default)]
    y: Option<f64>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SplitInput {
    Switch(bool),
    Position(f64),
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct SetArgs {
    enabled: Option<bool>,
    opacity: Option<f64>,
    blend: Option<Blend>,
    invert: Option<bool>,
    anchor: Option<Anchor>,
    offset: Option<OffsetInput>,
    split: Option<SplitInput>,
    designs: Option<BTreeMap<String, Option<String>>>,
    reset: bool,
    target: Option<Value>,
}

impl SetArgs {
    fn changes(&self) -> bool {
        self.enabled.is_some()
            || self.opacity.is_some()
            || self.blend.is_some()
            || self.invert.is_some()
            || self.anchor.is_some()
            || self.offset.is_some()
            || self.split.is_some()
            || self.designs.is_some()
            || self.reset
    }
}

/// The view `target` names: none for `"all"` or no target.
fn target_view(target: Option<&Value>) -> Result<Option<u32>, Failure> {
    match target {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(all)) if all == "all" => Ok(None),
        Some(Value::Number(number)) => number
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .map(Some)
            .ok_or_else(|| bad_args("target")),
        Some(_) => Err(bad_args("target")),
    }
}

/// Applies the arguments to the overrides; `current` is the state before.
fn apply(args: SetArgs, current: &State, overrides: &mut Overrides) -> Result<(), Failure> {
    if args.reset {
        *overrides = Overrides::default();
    }
    if let Some(enabled) = args.enabled {
        overrides.enabled = Some(enabled);
    }
    if let Some(opacity) = args.opacity {
        if !opacity.is_finite() || !(0.0..=100.0).contains(&opacity) {
            return Err(bad_args("opacity"));
        }
        // In range 0..=100 after the check, so the cast cannot truncate.
        overrides.opacity = Some(opacity.round() as u8);
    }
    overrides.blend = args.blend.or(overrides.blend);
    overrides.invert = args.invert.or(overrides.invert);
    overrides.anchor = args.anchor.or(overrides.anchor);
    if let Some(offset) = args.offset {
        let next = Offset {
            x: offset.x.unwrap_or(current.offset.x),
            y: offset.y.unwrap_or(current.offset.y),
        };
        if !offset_ok(next.x) || !offset_ok(next.y) {
            return Err(bad_args("offset"));
        }
        overrides.offset = Some(next);
    }
    if let Some(split) = args.split {
        let next = match split {
            SplitInput::Switch(on) => Split {
                on,
                position: current.split.position,
            },
            SplitInput::Position(position) if position_ok(position) => Split { on: true, position },
            SplitInput::Position(_) => return Err(bad_args("split")),
        };
        overrides.split = Some(next);
    }
    if let Some(designs) = args.designs {
        for (id, path) in designs {
            if !breakpoint_id_ok(&id) {
                return Err(bad_args("designs"));
            }
            if let Some(path) = &path {
                design_path(path).map_err(|problem| problem.failure())?;
            }
            overrides.designs.insert(id, path);
        }
        if overrides.designs.len() > MAX_DESIGNS {
            return Err(bad_args("designs"));
        }
    }
    overrides.check().map_err(bad_args)
}

/// The views `target` covers, each with its design or the reason it has none.
fn views(host: &mut dyn PluginHost, state: &State, target: Option<u32>) -> Vec<Value> {
    let listed = host.views().unwrap_or_default();
    listed
        .into_iter()
        .filter(|view| target.is_none_or(|id| id == view.id))
        .map(|view| {
            let facts = ViewFacts {
                breakpoint: view.breakpoint.clone(),
                width: f64::from(view.width),
                dpr: f64::from(view.dpr),
            };
            let choice: Choice = choose(host, &state.designs, state.auto_scale_dpr, &facts);
            let (image, problem) = choice_json(&choice);
            json!({
                "view": view.id,
                "label": view.label,
                "breakpoint": view.breakpoint,
                "image": image,
                "problem": problem,
            })
        })
        .collect()
}

/// The designs in `.rbrowser/designs/` (top level), for the panel's choice.
fn listed_designs(host: &mut dyn PluginHost) -> Vec<String> {
    host.project_list(DESIGNS_PROJECT_DIR)
        .unwrap_or_default()
        .into_iter()
        .filter(|name| DesignPath::listed(name).is_some())
        .take(MAX_LISTED)
        .collect()
}

/// Runs the tool.
///
/// # Errors
///
/// `bad_args` for an argument that does not read or is out of range; a design's refusal
/// (`path_outside`, `bad_type`).
pub fn run(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args: SetArgs = if args.trim().is_empty() {
        SetArgs::default()
    } else {
        serde_json::from_str(args).map_err(|_| bad_args("arguments"))?
    };
    let target = target_view(args.target.as_ref())?;
    let mut overrides = store::overrides(host, None);
    let shared = store::shared(host);
    let before = effective(&shared, &overrides);
    let changes = args.changes();
    if changes {
        apply(args, &before, &mut overrides)?;
    }
    let state = effective(&shared, &overrides);
    let extra = json!({
        "views": views(host, &state, target),
        "designs": listed_designs(host),
    });
    if !changes {
        let mut answer = extra;
        answer["state"] = json!(state);
        return Ok(answer.to_string());
    }
    Ok(changed(host, &before, &shared, &overrides, extra))
}

#[cfg(test)]
#[path = "set_tests.rs"]
mod tests;
