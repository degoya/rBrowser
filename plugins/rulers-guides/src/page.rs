//! The page script's requests (`call-tool("@page", {view, message})`, `docs/plugins.md`).
//!
//! - `{"op": "init", "geometry"}`: the state, the overrides (the page keeps a copy), the
//!   preferences and the view's drawing.
//! - `{"op": "layout", "geometry", "overrides"?}`: the view's drawing, again after a resize or a
//!   posted change.
//! - `{"op": "guide", "action": "add", "axis", "pos", "snap"?, "geometry"}`,
//!   `{"op": "guide", "action": "move", "id", "pos", "snap"?, "geometry"}`,
//!   `{"op": "guide", "action": "remove", "id"}`: a guide dragged out of a ruler, dragged by its
//!   handle, or double-clicked away. `pos` is page CSS px; the component turns it into the
//!   guide's unit, snapped to `snapPx` with `snap`. The new state is posted to every view and the
//!   panel.
//!
//! `overrides` is the page's copy, used only when the state file cannot be read (no project).

use rb_plugin_api::{Failure, PluginHost, ViewId};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::guides;
use crate::layout::{Geometry, breakpoint_of, layout};
use crate::model::{Axis, GuideUnit, Overrides, effective};
use crate::{bad_args, changed, store};

#[derive(Deserialize)]
struct PageArgs {
    view: ViewId,
    message: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Action {
    Add,
    Move,
    Remove,
}

#[derive(Deserialize)]
struct GuideRequest {
    action: Action,
    #[serde(default)]
    id: Option<u32>,
    #[serde(default)]
    axis: Option<Axis>,
    #[serde(default)]
    pos: Option<f64>,
    #[serde(default)]
    snap: bool,
    #[serde(default)]
    geometry: Option<Geometry>,
    #[serde(default)]
    overrides: Option<Overrides>,
}

#[derive(Deserialize)]
struct LayoutRequest {
    geometry: Geometry,
    #[serde(default)]
    overrides: Option<Overrides>,
}

fn parse<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, Failure> {
    serde_json::from_value(value).map_err(|_| bad_args("message"))
}

/// Answers a page script's request.
///
/// # Errors
///
/// The request is not one of the above, or a value is out of range.
pub fn handle(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args: PageArgs = serde_json::from_str(args).map_err(|_| bad_args("arguments"))?;
    let op = args.message.get("op").and_then(Value::as_str).unwrap_or("");
    match op {
        "init" | "layout" => {
            let request: LayoutRequest = parse(args.message.clone())?;
            request.geometry.check().map_err(bad_args)?;
            let overrides = store::overrides(host, request.overrides);
            let state = effective(&store::shared(host), &overrides);
            let drawn = layout(&state, breakpoint_of(host, args.view), request.geometry);
            if op == "layout" {
                return Ok(json!({ "layout": drawn }).to_string());
            }
            let prefs = store::prefs(host);
            Ok(json!({
                "state": state,
                "overrides": overrides,
                "prefs": { "unit": prefs.unit, "snapPx": prefs.snap_px },
                "layout": drawn,
            })
            .to_string())
        }
        "guide" => guide(host, parse(args.message)?),
        _ => Err(bad_args("op")),
    }
}

fn guide(host: &mut dyn PluginHost, request: GuideRequest) -> Result<String, Failure> {
    let mut overrides = store::overrides(host, request.overrides.clone());
    let shared = store::shared(host);
    let mut list = effective(&shared, &overrides).guides;
    let prefs = store::prefs(host);
    let at = |axis: Axis, unit: GuideUnit| -> Result<f64, Failure> {
        let geometry = request.geometry.ok_or_else(|| bad_args("geometry"))?;
        geometry.check().map_err(bad_args)?;
        let pos = request.pos.ok_or_else(|| bad_args("pos"))?;
        let pos = if request.snap {
            guides::snap(pos, prefs.snap_px)
        } else {
            pos
        };
        Ok(guides::value_at(axis, unit, pos, geometry.client_width))
    };
    match request.action {
        Action::Add => {
            let axis = request.axis.ok_or_else(|| bad_args("axis"))?;
            let unit = if axis == Axis::X {
                prefs.unit
            } else {
                GuideUnit::Px
            };
            let value = at(axis, unit)?;
            guides::add(&mut list, axis, unit, value).map_err(bad_args)?;
        }
        Action::Move => {
            let id = request.id.ok_or_else(|| bad_args("id"))?;
            let current = list
                .iter()
                .find(|guide| guide.id == id)
                .cloned()
                .ok_or_else(|| bad_args("guide.id"))?;
            let value = at(current.axis, current.unit)?;
            guides::move_to(&mut list, id, value).map_err(bad_args)?;
        }
        Action::Remove => {
            let id = request.id.ok_or_else(|| bad_args("id"))?;
            guides::remove(&mut list, id);
        }
    }
    overrides.guides = Some(list);
    changed(host, &shared, &overrides, json!({}))
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
