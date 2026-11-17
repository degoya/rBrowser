//! What a comparison takes, without the window: the environments (the call's, else
//! `envCompare.environments`, else every one; at least two, each an http(s) URL), the paths
//! (a path, or an absolute URL under one of the environments - a page on a foreign host is
//! refused, never rewritten), each path's URL under each environment, the pairs that are
//! diffed, which breakpoints are compared, and `envCompare.*` held to its bounds
//! ([`Effective`]).

pub use rb_config::bookmark_paths::url_in;
use rb_config::bookmark_paths::{as_path, rest_under, split};
use std::time::Duration;

use rb_config::{Environment, ProjectConfig};
use rb_engine::chromium::batch::{BatchOptions, MAX_PARALLEL};
use rb_visual::DiffOptions;

use super::tool::CompareArgs;
use crate::sitemap::settings::Effective as SitemapEffective;
use crate::sitemap::tool::CaptureArgs;

/// The most environments one comparison takes.
pub const MAX_ENVIRONMENTS: usize = 8;
/// The longest pause before a shot, in milliseconds.
pub const MAX_WAIT_MS: u32 = 30_000;

/// Why a comparison cannot be planned. The message starts with its stable code.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    #[error("env_compare.error.too_few_environments: at least two environments are compared")]
    TooFewEnvironments,
    #[error("env_compare.error.unknown_environment: no environment has the id {0}")]
    UnknownEnvironment(String),
    #[error("env_compare.error.environment_url: the environment {0} has no http(s) URL")]
    EnvironmentUrl(String),
    #[error("env_compare.error.bad_path: {0} is no path (/...) and no URL of an environment")]
    BadPath(String),
    #[error("env_compare.error.foreign_host: {0} lies under none of the environments")]
    ForeignHost(String),
    #[error("env_compare.error.breakpoints: {0}")]
    Breakpoints(String),
}

impl PlanError {
    /// The stable code of the error.
    pub fn code(&self) -> &'static str {
        match self {
            Self::TooFewEnvironments => "env_compare.error.too_few_environments",
            Self::UnknownEnvironment(_) => "env_compare.error.unknown_environment",
            Self::EnvironmentUrl(_) => "env_compare.error.environment_url",
            Self::BadPath(_) => "env_compare.error.bad_path",
            Self::ForeignHost(_) => "env_compare.error.foreign_host",
            Self::Breakpoints(_) => "env_compare.error.breakpoints",
        }
    }
}

