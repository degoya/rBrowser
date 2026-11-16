//! The visual regression check (RB-050-04): the findings of a baseline comparison - "view X
//! differs by n %", a size mismatch, a missing baseline, an unreliable comparison.
//!
//! **Where the work happens.** The core captures every view, compares it with its baseline
//! pixel by pixel (`rb-visual`) and writes the diff images; the plugin gets the outcome as its
//! arguments and turns it into findings, the same split as the breakpoint analyser (the host
//! reads the stylesheets, the plugin reads the rules). Diffing several full-page images in
//! WebAssembly would be slow under the fuel limit, the images would have to cross the boundary
//! (8 MiB per call) and the contract would need a new host interface; and baselines written by
//! the core stay inside `.rbrowser/baselines` without granting a plugin any write. The plugin
//! needs no capability.
//!
//! The arguments (`check-context.args`), as the core's command `visual.compare` passes them:
//! `{ "visual": { "comparisons": [Comparison], "failOnMetaMismatch": bool } }`, a comparison
//! being `rb_visual::Comparison` (`view`, `label`, `breakpoint`, `slug`, `url`, `status`,
//! `differentPixels`, `ratio`, `regions`, sizes, `metaMismatches`, `metaMissing`, paths,
//! `error`). Without `visual` (the general check run `checks.run_all`) there is nothing to
//! report and the answer is empty.
//!
//! The logic is tested natively against `rb_plugin_api::test_support::MemoryHost`; `guest.rs`
//! translates it into the WIT vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

use rb_plugin_api::check::Check;
use rb_plugin_api::{CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, Severity};
use serde::Deserialize;
use serde_json::{Value, json};

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "visual-regression";
/// A view differs from its baseline.
pub const DIFFERS: &str = "differs";
/// A view's capture and baseline differ in size.
pub const SIZE_MISMATCH: &str = "size-mismatch";
/// A view has no baseline yet.
pub const NO_BASELINE: &str = "no-baseline";
/// The setup differs from the baseline's (DPI, engine version, viewport).
pub const UNRELIABLE: &str = "unreliable";
/// A view could not be captured or compared.
pub const FAILED: &str = "failed";

/// The rules: id, the key of the title, the usual severity.
const RULES: [(&str, &str, Severity); 5] = [
    (DIFFERS, "differs", Severity::Error),
    (SIZE_MISMATCH, "size_mismatch", Severity::Error),
    (NO_BASELINE, "no_baseline", Severity::Info),
    (UNRELIABLE, "unreliable", Severity::Warning),
    (FAILED, "failed", Severity::Warning),
];

/// One field of the setup that differs.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MetaMismatch {
    pub field: String,
    pub baseline: String,
    pub current: String,
}

/// A size in pixels.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// One view against its baseline, as the core reports it (`rb_visual::Comparison`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Comparison {
    pub view: u32,
    pub label: String,
    pub breakpoint: String,
    pub slug: String,
    pub url: String,
    /// `ok`, `differs`, `new`, `sizeMismatch` or `failed`.
    pub status: String,
    pub different_pixels: u64,
    pub ratio: f64,
    pub regions: Vec<Value>,
    pub baseline_size: Option<Size>,
    pub actual_size: Option<Size>,
    pub meta_mismatches: Vec<MetaMismatch>,
    pub meta_missing: bool,
    pub baseline_path: Option<String>,
    pub actual_path: Option<String>,
    pub diff_path: Option<String>,
    pub error: Option<String>,
}

/// The comparisons of one run.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct VisualRun {
    pub comparisons: Vec<Comparison>,
    /// An unreliable comparison is an error, not a warning (`visual.failOnMetaMismatch`).
    pub fail_on_meta_mismatch: bool,
}

/// The arguments of a run.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Args {
    pub visual: Option<VisualRun>,
}

/// The check.
pub struct VisualRegression;

fn key(group: &str, name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{group}.{name}")
}

fn bad_args(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, &key("errors", "bad_args")).with_param("reason", reason)
}

/// `size` as `<w>x<h>`.
fn size_text(size: Option<Size>) -> String {
    size.map_or_else(String::new, |size| {
        format!("{}x{}", size.width, size.height)
    })
}

