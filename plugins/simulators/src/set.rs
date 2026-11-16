//! MCP `simulator_set` (`type`, `severity?`, `target?`), also the picker's call.
//!
//! - `type` sets the simulation of the `target` views - `"all"`, a view id, or without one the
//!   active view; `"none"` removes it. The answer is every view's simulation; the page scripts of
//!   the changed views and the panel get the change (`$post`).
//! - Without `type` it answers the views' simulations, the active view and the catalogue, and
//!   changes nothing.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, ViewInfo};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::apply::{self, Mode};
use crate::catalog::Simulation;
use crate::state::{self, Choice, Settings, State};
use crate::{PLUGIN_ID, bad_args};

/// The most posts of one answer (the host's limit); one is the panel's.
const MAX_POSTS: usize = 16;

#[derive(Deserialize)]
#[serde(untagged)]
enum Target {
    View(ViewId),
    Named(String),
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SetArgs {
    #[serde(rename = "type")]
    kind: Option<Value>,
    severity: Option<Value>,
    target: Option<Target>,
}

fn no_view() -> Failure {
    Failure::coded(
        FailureKind::NotFound,
        &format!("plugin.{PLUGIN_ID}.errors.no_view"),
    )
}

/// The views `target` names.
fn targets(
    host: &mut dyn PluginHost,
    views: &[ViewInfo],
    target: Option<Target>,
) -> Result<Vec<ViewInfo>, Failure> {
    let id = match target {
        Some(Target::Named(name)) if name == "all" => return Ok(views.to_vec()),
        Some(Target::Named(_)) => return Err(bad_args("target")),
        Some(Target::View(id)) => id,
        None => host.active_view()?.ok_or_else(no_view)?,
    };
    views
        .iter()
        .find(|view| view.id == id)
        .cloned()
        .map(|view| vec![view])
        .ok_or_else(|| bad_args("target"))
}

fn parse_choice(kind: Value, severity: Option<Value>) -> Result<Choice, Failure> {
    let kind: Simulation = serde_json::from_value(kind).map_err(|_| bad_args("type"))?;
    let severity = match severity {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_f64()
                .filter(|severity| (0.0..=1.0).contains(severity))
                .ok_or_else(|| bad_args("severity"))?,
        ),
    };
    // A simulation without a strength keeps none.
    Ok(Choice {
        kind,
        severity: severity.filter(|_| kind.default_severity().is_some()),
    })
}

/// Every view's simulation, as the answer and the panel show it.
pub fn reports(views: &[ViewInfo], state: &State, settings: &Settings) -> Vec<Value> {
    views
        .iter()
        .map(|view| {
            let choice = state.effective(view.id, settings);
            let mode = apply::mode(&choice, &view.engine, settings);
            apply::report(view, &choice, &mode)
        })
        .collect()
}

/// The posts of a change: the same message to every view as one post to `"all"`, else one per
/// changed view; then the panel's.
fn posts(
    host: &mut dyn PluginHost,
    changed: &[(ViewId, Value)],
    every_view: bool,
    panel: Value,
) -> Value {
    let mut posts = Vec::new();
    let same = changed.windows(2).all(|pair| pair[0].1 == pair[1].1);
    match changed.first() {
        Some((_, message)) if every_view && same => {
            posts.push(json!({ "target": "all", "message": message }));
        }
        _ => {
            for (view, message) in changed.iter().take(MAX_POSTS - 1) {
                posts.push(json!({ "target": view, "message": message }));
            }
            if changed.len() > MAX_POSTS - 1 {
                host.log(
                    "warn",
                    "more views than posts: the rest show the change at their next load",
                );
            }
        }
    }
    posts.push(json!({ "target": "panel", "message": panel }));
    Value::Array(posts)
}

/// Runs `simulator_set`.
///
/// # Errors
///
/// `plugin.simulators.errors.bad_args` (`reason`: `arguments`, `type`, `severity`, `target`),
/// `plugin.simulators.errors.no_view` without a target and an active view.
pub fn run(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args: SetArgs = if args.trim().is_empty() {
        SetArgs::default()
    } else {
        serde_json::from_str(args).map_err(|_| bad_args("arguments"))?
    };
    let settings = Settings::read(host);
    let views = host.views()?;
    let mut state = state::load(host);
    let active = host.active_view()?;
    let Some(kind) = args.kind else {
        return Ok(json!({
            "views": reports(&views, &state, &settings),
            "active": active,
            "catalog": apply::catalog(),
        })
        .to_string());
    };
    let choice = parse_choice(kind, args.severity)?;
    let every_view = matches!(&args.target, Some(Target::Named(name)) if name == "all");
    let targets = targets(host, &views, args.target)?;

    let mut changed = Vec::new();
    let mut in_force = Vec::new();
    for view in &targets {
        let before = state.effective(view.id, &settings);
        let mode: Mode = apply::apply(host, view, Some(&before), &choice, &settings);
        state.views.insert(view.id, choice);
        changed.push((view.id, apply::message(&mode)));
        in_force.push(apply::report(view, &choice, &mode));
    }
    // A full file keeps the views of the grid.
    while state.views.len() > state::MAX_VIEWS {
        let stale = state
            .views
            .keys()
            .copied()
            .find(|id| !views.iter().any(|view| view.id == *id));
        match stale.or_else(|| state.views.keys().next().copied()) {
            Some(id) => state.views.remove(&id),
            None => break,
        };
    }
    let saved = match state::save(host, &state) {
        Ok(()) => true,
        Err(failure) => {
            let code = failure.code.unwrap_or_default();
            host.log("warn", &format!("state.json not written: {code}"));
            false
        }
    };
    // The changed views as they came out: a refused emulation is the page script's drawing.
    let views_now: Vec<Value> = reports(&views, &state, &settings)
        .into_iter()
        .map(|entry| {
            in_force
                .iter()
                .find(|report| report["view"] == entry["view"])
                .cloned()
                .unwrap_or(entry)
        })
        .collect();
    let panel = json!({ "type": "state", "views": views_now, "active": active });
    let posts = posts(host, &changed, every_view, panel);
    Ok(json!({
        "views": views_now,
        "active": active,
        "saved": saved,
        "$post": posts,
    })
    .to_string())
}

#[cfg(test)]
#[path = "set_tests.rs"]
mod tests;
