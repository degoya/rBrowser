//! The plugin's MCP tools and commands: `coverage_get` (`mode: load|delta|last`),
//! `coverage_start`, `coverage_stop`, `coverage_file`, and the commands `run_load`, `start`,
//! `stop` (each its tool, the findings into the findings panel, the panel opened) and `export`.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, FailureKind, Finding, PluginHost, ViewId, ViewInfo, codes};
use serde_json::{Value, json};

use crate::answer::summary;
use crate::detail;
use crate::export;
use crate::findings;
use crate::key;
use crate::measure;
use crate::model::{Kind, Mode, Run};
use crate::resolve::Cache;
use crate::settings::Settings;
use crate::store::{self, Session};
use crate::{OPEN_KEY, PANEL_ID};

fn invalid(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
}

fn own(name: &str) -> Failure {
    Failure::coded(FailureKind::Other, &key(&format!("errors.{name}")))
}

/// The arguments: a JSON object, `{}` for an empty text.
///
/// # Errors
///
/// `plugins.error.invalid` for anything else.
pub fn parse(args: &str) -> Result<Value, Failure> {
    if args.trim().is_empty() {
        return Ok(json!({}));
    }
    match serde_json::from_str::<Value>(args) {
        Ok(value) if value.is_object() => Ok(value),
        _ => Err(invalid("args")),
    }
}

/// The views `target` names: `"all"` or none for every view, a view id for that one.
///
/// # Errors
///
/// `plugins.error.not_found` for a view the window does not have, `plugins.error.invalid` for
/// any other target.
pub fn targets(host: &mut dyn PluginHost, target: &Value) -> Result<Vec<ViewInfo>, Failure> {
    let views = host.views()?;
    match target {
        Value::Null => Ok(views),
        Value::String(all) if all == "all" => Ok(views),
        Value::Number(number) => {
            let id = number
                .as_u64()
                .and_then(|id| ViewId::try_from(id).ok())
                .ok_or_else(|| invalid("target"))?;
            views
                .into_iter()
                .find(|view| view.id == id)
                .map(|view| vec![view])
                .ok_or_else(|| {
                    Failure::coded(FailureKind::NotFound, codes::NOT_FOUND).with_param("view", id)
                })
        }
        _ => Err(invalid("target")),
    }
}

/// The stored texts of `run`'s style sheets by source key, for the findings' selectors.
pub fn texts(host: &mut dyn PluginHost, run: &Run) -> BTreeMap<String, String> {
    let mut texts = BTreeMap::new();
    for view in &run.views {
        for file in view.files.iter().filter(|file| file.kind == Kind::Css) {
            if let Some(key) = &file.source
                && !texts.contains_key(key)
                && let Some(text) = store::read_source(host, key)
            {
                texts.insert(key.clone(), text);
            }
        }
    }
    texts
}

fn run_findings(host: &mut dyn PluginHost, run: &Run, settings: &Settings) -> Vec<Finding> {
    let texts = texts(host, run);
    findings::findings(run, settings, &[], None, &texts)
}

fn finish(host: &mut dyn PluginHost, mut run: Run, settings: &Settings) -> (Run, Vec<Finding>) {
    run.running = false;
    run.finished = host.now_unix_seconds();
    if store::save_last(host, &run).is_err() {
        host.log("warn", "coverage: last.json could not be written");
    }
    let found = run_findings(host, &run, settings);
    (run, found)
}

/// A load run over the views of `args.target`.
fn load(
    host: &mut dyn PluginHost,
    args: &Value,
    settings: &Settings,
) -> Result<(Run, Vec<Finding>), Failure> {
    let now = host.now_unix_seconds();
    if store::session(host).active(now).is_some() {
        return Err(own("running"));
    }
    let views = targets(host, &args["target"])?;
    store::save_session(
        host,
        &Session {
            running: Some(Mode::Load),
            started: now,
            run: Run::default(),
        },
    )?;
    let measured = measure::load(host, &views, settings);
    store::clear_session(host);
    let run = Run {
        mode: Mode::Load,
        started: now,
        views: measured,
        ..Run::default()
    };
    Ok(finish(host, run, settings))
}

/// What the interaction session has used so far, tracking on.
fn delta(host: &mut dyn PluginHost, settings: &Settings) -> Result<(Run, Vec<Finding>), Failure> {
    let mut session = store::session(host);
    if session.running != Some(Mode::Interaction) {
        return Err(own("not-running"));
    }
    let mut cache = Cache::default();
    let mut take = Run::default();
    for view in &session.run.views {
        take.views
            .push(measure::take(host, view, false, settings, &mut cache));
    }
    take.finished = host.now_unix_seconds();
    session.run.absorb(&take);
    store::save_session(host, &session)?;
    let found = run_findings(host, &session.run, settings);
    Ok((session.run, found))
}

