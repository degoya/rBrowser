//! The start page of a sitemap capture (RB-099-09): `sitemapStart` with `op: "discover"` finds
//! the project's pages without capturing one, `sitemapDiscovery` answers them as a list and as a
//! tree by path segments; `op: "close"` closes the page's window. The same answer is the MCP tool
//! `sitemap_discover`'s. TypeScript mirror: `ui/src/ipc/sitemapDiscovery.ts`.

use serde::{Deserialize, Serialize};

/// What the start page asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SitemapStartOp {
    /// Find the pages of the project's start URL; answered with `sitemapDiscovery`.
    Discover,
    /// Close the start page's window; answered with `ack`.
    Close,
}

/// `sitemapStart`: the start page's request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SitemapStartRequest {
    pub op: SitemapStartOp,
}

/// One node of the page tree: a site (its origin) at the top, a path segment below it, a page at
/// the end. The node's pages are `pages[first .. first + count]` of [`SitemapDiscovery`]: the
/// list is in the tree's order, so a subtree is one range. A node without children is one page.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SitemapTreeNode {
    /// The origin, the segment, or the page's path (and query) below its parent.
    pub name: String,
    pub first: usize,
    pub count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<SitemapTreeNode>,
}

/// How many Chromium breakpoints a run takes with each choice of views (RB-099-08).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SitemapViewCounts {
    pub all: usize,
    pub visible: usize,
}

/// `sitemapDiscovery`: the pages found, before anything is captured.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SitemapDiscovery {
    /// Names this result for `sitemap.capture`'s `discovery`: only its pages may be captured.
    pub discovery_id: String,
    /// The source as `run.json` names it (the site, the sitemap, the file, `bookmarks`).
    pub source: String,
    /// The site's origin, when the source had one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    /// The pages, in the tree's order.
    pub pages: Vec<String>,
    pub tree: Vec<SitemapTreeNode>,
    /// A limit cut the discovery short.
    #[serde(default)]
    pub truncated: bool,
    /// The most pages one run takes.
    pub max_pages: usize,
    pub views: SitemapViewCounts,
}

#[cfg(test)]
#[path = "sitemap_discovery_tests.rs"]
mod tests;
