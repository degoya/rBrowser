//! The breakpoint analyser (RB-010-13), rBrowser's first first-party plugin and the reference
//! implementation of the plugin contract (`docs/plugins.md`).
//!
//! The host loads the site in a hidden view, reads every stylesheet over CDP and hands their
//! text to the plugin; the plugin reads the `@media` boundaries ([`parser`]), merges them and
//! recognises the framework ([`suggest`]), adopts the breakpoints the project's Tailwind,
//! Bootstrap, SCSS and Less sources declare ([`static_source`], RB-030-07), proposes views in the
//! configured mode ([`modes`]) and works out the config a choice of them makes ([`apply`]). It
//! writes nothing: the host writes what a person chose. Its one capability, `project-read`,
//! names the project files the host may hand over for the static source; the host reads them,
//! confined to the project, and passes their text in the arguments.
//!
//! Two worlds: `analyzer.analyze` answers proposals (`/breakpoints/-`, one per new view), and
//! the `feature` export serves the MCP tools `breakpoints_detect` (`detect`) and
//! `breakpoints_apply` (`apply`). Their arguments and answers are JSON:
//!
//! - `detect` / `analyze`: `{ sheets: [{ url, text, media? }], files?: [{ path, text }],
//!   rootFontSizePx?, mode?, edgeViews?, mergeTolerancePx?, existing?: [{ id, width }] }`;
//!   `detect` answers `{ detected, sources, breakpoints, containerQueries, framework, hints? }`
//!   in the shapes of `rb_ipc::BreakpointProposal` (a detected boundary with `source` and
//!   `heuristic` when the project declares it; `sources` lists the project files after the
//!   stylesheets).
//! - `apply`: [`apply::ApplyArgs`], answers [`apply::Applied`].
//!
//! The logic is tested natively against `rb_plugin_api::test_support::MemoryHost`; `guest.rs`
//! translates it into the WIT vocabulary and exists on `wasm32` alone.

pub mod apply;
#[cfg(target_arch = "wasm32")]
mod guest;
pub mod modes;
pub mod parser;
pub mod static_source;
pub mod suggest;

use rb_plugin_api::analyzer::Analyzer;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, Proposal, codes};
use serde::{Deserialize, Serialize};
use serde_json::json;

use apply::ProposedBreakpoint;
use modes::{Mode, Reason, View};
use parser::Side;
use static_source::{Hint, ProjectFile};

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "breakpoint-analyzer";
/// The default of `analyzer.mergeTolerancePx`.
pub const DEFAULT_TOLERANCE_PX: u32 = 8;
/// The most px two boundaries may be apart and still merge.
const MAX_TOLERANCE_PX: u32 = 200;
/// The root font size `em` and `rem` use when the page reports none.
const DEFAULT_ROOT_FONT_SIZE: f64 = 16.0;

/// One stylesheet the host read.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Sheet {
    /// Its URL (inline sheets: the page URL with `#inline-<n>`).
    pub url: String,
    /// Its text.
    pub text: String,
    /// The media list of its `<link>` or `<style>`.
    pub media: Option<String>,
}

/// A breakpoint the project has; only the id and the width count.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Existing {
    pub id: String,
    pub width: u32,
}

/// The arguments of `detect` and `analyze`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DetectArgs {
    pub sheets: Vec<Sheet>,
    pub root_font_size_px: Option<f64>,
    pub mode: Mode,
    pub edge_views: bool,
    pub merge_tolerance_px: Option<u32>,
    pub existing: Vec<Existing>,
    /// The project files of the static source (RB-030-07); none when `analyzer.static` is off.
    pub files: Vec<ProjectFile>,
}

/// One merged boundary, as `rb_ipc::DetectedBreakpoint`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Detected {
    pub width: u32,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub count: u32,
    /// The project file that declares it and its name there (`tailwind.config.js: md`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Declared only by a variable's name: the weaker source.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub heuristic: bool,
}

/// One stylesheet or project file read, as `rb_ipc::StylesheetSource`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Source {
    pub url: String,
    pub count: u32,
}

/// What `detect` answers.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detection {
    pub detected: Vec<Detected>,
    pub sources: Vec<Source>,
    pub breakpoints: Vec<ProposedBreakpoint>,
    pub container_queries: u32,
    pub framework: Option<&'static str>,
    /// What the static source could not read.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hints: Vec<Hint>,
}

