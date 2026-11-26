//! The discoveries of the start page and `sitemap_discover` (RB-099-09), and what holds a
//! capture of chosen pages to them. A discovery finds the pages without capturing one; it is
//! kept under its id ([`Discoveries`], the last [`KEPT`]), and `sitemap.capture` with `urls`
//! takes only pages it found ([`chosen`]: the core checks, the page is no boundary), at most
//! [`MAX_PAGES`]. The run then gets exactly those pages through the plugin's `urls` source
//! ([`run_request`]), with the environment's rewrite and no other selection.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rb_config::{EngineKind, ProjectConfig, ViewScope, resolve_scope};
use rb_ipc::{SitemapDiscovery, SitemapViewCounts};
use serde_json::{Value, json};

use super::discover_tool::DiscoveryFile;
use super::status::RunError;
use super::tool::MAX_PAGES;
use super::tree;
use crate::cdp::lock;

/// How many discoveries are kept.
pub const KEPT: usize = 4;
/// The largest URL list a capture reads (`rbrowser capture sitemap --urls <file>`).
pub const MAX_URLS_FILE_BYTES: u64 = 32 * 1024 * 1024;
/// Where `rbrowser capture sitemap --list` finds a discovery: its answer is larger than the
/// instance channel's reply (RB-099-09).
pub const DISCOVER_DIR: [&str; 3] = [".rbrowser", "out", "sitemap-discover"];
/// The longest URL a refusal names.
const NAMED_URL_CHARS: usize = 200;

/// A discovery as it is kept: its answer and where the environment's rewrite moves its pages
/// from (the site's origin, else the start URL's).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Discovered {
    pub answer: SitemapDiscovery,
    pub rewrite_from: Option<String>,
}

/// What a discovery found, before it is kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    pub source: String,
    pub site: Option<String>,
    pub rewrite_from: Option<String>,
    pub urls: Vec<String>,
    pub truncated: bool,
}

impl Found {
    /// The discovery `id` of these pages (at most [`MAX_PAGES`]) with its tree.
    #[must_use]
    pub fn into_discovered(self, id: String, views: SitemapViewCounts) -> Discovered {
        let Found {
            source,
            site,
            rewrite_from,
            mut urls,
            mut truncated,
        } = self;
        if urls.len() > MAX_PAGES {
            urls.truncate(MAX_PAGES);
            truncated = true;
        }
        let tree = tree::build(&urls);
        Discovered {
            answer: SitemapDiscovery {
                discovery_id: id,
                source,
                site,
                pages: tree.pages,
                tree: tree.nodes,
                truncated,
                max_pages: MAX_PAGES,
                views,
            },
            rewrite_from,
        }
    }
}

/// The discoveries of one window, the newest last, each with the project it was made for: a
/// discovery never drives a capture in another project (security review, wave 099-w36).
#[derive(Default)]
pub struct Discoveries {
    kept: Mutex<VecDeque<(PathBuf, Arc<Discovered>)>>,
    counter: AtomicU64,
}

impl Discoveries {
    /// A fresh id, unique in this process.
    pub fn next_id(&self, now_unix: u64) -> String {
        let n = self.counter.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{now_unix}-{n}")
    }

    /// Keeps `discovered`, made for the project at `root`, letting go of the oldest beyond
    /// [`KEPT`].
    pub fn keep(&self, root: &Path, discovered: Discovered) -> Arc<Discovered> {
        let discovered = Arc::new(discovered);
        let mut kept = lock(&self.kept);
        kept.push_back((root.to_path_buf(), discovered.clone()));
        while kept.len() > KEPT {
            kept.pop_front();
        }
        discovered
    }

    /// The discovery `id` of the project at `root`, while it is kept.
    pub fn get(&self, id: &str, root: &Path) -> Option<Arc<Discovered>> {
        lock(&self.kept)
            .iter()
            .find(|(made_for, discovered)| {
                discovered.answer.discovery_id == id && made_for.as_path() == root
            })
            .map(|(_, discovered)| discovered.clone())
    }
}

