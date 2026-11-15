//! The analysis without the window: the plugin's arguments from the config, the collected
//! stylesheets and the static source's project files ([`static_files`]), its answer as the
//! wizard's proposal, and writing a choice of it into `.rbrowser/config.json` - only the raw
//! `breakpoints` and `layout` of that file change, so `config.local.json` never leaks into it and
//! every other key stays as it was.

use std::path::Path;

use rb_config::{AnalyzerMode, Breakpoint, ConfigFile, ProjectConfig};
use rb_ipc::{AnalyzerHint, DetectedBreakpoint, ProposedBreakpoint, StylesheetSource};
use serde::Deserialize;
use serde_json::{Value, json};

use super::plugins::{LoadedPlugins, PluginError};
use super::project_files::{self, ProjectFile};
use super::source::Collected;

/// The plugin's id.
pub const PLUGIN_ID: &str = "breakpoint-analyzer";
/// The most pages one analysis loads.
pub const MAX_URLS: usize = 10;

/// Why an analysis or a write did not happen; the message keys are `analyzer.error.*` (or the
/// plugin's code), the text is English for the log.
#[derive(Debug, thiserror::Error)]
pub enum AnalyzerError {
    /// Neither `startUrl`, `analyzer.urls` nor the request names an `http(s)` page.
    #[error("no page to analyse: set startUrl or analyzer.urls")]
    NoUrl,
    /// No project is open, so there is no config to write.
    #[error("no project is open")]
    NoProject,
    /// Another analysis runs.
    #[error("an analysis is already running")]
    Busy,
    /// `applyBreakpoints` names a proposal that is not the last one.
    #[error("the proposal is no longer current")]
    NoProposal,
    /// No stylesheet could be read.
    #[error("the pages could not be read: {0}")]
    Collect(String),
    /// The plugin is not loaded or failed.
    #[error("the breakpoint analyser failed: {0}")]
    Plugin(PluginError),
    /// The config could not be read, was invalid after the change, or not written.
    #[error("the config was not written: {0}")]
    Config(String),
}

impl AnalyzerError {
    /// The i18n key of the message, and its parameters.
    pub fn message(&self) -> (String, Vec<(String, String)>) {
        let detail = |text: &String| vec![("detail".to_owned(), text.clone())];
        let (key, params) = match self {
            Self::NoUrl => ("analyzer.error.no_url", Vec::new()),
            Self::NoProject => ("analyzer.error.no_project", Vec::new()),
            Self::Busy => ("analyzer.error.busy", Vec::new()),
            Self::NoProposal => ("analyzer.error.no_proposal", Vec::new()),
            Self::Collect(text) => ("analyzer.error.collect", detail(text)),
            Self::Config(text) => ("analyzer.error.config", detail(text)),
            Self::Plugin(error) => return (error.code.clone(), error.params.clone()),
        };
        (key.to_owned(), params)
    }
}

/// The pages of an analysis: the request's, then `startUrl`, then `analyzer.urls`; `http(s)`,
/// each once, at most [`MAX_URLS`]. A `file:` page (a static site on disk) counts only among
/// `requested` and only with `local_files` - `rbrowser init --url file:///...`, the person's own
/// command line; never from the checked-in config, never in the app's hidden targets.
pub fn pages(config: &ProjectConfig, requested: &[String], local_files: bool) -> Vec<String> {
    let mut pages: Vec<String> = Vec::new();
    let named = requested.iter().map(|url| (url, local_files));
    let configured = config
        .start_url
        .iter()
        .chain(config.analyzer.urls.iter())
        .map(|url| (url, false));
    for (url, files) in named.chain(configured) {
        let url = url.trim();
        let lower = url.to_ascii_lowercase();
        let web = lower.starts_with("http://") || lower.starts_with("https://");
        let file = files && lower.starts_with("file://");
        if (web || file) && !pages.iter().any(|known| known == url) && pages.len() < MAX_URLS {
            pages.push(url.to_owned());
        }
    }
    pages
}

/// What `detect` answered, in the wizard's shapes.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Detection {
    pub detected: Vec<DetectedBreakpoint>,
    pub sources: Vec<StylesheetSource>,
    pub breakpoints: Vec<ProposedBreakpoint>,
    /// What the static source could not read (RB-030-07).
    #[serde(default)]
    pub hints: Vec<AnalyzerHint>,
}

/// The project files of the static source (RB-030-07) for the project at `root`: none when
/// `analyzer.static` is off, there is no project, or the plugin is not loaded or declares no
/// `project-read` globs - whose grant bounds `analyzer.staticGlobs`. Blocking; call it off the
/// UI thread.
pub fn static_files(
    plugins: &LoadedPlugins,
    config: &ProjectConfig,
    root: Option<&Path>,
) -> Vec<ProjectFile> {
    let (Some(root), Some(grants)) = (root, plugins.grants(PLUGIN_ID)) else {
        return Vec::new();
    };
    if !config.analyzer.static_scan {
        return Vec::new();
    }
    let files = project_files::collect(root, &config.analyzer.static_globs, &|path| {
        grants.glob_matches(path)
    });
    tracing::debug!(
        files = files.len(),
        "the static source read the project files"
    );
    files
}

