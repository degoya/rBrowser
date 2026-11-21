//! A run: every target view's fields read by the page script (`src/page.js`, `collect`), planned
//! from one seed (`src/plan.rs`) and set by the page script (`apply`) in the plugin's isolated
//! world. Each view is filled by its own script - the values are the same because the seed and
//! the field keys are - and the sync agent relays no part of it: it relays trusted input only,
//! and a script's events are not trusted, so nothing arrives twice. The page script never submits.
//! For the same reason a recording would lack the fill: each view's set fields go to the recorder
//! as `host` posts beside the answer (`src/record.rs`, RB-090-78).
//! A view is filled in passes (`src/passes.rs`, RB-094-22): control fields first, a wait for the
//! page to settle after each one changed, then the fields that appeared or became enabled; a fill
//! takes at most `fillBudgetSeconds` in all, then stops its passes and views and says so.
//! Firefox and WebKit views have no `evaluate` yet and are reported `unsupported`.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewInfo, codes};
use serde_json::{Value, json};

use crate::args::{Args, Mode, Target, bad};
use crate::field::{Collected, cut};
use crate::key;
use crate::passes::{Applied, AppliedField, Outcome, Pacing, actions, todo};
use crate::plan::{Options, Planned, plan};
use crate::settings::Settings;

/// The page script: one function expression called with the request, `targetOf` and `settled`.
const PAGE_JS: &str = include_str!("page.js");
/// The sync agent's selector, a function expression that answers `targetOf` (RB-090-78).
const TARGET_JS: &str = include_str!("target.js");
/// The settle runner, a function expression that answers `settled` (RB-094-22).
const SETTLE_JS: &str = include_str!("settle.js");
/// The longest value an answer shows, in characters.
const MAX_SHOWN: usize = 200;
/// The largest answer posted to the panel (a post may be 64 KiB).
const MAX_POST: usize = 60_000;

/// The expression that runs the page script with `request`, the agent's `targetOf` and the
/// settle runner.
#[must_use]
pub fn script(request: &Value) -> String {
    // The request is JSON text: data, never code, inside the expression.
    format!("(\n{PAGE_JS}\n)({request}, (\n{TARGET_JS}\n)(), (\n{SETTLE_JS}\n)())")
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

/// The view's fields as the page script reads them.
fn collect(
    host: &mut dyn PluginHost,
    view: &ViewInfo,
    request: &Value,
) -> Result<Collected, Failure> {
    let mut collect = request.clone();
    collect["op"] = json!("collect");
    host.evaluate(view.id, &script(&collect))
        .and_then(|answer| Collected::parse(&answer))
}

fn fill_view(
    host: &mut dyn PluginHost,
    view: &ViewInfo,
    request: &Value,
    options: &Options,
    pacing: Pacing,
    deadline: u64,
) -> Value {
    if view.engine != "chromium" {
        return view_head(view, "unsupported");
    }
    if host.now_unix_seconds() >= deadline {
        return view_head(view, "timeout");
    }
    let mut page = match collect(host, view, request) {
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
    head["scope"] = json!(page.scope);
    if !page.error.is_empty() {
        return head;
    }
    head["truncated"] = json!(page.truncated);
    let mut outcome = Outcome::default();
    let mut apply = request.clone();
    apply["op"] = json!("apply");
    if let Some(settle) = pacing.settle() {
        apply["settle"] = settle;
    }
    loop {
        let first = outcome.passes == 0;
        let planned = plan(&page, options);
        outcome.note(&planned, first);
        let list = todo(&planned, &outcome.done, first);
        if list.is_empty() && !first {
            break;
        }
        apply["actions"] = json!(actions(&list));
        let done = match host.evaluate(view.id, &script(&apply)) {
            Ok(answer) => Applied::parse(&answer),
            Err(failure) => {
                head["status"] = json!("failed");
                head["error"] = json!(failure.code);
                break;
            }
        };
        let changed = outcome.absorb(&list, done);
        if !changed || outcome.passes >= pacing.max_passes {
            break;
        }
        if host.now_unix_seconds() >= deadline {
            head["stopped"] = json!("deadline");
            break;
        }
        // The page settled in `apply`; what it shows now is read again (a later pass).
        match collect(host, view, request) {
            Ok(next) if next.error.is_empty() => page = next,
            _ => break,
        }
    }
    if outcome.is_empty() {
        head["status"] = json!("empty");
    }
    head["passes"] = json!(outcome.passes);
    head["formValid"] = json!(outcome.form_valid.as_bool());
    head["blockedSubmits"] = json!(outcome.blocked);
    head["fields"] = outcome
        .entries()
        .map(|(entry, found)| field_result(entry, found.filter(|_| entry.fill.is_some())))
        .collect();
    if !outcome.record.is_empty() {
        head["record"] = json!(outcome.record);
    }
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
        tick_consent: settings.tick_consent,
    };
    let request = json!({ "selector": args.selector, "skipHidden": settings.skip_hidden,
                          "hiddenInputs": settings.hidden_inputs });
    let pacing = Pacing {
        settle_ms: settings.settle_ms,
        max_passes: settings.max_passes,
    };
    let total = u32::try_from(views.len()).unwrap_or(u32::MAX);
    let deadline = host
        .now_unix_seconds()
        .saturating_add(u64::from(settings.fill_budget_seconds));
    let mut results = Vec::new();
    for (done, view) in views.iter().enumerate() {
        results.push(fill_view(host, view, &request, &options, pacing, deadline));
        host.report_progress(u32::try_from(done + 1).unwrap_or(total), total);
    }
    let out_of_time = results
        .iter()
        .any(|view| view["status"] == "timeout" || view["stopped"] == "deadline");
    Ok(json!({
        "mode": mode.id(),
        "seed": seed,
        "strategy": (mode == Mode::Invalid).then(|| strategy.id()),
        "profile": profile.map(|profile| profile.name),
        "selector": args.selector,
        "outOfTime": out_of_time,
        "views": results,
    }))
}

/// `answer` with a post of itself to the plugin's open panels (a notice when it is too large),
/// and its views' set fields for the recorder taken out into `host` posts (`src/record.rs`).
#[must_use]
pub fn answer_with_post(mut answer: Value) -> Value {
    let records = crate::record::take_posts(&mut answer);
    let message = if answer.to_string().len() <= MAX_POST {
        json!({ "type": "filled", "result": answer })
    } else {
        json!({ "type": "filled", "tooLarge": true, "seed": answer["seed"], "mode": answer["mode"] })
    };
    let mut posted = answer;
    let mut posts = vec![json!({ "target": "panel", "message": message })];
    posts.extend(records);
    posted["$post"] = json!(posts);
    posted
}

#[cfg(test)]
#[path = "fill_tests.rs"]
mod tests;