/// The arguments in `args`; an empty text is none.
pub fn parse(args: &str) -> Result<Args, Failure> {
    if args.trim().is_empty() {
        return Ok(Args::default());
    }
    serde_json::from_str(args).map_err(|error| bad_args(&error.to_string()))
}

impl Comparison {
    fn finding(&self, rule: &str, severity: Severity, params: Vec<(&str, String)>) -> Finding {
        let mut all = vec![
            ("label", self.label.clone()),
            ("breakpoint", self.breakpoint.clone()),
        ];
        all.extend(params);
        let data = json!({
            "slug": self.slug,
            "ratio": self.ratio,
            "differentPixels": self.different_pixels,
            "regions": self.regions,
            "baselinePath": self.baseline_path,
            "actualPath": self.actual_path,
            "diffPath": self.diff_path,
        });
        Finding {
            rule: rule.to_owned(),
            severity,
            message: key("findings", &rule.replace('-', "_")),
            params: all
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value))
                .collect(),
            view: Some(self.view),
            url: (!self.url.is_empty()).then(|| self.url.clone()),
            selector: None,
            // One finding per page and breakpoint: the same view in the next run has the same id.
            path: Some(format!("visual/{}/{}", self.slug, self.breakpoint)),
            data: Some(data.to_string()),
        }
    }

    /// The findings of this comparison under the rules `enabled`.
    fn findings(&self, enabled: &[String], fail_on_meta_mismatch: bool) -> Vec<Finding> {
        let on = |rule: &str| enabled.iter().any(|id| id == rule);
        let mut found = Vec::new();
        match self.status.as_str() {
            "differs" if on(DIFFERS) => found.push(self.finding(
                DIFFERS,
                Severity::Error,
                vec![
                    ("percent", format!("{:.2}", self.ratio * 100.0)),
                    ("pixels", self.different_pixels.to_string()),
                    ("areas", self.regions.len().to_string()),
                ],
            )),
            "sizeMismatch" if on(SIZE_MISMATCH) => found.push(self.finding(
                SIZE_MISMATCH,
                Severity::Error,
                vec![
                    ("baseline", size_text(self.baseline_size)),
                    ("actual", size_text(self.actual_size)),
                ],
            )),
            "new" if on(NO_BASELINE) => {
                found.push(self.finding(NO_BASELINE, Severity::Info, Vec::new()));
            }
            "failed" if on(FAILED) => found.push(self.finding(
                FAILED,
                Severity::Warning,
                vec![("error", self.error.clone().unwrap_or_default())],
            )),
            _ => {}
        }
        let unreliable = self.meta_missing || !self.meta_mismatches.is_empty();
        if unreliable && on(UNRELIABLE) {
            let fields = if self.meta_missing {
                "meta.json".to_owned()
            } else {
                self.meta_mismatches
                    .iter()
                    .map(|mismatch| {
                        format!(
                            "{} ({} / {})",
                            mismatch.field, mismatch.baseline, mismatch.current
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let severity = if fail_on_meta_mismatch {
                Severity::Error
            } else {
                Severity::Warning
            };
            found.push(self.finding(UNRELIABLE, severity, vec![("fields", fields)]));
        }
        found
    }
}

impl Check for VisualRegression {
    fn describe(&self) -> Vec<RuleInfo> {
        RULES
            .iter()
            .map(|(id, title, severity)| RuleInfo {
                id: (*id).to_owned(),
                title: key("rules", title),
                severity: *severity,
                shared_id: None,
                help_url: None,
            })
            .collect()
    }

    fn run(
        &mut self,
        _host: &mut dyn PluginHost,
        ctx: &CheckContext,
    ) -> Result<Vec<Finding>, Failure> {
        let Some(run) = parse(&ctx.args)?.visual else {
            return Ok(Vec::new());
        };
        Ok(run
            .comparisons
            .iter()
            .filter(|comparison| ctx.view.is_none_or(|view| view == comparison.view))
            .flat_map(|comparison| comparison.findings(&ctx.rules, run.fail_on_meta_mismatch))
            .collect())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
