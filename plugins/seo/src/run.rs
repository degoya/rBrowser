//! One SEO run: the page (or a mini crawl over the pages of its origin, or its language set),
//! the site rules once, the duplicates across the crawl; the parity of the views when asked.
//! RB-080-11: the origin's `llms.txt` and `security.txt` once (`rules::wellknown`); the start
//! page's share image.

use std::collections::BTreeSet;

use rb_plugin_api::{Failure, FailureKind, Finding, PluginHost, Severity, ViewId};
use serde::Serialize;

use crate::audit::{PageSummary, check_page};
use crate::config::Config;
use crate::hreflang::{self, matrix::SetReport};
use crate::page;
use crate::parity::{self, ParityReport};
use crate::rendered::Rendered;
use crate::rules::images::ImageEntry;
use crate::rules::{self, Sink};
use crate::site::Site;
use crate::url::Url;

/// The most URLs a crawl keeps waiting.
const MAX_QUEUE: usize = 10_000;

/// The page alone, a crawl from it, or its language set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    Page,
    Crawl,
    Hreflang,
}

/// What to check.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Request {
    /// The page; the view's when `None`.
    pub url: Option<String>,
    pub scope: Scope,
    /// The view whose rendered DOM is compared; none checks the raw page only.
    pub view: Option<ViewId>,
    /// The rules to run; every rule when empty.
    pub rules: Vec<String>,
    /// Compare the views showing the page.
    pub parity: bool,
}

/// Findings per severity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub total: usize,
    pub error: usize,
    pub warning: usize,
    pub info: usize,
}

impl Summary {
    #[must_use]
    pub fn of(findings: &[Finding]) -> Self {
        let count = |severity| findings.iter().filter(|f| f.severity == severity).count();
        Self {
            total: findings.len(),
            error: count(Severity::Error),
            warning: count(Severity::Warning),
            info: count(Severity::Info),
        }
    }
}

/// What a run found.
#[derive(Debug, Default)]
pub struct Outcome {
    /// The start page.
    pub url: String,
    pub scope: Scope,
    /// Whether the start page was compared with a view's rendered DOM.
    pub rendered: bool,
    pub pages: Vec<PageSummary>,
    /// The images of the start page.
    pub images: Vec<ImageEntry>,
    /// The language set and its matrix (scope `hreflang`).
    pub hreflang: Option<SetReport>,
    /// The views compared (`parity`).
    pub parity: Option<ParityReport>,
    pub findings: Vec<Finding>,
}

fn bad(code: &str) -> Failure {
    Failure::coded(FailureKind::BadData, code)
}

/// The rendered page of `view`; none (logged) when the host cannot give it.
fn render(host: &mut dyn PluginHost, view: Option<ViewId>) -> Option<Rendered> {
    let view = view?;
    match Rendered::collect(host, view) {
        Ok(rendered) => Some(rendered),
        Err(failure) => {
            host.log(
                "warn",
                &format!(
                    "seo: no rendered DOM of view {view}: {}",
                    failure.code().unwrap_or("unknown")
                ),
            );
            None
        }
    }
}

fn view_url(host: &mut dyn PluginHost, view: Option<ViewId>) -> Option<String> {
    let view = view?;
    host.views()
        .ok()?
        .into_iter()
        .find(|info| info.id == view)
        .map(|info| info.url)
}

/// Runs `request`.
///
/// # Errors
///
/// No page to check (`plugin.seo.errors.no_url`) or none that is an `http(s)` URL
/// (`plugin.seo.errors.bad_url`).
pub fn run(
    host: &mut dyn PluginHost,
    config: &Config,
    request: &Request,
) -> Result<Outcome, Failure> {
    let rendered = render(host, request.view);
    let text = request
        .url
        .clone()
        .or_else(|| rendered.as_ref().map(|rendered| rendered.url.clone()))
        .or_else(|| view_url(host, request.view))
        .ok_or_else(|| bad("plugin.seo.errors.no_url"))?;
    let start = Url::parse(&text).ok_or_else(|| {
        bad("plugin.seo.errors.bad_url").with_param("url", rules::short(&text, 200))
    })?;
    let site = Site::load(host, &start);
    let mut sink = Sink::new(config, &request.rules);
    let limit = match request.scope {
        Scope::Page | Scope::Hreflang => 1,
        Scope::Crawl => config.max_pages,
    };
    let mut outcome = Outcome {
        url: start.to_string(),
        scope: request.scope,
        ..Outcome::default()
    };
    if request.scope == Scope::Hreflang {
        outcome.rendered = rendered.is_some();
        outcome.hreflang = Some(hreflang::run(
            host,
            config,
            &site,
            &start,
            rendered.as_ref(),
            &mut sink,
        ));
        if request.parity {
            outcome.parity = parity::run(host, config, &start, &mut sink);
        }
        outcome.findings = sink.findings;
        return Ok(outcome);
    }
    // A run asked only for the origin's files (`plugin.seo.check_wellknown`) loads no page.
    let pages_wanted = rules::RULES
        .iter()
        .any(|rule| !rule.id.starts_with("wellknown.") && sink.wants(rule.id));
    let limit = if pages_wanted { limit } else { 0 };
    let mut queue = vec![start.clone()];
    let mut queued: BTreeSet<Url> = BTreeSet::from([start.clone()]);
    let mut next = 0;
    while next < queue.len() && outcome.pages.len() < limit {
        let url = queue[next].clone();
        next += 1;
        let first = outcome.pages.is_empty();
        if !first && !site.crawler.allows(&url) {
            continue;
        }
        let view = if first { rendered.as_ref() } else { None };
        let (summary, images, raw) = check_page(host, config, &site, &url, view, first, &mut sink);
        if first {
            outcome.rendered = summary.rendered;
            outcome.images = images;
        }
        if request.scope == Scope::Crawl {
            let base = Url::parse(&summary.final_url).unwrap_or_else(|| url.clone());
            let mut links = raw
                .map(|doc| page::links(&doc.html, &base))
                .unwrap_or_default();
            if first
                && summary.rendered
                && let Some(rendered) = &rendered
            {
                links.extend(page::links(&rendered.html, &base));
            }
            for link in links {
                if queue.len() < MAX_QUEUE
                    && link.origin() == site.origin
                    && queued.insert(link.clone())
                {
                    queue.push(link);
                }
            }
        }
        outcome.pages.push(summary);
        let done = u32::try_from(outcome.pages.len()).unwrap_or(u32::MAX);
        let total = u32::try_from(limit.min(queue.len())).unwrap_or(u32::MAX);
        host.report_progress(done, total.max(done));
    }
    if pages_wanted {
        rules::site::check(host, &site, &mut sink);
    }
    rules::wellknown::check(host, config, &start, &mut sink);
    if request.scope == Scope::Crawl {
        rules::meta::duplicates(&outcome.pages, &mut sink);
    }
    if request.parity {
        outcome.parity = parity::run(host, config, &start, &mut sink);
    }
    outcome.findings = sink.findings;
    Ok(outcome)
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
