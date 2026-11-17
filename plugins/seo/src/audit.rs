//! One page checked: its raw response and HTML, its rendered DOM when a view shows it, every
//! rule group run over them.

use rb_plugin_api::PluginHost;
use scraper::Html;
use serde::Serialize;

use crate::config::Config;
use crate::fetch::{self, Fetched, MAX_HOPS};
use crate::page::{self, Facts};
use crate::rendered::Rendered;
use crate::rules::images::ImageEntry;
use crate::rules::{self, Sink};
use crate::site::Site;
use crate::url::Url;

/// One parsed document and its facts.
#[derive(Debug)]
pub struct Doc {
    pub html: Html,
    pub facts: Facts,
}

impl Doc {
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let html = Html::parse_document(text);
        let facts = page::facts(&html);
        Self { html, facts }
    }
}

/// What the rules of one page read.
pub struct Page<'a> {
    pub requested: Url,
    /// The URL that answered (after the redirects), the requested one without an answer.
    pub url: Url,
    pub fetched: &'a Fetched,
    pub raw: Option<Doc>,
    /// The rendered DOM, only when the view shows this very page.
    pub rendered: Option<(&'a Rendered, Facts)>,
    pub site: &'a Site,
    pub config: &'a Config,
}

impl Page<'_> {
    /// The document the rules hold to the essentials: the rendered one when there is one (what
    /// a search engine that renders sees), else the raw one.
    #[must_use]
    pub fn primary(&self) -> Option<(&Html, &Facts)> {
        match (&self.rendered, &self.raw) {
            (Some((rendered, facts)), _) => Some((&rendered.html, facts)),
            (None, Some(doc)) => Some((&doc.html, &doc.facts)),
            (None, None) => None,
        }
    }

    /// The final answer's status.
    #[must_use]
    pub fn status(&self) -> Option<u16> {
        self.fetched
            .response
            .as_ref()
            .map(|response| response.status)
    }
}

/// The summary of one checked page in the report.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSummary {
    pub url: String,
    pub final_url: String,
    pub status: Option<u16>,
    pub redirects: usize,
    pub rendered: bool,
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical: Option<String>,
    pub indexable: bool,
}

/// Whether two URLs name the same page (the fragment is never part of a [`Url`]).
fn same_page(a: &str, b: &Url) -> bool {
    Url::parse(a).is_some_and(|a| a == *b)
}

/// Checks `url`, with `rendered` when a view shows it; the findings go into `sink`. `start`: the
/// start page of a run, whose share image is fetched (RB-080-11).
pub fn check_page(
    host: &mut dyn PluginHost,
    config: &Config,
    site: &Site,
    url: &Url,
    rendered: Option<&Rendered>,
    start: bool,
    sink: &mut Sink<'_>,
) -> (PageSummary, Vec<ImageEntry>, Option<Doc>) {
    let fetched = fetch::follow(host, url, MAX_HOPS);
    let final_url = fetched
        .response
        .as_ref()
        .and_then(|response| Url::parse(&response.url))
        .unwrap_or_else(|| url.clone());
    let raw = fetched
        .response
        .as_ref()
        .filter(|response| response.is_html())
        .map(|response| Doc::parse(&response.text()));
    let rendered = rendered
        .filter(|rendered| same_page(&rendered.url, &final_url) || same_page(&rendered.url, url))
        .map(|rendered| (rendered, page::facts(&rendered.html)));
    let page = Page {
        requested: url.clone(),
        url: final_url,
        fetched: &fetched,
        raw,
        rendered,
        site,
        config,
    };
    sink.page(
        &page.url.to_string(),
        page.rendered.as_ref().map(|(r, _)| r.view),
    );
    rules::indexability::check(&page, sink);
    let mut images = Vec::new();
    if page.primary().is_some() {
        rules::canonical::check(host, &page, sink);
        rules::meta::check(&page, sink);
        rules::headings::check(&page, sink);
        rules::social::check(&page, sink);
        if start {
            rules::social_image::check(host, &page, sink);
        }
        rules::diff::check(&page, sink);
        images = rules::images::check(&page, sink);
        rules::microdata::check(&page, sink);
        rules::head::check(&page, sink);
    }
    let summary = summarize(&page);
    (summary, images, page.raw)
}

fn summarize(page: &Page<'_>) -> PageSummary {
    let facts = page.primary().map(|(_, facts)| facts);
    let noindex = facts.is_some_and(|facts| facts.robots.iter().any(|v| page::says_noindex(v)))
        || page
            .fetched
            .response
            .as_ref()
            .is_some_and(|response| response.header("x-robots-tag").any(page::says_noindex));
    PageSummary {
        url: page.requested.to_string(),
        final_url: page.url.to_string(),
        status: page.status(),
        redirects: page.fetched.hops.len(),
        rendered: page.rendered.is_some(),
        title: facts.and_then(|facts| facts.titles.first().cloned()),
        description: facts.and_then(|facts| facts.descriptions.first().cloned()),
        canonical: facts.and_then(|facts| facts.canonicals.first().cloned()),
        indexable: page.status() == Some(200) && !noindex && page.site.engine.allows(&page.url),
    }
}
