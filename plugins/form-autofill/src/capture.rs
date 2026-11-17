//! "Take this form as a preset" (RB-090-70): the page script's `capture` reads the current values
//! of one form of one view - the form the request's `selector` names, else the focused field's
//! (a right-click on a field focuses it), else the page's only form, else every field - and this
//! side turns them into the entries of a preset: one per field, by the locator the fill looks a
//! profile value up by (`src/plan.rs`: the field's name, else its id). The panel shows them with a
//! box each and saves the ones chosen (`src/store.rs`); nothing is stored here.
//!
//! A password field, a field whose hints name a secret (`src/secret.rs`) and a file input never
//! give their value: the page script leaves it in the page, and this side drops it again should
//! one arrive. `type=hidden` inputs are read only with `captureHiddenInputs`.

use std::collections::BTreeSet;

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, ViewInfo, codes};
use serde_json::{Map, Value, json};

use crate::args::{Target, bad};
use crate::classify::classify;
use crate::field::{Field, MAX_FIELDS, cut};
use crate::fill::script;
use crate::key;
use crate::settings::{MAX_VALUE, Settings};
use crate::store::{self, MAX_NAME};

/// The panel the command opens.
pub const PANEL: &str = "autofill";
/// Why the page script gives no value; anything else it says is read as no reason.
const PAGE_REASONS: &[&str] = &["password", "secret", "file", "multiple", "long"];
/// The reasons that keep a value from ever being shown.
const NEVER_SHOWN: &[&str] = &["password", "secret", "file"];

/// The arguments of `autofill_capture` and the command `capture`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CaptureArgs {
    /// A view id; `None` (no target, or `"all"`) is the active view.
    pub view: Option<ViewId>,
    pub selector: Option<String>,
    /// The panel asks for the command's request: it is taken (and cleared) first.
    pub pending: bool,
}

impl CaptureArgs {
    /// # Errors
    ///
    /// `plugins.error.invalid` with `reason` naming the argument that does not read.
    pub fn parse(args: &str) -> Result<Self, Failure> {
        let value: Value = if args.trim().is_empty() {
            Value::Object(Map::new())
        } else {
            serde_json::from_str(args).map_err(|_| bad("args"))?
        };
        let map = value.as_object().ok_or_else(|| bad("args"))?;
        let view =
            match crate::args::Args::parse(&json!({ "target": map.get("target") }).to_string())
                .map_err(|_| bad("target"))?
                .target
            {
                Target::All => None,
                Target::View(id) => Some(id),
            };
        let selector = match map.get("selector") {
            None | Some(Value::Null) => None,
            Some(Value::String(text)) if text.chars().count() <= crate::args::MAX_SELECTOR => {
                Some(text.trim().to_owned()).filter(|text| !text.is_empty())
            }
            Some(_) => return Err(bad("selector")),
        };
        let pending = match map.get("pending") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(pending)) => *pending,
            Some(_) => return Err(bad("pending")),
        };
        Ok(Self {
            view,
            selector,
            pending,
        })
    }
}

/// The view `wanted` names, else the active one, else the first.
fn view_of(host: &mut dyn PluginHost, wanted: Option<ViewId>) -> Result<ViewInfo, Failure> {
    let views = host.views()?;
    let id = match wanted {
        Some(id) => Some(id),
        None => host
            .active_view()?
            .or_else(|| views.first().map(|view| view.id)),
    };
    let missing = || {
        Failure::coded(FailureKind::NotFound, codes::NOT_FOUND)
            .with_param("view", id.map_or_else(String::new, |id| id.to_string()))
    };
    let id = id.ok_or_else(missing)?;
    views
        .into_iter()
        .find(|view| view.id == id)
        .ok_or_else(missing)
}

/// The command `capture`: notes the view for the panel and opens it; the panel captures.
///
/// # Errors
///
/// `plugins.error.not_found` when there is no such view.
pub fn command(host: &mut dyn PluginHost, args: &CaptureArgs) -> Result<Value, Failure> {
    let view = view_of(host, args.view)?;
    if let Err(failure) = store::request_capture(host, view.id) {
        // No project: the panel that is open hears the post; one that opens now cannot.
        host.log(
            "debug",
            &format!("capture request not noted: {:?}", failure.code),
        );
    }
    Ok(json!({
        "view": view.id,
        "$open": { "panel": PANEL },
        "$post": [{ "target": "panel", "message": { "type": "capture", "view": view.id } }],
    }))
}

/// The name a preset of `page` is offered under: the form's, the page's title or its host.
#[must_use]
pub fn suggested_name(page: &Value) -> String {
    let url = page["url"].as_str().unwrap_or_default();
    let host = url.split_once("://").map_or(url, |(_, rest)| {
        rest.split(['/', '?', '#']).next().unwrap_or(rest)
    });
    [
        page["form"]["name"].as_str().unwrap_or_default(),
        page["title"].as_str().unwrap_or_default(),
        host,
    ]
    .into_iter()
    .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
    .find(|text| !text.is_empty())
    .map(|text| cut(&text, MAX_NAME))
    .unwrap_or_default()
}

