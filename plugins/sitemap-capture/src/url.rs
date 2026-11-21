//! Just enough of a URL for sitemaps: `http` and `https` only, no user name or password, the
//! fragment dropped. A sitemap's `<loc>`, a `Sitemap:` line and a redirect's `Location` are all
//! read through [`Url::parse`] or [`Url::join`], so nothing else ever reaches a fetch or a page.
//! The one `plugin_util::url`, shared with `plugins/seo` (RB-094-11).

pub use plugin_util::url::*;

#[cfg(test)]
#[path = "url_tests.rs"]
mod tests;
