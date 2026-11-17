//! The CDP side of the rules: every Chromium view's page session gets `Fetch.enable` with the
//! patterns of the rules that apply (`Fetch.disable` once none do) and the cache switch
//! (`Network.setCacheDisabled`), again after a reattach and whenever the rules change; each
//! `Fetch.requestPaused` of such a session gets its answer ([`super::state::Rules::decide`]) on a
//! task of its own, so a mock's delay holds nothing else up. A clearing asked for runs
//! `Storage.clearDataForOrigin` for every view's origin. Firefox and WebKit views have no page
//! session here: the state marks them unsupported.
//!
//! No paused request is left waiting (review finding M1 of Charge 7): the paused requests come
//! on a channel of their own ([`CdpRouter::tap`]), not the broadcast every view's events share;
//! when some did not fit, `Fetch.disable` releases every request still paused and `Fetch.enable`
//! intercepts again; an answer that fails is followed by [`super::fetch::fallbacks`]; and the loop's end
//! disables `Fetch` on every session it enabled it on. The breakpoints' holds and the response
//! stage (RB-080-37) are [`super::answer`]; the patterns are the rules' and the breakpoints'.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use rb_engine::{ViewEvent, ViewId};
use serde_json::{Value, json};
use tokio::sync::{broadcast, watch};

use super::answer::answer;
use super::state::Rules;
use crate::cdp::{CdpRouter, Tap};

/// The event of a paused request.
pub const PAUSED_METHOD: &str = "Fetch.requestPaused";
/// Paused requests waiting for the loop before more are dropped (and released, see above).
pub const PAUSED_CAPACITY: usize = 4096;

/// What the loop needs of a window.
pub trait Window: Send + Sync {
    /// Every attached Chromium view with its page session.
    fn sessions(&self) -> Vec<(ViewId, String)>;
    /// Every view's URL.
    fn urls(&self) -> Vec<(ViewId, String)>;
    /// The open project's root: what map-local folders resolve against.
    fn project_root(&self) -> Option<PathBuf> {
        None
    }
}

/// What one session was set up with.
#[derive(Clone, PartialEq, Eq)]
struct Setup {
    session: String,
    generation: u64,
    fetch: bool,
    cache: bool,
}

/// Runs the rules over `window`'s sessions until `stop` is set or dropped, or the connection
/// closes.
pub async fn run(
    rules: Arc<Rules>,
    cdp: Arc<CdpRouter>,
    window: Arc<dyn Window>,
    views: broadcast::Receiver<ViewEvent>,
    stop: watch::Receiver<bool>,
) {
    let paused = cdp.tap(PAUSED_METHOD, PAUSED_CAPACITY);
    run_tapped(rules, cdp, window, paused, views, stop).await;
}