/// One entry of the answer, from the page script's field.
fn entry(raw: &Value, taken: &mut BTreeSet<String>) -> Value {
    let field: Field = serde_json::from_value(raw.clone()).unwrap_or_default();
    let locator = [&field.name, &field.id]
        .into_iter()
        .find(|text| !text.is_empty())
        .cloned()
        .unwrap_or_default();
    let page_reason = raw["excluded"]
        .as_str()
        .and_then(|reason| PAGE_REASONS.iter().find(|known| **known == reason).copied());
    let excluded = page_reason
        .filter(|reason| NEVER_SHOWN.contains(reason))
        .or_else(|| crate::secret::reason(&field))
        .or(page_reason)
        .or_else(|| (field.control == "file").then_some("file"))
        .or_else(|| locator.is_empty().then_some("no-locator"))
        .or_else(|| (!taken.insert(locator.clone())).then_some("duplicate"));
    let value = raw["value"].as_str().map(|text| cut(text, MAX_VALUE));
    let mut out = json!({
        "locator": locator,
        "label": crate::plan::label(&field),
        "kind": classify(&field).id(),
        "control": field.control,
        "form": field.form,
        "selected": excluded.is_none() && value.as_deref().is_some_and(|text| !text.is_empty()),
    });
    if let Some(reason) = excluded {
        out["excluded"] = json!(reason);
    }
    if let Some(value) =
        value.filter(|_| !excluded.is_some_and(|reason| NEVER_SHOWN.contains(&reason)))
    {
        out["value"] = json!(value);
    }
    out
}

/// Reads the form of the view `args` names (or the pending request's) and answers its entries.
///
/// # Errors
///
/// `plugins.error.not_found` (no such view, or nothing matches `selector`),
/// `plugins.error.invalid` (`selector` is not valid),
/// `plugin.form-autofill.errors.capture_unsupported` (a Firefox or WebKit view); the host's
/// failure of `evaluate`.
pub fn capture(host: &mut dyn PluginHost, args: &CaptureArgs) -> Result<Value, Failure> {
    let requested = if args.pending {
        store::take_request(host)
    } else {
        None
    };
    if args.pending && args.view.is_none() && requested.is_none() {
        return Ok(json!({ "pending": false }));
    }
    let view = view_of(host, args.view.or(requested))?;
    if view.engine != "chromium" {
        return Err(Failure::coded(
            FailureKind::NotSupported,
            &key("errors.capture_unsupported"),
        )
        .with_param("engine", &view.engine));
    }
    let settings = Settings::read(host);
    let request = json!({
        "op": "capture",
        "selector": args.selector,
        "hiddenInputs": settings.hidden_inputs,
        "skipHidden": false,
    });
    let answer = host.evaluate(view.id, &script(&request))?;
    let page: Value = serde_json::from_str(&answer)
        .ok()
        .filter(Value::is_object)
        .ok_or_else(|| Failure::coded(FailureKind::BadData, codes::BAD_OUTPUT))?;
    let selector = args.selector.clone().unwrap_or_default();
    match page["error"].as_str() {
        Some("selector") => return Err(bad("selector")),
        Some("not-found") => {
            return Err(Failure::coded(FailureKind::NotFound, codes::NOT_FOUND)
                .with_param("selector", selector));
        }
        _ => {}
    }
    let mut taken = BTreeSet::new();
    let fields: Vec<Value> = page["fields"]
        .as_array()
        .map(|list| {
            list.iter()
                .take(MAX_FIELDS)
                .map(|raw| entry(raw, &mut taken))
                .collect()
        })
        .unwrap_or_default();
    // No project, or a file that does not read whole: the dialog names which (review finding 3).
    let (presets, store_error) = match store::read(host) {
        Ok(presets) => (presets, Value::Null),
        Err(failure) => (Vec::new(), store::failure_json(&failure)),
    };
    let form = page["form"].as_object().map(|form| {
        json!({
            "selector": cut(form.get("selector").and_then(Value::as_str).unwrap_or_default(), 1024),
            "name": cut(form.get("name").and_then(Value::as_str).unwrap_or_default(), 200),
        })
    });
    let scope = page["scope"].as_str().unwrap_or("document");
    Ok(json!({
        "view": view.id,
        "label": view.label,
        "pending": requested.is_some(),
        "url": cut(page["url"].as_str().unwrap_or_default(), 2048),
        "scope": cut(scope, 20),
        "form": form,
        "name": suggested_name(&page),
        "fields": fields,
        "truncated": page["truncated"].as_bool().unwrap_or(false),
        "presets": presets.iter().map(|preset| preset.name.clone()).collect::<Vec<_>>(),
        "profiles": settings.profiles.iter().map(|profile| profile.name.clone()).collect::<Vec<_>>(),
        "available": store_error.is_null(),
        "storeError": store_error,
    }))
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
