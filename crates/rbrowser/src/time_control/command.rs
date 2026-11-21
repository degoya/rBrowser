//! The commands (RB-094-07): `emulation.clock` sets the page clock of every view - a time from
//! `time` and a `speed`, the real time again with `reset`, and from the shortcut the settings'
//! clock or, while one runs, the real time - and `overlays.animations` the animation rate of the
//! views of `target` (every view without one): a `rate`, a `step` of the stopped animations, and
//! from the shortcut the next of the settings' rates. Only Chromium views have either (CDP); the
//! answer names the others as `unsupported`, as the emulation extras do (RB-080-10). After each
//! change the activity bar shows what is changed; a view that failed is named in the status bar.

use std::collections::BTreeMap;

use rb_commands::CommandError;
use rb_config::EngineKind;
use rb_engine::{Target, ViewHost as _, ViewId};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};

use super::hooks::{Outcome, step};
use super::model::{
    AnimationRequest, millis_of, next_clock, next_rate, parse_animations, parse_clock, rates_of,
    step_ms,
};
use crate::control::Core;
use crate::host::Host;

/// The status bar after views failed; `{views}`, `{error}`.
pub const FAILED_KEY: &str = "emulation.clock.notice.failed";
/// The status bar for views on Firefox or WebKit, which keep their time and animations;
/// `{views}`.
pub const UNSUPPORTED_KEY: &str = "emulation.clock.notice.unsupported";
/// Why a view on another engine is left out.
const UNSUPPORTED: &str = "the clock and the animation rate are Chromium's (CDP)";

/// `emulation.clock`.
///
/// # Errors
///
/// Arguments it cannot read, a broken clock setting, or no runtime to wait on.
pub fn clock(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let now = millis_of(time::OffsetDateTime::now_utc());
    let request = parse_clock(args, now).map_err(CommandError::InvalidArgs)?;
    let hooks = super::hooks();
    let defaults = core.user_settings().emulation.clock;
    let next = next_clock(&request, hooks.clock().as_ref(), &defaults, now)
        .map_err(CommandError::InvalidArgs)?;
    let cdp = core.host().cdp().clone();
    let outcomes =
        crate::analyzer::plugins::block_on(
            async move { super::hooks().set_clock(&cdp, next).await },
        )
        .map_err(|error| CommandError::Failed(error.to_string()))?;
    super::publish(core.host());
    let all = core
        .host()
        .target_views(Some(Target::All))
        .unwrap_or_default();
    let (_, unsupported) = split_by_engine(core.host(), &all);
    report(core, &outcomes, &unsupported);
    let mut answer = answer(&outcomes, &unsupported);
    answer["clock"] = next.map_or(Value::Null, |clock| json!(clock.activity()));
    Ok(answer)
}

/// `overlays.animations`.
///
/// # Errors
///
/// Arguments it cannot read, an unknown view, or no runtime to wait on.
pub fn animations(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let args = parse_animations(args).map_err(CommandError::InvalidArgs)?;
    let host = core.host();
    let views = host
        .target_views(Some(args.target.unwrap_or(Target::All)))
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
    let (chromium, unsupported) = split_by_engine(host, &views);
    let settings = core.user_settings().emulation;
    let answer = match args.request {
        AnimationRequest::Step(ms) => {
            let ms = step_ms(ms, &settings);
            let (outcomes, moved) = step_views(core, &chromium, ms)?;
            report(core, &outcomes, &unsupported);
            let mut answer = answer(&outcomes, &unsupported);
            answer["step"] = json!(ms);
            answer["moved"] = json!(moved);
            answer
        }
        request => {
            let rate = match request {
                AnimationRequest::Rate(rate) => rate,
                _ => {
                    let rates: BTreeMap<ViewId, f64> = super::hooks().rates().into_iter().collect();
                    let reference = host
                        .active_view()
                        .filter(|active| chromium.contains(active))
                        .or_else(|| chromium.first().copied());
                    let current = reference
                        .and_then(|view| rates.get(&view).copied())
                        .unwrap_or(1.0);
                    next_rate(current, &rates_of(&settings.animation_rates))
                }
            };
            let cdp = host.cdp().clone();
            let targets = chromium.clone();
            let outcomes = crate::analyzer::plugins::block_on(async move {
                super::hooks().set_rate(&cdp, &targets, rate).await
            })
            .map_err(|error| CommandError::Failed(error.to_string()))?;
            super::publish(host);
            report(core, &outcomes, &unsupported);
            let mut answer = answer(&outcomes, &unsupported);
            answer["rate"] = json!(rate);
            answer
        }
    };
    Ok(answer)
}

