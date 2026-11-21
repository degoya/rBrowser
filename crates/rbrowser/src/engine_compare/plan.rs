//! What an engine comparison runs, without the window: the engines asked for (all three by
//! default, always in the order Chromium, Firefox, WebKit), which of them can run - one that
//! cannot (not installed, switched off, no sidecar) is reported with its reason, never an
//! error -, the breakpoint and its shot spec, the page and the diff's bounds.
//!
//! The breakpoint's size, DPR, touch and media apply in every engine; its user agent and
//! platform overrides do not, so each engine sends its own and the site serves it as itself.
//! Every engine shoots the viewport, so the shots have the same size and the pixel diff
//! compares them; its anti-aliasing test keeps font smoothing out of the differences.

use std::time::Duration;

use rb_config::{Breakpoint, DevicePreset, EmulationFields, EngineCompareConfig, EngineKind};
use rb_engine::EngineHealth;
use rb_engine::chromium::batch::BatchOptions;
use rb_engine::chromium::hidden::{RenderSpec, WaitSpec};
use rb_visual::DiffOptions;
use rb_visual::sitemap::{RunBreakpoint, file_name};
use serde::{Deserialize, Serialize};

use crate::sitemap::backend::DEFAULT_SESSION;

/// The engines compared, in their fixed order.
pub const ENGINES: [EngineKind; 3] = [
    EngineKind::Chromium,
    EngineKind::Firefox,
    EngineKind::Webkit,
];
/// The longest settle pause before a shot.
pub const MAX_WAIT_MS: u32 = 30_000;

/// Why a comparison cannot be planned. The message starts with its stable code.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    #[error("engine_compare.error.unknown_engine: {0} is no engine (chromium, firefox, webkit)")]
    UnknownEngine(String),
    #[error("engine_compare.error.no_breakpoint: no breakpoint is named and no view shows one")]
    NoBreakpoint,
    #[error("engine_compare.error.unknown_breakpoint: no breakpoint has the id {0}")]
    UnknownBreakpoint(String),
    #[error("engine_compare.error.bad_url: {0} is no http, https or file URL")]
    BadUrl(String),
    #[error("engine_compare.error.emulation: {0}")]
    Emulation(String),
    #[error("engine_compare.error.url_refused: {0}")]
    UrlRefused(String),
}

impl PlanError {
    /// The stable code of the error.
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownEngine(_) => "engine_compare.error.unknown_engine",
            Self::NoBreakpoint => "engine_compare.error.no_breakpoint",
            Self::UnknownBreakpoint(_) => "engine_compare.error.unknown_breakpoint",
            Self::BadUrl(_) => "engine_compare.error.bad_url",
            Self::Emulation(_) => "engine_compare.error.emulation",
            Self::UrlRefused(_) => "engine_compare.error.url_refused",
        }
    }
}

/// An engine left out of a run, and why (its own words: "Firefox is not installed").
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedEngine {
    pub engine: String,
    pub reason: String,
}

/// The engines `names` asks for (any case, each once, in [`ENGINES`] order); all three when
/// empty.
///
/// # Errors
///
/// A name that is no engine.
pub fn engines(names: &[String]) -> Result<Vec<EngineKind>, PlanError> {
    if names.is_empty() {
        return Ok(ENGINES.to_vec());
    }
    let mut wanted = Vec::new();
    for name in names {
        let kind = ENGINES
            .into_iter()
            .find(|kind| kind.name().eq_ignore_ascii_case(name.trim()))
            .ok_or_else(|| PlanError::UnknownEngine(name.clone()))?;
        wanted.push(kind);
    }
    Ok(ENGINES
        .into_iter()
        .filter(|kind| wanted.contains(kind))
        .collect())
}

/// `engines` split by `health`: those that run (or start on their first view), and those that
/// cannot, with the engine's reason.
pub fn partition(
    engines: &[EngineKind],
    health: impl Fn(EngineKind) -> EngineHealth,
) -> (Vec<EngineKind>, Vec<SkippedEngine>) {
    let mut run = Vec::new();
    let mut skipped = Vec::new();
    for &kind in engines {
        match health(kind) {
            EngineHealth::Running | EngineHealth::Stopped => run.push(kind),
            EngineHealth::Unavailable(reason) => skipped.push(SkippedEngine {
                engine: kind.name().to_owned(),
                reason,
            }),
        }
    }
    (run, skipped)
}

