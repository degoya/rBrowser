//! Finding and reading a site's sitemaps over the host's `http` capability: `robots.txt` and its
//! `Sitemap:` lines first, else `/sitemap.xml`, then `/sitemap_index.xml`; or a given sitemap
//! URL, file or URL list. Indexes are followed breadth first and gzip is unpacked, every step
//! under [`Limits`]: bytes per file (packed and unpacked), the depth of indexes, the number of
//! sitemaps and of pages, the redirects. The host answers a redirect and never follows it; this
//! module follows one only within its origin (scheme, host and port stay). An index names
//! sitemaps of its own origin only (sitemaps.org); another origin is skipped, so a site's index
//! cannot send the fetches to other hosts - `robots.txt` may still list sitemaps elsewhere, as
//! the protocol allows, but never on this machine or the local network ([`Url::is_local`])
//! unless the site itself is local. The pages kept are the site's own: a `<loc>` of another
//! origin is dropped and counted ([`Discovery::foreign`]), so a sitemap cannot send the capture
//! to another host. A file or a URL list has no site; its pages are the person's own choice.

use std::collections::{BTreeSet, VecDeque};

use rb_plugin_api::{HttpRequest, PluginHost};
use serde::{Deserialize, Serialize};

use crate::gzip::{GzipError, gunzip, is_gzip};
use crate::robots::{Robots, TOKEN};
use crate::sitemap::{Entry, ParseError, Parsed, parse_any};
use crate::url::Url;

/// The default and the ceiling of each limit: `(default, ceiling)`.
pub const MAX_BYTES: (usize, usize) = (10 * 1024 * 1024, 50 * 1024 * 1024);
/// Indexes below the first sitemap.
pub const MAX_DEPTH: (u32, u32) = (3, 5);
/// Pages kept.
pub const MAX_URLS: (usize, usize) = (50_000, 200_000);
/// Sitemaps read.
pub const MAX_SITEMAPS: (usize, usize) = (200, 1000);
/// Redirects followed per fetch.
pub const MAX_REDIRECTS: (u32, u32) = (3, 5);
/// The most bytes of `robots.txt` read.
pub const MAX_ROBOTS_BYTES: usize = 512 * 1024;

/// The caps of one discovery; a value above its ceiling is taken as the ceiling.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Limits {
    pub max_bytes: usize,
    pub max_depth: u32,
    pub max_urls: usize,
    pub max_sitemaps: usize,
    pub max_redirects: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: MAX_BYTES.0,
            max_depth: MAX_DEPTH.0,
            max_urls: MAX_URLS.0,
            max_sitemaps: MAX_SITEMAPS.0,
            max_redirects: MAX_REDIRECTS.0,
        }
    }
}

impl Limits {
    /// Each limit at most its ceiling, and at least 1 (redirects and depth may be 0).
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            max_bytes: self.max_bytes.clamp(1, MAX_BYTES.1),
            max_depth: self.max_depth.min(MAX_DEPTH.1),
            max_urls: self.max_urls.clamp(1, MAX_URLS.1),
            max_sitemaps: self.max_sitemaps.clamp(1, MAX_SITEMAPS.1),
            max_redirects: self.max_redirects.min(MAX_REDIRECTS.1),
        }
    }
}

/// Where the pages come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// The site's own sitemaps, found from its origin.
    Auto(Url),
    /// One sitemap (or index, or URL list) at a URL.
    Sitemap(Url),
    /// The bytes of a file the core read (gzip, XML or a URL list).
    File(Vec<u8>),
    /// URLs as given (the project bookmarks).
    List(Vec<String>),
}

/// One sitemap read, or tried.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SitemapReport {
    pub url: String,
    pub pages: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
}

/// What a discovery found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Discovery {
    /// The site's origin, when the source had one (auto, a sitemap URL).
    pub site: Option<Url>,
    /// The site's `robots.txt`, when one was read.
    pub robots: Option<Robots>,
    /// The pages, each once, in the order found.
    pub entries: Vec<Entry>,
    /// Every sitemap read or tried.
    pub sitemaps: Vec<SitemapReport>,
    /// `<loc>` and list lines that were no `http(s)` URL, and sitemaps of an index on another
    /// origin.
    pub skipped: usize,
    /// Pages of another origin than the site's, left out.
    pub foreign: usize,
    /// A limit cut the discovery short.
    pub truncated: bool,
}

/// Why a fetch gave nothing; `code` is the last segment of an i18n key below
/// `plugin.sitemap-capture.errors.`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchError {
    pub code: String,
    pub status: Option<u16>,
}

