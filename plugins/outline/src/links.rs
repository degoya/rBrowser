//! The broken-link check of the current page (RB-070-03): each `http(s)` link once, without its
//! fragment, `HEAD` first and `GET` when the server refuses `HEAD`, redirects followed here (the
//! host answers them, never follows them) up to [`MAX_HOPS`], never from a public site to a local
//! host - the rules of the seo plugin's crawl, without robots.txt (only the links of the page the
//! person looks at are requested).
//!
//! **Pace**: the host lets at most [`MAX_RPS`] requests a second through (the manifest's
//! `rate_limit_per_second`). A lower `outline.linkCheck.rps` is kept here: a component cannot
//! sleep, so before each further request it waits a whole interval (`1000 / rps` ms) in a
//! view's timer (`evaluate` of a promise that resolves after it; host time, credited back to the
//! call) - measured from the previous answer, so two starts are never closer than the interval.
//! Without a view to wait in, a lower pace cannot be kept and nothing is requested.
//!
//! **Budget**: no request starts once `outline.linkCheck.timeoutMs` have passed since the first;
//! the rest stay `unchecked`.

use rb_plugin_api::{Failure, HttpRequest, PluginHost, ViewId, codes};
use serde::Serialize;

use crate::settings::{MAX_RPS, Settings};
use crate::url::{is_checkable, is_local, join, without_fragment};

/// The most redirects followed for one link.
pub const MAX_HOPS: usize = 5;
/// The most links one check takes.
pub const MAX_LINKS: usize = 200;

/// How a link ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// A final answer below 400.
    Ok,
    /// A final answer of 400 or more.
    Broken,
    /// The request failed (no such host, refused, reset): broken too.
    Failed,
    /// More than [`MAX_HOPS`] redirects, a loop, or a redirect without a usable `Location`.
    RedirectError,
    /// The host refused the address (a private network); not requested, not broken.
    Denied,
    /// No `http(s)` URL: not requested.
    Skipped,
    /// Not reached within the budget, or the pace could not be kept.
    Unchecked,
}

/// One link's outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkResult {
    pub url: String,
    pub state: State,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    /// The failure code or why it stayed unchecked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Where the redirects led, when they did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_url: Option<String>,
}

impl LinkResult {
    fn new(url: &str, state: State) -> Self {
        Self {
            url: url.to_owned(),
            state,
            status: None,
            reason: None,
            final_url: None,
        }
    }

    /// Whether the link counts as broken.
    #[must_use]
    pub fn is_broken(&self) -> bool {
        matches!(
            self.state,
            State::Broken | State::Failed | State::RedirectError
        )
    }

    /// The status or the reason, for a finding's message.
    #[must_use]
    pub fn describe(&self) -> String {
        self.status.map_or_else(
            || self.reason.clone().unwrap_or_default(),
            |status| status.to_string(),
        )
    }
}

/// Keeps the person's pace below the host's.
struct Pacer {
    /// Milliseconds waited before each further request (none at the host's own pace).
    extra_ms: u64,
    view: Option<ViewId>,
    started: Option<u64>,
    budget_ms: u64,
}

impl Pacer {
    fn new(settings: &Settings, view: Option<ViewId>) -> Self {
        let rps = settings.rps.clamp(1, MAX_RPS);
        Self {
            extra_ms: if rps < MAX_RPS {
                1000 / u64::from(rps)
            } else {
                0
            },
            view,
            started: None,
            budget_ms: settings.timeout_ms,
        }
    }

    /// Waits for the next request's slot; the reason when no request may start.
    fn next(&mut self, host: &mut dyn PluginHost) -> Result<(), &'static str> {
        let now = host.now_unix_seconds();
        let Some(started) = self.started else {
            self.started = Some(now);
            return if self.extra_ms > 0 && self.view.is_none() {
                Err("pace")
            } else {
                Ok(())
            };
        };
        if now.saturating_sub(started).saturating_mul(1000) >= self.budget_ms {
            return Err("budget");
        }
        if self.extra_ms > 0 {
            let view = self.view.ok_or("pace")?;
            let wait = format!("new Promise((done) => setTimeout(done, {}))", self.extra_ms);
            host.evaluate(view, &wait).map_err(|_| "pace")?;
        }
        Ok(())
    }
}

