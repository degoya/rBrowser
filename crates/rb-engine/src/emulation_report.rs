//! What an engine did with an [`Emulation`]: per field `applied`, `approximated(reason)` or
//! `unsupported(reason)`. The view shows the fields that are not `applied` as its "not
//! emulable" marker; `views_list` lists them as `unsupported`.
//!
//! The table per engine follows ADR 0001, Decision 8 (Firefox over BiDi) and Playwright's
//! options (WebKit). It is the contract RB-040-02 and RB-040-04 check on the real engines; a
//! field they find works moves to `applied` here, so every place that shows it agrees.

use rb_config::{EngineKind, MediaType};
use serde::{Deserialize, Serialize};

use crate::Emulation;

/// How one field was applied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "support", content = "reason")]
pub enum FieldSupport {
    /// As asked.
    Applied,
    /// Only partly (the user agent of a mobile device, no device mode).
    Approximated(String),
    /// Not at all.
    Unsupported(String),
}

/// One field of the emulation and what became of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldReport {
    /// The field's name as in the config (`reducedMotion`, `media.forcedColors`).
    pub field: String,
    /// What became of it.
    #[serde(flatten)]
    pub support: FieldSupport,
}

/// What an engine did with an emulation: one entry per field the emulation sets.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulationReport {
    /// The fields the emulation sets, in config order.
    pub fields: Vec<FieldReport>,
}

/// Why Firefox cannot: BiDi has no such override (ADR 0001, Decision 8).
const FIREFOX: &str = "no WebDriver BiDi override in Firefox";
/// Why WebKit cannot: Playwright has no such option for WebKit.
const WEBKIT: &str = "no Playwright option for WebKit";

impl EmulationReport {
    /// The report `engine` gives for `emulation`: the fields it sets, each with the engine's
    /// support. Chromium applies everything (CDP, RB-010-05).
    pub fn for_engine(engine: EngineKind, emulation: &Emulation) -> Self {
        let fields = requested(emulation)
            .into_iter()
            .map(|field| FieldReport {
                field: field.to_owned(),
                support: support(engine, field),
            })
            .collect();
        Self { fields }
    }

    /// The fields that were not applied as asked (`unsupported` and `approximated`).
    pub fn unsupported(&self) -> Vec<String> {
        self.fields
            .iter()
            .filter(|report| report.support != FieldSupport::Applied)
            .map(|report| report.field.clone())
            .collect()
    }
}

/// The fields `emulation` sets beyond the engine's own behaviour, in config order. Viewport and
/// DPR are always set.
fn requested(emulation: &Emulation) -> Vec<&'static str> {
    let media = &emulation.media;
    let network = emulation.network.as_ref();
    [
        ("viewport", true),
        ("dpr", true),
        ("mobile", emulation.mobile),
        ("touch", emulation.touch),
        ("userAgent", emulation.user_agent.is_some()),
        ("platform", emulation.platform.is_some()),
        ("locale", emulation.locale.is_some()),
        ("colorScheme", emulation.color_scheme.is_some()),
        ("reducedMotion", emulation.reduced_motion == Some(true)),
        ("media.reducedData", media.reduced_data == Some(true)),
        (
            "media.reducedTransparency",
            media.reduced_transparency == Some(true),
        ),
        ("media.forcedColors", media.forced_colors.is_some()),
        ("media.contrast", media.contrast.is_some()),
        ("media.colorGamut", media.color_gamut.is_some()),
        (
            "media.mediaType",
            media.media_type == Some(MediaType::Print),
        ),
        ("network.offline", network.is_some_and(|n| n.offline)),
        (
            "network.throttling",
            network.is_some_and(|n| n.latency_ms > 0 || n.download_kbps > 0 || n.upload_kbps > 0),
        ),
        ("headers", !emulation.headers.is_empty()),
        ("orientation", emulation.orientation.is_some()),
        ("safeAreas", emulation.safe_areas.is_some()),
        ("textScale", emulation.text_scale.is_some()),
        ("geolocation", emulation.geolocation.is_some()),
        ("timezone", emulation.timezone.is_some()),
        ("cpuThrottling", emulation.cpu_throttling.is_some()),
    ]
    .into_iter()
    .filter(|(_, set)| *set)
    .map(|(field, _)| field)
    .collect()
}

fn support(engine: EngineKind, field: &str) -> FieldSupport {
    let unsupported = |reason: &str| FieldSupport::Unsupported(reason.to_owned());
    match engine {
        EngineKind::Chromium => FieldSupport::Applied,
        EngineKind::Firefox => match field {
            "mobile" => FieldSupport::Approximated(
                "Firefox has no mobile device mode; user agent and viewport only".to_owned(),
            ),
            "touch"
            | "reducedMotion"
            | "media.reducedData"
            | "media.reducedTransparency"
            | "media.forcedColors"
            | "media.contrast"
            | "media.colorGamut"
            | "media.mediaType"
            | "network.throttling"
            | "safeAreas"
            | "textScale"
            | "geolocation"
            | "timezone"
            | "cpuThrottling" => unsupported(FIREFOX),
            "platform" => {
                FieldSupport::Approximated("navigator.platform stays the host's".to_owned())
            }
            _ => FieldSupport::Applied,
        },
        EngineKind::Webkit => match field {
            "platform"
            | "media.reducedData"
            | "media.reducedTransparency"
            | "media.colorGamut"
            | "network.throttling"
            | "safeAreas"
            | "textScale"
            | "cpuThrottling" => unsupported(WEBKIT),
            _ => FieldSupport::Applied,
        },
    }
}

#[cfg(test)]
#[path = "emulation_report_tests.rs"]
mod tests;
