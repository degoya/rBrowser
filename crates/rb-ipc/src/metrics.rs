//! The Web Vitals and resources of each view, as the label strips show them (RB-060-10): the
//! state's `metrics`, rated and ready to show - the strips (`rbrowser::cell_info`, RB-090-27)
//! format, they do not judge. Mirror: `ui/src/ipc/metrics.ts`.

use rb_engine::ViewId;
use rb_sync::VitalMetric;
use serde::{Deserialize, Serialize};

/// A rating after web.dev's scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rating {
    /// At most the `good` threshold.
    Good,
    /// Between the thresholds.
    NeedsImprovement,
    /// Above the `poor` threshold.
    Poor,
}

/// Why a reading has no value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Unavailable {
    /// Not measured yet (no paint, no interaction).
    Pending,
    /// The view's engine cannot measure it.
    Unsupported,
    /// A soft navigation's segment: the load metrics belong to the document load.
    Soft,
    /// The view's engine reports nothing of this kind.
    Engine,
}

/// One Web Vital of a view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VitalReading {
    /// The metric.
    pub metric: VitalMetric,
    /// ms, or unitless for CLS; absent when unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// The value's rating.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<Rating>,
    /// Why there is no value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<Unavailable>,
    /// A click marks the element in the view (the LCP element, the largest shift source, the
    /// interaction's target).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub highlight: bool,
}

/// The CPU share and memory of a view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceReading {
    /// The share of the page's main thread busy since the last reading, in percent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_percent: Option<f64>,
    /// The page's JS heap, in MB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_mb: Option<f64>,
    /// The CPU share stayed above `resources.cpuWarnPercent` for five seconds.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cpu_warn: bool,
    /// The memory is above `resources.ramWarnMb`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ram_warn: bool,
    /// Why there are no values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<Unavailable>,
}

/// A view the performance budget throttles (RB-020-09); a visible one has none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ViewThrottle {
    /// Out of the visible area: hidden for the engine.
    Hidden,
    /// Visible but not active: its CPU is throttled.
    CpuLimited {
        /// The frame rate it is held to.
        fps: u32,
    },
}

/// What a cell label shows beside the view's name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewMetrics {
    /// The view.
    pub view: ViewId,
    /// The Web Vitals in label order; empty while `vitals.enabled` is off.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vitals: Vec<VitalReading>,
    /// The vitals are of a soft navigation's segment.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub soft: bool,
    /// CPU and memory; absent while `resources.enabled` is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceReading>,
    /// The throttling, when the view is throttled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub throttle: Option<ViewThrottle>,
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;
