//! What switching an overlay does beyond the page script's drawing (RB-070-07):
//!
//! - a **checking overlay** (`scan`) switched on scans each view of the target through
//!   `evaluate` and answers the hits (`hits`), reported as findings while `reportFindings` is on;
//! - **`force-state`** forces pseudo-classes on the element its selector finds, through CDP
//!   `CSS.forcePseudoState` (Chromium views only), and releases them when switched off or moved
//!   to another selector (`forced`);
//! - **`js-off`** disables script execution through CDP `Emulation.setScriptExecutionDisabled`
//!   and reloads the view, and the reverse when switched off (`scripts`);
//! - **`tab-order`** switched on measures the tab order of each view and compares them
//!   (`tabOrder`, [`tab_order::switched_on`]), its warnings reported as findings.
//!
//! A Firefox or WebKit view has no CDP: its entry says `unsupported`. A new document of a view
//! gets its forced state and its disabled scripts again when its page script says hello.

use rb_plugin_api::{Failure, PluginHost, ViewId, ViewInfo, codes};
use serde_json::{Map, Value, json};

use crate::budget;
use crate::scan::{self, Scan};
use crate::settings::{self, PageSettings};
use crate::state::{OverlayState, State, Target};
use crate::tab_order;

/// What a view without CDP answers.
pub const UNSUPPORTED: &str = "unsupported";
/// What a view answers when the selector finds no element.
pub const NOT_FOUND: &str = "not_found";
/// The pseudo-class `force-state` forces when none is given.
pub const DEFAULT_STATE: &str = "hover";
/// The reload after scripts are switched on or off, run in the plugin's own world.
const RELOAD: &str = "(() => { location.reload(); return true; })()";

fn chromium(view: &ViewInfo) -> bool {
    view.engine.is_empty() || view.engine == "chromium"
}

fn code(failure: &Failure) -> String {
    failure.code().unwrap_or(codes::HOST_FAILED).to_owned()
}

fn send(
    host: &mut dyn PluginHost,
    view: ViewId,
    method: &str,
    params: &Value,
) -> Result<Value, Failure> {
    let answer = host.cdp_send(view, method, &params.to_string())?;
    Ok(serde_json::from_str(&answer).unwrap_or(Value::Null))
}

/// The front-end node id of the document of `view` (review finding L2 of Charge 7, peek's
/// pattern): the session is shared with the host and the other plugins, and `DOM.getDocument`
/// drops every node id handed out before and switches the DOM events on. So the document the root
/// of the accessibility tree names is pushed into the document someone already requested, and the
/// document is requested only when nobody has - then nobody holds a node id it could drop.
fn document(host: &mut dyn PluginHost, view: ViewId) -> Result<Option<u64>, Failure> {
    let tree = send(
        host,
        view,
        "Accessibility.getFullAXTree",
        &json!({ "depth": 1 }),
    )?;
    let Some(backend) = tree["nodes"][0]["backendDOMNodeId"].as_u64() else {
        return Ok(None);
    };
    let params = json!({ "backendNodeIds": [backend] });
    let pushed = send(host, view, "DOM.pushNodesByBackendIdsToFrontend", &params)
        .ok()
        .and_then(|pushed| pushed["nodeIds"][0].as_u64())
        .filter(|id| *id != 0);
    if pushed.is_some() {
        return Ok(pushed);
    }
    let document = send(host, view, "DOM.getDocument", &json!({ "depth": 0 }))?;
    Ok(document["root"]["nodeId"].as_u64())
}

/// The front-end node id of the first element `selector` finds in `view`; none for no element.
fn node(host: &mut dyn PluginHost, view: ViewId, selector: &str) -> Result<Option<u64>, Failure> {
    let Some(root) = document(host, view)? else {
        return Ok(None);
    };
    let params = json!({ "nodeId": root, "selector": selector });
    let found = send(host, view, "DOM.querySelector", &params)?;
    Ok(found["nodeId"].as_u64().filter(|id| *id != 0))
}

