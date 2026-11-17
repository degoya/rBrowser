//! A run: every target view's fields read by the page script (`src/page.js`, `collect`), planned
//! from one seed (`src/plan.rs`) and set by the page script (`apply`) in the plugin's isolated
//! world. Each view is filled by its own script - the values are the same because the seed and
//! the field keys are - and the sync agent relays no part of it: it relays trusted input only,
//! and a script's events are not trusted, so nothing arrives twice. The page script never submits.
//! Firefox and WebKit views have no `evaluate` yet and are reported `unsupported`.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewInfo, codes};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::args::{Args, Mode, Target, bad};
use crate::field::{Collected, cut};
use crate::key;
use crate::plan::{Options, Planned, plan};
use crate::settings::Settings;

/// The page script: one function expression called with the request.
const PAGE_JS: &str = include_str!("page.js");
/// The longest value an answer shows, in characters.
const MAX_SHOWN: usize = 200;
/// The largest answer posted to the panel (a post may be 64 KiB).
const MAX_POST: usize = 60_000;

/// The expression that runs the page script with `request`.
#[must_use]
pub fn script(request: &Value) -> String {
    // The request is JSON text: data, never code, inside the expression.
    format!("(\n{PAGE_JS}\n)({request})")
}

/// What `apply` says about one field.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AppliedField {
    index: u32,
    status: Value,
    value: Value,
    valid: Value,
    errors: Value,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Applied {
    fields: Vec<AppliedField>,
    form_valid: Value,
    blocked_submits: Value,
}

fn applied(answer: &str) -> Applied {
    serde_json::from_str(answer).unwrap_or_default()
}

fn errors(value: &Value) -> Value {
    let names: Vec<String> = value
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .take(12)
                .map(|name| cut(name, 40))
                .collect()
        })
        .unwrap_or_default();
    json!(names)
}

fn field_result(planned: &Planned, applied: Option<&AppliedField>) -> Value {
    let shown = applied
        .and_then(|entry| entry.value.as_str().map(str::to_owned))
        .or_else(|| planned.fill.as_ref().map(crate::value::Fill::shown))
        .unwrap_or_default();
    json!({
        "key": planned.key,
        "label": planned.label,
        "kind": planned.kind.id(),
        "value": cut(&shown, MAX_SHOWN),
        "violation": planned.violation.map(|violation| violation.id()),
        "profile": planned.profile,
        "required": planned.required,
        "form": planned.form,
        "skip": planned.skip,
        "status": applied.and_then(|entry| entry.status.as_str()).map(|s| cut(s, 20)),
        "valid": applied.and_then(|entry| entry.valid.as_bool()),
        "errors": applied.map_or_else(|| json!([]), |entry| errors(&entry.errors)),
    })
}

fn view_head(view: &ViewInfo, status: &str) -> Value {
    json!({ "view": view.id, "label": view.label, "breakpoint": view.breakpoint,
            "engine": view.engine, "status": status, "fields": [] })
}

fn fill_view(
    host: &mut dyn PluginHost,
    view: &ViewInfo,
    request: &Value,
    options: &Options,
) -> Value {
    if view.engine != "chromium" {
        return view_head(view, "unsupported");
    }
    let mut collect = request.clone();
    collect["op"] = json!("collect");
    let page = match host
        .evaluate(view.id, &script(&collect))
        .and_then(|answer| Collected::parse(&answer))
    {
        Ok(page) => page,
        Err(failure) => {
            let mut head = view_head(view, "failed");
            head["error"] = json!(failure.code);
            return head;
        }
    };
    let mut head = view_head(
        view,
        if page.error.is_empty() {
            "filled"
        } else {
            &page.error
        },
    );
    head["locale"] = json!(page.locale);
    head["url"] = json!(cut(&page.url, 2048));
    head["forms"] = json!(page.forms);
    if !page.error.is_empty() {
        return head;
    }
    let planned = plan(&page, options);
    let actions: Vec<Value> = planned.iter().filter_map(Planned::action).collect();
    let mut apply = request.clone();
    apply["op"] = json!("apply");
    apply["actions"] = json!(actions);
    let done = match host.evaluate(view.id, &script(&apply)) {
        Ok(answer) => applied(&answer),
        Err(failure) => {
            head["status"] = json!("failed");
            head["error"] = json!(failure.code);
            Applied::default()
        }
    };
    if planned.is_empty() {
        head["status"] = json!("empty");
    }
    head["truncated"] = json!(page.truncated);
    head["formValid"] = json!(done.form_valid.as_bool());
    head["blockedSubmits"] = json!(done.blocked_submits.as_u64().unwrap_or(0));
    head["fields"] = planned
        .iter()
        .map(|entry| {
            let found = done.fields.iter().find(|field| field.index == entry.index);
            field_result(entry, found.filter(|_| entry.fill.is_some()))
        })
        .collect();
    head
}