/// The breakpoint compared: the call's, else the target view's.
///
/// # Errors
///
/// Neither names one.
pub fn breakpoint_id(call: Option<&str>, view: Option<&str>) -> Result<String, PlanError> {
    call.or(view)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .ok_or(PlanError::NoBreakpoint)
}

/// The page compared: the call's URL, else the target view's; `http`, `https` or `file` only.
///
/// # Errors
///
/// No page, or one of another scheme.
pub fn page_url(call: Option<&str>, view: Option<&str>) -> Result<String, PlanError> {
    let url = call.or(view).unwrap_or_default().trim();
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase());
    match scheme.as_deref() {
        Some("http" | "https" | "file") if url.len() > "file:".len() => Ok(url.to_owned()),
        _ => Err(PlanError::BadUrl(url.to_owned())),
    }
}

/// `url` when an agent (MCP) may compare it: what MCP's `navigate` may load
/// (`crate::mcp::url_policy`) - http, https, or a file inside the project, no link -, so the
/// comparison's PNGs never show a local file outside the project.
///
/// # Errors
///
/// The policy refuses the URL.
pub fn agent_url(url: &str, project_root: Option<&std::path::Path>) -> Result<String, PlanError> {
    crate::mcp::url_policy::check(url, project_root).map_err(PlanError::UrlRefused)
}

/// The breakpoint as a run's record and its shot spec: its emulation resolved over the project
/// `defaults` and `devices`, without the user agent and platform overrides (each engine is
/// itself) and, in a workspace not trusted, without extra headers.
///
/// # Errors
///
/// The emulation does not resolve (an unknown device, a value out of range).
pub fn shot_spec(
    breakpoint: &Breakpoint,
    defaults: &EmulationFields,
    devices: &[DevicePreset],
    trusted: bool,
) -> Result<(RunBreakpoint, RenderSpec), PlanError> {
    let mut emulation = rb_engine::resolve_emulation(breakpoint, defaults, devices)
        .map_err(|error| PlanError::Emulation(error.to_string()))?;
    emulation.user_agent = None;
    emulation.platform = None;
    if !trusted {
        emulation.headers.clear();
    }
    let run = RunBreakpoint {
        name: file_name(&breakpoint.id),
        label: breakpoint
            .label
            .clone()
            .unwrap_or_else(|| breakpoint.id.clone()),
        width: emulation.width,
        height: emulation.height,
        dpr: emulation.dpr,
    };
    let spec = RenderSpec {
        breakpoint: breakpoint.id.clone(),
        emulation,
        session: breakpoint
            .session
            .clone()
            .unwrap_or_else(|| DEFAULT_SESSION.to_owned()),
    };
    Ok((run, spec))
}

/// A call's word over `engineCompare`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Overrides {
    /// Over `engineCompare.threshold`, 0 to 1.
    pub threshold: Option<f64>,
    /// Over `engineCompare.maxDiffPercent`, 0 to 100.
    pub max_diff_percent: Option<f64>,
}

/// What a run uses, bounded.
#[derive(Clone, Debug, PartialEq)]
pub struct Effective {
    /// One shot per engine: the viewport, the settle pause, no pause per host.
    pub batch: BatchOptions,
    /// The pixel diff; anti-aliased pixels never count.
    pub options: DiffOptions,
    /// The share of differing pixels (0 to 1) up to which two engines count as the same.
    pub max_ratio: f64,
}

/// `section` (over `visual_threshold`) with the call's `overrides` over it.
pub fn effective(
    section: &EngineCompareConfig,
    visual_threshold: f64,
    overrides: &Overrides,
) -> Effective {
    let in_range = |value: &f64| (0.0..=1.0).contains(value);
    let threshold = overrides
        .threshold
        .filter(in_range)
        .or(section.threshold.filter(in_range))
        .unwrap_or(visual_threshold);
    let percent = overrides
        .max_diff_percent
        .unwrap_or(section.max_diff_percent);
    let percent = if percent.is_finite() {
        percent.clamp(0.0, 100.0)
    } else {
        0.0
    };
    let wait = WaitSpec {
        delay: Duration::from_millis(u64::from(section.wait_ms.min(MAX_WAIT_MS))),
        ..WaitSpec::default()
    };
    Effective {
        batch: BatchOptions {
            parallel: 1,
            host_interval: Duration::ZERO,
            retries: 1,
            wait,
            full_page: false,
        },
        options: DiffOptions {
            threshold,
            include_aa: false,
        },
        max_ratio: percent / 100.0,
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
