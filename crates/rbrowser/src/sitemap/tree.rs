//! The pages of a discovery as a tree by path segments (RB-099-09): a site's origin at the top,
//! a folder per path segment below it, a page at the end. The start page and `sitemap_discover`
//! show it; a node's pages are one range of the list, which [`build`] puts in the tree's order.
//!
//! - A page whose path is a folder of other pages (`/de` beside `/de/produkte`) is that folder's
//!   first child, named by its whole path (`/de`, `/de/`); the site's root page is `/`.
//! - Any other page is a leaf named by its last segment and what follows it: a trailing slash,
//!   the query (`search?q=1`). A query never makes a folder; the fragment is left out.
//! - Within a folder the pages of its own path come first, then the folders and pages by name;
//!   the sites keep the order they were found in.
//! - At most [`MAX_DEPTH`] segments make folders; the rest stays in the last one's name.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rb_ipc::SitemapTreeNode;
use rb_util::url::origin_of;

/// The most path segments that become folders.
pub const MAX_DEPTH: usize = 16;

/// The pages in the tree's order and the tree over them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tree {
    pub pages: Vec<String>,
    pub nodes: Vec<SitemapTreeNode>,
}

/// A URL taken apart.
struct Parts {
    origin: String,
    segments: Vec<String>,
    /// What follows the last segment: `/` for a trailing slash, then `?query`.
    tail: String,
}

fn parts(url: &str) -> Parts {
    let start = url.find("://").map_or(0, |at| at + 3);
    let path_at = url[start..]
        .find(['/', '?', '#'])
        .map_or(url.len(), |at| start + at);
    let origin = origin_of(url).unwrap_or_else(|| url[..path_at].to_owned());
    let rest = &url[path_at..];
    let rest = rest.split_once('#').map_or(rest, |(before, _)| before);
    let (path, query) = rest
        .split_once('?')
        .map_or((rest, None), |(p, q)| (p, Some(q)));
    let mut segments: Vec<String> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(str::to_owned)
        .collect();
    if segments.len() > MAX_DEPTH {
        let rest = segments.split_off(MAX_DEPTH - 1).join("/");
        segments.push(rest);
    }
    let mut tail = String::new();
    if !segments.is_empty() && path.ends_with('/') {
        tail.push('/');
    }
    if let Some(query) = query {
        tail.push('?');
        tail.push_str(query);
    }
    Parts {
        origin,
        segments,
        tail,
    }
}

/// One folder while the tree is built: the pages of its own path, then its entries by name.
#[derive(Default)]
struct Folder {
    own: Vec<(String, usize)>,
    pages: Vec<(String, usize)>,
    folders: BTreeMap<String, Folder>,
}

/// A page or a folder inside a folder.
enum Entry {
    Page(usize),
    Folder(Folder),
}

impl Folder {
    fn at(&mut self, path: &[String]) -> &mut Folder {
        path.iter().fold(self, |folder, segment| {
            folder.folders.entry(segment.clone()).or_default()
        })
    }

    /// The folder as a node named `name`; its pages go to `order`.
    fn emit(self, name: String, urls: &[String], order: &mut Vec<String>) -> SitemapTreeNode {
        let first = order.len();
        let mut children: Vec<SitemapTreeNode> = self
            .own
            .into_iter()
            .map(|(label, index)| leaf(label, index, urls, order))
            .collect();
        let mut entries: Vec<(String, Entry)> = self
            .pages
            .into_iter()
            .map(|(label, index)| (label, Entry::Page(index)))
            .chain(
                self.folders
                    .into_iter()
                    .map(|(segment, folder)| (segment, Entry::Folder(folder))),
            )
            .collect();
        let is_folder = |entry: &Entry| matches!(entry, Entry::Folder(_));
        entries.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| is_folder(&a.1).cmp(&is_folder(&b.1)))
        });
        for (label, entry) in entries {
            children.push(match entry {
                Entry::Page(index) => leaf(label, index, urls, order),
                Entry::Folder(folder) => folder.emit(label, urls, order),
            });
        }
        SitemapTreeNode {
            name,
            first,
            count: order.len() - first,
            children,
        }
    }
}

/// Page `index` of `urls` as a leaf named `name`, put next into `order`.
fn leaf(name: String, index: usize, urls: &[String], order: &mut Vec<String>) -> SitemapTreeNode {
    let first = order.len();
    order.push(urls.get(index).cloned().unwrap_or_default());
    SitemapTreeNode {
        name,
        first,
        count: 1,
        children: Vec::new(),
    }
}

/// The tree of `urls` (each once) and the pages in its order.
#[must_use]
pub fn build(urls: &[String]) -> Tree {
    let split: Vec<Parts> = urls.iter().map(|url| parts(url)).collect();
    let mut folders: BTreeSet<(&str, &[String])> = BTreeSet::new();
    for part in &split {
        for end in 1..part.segments.len() {
            folders.insert((part.origin.as_str(), &part.segments[..end]));
        }
    }
    let mut sites: Vec<(String, Folder)> = Vec::new();
    let mut site_of: HashMap<String, usize> = HashMap::new();
    for (index, part) in split.iter().enumerate() {
        let at = *site_of.entry(part.origin.clone()).or_insert_with(|| {
            sites.push((part.origin.clone(), Folder::default()));
            sites.len() - 1
        });
        let Some((_, site)) = sites.get_mut(at) else {
            continue;
        };
        let segments = &part.segments;
        if segments.is_empty() {
            site.own.push((format!("/{}", part.tail), index));
        } else if folders.contains(&(part.origin.as_str(), segments.as_slice())) {
            let label = format!("/{}{}", segments.join("/"), part.tail);
            site.at(segments).own.push((label, index));
        } else {
            let (last, parent) = segments.split_last().expect("segments is not empty");
            site.at(parent)
                .pages
                .push((format!("{last}{}", part.tail), index));
        }
    }
    let mut order = Vec::with_capacity(urls.len());
    let nodes = sites
        .into_iter()
        .map(|(origin, site)| site.emit(origin, urls, &mut order))
        .collect();
    Tree {
        pages: order,
        nodes,
    }
}

/// `nodes` cut below `depth` levels (1: the sites alone); the counts stay.
pub fn prune(nodes: &mut [SitemapTreeNode], depth: usize) {
    for node in nodes {
        if depth <= 1 {
            node.children.clear();
        } else {
            prune(&mut node.children, depth - 1);
        }
    }
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
