//! The automatic copies of `engines.cookies.copy` (RB-040-05), read at every event so a changed
//! setting holds at once:
//!
//! - `onStart`: a Firefox or WebKit view that starts running (its first start, a restart, the
//!   engine switched on) gets its session's cookies, and loads its page again with them. The
//!   external views' status is read at each change the shell sees and every [`POLL`] while
//!   there are any or a view is on another engine (the shell sees none with
//!   `engines.external.showTiles` off; RB-090-38: a window of Chromium views alone has no timer).
//! - `onNavigate`: a page load of a Chromium view copies its session's cookies into the
//!   Firefox and WebKit views of that session, once the loads have been quiet for
//!   [`SETTLE`] (a login redirects several times).
//!
//! Neither tells the person (no notice); the log has the counts.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use rb_config::EngineKind;
use rb_engine::external::ExternalStatus;
use rb_engine::{ViewEvent, ViewHost, ViewId};
use tokio::sync::broadcast::error::RecvError;

use super::model::{CopyArgs, Source, Targets};
use crate::cdp::lock;
use crate::control::Core;

/// How long the page loads of a session must be quiet before `onNavigate` copies.
pub const SETTLE: Duration = Duration::from_millis(1500);
/// How often `onStart` looks at the external views' status besides the shell's changes.
pub const POLL: Duration = Duration::from_secs(1);

/// `engines.cookies.copy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// By hand only.
    Manual,
    /// Into each Firefox or WebKit view as it starts.
    OnStart,
    /// After the page loads of a Chromium view, into its session's external views.
    OnNavigate,
}

impl Mode {
    /// The mode of the setting's value; `manual` for anything else (the schema refuses such a
    /// value, a hand-edited file may still carry one).
    pub fn of(value: &str) -> Mode {
        match value {
            "onStart" => Mode::OnStart,
            "onNavigate" => Mode::OnNavigate,
            _ => Mode::Manual,
        }
    }
}

/// Whether the status poll is due: before the first look (`None`), while `seen` holds an
/// external view and while the host has a view on another engine than Chromium (`wanted`: its
/// window may not be listed yet); never for Chromium views alone.
pub fn needs_poll(seen: Option<&HashMap<ViewId, ExternalStatus>>, wanted: bool) -> bool {
    wanted || seen.is_none_or(|seen| !seen.is_empty())
}

/// The views that just started running: those `Running` in `now` that were not in `before`.
pub fn started(
    before: &HashMap<ViewId, ExternalStatus>,
    now: &[(ViewId, ExternalStatus)],
) -> Vec<ViewId> {
    now.iter()
        .filter(|(view, status)| {
            *status == ExternalStatus::Running && before.get(view) != Some(&ExternalStatus::Running)
        })
        .map(|(view, _)| *view)
        .collect()
}

/// Follows `core`'s window: its views' loads and the external views' status.
pub fn on_start(core: &Core) {
    let Some(core) = core.arc() else {
        return;
    };
    let mut events = core.host().subscribe();
    let weak = Arc::downgrade(&core);
    let runtime = core.runtime().clone();
    let pending: Arc<Mutex<BTreeMap<String, u64>>> = Arc::default();
    runtime.clone().spawn(async move {
        // `None` until the first look; a missed tick is not made up in a burst.
        let mut seen: Option<HashMap<ViewId, ExternalStatus>> = None;
        let mut poll = tokio::time::interval(POLL);
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            let wanted = weak.upgrade().is_some_and(|core| wants_external(&core));
            let polling = needs_poll(seen.as_ref(), wanted);
            let event = tokio::select! {
                event = events.recv() => Some(event),
                _ = poll.tick(), if polling => None,
            };
            let Some(core) = weak.upgrade() else {
                return;
            };
            let mode = core.with_user_settings(|user| Mode::of(&user.engines.cookies.copy));
            match event {
                Some(Ok(ViewEvent::Navigated { view, .. })) => {
                    if mode == Mode::OnNavigate {
                        settle_then_copy(&weak, &runtime, &pending, view);
                    }
                    continue;
                }
                Some(Err(RecvError::Closed)) => return,
                Some(Ok(
                    ViewEvent::StateChanged
                    | ViewEvent::Opened(_)
                    | ViewEvent::Closed(_)
                    | ViewEvent::LayoutChanged,
                ))
                | Some(Err(RecvError::Lagged(_)))
                | None => {}
                Some(Ok(_)) => continue,
            }
            let now = statuses(&core);
            let fresh = started(&seen.unwrap_or_default(), &now);
            seen = Some(now.into_iter().collect());
            if mode == Mode::OnStart && !fresh.is_empty() {
                let args = CopyArgs {
                    target: Some(Targets::Many(fresh)),
                    ..CopyArgs::default()
                };
                runtime.spawn(copy_quietly(weak.clone(), args));
            }
        }
    });
}

/// Whether a view of the host runs on another engine than Chromium.
fn wants_external(core: &Core) -> bool {
    core.host()
        .views()
        .iter()
        .any(|info| info.engine != EngineKind::Chromium)
}

/// The external views and where each stands.
fn statuses(core: &Core) -> Vec<(ViewId, ExternalStatus)> {
    crate::engines::for_host(core.host())
        .map(|engines| {
            engines
                .list()
                .into_iter()
                .map(|view| (view.view, view.status))
                .collect()
        })
        .unwrap_or_default()
}

/// `view` loaded a page: once its session's loads are quiet for [`SETTLE`], the session's
/// cookies go into its external views. A newer load of the session restarts the wait.
fn settle_then_copy(
    weak: &Weak<Core>,
    runtime: &tokio::runtime::Handle,
    pending: &Arc<Mutex<BTreeMap<String, u64>>>,
    view: ViewId,
) {
    let Some(core) = weak.upgrade() else {
        return;
    };
    let Some(info) = core
        .host()
        .views()
        .into_iter()
        .find(|info| info.id == view && info.engine == EngineKind::Chromium)
    else {
        return;
    };
    let session = info.session;
    let ticket = {
        let mut pending = lock(pending);
        let ticket = pending.entry(session.clone()).or_insert(0);
        *ticket += 1;
        *ticket
    };
    let (weak, pending) = (weak.clone(), pending.clone());
    runtime.spawn(async move {
        tokio::time::sleep(SETTLE).await;
        if lock(&pending).get(&session) != Some(&ticket) {
            return;
        }
        let Some(core) = weak.upgrade() else {
            return;
        };
        let targets: Vec<ViewId> = core
            .host()
            .views()
            .into_iter()
            .filter(|info| info.engine != EngineKind::Chromium && info.session == session)
            .map(|info| info.id)
            .collect();
        if targets.is_empty() {
            return;
        }
        let args = CopyArgs {
            source: Some(Source::Session(session)),
            target: Some(Targets::Many(targets)),
            domains: None,
        };
        copy_quietly(weak, args).await;
    });
}

/// A copy without a notice; nothing to copy into is no failure.
async fn copy_quietly(core: Weak<Core>, args: CopyArgs) {
    let Some(core) = core.upgrade() else {
        return;
    };
    match super::copy_now(&core, args, false).await {
        Ok(report) if report.failed() > 0 => {
            tracing::debug!(
                failed = report.failed(),
                "an automatic cookie copy left views out"
            );
        }
        Ok(_) => {}
        Err(error) => tracing::debug!(%error, "no automatic cookie copy"),
    }
}

#[cfg(test)]
#[path = "auto_tests.rs"]
mod tests;
