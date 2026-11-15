//! One page of a language set loaded: the start page with its redirects followed, every other
//! one with a single `GET` (an alternate has to answer 200 itself), never past the crawl rules of
//! its origin's `robots.txt` and never from a public site to this machine. What the rules need
//! is kept; the parsed document is not.

use std::collections::BTreeMap;

use rb_plugin_api::PluginHost;
use scraper::Html;

use crate::audit::Doc;
use crate::fetch::{self, MAX_HOPS};
use crate::hreflang::sources::{Alternate, Source, header_alternates, html_alternates};
use crate::page::{self, sel, text_of};
use crate::rendered::Rendered;
use crate::rules::canonical::link_header_canonicals;
use crate::rules::images::alt::normalize;
use crate::site::Site;
use crate::url::Url;

/// The most origins whose `robots.txt` a set loads.
pub const MAX_ORIGINS: usize = 10;
/// The most alt texts kept of one page.
const MAX_ALTS: usize = 500;

/// Why a page of the set was not loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skipped {
    /// `robots.txt` disallows it for rBrowser's crawl.
    Robots,
    /// A public site's alternate on this machine or the local network.
    Local,
    /// More pages or origins than a run loads.
    Limit,
}

/// How the page answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The final status (after the start page's redirects).
    Status(u16),
    /// An alternate that redirects: its status.
    Redirect(u16),
    /// No answer: the failure code or why the chain stopped.
    Failed(String),
    Skipped(Skipped),
}

/// What the rules read of one page of the set.
#[derive(Clone, Debug)]
pub struct Loaded {
    /// The page (the start page's final URL).
    pub url: Url,
    pub answer: Answer,
    /// The view whose rendered DOM was read, for the start page.
    pub view: Option<rb_plugin_api::ViewId>,
    /// The alternates by source; a source without the page has no entry.
    pub declared: BTreeMap<Source, Vec<Alternate>>,
    pub title: Option<String>,
    pub description: Option<String>,
    /// The canonicals of the head and the `Link` header, as written.
    pub canonicals: Vec<String>,
    pub lang: Option<String>,
    /// Every `Content-Language` value.
    pub content_language: Vec<String>,
    pub noindex: bool,
    /// `robots.txt` keeps search engines out.
    pub engine_blocked: bool,
    /// The visible text of `main` (or the body), collapsed and cut.
    pub text: String,
    /// The non-empty alt texts, normalised.
    pub alts: Vec<String>,
}

impl Loaded {
    #[must_use]
    pub fn new(url: Url, answer: Answer) -> Self {
        Self {
            url,
            answer,
            view: None,
            declared: BTreeMap::new(),
            title: None,
            description: None,
            canonicals: Vec::new(),
            lang: None,
            content_language: Vec::new(),
            noindex: false,
            engine_blocked: false,
            text: String::new(),
            alts: Vec::new(),
        }
    }

    /// Answered 200 itself: a page whose declarations and links count.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.answer == Answer::Status(200)
    }

    /// Every alternate of every source, in source order.
    pub fn alternates(&self) -> impl Iterator<Item = (Source, &Alternate)> {
        self.declared
            .iter()
            .flat_map(|(source, list)| list.iter().map(move |alternate| (*source, alternate)))
    }

    /// Whether any source declares `target`.
    #[must_use]
    pub fn declares(&self, target: &Url) -> bool {
        self.alternates()
            .any(|(_, alternate)| alternate.url.as_ref() == Some(target))
    }
}

/// The `robots.txt` of every origin the set reaches, each read once.
pub struct Origins {
    sites: BTreeMap<String, Site>,
}

impl Origins {
    #[must_use]
    pub fn new(start: Site) -> Self {
        Self {
            sites: BTreeMap::from([(start.origin.clone(), start)]),
        }
    }

    /// The site of `url`, loaded on first use; `None` past [`MAX_ORIGINS`].
    pub fn site(&mut self, host: &mut dyn PluginHost, url: &Url) -> Option<&Site> {
        let origin = url.origin();
        if !self.sites.contains_key(&origin) {
            if self.sites.len() >= MAX_ORIGINS {
                return None;
            }
            let site = Site::load(host, url);
            self.sites.insert(origin.clone(), site);
        }
        self.sites.get(&origin)
    }
}

