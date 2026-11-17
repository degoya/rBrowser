//! Reaching a plugin repository: https at every hop, bounded in size and time, and replaceable in
//! tests (RB-091-01).
//!
//! Adapted from rb-update's `fetch.rs` (rCommander's `rc-update`) and rDownloader's
//! `repository_fetch.rs`. Whoever refreshes or installs holds a [`Fetcher`], so a test serves an
//! index and a package from memory ([`MemoryFetcher`]) and counts the requests. The production
//! [`HttpFetcher`] (`ureq`, rustls) refuses plain http at the first address and at every
//! redirect, refuses a body above the caller's limit before and while reading it, and gives up
//! after a fixed time. A request carries the user agent `rBrowser/<version>` and nothing else of
//! this installation. Blocking: the app runs it off the UI thread.

use std::io::Read as _;
use std::time::Duration;

use ureq::config::ConfigBuilder;
use ureq::typestate::AgentScope;
use url::Url;

use super::RepositoryError;

/// How long connecting may take.
pub const CONNECT_TIMEOUT_SECONDS: u64 = 15;
/// How long a read may go without a byte.
pub(crate) const READ_TIMEOUT_SECONDS: u64 = 30;
/// How long fetching an index may take in all.
pub const INDEX_TIMEOUT_SECONDS: u64 = 60;
/// How long downloading a package may take in all.
pub const PACKAGE_TIMEOUT_SECONDS: u64 = 10 * 60;
/// Most redirects one request follows; a GitHub release asset takes two.
pub const MAX_REDIRECTS: u32 = 5;

/// `rBrowser/<version>`, all a request says about this installation.
#[must_use]
pub fn user_agent() -> String {
    format!("rBrowser/{}", env!("CARGO_PKG_VERSION"))
}

/// Reads what a refresh and an install need.
pub trait Fetcher: Send + Sync {
    /// Fetches `url` within `timeout`, refusing a body longer than `limit` bytes.
    ///
    /// # Errors
    ///
    /// Not https, unreachable, not `2xx`, too large or too slow.
    fn fetch(&self, url: &Url, limit: u64, timeout: Duration) -> Result<Vec<u8>, RepositoryError>;
}

/// The client's rules: https at every hop, a bounded connect and an idle read.
fn config() -> ConfigBuilder<AgentScope> {
    ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(MAX_REDIRECTS)
        .timeout_connect(Some(Duration::from_secs(CONNECT_TIMEOUT_SECONDS)))
        .timeout_per_read(Some(Duration::from_secs(READ_TIMEOUT_SECONDS)))
        .http_status_as_error(false)
        .user_agent(user_agent())
}

/// The production fetcher: `ureq` over rustls.
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
        Self {
            agent: config().build().into(),
        }
    }
}

impl Fetcher for HttpFetcher {
    fn fetch(&self, url: &Url, limit: u64, timeout: Duration) -> Result<Vec<u8>, RepositoryError> {
        if url.scheme() != "https" {
            return Err(RepositoryError::InvalidUrl);
        }
        let mut response = self
            .agent
            .get(url.as_str())
            .config()
            .timeout_global(Some(timeout))
            .build()
            .header("Accept", "application/json, application/octet-stream")
            .call()
            .map_err(|error| RepositoryError::Download(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(RepositoryError::Download(format!("answered {status}")));
        }
        // The declared length first, so an oversized body is refused before it is read; the
        // reader's limit catches a server that declares less than it sends.
        if response
            .body()
            .content_length()
            .is_some_and(|length| length > limit)
        {
            return Err(RepositoryError::TooLarge { limit });
        }
        let mut body = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(limit.saturating_add(1))
            .read_to_end(&mut body)
            .map_err(|error| RepositoryError::Download(error.to_string()))?;
        if u64::try_from(body.len()).unwrap_or(u64::MAX) > limit {
            return Err(RepositoryError::TooLarge { limit });
        }
        Ok(body)
    }
}

/// A fetcher that serves fixed bodies from memory and records what was asked of it; an address
/// it does not hold answers as a `404` would. For the tests here and in the app.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Default)]
pub struct MemoryFetcher {
    bodies: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>>,
    requests: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

#[cfg(any(test, feature = "test-support"))]
impl MemoryFetcher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Serves `body` at `url` from now on.
    pub fn serve(&self, url: &str, body: impl Into<Vec<u8>>) {
        rb_util::lock(&self.bodies).insert(url.to_owned(), body.into());
    }

    /// Every address asked for so far, in order.
    #[must_use]
    pub fn requests(&self) -> Vec<String> {
        rb_util::lock(&self.requests).clone()
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Fetcher for MemoryFetcher {
    fn fetch(&self, url: &Url, limit: u64, _timeout: Duration) -> Result<Vec<u8>, RepositoryError> {
        rb_util::lock(&self.requests).push(url.to_string());
        // The same rule as the network: nothing but https leaves.
        if url.scheme() != "https" {
            return Err(RepositoryError::InvalidUrl);
        }
        let body = rb_util::lock(&self.bodies)
            .get(url.as_str())
            .cloned()
            .ok_or_else(|| RepositoryError::Download("answered 404 Not Found".to_owned()))?;
        if u64::try_from(body.len()).unwrap_or(u64::MAX) > limit {
            return Err(RepositoryError::TooLarge { limit });
        }
        Ok(body)
    }
}