/// The pages of `urls`, each once and in the discovery's order.
///
/// # Errors
///
/// No URL ([`RunError::NoPages`]), more than [`MAX_PAGES`], or a URL the discovery did not find
/// ([`RunError::ForeignUrl`], the URL cut to a readable length).
pub fn chosen(discovered: &Discovered, urls: &[String]) -> Result<Vec<String>, RunError> {
    if urls.is_empty() {
        return Err(RunError::NoPages);
    }
    if urls.len() > MAX_PAGES {
        return Err(RunError::TooManyUrls(MAX_PAGES));
    }
    let found: HashSet<&str> = discovered.answer.pages.iter().map(String::as_str).collect();
    if let Some(foreign) = urls.iter().find(|url| !found.contains(url.as_str())) {
        let named: String = foreign.chars().take(NAMED_URL_CHARS).collect();
        return Err(RunError::ForeignUrl(named));
    }
    let wanted: HashSet<&str> = urls.iter().map(String::as_str).collect();
    Ok(discovered
        .answer
        .pages
        .iter()
        .filter(|page| wanted.contains(page.as_str()))
        .cloned()
        .collect())
}

/// The plugin's `discover` request of a run of exactly `pages`: their own list as the source, no
/// selection, and `rewrite` (`super::plan::rewrite` of the call's environment, from
/// [`Discovered::rewrite_from`]) when there is one.
#[must_use]
pub fn run_request(pages: Vec<String>, rewrite: Option<Value>) -> Value {
    let limits = json!({ "maxUrls": pages.len().max(1) });
    let mut request = json!({ "source": { "urls": pages }, "limits": limits, "selection": {} });
    if let Some(rewrite) = rewrite {
        request["rewrite"] = rewrite;
    }
    request
}

/// How many Chromium breakpoints a run takes with all views and with the visible ones (the
/// other engines are not captured in render views).
#[must_use]
pub fn view_counts(config: &ProjectConfig) -> SitemapViewCounts {
    let count = |scope: &ViewScope| {
        resolve_scope(config, scope).map_or(0, |ids| {
            ids.iter()
                .filter(|id| {
                    config
                        .breakpoints
                        .iter()
                        .any(|bp| bp.id == **id && bp.engine == EngineKind::Chromium)
                })
                .count()
        })
    };
    SitemapViewCounts {
        all: count(&ViewScope::All),
        visible: count(&ViewScope::Visible),
    }
}

/// The URLs of a list's `text`: one a line, blank lines and `#` comments left out, each once.
///
/// # Errors
///
/// No URL, or more than [`MAX_PAGES`].
pub fn parse_url_list(text: &str) -> Result<Vec<String>, String> {
    let mut seen = HashSet::new();
    let urls: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter(|line| seen.insert(*line))
        .map(str::to_owned)
        .collect();
    if urls.is_empty() {
        return Err("the URL list names no URL".to_owned());
    }
    if urls.len() > MAX_PAGES {
        return Err(format!("the URL list names more than {MAX_PAGES} URLs"));
    }
    Ok(urls)
}

/// Writes `answer` to [`DISCOVER_DIR`]`/<discoveryId>.json` below `root`; what the command line
/// is answered instead of the discovery.
///
/// # Errors
///
/// An id that is no plain name, or the file cannot be written.
pub fn write_discovery(root: &Path, answer: &SitemapDiscovery) -> Result<DiscoveryFile, String> {
    let id = &answer.discovery_id;
    if !rb_visual::slug::valid_name(id) {
        return Err(format!("{id} is no plain discovery id"));
    }
    let mut file = root.to_path_buf();
    file.extend(DISCOVER_DIR);
    std::fs::create_dir_all(&file).map_err(|error| error.to_string())?;
    file.push(format!("{id}.json"));
    let bytes = serde_json::to_vec_pretty(answer).map_err(|error| error.to_string())?;
    rb_util::fs::write_atomic(&file, &bytes).map_err(|error| error.to_string())?;
    Ok(DiscoveryFile {
        discovery_id: id.clone(),
        file: file.display().to_string(),
        pages: answer.pages.len(),
        truncated: answer.truncated,
    })
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