/// Whether `url` is an absolute http(s) URL.
pub fn is_web_url(url: &str) -> bool {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// The environments compared: `wanted` (the call's) in its order, else `preset`
/// (`envCompare.environments`), else every one of `config`; each named once.
///
/// # Errors
///
/// An unknown id, an environment without an http(s) URL, fewer than two or more than
/// [`MAX_ENVIRONMENTS`].
pub fn environments(
    config: &[Environment],
    wanted: &[String],
    preset: &[String],
) -> Result<Vec<Environment>, PlanError> {
    let ids: Vec<&str> = if !wanted.is_empty() {
        wanted.iter().map(String::as_str).collect()
    } else if !preset.is_empty() {
        preset.iter().map(String::as_str).collect()
    } else {
        config
            .iter()
            .map(|environment| environment.id.as_str())
            .collect()
    };
    let mut chosen: Vec<Environment> = Vec::new();
    for id in ids {
        if chosen.iter().any(|environment| environment.id == id) {
            continue;
        }
        let environment = config
            .iter()
            .find(|environment| environment.id == id)
            .ok_or_else(|| PlanError::UnknownEnvironment(id.to_owned()))?;
        if !is_web_url(&environment.url) {
            return Err(PlanError::EnvironmentUrl(environment.id.clone()));
        }
        chosen.push(environment.clone());
    }
    if chosen.len() < 2 || chosen.len() > MAX_ENVIRONMENTS {
        return Err(PlanError::TooFewEnvironments);
    }
    Ok(chosen)
}

/// The path `input` names: a path as written, or the part of an absolute URL after the
/// environment (of `environments`) it lies under.
///
/// # Errors
///
/// Neither a path nor an http(s) URL, or a URL on a host of no environment.
pub fn path_of(input: &str, environments: &[Environment]) -> Result<String, PlanError> {
    let text = input.trim();
    let plain = !text.is_empty() && !text.chars().any(|c| c.is_whitespace() || c.is_control());
    if !plain {
        return Err(PlanError::BadPath(input.to_owned()));
    }
    if text.starts_with('/') && !text.starts_with("//") {
        return Ok(text.to_owned());
    }
    if !is_web_url(text) {
        return Err(PlanError::BadPath(input.to_owned()));
    }
    environments
        .iter()
        .find_map(|environment| rest_under(text, &environment.url))
        .map(as_path)
        .ok_or_else(|| PlanError::ForeignHost(text.to_owned()))
}

/// The path a view showing `url` stands on: after its environment, else after its origin; `/`
/// for a page outside http(s).
pub fn view_path(url: &str, environments: &[Environment]) -> String {
    path_of(url, environments)
        .unwrap_or_else(|_| split(url).map_or_else(|| "/".to_owned(), |(_, rest)| as_path(rest)))
}

/// The pairs diffed among `count` environments: every two, the earlier first.
pub fn pairs(count: usize) -> Vec<(usize, usize)> {
    (0..count)
        .flat_map(|a| (a + 1..count).map(move |b| (a, b)))
        .collect()
}

/// The breakpoint ids compared: the call's, else those of the target views, else
/// `envCompare.breakpoints`; empty means every Chromium breakpoint.
pub fn breakpoint_ids(call: &[String], views: &[String], preset: &[String]) -> Vec<String> {
    let chosen = [call, views, preset]
        .into_iter()
        .find(|list| !list.is_empty())
        .unwrap_or_default();
    let mut ids: Vec<String> = Vec::new();
    for id in chosen {
        if !ids.contains(id) {
            ids.push(id.clone());
        }
    }
    ids
}

/// The breakpoints of a comparison for the sitemap capture's planning (the same render specs).
pub fn capture_args(breakpoints: Vec<String>) -> CaptureArgs {
    CaptureArgs {
        breakpoints,
        ..CaptureArgs::default()
    }
}

/// What a run uses, bounded.
#[derive(Clone, Debug, PartialEq)]
pub struct Effective {
    /// The shots: Capture > Sitemap's wait conditions, the pause, full
    /// page and parallelism of `envCompare`, no pause per host.
    pub batch: BatchOptions,
    /// The pixel diff.
    pub options: DiffOptions,
    /// The share of differing pixels (0 to 1) up to which two shots count as the same.
    pub max_ratio: f64,
}

/// `config`'s `envCompare` (over `sitemap` and `visual`) with the call's `args` over it.
pub fn effective(config: &ProjectConfig, args: &CompareArgs) -> Effective {
    let section = &config.env_compare;
    let mut batch = SitemapEffective::new(&config.sitemap, &CaptureArgs::default()).batch;
    batch.wait.delay = Duration::from_millis(u64::from(section.wait_ms.min(MAX_WAIT_MS)));
    batch.full_page = args.full_page.unwrap_or(section.full_page);
    batch.parallel = usize::try_from(section.parallel)
        .unwrap_or(MAX_PARALLEL)
        .clamp(1, MAX_PARALLEL);
    batch.host_interval = Duration::ZERO;
    let threshold = args
        .threshold
        .or(section.threshold)
        .filter(|threshold| (0.0..=1.0).contains(threshold))
        .unwrap_or(config.visual.threshold);
    let percent = if section.max_diff_percent.is_finite() {
        section.max_diff_percent.clamp(0.0, 100.0)
    } else {
        0.0
    };
    Effective {
        batch,
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