fn request(
    host: &mut dyn PluginHost,
    method: &str,
    url: &str,
) -> Result<(u16, Option<String>), Failure> {
    let answer = host.fetch(&HttpRequest {
        method: method.to_owned(),
        url: url.to_owned(),
        headers: vec![("accept".to_owned(), "*/*".to_owned())],
        body: Vec::new(),
    })?;
    let location = answer
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("location"))
        .map(|(_, value)| value.clone());
    Ok((answer.status, location))
}

fn code(failure: &Failure) -> String {
    failure.code().unwrap_or(codes::HTTP_FAILED).to_owned()
}

/// One link, its redirects followed. `Err` when the pacer stopped the check.
fn check_one(
    host: &mut dyn PluginHost,
    pacer: &mut Pacer,
    start: &str,
) -> Result<LinkResult, &'static str> {
    let mut current = start.to_owned();
    let mut seen = vec![current.clone()];
    let mut result = LinkResult::new(start, State::Ok);
    loop {
        pacer.next(host)?;
        let mut answer = request(host, "HEAD", &current);
        let refused_head = match &answer {
            Ok((status, _)) => matches!(status, 405 | 501),
            Err(failure) => failure.code() == Some(codes::HTTP_FAILED),
        };
        if refused_head {
            pacer.next(host)?;
            answer = request(host, "GET", &current);
        }
        let (status, location) = match answer {
            Ok(answer) => answer,
            Err(failure) => {
                let reason = code(&failure);
                result.state = match reason.as_str() {
                    codes::HTTP_URL_DENIED => State::Denied,
                    // The server answered with a body larger than the limit: it is there.
                    codes::TOO_LARGE => State::Ok,
                    _ => State::Failed,
                };
                result.reason = Some(reason);
                return Ok(result);
            }
        };
        if !(300..400).contains(&status) || status == 304 {
            result.status = Some(status);
            result.state = if status >= 400 {
                State::Broken
            } else {
                State::Ok
            };
            if current != start {
                result.final_url = Some(current);
            }
            return Ok(result);
        }
        let next = location
            .as_deref()
            .and_then(|location| join(&current, location));
        let next = match next {
            Some(next) if !(is_local(&next) && !is_local(start)) && !seen.contains(&next) => next,
            _ => {
                result.state = State::RedirectError;
                result.status = Some(status);
                return Ok(result);
            }
        };
        if seen.len() > MAX_HOPS {
            result.state = State::RedirectError;
            result.status = Some(status);
            return Ok(result);
        }
        seen.push(next.clone());
        current = next;
    }
}

/// The distinct checkable URLs of `hrefs` (fragments dropped), at most [`MAX_LINKS`], in order.
#[must_use]
pub fn distinct(hrefs: &[String]) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    for href in hrefs {
        let url = without_fragment(href);
        if !urls.contains(&url) {
            urls.push(url);
        }
    }
    urls.truncate(MAX_LINKS);
    urls
}

/// Checks `urls` (each once) at the person's pace and within their budget; `wait_view` is a
/// Chromium view to wait in when the pace is below the host's.
pub fn check(
    host: &mut dyn PluginHost,
    settings: &Settings,
    urls: &[String],
    wait_view: Option<ViewId>,
) -> Vec<LinkResult> {
    let mut pacer = Pacer::new(settings, wait_view);
    let mut results = Vec::new();
    let mut stopped: Option<&'static str> = None;
    let urls = distinct(urls);
    let total = u32::try_from(urls.len()).unwrap_or(u32::MAX);
    for (done, url) in urls.iter().enumerate() {
        if !is_checkable(url) {
            results.push(LinkResult::new(url, State::Skipped));
            continue;
        }
        if let Some(reason) = stopped {
            let mut result = LinkResult::new(url, State::Unchecked);
            result.reason = Some(reason.to_owned());
            results.push(result);
            continue;
        }
        match check_one(host, &mut pacer, url) {
            Ok(result) => results.push(result),
            Err(reason) => {
                stopped = Some(reason);
                let mut result = LinkResult::new(url, State::Unchecked);
                result.reason = Some(reason.to_owned());
                results.push(result);
            }
        }
        host.report_progress(u32::try_from(done + 1).unwrap_or(u32::MAX), total);
    }
    results
}

#[cfg(test)]
#[path = "links_tests.rs"]
mod tests;
