//! The Chrome Web Store side of an install (RB-080-23): what a person writes (an id or a store
//! URL), the update server's address, and the download - https only, to `clients2.google.com`
//! and the blob host it redirects to ([`ALLOWED_HOSTS`]), every hop checked before it is asked
//! (a relative `Location` resolved against the hop before it, RFC 7231), at most [`MAX_HOPS`]
//! redirects, the body cut off past the caller's limit. The bytes come back unchecked:
//! [`super::crx3::verify`] is next, and nothing is unpacked before it passes. The version check
//! without a download is [`super::update_check`], over the same [`fetch`]. Blocking: the callers
//! run it on a blocking thread.

use std::io::Read;
use std::time::Duration;

use ureq::config::ConfigBuilder;
use ureq::typestate::AgentScope;

/// The update server.
pub const UPDATE_HOST: &str = "clients2.google.com";
/// Where a download may go: the update server and the blob host it redirects to.
pub const ALLOWED_HOSTS: &[&str] = &["clients2.google.com", "clients2.googleusercontent.com"];
/// The largest CRX downloaded; the DevTools extensions are a few MB.
pub const MAX_CRX_BYTES: u64 = 64 * 1024 * 1024;
/// The most redirects followed (the update server answers with one).
pub const MAX_HOPS: u32 = 5;
/// The Chromium version sent when the CEF build is unknown: the CEF the workspace pins.
pub const FALLBACK_CHROMIUM: &str = "156.0.0.0";
/// How long connecting may take.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the body may go without a byte.
pub const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// How long the whole body may take.
pub const BODY_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// The length of an extension id.
const ID_LEN: usize = 32;
/// The store's page hosts and the path before the id.
const STORE_PAGES: &[(&str, &str)] = &[
    ("https://chromewebstore.google.com/detail/", ""),
    ("https://chrome.google.com/webstore/detail/", ""),
];
const CHUNK: usize = 64 * 1024;

/// Why a download failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DownloadError {
    /// Not an extension id.
    #[error("not an extension id: 32 letters a to p")]
    BadId,
    /// A hop to a host that is not Google's.
    #[error("the download leads to {0}, which is not a Chrome Web Store host")]
    HostNotAllowed(String),
    /// More redirects than [`MAX_HOPS`].
    #[error("the download redirects more than {MAX_HOPS} times")]
    TooManyRedirects,
    /// A body past the limit.
    #[error("the extension is larger than the limit")]
    TooLarge,
    /// The network or the server.
    #[error("the download failed: {0}")]
    Network(String),
}

impl DownloadError {
    /// The stable code of the error (MCP answers, notices).
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadId => "extensions.download_bad_id",
            Self::HostNotAllowed(_) => "extensions.download_host_not_allowed",
            Self::TooManyRedirects => "extensions.download_too_many_redirects",
            Self::TooLarge => "extensions.download_too_large",
            Self::Network(_) => "extensions.download_failed",
        }
    }
}

/// One answer of a hop.
pub enum Hop {
    /// A redirect to `Location`.
    Redirect(String),
    /// The body, with `Content-Length` when the server named one.
    Body {
        /// `Content-Length`.
        length: Option<u64>,
        /// The bytes.
        body: Box<dyn Read + Send>,
    },
}

/// Fetches one address without following redirects.
pub trait HopFetcher: Send + Sync {
    /// Asks `url` once.
    ///
    /// # Errors
    ///
    /// Unreachable, or neither a redirect nor `2xx`.
    fn get(&self, url: &str) -> Result<Hop, String>;
}

/// Whether `id` is an extension id: 32 letters `a` to `p`.
pub fn is_id(id: &str) -> bool {
    id.len() == ID_LEN && id.bytes().all(|b| (b'a'..=b'p').contains(&b))
}

/// The id a person wrote: the id itself or a store page URL
/// (`https://chromewebstore.google.com/detail/<slug>/<id>`, with or without the slug, the old
/// `chrome.google.com/webstore/detail/` too).
pub fn parse_ref(written: &str) -> Option<String> {
    let written = written.trim();
    if is_id(written) {
        return Some(written.to_owned());
    }
    let path = STORE_PAGES
        .iter()
        .find_map(|(prefix, _)| written.strip_prefix(prefix))?;
    let path = path.split(['?', '#']).next().unwrap_or_default();
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .take(2)
        .find(|segment| is_id(segment))
        .map(str::to_owned)
}

