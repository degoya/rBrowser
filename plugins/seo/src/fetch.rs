//! The raw response of a page through the host's `http` capability: one `GET` per hop, the
//! redirect chain followed here (the host answers a redirect, never follows it) up to
//! [`MAX_HOPS`], a loop stopped, and never a hop from a public site to this machine or the local
//! network (the host refuses that too, after DNS).

use rb_plugin_api::{Failure, HttpRequest, PluginHost};

use crate::url::Url;

/// The most redirects followed for one page.
pub const MAX_HOPS: usize = 10;
/// The `Accept` header of a page, `robots.txt` or a sitemap.
pub const ACCEPT: &str = "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8";
/// The `Accept` header of a share image (RB-080-11).
pub const ACCEPT_IMAGE: &str = "image/avif,image/webp,image/png,image/jpeg,image/*;q=0.8,*/*;q=0.5";
/// The `Accept` header of `llms.txt` and `security.txt` (RB-080-11).
pub const ACCEPT_TEXT: &str = "text/plain,text/markdown;q=0.9,*/*;q=0.5";

/// One answered request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Response {
    pub url: String,
    pub status: u16,
    /// Names in lower case; a repeated header is repeated.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    /// Every value of the header `name` (lower case), a comma-joined one split for `link`-style
    /// reading by the caller.
    pub fn header<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.headers
            .iter()
            .filter(move |(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Whether the answer is HTML (by `Content-Type`, or without one by its first bytes).
    #[must_use]
    pub fn is_html(&self) -> bool {
        match self.header("content-type").next() {
            Some(kind) => {
                let kind = kind.to_ascii_lowercase();
                kind.contains("text/html") || kind.contains("application/xhtml+xml")
            }
            None => {
                let start: String = String::from_utf8_lossy(&self.body[..self.body.len().min(512)])
                    .trim_start()
                    .to_ascii_lowercase();
                start.starts_with("<!doctype html") || start.starts_with("<html")
            }
        }
    }

    /// The body as text (UTF-8, invalid sequences replaced).
    #[must_use]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Why a chain ended without a final answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
    /// More than [`MAX_HOPS`] redirects, or a URL seen twice.
    Loop,
    /// A redirect without a usable `Location`, or to a local host from a public one.
    BadLocation(String),
    /// The host refused or failed the request: its failure code.
    Failed(String),
}

/// A page's answer and the redirects that led to it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fetched {
    /// Each redirect as (URL, status), in order.
    pub hops: Vec<(String, u16)>,
    pub response: Option<Response>,
    pub stop: Option<Stop>,
}

/// One `GET` of `url`.
///
/// # Errors
///
/// The host refused or failed the request.
pub fn get(host: &mut dyn PluginHost, url: &Url) -> Result<Response, Failure> {
    request(host, "GET", url, ACCEPT)
}

/// One `method` request (`GET` or `HEAD`) of `url` with the `Accept` header `accept`.
///
/// # Errors
///
/// The host refused or failed the request.
pub fn request(
    host: &mut dyn PluginHost,
    method: &str,
    url: &Url,
    accept: &str,
) -> Result<Response, Failure> {
    let request = HttpRequest {
        method: method.to_owned(),
        url: url.to_string(),
        headers: vec![("accept".to_owned(), accept.to_owned())],
        body: Vec::new(),
    };
    let answer = host.fetch(&request)?;
    Ok(Response {
        url: request.url,
        status: answer.status,
        headers: answer
            .headers
            .into_iter()
            .map(|(name, value)| (name.to_ascii_lowercase(), value))
            .collect(),
        body: answer.body,
    })
}

fn code(failure: &Failure) -> String {
    failure
        .code()
        .unwrap_or(rb_plugin_api::codes::HTTP_FAILED)
        .to_owned()
}

/// `start` fetched, its redirects followed up to `max_hops`.
pub fn follow(host: &mut dyn PluginHost, start: &Url, max_hops: usize) -> Fetched {
    follow_with(host, start, max_hops, ACCEPT)
}

/// [`follow`] with the `Accept` header `accept`.
pub fn follow_with(
    host: &mut dyn PluginHost,
    start: &Url,
    max_hops: usize,
    accept: &str,
) -> Fetched {
    follow_method(host, start, max_hops, "GET", accept)
}

/// [`follow_with`] with the request method `method` (`GET` or `HEAD`).
pub fn follow_method(
    host: &mut dyn PluginHost,
    start: &Url,
    max_hops: usize,
    method: &str,
    accept: &str,
) -> Fetched {
    let mut fetched = Fetched::default();
    let mut current = start.clone();
    let mut seen = vec![current.to_string()];
    loop {
        let response = match request(host, method, &current, accept) {
            Ok(response) => response,
            Err(failure) => {
                fetched.stop = Some(Stop::Failed(code(&failure)));
                return fetched;
            }
        };
        if !(300..400).contains(&response.status) || response.status == 304 {
            fetched.response = Some(response);
            return fetched;
        }
        fetched.hops.push((current.to_string(), response.status));
        let location = response.header("location").next().unwrap_or_default();
        let Some(next) = current.join(location) else {
            fetched.stop = Some(Stop::BadLocation(location.chars().take(256).collect()));
            return fetched;
        };
        if next.is_local() && !start.is_local() {
            fetched.stop = Some(Stop::BadLocation(next.to_string()));
            return fetched;
        }
        if fetched.hops.len() > max_hops || seen.contains(&next.to_string()) {
            fetched.stop = Some(Stop::Loop);
            return fetched;
        }
        seen.push(next.to_string());
        current = next;
    }
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
