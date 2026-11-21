//! The consent probe's browser (RB-094-09): a windowless Chromium browser the window makes in a
//! request context of its own, in memory and empty (`sessions::cef::fresh_request_context`, the
//! render view session [`super::PROBE_SESSION`]), driven over the app's CDP connection.
//!
//! - **Apart from the views.** The browser starts at a render view marker (random, hidden from
//!   every outer CDP client, `crate::devtools::hidden`); its target is attached only when its
//!   browser context is one no other target has ([`isolated`]) - never the global context nor a
//!   named session's - so neither the click nor a cookie the page sets reaches a view's session.
//! - **Fresh.** Whether the context held a cookie before the load is asked, not assumed
//!   (`session.fresh`).
//! - **Captured.** Requests from `Network.requestWillBeSent` of its session (a tap of their own,
//!   so the views' traffic cannot crowd them out), cookies from `Network.getAllCookies` of its
//!   page - its context's, which no other target shares (values are dropped in `super::shape`),
//!   the plugin's collector in an isolated world of the main frame (`Page.createIsolatedWorld`),
//!   so the page's scripts cannot see or change it.
//! - **Discarded.** `Network.clearBrowserCookies` of its page (its context's) and
//!   `Storage.clearDataForOrigin` of the probed origin, then the browser is closed and its target
//!   waited for; the in-memory context goes with its last browser.
//!
//! The cookies go through the page because CEF's DevTools find a browser context by
//! `browserContextId` only when CDP made it: `Storage.getCookies` of the request context fails.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::task::JoinHandle;

use super::address::Admission;
use super::run::{ProbeDriver, ProbeError};
use super::shape::MAX_REQUESTS;
use crate::cdp::{CdpRouter, lock};
use crate::host::Host;
use crate::sessions::STORAGE_TYPES;
use crate::sitemap::backend::NETWORK_IDLE_MAX;
use crate::sitemap::render_views::{
    CLOSE_WAIT, FIND_DEADLINE, FIND_POLL, RenderView, find_target, forget_when_closed, lists,
    next_marker,
};

/// The probe page's viewport: a common desktop size.
const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;
/// How long the network must stay without a new request after a click.
const SETTLE_QUIET: Duration = Duration::from_millis(1000);
/// How often the requests are counted while settling.
const SETTLE_POLL: Duration = Duration::from_millis(250);
/// The longest wait after a click.
const SETTLE_MAX: Duration = Duration::from_secs(10);
/// The isolated world the collector runs in.
const WORLD: &str = "rbrowser-consent-probe";
/// The requests waiting on the tap at most.
const TAP_CAPACITY: usize = 1024;

const fn failed(reason: &'static str) -> ProbeError {
    ProbeError(reason)
}

/// The browser context of `target` in `targets` (a `Target.getTargets` answer) when no other
/// target shares it; `None` when it has none or shares it with one (a view, the shell, ...).
#[must_use]
pub fn isolated(targets: &Value, target: &str) -> Option<String> {
    let infos = targets["targetInfos"].as_array()?;
    let context_of = |info: &Value| info["browserContextId"].as_str().map(str::to_owned);
    let own = infos
        .iter()
        .find(|info| info["targetId"].as_str() == Some(target))
        .and_then(context_of)
        .filter(|context| !context.is_empty())?;
    let shared = infos.iter().any(|info| {
        info["targetId"].as_str() != Some(target)
            && context_of(info).as_deref() == Some(own.as_str())
    });
    (!shared).then_some(own)
}

/// Whether `event` is the lifecycle event `name` of the document `frame`/`loader` in `session`.
#[must_use]
pub fn is_lifecycle(
    event: &crate::cdp::CdpEvent,
    session: &str,
    ids: (&str, &str),
    name: &str,
) -> bool {
    event.session_id.as_deref() == Some(session)
        && event.method == "Page.lifecycleEvent"
        && event.params["name"].as_str() == Some(name)
        && event.params["frameId"].as_str() == Some(ids.0)
        && event.params["loaderId"].as_str() == Some(ids.1)
}

