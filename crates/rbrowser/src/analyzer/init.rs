//! `rbrowser init`'s proposal (contract 0.1-w1b, [`super::init_config`]): before any window
//! exists, the page and its stylesheets are fetched ([`super::fetch`]) and, with
//! `analyzer.static`, the project's Tailwind, Bootstrap, SCSS and Less sources read
//! ([`super::run::static_files`], RB-030-07); the bundled plugin proposes breakpoints in the
//! configured mode, and the config takes every proposed view and the grid the plugin builds for
//! them. The CLI (RB-010-11) writes it; when there is none, the reason says why (RB-091-08,
//! [`init_proposal_with`]) - on stderr and in the setup wizard.

use std::path::Path;

use rb_config::ProjectConfig;
use serde_json::json;

use super::fetch::{Fetch, collect};
use super::plugins::LoadedPlugins;
use super::run::{detect, pages, static_files, with_breakpoints};
use crate::cli::init::{Analysed, Analysis, NoProposal, Reason};

/// The config for the site at `url` (else the project's `startUrl`) of the project at `root`;
/// `Ok(None)` when there is no page to read or the stylesheets name no breakpoint.
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
    match init_proposal_with(root, url, plugins, fetch) {
        Ok(analysed) => Ok(Some(analysed.config)),
        Err(why) if matches!(why.reason, Reason::NoPage | Reason::NothingFound) => Ok(None),
        Err(why) => Err(why.detail),
    }
}

/// [`init_config_with`] with the stylesheets read, or why there is no proposal (RB-091-08): the
/// reason `rbrowser init` prints and the setup wizard names.
///
/// # Errors
///
/// [`NoProposal`]: no page, none reachable or in time, no stylesheet, the plugin missing or
/// failed, nothing found, or no valid config.
pub fn init_proposal_with(
    root: &Path,
    url: Option<&str>,
    plugins: &LoadedPlugins,
    fetch: &dyn Fetch,
) -> Analysis {
    let current = rb_config::load_project(root)
        .map(|loaded| loaded.config)
        .unwrap_or_default();
    let requested: Vec<String> = url.map(str::to_owned).into_iter().collect();
    let urls = pages(&current, &requested, true);
    let Some(first) = urls.first().cloned() else {
        return Err(NoProposal::new(
            Reason::NoPage,
            "no page to analyse: pass an http(s) address",
        ));
    };
    let collected = collect(fetch, &urls).map_err(|error| NoProposal::unreachable(&error))?;
    let sheets = collected.sheets.len();
    // A new config: every view is proposed, none is known yet.
    let fresh = ProjectConfig {
        analyzer: current.analyzer.clone(),
        ..ProjectConfig::default()
    };
    let files = static_files(plugins, &current, Some(root));
    let detection = detect(plugins, &fresh, &collected, &files, None)
        .map_err(|error| NoProposal::of(&error))?;
    let raw = json!({
        "version": rb_config::CURRENT_VERSION,
        "startUrl": first,
    });
    let Some((config, _)) = with_breakpoints(plugins, &raw, &detection.breakpoints, &[], true)
        .map_err(|error| NoProposal::of(&error))?
    else {
        return Err(NoProposal::new(
            Reason::NothingFound,
            "the stylesheets name no breakpoint",
        ));
    };
    let mut config: ProjectConfig = serde_json::from_value(config)
        .map_err(|error| NoProposal::new(Reason::Invalid, error.to_string()))?;
    config.analyzer = current.analyzer;
    Ok(Analysed { config, sheets })
}

#[cfg(test)]
#[path = "init_tests.rs"]
mod tests;
