//! The MCP tools of sitemap capture (RB-050-05): `sitemap_capture` (finds the pages, starts the
//! run in the background and answers its `runId`), `sitemap_status`, `sitemap_cancel` and
//! `sitemap_gallery` (RB-090-83) - core tools, their names, descriptions, arguments and answers. `rb-mcp` (`tools/sitemap.rs`) hands
//! them to the host, which runs [`super::app`]; the commands `sitemap.capture`, `sitemap.cancel`
//! and `panels.sitemap` take the same arguments. The plugin's own tools
//! `sitemapcapture_discover` and `sitemapcapture_gallery` come from its manifest.

use rb_visual::sitemap::{Progress, RunState};
use serde::{Deserialize, Serialize};

/// The capture tool's name.
pub const CAPTURE_TOOL: &str = "sitemap_capture";
/// The capture tool's description for the agent.
pub const CAPTURE_DESCRIPTION: &str = "Capture every page of a sitemap in every breakpoint, in \
hidden views, without blocking the grid. url: a site (its robots.txt and sitemap.xml are found), \
a sitemap or sitemap index (gzip too), or a project-relative .xml, .xml.gz or .txt URL list; the \
project's startUrl when absent; bookmarks: true takes the project bookmarks instead. Optional: \
breakpoints (ids; all when absent), views (all or visible: only the breakpoints the grid \
shows; not with breakpoints), env (an environment id: the sitemap's URLs move to its \
origin, the path kept), max (pages), include/exclude (globs or re:regex), compare (against the \
visual-regression baselines), resume (a runId to continue: what is done stays, failed pages are \
tried again), session (a session id; each breakpoint's own when absent). Answers once the pages \
are found - the capture goes on in the background - with runId, the run's folder \
.rbrowser/out/sitemap-capture/<runId>/ (PNGs, run.json, index.html gallery) and the shots pending; \
poll sitemap_status. One run at a time. urls (with discovery: a sitemap_discover discoveryId) \
captures exactly those pages, each one the discovery found, at most 100000.";
/// The status tool's name.
pub const STATUS_TOOL: &str = "sitemap_status";
/// The status tool's description for the agent.
pub const STATUS_DESCRIPTION: &str = "The state of a sitemap capture (runId; the latest run when \
absent): running, cancelled or finished, pages x breakpoints, done, failed, pending, the \
estimated seconds left, the folder and the gallery once written, and the pages with an HTTP \
error, a redirect or a failed shot.";
/// The cancel tool's name.
pub const CANCEL_TOOL: &str = "sitemap_cancel";
/// The cancel tool's description for the agent.
pub const CANCEL_DESCRIPTION: &str = "Cancel a running sitemap capture (runId; the latest \
running one when absent). What was captured stays; sitemap_capture with resume: runId takes the \
rest without capturing a page twice.";

/// The gallery tool's name (RB-090-83).
pub const GALLERY_TOOL: &str = "sitemap_gallery";
/// The gallery tool's description for the agent.
pub const GALLERY_DESCRIPTION: &str = "Open the gallery (index.html) of a sitemap capture \
(runId; the latest run when absent) for the person in a window of rBrowser's own - never in the \
system browser or the grid's views. Answers the runId, the gallery's path and where it opened.";

/// The most pages a run takes.
pub const MAX_PAGES: usize = 100_000;
/// The most breakpoints, include and exclude patterns named in one call.
pub const MAX_NAMES: usize = 64;

/// The arguments of `sitemap_capture` and `sitemap.capture`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureArgs {
    /// A site, a sitemap URL or a project-relative file; the project's `startUrl` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The project bookmarks instead of a sitemap.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bookmarks: bool,
    /// Breakpoint ids; every breakpoint when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub breakpoints: Vec<String>,
    /// An environment id: the URLs move to its origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
    /// At most this many pages; `sitemap.maxPages` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<usize>,
    /// Over `sitemap.include`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
    /// Over `sitemap.exclude`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<String>>,
    /// Compare each shot with its visual-regression baseline.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub compare: bool,
    /// Continue this run instead of starting one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<String>,
    /// The session (cookies and storage) every page is captured in; each breakpoint's own when
    /// absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Hide cookie banners and chat widgets; `capture.hideBanners` when absent (RB-080-12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_banners: Option<bool>,
    /// Freeze animations and videos; `capture.freezeAnimations` when absent (RB-080-12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze_animations: Option<bool>,
    /// Every breakpoint or only those the grid shows (RB-099-08); `all` when absent. Excludes
    /// `breakpoints`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub views: Option<CaptureViews>,
    /// Exactly these pages, each one the discovery found (RB-099-09); `discovery` names it, else
    /// the source is discovered again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub urls: Option<Vec<String>>,
    /// The `discoveryId` of `sitemap_discover` or the start page the `urls` come from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery: Option<String>,
    /// A project-relative `.txt` list of the pages, one URL a line, read by the core
    /// (`rbrowser capture sitemap --urls <file>`: the instance channel carries no long list).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub urls_file: Option<String>,
}

/// The views a capture takes (RB-099-08): `"all"` or `"visible"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptureViews {
    /// Every breakpoint of the config.
    #[default]
    All,
    /// The breakpoints the grid shows at the start.
    Visible,
}

