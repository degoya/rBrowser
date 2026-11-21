//! The record of one engine comparison, `report.json`: the environment comparison's record
//! ([`EnvRun`], RB-080-03) with the engines as its environments - one breakpoint, one page -,
//! the page's URL and the engines left out and why. [`EngineSummary`] is the answer of
//! `compare_engines`: per engine its shot, per engine pair the share of differing pixels, the
//! diff image and the differing areas, every file as a path. `report.html` is the environment
//! comparison's page (side by side, slider, difference, areas) with the engines left out above.

use std::fmt::Write as _;
use std::path::Path;

use rb_config::EngineKind;
use rb_visual::regions::Region;
use rb_visual::sitemap::{RunBreakpoint, RunState};
use serde::{Deserialize, Serialize};

use super::plan::SkippedEngine;
use crate::compare::report::{EnvRun, PairStatus, RunEnvironment, RunPath};

/// The runs' folder below the project root.
pub const OUT_DIR: [&str; 3] = [".rbrowser", "out", "engine-compare"];
/// The folder of the page's images in a run's folder.
pub const PAGE_FOLDER: &str = "page";

/// `report.json` of an engine comparison.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineRun {
    /// The shots and diffs, the engines as environments.
    #[serde(flatten)]
    pub run: EnvRun,
    /// The page compared.
    pub url: String,
    /// The engines asked for that cannot run, with their reason.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<SkippedEngine>,
}

impl EngineRun {
    /// A pending run of `url` at `breakpoint` in `engines` (in their order), every shot and
    /// every pair's diff pending.
    pub fn new(
        url: &str,
        breakpoint: RunBreakpoint,
        engines: &[EngineKind],
        skipped: Vec<SkippedEngine>,
        max_ratio: f64,
    ) -> Self {
        let mut run = EnvRun {
            max_ratio,
            environments: engines
                .iter()
                .map(|kind| RunEnvironment {
                    id: kind.name().to_owned(),
                    url: url.to_owned(),
                    color: None,
                })
                .collect(),
            breakpoints: vec![breakpoint],
            paths: vec![RunPath {
                path: url.to_owned(),
                folder: PAGE_FOLDER.to_owned(),
            }],
            ..EnvRun::default()
        };
        run.fill(&crate::compare::plan::pairs(engines.len()), |base, _| {
            base.to_owned()
        });
        Self {
            run,
            url: url.to_owned(),
            skipped,
        }
    }

    /// The answer of `compare_engines`; `folder` is the run's folder, every file below it.
    pub fn summary(&self, folder: &str, report: Option<String>) -> EngineSummary {
        let path = |file: &Option<String>| {
            file.as_ref()
                .map(|file| Path::new(folder).join(file).display().to_string())
        };
        let engine = |index: usize| {
            self.run
                .environments
                .get(index)
                .map(|environment| environment.id.clone())
                .unwrap_or_default()
        };
        let pairs: Vec<EnginePair> = self
            .run
            .diffs
            .iter()
            .map(|diff| EnginePair {
                a: engine(diff.a),
                b: engine(diff.b),
                status: diff.status,
                percent: (diff.ratio * 10_000.0).round() / 100.0,
                different_pixels: diff.different_pixels,
                image: path(&diff.image),
                regions: diff.regions.clone(),
                error: diff.error.clone(),
            })
            .collect();
        EngineSummary {
            run_id: self.run.run_id.clone(),
            state: self.run.state,
            url: self.url.clone(),
            breakpoint: self
                .run
                .breakpoints
                .first()
                .map(|breakpoint| breakpoint.label.clone())
                .unwrap_or_default(),
            threshold: self.run.threshold,
            folder: folder.to_owned(),
            report,
            shots: self
                .run
                .shots
                .iter()
                .map(|shot| EngineShot {
                    engine: engine(shot.environment),
                    file: path(&shot.file),
                    error: shot.error.clone(),
                })
                .collect(),
            skipped: self.skipped.clone(),
            differing: pairs
                .iter()
                .filter(|pair| {
                    matches!(pair.status, PairStatus::Differs | PairStatus::SizeMismatch)
                })
                .count(),
            pairs,
        }
    }
}

/// The answer of `compare_engines`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineSummary {
    pub run_id: String,
    pub state: RunState,
    pub url: String,
    /// The breakpoint's label.
    pub breakpoint: String,
    /// The colour tolerance of the diff.
    pub threshold: f64,
    /// The run's folder.
    pub folder: String,
    /// `report.html`, once written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
    /// Each engine's shot.
    pub shots: Vec<EngineShot>,
    /// The engines left out, with their reason.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<SkippedEngine>,
    /// The pairs that differ or differ in size.
    pub differing: usize,
    pub pairs: Vec<EnginePair>,
}

/// One engine's shot in the answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineShot {
    pub engine: String,
    /// The PNG's path, once taken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One engine pair in the answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnginePair {
    pub a: String,
    pub b: String,
    pub status: PairStatus,
    /// The share of differing pixels in percent, two decimals.
    pub percent: f64,
    pub different_pixels: u64,
    /// The diff image's path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// The differing areas in pixels of the shots, largest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<Region>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `report.html` of `run`: the environment comparison's page, the engines left out above its
/// matrix. `text` gives the text of a key of `crate::compare::html::TEXT_KEYS` and `skipped`.
pub fn render_html(run: &EngineRun, text: &dyn Fn(&str) -> String) -> String {
    let page = crate::compare::html::render(&run.run, text);
    if run.skipped.is_empty() {
        return page;
    }
    let escape = rb_plugin_types::html::escape;
    let mut list = format!("<p>{}</p><ul>", escape(&text("skipped")));
    for skipped in &run.skipped {
        let _ = write!(
            list,
            "<li>{}: {}</li>",
            escape(&skipped.engine),
            escape(&skipped.reason)
        );
    }
    list.push_str("</ul>");
    match page.find("</h1>") {
        Some(at) => {
            let at = at + "</h1>".len();
            format!("{}{list}{}", &page[..at], &page[at..])
        }
        None => page,
    }
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
