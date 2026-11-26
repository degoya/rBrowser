//! The MCP tool `sitemap_discover` (RB-099-09): its name, description and arguments. It finds
//! the pages as `sitemap_capture` would, without capturing one, and answers them as a list and as
//! a tree by path segments with a `discoveryId`; `sitemap_capture` with `discovery` and `urls`
//! then takes exactly the pages chosen. `rbrowser capture sitemap --list` asks the same.

use serde::{Deserialize, Serialize};

use super::tool::CaptureArgs;

/// The tool's name.
pub const DISCOVER_TOOL: &str = "sitemap_discover";
/// The tool's description for the agent.
pub const DISCOVER_DESCRIPTION: &str = "Find the pages of a sitemap without capturing one. url: \
a site, a sitemap or a project-relative .xml, .xml.gz or .txt file; the project's startUrl when \
absent; bookmarks: true takes the project bookmarks. Optional: include/exclude (globs or \
re:regex; the settings' when absent), max, depth (tree levels answered, default 3). Answers \
discoveryId, source, site, pages (every URL, in the tree's order), tree (nodes name, first, count, \
children: a node's pages are pages[first..first+count]), truncated, maxPages and views (Chromium \
breakpoints with all and with the visible views). Capture a choice with sitemap_capture \
{ discovery: discoveryId, urls: [...] }: only URLs of that discovery are taken.";

/// The tree levels answered when `depth` is absent.
pub const DEFAULT_DEPTH: usize = 3;
/// The most tree levels answered.
pub const MAX_DEPTH: usize = 32;

/// The arguments of `sitemap_discover` and of the start page's discovery (none: the project's
/// start URL).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoverArgs {
    /// A site, a sitemap URL or a project-relative file; the project's `startUrl` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The project bookmarks instead of a sitemap.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bookmarks: bool,
    /// At most this many pages; every page found when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<usize>,
    /// Over `sitemap.include`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
    /// Over `sitemap.exclude`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<String>>,
    /// The tree levels answered ([`DEFAULT_DEPTH`] when absent); the counts stay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<usize>,
}

impl DiscoverArgs {
    /// The arguments in `value` (`null` for none), checked.
    pub fn parse(value: &serde_json::Value) -> Result<Self, String> {
        if value.is_null() {
            return Ok(Self::default());
        }
        let args: Self =
            serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
        args.check()?;
        Ok(args)
    }

    /// Whether the arguments are within their limits (those `sitemap_capture` holds its source
    /// to, and `depth` at most [`MAX_DEPTH`]).
    pub fn check(&self) -> Result<(), String> {
        if self
            .depth
            .is_some_and(|depth| depth == 0 || depth > MAX_DEPTH)
        {
            return Err(format!("depth is 1 to {MAX_DEPTH}"));
        }
        self.source_args().check()
    }

    /// The source and selection as a capture's arguments: what `super::plan` and
    /// `super::settings` read. `max` absent is no limit (0), not `sitemap.maxPages`: the person
    /// chooses from every page found.
    #[must_use]
    pub fn source_args(&self) -> CaptureArgs {
        CaptureArgs {
            url: self.url.clone(),
            bookmarks: self.bookmarks,
            max: Some(self.max.unwrap_or(0)),
            include: self.include.clone(),
            exclude: self.exclude.clone(),
            ..CaptureArgs::default()
        }
    }

    /// The discovery a capture with `urls` and no `discovery` makes itself
    /// (`rbrowser capture sitemap --urls <file>`): the capture's source and patterns.
    #[must_use]
    pub fn of_capture(args: &CaptureArgs) -> Self {
        Self {
            url: args.url.clone(),
            bookmarks: args.bookmarks,
            max: args.max,
            include: args.include.clone(),
            exclude: args.exclude.clone(),
            depth: None,
        }
    }
}

/// What `rbrowser capture sitemap --list` is answered: the discovery itself is written to `file`
/// (`.rbrowser/out/sitemap-discover/<discoveryId>.json`), larger than the instance channel's
/// reply may be.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryFile {
    pub discovery_id: String,
    pub file: String,
    pub pages: usize,
    pub truncated: bool,
}

#[cfg(test)]
#[path = "discover_tool_tests.rs"]
mod tests;
