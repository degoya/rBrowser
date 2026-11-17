//! `site.*`, once per origin and run: `robots.txt` names a sitemap, and each one it names (the
//! first [`MAX_SITEMAPS`]) answers 200.

use rb_plugin_api::PluginHost;

use crate::fetch;
use crate::rules::Sink;
use crate::site::Site;

/// The most sitemaps of one `robots.txt` fetched.
pub const MAX_SITEMAPS: usize = 5;
/// Redirects followed for a sitemap.
const SITEMAP_HOPS: usize = 5;

pub fn check(host: &mut dyn PluginHost, site: &Site, sink: &mut Sink<'_>) {
    sink.page(&site.robots_url, None);
    if site.sitemaps().is_empty() {
        let message = if site.robots_status == Some(200) {
            "site.sitemap_missing"
        } else {
            "site.robots_missing"
        };
        sink.emit(
            "site.sitemap_missing",
            message,
            None,
            &[(
                "status",
                site.robots_status
                    .map_or_else(|| "-".to_owned(), |status| status.to_string()),
            )],
        );
        return;
    }
    if !sink.wants("site.sitemap_unreachable") {
        return;
    }
    for sitemap in site.sitemaps().iter().take(MAX_SITEMAPS) {
        let fetched = fetch::follow(host, sitemap, SITEMAP_HOPS);
        let status = fetched.response.as_ref().map(|response| response.status);
        if status != Some(200) {
            sink.emit(
                "site.sitemap_unreachable",
                "site.sitemap_unreachable",
                None,
                &[
                    ("sitemap", sitemap.to_string()),
                    (
                        "status",
                        status.map_or_else(|| "-".to_owned(), |status| status.to_string()),
                    ),
                ],
            );
        }
    }
}

#[cfg(test)]
#[path = "site_tests.rs"]
mod tests;
