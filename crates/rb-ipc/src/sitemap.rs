//! The window's last sitemap capture as the shell shows it (RB-050-05): the toolbar's progress
//! while it runs, and its outcome with the gallery afterwards.

use serde::{Deserialize, Serialize};

/// Where a sitemap capture stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SitemapRunState {
    /// Pages are captured.
    Running,
    /// Cancelled; it can be resumed.
    Cancelled,
    /// Every page was taken (or failed for good).
    Finished,
}

/// A sitemap capture's progress.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SitemapStatus {
    /// The run's id (`sitemap_status`, the resume).
    pub run_id: String,
    pub state: SitemapRunState,
    /// Pages times breakpoints.
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    /// The estimated seconds left; absent before the first shot and once it ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<u64>,
    /// The run's folder.
    pub folder: String,
    /// The gallery (`index.html`), once written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gallery: Option<String>,
}