/// The analysis behind both answers: the detection and the views with their reasons.
#[must_use]
pub fn detect(args: &DetectArgs) -> (Detection, Vec<View>) {
    let root = args
        .root_font_size_px
        .filter(|size| (1.0..=200.0).contains(size))
        .unwrap_or(DEFAULT_ROOT_FONT_SIZE);
    let tolerance = args
        .merge_tolerance_px
        .unwrap_or(DEFAULT_TOLERANCE_PX)
        .min(MAX_TOLERANCE_PX);
    let mut boundaries = Vec::new();
    let mut sources = Vec::new();
    let mut container_queries = 0;
    for sheet in &args.sheets {
        let mut scanned = parser::scan(&sheet.text, root);
        if let Some(media) = &sheet.media {
            let list = parser::scan_media_list(media, root);
            scanned.queries += list.queries;
            scanned.boundaries.extend(list.boundaries);
        }
        container_queries += scanned.containers;
        boundaries.extend(scanned.boundaries);
        sources.push(Source {
            url: sheet.url.clone(),
            count: scanned.queries,
        });
    }
    let declared = static_source::scan(&args.files, root);
    let mut clusters = suggest::merge(&boundaries, tolerance);
    let recognised = suggest::recognise(&clusters);
    suggest::label(&mut clusters, recognised);
    suggest::adopt(&mut clusters, &declared.declared, tolerance);
    let framework = recognised.or_else(|| {
        declared
            .declared
            .iter()
            .find(|item| !item.heuristic)
            .and_then(|item| item.framework)
    });
    sources.extend(declared.sources);
    let views = modes::views(&clusters, args.mode, args.edge_views);
    let detected = clusters
        .iter()
        .map(|cluster| Detected {
            width: cluster.query_width(),
            kind: match cluster.side {
                Side::Min => "min",
                Side::Max => "max",
            },
            label: cluster.label.clone(),
            count: cluster.count,
            source: cluster.source.clone(),
            heuristic: cluster.heuristic,
        })
        .collect();
    let breakpoints = views
        .iter()
        .map(|view| ProposedBreakpoint {
            id: view.id.clone(),
            label: view.label.clone(),
            width: view.width,
            height: view.height,
            is_new: !args.existing.iter().any(|known| known.width == view.width),
        })
        .collect();
    let detection = Detection {
        detected,
        sources,
        breakpoints,
        container_queries,
        framework: framework.map(suggest::Framework::id),
        hints: declared.hints,
    };
    (detection, views)
}

/// The full key of one of the plugin's texts.
fn key(rest: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{rest}")
}

fn bad_args(error: &serde_json::Error) -> Failure {
    Failure::coded(FailureKind::BadData, &key("errors.bad_args")).with_param("reason", error)
}

fn parse<T: for<'de> Deserialize<'de>>(args: &str) -> Result<T, Failure> {
    serde_json::from_str(args).map_err(|error| bad_args(&error))
}

/// The proposal of one new view.
fn proposal(view: &View) -> Proposal {
    let width = ("width".to_owned(), view.width.to_string());
    let (reason, params) = match &view.reason {
        Reason::Boundary { count } => {
            ("boundary", vec![width, ("count".into(), count.to_string())])
        }
        Reason::Mobile => ("mobile", vec![width]),
        Reason::Desktop => ("desktop", vec![width]),
        Reason::Edge { boundary } => (
            "edge",
            vec![width, ("boundary".into(), boundary.to_string())],
        ),
        Reason::Declared { source } => ("declared", vec![width, ("source".into(), source.clone())]),
        Reason::Group { from, to } => (
            "group",
            vec![
                width,
                ("from".into(), from.to_string()),
                ("to".into(), to.to_string()),
            ],
        ),
    };
    let value =
        json!({ "id": view.id, "label": view.label, "width": view.width, "height": view.height });
    Proposal {
        pointer: "/breakpoints/-".to_owned(),
        value: value.to_string(),
        reason: key(&format!("reasons.{reason}")),
        params,
    }
}

/// The plugin, natively: both worlds over [`detect`] and [`apply::apply`].
#[derive(Clone, Copy, Debug, Default)]
pub struct BreakpointAnalyzer;

impl Analyzer for BreakpointAnalyzer {
    fn analyze(&mut self, host: &mut dyn PluginHost, args: &str) -> Result<Vec<Proposal>, Failure> {
        let args: DetectArgs = parse(args)?;
        let (detection, views) = detect(&args);
        host.log(
            "debug",
            &format!(
                "{} boundaries in {} stylesheets and project files, {} static hints",
                detection.detected.len(),
                detection.sources.len(),
                detection.hints.len()
            ),
        );
        Ok(views
            .iter()
            .zip(&detection.breakpoints)
            .filter(|(_, proposed)| proposed.is_new)
            .map(|(view, _)| proposal(view))
            .collect())
    }
}

impl Feature for BreakpointAnalyzer {
    fn run_command(
        &mut self,
        _host: &mut dyn PluginHost,
        command: &str,
        _args: &str,
    ) -> Result<String, Failure> {
        Err(unknown(command))
    }

    fn call_tool(
        &mut self,
        _host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let answer = match tool {
            "detect" => serde_json::to_string(&detect(&parse::<DetectArgs>(args)?).0),
            "apply" => serde_json::to_string(&apply::apply(&parse(args)?)),
            _ => return Err(unknown(tool)),
        };
        answer.map_err(|error| bad_args(&error))
    }
}

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION).with_param("name", name)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
