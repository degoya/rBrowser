//! The state of one sitemap capture (RB-050-05), kept as `run.json` in the run's folder
//! ([`crate::sitemap_store`]): the pages (URL, slug, HTTP status, where a redirect ended) and
//! per breakpoint one shot - pending, done (its file and duration) or failed (the reason and the
//! attempts). The run is written after every shot, so an aborted run resumes with exactly the
//! shots that are not done yet ([`SitemapRun::tasks`]) and nothing is captured twice.
//!
//! The gallery of the `sitemap-capture` plugin reads this file; its fields are the contract.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::naming::sanitize_segment;
use crate::slug::{MAX_NAME, MAX_SLUG, SlugStrategy, slug, valid_name};

/// The version of `run.json` this module writes.
pub const RUN_VERSION: u32 = 1;

/// One breakpoint of the run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunBreakpoint {
    /// The file name of its shots (`mobile` gives `mobile.png`).
    pub name: String,
    /// The label shown (the config's name).
    #[serde(default)]
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub dpr: f64,
}

/// Where a shot stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShotState {
    #[default]
    Pending,
    Done,
    Failed,
}

/// The comparison of a shot with its visual-regression baseline (`--compare`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ShotCompare {
    /// `ok`, `differs`, `new`, `sizeMismatch` or `failed` (`rb_visual::Status`).
    pub status: String,
    /// The share of differing pixels, 0 to 1.
    pub ratio: f64,
    /// The diff image, relative to the run's folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_file: Option<String>,
}

/// One page in one breakpoint.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Shot {
    pub state: ShotState,
    /// The PNG, relative to the run's folder (`about/mobile.png`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// How long the last attempt took.
    pub ms: u64,
    pub attempts: u32,
    /// Why the last attempt failed (`timeout`, `navigation`, ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compare: Option<ShotCompare>,
}

/// One page of the run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RunPage {
    /// The URL captured.
    pub url: String,
    /// The URL in the sitemap, when the environment rewrite changed it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sitemap_url: Option<String>,
    /// The page's folder, unique in the run.
    pub slug: String,
    /// The HTTP status of the main document, once loaded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    /// Where the page ended when it redirected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_url: Option<String>,
    /// The shots by breakpoint name.
    pub shots: BTreeMap<String, Shot>,
}

/// Where the run stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunState {
    #[default]
    Running,
    Cancelled,
    Finished,
}

/// `run.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SitemapRun {
    pub version: u32,
    pub run_id: String,
    /// The sitemap, site or file the pages came from.
    pub source: String,
    /// The environment the URLs were rewritten to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
    pub full_page: bool,
    pub state: RunState,
    pub created_unix: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_unix: Option<u64>,
    pub breakpoints: Vec<RunBreakpoint>,
    pub pages: Vec<RunPage>,
}

/// One shot to take: the page's index and the breakpoint's name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Task {
    pub page: usize,
    pub breakpoint: String,
}

/// What one attempt gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The main document's HTTP status, when the page loaded.
    pub status: Option<u16>,
    /// Where it ended, when that is not the URL asked for.
    pub final_url: Option<String>,
    /// The PNG's path relative to the run's folder, or why it failed.
    pub result: Result<String, String>,
    pub ms: u64,
}

/// How far the run is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub pages: usize,
    pub breakpoints: usize,
    /// Pages times breakpoints.
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub pending: usize,
    /// The seconds still to go at the mean duration so far; `None` before the first shot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<u64>,
}

/// A breakpoint's file name: ASCII letters, digits, `.`, `_` and `-`, no device name.
#[must_use]
pub fn file_name(text: &str) -> String {
    let plain: String = text
        .chars()
        .take(MAX_NAME)
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let name = sanitize_segment(&plain);
    if valid_name(&name) {
        name
    } else {
        "breakpoint".to_owned()
    }
}

/// A slug for `url` not in `taken`: its path and query, `-2`, `-3`, ... on a clash.
fn unique_slug(url: &str, taken: &mut BTreeSet<String>) -> String {
    let base = sanitize_segment(&slug(url, SlugStrategy::PathQuery));
    let mut candidate = base.clone();
    let mut n = 1;
    while !taken.insert(candidate.clone()) {
        n += 1;
        let suffix = format!("-{n}");
        let keep = MAX_SLUG.saturating_sub(suffix.len());
        candidate = format!("{}{suffix}", &base[..base.len().min(keep)]);
    }
    candidate
}

