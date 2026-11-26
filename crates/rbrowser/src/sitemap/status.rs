//! What `sitemap_status` answers of a run ([`status_of`]), and why a run call fails
//! ([`RunError`], with the stable code the MCP tools and the shell's notices name).

use std::path::Path;

use rb_visual::VisualError;
use rb_visual::sitemap::{ShotState, SitemapRun};
use rb_visual::sitemap_store::GALLERY_FILE;

use super::plan::PlanError;
use super::tool::{PageProblem, StatusAnswer};

/// The most pages with a problem `sitemap_status` lists (the gallery has them all).
pub const MAX_PROBLEMS: usize = 200;

/// Why a run call failed.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// Another run is in progress.
    #[error("sitemap.error.busy: a sitemap capture is already running")]
    Busy,
    /// No run of that id (empty: no run at all).
    #[error("sitemap.error.not_found: no sitemap capture {0}")]
    NotFound(String),
    /// The call cannot become a run.
    #[error("{0}")]
    Plan(PlanError),
    /// The plugin's `discover` failed: its code and parameters.
    #[error("sitemap.error.discover: {0}")]
    Discover(String),
    /// The run has no gallery yet (RB-090-83).
    #[error("sitemap.error.no_gallery: the sitemap capture {0} has no gallery yet")]
    NoGallery(String),
    /// The sources gave no page.
    #[error("sitemap.error.no_pages: the sitemap gave no page to capture")]
    NoPages,
    /// Open a project first.
    #[error("sitemap.error.no_project: open a project to capture its sitemap")]
    NoProject,
    /// The run's folder or `run.json`.
    #[error("sitemap.error.store: {0}")]
    Store(String),
    /// Anything else.
    #[error("sitemap.error.failed: {0}")]
    Failed(String),
    /// `discovery` names no discovery kept (RB-099-09): a new one is needed.
    #[error("sitemap.error.unknown_discovery: no sitemap discovery {0}; discover the pages again")]
    UnknownDiscovery(String),
    /// A URL of `urls` that the discovery did not find (RB-099-09).
    #[error("sitemap.error.foreign_url: {0} is not a page the discovery found")]
    ForeignUrl(String),
    /// More URLs than one run takes (RB-099-09).
    #[error("sitemap.error.too_many_urls: at most {0} pages per run")]
    TooManyUrls(usize),
}

impl RunError {
    /// The stable code (an i18n key).
    pub fn code(&self) -> &'static str {
        match self {
            Self::Busy => "sitemap.error.busy",
            Self::NotFound(_) => "sitemap.error.not_found",
            Self::NoGallery(_) => "sitemap.error.no_gallery",
            Self::Plan(error) => error.code(),
            Self::Discover(_) => "sitemap.error.discover",
            Self::NoPages => "sitemap.error.no_pages",
            Self::NoProject => "sitemap.error.no_project",
            Self::Store(_) => "sitemap.error.store",
            Self::Failed(_) => "sitemap.error.failed",
            Self::UnknownDiscovery(_) => "sitemap.error.unknown_discovery",
            Self::ForeignUrl(_) => "sitemap.error.foreign_url",
            Self::TooManyUrls(_) => "sitemap.error.too_many_urls",
        }
    }
}

impl From<VisualError> for RunError {
    fn from(error: VisualError) -> Self {
        Self::Store(error.to_string())
    }
}

/// The pages of `run` with an HTTP error, a redirect or a failed shot, at most
/// [`MAX_PROBLEMS`].
pub fn problems(run: &SitemapRun) -> Vec<PageProblem> {
    run.pages
        .iter()
        .filter_map(|page| {
            let failed: Vec<(String, String)> = page
                .shots
                .iter()
                .filter(|(_, shot)| shot.state == ShotState::Failed)
                .map(|(name, shot)| (name.clone(), shot.error.clone().unwrap_or_default()))
                .collect();
            let http_error = page.status.is_some_and(|status| status >= 400);
            (http_error || page.final_url.is_some() || !failed.is_empty()).then(|| PageProblem {
                url: page.url.clone(),
                status: page.status,
                final_url: page.final_url.clone(),
                failed,
            })
        })
        .take(MAX_PROBLEMS)
        .collect()
}

/// The status of `run`, whose folder is `dir`, at `parallel` shots at once.
pub fn status_of(run: &SitemapRun, dir: &Path, parallel: usize) -> StatusAnswer {
    let gallery = dir.join(GALLERY_FILE);
    StatusAnswer {
        run_id: run.run_id.clone(),
        state: run.state,
        progress: run.progress(parallel),
        folder: dir.display().to_string(),
        gallery: gallery.is_file().then(|| gallery.display().to_string()),
        problems: problems(run),
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