/// The probe's page once attached.
struct Open {
    session: String,
}

/// Requests of the probe's session, collected off the tap.
#[derive(Default)]
struct Requests {
    list: Mutex<Vec<Value>>,
    /// Every request seen, for settling.
    seen: AtomicU64,
}

/// The probe over `cdp` and the window of `host`.
pub struct CdpDriver {
    cdp: Arc<CdpRouter>,
    host: Weak<Host>,
    marker: Option<String>,
    target: Option<String>,
    open: Option<Open>,
    /// The origin loaded, cleared at the end.
    origin: Option<String>,
    requests: Arc<Requests>,
    drain: Option<JoinHandle<()>>,
    /// The addresses its page may show while a collector runs ([`super::address`]).
    admission: Admission,
}

impl CdpDriver {
    #[must_use]
    pub fn new(cdp: Arc<CdpRouter>, host: Weak<Host>, admission: Admission) -> Self {
        Self {
            cdp,
            host,
            marker: None,
            target: None,
            open: None,
            origin: None,
            requests: Arc::default(),
            drain: None,
            admission,
        }
    }

    async fn call(
        &self,
        method: &str,
        params: Value,
        on: Option<&str>,
    ) -> Result<Value, ProbeError> {
        self.cdp.call(method, params, on).await.map_err(|error| {
            // The reason the answer names is fixed; the browser's own words go to the log.
            tracing::warn!(method, %error, "a call of the consent probe failed");
            failed("the probe's browser did not answer")
        })
    }

    fn session(&self) -> Result<&Open, ProbeError> {
        self.open
            .as_ref()
            .ok_or(failed("the probe session is not open"))
    }

    /// Has the window make the browser at `marker` in the probe session.
    async fn make(&self, marker: &str) -> Result<(), ProbeError> {
        let host = self.host.upgrade().ok_or(failed("the window is closed"))?;
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let view = RenderView::new(marker, super::PROBE_SESSION, WIDTH, HEIGHT, 1.0);
        host.ui().open_render_view(
            view,
            Box::new(move |made| {
                // The open gave up already: nothing waits for the answer.
                let _ = sender.send(made);
            }),
        );
        match tokio::time::timeout(FIND_DEADLINE, receiver).await {
            Ok(Ok(Ok(()))) => Ok(()),
            _ => Err(failed("the window made no probe browser")),
        }
    }

    /// Collects the `Network.requestWillBeSent` events of `session` from now on.
    fn start_drain(&mut self, session: String) {
        let mut tap = self.cdp.tap("Network.requestWillBeSent", TAP_CAPACITY);
        let requests = Arc::clone(&self.requests);
        self.drain = Some(tokio::spawn(async move {
            while let Some(event) = tap.recv().await {
                if event.session_id.as_deref() != Some(session.as_str()) {
                    continue;
                }
                requests.seen.fetch_add(1, Ordering::AcqRel);
                let mut list = lock(&requests.list);
                // One more than a phase keeps, so the phase knows it was cut.
                if list.len() <= MAX_REQUESTS {
                    list.push(event.params);
                }
            }
        }));
    }

