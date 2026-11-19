//! `sitemap` (Capture > Sitemap) in effect: the project's section, every value held to its
//! bounds again, turned into what the batch ([`BatchOptions`]) and the plugin's `discover`
//! (`limits`, `selection`) take.

use std::time::Duration;

use rb_config::SitemapConfig;
use rb_engine::chromium::batch::{BatchOptions, MAX_PARALLEL, MAX_RETRIES};
use rb_engine::chromium::hidden::WaitSpec;
use serde_json::{Value, json};

use super::tool::{CaptureArgs, MAX_PAGES};

/// The longest selector taken (as `capture::cdp::MAX_SELECTOR_LEN`).
pub const MAX_SELECTOR: usize = 1024;
/// The most attempts of one shot over all runs: a resume retries a failure until then.
pub const MAX_ATTEMPTS: u32 = MAX_RETRIES + 1;

/// What a run uses, bounded.
#[derive(Clone, Debug, PartialEq)]
pub struct Effective {
    pub batch: BatchOptions,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub max_pages: usize,
    pub respect_robots: bool,
    pub max_depth: u32,
    pub max_bytes: usize,
    pub max_urls: usize,
    /// The longest one screenshot may take (`sitemap.shotTimeoutSec`, RB-090-83).
    pub shot_timeout: Duration,
}

impl Effective {
    /// `config` bounded, with the call's `args` over it.
    pub fn new(config: &SitemapConfig, args: &CaptureArgs) -> Self {
        let selector = config.wait_selector.trim();
        let wait = WaitSpec {
            network_idle: config.wait_network_idle,
            selector: (!selector.is_empty() && selector.len() <= MAX_SELECTOR)
                .then(|| selector.to_owned()),
            delay: Duration::from_millis(u64::from(config.delay_ms.min(30_000))),
            timeout: Duration::from_secs(u64::from(config.timeout_sec.clamp(5, 300))),
            auto_scroll: config.auto_scroll,
        };
        let parallel = usize::try_from(config.parallel)
            .unwrap_or(MAX_PARALLEL)
            .clamp(1, MAX_PARALLEL);
        let max_pages = args
            .max
            .unwrap_or_else(|| usize::try_from(config.max_pages).unwrap_or(MAX_PAGES))
            .min(MAX_PAGES);
        Self {
            batch: BatchOptions {
                parallel,
                host_interval: Duration::from_millis(u64::from(
                    config.host_interval_ms.min(60_000),
                )),
                retries: config.retries.min(MAX_RETRIES),
                wait,
                full_page: config.full_page,
            },
            include: args
                .include
                .clone()
                .unwrap_or_else(|| config.include.clone()),
            exclude: args
                .exclude
                .clone()
                .unwrap_or_else(|| config.exclude.clone()),
            max_pages,
            respect_robots: config.respect_robots,
            max_depth: config.max_depth.min(5),
            max_bytes: usize::try_from(config.max_sitemap_mib.clamp(1, 50)).unwrap_or(50)
                * 1024
                * 1024,
            max_urls: usize::try_from(config.max_urls.clamp(1, 200_000)).unwrap_or(200_000),
            shot_timeout: Duration::from_secs(u64::from(config.shot_timeout_sec.clamp(5, 120))),
        }
    }

    /// The `limits` and `selection` of the plugin's `discover`.
    pub fn discover_args(&self) -> (Value, Value) {
        (
            json!({
                "maxBytes": self.max_bytes,
                "maxDepth": self.max_depth,
                "maxUrls": self.max_urls,
            }),
            json!({
                "include": self.include,
                "exclude": self.exclude,
                "max": self.max_pages,
                "respectRobots": self.respect_robots,
            }),
        )
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
