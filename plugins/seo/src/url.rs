//! Just enough of a URL for the SEO check: `http` and `https` only, no user name or password, the
//! fragment dropped. A page URL, a link, a canonical, a `Sitemap:` line and a redirect's
//! `Location` are all read through [`Url::parse`] or [`Url::join`], so nothing else ever reaches a
//! fetch. The one `plugin_util::url`, shared with `plugins/sitemap-capture` (RB-094-11).

pub use plugin_util::url::*;

#[cfg(test)]
#[path = "url_tests.rs"]
mod tests;
