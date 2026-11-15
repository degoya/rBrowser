//! The commands, the MCP tools and the page scripts' requests.
//!
//! - `set` (`overlay_set`): `{ overlay, enabled?, options?, target? }` switches one overlay (or
//!   `"all"` off) for every view, one, or the active one (`"active"`); answers `{ overlays,
//!   matches?, hits?, forced?, scripts? }` - the overlays of the target, for a new highlight
//!   selector its match count per view, and what [`crate::effects`] did.
//! - `overflow` (`overlay_overflow`): `{ show100vw?, target? }` measures each view of the target
//!   and answers `{ views: [{ view, label, breakpoint, hasHorizontalScroll, scrollWidth,
//!   clientWidth, innerWidth, causes, vw100 } | { view, label, breakpoint, error }] }`.
//! - `state` (`overlay_state`): `{ target? }` answers `{ overlays, settings }`.
//! - every command of [`overlay::COMMANDS`] toggles its overlay (`highlight` takes `{ selector }`
//!   or `{ step }`, `force_state` `{ selector, states }`); `off` switches every overlay off.
//! - `@page`: `{ type: "hello" }` answers `{ state, settings }` (and forces the view's state and
//!   disables its scripts again, [`effects::hello`]); `{ type: "overflow", report }` logs the
//!   report; `{ type: "tab-focus", path }` is the tab order's live focus ([`tab_order::focus`]).
//!
//! Every change is posted to the page scripts (`"all"`) and the panel as
//! `{ type: "state", state, settings }`; each page script takes its own view's part. The tab
//! order switched on also posts its differences to the panel (`{ type: "tab-order", .. }`).

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, ViewInfo, codes};
use serde_json::{Map, Value, json};

use crate::effects;
use crate::overlay::{self, OVERLAYS, OverlayKind, refusal};
use crate::report::{self, Report};
use crate::settings;
use crate::state::{OverlayState, State, Target};
use crate::tab_order;

pub(crate) fn bad_args() -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", "arguments")
}

/// The arguments: a JSON object.
pub(crate) fn object(args: &str) -> Result<Map<String, Value>, Failure> {
    match serde_json::from_str(args) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Err(bad_args()),
    }
}

/// The target of a call: a view id, the active view for `"active"`, else every view.
pub(crate) fn target_of(host: &mut dyn PluginHost, args: &Map<String, Value>) -> Target {
    if args.get("target").and_then(Value::as_str) == Some("active")
        && let Ok(Some(view)) = host.active_view()
    {
        return Target::View(view);
    }
    Target::from_args(&Value::Object(args.clone()))
}

/// What the views of `target` show, by overlay id.
fn overlays_for(state: &State, target: Target) -> Value {
    let overlays: Map<String, Value> = OVERLAYS
        .iter()
        .map(|kind| {
            let effective = state.effective(target.view(), kind.id);
            (
                kind.id.to_owned(),
                serde_json::to_value(effective).unwrap_or(Value::Null),
            )
        })
        .collect();
    Value::Object(overlays)
}

/// The state message the page scripts and the panel receive.
fn state_message(host: &mut dyn PluginHost, state: &State) -> Value {
    json!({ "type": "state", "state": state, "settings": settings::page_settings(host) })
}

/// Saves `state`; a project without an out folder keeps the change for the posts only.
fn store(host: &mut dyn PluginHost, state: &State) {
    if let Err(failure) = state.save(host) {
        let code = failure.code().unwrap_or("io").to_owned();
        host.log(
            "warn",
            &format!("overlays: the overlay state was not saved ({code})"),
        );
    }
}

/// `body` with the target's overlays, and the state posted to every page script and the panel.
fn answer(
    host: &mut dyn PluginHost,
    state: &State,
    target: Target,
    mut body: Map<String, Value>,
) -> String {
    store(host, state);
    let message = state_message(host, state);
    body.insert("overlays".to_owned(), overlays_for(state, target));
    let mut posts = vec![
        json!({ "target": "all", "message": message }),
        json!({ "target": "panel", "message": message }),
    ];
    posts.extend(body.get("tabOrder").and_then(tab_order::panel_post));
    body.insert("$post".to_owned(), Value::Array(posts));
    Value::Object(body).to_string()
}

/// The views of `target`, with their labels where the host lists them.
pub(crate) fn target_views(
    host: &mut dyn PluginHost,
    target: Target,
) -> Result<Vec<ViewInfo>, Failure> {
    match target {
        Target::All => host.views(),
        Target::View(id) => {
            let known = host
                .views()
                .ok()
                .and_then(|views| views.into_iter().find(|view| view.id == id));
            Ok(vec![known.unwrap_or(ViewInfo {
                id,
                ..ViewInfo::default()
            })])
        }
    }
}