/// A view's capture session and the sync agent's context of its top document, when it has one.
type Page = (String, Option<i64>);

/// One step in each of `views`, in the sync agent's world of its top document where it has
/// one; the outcomes and how many animations moved in all.
fn step_views(
    core: &Core,
    views: &[ViewId],
    ms: u32,
) -> Result<(Vec<(ViewId, Outcome)>, u64), CommandError> {
    let host = core.host();
    let pages: Vec<(ViewId, Option<Page>)> = views
        .iter()
        .map(|view| {
            let session = host.capture_target(*view).map(|target| target.session);
            let context = host
                .sync_agents()
                .top_context(*view)
                .ok()
                .filter(|(agent, _)| Some(agent) == session.as_ref())
                .map(|(_, context)| context);
            (*view, session.map(|session| (session, context)))
        })
        .collect();
    let cdp = host.cdp().clone();
    crate::analyzer::plugins::block_on(async move {
        let mut outcomes = Vec::with_capacity(pages.len());
        let mut moved = 0;
        for (view, page) in pages {
            let outcome = match page {
                Some((session, context)) => match step(&cdp, &session, context, ms).await {
                    Ok(count) => {
                        moved += count;
                        Ok(())
                    }
                    Err(error) => Err(error),
                },
                None => Err(format!("view {view} is not attached yet")),
            };
            outcomes.push((view, outcome));
        }
        (outcomes, moved)
    })
    .map_err(|error| CommandError::Failed(error.to_string()))
}

/// `views` split into the Chromium views and the others.
fn split_by_engine(host: &Host, views: &[ViewId]) -> (Vec<ViewId>, Vec<ViewId>) {
    views.iter().copied().partition(|view| {
        host.view(*view)
            .is_none_or(|info| info.engine == EngineKind::Chromium)
    })
}

/// `{ views, failed: [{ view, error }], unsupported: [{ view, reason }] }`.
fn answer(outcomes: &[(ViewId, Outcome)], unsupported: &[ViewId]) -> Value {
    let done: Vec<ViewId> = outcomes
        .iter()
        .filter(|(_, outcome)| outcome.is_ok())
        .map(|(view, _)| *view)
        .collect();
    let failed: Vec<Value> = outcomes
        .iter()
        .filter_map(|(view, outcome)| {
            outcome
                .as_ref()
                .err()
                .map(|error| json!({ "view": view, "error": error }))
        })
        .collect();
    let unsupported: Vec<Value> = unsupported
        .iter()
        .map(|view| json!({ "view": view, "reason": UNSUPPORTED }))
        .collect();
    json!({ "views": done, "failed": failed, "unsupported": unsupported })
}

/// The status bar: the views that failed, the views on another engine.
fn report(core: &Core, outcomes: &[(ViewId, Outcome)], unsupported: &[ViewId]) {
    let failed: Vec<(ViewId, &String)> = outcomes
        .iter()
        .filter_map(|(view, outcome)| outcome.as_ref().err().map(|error| (*view, error)))
        .collect();
    if let Some((_, error)) = failed.first() {
        let views: Vec<ViewId> = failed.iter().map(|(view, _)| *view).collect();
        core.notify(notice(
            NotificationLevel::Warning,
            FAILED_KEY,
            &[
                ("views", names_of(core, &views)),
                ("error", (*error).clone()),
            ],
        ));
    }
    if !unsupported.is_empty() {
        core.notify(notice(
            NotificationLevel::Info,
            UNSUPPORTED_KEY,
            &[("views", names_of(core, unsupported))],
        ));
    }
}

/// The views' labels, comma-separated (the id for a view that is gone).
fn names_of(core: &Core, views: &[ViewId]) -> String {
    let host = core.host();
    views
        .iter()
        .map(|view| {
            host.view(*view)
                .map_or_else(|| view.to_string(), |info| info.label)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn notice(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
        settings_key: None,
    })
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
