//! `vitals` and `resources` of the user settings (RB-060-10): the Web Vitals badges with their
//! rating thresholds (web.dev's by default) and the CPU and memory readings per view with their
//! warning thresholds. Settings > DevTools & Debugging > Web Vitals & resources.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One metric's rating thresholds: at most `good` is good, above `poor` is poor, between them
/// needs improvement.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VitalThreshold {
    /// The highest good value.
    #[schemars(range(min = 0.0))]
    pub good: f64,
    /// The highest value that only needs improvement.
    #[schemars(range(min = 0.0))]
    pub poor: f64,
}

impl VitalThreshold {
    /// Whether the pair can rate: both finite, not negative, `good` at most `poor`.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.good.is_finite() && self.poor.is_finite() && self.good >= 0.0 && self.good <= self.poor
    }
}

/// The thresholds per metric; times in ms, CLS unitless.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct VitalThresholds {
    /// Largest Contentful Paint.
    pub lcp: VitalThreshold,
    /// Cumulative Layout Shift.
    pub cls: VitalThreshold,
    /// Interaction to Next Paint.
    pub inp: VitalThreshold,
    /// First Contentful Paint.
    pub fcp: VitalThreshold,
    /// Time to First Byte.
    pub ttfb: VitalThreshold,
}

impl Default for VitalThresholds {
    /// web.dev's thresholds (`https://web.dev/articles/vitals`).
    fn default() -> Self {
        Self {
            lcp: VitalThreshold {
                good: 2500.0,
                poor: 4000.0,
            },
            cls: VitalThreshold {
                good: 0.1,
                poor: 0.25,
            },
            inp: VitalThreshold {
                good: 200.0,
                poor: 500.0,
            },
            fcp: VitalThreshold {
                good: 1800.0,
                poor: 3000.0,
            },
            ttfb: VitalThreshold {
                good: 800.0,
                poor: 1800.0,
            },
        }
    }
}

/// `vitals`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct VitalsSettings {
    /// Measure the Web Vitals of every view and show them as badges in the cell label.
    pub enabled: bool,
    /// The rating thresholds per metric; web.dev's by default.
    pub thresholds: VitalThresholds,
}

impl Default for VitalsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            thresholds: VitalThresholds::default(),
        }
    }
}

/// `resources`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ResourcesSettings {
    /// Read each view's CPU share and memory and show them in the cell label.
    pub enabled: bool,
    /// A view whose memory is above this many MB warns.
    #[schemars(range(min = 16, max = 65_536))]
    pub ram_warn_mb: u32,
    /// A view whose CPU share stays above this percentage for five seconds warns.
    #[schemars(range(min = 1, max = 100))]
    pub cpu_warn_percent: u32,
}

impl Default for ResourcesSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            ram_warn_mb: 500,
            cpu_warn_percent: 50,
        }
    }
}

#[cfg(test)]
#[path = "user_vitals_tests.rs"]
mod tests;