fn error(code: &str, status: Option<u16>) -> FetchError {
    FetchError {
        code: code.to_owned(),
        status,
    }
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// `bytes` unpacked when they are gzip, at most `limit` bytes.
///
/// # Errors
///
/// `gzip` for a broken frame, `too_large` past the limit.
pub fn unpack(bytes: Vec<u8>, limit: usize) -> Result<Vec<u8>, FetchError> {
    if bytes.len() > limit {
        return Err(error("too_large", None));
    }
    if !is_gzip(&bytes) {
        return Ok(bytes);
    }
    gunzip(&bytes, limit).map_err(|failure| match failure {
        GzipError::TooLarge => error("too_large", None),
        GzipError::Malformed => error("gzip", None),
    })
}

/// GETs `url`, following at most `redirects` redirects within its origin; the final URL and
/// the body, unpacked, at most `limit` bytes.
///
/// # Errors
///
/// [`FetchError`]: `http_status` (with the status), `redirect_scheme`, `redirect_origin`,
/// `redirect_loop`, `redirect_invalid`, `too_large`, `gzip`, or `fetch` for a failure of the
/// host.
pub fn fetch(
    host: &mut dyn PluginHost,
    url: &Url,
    redirects: u32,
    limit: usize,
) -> Result<(Url, Vec<u8>), FetchError> {
    let mut current = url.clone();
    let mut hops = 0;
    loop {
        let request = HttpRequest {
            method: "GET".to_owned(),
            url: current.to_string(),
            headers: vec![(
                "Accept".to_owned(),
                "application/xml, text/xml, text/plain, application/gzip, */*;q=0.5".to_owned(),
            )],
            body: Vec::new(),
        };
        let response = host.fetch(&request).map_err(|failure| FetchError {
            code: "fetch".to_owned(),
            status: failure.param("status").and_then(|text| text.parse().ok()),
        })?;
        match response.status {
            200..=299 => return Ok((current, unpack(response.body, limit)?)),
            301 | 302 | 303 | 307 | 308 => {
                hops += 1;
                if hops > redirects {
                    return Err(error("redirect_loop", Some(response.status)));
                }
                let next = header(&response.headers, "location")
                    .and_then(|location| current.join(location))
                    .ok_or_else(|| error("redirect_invalid", Some(response.status)))?;
                if next.scheme() != current.scheme() {
                    return Err(error("redirect_scheme", Some(response.status)));
                }
                if next.origin() != current.origin() {
                    return Err(error("redirect_origin", Some(response.status)));
                }
                current = next;
            }
            status => return Err(error("http_status", Some(status))),
        }
    }
}

fn report(url: &Url, pages: usize, failure: Option<FetchError>) -> SitemapReport {
    SitemapReport {
        url: url.to_string(),
        pages,
        status: failure.as_ref().and_then(|failure| failure.status),
        error: failure.map(|failure| failure.code),
    }
}

fn parse_failure(failure: &ParseError) -> FetchError {
    match failure {
        ParseError::Encoding => error("encoding", None),
        ParseError::Xml(_) => error("xml", None),
        ParseError::NotASitemap(_) => error("not_a_sitemap", None),
    }
}

/// The walk over sitemaps and indexes.
struct Walk<'a> {
    host: &'a mut dyn PluginHost,
    limits: Limits,
    queue: VecDeque<(Url, u32)>,
    seen: BTreeSet<String>,
    pages: BTreeSet<String>,
    found: Discovery,
}

