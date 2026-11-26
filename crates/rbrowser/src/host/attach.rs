//! The CDP side of the views: attaching each view's target, sending its device metrics and the
//! rest of its emulation (RB-010-05) - the user agent with the ` rBrowser/<version>` suffix
//! (RB-020-08) - and attaching again (and re-applying all of it) when a session is lost. Overrides of a session outlive navigations; a swapped target detaches and
//! comes back through the reconnect path.

use std::collections::BTreeMap;
use std::sync::Arc;

use rb_engine::{Emulation, EngineError, ViewEvent, ViewId};
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

use super::Host;
use super::nav::marker_url;
use crate::cdp::CdpEvent;
use crate::emulation::cdp::{emulation_calls, metrics_params};
use crate::emulation::cdp_extras::{PermissionPlan, geolocation_permission, in_context};
use crate::emulation::fields::allowed_headers;
use crate::fit::DeviceMetrics;
use crate::sync_agent::page_tasks::state_of;
use crate::sync_agent::ua::{suffixed, with_brands};

/// Starts the task that follows the browser's CDP events for the host.
pub(super) fn watch_cdp(host: &Arc<Host>) {
    let mut events = host.cdp.subscribe();
    let weak = Arc::downgrade(host);
    host.runtime.spawn(async move {
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(missed)) => {
                    tracing::warn!(missed, "CDP events dropped");
                    continue;
                }
                Err(RecvError::Closed) => return,
            };
            let Some(host) = weak.upgrade() else {
                return;
            };
            on_event(&host, &event);
        }
    });
}

fn on_event(host: &Arc<Host>, event: &CdpEvent) {
    match event.method.as_str() {
        // The session is gone (the target swapped processes, a client detached it): attach
        // again; the metrics follow the new session.
        "Target.detachedFromTarget" => {
            let Some(session) = event.params.get("sessionId").and_then(Value::as_str) else {
                return;
            };
            let view = {
                let mut state = host.state();
                let found = state
                    .views
                    .values_mut()
                    .find(|entry| entry.session.as_deref() == Some(session));
                found.map(|entry| {
                    entry.session = None;
                    entry.applied = None;
                    entry.applied_emulation = None;
                    entry.id
                })
            };
            if let Some(view) = view {
                // The window closes: its browsers go one by one, none comes back (RB-080-91).
                if host.state().closing {
                    tracing::debug!(%view, "CDP session ended while the window closes");
                    return;
                }
                tracing::info!(%view, "CDP session lost, attaching again");
                spawn_attach(host, view);
            }
        }
        // A crashed renderer came back: its emulation starts from scratch.
        "Inspector.targetReloadedAfterCrash" => {
            let view = event.session_id.as_deref().and_then(|session| {
                host.state()
                    .views
                    .values()
                    .find(|entry| entry.session.as_deref() == Some(session))
                    .map(|entry| entry.id)
            });
            if let Some(view) = view {
                host.spawn_apply(view, true);
            }
        }
        _ => {}
    }
}

/// Attaches `view`'s target (found by its marker URL the first time), unless an attach is
/// already under way or the window closes.
pub(super) fn spawn_attach(host: &Host, view: ViewId) {
    {
        let mut state = host.state();
        if state.closing {
            return;
        }
        let Some(entry) = state.views.get_mut(&view) else {
            return;
        };
        if entry.attaching || entry.session.is_some() {
            return;
        }
        entry.attaching = true;
    }
    let Some(host) = host.me.upgrade() else {
        return;
    };
    let runtime = host.runtime.clone();
    runtime.spawn(async move {
        let result = attach(&host, view).await;
        if let Some(entry) = host.state().views.get_mut(&view) {
            entry.attaching = false;
        }
        match result {
            Ok(()) => {
                // The attach sent the emulation's CPU rate: the budget goes on top (RB-020-09).
                host.throttle_session_reset(view);
                host.emit(ViewEvent::Updated(view));
            }
            Err(error) => tracing::warn!(%view, %error, "CDP attach failed"),
        }
    });
}

impl Host {
    /// The main window is asked to close (RB-080-91): from now on no view attaches its target
    /// again - a session the closing browsers end stays ended, so nothing calls into a browser
    /// that is going (the owner's log: `CDP attach failed ... the browser is not available`
    /// for every view). The views, the pipe, the sidecar and the plugins then go in the core's
    /// order, and CEF shuts down last.
    pub fn begin_shutdown(&self) {
        let mut state = self.state();
        if !state.closing {
            state.closing = true;
            tracing::info!("the host stops attaching views: the window closes");
        }
    }
}