impl SitemapRun {
    /// A new run of `pages` (`(url, sitemap_url)`) in `breakpoints`, every shot pending.
    /// Breakpoints get file names ([`file_name`]); one whose name is taken is dropped.
    #[must_use]
    pub fn new(
        run_id: &str,
        source: &str,
        breakpoints: Vec<RunBreakpoint>,
        pages: &[(String, Option<String>)],
        now_unix: u64,
    ) -> Self {
        let mut names = BTreeSet::new();
        let breakpoints: Vec<RunBreakpoint> = breakpoints
            .into_iter()
            .filter_map(|mut breakpoint| {
                breakpoint.name = file_name(&breakpoint.name);
                names.insert(breakpoint.name.clone()).then_some(breakpoint)
            })
            .collect();
        let mut taken = BTreeSet::new();
        let pages = pages
            .iter()
            .map(|(url, sitemap_url)| RunPage {
                slug: unique_slug(url, &mut taken),
                url: url.clone(),
                sitemap_url: sitemap_url.clone(),
                shots: breakpoints
                    .iter()
                    .map(|breakpoint| (breakpoint.name.clone(), Shot::default()))
                    .collect(),
                ..RunPage::default()
            })
            .collect();
        Self {
            version: RUN_VERSION,
            run_id: run_id.to_owned(),
            source: source.to_owned(),
            created_unix: now_unix,
            breakpoints,
            pages,
            ..Self::default()
        }
    }

    /// The shots still to take, page by page in breakpoint order: every pending one, and with
    /// `retry` every failed one with fewer than `max_attempts` attempts.
    #[must_use]
    pub fn tasks(&self, retry: bool, max_attempts: u32) -> Vec<Task> {
        let mut tasks = Vec::new();
        for (index, page) in self.pages.iter().enumerate() {
            for breakpoint in &self.breakpoints {
                let wanted = page.shots.get(&breakpoint.name).is_none_or(|shot| {
                    shot.state == ShotState::Pending
                        || (retry
                            && shot.state == ShotState::Failed
                            && shot.attempts < max_attempts)
                });
                if wanted {
                    tasks.push(Task {
                        page: index,
                        breakpoint: breakpoint.name.clone(),
                    });
                }
            }
        }
        tasks
    }

    /// Takes the outcome of one attempt at `task`; a task of no page is ignored.
    pub fn record(&mut self, task: &Task, outcome: Outcome) {
        let Some(page) = self.pages.get_mut(task.page) else {
            return;
        };
        if outcome.status.is_some() {
            page.status = outcome.status;
        }
        if outcome.final_url.is_some() {
            page.final_url = outcome.final_url;
        }
        let shot = page.shots.entry(task.breakpoint.clone()).or_default();
        shot.attempts = shot.attempts.saturating_add(1);
        shot.ms = outcome.ms;
        match outcome.result {
            Ok(file) => {
                shot.state = ShotState::Done;
                shot.file = Some(file);
                shot.error = None;
            }
            Err(error) => {
                shot.state = ShotState::Failed;
                shot.error = Some(error);
            }
        }
    }

    /// How far the run is; the rest at `parallel` shots at once.
    #[must_use]
    pub fn progress(&self, parallel: usize) -> Progress {
        let mut progress = Progress {
            pages: self.pages.len(),
            breakpoints: self.breakpoints.len(),
            total: self.pages.len() * self.breakpoints.len(),
            ..Progress::default()
        };
        let mut spent = 0u64;
        let mut timed = 0u64;
        for shot in self.pages.iter().flat_map(|page| page.shots.values()) {
            match shot.state {
                ShotState::Done => progress.done += 1,
                ShotState::Failed => progress.failed += 1,
                ShotState::Pending => {}
            }
            if shot.state != ShotState::Pending {
                spent = spent.saturating_add(shot.ms);
                timed += 1;
            }
        }
        progress.pending = progress.total - progress.done - progress.failed;
        if let Some(mean) = spent.checked_div(timed) {
            let lanes = u64::try_from(parallel.max(1)).unwrap_or(1);
            let rest = u64::try_from(progress.pending).unwrap_or(u64::MAX);
            progress.eta_seconds = Some(rest.saturating_mul(mean) / lanes / 1000);
        }
        progress
    }
}

#[cfg(test)]
#[path = "sitemap_tests.rs"]
mod tests;
