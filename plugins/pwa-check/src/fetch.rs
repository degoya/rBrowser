//! Requests through the host's `http` capability, held to the page's own origin: a URL of
//! another origin is never requested (the manifest, its icons, its start URL and the service
//! worker's script belong to the page), a redirect is followed here (the host answers it, never
//! follows it) within the origin and up to [`MAX_HOPS`], and no request starts once the check's
//! time budget is spent. The host's own rules - its address rule, its rate limit, its answer
//! size - apply to every request.

use rb_plugin_api::{Failure, HttpRequest, PluginHost, codes};

use crate::url::Url;

/// The most redirects followed for one request.
pub const MAX_HOPS: usize = 5;
pub const ACCEPT_JSON: &str = "application/manifest+json,application/json;q=0.9,*/*;q=0.5";
pub const ACCEPT_IMAGE: &str =
    "image/avif,image/webp,image/png,image/svg+xml,image/*;q=0.8,*/*;q=0.5";
pub const ACCEPT_HTML: &str = "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8";
pub const ACCEPT_SCRIPT: &str = "text/javascript,application/javascript;q=0.9,*/*;q=0.5";

/// The time budget of one check, on the host's clock (seconds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    deadline: u64,
}

impl Budget {
    /// A budget of `timeout_ms` from now (rounded up to whole seconds).
    pub fn new(host: &mut dyn PluginHost, timeout_ms: u64) -> Self {
        Self {
            deadline: host
                .now_unix_seconds()
                .saturating_add(timeout_ms.div_ceil(1000)),
        }
    }

    /// Whether the budget is spent.
    pub fn spent(&self, host: &mut dyn PluginHost) -> bool {
        host.now_unix_seconds() >= self.deadline
    }
}

/// One final answer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Response {
    /// The URL that answered (after the redirects).
    pub url: String,
    pub status: u16,
    /// Names in lower case.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    /// The first value of the header `name` (lower case).
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// How a request ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fetched {
    /// The final answer, whatever its status (no redirect).
    Ok(Response),
    /// The URL, or a redirect, leaves the page's origin: not requested.
    OtherOrigin(String),
    /// The host's failure code, `redirects` (too many, or a loop, or no usable `Location`) or
    /// `time` (the budget is spent).
    Failed(String),
}

/// `url` requested with `GET` and `accept`, as the page at `page` may: its own origin only.
pub fn get(
    host: &mut dyn PluginHost,
    budget: &Budget,
    page: &Url,
    url: &Url,
    accept: &str,
) -> Fetched {
    let mut current = url.clone();
    let mut seen: Vec<Url> = Vec::new();
    loop {
        if !current.same_origin(page) {
            return Fetched::OtherOrigin(current.to_string());
        }
        if budget.spent(host) {
            return Fetched::Failed("time".to_owned());
        }
        let request = HttpRequest {
            method: "GET".to_owned(),
            url: current.to_string(),
            headers: vec![("accept".to_owned(), accept.to_owned())],
            body: Vec::new(),
        };
        let answer = match host.fetch(&request) {
            Ok(answer) => answer,
            Err(failure) => return Fetched::Failed(code(&failure)),
        };
        let headers: Vec<(String, String)> = answer
            .headers
            .into_iter()
            .map(|(name, value)| (name.to_ascii_lowercase(), value))
            .collect();
        if !(300..400).contains(&answer.status) || answer.status == 304 {
            return Fetched::Ok(Response {
                url: request.url,
                status: answer.status,
                headers,
                body: answer.body,
            });
        }
        let location = headers
            .iter()
            .find(|(name, _)| name == "location")
            .map(|(_, value)| value.as_str())
            .unwrap_or_default();
        seen.push(current.clone());
        let Some(next) = current.join(location) else {
            return Fetched::Failed("redirects".to_owned());
        };
        if seen.len() > MAX_HOPS || seen.contains(&next) {
            return Fetched::Failed("redirects".to_owned());
        }
        current = next;
    }
}

fn code(failure: &Failure) -> String {
    failure.code().unwrap_or(codes::HTTP_FAILED).to_owned()
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