/// [`run`] with the paused requests on `paused`.
pub(crate) async fn run_tapped(
    rules: Arc<Rules>,
    cdp: Arc<CdpRouter>,
    window: Arc<dyn Window>,
    mut paused: Tap,
    mut views: broadcast::Receiver<ViewEvent>,
    mut stop: watch::Receiver<bool>,
) {
    let mut changed = rules.subscribe();
    let mut setups: HashMap<ViewId, Setup> = HashMap::new();
    configure(&rules, &cdp, &*window, &mut setups).await;
    loop {
        tokio::select! {
            result = stop.changed() => {
                if result.is_err() || *stop.borrow() {
                    break;
                }
            }
            result = changed.changed() => {
                if result.is_err() {
                    break;
                }
                configure(&rules, &cdp, &*window, &mut setups).await;
            }
            event = paused.recv() => {
                let Some(event) = event else { break };
                if let Some(session) = event.session_id
                    && let Some(view) = setups
                        .iter()
                        .find(|(_, setup)| setup.session == session && setup.fetch)
                        .map(|(view, _)| *view)
                {
                    answer(&rules, &cdp, view, session, &event.params);
                }
                let missed = paused.take_dropped();
                if missed > 0 {
                    tracing::warn!(missed, "paused requests missed; restarting interception");
                    restart_fetch(&rules, &cdp, &mut setups).await;
                }
            }
            event = views.recv() => match event {
                Ok(ViewEvent::Closed(view)) => {
                    setups.remove(&view);
                    rules.set_intercepting(intercepting(&setups));
                }
                Ok(ViewEvent::Opened(_) | ViewEvent::Updated(_) | ViewEvent::LayoutChanged)
                | Err(broadcast::error::RecvError::Lagged(_)) => {
                    configure(&rules, &cdp, &*window, &mut setups).await;
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            },
        }
    }
    release(&rules, &cdp, &setups).await;
}

/// `Fetch.disable` on every session intercepting: it lets every request still paused go on,
/// the held ones included.
async fn release(rules: &Rules, cdp: &CdpRouter, setups: &HashMap<ViewId, Setup>) {
    rules.held().clear();
    for setup in setups.values().filter(|setup| setup.fetch) {
        if let Err(error) = cdp
            .call("Fetch.disable", json!({}), Some(&setup.session))
            .await
        {
            tracing::debug!(%error, "Fetch.disable failed");
        }
    }
    rules.set_intercepting(BTreeSet::new());
}

/// Paused requests were lost: `Fetch.disable` releases them, `Fetch.enable` with the same
/// patterns intercepts again, on every session intercepting. A session whose `Fetch.enable`
/// failed no longer counts as intercepting, and the next configuration sets it up again
/// (Charge 8 review L2).
async fn restart_fetch(rules: &Rules, cdp: &CdpRouter, setups: &mut HashMap<ViewId, Setup>) {
    let patterns = rules.patterns();
    rules.held().clear();
    for setup in setups.values_mut().filter(|setup| setup.fetch) {
        if let Err(error) = cdp
            .call("Fetch.disable", json!({}), Some(&setup.session))
            .await
        {
            tracing::debug!(%error, "Fetch.disable failed");
        }
        setup.fetch =
            !patterns.is_empty() && set_fetch(cdp, &setup.session, &patterns, false).await;
        if !setup.fetch {
            setup.generation = 0;
        }
    }
    rules.set_intercepting(intercepting(setups));
}

fn intercepting(setups: &HashMap<ViewId, Setup>) -> BTreeSet<ViewId> {
    setups
        .iter()
        .filter(|(_, setup)| setup.fetch)
        .map(|(view, _)| *view)
        .collect()
}

/// Brings every session up to the rules' generation, and runs a clearing asked for.
async fn configure(
    rules: &Rules,
    cdp: &CdpRouter,
    window: &dyn Window,
    setups: &mut HashMap<ViewId, Setup>,
) {
    rules.set_project_root(window.project_root());
    let generation = *rules.subscribe().borrow();
    let patterns = rules.patterns();
    let cache = rules.cache_disabled();
    let sessions = window.sessions();
    setups.retain(|view, _| sessions.iter().any(|(known, _)| known == view));
    for (view, session) in &sessions {
        let before = setups
            .get(view)
            .filter(|setup| setup.session == *session)
            .cloned();
        if before
            .as_ref()
            .is_some_and(|setup| setup.generation == generation)
        {
            continue;
        }
        let was = before.unwrap_or(Setup {
            session: session.clone(),
            generation: 0,
            fetch: false,
            cache: false,
        });
        let fetch = set_fetch(cdp, session, &patterns, was.fetch).await;
        let cache = set_cache(cdp, session, cache, was.cache).await;
        setups.insert(
            *view,
            Setup {
                session: session.clone(),
                generation,
                fetch,
                cache,
            },
        );
    }
    rules.set_intercepting(intercepting(setups));
    if rules.take_clear() {
        clear_site_data(cdp, window, &sessions).await;
    }
}

/// `Fetch.enable` with `patterns`, or `Fetch.disable` when there are none and it was on;
/// whether interception is on now.
async fn set_fetch(cdp: &CdpRouter, session: &str, patterns: &[Value], was: bool) -> bool {
    if patterns.is_empty() {
        if was && let Err(error) = cdp.call("Fetch.disable", json!({}), Some(session)).await {
            tracing::debug!(%error, "Fetch.disable failed");
        }
        return false;
    }
    match cdp
        .call(
            "Fetch.enable",
            json!({ "patterns": patterns }),
            Some(session),
        )
        .await
    {
        Ok(_) => true,
        Err(error) => {
            tracing::debug!(%error, "Fetch.enable failed");
            false
        }
    }
}

/// The cache switch, sent when it is on or was; whether it is off now.
async fn set_cache(cdp: &CdpRouter, session: &str, disabled: bool, was: bool) -> bool {
    if !disabled && !was {
        return false;
    }
    if disabled && let Err(error) = cdp.call("Network.enable", json!({}), Some(session)).await {
        tracing::debug!(%error, "Network.enable failed");
    }
    match cdp
        .call(
            "Network.setCacheDisabled",
            json!({ "cacheDisabled": disabled }),
            Some(session),
        )
        .await
    {
        Ok(_) => disabled,
        Err(error) => {
            tracing::debug!(%error, "Network.setCacheDisabled failed");
            was
        }
    }
}

/// `Storage.clearDataForOrigin` (every storage type) for the origin of each view with a page
/// session, once per origin and session.
async fn clear_site_data(cdp: &CdpRouter, window: &dyn Window, sessions: &[(ViewId, String)]) {
    let urls = window.urls();
    let mut done: BTreeSet<(String, String)> = BTreeSet::new();
    for (view, session) in sessions {
        let Some(origin) = urls
            .iter()
            .find(|(known, _)| known == view)
            .and_then(|(_, url)| rb_util::url::origin_of(url))
        else {
            continue;
        };
        if !done.insert((session.clone(), origin.clone())) {
            continue;
        }
        let params = json!({ "origin": origin, "storageTypes": "all" });
        if let Err(error) = cdp
            .call("Storage.clearDataForOrigin", params, Some(session))
            .await
        {
            tracing::debug!(%error, "Storage.clearDataForOrigin failed");
        }
    }
}

#[cfg(test)]
#[path = "executor_tests.rs"]
mod tests;