    /// Waits for the lifecycle event `name` of the document `ids`; `false` when events were lost.
    async fn until(
        events: &mut tokio::sync::broadcast::Receiver<Arc<crate::cdp::CdpEvent>>,
        session: &str,
        ids: (&str, &str),
        name: &str,
    ) -> bool {
        loop {
            match events.recv().await {
                Ok(event) if is_lifecycle(&event, session, ids, name) => return true,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
    }
}

#[async_trait]
impl ProbeDriver for CdpDriver {
    async fn open(&mut self) -> Result<bool, ProbeError> {
        let marker = next_marker().map_err(|_| failed("no randomness for the probe"))?;
        self.marker = Some(marker.clone());
        self.make(&marker).await?;
        let target = find_target(&self.cdp, &marker, None)
            .await
            .map_err(|_| failed("the probe browser has no target"))?;
        crate::devtools::hidden::register(&target);
        self.target = Some(target.clone());
        let targets = self.call("Target.getTargets", json!({}), None).await?;
        isolated(&targets, &target)
            .ok_or(failed("the probe session is not apart from the views"))?;
        let attached = self
            .call(
                "Target.attachToTarget",
                json!({ "targetId": target, "flatten": true }),
                None,
            )
            .await?;
        let session = attached["sessionId"]
            .as_str()
            .ok_or(failed("the probe browser could not be attached"))?
            .to_owned();
        self.open = Some(Open {
            session: session.clone(),
        });
        let on = Some(session.as_str());
        self.call("Page.enable", json!({}), on).await?;
        self.call(
            "Page.setLifecycleEventsEnabled",
            json!({ "enabled": true }),
            on,
        )
        .await?;
        self.start_drain(session.clone());
        self.call("Network.enable", json!({}), on).await?;
        let jar = self.call("Network.getAllCookies", json!({}), on).await?;
        Ok(jar["cookies"].as_array().is_some_and(Vec::is_empty))
    }

    async fn load(&mut self, url: &str) -> Result<(), ProbeError> {
        let session = self.session()?.session.clone();
        let on = Some(session.as_str());
        self.origin = rb_util::url::origin_of(url);
        let mut events = self.cdp.subscribe();
        let answer = self
            .call("Page.navigate", json!({ "url": url }), on)
            .await?;
        if answer["errorText"]
            .as_str()
            .is_some_and(|text| !text.is_empty())
        {
            return Err(failed("the page did not load"));
        }
        let frame = answer["frameId"].as_str().unwrap_or_default().to_owned();
        let loader = answer["loaderId"].as_str().unwrap_or_default().to_owned();
        let ids = (frame.as_str(), loader.as_str());
        if !Self::until(&mut events, &session, ids, "load").await {
            // Events were lost: the page says whether it has loaded.
            loop {
                let state = self
                    .call(
                        "Runtime.evaluate",
                        json!({ "expression": "document.readyState", "returnByValue": true }),
                        on,
                    )
                    .await?;
                if state["result"]["value"] == "complete" {
                    return Ok(());
                }
                tokio::time::sleep(FIND_POLL).await;
            }
        }
        // A page that never goes quiet (a socket, polling) is captured all the same.
        let _ = tokio::time::timeout(
            NETWORK_IDLE_MAX,
            Self::until(&mut events, &session, ids, "networkIdle"),
        )
        .await;
        Ok(())
    }

    async fn settle(&mut self) {
        let deadline = tokio::time::Instant::now() + SETTLE_MAX;
        let mut last = self.requests.seen.load(Ordering::Acquire);
        let mut quiet = Duration::ZERO;
        while quiet < SETTLE_QUIET && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(SETTLE_POLL).await;
            let seen = self.requests.seen.load(Ordering::Acquire);
            quiet = if seen == last {
                quiet + SETTLE_POLL
            } else {
                Duration::ZERO
            };
            last = seen;
        }
    }

    async fn evaluate(&mut self, expression: &str) -> Result<Value, ProbeError> {
        let session = self.session()?.session.clone();
        let on = Some(session.as_str());
        let tree = self.call("Page.getFrameTree", json!({}), on).await?;
        let frame = tree["frameTree"]["frame"]["id"]
            .as_str()
            .ok_or(failed("the probe page has no main frame"))?;
        // A redirect or a script may have moved the page: no collector runs in a page the
        // probe may not load.
        let shown = tree["frameTree"]["frame"]["url"]
            .as_str()
            .unwrap_or_default();
        self.admission
            .check(shown)
            .await
            .map_err(|_| failed("the probe page left the addresses it may load"))?;
        let world = self
            .call(
                "Page.createIsolatedWorld",
                json!({ "frameId": frame, "worldName": WORLD }),
                on,
            )
            .await?;
        let context = world["executionContextId"]
            .as_i64()
            .ok_or(failed("the probe page has no isolated world"))?;
        let answer = self
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "contextId": context,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
                on,
            )
            .await?;
        if answer.get("exceptionDetails").is_some() {
            return Err(failed("the plugin's collector failed in the probe page"));
        }
        Ok(answer["result"]["value"].clone())
    }

