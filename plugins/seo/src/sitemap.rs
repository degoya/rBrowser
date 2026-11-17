//! The `xhtml:link` alternates of a site's XML sitemaps (RB-060-09): the sitemaps `robots.txt`
//! names (else `/sitemap.xml`), a sitemap index followed breadth first within its own origin, at
//! most [`MAX_FILES`] files and [`MAX_DEPTH`] levels of indexes. A DTD is refused (no entity
//! expansion) and the nodes are capped, as in `plugins/sitemap-capture`; a gzipped sitemap is not
//! read. Never a sitemap on this machine or the local network for a public site.

use std::collections::{BTreeMap, VecDeque};

use rb_plugin_api::PluginHost;

use crate::fetch;
use crate::hreflang::sources::{Alternate, MAX_ALTERNATES};
use crate::url::Url;

/// The namespace of the `xhtml:link` alternates.
const XHTML_NS: &str = "http://www.w3.org/1999/xhtml";
/// The most XML nodes of one sitemap.
const MAX_NODES: u32 = 2_000_000;
/// The most sitemap files read in one run.
pub const MAX_FILES: usize = 10;
/// How deep indexes are followed.
pub const MAX_DEPTH: usize = 2;
/// The most pages with alternates kept.
pub const MAX_PAGES: usize = 10_000;
/// The most alternates kept over every sitemap (a bound on the memory a hostile sitemap takes).
pub const MAX_KEPT: usize = 50_000;
/// Redirects followed for a sitemap.
const HOPS: usize = 5;

/// What one sitemap file holds.
#[derive(Debug, PartialEq, Eq)]
pub enum Parsed {
    /// The pages with alternates and their alternates.
    Pages(BTreeMap<Url, Vec<Alternate>>),
    /// The sitemaps of an index.
    Index(Vec<Url>),
}

fn children<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    name: &'a str,
) -> impl Iterator<Item = roxmltree::Node<'a, 'input>> + 'a {
    node.children()
        .filter(move |child| child.is_element() && child.tag_name().name() == name)
}

fn loc(node: roxmltree::Node<'_, '_>, base: &Url) -> Option<Url> {
    children(node, "loc")
        .next()
        .and_then(|loc| loc.text())
        .and_then(|text| base.join(text.trim()))
}

/// The sitemap in `bytes`, its URLs resolved against `base`; `None` when it is no UTF-8, no
/// well-formed XML (a DTD included) or no sitemap.
#[must_use]
pub fn parse(bytes: &[u8], base: &Url) -> Option<Parsed> {
    let text = std::str::from_utf8(bytes).ok()?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let options = roxmltree::ParsingOptions {
        allow_dtd: false,
        nodes_limit: MAX_NODES,
        ..roxmltree::ParsingOptions::default()
    };
    let document = roxmltree::Document::parse_with_options(text, options).ok()?;
    let root = document.root_element();
    match root.tag_name().name() {
        "urlset" => {
            let mut pages = BTreeMap::new();
            for node in children(root, "url") {
                let Some(page) = loc(node, base) else {
                    continue;
                };
                let alternates: Vec<Alternate> = children(node, "link")
                    .filter(|link| {
                        link.tag_name().namespace().is_none_or(|ns| ns == XHTML_NS)
                            && link
                                .attribute("rel")
                                .is_some_and(|rel| rel.trim().eq_ignore_ascii_case("alternate"))
                    })
                    .filter_map(|link| {
                        Some(Alternate::new(
                            link.attribute("hreflang")?,
                            link.attribute("href")?,
                            base,
                        ))
                    })
                    .take(MAX_ALTERNATES)
                    .collect();
                if !alternates.is_empty() && pages.len() < MAX_PAGES {
                    pages.insert(page, alternates);
                }
            }
            Some(Parsed::Pages(pages))
        }
        "sitemapindex" => Some(Parsed::Index(
            children(root, "sitemap")
                .filter_map(|node| loc(node, base))
                .collect(),
        )),
        _ => None,
    }
}

/// The alternates every sitemap read declares, by page.
#[derive(Debug, Default)]
pub struct Sitemaps {
    pub pages: BTreeMap<Url, Vec<Alternate>>,
}

impl Sitemaps {
    /// The sitemaps `start` and the indexes they lead to; `local_site` allows sitemaps on this
    /// machine (the project's own development server).
    pub fn load(host: &mut dyn PluginHost, start: &[Url], local_site: bool) -> Self {
        let mut found = Self::default();
        let mut queue: VecDeque<(Url, usize)> = start.iter().map(|url| (url.clone(), 0)).collect();
        let mut read = 0;
        let mut kept = 0;
        let mut seen = Vec::new();
        while let Some((url, depth)) = queue.pop_front() {
            if read >= MAX_FILES {
                break;
            }
            if seen.contains(&url) || (url.is_local() && !local_site) {
                continue;
            }
            seen.push(url.clone());
            read += 1;
            let fetched = fetch::follow(host, &url, HOPS);
            let Some(response) = fetched.response.filter(|response| response.status == 200) else {
                continue;
            };
            match parse(&response.body, &url) {
                Some(Parsed::Pages(pages)) => {
                    for (page, alternates) in pages {
                        if found.pages.len() >= MAX_PAGES || kept + alternates.len() > MAX_KEPT {
                            break;
                        }
                        if let std::collections::btree_map::Entry::Vacant(entry) =
                            found.pages.entry(page)
                        {
                            kept += alternates.len();
                            entry.insert(alternates);
                        }
                    }
                }
                Some(Parsed::Index(sitemaps)) if depth < MAX_DEPTH => {
                    queue.extend(
                        sitemaps
                            .into_iter()
                            .filter(|sitemap| sitemap.origin() == url.origin())
                            .map(|sitemap| (sitemap, depth + 1)),
                    );
                }
                _ => {}
            }
        }
        found
    }
}

#[cfg(test)]
#[path = "sitemap_tests.rs"]
mod tests;
