//! What the core keeps per view (RB-060-10): the agent's last Web Vitals report of the current
//! document and the resource readings, and what the cell labels and `view_metrics` make of them
//! ([`Store::metrics`]).

use std::collections::HashMap;
use std::time::Instant;

use rb_config::{EngineKind, UserSettings, VitalThresholds};
use rb_engine::ViewId;
use rb_ipc::{ResourceReading, Unavailable, ViewMetrics, ViewThrottle};
use rb_sync::VitalsMessage;

use super::rating::{self, Source};
use super::resources::{Change, PageMetrics, ResourceLimits, ViewResources};

/// The settings the store reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// `vitals.enabled`.
    pub vitals: bool,
    /// `vitals.thresholds`.
    pub thresholds: VitalThresholds,
    /// `resources.enabled`.
    pub resources: bool,
    /// `resources.ramWarnMb`, `resources.cpuWarnPercent`.
    pub limits: ResourceLimits,
}

impl Options {
    /// The options in `user`.
    #[must_use]
    pub fn of(user: &UserSettings) -> Self {
        Self {
            vitals: user.vitals.enabled,
            thresholds: user.vitals.thresholds,
            resources: user.resources.enabled,
            limits: ResourceLimits {
                ram_warn_mb: user.resources.ram_warn_mb,
                cpu_warn_percent: user.resources.cpu_warn_percent,
            },
        }
    }
}

/// A view as the metrics need it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewFacts {
    /// The view.
    pub view: ViewId,
    /// Its engine.
    pub engine: EngineKind,
    /// Its throttling (RB-020-09); `None` while visible.
    pub throttle: Option<ViewThrottle>,
}

#[derive(Debug, Default)]
struct Entry {
    report: Option<VitalsMessage>,
    resources: ViewResources,
    reading: Option<ResourceReading>,
}

/// Every view's vitals and resources.
#[derive(Debug, Default)]
pub struct Store {
    views: HashMap<ViewId, Entry>,
}

impl Store {
    /// A checked report of `view`'s top document; replaces the one before.
    pub fn on_report(&mut self, view: ViewId, report: VitalsMessage) {
        self.views.entry(view).or_default().report = Some(report);
    }

    /// A new document started in `view`: its old report is gone.
    pub fn on_document(&mut self, view: ViewId) {
        if let Some(entry) = self.views.get_mut(&view) {
            entry.report = None;
        }
    }

    /// One resource sample of `view`: the warnings that started or ended.
    pub fn on_sample(
        &mut self,
        view: ViewId,
        metrics: PageMetrics,
        now: Instant,
        limits: ResourceLimits,
    ) -> Vec<Change> {
        let entry = self.views.entry(view).or_default();
        let (reading, changes) = entry.resources.sample(metrics, now, limits);
        entry.reading = Some(reading);
        changes
    }

    /// `view` could not be sampled (no page yet, the view is closing).
    pub fn on_no_sample(&mut self, view: ViewId) {
        if let Some(entry) = self.views.get_mut(&view) {
            entry.reading = None;
        }
    }

    /// Resources were switched off: the readings and the warnings start over.
    pub fn clear_resources(&mut self) {
        for entry in self.views.values_mut() {
            entry.resources = ViewResources::default();
            entry.reading = None;
        }
    }

    /// Forgets the views not in `open`.
    pub fn retain(&mut self, open: &[ViewId]) {
        self.views.retain(|view, _| open.contains(view));
    }

    /// `view`'s last report.
    #[must_use]
    pub fn report(&self, view: ViewId) -> Option<&VitalsMessage> {
        self.views.get(&view)?.report.as_ref()
    }

    /// What the labels show for `views`, in their order.
    #[must_use]
    pub fn metrics(&self, views: &[ViewFacts], options: &Options) -> Vec<ViewMetrics> {
        views
            .iter()
            .map(|facts| {
                let entry = self.views.get(&facts.view);
                let (vitals, soft) = if options.vitals {
                    let source = match facts.engine {
                        EngineKind::Chromium | EngineKind::Firefox => Source::Reports,
                        EngineKind::Webkit
                        | EngineKind::Android
                        | EngineKind::Safari
                        | EngineKind::Ios => Source::None,
                    };
                    let report = entry.and_then(|entry| entry.report.as_ref());
                    rating::readings(report, source, &options.thresholds)
                } else {
                    (Vec::new(), false)
                };
                let resources = options.resources.then(|| {
                    if facts.engine != EngineKind::Chromium {
                        return unavailable(Unavailable::Engine);
                    }
                    entry
                        .and_then(|entry| entry.reading.clone())
                        .unwrap_or_else(|| unavailable(Unavailable::Pending))
                });
                ViewMetrics {
                    view: facts.view,
                    vitals,
                    soft,
                    resources,
                    throttle: facts.throttle,
                }
            })
            .collect()
    }
}

fn unavailable(reason: Unavailable) -> ResourceReading {
    ResourceReading {
        cpu_percent: None,
        memory_mb: None,
        cpu_warn: false,
        ram_warn: false,
        unavailable: Some(reason),
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