/// The number of elements `selector` matches in each view of `target`. A view that cannot be
/// asked is left out; a selector a view refuses is refused.
fn count_matches(
    host: &mut dyn PluginHost,
    target: Target,
    selector: &str,
) -> Result<Value, Failure> {
    let literal = Value::String(selector.to_owned()).to_string();
    let script = format!(
        "(() => {{ try {{ return document.querySelectorAll({literal}).length; }} catch (error) {{ return -1; }} }})()"
    );
    let mut counts = Vec::new();
    for view in target_views(host, target).unwrap_or_default() {
        let Ok(answer) = host.evaluate(view.id, &script) else {
            continue;
        };
        match serde_json::from_str::<i64>(answer.trim()) {
            Ok(-1) => return Err(refusal("bad_selector", &[("selector", selector)])),
            Ok(count) if count >= 0 => counts.push(json!({ "view": view.id, "count": count })),
            _ => {}
        }
    }
    Ok(Value::Array(counts))
}

/// Switches `kind` for `target` with `options` over its current ones; answers the body's
/// `matches` for a new highlight selector.
fn apply(
    host: &mut dyn PluginHost,
    state: &mut State,
    target: Target,
    kind: &OverlayKind,
    enabled: bool,
    options: &Map<String, Value>,
) -> Result<Map<String, Value>, Failure> {
    let options = overlay::validate_options(kind, options)?;
    let previous = state.effective(target.view(), kind.id);
    let mut next = previous.clone();
    let new_selector = options
        .get("selector")
        .filter(|selector| next.options.get("selector") != Some(*selector))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let has_index = kind.options.iter().any(|(name, _)| *name == "index");
    if has_index && new_selector.is_some() && !options.contains_key("index") {
        next.options.insert("index".to_owned(), json!(0));
    }
    next.options.extend(options);
    next.enabled = enabled;
    effects::complete(kind.id, state, target, &mut next);
    if enabled
        && let Some(required) = kind.requires
        && next
            .options
            .get(required)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
    {
        return Err(refusal(&format!("no_{required}"), &[]));
    }
    let mut body = Map::new();
    if let Some(selector) = new_selector.filter(|selector| enabled && !selector.is_empty()) {
        let matches = count_matches(host, target, &selector)?;
        body.insert("matches".to_owned(), matches);
    }
    let views = target_views(host, target).unwrap_or_default();
    body.extend(effects::changed(host, &views, kind.id, &previous, &next));
    state.set(target, kind.id, next);
    Ok(body)
}

/// Every overlay of `target` off; the forced states released and the scripts on again.
fn all_off(host: &mut dyn PluginHost, state: &mut State, target: Target) {
    for id in ["force-state", "js-off"] {
        let previous = state.effective(target.view(), id);
        if previous.enabled {
            let views = target_views(host, target).unwrap_or_default();
            let off = OverlayState {
                enabled: false,
                options: previous.options.clone(),
            };
            effects::changed(host, &views, id, &previous, &off);
        }
    }
    match target {
        Target::All => state.off(),
        Target::View(_) => {
            for kind in OVERLAYS {
                let mut off = state.effective(target.view(), kind.id);
                off.enabled = false;
                state.set(target, kind.id, off);
            }
        }
    }
}

/// `set`.
///
/// # Errors
///
/// Arguments that are no object, an unknown overlay or option, a highlight without a selector, a
/// selector a view refuses, `"all"` switched on.
pub fn set(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args = object(args)?;
    let target = target_of(host, &args);
    let overlay = args
        .get("overlay")
        .and_then(Value::as_str)
        .ok_or_else(|| refusal("unknown_overlay", &[("overlay", "")]))?;
    let enabled = match args.get("enabled") {
        None => true,
        Some(value) => value.as_bool().ok_or_else(bad_args)?,
    };
    let options = match args.get("options") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(options)) => options.clone(),
        Some(_) => return Err(bad_args()),
    };
    let mut state = State::load(host);
    if overlay == "all" {
        if enabled {
            return Err(refusal("all_on", &[]));
        }
        all_off(host, &mut state, target);
        return Ok(answer(host, &state, target, Map::new()));
    }
    let kind = overlay::kind(overlay)
        .ok_or_else(|| refusal("unknown_overlay", &[("overlay", overlay)]))?;
    let body = apply(host, &mut state, target, kind, enabled, &options)?;
    Ok(answer(host, &state, target, body))
}

