//! The tab order overlay's component side (RB-070-12). The page script draws the order; the
//! component measures it in each view through `evaluate` (the bundle `assets/measure.js`, global
//! `__rbOverlaysMeasure.tabOrder`) and compares the views ([`crate::tab_compare`]):
//!
//! - `tab_order` (`overlay_tab_order`): `{ target?, includeIframes? }` answers `{ views: [{ view,
//!   label, breakpoint, count, shown, items: [{ index, selector, path, tabindex, rect, warnings,
//!   iframe? }] } | { view, label, breakpoint, error }], differences }` and posts the differences
//!   to the panel; `items` are the first `shown` stops that fit the answer
//!   ([`budget::TAB_BYTES`] for every view together, RB-090-17), `count` all of them;
//! - switching the overlay on (`overlay_set { overlay: "tab-order" }`, [`crate::effects`])
//!   answers the same as `tabOrder` and reports the warnings as findings while `reportFindings` is
//!   on: `tab-positive-tabindex` and `tab-invisible-focusable`;
//! - a page script's `{ type: "tab-focus", path }` (live mode) is posted to every view as
//!   `{ type: "tab-focus", view, path }` while `tabOrder.live` is on.
//!
//! No CDP: the order is the page's own DOM, read in the plugin's isolated world. A list comes from
//! the page, so every list and text is bounded here.

use rb_plugin_api::{Failure, Finding, PluginHost, Severity, ViewId, ViewInfo, codes};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::budget;
use crate::report::{MEASURE_JS, Rect};
use crate::settings;
use crate::tab_compare::{self, Stop};
use crate::tools;

/// The most stops a view's list keeps (the page sends at most as many).
pub const MAX_ITEMS: usize = 250;
/// The longest selector or path kept, in characters.
pub const MAX_TEXT_CHARS: usize = 300;
/// The warnings a stop may carry.
pub const WARNINGS: &[&str] = &[
    "positive-tabindex",
    "invisible-size",
    "invisible-opacity",
    "invisible-offscreen",
    "invisible-clipped",
    "reverse-jump",
    "visual-order",
];

/// The expression `evaluate` runs: the tab order of the view's page, an object.
#[must_use]
pub fn script(include_iframes: bool) -> String {
    let options = json!({ "includeIframes": include_iframes });
    format!("(() => {{\n{MEASURE_JS}\nreturn __rbOverlaysMeasure.tabOrder({options});\n}})()")
}

/// One stop of a view's tab order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TabItem {
    /// Its place in the order, from 1.
    pub index: u32,
    /// A short selector; ` >>> ` steps into a shadow root or an iframe.
    pub selector: String,
    /// Its full path: the same element has the same path in every view.
    pub path: String,
    pub tabindex: i64,
    /// Its box in document px.
    pub rect: Rect,
    /// Of [`WARNINGS`].
    pub warnings: Vec<String>,
    /// A cross-origin iframe (or any iframe, with iframes off): one stop for all of it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub iframe: bool,
}

/// One view's tab order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TabList {
    /// Every stop, also those past [`MAX_ITEMS`].
    pub count: u64,
    pub items: Vec<TabItem>,
}

fn clean(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_TEXT_CHARS)
        .collect()
}

