//! What the check knows of a page's origin: its `robots.txt`, read once per origin and run - the
//! rules for a search engine (indexability), for rBrowser's crawl, and its `Sitemap:` lines.

use rb_plugin_api::PluginHost;

use crate::fetch;
use crate::robots::{CRAWLER, Robots, SEARCH_ENGINE};
use crate::url::Url;

/// The most redirects followed for `robots.txt` (Google follows five).
const ROBOTS_HOPS: usize = 5;

/// One origin's `robots.txt`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Site {
    pub origin: String,
    pub robots_url: String,
    /// The final status of `robots.txt`; `None` when it could not be fetched.
    pub robots_status: Option<u16>,
    /// The rules for a search engine; everything allowed without a `robots.txt`.
    pub engine: Robots,
    /// The rules rBrowser's crawl keeps.
    pub crawler: Robots,
}

impl Site {
    /// `robots.txt` of `page`'s origin, fetched; a missing or failing one allows everything.
    pub fn load(host: &mut dyn PluginHost, page: &Url) -> Self {
        let origin = page.origin();
        let Some(robots_url) = Url::parse(&format!("{origin}/robots.txt")) else {
            return Self::default();
        };
        let fetched = fetch::follow(host, &robots_url, ROBOTS_HOPS);
        let status = fetched.response.as_ref().map(|response| response.status);
        let text = fetched
            .response
            .filter(|response| response.status == 200)
            .map(|response| response.text())
            .unwrap_or_default();
        Self {
            robots_url: robots_url.to_string(),
            robots_status: status,
            engine: Robots::parse(&text, &robots_url, SEARCH_ENGINE),
            crawler: Robots::parse(&text, &robots_url, CRAWLER),
            origin,
        }
    }

    /// The `Sitemap:` lines.
    #[must_use]
    pub fn sitemaps(&self) -> &[Url] {
        &self.engine.sitemaps
    }
}

#[cfg(test)]
#[path = "site_tests.rs"]
mod tests;
