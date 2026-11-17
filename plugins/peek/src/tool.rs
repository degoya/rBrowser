//! The MCP tool `peek_inspect` (`[[tools]] inspect`): the inspection the tooltip shows, as JSON,
//! for the element at a `selector` or a viewport point `x`/`y`, in the view `target` or in every
//! view.
//!
//! The page's part is read by running the collector - the page script's own data code, built
//! from `web/src/collect-entry.ts` into `src/collect.js` - through `evaluate` in each view's
//! isolated world; CDP adds the rendered fonts and the accessibility role and name in Chromium,
//! for the element the collector found (its selector, else its point moved by the page's scroll
//! into the document - `crate::page::locator`, RB-090-54).

use rb_plugin_api::{PluginHost, ViewId};
use serde_json::{Value, json};

use crate::inspect::build;
use crate::node::Extras;
use crate::page::{MAX_SELECTOR, coordinate, invalid, locator};

/// The collector bundle (`npm run build` in `web/`): defines `PeekCollect`.
pub const COLLECTOR: &str = include_str!("collect.js");
/// The most views one call inspects.
pub const MAX_VIEWS: usize = 32;

/// The script `evaluate` runs: the collector, asked for one element.
#[must_use]
pub fn collector_script(query: &Value) -> String {
    format!("(() => {{\n{COLLECTOR}\nreturn PeekCollect.inspect({query});\n}})()")
}

/// The arguments: the views and the collector's query.
fn parse(args: &Value) -> Result<(Option<ViewId>, Value), rb_plugin_api::Failure> {
    if !args.is_object() {
        return Err(invalid("arguments"));
    }
    let view = match &args["target"] {
        Value::Null => None,
        Value::String(all) if all == "all" => None,
        Value::Number(view) => Some(
            view.as_u64()
                .and_then(|view| ViewId::try_from(view).ok())
                .ok_or_else(|| invalid("target"))?,
        ),
        _ => return Err(invalid("target")),
    };
    let has_point = !args["x"].is_null() || !args["y"].is_null();
    match (&args["selector"], has_point) {
        (Value::String(selector), false) => {
            let length = selector.chars().count();
            if !(1..=MAX_SELECTOR).contains(&length) {
                return Err(invalid("selector"));
            }
            Ok((view, json!({ "selector": selector })))
        }
        (Value::Null, true) => {
            let x = coordinate(&args["x"]).ok_or_else(|| invalid("x"))?;
            let y = coordinate(&args["y"]).ok_or_else(|| invalid("y"))?;
            Ok((view, json!({ "x": x, "y": y })))
        }
        _ => Err(invalid("selector_or_point")),
    }
}

/// The page data `evaluate` answered: an object, also when it came as JSON text in a string.
fn page_data(answer: &str) -> Option<Value> {
    let value: Value = serde_json::from_str(answer).ok()?;
    let value = match value {
        Value::String(text) => serde_json::from_str(&text).ok()?,
        other => other,
    };
    value.is_object().then_some(value)
}

/// `peek_inspect`.
///
/// # Errors
///
/// `plugins.error.invalid` for bad arguments; the views' failure without a `target`.
pub fn inspect(host: &mut dyn PluginHost, args: &str) -> Result<String, rb_plugin_api::Failure> {
    let args: Value = serde_json::from_str(args).map_err(|_| invalid("arguments"))?;
    let (target, query) = parse(&args)?;
    let views: Vec<(ViewId, bool)> = match target {
        // One view: whether it speaks CDP is tried, not looked up.
        Some(view) => vec![(view, true)],
        None => host
            .views()?
            .into_iter()
            .take(MAX_VIEWS)
            .map(|view| (view.id, view.engine == "chromium"))
            .collect(),
    };
    let script = collector_script(&query);
    let results: Vec<Value> = views
        .into_iter()
        .map(|(view, cdp)| match host.evaluate(view, &script) {
            Err(failure) => json!({
                "view": view,
                "found": false,
                "error": failure.code.unwrap_or_default(),
            }),
            Ok(answer) => match page_data(&answer) {
                None => json!({ "view": view, "found": false }),
                Some(data) => {
                    let extras = match locator(&data) {
                        Some(locator) if cdp => Extras::read(host, view, &locator),
                        _ => Extras::default(),
                    };
                    json!({ "view": view, "found": true, "inspection": build(&data, &extras) })
                }
            },
        })
        .collect();
    Ok(json!({ "views": results }).to_string())
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
