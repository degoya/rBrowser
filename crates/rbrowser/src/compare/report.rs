//! The record of one comparison, `report.json`: its environments, breakpoints and paths, each
//! shot (environment x breakpoint x path) and each diff (environment pair x breakpoint x path)
//! with its share of differing pixels. [`diff_group`] diffs the shots of one breakpoint and path
//! across every pair - pure, run on a blocking thread. The answer of `env_compare_result` is
//! [`Summary`].

use rb_visual::sitemap::{RunBreakpoint, RunState};
use rb_visual::{DiffOptions, DiffOutcome, decode_png, diff, encode_png};
use serde::{Deserialize, Serialize};

/// The record's file in the run's folder.
pub const REPORT_JSON: &str = "report.json";
/// The report page in the run's folder.
pub const REPORT_HTML: &str = "report.html";

/// One environment of a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEnvironment {
    pub id: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// One path of a run and the folder of its images.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPath {
    pub path: String,
    pub folder: String,
}

/// One shot: indices into the run's lists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shot {
    pub path: usize,
    pub environment: usize,
    pub breakpoint: usize,
    pub url: String,
    /// The PNG, relative to the run's folder, once taken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Why it was not taken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The page's HTTP status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
}

impl Shot {
    /// Whether the shot was taken or failed for good.
    pub fn done(&self) -> bool {
        self.file.is_some() || self.error.is_some()
    }
}

/// What the diff of a pair found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PairStatus {
    /// Not diffed yet.
    #[default]
    Pending,
    /// Within the tolerance.
    Same,
    /// More pixels differ than `envCompare.maxDiffPercent` allows.
    Differs,
    /// The shots differ in size: nothing compared.
    SizeMismatch,
    /// A shot is missing or could not be read.
    Failed,
}

/// One pair of environments in one breakpoint and path.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairDiff {
    pub path: usize,
    /// The first environment of the pair.
    pub a: usize,
    /// The second.
    pub b: usize,
    pub breakpoint: usize,
    pub status: PairStatus,
    /// The share of differing pixels, 0 to 1 (1 for a size mismatch).
    pub ratio: f64,
    pub different_pixels: u64,
    pub total_pixels: u64,
    /// The diff image, relative to the run's folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `report.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvRun {
    pub run_id: String,
    pub created_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_unix: Option<u64>,
    pub state: RunState,
    /// The colour tolerance of the diff.
    pub threshold: f64,
    /// The share of differing pixels (0 to 1) up to which a pair counts as the same.
    pub max_ratio: f64,
    pub full_page: bool,
    pub environments: Vec<RunEnvironment>,
    pub breakpoints: Vec<RunBreakpoint>,
    pub paths: Vec<RunPath>,
    pub shots: Vec<Shot>,
    pub diffs: Vec<PairDiff>,
}

impl EnvRun {
    /// The index of the shot of `path`, `environment` and `breakpoint` in [`Self::shots`].
    pub fn shot_index(&self, path: usize, environment: usize, breakpoint: usize) -> usize {
        (path * self.environments.len() + environment) * self.breakpoints.len() + breakpoint
    }

    /// Every shot (pending) and every pair's diff (pending), in index order; `url` gives the
    /// URL of a path under an environment.
    pub fn fill(&mut self, pairs: &[(usize, usize)], url: impl Fn(&str, &str) -> String) {
        self.shots.clear();
        self.diffs.clear();
        for (p, path) in self.paths.iter().enumerate() {
            for (e, environment) in self.environments.iter().enumerate() {
                for b in 0..self.breakpoints.len() {
                    self.shots.push(Shot {
                        path: p,
                        environment: e,
                        breakpoint: b,
                        url: url(&environment.url, &path.path),
                        ..Shot::default()
                    });
                }
            }
            for b in 0..self.breakpoints.len() {
                for &(a, other) in pairs {
                    self.diffs.push(PairDiff {
                        path: p,
                        a,
                        b: other,
                        breakpoint: b,
                        ..PairDiff::default()
                    });
                }
            }
        }
    }

