//! Reaching the download host: https at every hop, bounded, and replaceable in the tests. The
//! installer holds a [`Fetcher`], so a test serves an archive from memory ([`MemoryFetcher`])
//! and counts the requests - the proof that nothing is fetched before the person agreed. The
//! production [`HttpFetcher`] (`ureq`, rustls, as `rb_update::fetch`) refuses plain http at the
//! first address and at every redirect and sends nothing of this installation but the user
//! agent `rBrowser/<version>`. Blocking: the installer runs it on a blocking thread.

use std::io::Read;
use std::time::Duration;

use ureq::config::ConfigBuilder;
use ureq::typestate::AgentScope;

/// How long connecting may take.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the body may go without a byte (each read).
pub const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// How long the whole body may take: the pinned ffmpeg at 200 kbit/s.
pub const BODY_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
/// Most redirects one request follows (a release asset takes two).
pub const MAX_REDIRECTS: u32 = 5;

/// An opened download: its declared length, when the server named one, and its bytes.
pub struct Download {
    /// `Content-Length`.
    pub length: Option<u64>,
    /// The body.
    pub body: Box<dyn Read + Send>,
}

/// Opens downloads.
pub trait Fetcher: Send + Sync {
    /// Opens `url` for streaming.
    ///
    /// # Errors
    ///
    /// Not https, unreachable, or not `2xx`: the reason, in English.
    fn open(&self, url: &str) -> Result<Download, String>;
}

/// The production fetcher.
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
    pub fn new() -> Self {
        Self {
            agent: config(READ_TIMEOUT, BODY_TIMEOUT).build().into(),
        }
    }
}

/// The client's rules: `idle` without a byte per read, `body` for the whole body. In ureq 3
/// `timeout_recv_body` alone is the whole body's budget - a minute for the 168 MB ffmpeg needed
/// 22 Mbit/s (Charge 9 review H1).
fn config(idle: Duration, body: Duration) -> ConfigBuilder<AgentScope> {
    ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(MAX_REDIRECTS)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_per_read(Some(idle))
        .timeout_recv_body(Some(body))
        .http_status_as_error(false)
        .user_agent(format!("rBrowser/{}", env!("CARGO_PKG_VERSION")))
}

impl Fetcher for HttpFetcher {
    fn open(&self, url: &str) -> Result<Download, String> {
        if !url.starts_with("https://") {
            return Err(format!("{url} is not https"));
        }
        let response = self
            .agent
            .get(url)
            .header("Accept", "application/octet-stream")
            .call()
            .map_err(|error| format!("{url}: {error}"))?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("{url} answered {status}"));
        }
        let length = response.body().content_length();
        Ok(Download {
            length,
            body: Box::new(response.into_body().into_reader()),
        })
    }
}

/// A fetcher that serves bodies from memory and records every address asked for; an address it
/// does not hold answers `404`. For the tests.
#[cfg(test)]
#[derive(Clone, Default)]
pub struct MemoryFetcher {
    bodies: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>>,
    requests: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

#[cfg(test)]
impl MemoryFetcher {
    /// Serves `body` at `url` from now on.
    pub fn serve(&self, url: &str, body: Vec<u8>) {
        crate::cdp::lock(&self.bodies).insert(url.to_owned(), body);
    }

    /// Every address asked for so far.
    pub fn requests(&self) -> Vec<String> {
        crate::cdp::lock(&self.requests).clone()
    }
}

#[cfg(test)]
impl Fetcher for MemoryFetcher {
    fn open(&self, url: &str) -> Result<Download, String> {
        crate::cdp::lock(&self.requests).push(url.to_owned());
        if !url.starts_with("https://") {
            return Err(format!("{url} is not https"));
        }
        let body = crate::cdp::lock(&self.bodies)
            .get(url)
            .cloned()
            .ok_or_else(|| format!("{url} answered 404 Not Found"))?;
        Ok(Download {
            length: u64::try_from(body.len()).ok(),
            body: Box::new(std::io::Cursor::new(body)),
        })
    }
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