/// The update server's address for `id`, answering with a redirect to the CRX3.
pub fn update_url(id: &str, chromium_version: &str) -> String {
    format!(
        "https://{UPDATE_HOST}/service/update2/crx?response=redirect&prodversion={chromium_version}&acceptformat=crx3&x=id%3D{id}%26uc"
    )
}

/// The Chromium version in a CEF version (`156.4.0+g...+chromium-156.0.7000.1`), else
/// [`FALLBACK_CHROMIUM`].
pub fn chromium_version(cef_version: Option<&str>) -> String {
    cef_version
        .and_then(|version| version.rsplit_once("+chromium-"))
        .map(|(_, chromium)| chromium.trim())
        .filter(|chromium| {
            !chromium.is_empty() && chromium.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        })
        .map_or_else(|| FALLBACK_CHROMIUM.to_owned(), str::to_owned)
}

/// The host of `url` when it is https (any letter case), plain (no user info, no port) and one
/// of [`ALLOWED_HOSTS`]; a bare host without a path counts.
fn allowed_host(url: &str) -> Option<String> {
    let rest = url
        .get(..8)
        .filter(|scheme| scheme.eq_ignore_ascii_case("https://"))?;
    let rest = url.get(rest.len()..)?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.to_ascii_lowercase();
    ALLOWED_HOSTS.contains(&host.as_str()).then_some(host)
}

/// Whether a download may go to `url`.
pub fn host_allowed(url: &str) -> bool {
    allowed_host(url).is_some()
}