/// `coverage_get`.
///
/// # Errors
///
/// A bad mode or target, a run already going on (`errors.running`), `delta` without an
/// interaction session (`errors.not-running`), `last` without a measurement (`errors.none`).
pub fn get(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let settings = Settings::read(host);
    let mode = match args["mode"].as_str() {
        Some("load") => "load",
        Some("delta") => "delta",
        Some("last") => "last",
        None => match settings.default_mode {
            Mode::Load => "load",
            Mode::Interaction => "delta",
        },
        Some(_) => return Err(invalid("mode")),
    };
    let (run, found) = match mode {
        "load" => load(host, args, &settings)?,
        "delta" => delta(host, &settings)?,
        _ => {
            let session = store::session(host);
            let now = host.now_unix_seconds();
            let run = match session.active(now) {
                Some(Mode::Interaction) => session.run,
                _ => store::last(host).ok_or_else(|| own("none"))?,
            };
            let found = run_findings(host, &run, &settings);
            (run, found)
        }
    };
    Ok(summary(&run, &found))
}

/// `coverage_start`: an interaction session over the views of `args.target`; views not yet in a
/// running session join it.
///
/// # Errors
///
/// A bad target, a load run going on, no Chromium view to track.
pub fn start(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let now = host.now_unix_seconds();
    let mut session = store::session(host);
    if session.active(now) == Some(Mode::Load) {
        return Err(own("running"));
    }
    if session.running != Some(Mode::Interaction) {
        session = Session {
            running: Some(Mode::Interaction),
            started: now,
            run: Run {
                mode: Mode::Interaction,
                started: now,
                running: true,
                ..Run::default()
            },
        };
    }
    let views: Vec<ViewInfo> = targets(host, &args["target"])?
        .into_iter()
        .filter(|view| !session.run.views.iter().any(|known| known.view == view.id))
        .collect();
    let started = measure::start(host, &views);
    session.run.views.extend(started);
    if !session.run.measured().any(|_| true) {
        return Err(Failure::coded(
            FailureKind::NotSupported,
            codes::NOT_AVAILABLE,
        ));
    }
    if let Err(failure) = store::save_session(host, &session) {
        // Without the session nothing could stop it later: stop now.
        for view in session.run.measured() {
            measure::stop(host, view.view);
        }
        return Err(failure);
    }
    Ok(summary(&session.run, &[]))
}

/// `coverage_stop`: the session's last take, tracking stopped in every view, the result stored
/// as the last measurement. Without a session, the last measurement.
///
/// # Errors
///
/// No session and no measurement (`errors.none`).
pub fn stop(host: &mut dyn PluginHost) -> Result<(Value, Vec<Finding>), Failure> {
    let settings = Settings::read(host);
    let session = store::session(host);
    if session.running != Some(Mode::Interaction) {
        let run = store::last(host).ok_or_else(|| own("not-running"))?;
        let found = run_findings(host, &run, &settings);
        return Ok((summary(&run, &found), Vec::new()));
    }
    let mut run = session.run;
    let mut cache = Cache::default();
    let mut take = Run::default();
    for view in &run.views {
        take.views
            .push(measure::take(host, view, true, &settings, &mut cache));
    }
    run.absorb(&take);
    store::clear_session(host);
    let (run, found) = finish(host, run, &settings);
    Ok((summary(&run, &found), found))
}

/// `coverage_file`: one file of the session or the last measurement, a window at a time.
///
/// # Errors
///
/// No measurement, a bad or unknown URL.
pub fn file(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let session = store::session(host);
    let now = host.now_unix_seconds();
    let run = match session.active(now) {
        Some(Mode::Interaction) => session.run,
        _ => store::last(host).ok_or_else(|| own("none"))?,
    };
    detail::file(host, &run, args)
}

/// The command `export`: the last measurement into `coverage.json` and `coverage.csv`.
///
/// # Errors
///
/// No measurement, or the host's failure to write.
pub fn export(host: &mut dyn PluginHost) -> Result<Value, Failure> {
    let run = store::last(host).ok_or_else(|| own("none"))?;
    let written = export::write(host, &run)?;
    let paths: Vec<String> = written
        .iter()
        .map(|name| format!("{}{name}", store::OUT))
        .collect();
    Ok(json!({ "written": paths }))
}

/// Reports `found` to the findings panel; a host without it is logged, not raised.
pub fn report(host: &mut dyn PluginHost, found: &[Finding]) {
    if let Err(failure) = host.report_findings(found) {
        host.log(
            "warn",
            &format!(
                "coverage: findings not reported ({})",
                failure.code().unwrap_or_default()
            ),
        );
    }
}

/// A command's answer: the panel opened and told to refresh, `extra` kept.
#[must_use]
pub fn opened(extra: Value) -> Value {
    let mut answer = if extra.is_object() { extra } else { json!({}) };
    answer[OPEN_KEY] = json!({ "panel": PANEL_ID });
    answer["$post"] = json!([{ "target": "panel", "message": { "refresh": true } }]);
    answer
}

/// The command `run_load`: a load run over every view, its findings reported.
///
/// # Errors
///
/// As [`get`].
pub fn run_load(host: &mut dyn PluginHost) -> Result<Value, Failure> {
    let settings = Settings::read(host);
    let (run, found) = load(host, &json!({}), &settings)?;
    report(host, &found);
    Ok(opened(counts(&run, found.len())))
}

/// The views, files and findings of a run, for a command's short answer.
#[must_use]
pub fn counts(run: &Run, findings: usize) -> Value {
    let files: usize = run.views.iter().map(|view| view.files.len()).sum();
    json!({ "views": run.views.len(), "files": files, "findings": findings })
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