/// Forces `states` on `node`. The CSS domain is switched on only when the force is refused for
/// want of it (it needs the DOM domain first): no `*.disable` may follow, so every domain switched
/// on stays on for the shared session and floods it with events (review finding L2 of Charge 7).
fn force_node(
    host: &mut dyn PluginHost,
    view: ViewId,
    node: u64,
    states: &Value,
) -> Result<(), Failure> {
    let params = json!({ "nodeId": node, "forcedPseudoClasses": states });
    if send(host, view, "CSS.forcePseudoState", &params).is_ok() {
        return Ok(());
    }
    if send(host, view, "CSS.enable", &json!({})).is_err() {
        send(host, view, "DOM.enable", &json!({}))?;
        send(host, view, "CSS.enable", &json!({}))?;
    }
    send(host, view, "CSS.forcePseudoState", &params).map(drop)
}

fn force_one(
    host: &mut dyn PluginHost,
    view: ViewId,
    selector: &str,
    states: &Value,
) -> Result<bool, Failure> {
    let Some(node) = node(host, view, selector)? else {
        return Ok(false);
    };
    force_node(host, view, node, states)?;
    Ok(true)
}

/// Forces `states` on the element `selector` finds in each of `views`.
pub fn force(
    host: &mut dyn PluginHost,
    views: &[ViewInfo],
    selector: &str,
    states: &Value,
) -> Value {
    let entries = views
        .iter()
        .map(|view| {
            if !chromium(view) {
                return json!({ "view": view.id, "error": UNSUPPORTED });
            }
            match force_one(host, view.id, selector, states) {
                Ok(true) => json!({ "view": view.id, "ok": true }),
                Ok(false) => json!({ "view": view.id, "error": NOT_FOUND }),
                Err(failure) => json!({ "view": view.id, "error": code(&failure) }),
            }
        })
        .collect();
    Value::Array(entries)
}

/// Releases every forced state of the element `selector` finds in each of `views`; a view that
/// cannot be asked is passed over (its document, and the state with it, may be gone).
pub fn release(host: &mut dyn PluginHost, views: &[ViewInfo], selector: &str) {
    for view in views.iter().filter(|view| chromium(view)) {
        if let Ok(Some(node)) = node(host, view.id, selector) {
            let params = json!({ "nodeId": node, "forcedPseudoClasses": [] });
            let _ = send(host, view.id, "CSS.forcePseudoState", &params);
        }
    }
}

/// Switches script execution off (`disabled`) or on in each of `views`, reloading each when
/// `reload` (a hello only repeats the switch for the document that just started).
pub fn scripts(
    host: &mut dyn PluginHost,
    views: &[ViewInfo],
    disabled: bool,
    reload: bool,
) -> Value {
    let entries = views
        .iter()
        .map(|view| {
            if !chromium(view) {
                return json!({ "view": view.id, "error": UNSUPPORTED });
            }
            let params = json!({ "value": disabled });
            let done = send(
                host,
                view.id,
                "Emulation.setScriptExecutionDisabled",
                &params,
            )
            .and_then(|_| {
                if reload {
                    host.evaluate(view.id, RELOAD).map(drop)
                } else {
                    Ok(())
                }
            });
            match done {
                Ok(()) => json!({ "view": view.id, "ok": true }),
                Err(failure) => json!({ "view": view.id, "error": code(&failure) }),
            }
        })
        .collect();
    Value::Array(entries)
}

/// The options a scan of `overlay` runs with: its own, else the settings'.
#[must_use]
pub fn scan_options(overlay: &str, options: &Map<String, Value>, settings: &PageSettings) -> Value {
    let number =
        |name: &str, default: u64| options.get(name).and_then(Value::as_u64).unwrap_or(default);
    match overlay {
        "target-size" => json!({ "sizePx": number("sizePx", settings.target_size_px) }),
        "small-text" => json!({ "px": number("px", settings.small_text_px) }),
        "scrollbars" => {
            json!({ "tolerancePx": number("tolerancePx", settings.scrollbar_tolerance_px) })
        }
        _ => json!({}),
    }
}