/// `location` as an absolute URL: as written with a scheme, else resolved against `previous`
/// (`//host/path` with its scheme, `/path` against its origin, `path` against its folder).
pub fn resolve(previous: &str, location: &str) -> String {
    if has_scheme(location) {
        return location.to_owned();
    }
    let (scheme, rest) = previous.split_once("://").unwrap_or(("https", previous));
    if let Some(network_path) = location.strip_prefix("//") {
        return format!("{scheme}://{network_path}");
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if location.starts_with('/') {
        return format!("{scheme}://{authority}{location}");
    }
    let path = rest
        .get(authority.len()..)
        .unwrap_or_default()
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    let folder = path.rfind('/').map_or("/", |end| &path[..=end]);
    format!("{scheme}://{authority}{folder}{location}")
}

/// Whether `reference` starts with a URL scheme (`https:`, `javascript:`).
fn has_scheme(reference: &str) -> bool {
    let Some(end) = reference.find(':') else {
        return false;
    };
    let scheme = &reference[..end];
    scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.')
}

/// Downloads the CRX of `id`, at most `limit` bytes.
///
/// # Errors
///
/// A bad id, a hop off Google's hosts, too many hops, a body past `limit`, the network.
pub fn download(
    fetcher: &dyn HopFetcher,
    id: &str,
    chromium_version: &str,
    limit: u64,
) -> Result<Vec<u8>, DownloadError> {
    if !is_id(id) {
        return Err(DownloadError::BadId);
    }
    fetch(fetcher, update_url(id, chromium_version), limit)
}

/// The body at `url` after its redirects, at most `limit` bytes; every hop checked against
/// [`ALLOWED_HOSTS`] before it is asked.
///
/// # Errors
///
/// A hop off Google's hosts, too many hops, a body past `limit`, the network.
pub fn fetch(fetcher: &dyn HopFetcher, url: String, limit: u64) -> Result<Vec<u8>, DownloadError> {
    let mut url = url;
    for _ in 0..=MAX_HOPS {
        if allowed_host(&url).is_none() {
            return Err(DownloadError::HostNotAllowed(host_of(&url)));
        }
        match fetcher.get(&url).map_err(DownloadError::Network)? {
            Hop::Redirect(next) => url = resolve(&url, &next),
            Hop::Body { length, body } => return read_limited(body, length, limit),
        }
    }
    Err(DownloadError::TooManyRedirects)
}

/// What an error names for `url`: the host (lower case, the path left out), with the scheme
/// when it is not https, and "relative" for a reference without a host.
pub fn host_of(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once(':') else {
        return "relative".to_owned();
    };
    if !has_scheme(url) {
        return "relative".to_owned();
    }
    let Some(rest) = rest.strip_prefix("//") else {
        return format!("{}:", scheme.to_ascii_lowercase());
    };
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if scheme.eq_ignore_ascii_case("https") {
        host
    } else {
        format!("{}://{host}", scheme.to_ascii_lowercase())
    }
}

fn read_limited(
    mut body: Box<dyn Read + Send>,
    length: Option<u64>,
    limit: u64,
) -> Result<Vec<u8>, DownloadError> {
    if length.is_some_and(|length| length > limit) {
        return Err(DownloadError::TooLarge);
    }
    let mut bytes = Vec::new();
    let mut buffer = vec![0_u8; CHUNK];
    loop {
        let read = body
            .read(&mut buffer)
            .map_err(|error| DownloadError::Network(error.to_string()))?;
        if read == 0 {
            return Ok(bytes);
        }
        let total = u64::try_from(bytes.len() + read).map_err(|_| DownloadError::TooLarge)?;
        if total > limit {
            return Err(DownloadError::TooLarge);
        }
        bytes.extend_from_slice(buffer.get(..read).unwrap_or_default());
    }
}

/// The production fetcher: `ureq` with rustls, https only, redirects answered not followed.
pub struct HttpHops {
    agent: ureq::Agent,
}

impl Default for HttpHops {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpHops {
    /// A client with the timeouts above.
    pub fn new() -> Self {
        Self {
            agent: config().build().into(),
        }
    }
}

fn config() -> ConfigBuilder<AgentScope> {
    ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_per_read(Some(READ_TIMEOUT))
        .timeout_recv_body(Some(BODY_TIMEOUT))
        .http_status_as_error(false)
        .user_agent(format!("rBrowser/{}", env!("CARGO_PKG_VERSION")))
}

impl HopFetcher for HttpHops {
    fn get(&self, url: &str) -> Result<Hop, String> {
        let response = self
            .agent
            .get(url)
            .header(
                "Accept",
                "application/x-chrome-extension, application/octet-stream",
            )
            .call()
            .map_err(|error| error.to_string())?;
        let status = response.status();
        if status.is_redirection() {
            let location = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| format!("answered {status} without a location"))?;
            return Ok(Hop::Redirect(location.to_owned()));
        }
        if !status.is_success() {
            return Err(format!("the server answered {status}"));
        }
        let length = response.body().content_length();
        Ok(Hop::Body {
            length,
            body: Box::new(response.into_body().into_reader()),
        })
    }
}

/// A fetcher that answers from a table and counts every request; an address it does not
/// hold answers `404`. For the tests.
#[cfg(test)]
#[derive(Clone, Default)]
pub struct MemoryHops {
    answers: std::sync::Arc<std::sync::Mutex<Vec<(String, Answer)>>>,
    asked: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

#[cfg(test)]
#[derive(Clone)]
enum Answer {
    Redirect(String),
    Body(Vec<u8>, Option<u64>),
}

#[cfg(test)]
impl MemoryHops {
    /// Answers `from` with a redirect to `to`.
    pub fn redirect(&self, from: &str, to: &str) -> &Self {
        self.answers
            .lock()
            .expect("lock")
            .push((from.to_owned(), Answer::Redirect(to.to_owned())));
        self
    }

    /// Answers `at` with `bytes` (and `length` as `Content-Length`).
    pub fn body(&self, at: &str, bytes: &[u8], length: Option<u64>) -> &Self {
        self.answers
            .lock()
            .expect("lock")
            .push((at.to_owned(), Answer::Body(bytes.to_vec(), length)));
        self
    }

    /// Every address asked for so far.
    pub fn asked(&self) -> Vec<String> {
        self.asked
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[cfg(test)]
impl HopFetcher for MemoryHops {
    fn get(&self, url: &str) -> Result<Hop, String> {
        self.asked
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(url.to_owned());
        let answers = self
            .answers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (_, answer) = answers
            .iter()
            .find(|(at, _)| at == url)
            .ok_or_else(|| "404".to_owned())?;
        Ok(match answer {
            Answer::Redirect(to) => Hop::Redirect(to.clone()),
            Answer::Body(bytes, length) => Hop::Body {
                length: *length,
                body: Box::new(std::io::Cursor::new(bytes.clone())),
            },
        })
    }
}

#[cfg(test)]
#[path = "webstore_tests.rs"]
mod tests;
