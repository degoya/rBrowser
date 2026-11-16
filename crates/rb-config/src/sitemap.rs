//! `sitemap`: Capture > Sitemap (RB-050-05) - how a sitemap capture finds, picks and captures
//! the pages of a site. Read by `crates/rbrowser/src/sitemap/`, which holds every value to its
//! bounds again (a hand-edited config may carry anything). Re-exported from the crate root.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `sitemap`: Capture > Sitemap (RB-050-05).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct SitemapConfig {
    /// Pages captured at once, each in its own hidden view (1 to 8).
    #[schemars(range(min = 1, max = 8))]
    pub parallel: u32,
    /// Wait until the page has made no request for a moment after its load.
    pub wait_network_idle: bool,
    /// A CSS selector the page must show before its screenshot; empty for none.
    #[schemars(length(max = 1024))]
    pub wait_selector: String,
    /// A further pause before each screenshot, in milliseconds (0 to 30000).
    #[schemars(range(max = 30000))]
    pub delay_ms: u32,
    /// The longest one page may take, from navigation to screenshot, in seconds (5 to 300).
    #[schemars(range(min = 5, max = 300))]
    pub timeout_sec: u32,
    /// Capture the whole page, not only the viewport.
    pub full_page: bool,
    /// Scroll each page to its end and back first, so lazy-loaded content is there.
    pub auto_scroll: bool,
    /// Capture only the pages matching one of these patterns: a glob over the path (`/blog/**`)
    /// or a regular expression after `re:`; empty for every page.
    pub include: Vec<String>,
    /// Leave out the pages matching one of these patterns (glob or `re:`).
    pub exclude: Vec<String>,
    /// At most this many pages per run; 0 for all.
    #[schemars(range(max = 100000))]
    pub max_pages: u32,
    /// The least time between two page loads on one host, in milliseconds (0 to 60000).
    #[schemars(range(max = 60000))]
    pub host_interval_ms: u32,
    /// Leave out the pages the site's robots.txt disallows.
    pub respect_robots: bool,
    /// Deprecated, kept so older configs work: more elements to hide before every capture, as
    /// CSS selectors - they join `capture.hideSelectors` (RB-080-12), which also holds the
    /// built-in list of consent managers.
    pub hide_selectors: Vec<String>,
    /// How often a failed page is tried again (0 to 5).
    #[schemars(range(max = 5))]
    pub retries: u32,
    /// How deep sitemap indexes may nest (0 to 5).
    #[schemars(range(max = 5))]
    pub max_depth: u32,
    /// The largest sitemap read, unpacked, in MiB (1 to 50).
    #[schemars(range(min = 1, max = 50))]
    pub max_sitemap_mib: u32,
    /// The most pages read from the sitemaps (1 to 200000).
    #[schemars(range(min = 1, max = 200000))]
    pub max_urls: u32,
}

impl Default for SitemapConfig {
    fn default() -> Self {
        Self {
            parallel: 4,
            wait_network_idle: true,
            wait_selector: String::new(),
            delay_ms: 0,
            timeout_sec: 30,
            full_page: true,
            auto_scroll: true,
            include: Vec::new(),
            exclude: Vec::new(),
            max_pages: 0,
            host_interval_ms: 250,
            respect_robots: true,
            hide_selectors: Vec::new(),
            retries: 1,
            max_depth: 3,
            max_sitemap_mib: 10,
            max_urls: 50_000,
        }
    }
}

#[cfg(test)]
#[path = "sitemap_tests.rs"]
mod tests;
