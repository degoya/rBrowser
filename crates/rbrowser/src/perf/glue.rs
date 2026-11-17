//! The runtime of the performance budget in one window: the settings into the host, the
//! leaders' last scrolls for the catch-up, the memory watch.

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use rb_engine::{ViewEvent, ViewHost, ViewId};
use rb_sync::{AgentMessage, Replay, ReplayOutcome, ReplayRequest, SyncEvent};
use tokio::sync::broadcast::error::RecvError;

use super::catchup::{self, CatchUpFacts, CatchUpStep, ScrollMemo};
use super::memory::{CountWatch, MemoryWatch, UntiedWatch, notification, untied_notification};
use super::process;
use super::throttle::ThrottleSettings;
use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;

/// How often the memory is sampled.
const MEMORY_INTERVAL: Duration = Duration::from_secs(10);
/// How many image names a log line of the process count names.
const TOP_IMAGES: usize = 6;

/// One window's memo: its host and the leaders' last scrolls.
type MemoEntry = (Weak<Host>, Arc<Mutex<ScrollMemo>>);

/// The leaders' last scrolls of every live window, by host.
fn registry() -> &'static Mutex<Vec<MemoEntry>> {
    static REGISTRY: Mutex<Vec<MemoEntry>> = Mutex::new(Vec::new());
    &REGISTRY
}

fn memo_of(host: &Host) -> Option<Arc<Mutex<ScrollMemo>>> {
    let mut entries = lock(registry());
    entries.retain(|(weak, _)| weak.strong_count() > 0);
    entries
        .iter()
        .find(|(weak, _)| std::ptr::eq(weak.as_ptr(), host))
        .map(|(_, memo)| memo.clone())
}

/// Starts the budget of `core`'s window: the throttling settings, the scroll memo and the
/// memory watch.
pub fn on_start(core: &Core) {
    on_user_settings_changed(core);
    let host = core.host();
    if memo_of(host).is_some() {
        return;
    }
    let memo = Arc::new(Mutex::new(ScrollMemo::default()));
    lock(registry()).push((Arc::downgrade(host), memo.clone()));
    follow_reports(host, &memo);
    follow_views(host, &memo);
    if let Some(core) = core.arc() {
        watch_memory(&core);
    }
}

/// The user settings changed: the throttling follows at once (the memory watch reads them at
/// every sample).
pub fn on_user_settings_changed(core: &Core) {
    let settings = ThrottleSettings::from(&core.user_settings().performance);
    core.host().set_throttle_settings(settings);
}

/// The leaders' document scrolls, as their agents report them.
fn follow_reports(host: &Arc<Host>, memo: &Arc<Mutex<ScrollMemo>>) {
    let mut reports = host.sync_agents().subscribe();
    let memo = Arc::downgrade(memo);
    host.runtime().spawn(async move {
        loop {
            let report = match reports.recv().await {
                Ok(report) => report,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return,
            };
            let Some(memo) = memo.upgrade() else {
                return;
            };
            if let AgentMessage::Event(message) = &report.message {
                lock(&memo).observe(report.view, message);
            }
        }
    });
}

/// A navigation forgets the scroll of the document left; a closed view everything.
fn follow_views(host: &Arc<Host>, memo: &Arc<Mutex<ScrollMemo>>) {
    let mut events = host.subscribe();
    let weak = Arc::downgrade(host);
    let memo = Arc::downgrade(memo);
    host.runtime().spawn(async move {
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return,
            };
            let (Some(memo), Some(host)) = (memo.upgrade(), weak.upgrade()) else {
                return;
            };
            match event {
                ViewEvent::Navigated { view, .. } => lock(&memo).navigated(view),
                ViewEvent::Closed(_) => {
                    let views: Vec<ViewId> = host.views().iter().map(|info| info.id).collect();
                    lock(&memo).retain(&views);
                }
                _ => {}
            }
        }
    });
}

