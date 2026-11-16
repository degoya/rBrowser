//! What a baseline was taken with (`meta.json` beside the PNGs of a page): machine, operating
//! system, pixel density, engine and its version, the app, the page and the viewport, and when.
//! A comparison is only as good as the match of the rendering setup; [`compare_meta`] names
//! every field of it that differs, so a baseline from another engine version or another DPI
//! gives a warning ("comparison unreliable") instead of a silent wrong result. Machine and
//! operating system are only recorded: a team shares its baselines across machines.
//!
//! The file is checked in: the URL is kept without its query, fragment and credentials.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The version of `meta.json`.
pub const META_VERSION: u32 = 1;

/// A CSS viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Viewport {
    /// CSS px across.
    pub width: u32,
    /// CSS px down.
    pub height: u32,
}

/// What one baseline was taken with.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BaselineMeta {
    /// The machine's name.
    pub machine: String,
    /// The operating system (`windows`, `linux`, `macos`).
    pub os: String,
    /// The monitor's DPI, when the app knows it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitor_dpi: Option<f64>,
    /// Device pixels per CSS pixel of the image (its width over the viewport's).
    pub dpr: f64,
    /// `chromium`, `firefox` or `webkit`.
    pub engine: String,
    /// The engine's version (Chromium's for the built-in engine).
    pub engine_version: String,
    /// The CEF build of the built-in engine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cef_version: Option<String>,
    /// rBrowser's version.
    pub app_version: String,
    /// The page, without query, fragment and credentials.
    pub url: String,
    /// The page's folder below `.rbrowser/baselines`.
    pub slug: String,
    /// The breakpoint's viewport.
    pub viewport: Viewport,
    /// The whole page, not only the viewport.
    pub full_page: bool,
    /// Unix seconds.
    pub created_at: u64,
}

/// `meta.json`: the metadata of each baseline of a page, by breakpoint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaFile {
    /// [`META_VERSION`].
    pub version: u32,
    /// By breakpoint name.
    #[serde(default)]
    pub baselines: BTreeMap<String, BaselineMeta>,
}

impl Default for MetaFile {
    fn default() -> Self {
        Self {
            version: META_VERSION,
            baselines: BTreeMap::new(),
        }
    }
}

/// One field that differs between a baseline and the capture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaMismatch {
    /// The field's name in `meta.json` (`engineVersion`).
    pub field: String,
    /// The baseline's value.
    pub baseline: String,
    /// The capture's value.
    pub current: String,
}

/// Two pixel ratios closer than this are the same.
const DPR_TOLERANCE: f64 = 0.01;

/// The fields of `current` that differ from `baseline` and make a comparison unreliable:
/// monitor DPI (when both know it), pixel ratio, engine and its version, CEF build, viewport
/// and full page. Machine, operating system, app version, URL and date never do.
pub fn compare_meta(baseline: &BaselineMeta, current: &BaselineMeta) -> Vec<MetaMismatch> {
    let mut found = Vec::new();
    let mut check = |field: &str, then: String, now: String| {
        if then != now {
            found.push(MetaMismatch {
                field: field.to_owned(),
                baseline: then,
                current: now,
            });
        }
    };
    if let (Some(then), Some(now)) = (baseline.monitor_dpi, current.monitor_dpi)
        && (then - now).abs() > DPR_TOLERANCE
    {
        check("monitorDpi", then.to_string(), now.to_string());
    }
    if (baseline.dpr - current.dpr).abs() > DPR_TOLERANCE {
        check("dpr", baseline.dpr.to_string(), current.dpr.to_string());
    }
    check("engine", baseline.engine.clone(), current.engine.clone());
    check(
        "engineVersion",
        baseline.engine_version.clone(),
        current.engine_version.clone(),
    );
    if let (Some(then), Some(now)) = (&baseline.cef_version, &current.cef_version) {
        check("cefVersion", then.clone(), now.clone());
    }
    let size = |viewport: Viewport| format!("{}x{}", viewport.width, viewport.height);
    check("viewport", size(baseline.viewport), size(current.viewport));
    check(
        "fullPage",
        baseline.full_page.to_string(),
        current.full_page.to_string(),
    );
    found
}

/// `url` as a baseline keeps it: scheme, host and path, without credentials, query and
/// fragment.
pub fn public_url(url: &str) -> String {
    let url = url.split(['#', '?']).next().unwrap_or_default();
    match url.split_once("://") {
        Some((scheme, rest)) => {
            let (authority, path) = match rest.find('/') {
                Some(at) => rest.split_at(at),
                None => (rest, ""),
            };
            let host = authority.rsplit('@').next().unwrap_or_default();
            format!("{scheme}://{host}{path}")
        }
        None => url.to_owned(),
    }
}

/// The machine's name from the environment `var` gives (`COMPUTERNAME` on Windows, `HOSTNAME`
/// elsewhere); `unknown` without one.
pub fn machine_name(var: impl Fn(&str) -> Option<String>) -> String {
    ["COMPUTERNAME", "HOSTNAME"]
        .iter()
        .filter_map(|name| var(name))
        .map(|value| value.trim().to_owned())
        .find(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

/// The pixel ratio of an image `image_width` device pixels wide showing `css_width` CSS px,
/// rounded to two decimals; `0` without a width.
pub fn pixel_ratio(image_width: u32, css_width: u32) -> f64 {
    if css_width == 0 {
        return 0.0;
    }
    (f64::from(image_width) / f64::from(css_width) * 100.0).round() / 100.0
}

#[cfg(test)]
#[path = "meta_tests.rs"]
mod tests;