/// The facts of a document and its response into `loaded`.
fn read(
    loaded: &mut Loaded,
    html: &Html,
    headers: &[(String, String)],
    sitemap: Option<&[Alternate]>,
) {
    let header = |name: &'static str| {
        headers
            .iter()
            .filter(move |(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    };
    let facts = page::facts(html);
    loaded
        .declared
        .insert(Source::Html, html_alternates(html, &loaded.url));
    loaded.declared.insert(
        Source::Header,
        header_alternates(header("link"), &loaded.url),
    );
    loaded.declared.retain(|_, list| !list.is_empty());
    if let Some(sitemap) = sitemap {
        loaded.declared.insert(Source::Sitemap, sitemap.to_vec());
    }
    loaded.title = facts.titles.first().cloned();
    loaded.description = facts.descriptions.first().cloned();
    loaded.canonicals = facts.canonicals.clone();
    loaded
        .canonicals
        .extend(link_header_canonicals(header("link")));
    loaded.lang = facts.lang.clone();
    loaded.content_language = header("content-language")
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect();
    loaded.noindex = facts.robots.iter().any(|value| page::says_noindex(value))
        || header("x-robots-tag").any(page::says_noindex);
    loaded.text = html
        .select(&sel("main, [role=main]"))
        .next()
        .or_else(|| html.select(&sel("body")).next())
        .map(text_of)
        .unwrap_or_default();
    loaded.alts = html
        .select(&sel("img[alt]"))
        .filter_map(|img| img.value().attr("alt"))
        .map(normalize)
        .filter(|alt| !alt.is_empty())
        .take(MAX_ALTS)
        .collect();
}

/// The start page: its redirects followed, the rendered DOM of the view when it shows the page.
pub fn start(
    host: &mut dyn PluginHost,
    origins: &mut Origins,
    url: &Url,
    rendered: Option<&Rendered>,
    sitemap: &BTreeMap<Url, Vec<Alternate>>,
) -> Loaded {
    let fetched = fetch::follow(host, url, MAX_HOPS);
    let Some(response) = fetched.response else {
        let reason = match fetched.stop {
            Some(fetch::Stop::Failed(code)) => code,
            _ => "redirect".to_owned(),
        };
        return Loaded::new(url.clone(), Answer::Failed(reason));
    };
    let final_url = Url::parse(&response.url).unwrap_or_else(|| url.clone());
    let mut loaded = Loaded::new(final_url.clone(), Answer::Status(response.status));
    let same = |text: &str| Url::parse(text).is_some_and(|seen| seen == final_url || seen == *url);
    let html = match rendered.filter(|rendered| same(&rendered.url)) {
        Some(rendered) => {
            loaded.view = Some(rendered.view);
            Some(rendered.html.clone())
        }
        None => response
            .is_html()
            .then(|| Doc::parse(&response.text()).html),
    };
    if let Some(html) = html {
        read(
            &mut loaded,
            &html,
            &response.headers,
            sitemap.get(&final_url).map(Vec::as_slice),
        );
    }
    if let Some(site) = origins.site(host, &final_url) {
        loaded.engine_blocked = !site.engine.allows(&final_url);
    }
    loaded
}

/// An alternate of the set: one `GET`, its redirect a finding, not followed.
pub fn member(
    host: &mut dyn PluginHost,
    origins: &mut Origins,
    start: &Url,
    url: &Url,
    sitemap: &BTreeMap<Url, Vec<Alternate>>,
) -> Loaded {
    if url.is_local() && !start.is_local() {
        return Loaded::new(url.clone(), Answer::Skipped(Skipped::Local));
    }
    let Some(site) = origins.site(host, url) else {
        return Loaded::new(url.clone(), Answer::Skipped(Skipped::Limit));
    };
    let engine_blocked = !site.engine.allows(url);
    if !site.crawler.allows(url) {
        let mut loaded = Loaded::new(url.clone(), Answer::Skipped(Skipped::Robots));
        loaded.engine_blocked = engine_blocked;
        return loaded;
    }
    let mut loaded = match fetch::get(host, url) {
        Err(failure) => Loaded::new(
            url.clone(),
            Answer::Failed(failure.code().unwrap_or_default().to_owned()),
        ),
        Ok(response) if (300..400).contains(&response.status) && response.status != 304 => {
            Loaded::new(url.clone(), Answer::Redirect(response.status))
        }
        Ok(response) => {
            let mut loaded = Loaded::new(url.clone(), Answer::Status(response.status));
            if response.is_html() {
                let html = Html::parse_document(&response.text());
                read(
                    &mut loaded,
                    &html,
                    &response.headers,
                    sitemap.get(url).map(Vec::as_slice),
                );
            }
            loaded
        }
    };
    loaded.engine_blocked = engine_blocked;
    loaded
}

#[cfg(test)]
#[path = "load_tests.rs"]
mod tests;
