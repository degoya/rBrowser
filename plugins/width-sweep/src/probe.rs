//! One width of the sweep in the page: the detector (`src/detect.js`, built from `web/src` by
//! `npm run build`; its global `WidthSweepDetect`) evaluated in the plugin's isolated world with
//! the width it waits for, the kinds to look for and whether to forget an Escape pressed before
//! the sweep. Its answer: the width the page had, whether Escape was pressed since, and the
//! issues found.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId};
use serde::Deserialize;
use serde_json::json;

use crate::plan::Issue;

/// The detector, an IIFE defining `WidthSweepDetect`.
pub const DETECT: &str = include_str!("detect.js");
/// A width for the view: the host's width lease (`rb_plugin_host`'s CDP rule takes only
/// `{ "width": n }`).
pub const SET: &str = "Emulation.setDeviceMetricsOverride";
/// The lease's end: the view's own width and sync back. Answers `{ "ended": bool }`.
pub const CLEAR: &str = "Emulation.clearDeviceMetricsOverride";
/// The page's answer was no probe.
pub const PAGE_FAILED_KEY: &str = "plugin.width-sweep.error.page";

/// What the page answered at one width.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Answer {
    /// The page's `innerWidth` when it was looked at.
    pub width: u32,
    /// Escape was pressed in the view since the sweep began.
    #[serde(default)]
    pub escape: bool,
    #[serde(default)]
    pub issues: Vec<Issue>,
}

/// The expression `evaluate` runs at `width`.
#[must_use]
pub fn script(width: u32, kinds: &[String], reset: bool) -> String {
    let options = json!({ "width": width, "kinds": kinds, "reset": reset });
    format!("{DETECT}\n;WidthSweepDetect.probe({options})")
}

/// The page's answer in `text`.
///
/// # Errors
///
/// [`PAGE_FAILED_KEY`] when it is no probe's answer.
pub fn parse(text: &str) -> Result<Answer, Failure> {
    serde_json::from_str(text).map_err(|_| Failure::coded(FailureKind::BadData, PAGE_FAILED_KEY))
}

/// Looks at the page of `view`, which the lease has at `width` now.
///
/// # Errors
///
/// `evaluate`'s failure (the view is gone, the script threw), or [`PAGE_FAILED_KEY`].
pub fn probe(
    host: &mut dyn PluginHost,
    view: ViewId,
    width: u32,
    kinds: &[String],
    reset: bool,
) -> Result<Answer, Failure> {
    let text = host.evaluate(view, &script(width, kinds, reset))?;
    parse(&text)
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
