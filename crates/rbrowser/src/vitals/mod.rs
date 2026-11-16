//! Web Vitals and resources per view (RB-060-10), in the core, not a plugin. The sync agent
//! measures the top document's LCP, CLS, INP, FCP and TTFB in its isolated world
//! (`agent/src/vitals/`) and reports them over the binding (`rb_sync::VitalsMessage`, checked
//! here before anything is kept); the tick reads each Chromium view's CPU share and JS heap over
//! CDP (`resources.rs`). Both are rated and shown as badges in the cell labels (the state's
//! `metrics`), and answered by the MCP tool `view_metrics` (`mcp.rs`). A badge's click marks
//! the element behind it (`highlightVital`, through the core highlight of RB-060-01).
//!
//! | File | What |
//! | --- | --- |
//! | `rating.rs` | web.dev's rating with `vitals.thresholds`; the readings per view |
//! | `resources.rs` | CPU share and memory from `Performance.getMetrics`; the warnings |
//! | `store.rs` | the reports and readings per view; what the labels show |
//! | `tasks.rs` | the report listener and the tick |
//! | `defs.rs` | `debug.vitals` (`Leader D C`), `debug.resources` (`Leader D R`) |
//! | `mcp.rs` | `view_metrics` |
//! | `throttle.rs` | the throttling of RB-020-09 (`crate::perf`) |

pub mod defs;
pub mod mcp;
pub mod rating;
pub mod resources;
pub mod store;
mod tasks;
pub mod throttle;

use std::sync::{Arc, Mutex, Weak};

use rb_engine::{ViewHost as _, ViewId};
use rb_ipc::{CoreToShell, IpcErrorCode, ShellToCore};
use rb_sync::{AgentMessage, VitalMetric};

pub use defs::{RESOURCES, VITALS, definitions, register};
pub use mcp::VIEW_METRICS_TOOL;

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;
use crate::ipc::Failure;
use store::{Options, Store, ViewFacts};

/// A window's metrics.
pub struct HostVitals {
    host: Weak<Host>,
    core: Weak<Core>,
    store: Mutex<Store>,
}

impl HostVitals {
    fn host(&self) -> Option<Arc<Host>> {
        self.host.upgrade()
    }

    /// The settings in effect; the defaults once the window is gone.
    fn options(&self) -> Options {
        self.core.upgrade().map_or_else(
            || Options::of(&rb_config::UserSettings::default()),
            |core| Options::of(&core.user_settings()),
        )
    }

    /// Runs `work` on the store.
    pub fn with_store<T>(&self, work: impl FnOnce(&mut Store) -> T) -> T {
        work(&mut lock(&self.store))
    }

    /// An agent's message from `view`: a top document's start or its checked vitals.
    pub fn on_agent_message(&self, view: ViewId, message: &AgentMessage) {
        match message {
            AgentMessage::Ready(ready) if ready.top => {
                self.with_store(|store| store.on_document(view));
            }
            AgentMessage::Vitals(vitals) if vitals.top => {
                if let Err(error) = vitals.check() {
                    tracing::debug!(%view, %error, "a Web Vitals report was refused");
                    return;
                }
                // Kept while the badges are off too, so `debug.vitals` shows the last readings
                // at once (RB-080-18); `publish` leaves them out until they are on.
                self.with_store(|store| store.on_report(view, vitals.clone()));
                if self.options().vitals
                    && let Some(host) = self.host()
                {
                    self.publish(&host);
                }
            }
            _ => {}
        }
    }

    /// Shows the labels' metrics again; the views gone are forgotten.
    fn publish(&self, host: &Host) {
        let views = host.views();
        let open: Vec<ViewId> = views.iter().map(|info| info.id).collect();
        let facts: Vec<ViewFacts> = views
            .iter()
            .map(|info| ViewFacts {
                view: info.id,
                engine: info.engine,
                throttle: throttle::throttle_of(host, info.id),
            })
            .collect();
        let options = self.options();
        let metrics = self.with_store(|store| {
            store.retain(&open);
            if options.vitals || options.resources {
                store.metrics(&facts, &options)
            } else {
                Vec::new()
            }
        });
        host.update_shell_state(|shell| shell.metrics = metrics);
    }
}

fn registry() -> &'static Mutex<Vec<(Weak<Host>, Arc<HostVitals>)>> {
    static REGISTRY: Mutex<Vec<(Weak<Host>, Arc<HostVitals>)>> = Mutex::new(Vec::new());
    &REGISTRY
}

/// The metrics of `host`'s window, once [`on_start`] ran for it.
pub fn for_host(host: &Host) -> Option<Arc<HostVitals>> {
    let mut entries = lock(registry());
    entries.retain(|(weak, _)| weak.strong_count() > 0);
    entries
        .iter()
        .find(|(weak, _)| std::ptr::eq(weak.as_ptr(), host))
        .map(|(_, vitals)| vitals.clone())
}

/// Starts the report listener and the tick of `core`'s window.
pub fn on_start(core: &Core) {
    if for_host(core.host()).is_some() {
        return;
    }
    let vitals = Arc::new(HostVitals {
        host: Arc::downgrade(core.host()),
        core: core.arc().as_ref().map(Arc::downgrade).unwrap_or_default(),
        store: Mutex::new(Store::default()),
    });
    tasks::start(&vitals, core.host(), core.runtime());
    lock(registry()).push((Arc::downgrade(core.host()), vitals));
}

/// A report of an agent the host's own report stream does not carry (Firefox, RB-040-02).
pub fn on_agent_message(host: &Host, view: ViewId, message: &AgentMessage) {
    if let Some(vitals) = for_host(host) {
        vitals.on_agent_message(view, message);
    }
}

/// Shows the labels' metrics again at once (a switch changed).
pub fn publish(core: &Core) {
    if let Some(vitals) = for_host(core.host()) {
        vitals.publish(core.host());
    }
}

/// `highlightVital`: marks the element behind `metric` in `view`, taken from the view's own
/// report; `None` for every other message.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let ShellToCore::HighlightVital { view, metric } = message else {
        return None;
    };
    Some(highlight(core, *view, *metric))
}

fn highlight(core: &Core, view: ViewId, metric: VitalMetric) -> Result<CoreToShell, Failure> {
    let host = core.host();
    let selector = for_host(host)
        .and_then(|vitals| {
            vitals.with_store(|store| {
                store
                    .report(view)
                    .and_then(|report| rating::selector_of(report, metric))
                    .map(str::to_owned)
            })
        })
        .ok_or_else(|| {
            (
                IpcErrorCode::NotFound,
                format!("view {view} has no element for {}", metric.id()),
            )
        })?;
    let host = host.clone();
    core.runtime().spawn(async move {
        if let Err(error) =
            crate::checks::highlight::highlight_view(host.as_ref(), view, &selector).await
        {
            tracing::debug!(%view, %error, "the Web Vitals highlight failed");
        }
    });
    Ok(CoreToShell::Ack)
}