/// The seed of the run: the argument's, the setting's, else a new one from the clock.
fn seed(host: &mut dyn PluginHost, args: &Args, settings: &Settings) -> u64 {
    args.seed
        .or((settings.seed != 0).then_some(settings.seed))
        .unwrap_or_else(|| 1 + host.now_unix_seconds() % 999_999)
}

/// Fills the target views; `mode` is the command's, else the arguments' (the tool's).
///
/// # Errors
///
/// `plugins.error.invalid` (no mode), `plugins.error.not_found` (no such view),
/// `plugin.form-autofill.errors.profile_unknown` (neither a settings profile nor a stored preset);
/// a view's own failure is its status.
pub fn fill(host: &mut dyn PluginHost, args: &Args, mode: Option<Mode>) -> Result<Value, Failure> {
    let mode = mode.or(args.mode).ok_or_else(|| bad("mode"))?;
    let settings = Settings::read(host);
    let strategy = args.strategy.unwrap_or(settings.strategy);
    let profile_name = args
        .profile
        .clone()
        .or_else(|| Some(settings.active_profile.clone()).filter(|name| !name.is_empty()));
    // A settings profile first, then a preset taken from a form (RB-090-70, `src/store.rs`).
    let profile = match (&profile_name, mode) {
        (Some(name), Mode::Valid) => Some(
            settings
                .profile(name)
                .cloned()
                .or_else(|| crate::store::find(host, name))
                .ok_or_else(|| {
                    Failure::coded(FailureKind::NotFound, &key("errors.profile_unknown"))
                        .with_param("name", name)
                })?,
        ),
        _ => None,
    };
    let seed = seed(host, args, &settings);
    let views: Vec<ViewInfo> = host
        .views()?
        .into_iter()
        .filter(|view| args.target == Target::All || args.target == Target::View(view.id))
        .collect();
    if let Target::View(id) = args.target
        && views.is_empty()
    {
        return Err(Failure::coded(FailureKind::NotFound, codes::NOT_FOUND)
            .with_param("view", id.to_string()));
    }
    let options = Options {
        mode,
        strategy,
        seed,
        profile: profile.clone(),
    };
    let request = json!({ "selector": args.selector, "skipHidden": settings.skip_hidden,
                          "hiddenInputs": settings.hidden_inputs });
    let total = u32::try_from(views.len()).unwrap_or(u32::MAX);
    let mut results = Vec::new();
    for (done, view) in views.iter().enumerate() {
        results.push(fill_view(host, view, &request, &options));
        host.report_progress(u32::try_from(done + 1).unwrap_or(total), total);
    }
    Ok(json!({
        "mode": mode.id(),
        "seed": seed,
        "strategy": (mode == Mode::Invalid).then(|| strategy.id()),
        "profile": profile.map(|profile| profile.name),
        "selector": args.selector,
        "views": results,
    }))
}

/// `answer` with a post of itself to the plugin's open panels (a notice when it is too large).
#[must_use]
pub fn answer_with_post(answer: Value) -> Value {
    let message = if answer.to_string().len() <= MAX_POST {
        json!({ "type": "filled", "result": answer })
    } else {
        json!({ "type": "filled", "tooLarge": true, "seed": answer["seed"], "mode": answer["mode"] })
    };
    let mut posted = answer;
    posted["$post"] = json!([{ "target": "panel", "message": message }]);
    posted
}

#[cfg(test)]
#[path = "fill_tests.rs"]
mod tests;
