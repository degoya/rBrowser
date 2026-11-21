//! `robots.txt` (RFC 9309) for one product token or, without one, `*`: the one
//! `plugin_util::robots`, shared with `plugins/sitemap-capture` (RB-094-11). The crawl answers to
//! rBrowser's token ([`CRAWLER`]), indexability to a search engine's ([`SEARCH_ENGINE`]).

pub use plugin_util::robots::*;

/// The token indexability is judged for: Google's crawler, the `*` group without one.
pub const SEARCH_ENGINE: &str = "googlebot";

#[cfg(test)]
#[path = "robots_tests.rs"]
mod tests;