/// `view` shows again after it was throttled: it takes the leader's URL or last scroll
/// ([`catchup::plan`]).
pub fn catch_up(host: &Arc<Host>, view: ViewId) {
    let Some(sync) = crate::sync_hub::for_host(host) else {
        return;
    };
    let state = sync.state(&host.config().sync);
    let leader = sync
        .hub()
        .with(|hub| hub.active())
        .and_then(|leader| host.view(leader).map(|info| (leader, info.url)));
    let Some(view_url) = host.view(view).map(|info| info.url) else {
        return;
    };
    let facts = CatchUpFacts {
        view,
        view_url,
        leader,
        enabled: state.enabled,
        navigation: state.features.navigation,
        scroll: state.features.scroll,
        excluded: state.excluded.contains(&view),
        decoupled: state.navigation_decoupled.contains(&view),
        desynced: state.desynced.iter().any(|desync| desync.view == view),
    };
    let memo = memo_of(host)
        .map(|memo| lock(&memo).clone())
        .unwrap_or_default();
    match catchup::plan(&facts, &memo) {
        Some(CatchUpStep::Resync) => {
            if let Err(error) = sync.resync(view) {
                tracing::warn!(%view, %error, "the shown view did not catch up");
            }
        }
        Some(CatchUpStep::Scroll { source, data }) => {
            let request = ReplayRequest {
                source,
                target: view,
                top: true,
                frame: None,
                event: SyncEvent::Scroll(*data).into(),
                block_submit: false,
            };
            host.runtime().spawn(async move {
                let outcome = sync.router().dispatch(request).await;
                if !matches!(outcome, ReplayOutcome::Done | ReplayOutcome::Unsupported) {
                    tracing::info!(%view, ?outcome, "the shown view kept its scroll position");
                }
            });
        }
        None => {}
    }
}

/// Samples the memory of rBrowser's processes every [`MEMORY_INTERVAL`] while the warning is
/// on, and raises it in the shell when due. The process count and its most frequent images go
/// into the log when the count has moved ([`CountWatch`], RB-080-67).
fn watch_memory(core: &Arc<Core>) {
    let weak = Arc::downgrade(core);
    core.runtime().spawn(async move {
        let mut watch = MemoryWatch::default();
        let mut count = CountWatch::default();
        let mut untied = UntiedWatch::default();
        let start = tokio::time::Instant::now() + MEMORY_INTERVAL;
        let mut ticker = tokio::time::interval_at(start, MEMORY_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let Some(core) = weak.upgrade() else {
                return;
            };
            // A helper outside its job object may leave processes behind: said once, in the
            // shell (review 9 of RB-080-67).
            if let Some(count) = untied.observe(rb_process::untied_spawns()) {
                tracing::warn!(count, "helper processes run outside their job object");
                core.notify(untied_notification(count));
            }
            let limit = core.user_settings().performance.clamped().memory_warning_mb;
            drop(core);
            if limit == 0 {
                watch.observe(0, 0);
                continue;
            }
            let sample = tokio::task::spawn_blocking(process::sample)
                .await
                .ok()
                .flatten();
            let Some(census) = sample else {
                continue;
            };
            let Some(core) = weak.upgrade() else {
                return;
            };
            let sample = census.sample;
            if count.observe(sample.processes) {
                tracing::info!(
                    processes = sample.processes,
                    used_mb = sample.private_bytes / (1024 * 1024),
                    images = %census.top(TOP_IMAGES),
                    "rBrowser's process count"
                );
            }
            if let Some(warning) = watch.observe(sample.private_bytes, limit) {
                tracing::warn!(
                    used_mb = warning.used_mb,
                    limit_mb = warning.limit_mb,
                    processes = sample.processes,
                    images = %census.top(TOP_IMAGES),
                    untied = rb_process::untied_spawns(),
                    "memory above the warning threshold"
                );
                core.notify(notification(warning));
            }
        }
    });
}

#[cfg(test)]
#[path = "glue_tests.rs"]
mod tests;
