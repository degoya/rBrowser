//! The Web Vitals report of the sync agent (RB-060-10, `docs/sync-protocol.md`, "Web Vitals"):
//! the top document's LCP, CLS, INP, FCP and TTFB of the current segment, measured in the agent's
//! isolated world (`agent/src/vitals/`). A report comes from a renderer the core does not trust:
//! [`VitalsMessage::check`] holds the values to a sane range and the selectors to a length
//! without control characters before the core keeps or shows any of it.

use serde::{Deserialize, Serialize};

/// The longest selector a report may carry, in characters.
pub const MAX_SELECTOR_CHARS: usize = 1024;
/// The most shift sources a report may name.
pub const MAX_SOURCES: usize = 5;
/// The longest URL a report may carry, in characters.
pub const MAX_URL_CHARS: usize = 8192;
/// The largest time taken, in ms (a day): anything above is a broken clock, not a page.
pub const MAX_TIME_MS: u32 = 86_400_000;
/// The largest CLS taken.
pub const MAX_CLS: f64 = 1000.0;

/// A Web Vital.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VitalMetric {
    /// Largest Contentful Paint.
    Lcp,
    /// Cumulative Layout Shift.
    Cls,
    /// Interaction to Next Paint.
    Inp,
    /// First Contentful Paint.
    Fcp,
    /// Time to First Byte.
    Ttfb,
}

impl VitalMetric {
    /// Every metric, in the order the label shows them.
    pub const ALL: [Self; 5] = [Self::Lcp, Self::Cls, Self::Inp, Self::Fcp, Self::Ttfb];

    /// The id as the protocol, the settings and MCP write it.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Lcp => "lcp",
            Self::Cls => "cls",
            Self::Inp => "inp",
            Self::Fcp => "fcp",
            Self::Ttfb => "ttfb",
        }
    }
}

/// LCP with the element that painted it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LcpValue {
    /// ms since the navigation started.
    pub value: u32,
    /// The element's CSS path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
}

/// CLS with its largest sources.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClsValue {
    /// The largest session window's sum.
    pub value: f64,
    /// The CSS paths of the shifted elements, the largest first.
    pub sources: Vec<String>,
}

/// INP with the target of the interaction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InpValue {
    /// ms.
    pub value: u32,
    /// The CSS path of the interaction's target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// The interactions of the segment so far.
    pub interactions: u32,
}

/// The agent's Web Vitals report (agent -> core, `kind: "vitals"`); only the top frame sends
/// one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VitalsMessage {
    /// The protocol version.
    pub v: u32,
    /// Whether the agent runs in the top frame.
    pub top: bool,
    /// As [`crate::Ready::frame`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<Vec<String>>,
    /// 0 for the document load, counting up per soft navigation.
    pub segment: u32,
    /// A soft navigation's segment: only CLS and INP are measured.
    pub soft: bool,
    /// The document's URL.
    pub url: String,
    /// Largest Contentful Paint; absent until painted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lcp: Option<LcpValue>,
    /// Cumulative Layout Shift; absent when the engine has no layout shift entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cls: Option<ClsValue>,
    /// Interaction to Next Paint; absent before the first interaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inp: Option<InpValue>,
    /// First Contentful Paint, ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fcp: Option<u32>,
    /// Time to First Byte, ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttfb: Option<u32>,
    /// What the engine cannot measure.
    pub unsupported: Vec<VitalMetric>,
}

/// Why a report is not taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum VitalsError {
    /// A value is out of range or not finite.
    #[error("a Web Vital is out of range")]
    Value,
    /// A selector is too long, holds control characters, or there are too many.
    #[error("a Web Vitals selector is malformed")]
    Selector,
    /// The URL is too long.
    #[error("the Web Vitals URL is too long")]
    Url,
}

fn time_ok(value: u32) -> bool {
    value <= MAX_TIME_MS
}

fn selector_ok(selector: &str) -> bool {
    !selector.is_empty()
        && selector.chars().count() <= MAX_SELECTOR_CHARS
        && !selector.chars().any(char::is_control)
}

impl VitalsMessage {
    /// Checks the report's values, selectors and URL.
    ///
    /// # Errors
    ///
    /// The first part that does not hold.
    pub fn check(&self) -> Result<(), VitalsError> {
        let times = [
            self.lcp.as_ref().map(|lcp| lcp.value),
            self.inp.as_ref().map(|inp| inp.value),
            self.fcp,
            self.ttfb,
        ];
        let cls_ok = self
            .cls
            .as_ref()
            .is_none_or(|cls| cls.value.is_finite() && (0.0..=MAX_CLS).contains(&cls.value));
        if !cls_ok || !times.into_iter().flatten().all(time_ok) {
            return Err(VitalsError::Value);
        }
        let sources = self
            .cls
            .as_ref()
            .map_or(&[][..], |cls| cls.sources.as_slice());
        let mut selectors = self
            .lcp
            .as_ref()
            .and_then(|lcp| lcp.element.as_deref())
            .into_iter()
            .chain(self.inp.as_ref().and_then(|inp| inp.target.as_deref()))
            .chain(sources.iter().map(String::as_str));
        if sources.len() > MAX_SOURCES || !selectors.all(selector_ok) {
            return Err(VitalsError::Selector);
        }
        if self.url.chars().count() > MAX_URL_CHARS {
            return Err(VitalsError::Url);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "vitals_tests.rs"]
mod tests;
