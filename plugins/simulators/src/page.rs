//! The page script's requests (`call-tool("@page", {view, message})`, `docs/plugins.md`).
//!
//! - `{"op": "init"}`, sent when a document starts: the view's simulation. A Chromium view whose
//!   simulation Chromium renders gets the emulation again (a new session after a renderer came
//!   back has none) and answers `{"spec": null}`; otherwise the answer is the page script's
//!   drawing, `{"spec": {...}}`, or none.

use rb_plugin_api::{Failure, PluginHost, ViewId, ViewInfo};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::apply::{self, Mode};
use crate::bad_args;
use crate::state::{self, Settings};

#[derive(Deserialize)]
struct PageArgs {
    view: ViewId,
    message: Value,
}

/// Answers a page script's request.
///
/// # Errors
///
/// `plugin.simulators.errors.bad_args` for a request that is not `init`.
pub fn handle(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args: PageArgs = serde_json::from_str(args).map_err(|_| bad_args("arguments"))?;
    if args.message.get("op").and_then(Value::as_str) != Some("init") {
        return Err(bad_args("op"));
    }
    let settings = Settings::read(host);
    let choice = state::load(host).effective(args.view, &settings);
    // A view the host does not list (no window yet) is drawn by the page script.
    let view = host
        .views()
        .unwrap_or_default()
        .into_iter()
        .find(|view| view.id == args.view)
        .unwrap_or_else(|| ViewInfo {
            id: args.view,
            ..ViewInfo::default()
        });
    let mode = match apply::mode(&choice, &view.engine, &settings) {
        Mode::Off => Mode::Off,
        _ => apply::apply(host, &view, None, &choice, &settings),
    };
    Ok(json!({ "spec": apply::message(&mode)["spec"] }).to_string())
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