/// The commands: each of [`overlay::COMMANDS`] toggles its overlay, `off` switches every overlay
/// off. `highlight` with `{ selector }` searches it, with `{ step }` moves to another match;
/// `force_state` with `{ selector }` forces it.
///
/// # Errors
///
/// As [`set`]; an unknown command is the caller's.
pub fn command(host: &mut dyn PluginHost, command: &str, args: &str) -> Result<String, Failure> {
    let mut args = object(args)?;
    let target = target_of(host, &args);
    args.remove("target");
    let mut state = State::load(host);
    if command == "off" {
        all_off(host, &mut state, target);
        return Ok(answer(host, &state, target, Map::new()));
    }
    let kind = overlay::command_overlay(command)
        .and_then(overlay::kind)
        .ok_or_else(|| refusal("unknown_overlay", &[("overlay", command)]))?;
    let current = state.effective(target.view(), kind.id);
    let (enabled, options) = if kind.id == "force-state" && args.contains_key("selector") {
        (true, args)
    } else if kind.id != "highlight" {
        (!current.enabled, args)
    } else if args.get("selector").and_then(Value::as_str).is_some() {
        (true, args)
    } else if let Some(step) = args.get("step") {
        let step = step.as_i64().ok_or_else(bad_args)?;
        let index = current
            .options
            .get("index")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let index = index
            .saturating_add(step)
            .clamp(-overlay::MAX_INDEX, overlay::MAX_INDEX);
        let mut options = Map::new();
        options.insert("index".to_owned(), json!(index));
        (true, options)
    } else {
        (!current.enabled, Map::new())
    };
    let body = apply(host, &mut state, target, kind, enabled, &options)?;
    Ok(answer(host, &state, target, body))
}

/// `state`.
///
/// # Errors
///
/// Arguments that are no object.
pub fn state(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args = object(args)?;
    let state = State::load(host);
    let target = target_of(host, &args);
    let body = json!({
        "overlays": overlays_for(&state, target),
        "settings": settings::page_settings(host),
    });
    Ok(body.to_string())
}

/// `overflow`.
///
/// # Errors
///
/// Arguments that are no object; the views cannot be listed for every view.
pub fn overflow(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args = object(args)?;
    let show100vw = match args.get("show100vw") {
        None => settings::page_settings(host).show100vw,
        Some(value) => value.as_bool().ok_or_else(bad_args)?,
    };
    let script = report::measure_script(show100vw);
    let mut entries = Vec::new();
    let target = target_of(host, &args);
    for view in target_views(host, target)? {
        let mut entry = Map::new();
        entry.insert("view".to_owned(), json!(view.id));
        entry.insert("label".to_owned(), json!(view.label));
        entry.insert("breakpoint".to_owned(), json!(view.breakpoint));
        match host.evaluate(view.id, &script) {
            Ok(answer) => match Report::parse(&answer) {
                Some(report) => {
                    if let Some(line) = report.log_line(view.id) {
                        host.log("warn", &line);
                    }
                    if let Ok(Value::Object(fields)) = serde_json::to_value(&report) {
                        entry.extend(fields);
                    }
                }
                None => {
                    entry.insert("error".to_owned(), json!(codes::BAD_OUTPUT));
                }
            },
            Err(failure) => {
                let code = failure.code().unwrap_or(codes::HOST_FAILED).to_owned();
                entry.insert("error".to_owned(), json!(code));
            }
        }
        entries.push(Value::Object(entry));
    }
    Ok(json!({ "views": entries }).to_string())
}

/// A page script's request: `{ view, message }`.
///
/// # Errors
///
/// Not the host's `@page` shape, or a message of an unknown type.
pub fn page(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args = object(args)?;
    let view: ViewId = args
        .get("view")
        .and_then(Value::as_u64)
        .and_then(|view| ViewId::try_from(view).ok())
        .ok_or_else(bad_args)?;
    let message = args.get("message").ok_or_else(bad_args)?;
    match message.get("type").and_then(Value::as_str) {
        Some("hello") => {
            let state = State::load(host);
            effects::hello(host, view, &state);
            let body = json!({ "state": state, "settings": settings::page_settings(host) });
            Ok(body.to_string())
        }
        Some("tab-focus") => tab_order::focus(host, view, message),
        Some("overflow") => {
            let report = message
                .get("report")
                .and_then(|report| Report::parse(&report.to_string()))
                .ok_or_else(bad_args)?;
            match report.log_line(view) {
                Some(line) => host.log("warn", &line),
                None => host.log("info", &format!("view {view}: no horizontal scroll")),
            }
            Ok("{}".to_owned())
        }
        _ => Err(bad_args()),
    }
}

/// At a start: the state of single views is gone, and scripts are never off (a view's new
/// session runs them); without `restoreOnStart` every overlay is gone.
pub fn activate(host: &mut dyn PluginHost) {
    let state = State::load(host);
    let next = if settings::restore_on_start(host) {
        let mut kept = state.clone();
        kept.forget_views();
        if let Some(scripts) = kept.all.get_mut("js-off") {
            scripts.enabled = false;
        }
        kept
    } else {
        State::default()
    };
    if next != state {
        store(host, &next);
    }
}
