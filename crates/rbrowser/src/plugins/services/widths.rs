//! A plugin's width lease (RB-093-01, the width sweep): rBrowser owns every view's
//! `Emulation.setDeviceMetricsOverride` (ADR 0001), so the two metrics methods a plugin may send
//! (`rb_plugin_host` holds `setDeviceMetricsOverride` to `{ "width": n }` and
//! `clearDeviceMetricsOverride` to `{}`) are never sent as given. They are a lease of the view:
//!
//! - **a width** ([`SET`]): the host's own metrics of the view at that width
//!   (`Host::width_metrics`: its height, DPR and zoom, fitted into its place, its widget kept),
//!   and the first one takes the view out of sync ([`SyncPause`]: a view the person had taken
//!   out stays out);
//! - **the end** ([`CLEAR`]): the host's own metrics again (`Host::capture_target`, what a
//!   capture restores) and the view back in sync. An ended lease stays ended while the plugin
//!   runs: its next width is refused as `plugins.error.cancelled`, so the plugin's `cancel`
//!   command, a call of its own beside the sweep's, stops the sweep at its next step;
//! - **the plugin's last running call ending** (`HostServices::call_ended`: an answer, a failure,
//!   a trap, a timeout or a cancelled check run) and **the plugin stopping** (disabled, or a
//!   project's without trust): every live lease of the plugin ends as above.
//!
//! One plugin leases a view at a time; another plugin's width for it is
//! `plugins.error.view_leased` naming the view. The bookkeeping ([`WidthLeases`]) is apart
//! from the view and sync work ([`send`], [`restore`]) so it is tested alone.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rb_engine::ViewId;
use rb_plugin_types::{Failure, FailureKind, codes};
use serde_json::{Value, json};

use super::page;
use crate::control::Core;
use crate::flows::sync_pause::SyncPause;
use crate::host::Host;

/// A width for the view.
pub const SET: &str = "Emulation.setDeviceMetricsOverride";
/// The lease's end.
pub const CLEAR: &str = "Emulation.clearDeviceMetricsOverride";
/// How long a restore off a plugin's call may take.
const RESTORE_TIMEOUT: Duration = Duration::from_secs(5);

/// What a live lease holds: the view's sync pause, given back when dropped.
pub type Guard = Box<dyn Send>;

/// Whether `method` is one of the lease's two methods.
#[must_use]
pub fn lease_method(method: &str) -> bool {
    method == SET || method == CLEAR
}

/// The `width` of [`SET`]'s parameters (checked by the host before: a whole number in range).
#[must_use]
pub fn requested_width(params: &str) -> Option<u32> {
    let params: Value = serde_json::from_str(params).ok()?;
    u32::try_from(params.get("width")?.as_u64()?).ok()
}

struct Lease {
    plugin: String,
    /// `None` once ended: the plugin's next width is refused until its calls are over.
    pause: Option<Guard>,
}

#[derive(Default)]
struct Inner {
    leases: BTreeMap<u32, Lease>,
    /// The calls running per plugin.
    running: BTreeMap<String, u32>,
    /// The leases taken of a view so far: a restore for an older one sends nothing.
    generations: BTreeMap<u32, u64>,
    /// Per view, held while a width or a restore is sent, so a restore's check and its send are
    /// one step against the view's next width.
    gates: BTreeMap<u32, Arc<tokio::sync::Mutex<()>>>,
}

/// The views the plugins lease, by view id.
#[derive(Default)]
pub struct WidthLeases {
    inner: Mutex<Inner>,
}

fn busy(view: u32) -> Failure {
    Failure::coded(FailureKind::AccessDenied, codes::VIEW_LEASED)
        .with_param("view", view.to_string())
}