fn finite(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

impl TabList {
    /// The list in `json` (an object, or the same as JSON text); bounded.
    #[must_use]
    pub fn parse(json: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(json).ok()?;
        let value = match value {
            Value::String(text) => serde_json::from_str(&text).ok()?,
            other => other,
        };
        if !value.is_object() {
            return None;
        }
        let mut list: Self = serde_json::from_value(value).ok()?;
        list.items.truncate(MAX_ITEMS);
        for item in &mut list.items {
            item.selector = clean(&item.selector);
            item.path = clean(&item.path);
            item.warnings
                .retain(|warning| WARNINGS.contains(&warning.as_str()));
            item.warnings.dedup();
            item.rect = Rect {
                x: finite(item.rect.x),
                y: finite(item.rect.y),
                width: finite(item.rect.width),
                height: finite(item.rect.height),
            };
        }
        Some(list)
    }

    /// The stops as the comparison takes them.
    #[must_use]
    pub fn stops(&self) -> Vec<Stop> {
        self.items
            .iter()
            .map(|item| Stop {
                path: item.path.clone(),
                selector: item.selector.clone(),
                index: item.index,
            })
            .collect()
    }
}

/// The findings of `list` in `view`: `tab-positive-tabindex` (label: the tabindex) and
/// `tab-invisible-focusable` (label: why it is unseen: size, opacity, offscreen, clipped).
#[must_use]
pub fn findings(view: &ViewInfo, list: &TabList) -> Vec<Finding> {
    let mut found = Vec::new();
    for item in &list.items {
        for warning in &item.warnings {
            let (rule, label) = if warning == "positive-tabindex" {
                ("tab-positive-tabindex", item.tabindex.to_string())
            } else if let Some(reason) = warning.strip_prefix("invisible-") {
                ("tab-invisible-focusable", reason.to_owned())
            } else {
                continue;
            };
            found.push(Finding {
                rule: rule.to_owned(),
                severity: Severity::Warning,
                message: format!("plugin.overlays.findings.{}", rule.replace('-', "_")),
                params: vec![("label".to_owned(), label)],
                view: Some(view.id),
                url: (!view.url.is_empty()).then(|| view.url.clone()),
                selector: (!item.selector.is_empty()).then(|| item.selector.clone()),
                path: (!item.path.is_empty()).then(|| item.path.clone()),
                data: None,
            });
        }
    }
    found
}

/// The tab order of each of `views`: the answer's `{ views, differences }`, and the lists that
/// were measured (whole: the comparison and the findings take every stop, the answer lists those
/// that fit).
pub fn measure(
    host: &mut dyn PluginHost,
    views: &[ViewInfo],
    include_iframes: bool,
) -> (Value, Vec<(ViewInfo, TabList)>) {
    let script = script(include_iframes);
    let outcomes: Vec<Result<TabList, String>> = views
        .iter()
        .map(|view| match host.evaluate(view.id, &script) {
            Ok(answer) => TabList::parse(&answer).ok_or_else(|| codes::BAD_OUTPUT.to_owned()),
            Err(failure) => Err(failure.code().unwrap_or(codes::HOST_FAILED).to_owned()),
        })
        .collect();
    let items: Vec<&[TabItem]> = outcomes
        .iter()
        .map(|outcome| {
            outcome
                .as_ref()
                .map_or(&[][..], |list| list.items.as_slice())
        })
        .collect();
    let shown = budget::share(&items, MAX_ITEMS, budget::TAB_BYTES);
    let mut entries = Vec::new();
    let mut lists = Vec::new();
    for ((view, outcome), shown) in views.iter().zip(outcomes).zip(shown) {
        let mut entry = Map::new();
        entry.insert("view".to_owned(), json!(view.id));
        entry.insert("label".to_owned(), json!(view.label));
        entry.insert("breakpoint".to_owned(), json!(view.breakpoint));
        match outcome {
            Ok(list) => {
                let listed = &list.items[..shown.min(list.items.len())];
                entry.insert("count".to_owned(), json!(list.count));
                entry.insert("shown".to_owned(), json!(listed.len()));
                entry.insert("items".to_owned(), json!(listed));
                lists.push((view.clone(), list));
            }
            Err(code) => {
                entry.insert("error".to_owned(), json!(code));
            }
        }
        entries.push(Value::Object(entry));
    }
    let stops: Vec<(ViewId, Vec<Stop>)> = lists
        .iter()
        .map(|(view, list)| (view.id, list.stops()))
        .collect();
    let compared: Vec<(ViewId, &[Stop])> = stops
        .iter()
        .map(|(view, stops)| (*view, stops.as_slice()))
        .collect();
    let differences = tab_compare::compare(&compared);
    (
        json!({ "views": entries, "differences": differences }),
        lists,
    )
}

/// The overlay switched on in `views`: the answer's `tabOrder`, the warnings reported as
/// findings while `reportFindings` is on.
pub fn switched_on(host: &mut dyn PluginHost, views: &[ViewInfo]) -> Value {
    let include_iframes = settings::page_settings(host).tab_order_include_iframes;
    let (answer, lists) = measure(host, views, include_iframes);
    let found: Vec<Finding> = lists
        .iter()
        .flat_map(|(view, list)| findings(view, list))
        .collect();
    if !found.is_empty()
        && settings::report_findings(host)
        && let Err(failure) = host.report_findings(&found)
    {
        let code = failure.code().unwrap_or(codes::HOST_FAILED).to_owned();
        host.log(
            "warn",
            &format!("overlays: the tab order findings were not reported ({code})"),
        );
    }
    answer
}

/// The panel's post of `answer`'s differences.
#[must_use]
pub fn panel_post(answer: &Value) -> Option<Value> {
    let differences = answer.get("differences")?;
    Some(
        json!({ "target": "panel", "message": { "type": "tab-order", "differences": differences } }),
    )
}

/// `tab_order`.
///
/// # Errors
///
/// Arguments that are no object, an `includeIframes` that is no boolean; the views cannot be
/// listed for every view.
pub fn tool(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args = tools::object(args)?;
    let include_iframes = match args.get("includeIframes") {
        None => settings::page_settings(host).tab_order_include_iframes,
        Some(value) => value.as_bool().ok_or_else(tools::bad_args)?,
    };
    let target = tools::target_of(host, &args);
    let views = tools::target_views(host, target)?;
    let (mut answer, _) = measure(host, &views, include_iframes);
    if let Some(post) = panel_post(&answer)
        && let Some(fields) = answer.as_object_mut()
    {
        fields.insert("$post".to_owned(), json!([post]));
    }
    Ok(answer.to_string())
}

/// A page script's focus in live mode: posted to every view while `tabOrder.live` is on.
///
/// # Errors
///
/// A path that is empty, no text, too long or holds control characters.
pub fn focus(host: &mut dyn PluginHost, view: ViewId, message: &Value) -> Result<String, Failure> {
    let path = message
        .get("path")
        .and_then(Value::as_str)
        .filter(|path| {
            !path.is_empty()
                && path.chars().count() <= MAX_TEXT_CHARS
                && !path.chars().any(char::is_control)
        })
        .ok_or_else(tools::bad_args)?;
    if !settings::page_settings(host).tab_order_live {
        return Ok("{}".to_owned());
    }
    let post = json!({ "type": "tab-focus", "view": view, "path": path });
    Ok(json!({ "$post": [{ "target": "all", "message": post }] }).to_string())
}

#[cfg(test)]
#[path = "tab_order_tests.rs"]
mod tests;