impl Walk<'_> {
    /// Takes what `parsed` holds: pages up to the limit, sitemaps of an index into the queue.
    /// The number of pages taken.
    fn take(
        &mut self,
        parsed: Parsed,
        depth: u32,
        base: Option<&Url>,
    ) -> Result<usize, FetchError> {
        match parsed {
            Parsed::Pages { entries, skipped } => {
                self.found.skipped += skipped;
                let site = self.found.site.as_ref().map(Url::origin);
                let mut taken = 0;
                for entry in entries {
                    if site
                        .as_ref()
                        .is_some_and(|site| *site != entry.loc.origin())
                    {
                        self.found.foreign += 1;
                        continue;
                    }
                    if self.found.entries.len() >= self.limits.max_urls {
                        self.found.truncated = true;
                        break;
                    }
                    if self.pages.insert(entry.loc.to_string()) {
                        self.found.entries.push(entry);
                        taken += 1;
                    }
                }
                Ok(taken)
            }
            Parsed::Index { sitemaps, skipped } => {
                self.found.skipped += skipped;
                if depth >= self.limits.max_depth {
                    self.found.truncated = true;
                    return Err(error("too_deep", None));
                }
                for url in sitemaps {
                    if base.is_some_and(|base| base.origin() != url.origin()) {
                        self.found.skipped += 1;
                    } else {
                        self.queue.push_back((url, depth + 1));
                    }
                }
                Ok(0)
            }
        }
    }

    /// Whether `url` may be fetched: anywhere but this machine and the local network, and
    /// there too when the site is local itself.
    fn may_fetch(&self, url: &Url) -> bool {
        !url.is_local() || self.found.site.as_ref().is_some_and(Url::is_local)
    }

    /// Reads one sitemap; `Ok(false)` when it could not be read (and is reported).
    fn read(&mut self, url: &Url, depth: u32) -> bool {
        if !self.seen.insert(url.to_string()) {
            return true;
        }
        if self.found.sitemaps.len() >= self.limits.max_sitemaps {
            self.found.truncated = true;
            return false;
        }
        if !self.may_fetch(url) {
            self.found
                .sitemaps
                .push(report(url, 0, Some(error("private_address", None))));
            return false;
        }
        let outcome = fetch(
            self.host,
            url,
            self.limits.max_redirects,
            self.limits.max_bytes,
        )
        .and_then(|(final_url, bytes)| {
            parse_any(&bytes, Some(&final_url))
                .map(|parsed| (final_url, parsed))
                .map_err(|failure| parse_failure(&failure))
        })
        .and_then(|(final_url, parsed)| self.take(parsed, depth, Some(&final_url)));
        let ok = outcome.is_ok();
        let (pages, failure) = match outcome {
            Ok(pages) => (pages, None),
            Err(failure) => (0, Some(failure)),
        };
        self.found.sitemaps.push(report(url, pages, failure));
        ok
    }

    fn drain(&mut self) {
        while let Some((url, depth)) = self.queue.pop_front() {
            if self.found.sitemaps.len() >= self.limits.max_sitemaps {
                self.found.truncated = true;
                return;
            }
            self.read(&url, depth);
        }
    }

    fn robots(&mut self, site: &Url) {
        let Some(url) = site.join("/robots.txt") else {
            return;
        };
        let limit = MAX_ROBOTS_BYTES.min(self.limits.max_bytes);
        if let Ok((_, bytes)) = fetch(self.host, &url, self.limits.max_redirects, limit) {
            let text = String::from_utf8_lossy(&bytes);
            self.found.robots = Some(Robots::parse(&text, &url, TOKEN));
        }
    }
}

/// Finds and reads the pages of `source`; `robots` reads the site's `robots.txt` for a sitemap
/// URL as well (auto discovery always reads it).
pub fn discover(
    host: &mut dyn PluginHost,
    source: &Source,
    limits: Limits,
    robots: bool,
) -> Discovery {
    let mut walk = Walk {
        host,
        limits: limits.clamped(),
        queue: VecDeque::new(),
        seen: BTreeSet::new(),
        pages: BTreeSet::new(),
        found: Discovery::default(),
    };
    match source {
        Source::Auto(site) => {
            walk.found.site = Some(site.clone());
            walk.robots(site);
            let listed = walk
                .found
                .robots
                .as_ref()
                .map(|robots| robots.sitemaps.clone())
                .unwrap_or_default();
            if listed.is_empty() {
                for path in ["/sitemap.xml", "/sitemap_index.xml"] {
                    if let Some(url) = site.join(path)
                        && walk.read(&url, 0)
                    {
                        break;
                    }
                }
            } else {
                walk.queue.extend(listed.into_iter().map(|url| (url, 0)));
            }
        }
        Source::Sitemap(url) => {
            walk.found.site = Some(url.clone());
            if robots {
                walk.robots(url);
            }
            walk.queue.push_back((url.clone(), 0));
        }
        Source::File(bytes) => {
            let limit = walk.limits.max_bytes;
            let parsed = unpack(bytes.clone(), limit).and_then(|bytes| {
                parse_any(&bytes, None).map_err(|failure| parse_failure(&failure))
            });
            let outcome = parsed.and_then(|parsed| walk.take(parsed, 0, None));
            if let Err(failure) = outcome {
                walk.found.sitemaps.push(SitemapReport {
                    url: String::new(),
                    pages: 0,
                    status: None,
                    error: Some(failure.code),
                });
            }
        }
        Source::List(urls) => {
            let parsed = crate::sitemap::parse_list(urls.join("\n").as_bytes(), None)
                .map_err(|failure| parse_failure(&failure));
            if let Ok(parsed) = parsed {
                let _ = walk.take(parsed, 0, None);
            }
        }
    }
    walk.drain();
    walk.found
}

#[cfg(test)]
#[path = "discover_tests.rs"]
mod tests;
