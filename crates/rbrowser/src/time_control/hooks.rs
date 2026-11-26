//! The page clock and the animation rates over CDP (RB-094-07), per view and free of the window:
//! each view's page session as the host attached it, the clock's script registered there, its
//! animation rate. The clock is one for every view: set, it goes into each attached view at once
//! (`Page.addScriptToEvaluateOnNewDocument` in the main world with `runImmediately`, so the
//! document there reads it too and every later one before its scripts) and into each view
//! attached later; reset, the script goes and the document there gets the real time back. A rate
//! is the document timeline's (`Animation.setPlaybackRate`), which the session keeps for the
//! documents that follow; a new session (a reconnect) gets it again. What the browser refuses is
//! that view's failure; the wish stays and goes again with the next attach.

use std::collections::BTreeMap;
use std::sync::Mutex;

use rb_engine::ViewId;
use serde_json::{Value, json};

use super::model::{Clock, is_normal};
use super::script::{RATE_METHOD, RESET_EXPRESSION, install_source, rate_params, step_expression};
use crate::cdp::{CdpRouter, lock};

/// What became of one view: done, or why not.
pub type Outcome = Result<(), String>;

/// One view's part.
#[derive(Debug)]
struct ViewTime {
    /// The page session; `None` until the attach.
    session: Option<String>,
    /// The clock's script registered in `session`, by identifier.
    script: Option<String>,
    /// The animation rate: 1 normal, 0 stopped.
    rate: f64,
}

impl Default for ViewTime {
    fn default() -> Self {
        Self {
            session: None,
            script: None,
            rate: 1.0,
        }
    }
}

#[derive(Debug, Default)]
struct Inner {
    clock: Option<Clock>,
    views: BTreeMap<ViewId, ViewTime>,
}

/// The clock and the rates of the window's views.
#[derive(Debug, Default)]
pub struct TimeHooks {
    inner: Mutex<Inner>,
    /// One change at a time, so a later one never loses against an earlier one.
    serial: tokio::sync::Mutex<()>,
}

impl TimeHooks {
    /// `view` has a new page `session`: the clock and its rate go in, before its next
    /// document.
    pub async fn attach(&self, cdp: &CdpRouter, view: ViewId, session: &str) {
        let _serial = self.serial.lock().await;
        let (clock, rate) = {
            let mut inner = lock(&self.inner);
            let clock = inner.clock;
            let entry = inner.views.entry(view).or_default();
            entry.session = Some(session.to_owned());
            // The old script went with the old session.
            entry.script = None;
            (clock, entry.rate)
        };
        if let Some(clock) = clock {
            match add_script(cdp, session, &clock).await {
                Ok(identifier) => self.registered(view, session, Some(identifier)),
                Err(error) => tracing::warn!(%view, %error, "the page clock was not registered"),
            }
        }
        if !is_normal(rate)
            && let Err(error) = send_rate(cdp, session, rate).await
        {
            tracing::warn!(%view, %error, "the animation rate was not sent");
        }
    }

    /// Sets the clock of every view (`None`: the real time); one outcome per attached view, in
    /// view order.
    pub async fn set_clock(&self, cdp: &CdpRouter, clock: Option<Clock>) -> Vec<(ViewId, Outcome)> {
        let _serial = self.serial.lock().await;
        let views: Vec<(ViewId, String, Option<String>)> = {
            let mut inner = lock(&self.inner);
            inner.clock = clock;
            inner
                .views
                .iter()
                .filter_map(|(view, entry)| {
                    Some((*view, entry.session.clone()?, entry.script.clone()))
                })
                .collect()
        };
        let mut outcomes = Vec::with_capacity(views.len());
        for (view, session, script) in views {
            if let Some(identifier) = script {
                remove_script(cdp, &session, &identifier).await;
                self.registered(view, &session, None);
            }
            let outcome = match &clock {
                Some(clock) => match add_script(cdp, &session, clock).await {
                    Ok(identifier) => {
                        self.registered(view, &session, Some(identifier));
                        Ok(())
                    }
                    Err(error) => Err(error),
                },
                None => evaluate(cdp, &session, RESET_EXPRESSION, None)
                    .await
                    .map(|_| ()),
            };
            if let Err(error) = &outcome {
                tracing::warn!(%view, %error, "the page clock did not change");
            }
            outcomes.push((view, outcome));
        }
        outcomes
    }

    /// Sets the animation rate of `views`; a view not attached yet gets it at its attach. One
    /// outcome per view, in the order given.
    pub async fn set_rate(
        &self,
        cdp: &CdpRouter,
        views: &[ViewId],
        rate: f64,
    ) -> Vec<(ViewId, Outcome)> {
        let _serial = self.serial.lock().await;
        let sessions: Vec<(ViewId, Option<String>)> = {
            let mut inner = lock(&self.inner);
            views
                .iter()
                .map(|view| {
                    let entry = inner.views.entry(*view).or_default();
                    entry.rate = rate;
                    (*view, entry.session.clone())
                })
                .collect()
        };
        let mut outcomes = Vec::with_capacity(sessions.len());
        for (view, session) in sessions {
            let outcome = match session {
                Some(session) => send_rate(cdp, &session, rate).await,
                None => Ok(()),
            };
            if let Err(error) = &outcome {
                tracing::warn!(%view, %error, "the animation rate was not sent");
            }
            outcomes.push((view, outcome));
        }
        outcomes
    }

