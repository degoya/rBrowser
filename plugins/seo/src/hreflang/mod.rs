//! The hreflang check (RB-060-09): the language set of a page from all three sources - the
//! head, the `Link` header and the XML sitemap - followed breadth first through the alternates
//! of every page that answers 200, at most [`MAX_SET`] pages. Each page is checked as a target
//! (`target`) and for what it declares (`declared`); the set for reciprocity (`matrix`), the
//! expected locales and the duplicates across languages (`set`).

use std::collections::BTreeSet;

use rb_plugin_api::PluginHost;

use crate::config::Config;
use crate::rendered::Rendered;
use crate::rules::Sink;
use crate::site::Site;
use crate::sitemap::Sitemaps;
use crate::url::Url;

pub mod code;
pub mod declared;
pub mod load;
pub mod matrix;
pub mod set;
pub mod sources;
pub mod target;

use load::{Loaded, Origins};
use matrix::SetReport;

/// The most pages of one language set loaded (the start page included).
pub const MAX_SET: usize = 50;

/// The code each page is given: the first alternate pointing to it, the start page's sources
/// first, in set order.
fn codes(pages: &[Loaded]) -> Vec<(Url, String)> {
    let mut codes: Vec<(Url, String)> = Vec::new();
    for page in pages {
        for (_, alternate) in page.alternates() {
            if let Some(url) = &alternate.url
                && !codes.iter().any(|(seen, _)| seen == url)
            {
                codes.push((url.clone(), alternate.code.clone()));
            }
        }
    }
    codes
}

/// Checks the language set of `start` into `sink`; answers the set and its matrix.
pub fn run(
    host: &mut dyn PluginHost,
    config: &Config,
    site: &Site,
    start: &Url,
    rendered: Option<&Rendered>,
    sink: &mut Sink<'_>,
) -> SetReport {
    let named = site.sitemaps().to_vec();
    let sitemaps = if named.is_empty() {
        Url::parse(&format!("{}/sitemap.xml", start.origin()))
            .map(|url| vec![url])
            .unwrap_or_default()
    } else {
        named
    };
    let sitemap = Sitemaps::load(host, &sitemaps, start.is_local()).pages;
    let mut origins = Origins::new(site.clone());
    let first = load::start(host, &mut origins, start, rendered, &sitemap);
    let mut queued: BTreeSet<Url> = BTreeSet::from([start.clone(), first.url.clone()]);
    let mut pages = vec![first];
    let mut next = 0;
    let mut over = 0usize;
    while next < pages.len() {
        let targets: Vec<Url> = if pages[next].ok() {
            pages[next]
                .alternates()
                .filter_map(|(_, alternate)| alternate.url.clone())
                .collect()
        } else {
            Vec::new()
        };
        next += 1;
        for url in targets {
            if !queued.insert(url.clone()) {
                continue;
            }
            if pages.len() >= MAX_SET {
                over += 1;
                continue;
            }
            let loaded = load::member(host, &mut origins, &pages[0].url, &url, &sitemap);
            pages.push(loaded);
            let done = u32::try_from(pages.len()).unwrap_or(u32::MAX);
            host.report_progress(done, done.saturating_add(1));
        }
    }
    if over > 0 {
        host.log(
            "info",
            &format!("seo: hreflang set cut at {MAX_SET} pages, {over} not loaded"),
        );
        sink.page(&pages[0].url.to_string(), pages[0].view);
        sink.emit(
            "hreflang.target_unchecked",
            "hreflang.unchecked_set",
            None,
            &[("count", over.to_string()), ("max", MAX_SET.to_string())],
        );
    }
    let by_url = codes(&pages);
    let known = target::Codes { by_url: &by_url };
    set::locales(config, &pages[0], sink);
    for page in &pages {
        sink.page(&page.url.to_string(), page.view);
        target::check(config, page, &known, sink);
        if page.ok() {
            declared::check(config, page, sink);
        }
    }
    let coded: Vec<(Loaded, String)> = pages
        .into_iter()
        .map(|page| {
            let code = known.of(&page.url).unwrap_or_default().to_owned();
            (page, code)
        })
        .collect();
    let report = matrix::build(&coded, sink);
    set::duplicates(&coded, sink);
    set::alts(&coded, sink);
    report
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
