//! What the first start remembers (RB-091-05): `first-run.json` beside the user settings (portable
//! mode included), per person and never per project - whether the setup wizard opened by itself,
//! and the projects the "Set up project" hint was shown for. [`decide`] is the whole rule of the
//! start; the caller reads and writes the file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The file's name, beside `settings.json`.
pub const FILE_NAME: &str = "first-run.json";
/// The most projects the hint list keeps; the oldest go first.
pub const MAX_HINTED: usize = 200;

/// The remembered state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FirstRunState {
    /// The wizard opened by itself once (or this person used rBrowser before it existed).
    pub wizard_shown: bool,
    /// The projects (`crate::instance::file::key`) the hint was shown for, oldest first.
    pub hinted_projects: Vec<String>,
}

/// The file as it was read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Loaded {
    /// No file: nothing remembered yet.
    Missing,
    /// The file and what it says.
    Found(FirstRunState),
    /// A file that does not read: treated as "everything shown", never written over.
    Broken,
}

/// The open project at the start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectAtStart<'a> {
    /// Its instance key.
    pub key: &'a str,
    /// Whether it has `.rbrowser/config.json`.
    pub has_config: bool,
}

/// What the start knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartInput<'a> {
    /// `ui.firstRunWizard`.
    pub enabled: bool,
    /// The user settings file existed: this person used rBrowser before.
    pub settings_file_exists: bool,
    /// The open project, when there is one.
    pub project: Option<ProjectAtStart<'a>>,
}

/// What the start does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartAction {
    /// Open the wizard.
    OpenWizard,
    /// Say "Set up project" once in the status bar.
    ShowHint,
    /// Nothing.
    Nothing,
}

/// The start's action and the state to write afterwards (`None`: write nothing). Off, nothing
/// shows and nothing is written; a broken file shows nothing. The very first start - no state and
/// no user settings file - opens the wizard, which also covers the project it opens on; a person
/// with settings from before is no first start. A project without config is hinted once.
#[must_use]
pub fn decide(input: &StartInput<'_>, loaded: Loaded) -> (StartAction, Option<FirstRunState>) {
    if !input.enabled {
        return (StartAction::Nothing, None);
    }
    let (state, changed) = match loaded {
        Loaded::Broken => return (StartAction::Nothing, None),
        Loaded::Missing if input.settings_file_exists => (
            FirstRunState {
                wizard_shown: true,
                hinted_projects: Vec::new(),
            },
            true,
        ),
        Loaded::Missing => (FirstRunState::default(), true),
        Loaded::Found(state) => (state, false),
    };
    let unconfigured = input
        .project
        .filter(|project| !project.has_config)
        .map(|project| project.key);
    if !state.wizard_shown {
        let mut next = state;
        next.wizard_shown = true;
        if let Some(key) = unconfigured {
            remember(&mut next, key);
        }
        return (StartAction::OpenWizard, Some(next));
    }
    match unconfigured {
        Some(key) if !state.hinted_projects.iter().any(|hinted| hinted == key) => {
            let mut next = state;
            remember(&mut next, key);
            (StartAction::ShowHint, Some(next))
        }
        _ => (StartAction::Nothing, changed.then_some(state)),
    }
}

fn remember(state: &mut FirstRunState, key: &str) {
    if state.hinted_projects.iter().any(|hinted| hinted == key) {
        return;
    }
    state.hinted_projects.push(key.to_owned());
    let over = state.hinted_projects.len().saturating_sub(MAX_HINTED);
    state.hinted_projects.drain(..over);
}

/// The state file beside the user settings file at `settings`.
#[must_use]
pub fn path_for(settings: &Path) -> PathBuf {
    settings.with_file_name(FILE_NAME)
}

/// Reads the state file at `path`.
#[must_use]
pub fn load(path: &Path) -> Loaded {
    if !path.exists() {
        return Loaded::Missing;
    }
    rb_config::read_json(path)
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
        .map_or(Loaded::Broken, Loaded::Found)
}

/// Writes `state` to `path`, creating its folder (a first start has no settings folder yet).
///
/// # Errors
///
/// The folder or the file cannot be written.
pub fn save(path: &Path, state: &FirstRunState) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let value = serde_json::to_value(state).map_err(|error| error.to_string())?;
    rb_config::write_json(path, &value).map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