impl WidthLeases {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Plugin `plugin` sets a width on `view`: its first takes the lease (`pause` takes the
    /// view out of sync), the next ones hold it.
    ///
    /// # Errors
    ///
    /// Another plugin leases the view (`plugins.error.view_leased`), or the
    /// plugin's lease of it ended while its calls run (`plugins.error.cancelled`).
    pub fn take(
        &self,
        plugin: &str,
        view: u32,
        pause: impl FnOnce() -> Guard,
    ) -> Result<(), Failure> {
        let mut inner = self.lock();
        match inner.leases.get(&view) {
            Some(lease) if lease.plugin != plugin => Err(busy(view)),
            Some(lease) if lease.pause.is_some() => Ok(()),
            Some(_) => Err(Failure::coded(FailureKind::Cancelled, codes::CANCELLED)),
            None => {
                let lease = Lease {
                    plugin: plugin.to_owned(),
                    pause: Some(pause()),
                };
                inner.leases.insert(view, lease);
                *inner.generations.entry(view).or_default() += 1;
                Ok(())
            }
        }
    }

    /// How many leases of `view` were taken so far (`0`: none); the generation a restore is for.
    pub fn generation(&self, view: u32) -> u64 {
        self.lock()
            .generations
            .get(&view)
            .copied()
            .unwrap_or_default()
    }

    /// The gate a width or a restore of `view` is sent under.
    pub fn gate(&self, view: u32) -> Arc<tokio::sync::Mutex<()>> {
        Arc::clone(self.lock().gates.entry(view).or_default())
    }

    /// Plugin `plugin` ends its lease of `view`: the view is back in sync; `true` when the lease
    /// was live, so its metrics are to be restored.
    pub fn end(&self, plugin: &str, view: u32) -> bool {
        let pause = match self.lock().leases.get_mut(&view) {
            Some(lease) if lease.plugin == plugin => lease.pause.take(),
            _ => None,
        };
        pause.is_some()
    }

    /// Whether a live lease holds `view` now.
    pub fn leased(&self, view: u32) -> bool {
        self.lock()
            .leases
            .get(&view)
            .is_some_and(|lease| lease.pause.is_some())
    }

    /// A call of `plugin` started.
    pub fn started(&self, plugin: &str) {
        *self.lock().running.entry(plugin.to_owned()).or_default() += 1;
    }

    /// A call of `plugin` ended; after its last running one, its leases end ([`Self::stop`]).
    pub fn ended(&self, plugin: &str) -> Vec<u32> {
        {
            let mut inner = self.lock();
            match inner.running.get_mut(plugin) {
                Some(count) if *count > 1 => {
                    *count -= 1;
                    return Vec::new();
                }
                Some(_) => {
                    inner.running.remove(plugin);
                }
                None => return Vec::new(),
            }
        }
        self.stop(plugin)
    }

    /// Every lease of `plugin` ends and is forgotten: its views are back in sync; the views of
    /// the live ones, whose metrics are to be restored.
    pub fn stop(&self, plugin: &str) -> Vec<u32> {
        let taken: Vec<(u32, Option<Guard>)> = {
            let mut inner = self.lock();
            let views: Vec<u32> = inner
                .leases
                .iter()
                .filter(|(_, lease)| lease.plugin == plugin)
                .map(|(view, _)| *view)
                .collect();
            views
                .into_iter()
                .filter_map(|view| inner.leases.remove(&view).map(|lease| (view, lease.pause)))
                .collect()
        };
        // The pauses drop here, outside the lock: the views go back into the SyncHub.
        taken
            .into_iter()
            .filter_map(|(view, pause)| pause.map(|_| view))
            .collect()
    }
}

/// Plugin `plugin`'s `cdp.send` of [`SET`] or [`CLEAR`] to `view`, as the lease's step; the
/// answer is `{}`, the end's `{ "ended": bool }` (whether a live lease ended).
///
/// # Errors
///
/// The view is no attached Chromium view, the parameters name no width, the lease is refused
/// ([`WidthLeases::take`]), or the metrics were not taken within `timeout`.
pub async fn send(
    leases: &WidthLeases,
    core: &Core,
    plugin: &str,
    view: ViewId,
    method: &str,
    params: &str,
    timeout: Duration,
) -> Result<String, Failure> {
    let host = core.host();
    page::chromium(&**host, view, "cdp")?;
    // A restore in flight is sent first: this width comes after it and holds.
    let gate = leases.gate(view.0);
    let _sending = gate.lock().await;
    if method == CLEAR {
        let ended = leases.end(plugin, view.0);
        if ended {
            restore(host, view, timeout).await;
        }
        return Ok(json!({ "ended": ended }).to_string());
    }
    let width = requested_width(params)
        .ok_or_else(|| Failure::coded(FailureKind::BadData, codes::INVALID))?;
    let metrics = host.width_metrics(view, width).ok_or_else(|| {
        Failure::coded(FailureKind::NotFound, codes::NOT_FOUND)
            .with_param("view", view.0.to_string())
    })?;
    leases.take(plugin, view.0, || pause(core, view))?;
    page::cdp(&**host, view, SET, &metrics.to_string(), timeout)
        .await
        .map(|_| "{}".to_owned())
}

/// `view` out of the window's SyncHub while the guard lives.
fn pause(core: &Core, view: ViewId) -> Guard {
    match crate::sync_hub::for_host(core.host()) {
        Some(sync) => Box::new(SyncPause::new(sync, &[view])),
        None => Box::new(()),
    }
}

/// Sends `view` the host's own metrics again; a view gone since is skipped.
pub async fn restore(host: &Host, view: ViewId, timeout: Duration) {
    let Some(target) = host.capture_target(view) else {
        return;
    };
    let params = target.metrics.to_string();
    if let Err(failure) = page::cdp(host, view, SET, &params, timeout).await {
        tracing::debug!(%view, %failure, "a view's own width was not restored after a plugin's");
    }
}

/// Runs `send` - a restore of `view`'s own metrics after its lease of `generation` ended -
/// unless a newer lease was taken of the view by now; whether it ran. A restore spawned when a
/// plugin's call ended runs later: a lease taken in between (the next sweep call) keeps its
/// width, and its own end restores the view. The check and the send are one step under the
/// view's gate ([`WidthLeases::gate`]), so a width sent meanwhile comes after the restore.
pub async fn restore_unless_leased<F, Fut>(
    leases: &WidthLeases,
    view: u32,
    generation: u64,
    send: F,
) -> bool
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = ()>,
{
    let gate = leases.gate(view);
    let _sending = gate.lock().await;
    if leases.leased(view) || leases.generation(view) != generation {
        return false;
    }
    send().await;
    true
}

/// [`restore`] of `views` on `core`'s runtime, off the plugin's call, each skipped when a lease
/// took the view again in the meantime ([`restore_unless_leased`]).
pub fn restore_later(core: &Core, leases: &Arc<WidthLeases>, views: Vec<u32>) {
    if views.is_empty() {
        return;
    }
    // The generation of the lease that just ended: a newer one owns the view's restore.
    let views: Vec<(u32, u64)> = views
        .into_iter()
        .map(|view| (view, leases.generation(view)))
        .collect();
    let host = Arc::downgrade(core.host());
    let leases = Arc::clone(leases);
    core.runtime().spawn(async move {
        for (view, generation) in views {
            let Some(host) = host.upgrade() else {
                return;
            };
            let send = || restore(&host, ViewId(view), RESTORE_TIMEOUT);
            restore_unless_leased(&leases, view, generation, send).await;
        }
    });
}

#[cfg(test)]
#[path = "widths_tests.rs"]
mod tests;
