//! Reaching the release host: https at every hop, bounded, and replaceable in tests.
//!
//! Whoever checks holds a [`Fetcher`] rather than a client, so a test serves a manifest and an
//! artifact from memory ([`MemoryFetcher`]) without a TLS server and can count every request -
//! the proof that a switched-off update makes none (`gate.rs`). The production [`HttpFetcher`]
//! (`ureq`, rustls) refuses plain http at the first address and at every redirect. A request
//! carries the user agent `rBrowser/<version>` and nothing else of this installation: no cookie,
//! no query, no installation id. Blocking: the updater and a check run it off the UI thread.
//!
//! Copied and adapted from rCommander's `crates/rc-update/src/fetch.rs` (RB-090-02): blocking
//! `ureq` instead of `reqwest`, no proxy setting (the environment's proxy applies).

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use ureq::config::ConfigBuilder;
use ureq::typestate::AgentScope;
/// The address type of [`Fetcher`], for an implementation outside this crate.
pub use url::Url;

/// How long fetching one small document (a manifest) may take.
pub const FETCH_TIMEOUT_SECONDS: u64 = 60;
/// How long connecting may take.
pub const CONNECT_TIMEOUT_SECONDS: u64 = 30;
/// How long a download may go without receiving a byte.
pub const READ_TIMEOUT_SECONDS: u64 = 60;
/// How long a whole body may take (a release ZIP on a slow line).
pub const BODY_TIMEOUT_SECONDS: u64 = 2 * 60 * 60;
/// Most redirects one request follows; a GitHub release asset takes two.
pub const MAX_REDIRECTS: u32 = 5;

/// A body larger than the caller's limit: reported as `update.too_large`.
#[derive(Debug, thiserror::Error)]
#[error("the answer is larger than {limit} bytes")]
pub struct BodyTooLarge {
    pub limit: u64,
}

/// `rBrowser/<version>`, all a request says about this installation.
#[must_use]
pub fn user_agent() -> String {
    format!("rBrowser/{}", env!("CARGO_PKG_VERSION"))
}

/// An opened download: its declared length, when the server named one, and its bytes.
pub struct Download {
    pub length: Option<u64>,
    pub body: Box<dyn Read + Send>,
}

/// Reads what the update check and the download need.
pub trait Fetcher: Send + Sync {
    /// Fetches `url`, refusing a body longer than `limit` bytes ([`BodyTooLarge`]). `Ok(None)`
    /// is a `404`: nothing published there.
    ///
    /// # Errors
    ///
    /// Not https, unreachable, another status, or too large.
    fn fetch(&self, url: &Url, limit: u64) -> Result<Option<Vec<u8>>>;
    /// Opens `url` for streaming.
    ///
    /// # Errors
    ///
    /// Not https, unreachable, or not `2xx`.
    fn open(&self, url: &Url) -> Result<Download>;
}

/// The client's rules: https at every hop, `idle` without a byte per read (`timeout_recv_body`
/// alone is the whole body's budget in ureq 3, which a large download on a slow line exceeds),
/// `body` for the whole body.
fn config(idle: Duration, body: Duration) -> ConfigBuilder<AgentScope> {
    ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(MAX_REDIRECTS)
        .timeout_connect(Some(Duration::from_secs(CONNECT_TIMEOUT_SECONDS)))
        .timeout_per_read(Some(idle))
        .timeout_recv_body(Some(body))
        .http_status_as_error(false)
        .user_agent(user_agent())
}

/// The production fetcher: `ureq` over rustls, https at every hop.
pub struct HttpFetcher {
    agent: ureq::Agent,
}

impl Default for HttpFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpFetcher {
    /// A client with the rules above.
    #[must_use]
    pub fn new() -> Self {
        let config = config(
            Duration::from_secs(READ_TIMEOUT_SECONDS),
            Duration::from_secs(BODY_TIMEOUT_SECONDS),
        );
        Self {
            agent: config.build().into(),
        }
    }

