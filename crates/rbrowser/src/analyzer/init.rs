//! `rbrowser init`'s proposal (contract 0.1-w1b, [`super::init_config`]): before any window
//! exists, the page and its stylesheets are fetched ([`super::fetch`]) and, with
//! `analyzer.static`, the project's Tailwind, Bootstrap, SCSS and Less sources read
//! ([`super::run::static_files`], RB-030-07); the bundled plugin proposes breakpoints in the
//! configured mode, and the config takes every proposed view and the grid the plugin builds for
//! them. The CLI (RB-010-11) writes it.

use std::path::Path;

use rb_config::ProjectConfig;
use serde_json::json;

use super::fetch::{Fetch, collect};
use super::plugins::LoadedPlugins;
use super::run::{AnalyzerError, detect, pages, static_files, with_breakpoints};

/// The config for the site at `url` (else the project's `startUrl`) of the project at `root`;
/// `Ok(None)` when there is no page to read.
///
/// # Errors
///
/// No page or stylesheet could be read, the plugin is not available or failed, or the result is
/// no valid config; the message is English, for the command line.
pub fn init_config_with(
    root: &Path,
    url: Option<&str>,
    plugins: &LoadedPlugins,
    fetch: &dyn Fetch,
) -> Result<Option<ProjectConfig>, String> {
    let current = rb_config::load_project(root)
        .map(|loaded| loaded.config)
        .unwrap_or_default();
    let requested: Vec<String> = url.map(str::to_owned).into_iter().collect();
    let urls = pages(&current, &requested, true);
    let Some(first) = urls.first().cloned() else {
        return Ok(None);
    };
    let collected =
        collect(fetch, &urls).map_err(|error| AnalyzerError::Collect(error).to_string())?;
    // A new config: every view is proposed, none is known yet.
    let fresh = ProjectConfig {
        analyzer: current.analyzer.clone(),
        ..ProjectConfig::default()
    };
    let files = static_files(plugins, &current, Some(root));
    let detection =
        detect(plugins, &fresh, &collected, &files, None).map_err(|error| error.to_string())?;
    let raw = json!({
        "version": rb_config::CURRENT_VERSION,
        "startUrl": first,
    });
    let Some((config, _)) = with_breakpoints(plugins, &raw, &detection.breakpoints, &[], true)
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let mut config: ProjectConfig =
        serde_json::from_value(config).map_err(|error| error.to_string())?;
    config.analyzer = current.analyzer;
    Ok(Some(config))
}

#[cfg(test)]
#[path = "init_tests.rs"]
mod tests;
