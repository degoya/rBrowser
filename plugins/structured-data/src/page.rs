//! The page a view shows: its rendered document from the collector (`src/collect.js`, through
//! `evaluate` in the plugin's isolated world, read only), its JSON-LD blocks with their raw text
//! and whether they are JSON, and the tree of every JSON-LD, Microdata and RDFa node.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, codes};
use schema_org::html::sel;
use schema_org::path::element_path;
use scraper::Html;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tree::{self, Builder, Node};

/// The collector, one expression.
pub const COLLECTOR: &str = include_str!("collect.js");
/// The longest raw text kept of one JSON-LD block, in characters.
pub const MAX_RAW: usize = 100_000;
/// The longest reason kept of a block that is no JSON.
const MAX_REASON: usize = 200;

/// One `<script type="application/ld+json">`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub index: usize,
    pub path: String,
    /// The raw text, cut to [`MAX_RAW`]; `None` once an answer had to drop it.
    pub text: Option<String>,
    pub text_cut: bool,
    /// Why the text is no JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The structured data of one page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    pub url: String,
    pub blocks: Vec<Block>,
    /// The top-level nodes: the JSON-LD ones first, then Microdata, then RDFa.
    pub nodes: Vec<Node>,
    /// Nodes were left out (more than [`tree::MAX_NODES`], or deeper than [`tree::MAX_DEPTH`]).
    pub truncated: bool,
}

/// The structured data of `html`, the document of `url`.
#[must_use]
pub fn read(url: &str, html: &str) -> Page {
    let document = Html::parse_document(html);
    let mut builder = Builder::default();
    let mut page = Page {
        url: url.to_owned(),
        ..Page::default()
    };
    let script_type = sel("script[type]");
    let scripts = document.select(&script_type).filter(|script| {
        script
            .value()
            .attr("type")
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/ld+json"))
    });
    for (index, script) in scripts.enumerate() {
        let path = element_path(script);
        let text: String = script.text().collect();
        let error = match serde_json::from_str::<Value>(&text) {
            Ok(value) => {
                page.nodes.extend(builder.json_ld(&value, index, &path));
                None
            }
            Err(error) => Some(error.to_string().chars().take(MAX_REASON).collect()),
        };
        page.blocks.push(Block {
            index,
            path,
            text_cut: text.chars().count() > MAX_RAW,
            text: Some(text.chars().take(MAX_RAW).collect()),
            error,
        });
    }
    for item in schema_org::microdata::items(&document) {
        if builder.full() {
            builder.truncated = true;
            break;
        }
        page.nodes.push(builder.item(&item, String::new()));
    }
    page.truncated = builder.truncated;
    tree::finish(&mut page.nodes);
    page
}

/// What the collector answers.
#[derive(Deserialize)]
struct Collected {
    url: String,
    length: usize,
    html: Option<String>,
}

/// The rendered document a view shows, as the collector read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub url: String,
    pub html: String,
}

/// The rendered document of the page `view` shows.
///
/// # Errors
///
/// The host's failure (the view is gone, its engine has no `evaluate`),
/// `plugins.error.too_large` (`length`) for a document longer than the collector sends, or
/// `plugins.error.bad_output` for an answer that is no document.
pub fn fetch(host: &mut dyn PluginHost, view: ViewId) -> Result<Document, Failure> {
    let answer = host.evaluate(view, COLLECTOR)?;
    let collected = serde_json::from_str::<Option<Collected>>(&answer)
        .ok()
        .flatten()
        .ok_or_else(|| {
            Failure::coded(FailureKind::BadData, codes::BAD_OUTPUT).with_param("reason", "collect")
        })?;
    let html = collected.html.ok_or_else(|| {
        Failure::coded(FailureKind::Limit, codes::TOO_LARGE).with_param("length", collected.length)
    })?;
    Ok(Document {
        url: collected.url,
        html,
    })
}

/// The structured data of the page `view` shows.
///
/// # Errors
///
/// The failure of [`fetch`].
pub fn collect(host: &mut dyn PluginHost, view: ViewId) -> Result<Page, Failure> {
    let document = fetch(host, view)?;
    Ok(read(&document.url, &document.html))
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