    /// `view` closed.
    pub fn forget(&self, view: ViewId) {
        lock(&self.inner).views.remove(&view);
    }

    /// Forgets every view `alive` says is gone - a follower that fell behind the host's events
    /// missed their `Closed` (RB-099-19); `true` when one was forgotten. `alive` is asked per
    /// view under the hooks' lock, so a view attached meanwhile is never pruned by a stale list;
    /// it must not call back into the hooks.
    pub fn forget_gone(&self, alive: impl Fn(ViewId) -> bool) -> bool {
        let mut inner = lock(&self.inner);
        let before = inner.views.len();
        inner.views.retain(|view, _| alive(*view));
        inner.views.len() != before
    }

    /// The views' clock; `None` while they have the real time.
    pub fn clock(&self) -> Option<Clock> {
        lock(&self.inner).clock
    }

    /// Every known view's rate, in view order.
    pub fn rates(&self) -> Vec<(ViewId, f64)> {
        lock(&self.inner)
            .views
            .iter()
            .map(|(view, entry)| (*view, entry.rate))
            .collect()
    }

    /// The rate held for the view of page `session`, unless it is normal: a capture's undo
    /// puts it back (`crate::capture::clean`).
    pub fn held_rate(&self, session: &str) -> Option<f64> {
        lock(&self.inner)
            .views
            .values()
            .find(|entry| entry.session.as_deref() == Some(session))
            .map(|entry| entry.rate)
            .filter(|rate| !is_normal(*rate))
    }

    /// Notes the script `identifier` (or none) as registered in `view`, unless the view has
    /// another session by now.
    fn registered(&self, view: ViewId, session: &str, identifier: Option<String>) {
        if let Some(entry) = lock(&self.inner).views.get_mut(&view)
            && entry.session.as_deref() == Some(session)
        {
            entry.script = identifier;
        }
    }
}

/// Registers the clock in `session`'s main world and runs it in the document there; its
/// identifier. A script for new documents runs only while the Page domain is on: on already
/// with the sync agent, here once more for a view without one.
async fn add_script(cdp: &CdpRouter, session: &str, clock: &Clock) -> Result<String, String> {
    cdp.call("Page.enable", json!({}), Some(session))
        .await
        .map_err(|error| error.to_string())?;
    let params = json!({ "source": install_source(clock), "runImmediately": true });
    let answer = cdp
        .call(
            "Page.addScriptToEvaluateOnNewDocument",
            params,
            Some(session),
        )
        .await
        .map_err(|error| error.to_string())?;
    answer["identifier"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "the browser named no script".to_owned())
}

async fn remove_script(cdp: &CdpRouter, session: &str, identifier: &str) {
    let params = json!({ "identifier": identifier });
    if let Err(error) = cdp
        .call(
            "Page.removeScriptToEvaluateOnNewDocument",
            params,
            Some(session),
        )
        .await
    {
        tracing::debug!(%error, "the page clock's script was not removed");
    }
}

async fn send_rate(cdp: &CdpRouter, session: &str, rate: f64) -> Outcome {
    cdp.call(RATE_METHOD, rate_params(rate), Some(session))
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// The value of `expression` in `session`'s top document - in `context`, else the main world;
/// an exception is an error.
async fn evaluate(
    cdp: &CdpRouter,
    session: &str,
    expression: &str,
    context: Option<i64>,
) -> Result<Value, String> {
    let mut params = json!({ "expression": expression, "returnByValue": true });
    if let Some(context) = context {
        params["contextId"] = json!(context);
    }
    let answer = cdp
        .call("Runtime.evaluate", params, Some(session))
        .await
        .map_err(|error| error.to_string())?;
    if answer.get("exceptionDetails").is_some() {
        return Err("the page script threw".to_owned());
    }
    Ok(answer
        .pointer("/result/value")
        .cloned()
        .unwrap_or(Value::Null))
}

/// Moves the stopped animations of `session`'s top document `ms` on, in `context` (the sync
/// agent's isolated world, out of the page's reach) or else the main world; how many moved.
///
/// # Errors
///
/// The browser refused the call or the script threw.
pub async fn step(
    cdp: &CdpRouter,
    session: &str,
    context: Option<i64>,
    ms: u32,
) -> Result<u64, String> {
    let moved = evaluate(cdp, session, &step_expression(ms), context).await?;
    Ok(moved.as_u64().unwrap_or(0))
}

#[cfg(test)]
#[path = "hooks_tests.rs"]
mod tests;