async fn attach(host: &Arc<Host>, view: ViewId) -> Result<(), EngineError> {
    let known = host
        .state()
        .views
        .get(&view)
        .ok_or(EngineError::UnknownView(view))?
        .target_id
        .clone();
    let target_id = match known {
        Some(target_id) => target_id,
        None => find_target(host, view).await?,
    };
    let attached = host
        .cdp
        .call(
            "Target.attachToTarget",
            json!({ "targetId": target_id, "flatten": true }),
            None,
        )
        .await?;
    let session = attached
        .get("sessionId")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::Invalid("attachToTarget gave no session".to_owned()))?
        .to_owned();
    let browser_context = browser_context_of(host, &target_id).await;
    let pending = {
        let mut state = host.state();
        let entry = state
            .views
            .get_mut(&view)
            .ok_or(EngineError::UnknownView(view))?;
        entry.target_id = Some(target_id.clone());
        entry.session = Some(session.clone());
        entry.browser_context = browser_context;
        entry.applied = None;
        entry.applied_emulation = None;
        entry.pending_url.take()
    };
    apply_metrics(host, view, true).await?;
    // The sync agent (RB-020-01): before the pending URL loads, so its first document has it.
    let config = crate::sync_hub::agent_config_for(host, view);
    if let Err(error) = host.agents.attach(view, &target_id, &session, config).await {
        tracing::warn!(%view, %error, "sync agent not injected");
    }
    // The page API (RB-020-08): its main-world script before the first document too.
    let switches = host.page.switches(&host.config());
    if let Some(state) = state_of(host, view, &switches) {
        host.page.attach(view, &session, state, switches.api).await;
    }
    // The Framework DevTools probe and hooks (RB-080-24), before the first document too.
    host.frameworks.attach(view, &session).await;
    // The page clock and the animation rate (RB-094-07), before the first document too.
    crate::time_control::attach(&host.cdp, view, &session).await;
    if let Some(url) = pending {
        host.ui.navigate(view, &url);
    }
    Ok(())
}

/// The target whose URL is `view`'s marker.
async fn find_target(host: &Host, view: ViewId) -> Result<String, EngineError> {
    let marker = marker_url(view);
    let targets = host.cdp.call("Target.getTargets", json!({}), None).await?;
    targets
        .get("targetInfos")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|info| info.get("url").and_then(Value::as_str) == Some(marker.as_str()))
        .and_then(|info| info.get("targetId").and_then(Value::as_str))
        .map(str::to_owned)
        .ok_or_else(|| EngineError::Invalid(format!("no CDP target shows {marker}")))
}

/// The `browserContextId` of `target`; `None` when the browser does not say (the permission
/// calls then go without it, to the default context).
async fn browser_context_of(host: &Host, target: &str) -> Option<String> {
    match host
        .cdp
        .call("Target.getTargetInfo", json!({ "targetId": target }), None)
        .await
    {
        Ok(info) => info
            .pointer("/targetInfo/browserContextId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        Err(error) => {
            tracing::debug!(%error, "the target's browser context is unknown");
            None
        }
    }
}

/// What one apply sends, read from the state under its lock.
struct Pending {
    session: String,
    metrics: DeviceMetrics,
    applied: Option<DeviceMetrics>,
    previous: Option<Emulation>,
    next: Emulation,
    live_headers: BTreeMap<String, String>,
    /// `pageApi.uaSuffix` in effect (RB-020-08).
    ua_suffix: bool,
    /// The view's browser context (its session), when it has a position or a grant to take
    /// back (RB-080-10): only then is there a grant to plan.
    position_context: Option<String>,
}

/// Sends `view`'s device metrics and emulation where they changed (or all of it with `force`:
/// a new session, a crashed renderer, `reapply_emulation`). Without a session nothing is sent;
/// the attach sends it. Headers from `config.json` and `config.local.json` go out only in a
/// trusted workspace; an untrusted one sends only those set live (`emulation.set`).
///
/// One apply of a view at a time, beside the other views' (RB-099-19, [`super::apply_locks`]);
/// the geolocation grant under its browser context's lock ([`apply_position`]).
pub(super) async fn apply_metrics(
    host: &Arc<Host>,
    view: ViewId,
    force: bool,
) -> Result<(), EngineError> {
    let _view = host.apply.view(view).await;
    let trusted = host.shell_state().trusted;
    let pending = {
        let state = host.state();
        let entry = state
            .views
            .get(&view)
            .ok_or(EngineError::UnknownView(view))?;
        if state.rect_of(entry).is_none() {
            // Not laid out yet: the first bounds send the metrics.
            return Ok(());
        }
        let Some(session) = entry.session.clone() else {
            return Ok(());
        };
        let position = entry.emulation.geolocation.is_some() || entry.geo_origin.is_some();
        Pending {
            session,
            metrics: state.metrics_of(entry),
            applied: entry.applied,
            previous: entry.applied_emulation.clone(),
            next: entry.emulation.clone(),
            live_headers: entry.overrides.headers.clone(),
            ua_suffix: host.page.switches(&state.config).ua_suffix,
            position_context: position.then(|| entry.context_session().to_owned()),
        }
    };
    // `dir` and `lang` ride on the sync agent, not on a CDP call (RB-092-02).
    host.agents.follow_emulation(view, &pending.next).await;
    let Pending {
        session, metrics, ..
    } = &pending;
    let mut next = pending.next.clone();
    if !trusted && !next.headers.is_empty() {
        next.headers = allowed_headers(&next.headers, &pending.live_headers);
    }
    // The suffix rides on the user agent: the emulated one, else the engine's own.
    let original = next.clone();
    let engine = if pending.ua_suffix {
        host.page.engine_user_agent().await
    } else {
        None
    };
    if pending.ua_suffix {
        next = suffixed(&next, engine.as_ref());
    }
    let send_metrics = force
        || pending.applied != Some(*metrics)
        || pending.previous.as_ref().map(|before| before.orientation) != Some(next.orientation);
    let mut calls = emulation_calls(pending.previous.as_ref(), &next, force);
    if let Some(engine) = &engine {
        for call in calls
            .iter_mut()
            .filter(|call| call.method == "Emulation.setUserAgentOverride")
        {
            call.params = with_brands(std::mem::take(&mut call.params), &original, engine);
        }
    }
    let mut sent = send_metrics || !calls.is_empty();
    if send_metrics {
        host.cdp
            .call(
                "Emulation.setDeviceMetricsOverride",
                metrics_params(metrics, next.orientation),
                Some(session),
            )
            .await?;
    }
    for call in calls {
        // One override the browser refuses (an older Chromium without safe-area insets) does
        // not stop the others.
        if let Err(error) = host.cdp.call(call.method, call.params, Some(session)).await {
            tracing::warn!(%view, method = call.method, %error, "an emulation override was not applied");
        }
    }
    let mut still_current = !sent || {
        let mut state = host.state();
        match state.views.get_mut(&view) {
            Some(entry) if entry.session.as_deref() == Some(session.as_str()) => {
                entry.applied = Some(*metrics);
                entry.applied_emulation = Some(next);
                true
            }
            _ => false,
        }
    };
    if let Some(context) = &pending.position_context {
        match apply_position(host, view, session, context, force).await {
            Position::Sent { current } => {
                sent = true;
                still_current &= current;
            }
            Position::Unchanged => {}
        }
    }
    if sent && still_current {
        host.emit(ViewEvent::Updated(view));
    }
    Ok(())
}