    fn get(
        &self,
        url: &Url,
        timeout: Option<Duration>,
    ) -> Result<Option<ureq::http::Response<ureq::Body>>> {
        ensure!(url.scheme() == "https", "{url} is not https");
        let response = self
            .agent
            .get(url.as_str())
            .config()
            .timeout_global(timeout)
            .build()
            .header("Accept", "application/json, application/octet-stream")
            .call()
            .with_context(|| format!("fetch {url}"))?;
        let status = response.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }
        if !status.is_success() {
            bail!("{url} answered {status}");
        }
        Ok(Some(response))
    }
}

impl Fetcher for HttpFetcher {
    fn fetch(&self, url: &Url, limit: u64) -> Result<Option<Vec<u8>>> {
        let Some(mut response) = self.get(url, Some(Duration::from_secs(FETCH_TIMEOUT_SECONDS)))?
        else {
            return Ok(None);
        };
        // The declared length first, so an oversized body is refused before it is read; the
        // reader's limit catches a server that declares less than it sends.
        if response
            .body()
            .content_length()
            .is_some_and(|length| length > limit)
        {
            return Err(BodyTooLarge { limit }.into());
        }
        let mut body = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(limit.saturating_add(1))
            .read_to_end(&mut body)
            .with_context(|| format!("read {url}"))?;
        if u64::try_from(body.len()).unwrap_or(u64::MAX) > limit {
            return Err(BodyTooLarge { limit }.into());
        }
        Ok(Some(body))
    }

    fn open(&self, url: &Url) -> Result<Download> {
        let response = self
            .get(url, None)?
            .with_context(|| format!("{url} answered 404 Not Found"))?;
        let length = response.body().content_length();
        Ok(Download {
            length,
            body: Box::new(response.into_body().into_reader()),
        })
    }
}

/// A fetcher that serves fixed bodies from memory and records what was asked of it; an address
/// it does not hold answers `404`. For the tests here and in the app.
#[derive(Clone, Default)]
pub struct MemoryFetcher {
    bodies: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    requests: Arc<Mutex<Vec<String>>>,
}

impl MemoryFetcher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Serves `body` at `url` from now on.
    pub fn serve(&self, url: &str, body: impl Into<Vec<u8>>) {
        if let Ok(mut bodies) = self.bodies.lock() {
            bodies.insert(url.to_owned(), body.into());
        }
    }

    /// Every address asked for so far, in order.
    #[must_use]
    pub fn requests(&self) -> Vec<String> {
        self.requests
            .lock()
            .map(|requests| requests.clone())
            .unwrap_or_default()
    }

    fn body(&self, url: &Url) -> Result<Option<Vec<u8>>> {
        if let Ok(mut requests) = self.requests.lock() {
            requests.push(url.to_string());
        }
        // The same rule as the network: nothing but https leaves.
        ensure!(url.scheme() == "https", "{url} is not https");
        Ok(self
            .bodies
            .lock()
            .ok()
            .and_then(|bodies| bodies.get(url.as_str()).cloned()))
    }
}

/// Hands out its bytes a few at a time, as a network does, so the streaming path is the one
/// tested.
struct Trickle {
    bytes: Vec<u8>,
    at: usize,
}

impl Read for Trickle {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let rest = &self.bytes[self.at..];
        let count = rest.len().min(buffer.len()).min(7);
        buffer[..count].copy_from_slice(&rest[..count]);
        self.at += count;
        Ok(count)
    }
}

impl Fetcher for MemoryFetcher {
    fn fetch(&self, url: &Url, limit: u64) -> Result<Option<Vec<u8>>> {
        let Some(body) = self.body(url)? else {
            return Ok(None);
        };
        if u64::try_from(body.len()).unwrap_or(u64::MAX) > limit {
            return Err(BodyTooLarge { limit }.into());
        }
        Ok(Some(body))
    }

    fn open(&self, url: &Url) -> Result<Download> {
        let body = self
            .body(url)?
            .with_context(|| format!("{url} answered 404 Not Found"))?;
        let length = u64::try_from(body.len()).ok();
        Ok(Download {
            length,
            body: Box::new(Trickle { bytes: body, at: 0 }),
        })
    }
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
