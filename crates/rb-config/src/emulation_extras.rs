//! The emulation extras of a breakpoint (RB-080-10): geolocation, time zone, CPU throttling,
//! offline and the paper of a print view, with the rules a schema cannot say - IANA time zone
//! names, coordinate ranges, the throttling factor. The fields live on
//! [`crate::EmulationFields`]; RB-080-10 resolves them in `rb-engine` and maps them onto CDP in
//! the app.

use std::sync::OnceLock;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{ConfigIssue, EmulationFields, Viewport};

/// The accuracy a geolocation without one reports, in metres.
pub const DEFAULT_ACCURACY_M: f64 = 50.0;
/// The smallest CPU throttling factor: `1` is no throttling.
pub const MIN_CPU_THROTTLING: f64 = 1.0;
/// The largest CPU throttling factor.
pub const MAX_CPU_THROTTLING: f64 = 100.0;

/// The IANA time zone names (zones and links), sorted.
const TIMEZONES: &str = include_str!("../data/timezones.txt");

/// An emulated position (`navigator.geolocation`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Geolocation {
    /// Latitude in degrees, -90 to 90.
    #[schemars(range(min = -90.0, max = 90.0))]
    pub latitude: f64,
    /// Longitude in degrees, -180 to 180.
    #[schemars(range(min = -180.0, max = 180.0))]
    pub longitude: f64,
    /// Accuracy in metres; 50 when absent.
    #[serde(default = "default_accuracy")]
    #[schemars(range(min = 0.0))]
    pub accuracy: f64,
}

fn default_accuracy() -> f64 {
    DEFAULT_ACCURACY_M
}

impl Geolocation {
    /// What is wrong with the values, if anything: a coordinate out of range or not finite, a
    /// negative accuracy.
    pub fn problem(&self) -> Option<&'static str> {
        if !(self.latitude.is_finite() && (-90.0..=90.0).contains(&self.latitude)) {
            return Some("latitude is not in -90 to 90");
        }
        if !(self.longitude.is_finite() && (-180.0..=180.0).contains(&self.longitude)) {
            return Some("longitude is not in -180 to 180");
        }
        if !(self.accuracy.is_finite() && self.accuracy >= 0.0) {
            return Some("accuracy is not a distance of 0 m or more");
        }
        None
    }
}

/// The paper a print view is as wide (and high) as, at 96 dpi.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum Paper {
    /// ISO A4, 210 x 297 mm: 794 x 1123 px.
    A4,
    /// US Letter, 8.5 x 11 in: 816 x 1056 px.
    Letter,
}

impl Paper {
    /// The paper's size in CSS px at 96 dpi.
    pub fn viewport(self) -> Viewport {
        match self {
            Paper::A4 => Viewport {
                width: 794,
                height: 1123,
            },
            Paper::Letter => Viewport {
                width: 816,
                height: 1056,
            },
        }
    }

    /// The id part of a print breakpoint on this paper (`a4`, `letter`).
    pub fn id(self) -> &'static str {
        match self {
            Paper::A4 => "a4",
            Paper::Letter => "letter",
        }
    }
}

/// Whether `name` is an IANA time zone name (a zone or a link, as tzdb spells it).
pub fn is_iana_timezone(name: &str) -> bool {
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    let names = NAMES.get_or_init(|| {
        let mut names: Vec<&str> = TIMEZONES
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();
        names.sort_unstable();
        names
    });
    names.binary_search(&name).is_ok()
}

/// What is wrong with a CPU throttling factor, if anything.
pub fn cpu_throttling_problem(rate: f64) -> Option<&'static str> {
    (!(rate.is_finite() && (MIN_CPU_THROTTLING..=MAX_CPU_THROTTLING).contains(&rate)))
        .then_some("is not a factor from 1 to 100")
}

/// The problems of the extras of `fields` (a breakpoint at `at`, or the project's `/emulation`
/// defaults): a geolocation out of range, a time zone tzdb does not know, a throttling factor
/// outside 1 to 100. An empty time zone is the engine's own.
pub fn check_extras(fields: &EmulationFields, at: &str) -> Vec<ConfigIssue> {
    let mut issues = Vec::new();
    if let Some(Some(geolocation)) = &fields.geolocation
        && let Some(why) = geolocation.problem()
    {
        issues.push(ConfigIssue {
            path: format!("{at}/geolocation"),
            key: "config.error.geolocation_range".to_owned(),
            message: format!("geolocation: {why}"),
        });
    }
    if let Some(timezone) = fields.timezone.as_deref()
        && !timezone.is_empty()
        && !is_iana_timezone(timezone)
    {
        issues.push(ConfigIssue {
            path: format!("{at}/timezone"),
            key: "config.error.unknown_timezone".to_owned(),
            message: format!("timezone `{timezone}` is not an IANA time zone name"),
        });
    }
    if let Some(rate) = fields.cpu_throttling
        && let Some(why) = cpu_throttling_problem(rate)
    {
        issues.push(ConfigIssue {
            path: format!("{at}/cpuThrottling"),
            key: "config.error.cpu_throttling_range".to_owned(),
            message: format!("cpuThrottling {rate} {why}"),
        });
    }
    issues
}

/// Reads a field whose `null` means something (explicitly off) apart from its absence (unset,
/// with `#[serde(default)]`): `null` is `Some(None)`.
pub(crate) fn explicit_null<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[cfg(test)]
#[path = "emulation_extras_tests.rs"]
mod tests;