/// What [`apply_position`] did.
enum Position {
    /// Nothing to send.
    Unchanged,
    /// Grant calls went out; `current` while the view still has the session they went on.
    Sent { current: bool },
}

/// Brings `view`'s geolocation permission to what its page needs (RB-080-10), under the lock
/// of its browser context `context`: the plan reads which origins the context's other views
/// hold, and a grant holds per browser context - only views of the same session share it.
async fn apply_position(
    host: &Arc<Host>,
    view: ViewId,
    session: &str,
    context: &str,
    force: bool,
) -> Position {
    let _context = host.apply.context(context).await;
    let plan = {
        let state = host.state();
        // The view may have attached again, to another session or context, since the apply
        // read it: the plan belongs under that context's lock, the next apply makes it.
        let Some(entry) = state.views.get(&view).filter(|entry| {
            entry.context_session() == context && entry.session.as_deref() == Some(session)
        }) else {
            return Position::Unchanged;
        };
        let held: Vec<&str> = state
            .views
            .values()
            .filter(|other| other.id != view && other.emulation.geolocation.is_some())
            .filter(|other| other.context_session() == entry.context_session())
            .filter_map(|other| other.geo_origin.as_deref())
            .collect();
        let mut plan = geolocation_permission(
            entry.geo_origin.as_deref(),
            &entry.url,
            entry.emulation.geolocation.is_some(),
            force,
            &held,
        );
        in_context(&mut plan.calls, entry.browser_context.as_deref());
        plan
    };
    let PermissionPlan { calls, granted } = plan;
    let sent = !calls.is_empty();
    let mut failed = false;
    for call in calls {
        if let Err(error) = host.cdp.call(call.method, call.params, Some(session)).await {
            tracing::warn!(%view, method = call.method, %error, "an emulation override was not applied");
            failed = true;
        }
    }
    let mut state = host.state();
    let current = match state.views.get_mut(&view) {
        Some(entry) if entry.session.as_deref() == Some(session) => {
            // A refused permission call leaves what was granted before, so the next apply
            // tries again.
            if !failed {
                entry.geo_origin = granted;
            }
            true
        }
        _ => false,
    };
    if sent {
        Position::Sent { current }
    } else {
        Position::Unchanged
    }
}

impl Host {
    /// Sends every view's user agent again where it changed: the suffix was switched
    /// (`page_api.toggle_ua_suffix`, RB-020-08).
    pub fn reapply_user_agent(&self) {
        let views: Vec<ViewId> = self.state().order.clone();
        for view in views {
            self.spawn_apply(view, false);
        }
    }

    /// Drops `view`'s CDP session on purpose; the browser reports the detach and the view is
    /// attached again with its emulation (the reconnect path, also used by the tests).
    pub async fn reconnect(&self, view: ViewId) -> Result<(), EngineError> {
        let session = self
            .state()
            .views
            .get(&view)
            .ok_or(EngineError::UnknownView(view))?
            .session
            .clone()
            .ok_or_else(|| EngineError::Invalid(format!("view {view} is not attached")))?;
        self.cdp
            .call(
                "Target.detachFromTarget",
                json!({ "sessionId": session }),
                None,
            )
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
#[path = "attach_tests.rs"]
mod tests;