impl CaptureArgs {
    /// The call's word on hiding and freezing (RB-080-12).
    pub fn clean(&self) -> rb_visual::CleanOverrides {
        rb_visual::CleanOverrides {
            hide_banners: self.hide_banners,
            freeze_animations: self.freeze_animations,
        }
    }

    /// The breakpoints the call takes: those named, else the views chosen (RB-099-08).
    pub fn scope(&self) -> rb_config::ViewScope {
        if !self.breakpoints.is_empty() {
            return rb_config::ViewScope::Breakpoints(self.breakpoints.clone());
        }
        match self.views.unwrap_or_default() {
            CaptureViews::All => rb_config::ViewScope::All,
            CaptureViews::Visible => rb_config::ViewScope::Visible,
        }
    }

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

    /// Whether the arguments are within their limits.
    pub fn check(&self) -> Result<(), String> {
        if self.bookmarks && self.url.is_some() {
            return Err("url and bookmarks exclude each other".to_owned());
        }
        if self.views.is_some() && !self.breakpoints.is_empty() {
            return Err("views and breakpoints exclude each other".to_owned());
        }
        if self.max.is_some_and(|max| max > MAX_PAGES) {
            return Err(format!("max is at most {MAX_PAGES}"));
        }
        let lists = [
            Some(&self.breakpoints),
            self.include.as_ref(),
            self.exclude.as_ref(),
        ];
        if lists
            .into_iter()
            .flatten()
            .any(|list| list.len() > MAX_NAMES)
        {
            return Err(format!("at most {MAX_NAMES} breakpoints and patterns each"));
        }
        if self
            .url
            .as_deref()
            .is_some_and(|url| url.is_empty() || url.len() > 2048)
        {
            return Err("url is empty or longer than 2048 characters".to_owned());
        }
        let ids = [
            self.env.as_deref(),
            self.resume.as_deref(),
            self.session.as_deref(),
        ];
        if ids
            .into_iter()
            .flatten()
            .any(|id| id.is_empty() || id.len() > 128)
        {
            return Err("env, resume and session are 1 to 128 characters".to_owned());
        }
        self.check_urls()
    }

    /// Whether the call names its pages (`urls` or `urlsFile`) instead of a sitemap's selection.
    #[must_use]
    pub fn chooses_pages(&self) -> bool {
        self.urls.is_some() || self.urls_file.is_some()
    }

    /// Whether `urls` and `discovery` are within their limits and alone where they name the
    /// pages (RB-099-09).
    fn check_urls(&self) -> Result<(), String> {
        if let Some(urls) = &self.urls {
            if urls.is_empty() || urls.len() > MAX_PAGES {
                return Err(format!("urls holds 1 to {MAX_PAGES} pages"));
            }
            if urls.iter().any(|url| url.is_empty() || url.len() > 2048) {
                return Err("a url is empty or longer than 2048 characters".to_owned());
            }
            if self.resume.is_some() {
                return Err("urls and resume exclude each other".to_owned());
            }
        }
        if let Some(file) = &self.urls_file {
            if file.is_empty() || file.len() > 1024 || !file.to_ascii_lowercase().ends_with(".txt")
            {
                return Err("urlsFile is a .txt file of 1 to 1024 characters".to_owned());
            }
            if self.urls.is_some() || self.discovery.is_some() || self.resume.is_some() {
                return Err("urlsFile is not taken with urls, discovery or resume".to_owned());
            }
        }
        if let Some(discovery) = &self.discovery {
            if self.urls.is_none() {
                return Err("discovery needs urls".to_owned());
            }
            if discovery.is_empty() || discovery.len() > 128 {
                return Err("discovery is 1 to 128 characters".to_owned());
            }
            let sourced = self.url.is_some()
                || self.bookmarks
                || self.max.is_some()
                || self.include.is_some()
                || self.exclude.is_some();
            if sourced {
                return Err(
                    "discovery names the pages: url, bookmarks, max, include and exclude are \
                     not taken with it"
                        .to_owned(),
                );
            }
        }
        Ok(())
    }
}

/// The answer of `sitemap_capture`: the run started (or resumed).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStarted {
    pub run_id: String,
    /// The run's folder.
    pub folder: String,
    /// Pages times breakpoints still to take.
    pub pending: usize,
    /// The run's pages (RB-090-83: the start's notice names them); 0 from an instance that
    /// does not send them.
    #[serde(default)]
    pub pages: usize,
    /// The run's breakpoints; 0 from an instance that does not send them.
    #[serde(default)]
    pub breakpoints: usize,
}

/// The arguments of `sitemap_status` and `sitemap_cancel`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunArgs {
    /// The run; the latest when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
}

impl RunArgs {
    /// The arguments in `value` (`null` for none).
    pub fn parse(value: &serde_json::Value) -> Result<Self, String> {
        if value.is_null() {
            return Ok(Self::default());
        }
        serde_json::from_value(value.clone()).map_err(|error| error.to_string())
    }
}

/// A page with a problem, as `sitemap_status` lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageProblem {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_url: Option<String>,
    /// Breakpoint and reason of each failed shot.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed: Vec<(String, String)>,
}

/// The answer of `sitemap_status`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusAnswer {
    pub run_id: String,
    pub state: RunState,
    pub progress: Progress,
    pub folder: String,
    /// The gallery, once written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gallery: Option<String>,
    pub problems: Vec<PageProblem>,
}

/// The answer of `sitemap_cancel`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAnswer {
    pub run_id: String,
    /// Whether the run was still running.
    pub cancelled: bool,
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
