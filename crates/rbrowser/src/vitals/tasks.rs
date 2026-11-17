//! The two tasks of a window's metrics (RB-060-10), both as long as the host lives: the agents'
//! reports (a checked Web Vitals report is kept, a new document drops the old one), and the
//! tick every [`TICK`] that samples each Chromium view's resources over CDP while
//! `resources.enabled` is on, logs a warning's start and end, and shows the labels again.

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::future::join_all;
use rb_config::EngineKind;
use rb_engine::{ViewHost as _, ViewId};
use serde_json::{Value, json};
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;

use super::HostVitals;
use super::resources::{Change, PageMetrics, ResourceLimits, parse_metrics};
use crate::host::Host;

/// How often the resources are read and the labels refreshed.
pub const TICK: Duration = Duration::from_secs(2);

/// Starts both tasks.
pub fn start(vitals: &Arc<HostVitals>, host: &Arc<Host>, runtime: &Handle) {
    let mut reports = host.sync_agents().subscribe();
    let weak = Arc::downgrade(vitals);
    runtime.spawn(async move {
        loop {
            let report = match reports.recv().await {
                Ok(report) => report,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return,
            };
            let Some(vitals) = weak.upgrade() else {
                return;
            };
            vitals.on_agent_message(report.view, &report.message);
        }
    });
    let weak = Arc::downgrade(vitals);
    runtime.spawn(async move {
        let mut tick = tokio::time::interval(TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut sampling = false;
        loop {
            tick.tick().await;
            let Some(vitals) = weak.upgrade() else {
                return;
            };
            let Some(host) = vitals.host() else {
                return;
            };
            let options = vitals.options();
            if options.resources {
                sample(&vitals, &host, options.limits).await;
                sampling = true;
            } else if sampling {
                vitals.with_store(super::store::Store::clear_resources);
                sampling = false;
            }
            vitals.publish(&host);
        }
    });
}

/// `view`'s page metrics; the Performance domain is enabled when they are missing, so the
/// next tick has them.
async fn page_metrics(host: &Host, view: ViewId) -> Option<PageMetrics> {
    let answer = host
        .cdp_send(view, "Performance.getMetrics", json!({}))
        .await
        .unwrap_or(Value::Null);
    let metrics = parse_metrics(&answer);
    if metrics.is_none()
        && let Err(error) = host
            .cdp_send(
                view,
                "Performance.enable",
                json!({ "timeDomain": "timeTicks" }),
            )
            .await
    {
        tracing::debug!(%view, %error, "Performance.enable failed");
    }
    metrics
}

/// Samples every Chromium view at once and logs the warnings that started or ended.
async fn sample(vitals: &HostVitals, host: &Host, limits: ResourceLimits) {
    let views: Vec<ViewId> = host
        .views()
        .into_iter()
        .filter(|info| info.engine == EngineKind::Chromium)
        .map(|info| info.id)
        .collect();
    let samples = join_all(
        views
            .iter()
            .map(|view| async move { (*view, page_metrics(host, *view).await) }),
    )
    .await;
    let now = Instant::now();
    for (view, metrics) in samples {
        let Some(metrics) = metrics else {
            vitals.with_store(|store| store.on_no_sample(view));
            continue;
        };
        let changes = vitals.with_store(|store| store.on_sample(view, metrics, now, limits));
        for change in changes {
            log(view, change, limits);
        }
    }
}

/// The log entry of a warning's start or end.
fn log(view: ViewId, change: Change, limits: ResourceLimits) {
    match change {
        Change::RamWarn(memory_mb) => tracing::warn!(
            %view,
            memory_mb,
            limit_mb = limits.ram_warn_mb,
            "view memory is above resources.ramWarnMb"
        ),
        Change::RamCalm => tracing::info!(%view, "view memory is below resources.ramWarnMb again"),
        Change::CpuWarn(cpu_percent) => tracing::warn!(
            %view,
            cpu_percent,
            limit_percent = limits.cpu_warn_percent,
            "view CPU share stayed above resources.cpuWarnPercent for 5 s"
        ),
        Change::CpuCalm => {
            tracing::info!(%view, "view CPU share is below resources.cpuWarnPercent again");
        }
    }
}