/// Scans each of `views` for `overlay`; reports the hits as findings while `reportFindings` is
/// on. Answers one entry per view: its count and the first hits that fit the answer
/// ([`budget::SCAN_BYTES`], `shown`), or its error.
pub fn scan(
    host: &mut dyn PluginHost,
    views: &[ViewInfo],
    overlay: &str,
    options: &Value,
) -> Value {
    let script = scan::script(overlay, options);
    let mut found = Vec::new();
    let outcomes: Vec<Result<Scan, String>> = views
        .iter()
        .map(|view| match host.evaluate(view.id, &script) {
            Ok(answer) => Scan::parse(&answer).ok_or_else(|| codes::BAD_OUTPUT.to_owned()),
            Err(failure) => Err(code(&failure)),
        })
        .collect();
    let lists: Vec<&[scan::Hit]> = outcomes
        .iter()
        .map(|outcome| {
            outcome
                .as_ref()
                .map_or(&[][..], |result| result.items.as_slice())
        })
        .collect();
    let shown = budget::share(&lists, scan::MAX_ANSWERED, budget::SCAN_BYTES);
    let mut entries = Vec::new();
    for ((view, outcome), shown) in views.iter().zip(&outcomes).zip(shown) {
        match outcome {
            Ok(result) => {
                found.extend(scan::findings(overlay, view, result));
                entries.push(result.answer(view, shown));
            }
            Err(error) => entries.push(json!({ "view": view.id, "error": error })),
        }
    }
    if !found.is_empty()
        && settings::report_findings(host)
        && let Err(failure) = host.report_findings(&found)
    {
        let code = code(&failure);
        host.log(
            "warn",
            &format!("overlays: the findings were not reported ({code})"),
        );
    }
    Value::Array(entries)
}

/// The options `next` of `overlay` completed before it is switched on: `force-state` takes the
/// highlight's selector when it has none, and `hover` when it forces nothing.
pub fn complete(overlay: &str, state: &State, target: Target, next: &mut OverlayState) {
    if overlay != "force-state" || !next.enabled {
        return;
    }
    let has_selector = next
        .options
        .get("selector")
        .and_then(Value::as_str)
        .is_some_and(|selector| !selector.is_empty());
    if !has_selector
        && let Some(selector) = state
            .effective(target.view(), "highlight")
            .options
            .get("selector")
            .cloned()
    {
        next.options.insert("selector".to_owned(), selector);
    }
    next.options
        .entry("states")
        .or_insert_with(|| json!([DEFAULT_STATE]));
}

fn selector(state: &OverlayState) -> Option<&str> {
    state
        .options
        .get("selector")
        .and_then(Value::as_str)
        .filter(|selector| !selector.is_empty())
}

/// What `overlay` going from `previous` to `next` in `views` does; answers the body's fields.
pub fn changed(
    host: &mut dyn PluginHost,
    views: &[ViewInfo],
    overlay: &str,
    previous: &OverlayState,
    next: &OverlayState,
) -> Map<String, Value> {
    let mut body = Map::new();
    match overlay {
        "force-state" => {
            if previous.enabled
                && let Some(old) = selector(previous)
                && (!next.enabled || selector(next) != Some(old))
            {
                release(host, views, old);
            }
            if next.enabled
                && let Some(new) = selector(next)
            {
                let states = next
                    .options
                    .get("states")
                    .cloned()
                    .unwrap_or(json!([DEFAULT_STATE]));
                body.insert("forced".to_owned(), force(host, views, new, &states));
            }
        }
        "js-off" if previous.enabled != next.enabled => {
            body.insert(
                "scripts".to_owned(),
                scripts(host, views, next.enabled, true),
            );
        }
        "tab-order" if next.enabled => {
            body.insert("tabOrder".to_owned(), tab_order::switched_on(host, views));
        }
        id if next.enabled && scan::is_scan(id) => {
            let options = scan_options(id, &next.options, &settings::page_settings(host));
            body.insert("hits".to_owned(), scan(host, views, id, &options));
        }
        _ => {}
    }
    body
}

/// A page script's hello from `view`: its new document gets the forced state and the disabled
/// scripts its view shows again (no reload).
pub fn hello(host: &mut dyn PluginHost, view: ViewId, state: &State) {
    let forced = state.effective(Some(view), "force-state");
    let scripts_off = state.effective(Some(view), "js-off").enabled;
    if !(forced.enabled || scripts_off) {
        return;
    }
    let info = host
        .views()
        .ok()
        .and_then(|views| views.into_iter().find(|info| info.id == view))
        .unwrap_or(ViewInfo {
            id: view,
            ..ViewInfo::default()
        });
    let views = [info];
    if forced.enabled
        && let Some(selector) = selector(&forced)
    {
        let states = forced
            .options
            .get("states")
            .cloned()
            .unwrap_or(json!([DEFAULT_STATE]));
        force(host, &views, selector, &states);
    }
    if scripts_off {
        scripts(host, &views, true, false);
    }
}

#[cfg(test)]
#[path = "effects_tests.rs"]
mod tests;
