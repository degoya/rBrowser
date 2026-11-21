//! `robots.txt` (RFC 9309): its `Sitemap:` lines, and the `Allow`/`Disallow` rules of the group
//! for rBrowser's token ([`TOKEN`]) or, without one, of `*`. The one `plugin_util::robots`, shared
//! with `plugins/seo` (RB-094-11).

pub use plugin_util::robots::*;

/// The product token rBrowser's capture answers to.
pub const TOKEN: &str = CRAWLER;

#[cfg(test)]
#[path = "robots_tests.rs"]
mod tests;