    fn take_requests(&mut self) -> Vec<Value> {
        std::mem::take(&mut *lock(&self.requests.list))
    }

    async fn cookies(&mut self) -> Result<Value, ProbeError> {
        let session = self.session()?.session.clone();
        self.call("Network.getAllCookies", json!({}), Some(session.as_str()))
            .await
    }

    fn abandon(&mut self) {
        self.abandon_session();
    }

    async fn discard(&mut self) -> bool {
        if let Some(drain) = self.drain.take() {
            drain.abort();
        }
        lock(&self.requests.list).clear();
        let Some(marker) = self.marker.take() else {
            return true;
        };
        let cleared = match self.open.take() {
            Some(open) => self.clear(&open).await,
            None => false,
        };
        if let Some(host) = self.host.upgrade() {
            host.ui().close_render_view(marker);
        }
        let closed = match self.target.take() {
            Some(target) => self.closed(&target).await,
            None => false,
        };
        cleared && closed
    }
}

impl Drop for CdpDriver {
    /// A driver dropped with its session still open (its probe's caller gave up) abandons it.
    fn drop(&mut self) {
        self.abandon_session();
    }
}

impl CdpDriver {
    /// [`ProbeDriver::abandon`]: stops the collection, has the window close the browser and
    /// forgets its target as hidden once it is gone (on a task, when a runtime is there; else it
    /// stays hidden). Nothing after a finished discard.
    fn abandon_session(&mut self) {
        if let Some(drain) = self.drain.take() {
            drain.abort();
        }
        lock(&self.requests.list).clear();
        self.open = None;
        if let Some(marker) = self.marker.take()
            && let Some(host) = self.host.upgrade()
        {
            host.ui().close_render_view(marker);
        }
        if let Some(target) = self.target.take()
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            let cdp = Arc::clone(&self.cdp);
            runtime.spawn(async move { forget_when_closed(&cdp, &target).await });
        }
    }

    /// Deletes every cookie of the probe's context (through its page) and the storage of the origin it loaded.
    async fn clear(&self, open: &Open) -> bool {
        let cookies = self
            .call(
                "Network.clearBrowserCookies",
                json!({}),
                Some(open.session.as_str()),
            )
            .await
            .is_ok();
        let storage = match &self.origin {
            Some(origin) => self
                .call(
                    "Storage.clearDataForOrigin",
                    json!({ "origin": origin, "storageTypes": STORAGE_TYPES }),
                    Some(open.session.as_str()),
                )
                .await
                .is_ok(),
            None => true,
        };
        cookies && storage
    }

    /// Waits until `target` is no longer listed (at most [`CLOSE_WAIT`]); it is then forgotten
    /// as hidden. One still listed stays hidden.
    async fn closed(&self, target: &str) -> bool {
        let deadline = tokio::time::Instant::now() + CLOSE_WAIT;
        loop {
            match self.cdp.call("Target.getTargets", json!({}), None).await {
                Ok(targets) if !lists(&targets, target) => {
                    crate::devtools::hidden::forget(target);
                    return true;
                }
                Ok(_) if tokio::time::Instant::now() < deadline => {}
                _ => return false,
            }
            tokio::time::sleep(FIND_POLL).await;
        }
    }
}

#[cfg(test)]
#[path = "cdp_tests.rs"]
mod tests;