    /// The summary `env_compare_result` answers.
    pub fn summary(&self, folder: &str, report: Option<String>) -> Summary {
        let shots_done = self.shots.iter().filter(|shot| shot.done()).count();
        let count = |status: PairStatus| self.diffs.iter().filter(|d| d.status == status).count();
        let name = |index: usize, list: &[RunEnvironment]| {
            list.get(index).map(|e| e.id.clone()).unwrap_or_default()
        };
        Summary {
            run_id: self.run_id.clone(),
            state: self.state,
            shots: self.shots.len(),
            shots_done,
            shots_failed: self.shots.iter().filter(|s| s.error.is_some()).count(),
            differing: count(PairStatus::Differs) + count(PairStatus::SizeMismatch),
            folder: folder.to_owned(),
            report,
            pairs: self
                .diffs
                .iter()
                .map(|d| SummaryPair {
                    path: self
                        .paths
                        .get(d.path)
                        .map(|p| p.path.clone())
                        .unwrap_or_default(),
                    a: name(d.a, &self.environments),
                    b: name(d.b, &self.environments),
                    breakpoint: self
                        .breakpoints
                        .get(d.breakpoint)
                        .map(|b| b.name.clone())
                        .unwrap_or_default(),
                    status: d.status,
                    percent: (d.ratio * 10_000.0).round() / 100.0,
                    image: d.image.clone(),
                    error: d.error.clone(),
                })
                .collect(),
        }
    }
}

/// The answer of `env_compare_result`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub run_id: String,
    pub state: RunState,
    pub shots: usize,
    pub shots_done: usize,
    pub shots_failed: usize,
    /// The pairs that differ or differ in size.
    pub differing: usize,
    pub folder: String,
    /// `report.html`, once written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
    pub pairs: Vec<SummaryPair>,
}

/// One pair in the answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryPair {
    pub path: String,
    pub a: String,
    pub b: String,
    pub breakpoint: String,
    pub status: PairStatus,
    /// The share of differing pixels in percent, two decimals.
    pub percent: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// What one pair's diff gave: the record's fields and the diff image to write.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupDiff {
    pub a: usize,
    pub b: usize,
    pub status: PairStatus,
    pub ratio: f64,
    pub different_pixels: u64,
    pub total_pixels: u64,
    pub png: Option<Vec<u8>>,
    pub error: Option<String>,
}

/// Diffs the shots of one breakpoint and path (`shots[environment]`, `None` for a failed one)
/// across `pairs`: a pair counts as differing when more than `max_ratio` of its pixels differ.
pub fn diff_group(
    shots: &[Option<Vec<u8>>],
    pairs: &[(usize, usize)],
    options: &DiffOptions,
    max_ratio: f64,
) -> Vec<GroupDiff> {
    let decoded: Vec<Option<Result<rb_visual::Rgba, String>>> = shots
        .iter()
        .map(|png| {
            png.as_deref()
                .map(|bytes| decode_png(bytes).map_err(|error| error.to_string()))
        })
        .collect();
    pairs
        .iter()
        .map(|&(a, b)| {
            let failed = |error: String| GroupDiff {
                a,
                b,
                status: PairStatus::Failed,
                ratio: 0.0,
                different_pixels: 0,
                total_pixels: 0,
                png: None,
                error: Some(error),
            };
            let (Some(Some(Ok(left))), Some(Some(Ok(right)))) = (decoded.get(a), decoded.get(b))
            else {
                return failed("a shot of the pair is missing".to_owned());
            };
            match diff(left, right, options) {
                Ok(DiffOutcome::Compared(pixels)) => {
                    let ratio = pixels.ratio();
                    let differs = pixels.different > 0 && ratio > max_ratio;
                    GroupDiff {
                        a,
                        b,
                        status: if differs {
                            PairStatus::Differs
                        } else {
                            PairStatus::Same
                        },
                        ratio,
                        different_pixels: pixels.different,
                        total_pixels: pixels.total,
                        png: (pixels.different > 0)
                            .then(|| encode_png(&pixels.image).ok())
                            .flatten(),
                        error: None,
                    }
                }
                Ok(DiffOutcome::SizeMismatch { baseline, actual }) => GroupDiff {
                    a,
                    b,
                    status: PairStatus::SizeMismatch,
                    ratio: 1.0,
                    different_pixels: 0,
                    total_pixels: 0,
                    png: None,
                    error: Some(format!(
                        "{}x{} against {}x{}",
                        baseline.width, baseline.height, actual.width, actual.height
                    )),
                },
                Err(error) => failed(error.to_string()),
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