/// Runs the plugin's `detect` over `collected` and the static source's `files` with the config's
/// analyser settings.
///
/// # Errors
///
/// Neither a stylesheet nor a project file was read, or the plugin failed.
pub fn detect(
    plugins: &LoadedPlugins,
    config: &ProjectConfig,
    collected: &Collected,
    files: &[ProjectFile],
    mode: Option<AnalyzerMode>,
) -> Result<Detection, AnalyzerError> {
    if collected.sheets.is_empty() && files.is_empty() {
        return Err(AnalyzerError::Collect("no stylesheet was found".to_owned()));
    }
    // A breakpoint that takes its width from its device preset counts with that width
    // (RB-010-12); one without any is left out.
    let devices = crate::emulation::device_list();
    let existing: Vec<Value> = config
        .breakpoints
        .iter()
        .filter_map(|breakpoint| {
            let viewport = breakpoint.viewport(&devices)?;
            Some(json!({ "id": breakpoint.id, "width": viewport.width }))
        })
        .collect();
    let args = json!({
        "sheets": collected.sheets,
        "rootFontSizePx": collected.root_font_size_px,
        "mode": mode.unwrap_or(config.analyzer.mode),
        "edgeViews": config.analyzer.edge_views,
        "mergeTolerancePx": config.analyzer.merge_tolerance_px,
        "existing": existing,
        "files": files,
    });
    let answer = plugins
        .call_tool(PLUGIN_ID, "detect", &args)
        .map_err(AnalyzerError::Plugin)?;
    let detection: Detection = serde_json::from_value(answer).map_err(|error| {
        AnalyzerError::Plugin(PluginError {
            code: "plugins.error.bad_output".to_owned(),
            params: vec![("reason".to_owned(), error.to_string())],
        })
    })?;
    for hint in &detection.hints {
        tracing::info!(path = %hint.path, key = %hint.key, "the static source left a value out");
    }
    Ok(detection)
}

/// The plugin's `apply` over `breakpoints` and `layout` (raw config JSON): the new ones and the
/// ids added.
fn applied(
    plugins: &LoadedPlugins,
    raw: &Value,
    proposed: &[ProposedBreakpoint],
    selected: &[String],
    replace: bool,
) -> Result<(Value, Value, Vec<String>), AnalyzerError> {
    let existing = raw.get("breakpoints").cloned().unwrap_or_else(|| json!([]));
    let args = json!({
        "existingWidths": resolved_widths(&existing),
        "existing": existing,
        "layout": raw.get("layout").cloned().unwrap_or(Value::Null),
        "proposed": proposed,
        "selected": selected,
        "replace": replace,
    });
    let answer = plugins
        .call_tool(PLUGIN_ID, "apply", &args)
        .map_err(AnalyzerError::Plugin)?;
    let added = answer["added"]
        .as_array()
        .map(|ids| {
            ids.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Ok((
        answer["breakpoints"].clone(),
        answer["layout"].clone(),
        added,
    ))
}

/// The widths of the raw `breakpoints`, each resolved against the device presets in effect: a
/// breakpoint that leaves out `width` counts with its preset's (RB-010-12), which the plugin
/// cannot know. One that does not parse or resolve is left out.
pub fn resolved_widths(breakpoints: &Value) -> Vec<u32> {
    let devices = crate::emulation::device_list();
    breakpoints
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|value| serde_json::from_value::<Breakpoint>(value.clone()).ok())
        .filter_map(|breakpoint| breakpoint.viewport(&devices))
        .map(|viewport| viewport.width)
        .collect()
}

/// `raw` with the chosen views of `proposed`, checked; `None` when nothing is added.
///
/// # Errors
///
/// The plugin failed, or the result is no valid config.
pub fn with_breakpoints(
    plugins: &LoadedPlugins,
    raw: &Value,
    proposed: &[ProposedBreakpoint],
    selected: &[String],
    replace: bool,
) -> Result<Option<(Value, Vec<String>)>, AnalyzerError> {
    let (breakpoints, layout, added) = applied(plugins, raw, proposed, selected, replace)?;
    if added.is_empty() {
        return Ok(None);
    }
    let mut changed = raw.clone();
    let object = changed
        .as_object_mut()
        .ok_or_else(|| AnalyzerError::Config("config.json is no JSON object".to_owned()))?;
    object.insert("breakpoints".to_owned(), breakpoints);
    if !layout.is_null() {
        object.insert("layout".to_owned(), layout);
    }
    let config: ProjectConfig = serde_json::from_value(changed.clone())
        .map_err(|error| AnalyzerError::Config(error.to_string()))?;
    if let Some(issue) = rb_config::check(&config).first() {
        return Err(AnalyzerError::Config(issue.message.clone()));
    }
    Ok(Some((changed, added)))
}

/// Writes the chosen views of `proposed` into the project at `root`; the ids added.
///
/// # Errors
///
/// As [`with_breakpoints`], or the file could not be read or written.
pub fn write(
    plugins: &LoadedPlugins,
    root: &Path,
    proposed: &[ProposedBreakpoint],
    selected: &[String],
    replace: bool,
) -> Result<Vec<String>, AnalyzerError> {
    let path = ConfigFile::Project.path(root);
    let raw = if path.is_file() {
        rb_config::read_json(&path).map_err(|error| AnalyzerError::Config(error.to_string()))?
    } else {
        json!({ "version": rb_config::CURRENT_VERSION })
    };
    let Some((changed, added)) = with_breakpoints(plugins, &raw, proposed, selected, replace)?
    else {
        return Ok(Vec::new());
    };
    rb_config::write_config_file(root, ConfigFile::Project, &changed)
        .map_err(|error| AnalyzerError::Config(error.to_string()))?;
    Ok(added)
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
