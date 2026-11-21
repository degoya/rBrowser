//! The wizard's project step (RB-091-05): the config `rbrowser init` would write - its checks,
//! its analyser and its mobile + desktop fallback ([`crate::cli::init::init`]) - or one view per
//! chosen device preset, shown before anything is written; then exactly the shown config written
//! the way `init` writes it (`.rbrowser/config.json` and `.gitignore`, never over a config).
//! RB-091-08: the analyser template's proposal says how many stylesheets the analyser read, or
//! why it had none ([`SetupReason`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rb_config::ConfigFile;
use rb_ipc::{SetupReason, SetupSource, SetupTemplate};
use serde_json::{Value, json};

use crate::cli::init::{Analysis, InitError, NoProposal, Proposed, Reason, Source};

/// The folder does not exist.
pub const NOT_FOLDER_KEY: &str = "firstrun.project.error.not_folder";
/// The folder has a config already.
pub const EXISTS_KEY: &str = "firstrun.project.error.exists";
/// The address is not an http(s) URL.
pub const URL_KEY: &str = "firstrun.project.error.url";
/// The analyser was chosen without an address.
pub const NEEDS_URL_KEY: &str = "firstrun.project.error.needs_url";
/// The device template without a device.
pub const NO_DEVICES_KEY: &str = "firstrun.project.error.no_devices";
/// A device id the presets do not hold.
pub const UNKNOWN_DEVICE_KEY: &str = "firstrun.project.error.unknown_device";
/// The proposal does not pass the config's checks.
pub const INVALID_KEY: &str = "firstrun.project.error.invalid";
/// The proposal to write is not the last one shown.
pub const NO_PROPOSAL_KEY: &str = "firstrun.project.error.no_proposal";
/// The file could not be written.
pub const WRITE_KEY: &str = "firstrun.project.error.write";
/// The most device presets one setup takes.
pub const MAX_DEVICES: usize = 12;

/// A failure: the i18n key and its values.
pub type StepFailure = (&'static str, BTreeMap<String, String>);

fn failure(key: &'static str, args: &[(&str, String)]) -> StepFailure {
    (
        key,
        args.iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
    )
}

/// What the project step asks for.
#[derive(Clone, Copy, Debug)]
pub struct Preview<'a> {
    pub root: &'a Path,
    pub url: Option<&'a str>,
    pub template: SetupTemplate,
    pub devices: &'a [String],
}

/// What the step proposes: the config, where its views came from, and - for the analyser
/// template - the stylesheets it read or why it had no proposal.
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub config: Value,
    pub source: SetupSource,
    pub sheets: Option<u32>,
    pub reason: Option<SetupReason>,
}

/// The config the step proposes and where its views came from; nothing is written. `analyser`
/// is `rbrowser init`'s (`crate::analyzer::init_config`), asked only for the analyser template.
///
/// # Errors
///
/// The folder is missing or has a config, the address is no web URL, the analyser has no
/// address, a device is unknown or none is chosen, or the result fails the config's checks.
pub fn propose(
    preview: &Preview<'_>,
    analyser: impl FnOnce(&Path, Option<&str>) -> Analysis,
) -> Result<Proposal, StepFailure> {
    let root = preview.root;
    if !root.is_dir() {
        return Err(failure(
            NOT_FOLDER_KEY,
            &[("path", root.display().to_string())],
        ));
    }
    let devices = match preview.template {
        SetupTemplate::Devices => Some(device_views(preview.devices)?),
        _ => None,
    };
    if preview.template == SetupTemplate::Analyzer && preview.url.is_none() {
        return Err(failure(NEEDS_URL_KEY, &[]));
    }
    let mut proposed = None;
    let confirm = |config: &Value, how: &Proposed| {
        proposed = Some((config.clone(), how.clone()));
        false
    };
    let outcome = if preview.template == SetupTemplate::Analyzer {
        crate::cli::init::init(root, preview.url, analyser, confirm)
    } else {
        // Not asked: the template is the person's.
        let not_asked =
            |_: &Path, _: Option<&str>| Err(NoProposal::new(Reason::NoPage, "not asked"));
        crate::cli::init::init(root, preview.url, not_asked, confirm)
    };
    match outcome {
        Err(InitError::NotConfirmed) => {}
        Err(InitError::Exists(path)) => {
            return Err(failure(EXISTS_KEY, &[("path", path.display().to_string())]));
        }
        Err(InitError::Url(message)) => return Err(failure(URL_KEY, &[("message", message)])),
        Err(InitError::Write(error)) => {
            return Err(failure(WRITE_KEY, &[("message", error.to_string())]));
        }
        Ok(path) => {
            return Err(failure(
                WRITE_KEY,
                &[("message", path.display().to_string())],
            ));
        }
    }
    let (mut config, how) = proposed.ok_or_else(|| failure(NO_PROPOSAL_KEY, &[]))?;
    let analyser = preview.template == SetupTemplate::Analyzer;
    let (source, sheets, reason) = match (devices, how.source) {
        (Some(views), _) => {
            if let Some(object) = config.as_object_mut() {
                object.insert("breakpoints".to_owned(), Value::Array(views));
            }
            (SetupSource::Devices, None, None)
        }
        (None, Source::Analyser) => (
            SetupSource::Analyzer,
            Some(u32::try_from(how.sheets).unwrap_or(u32::MAX)),
            None,
        ),
        (None, Source::Default) => (
            SetupSource::Default,
            None,
            how.fallback.filter(|_| analyser).map(|why| SetupReason {
                code: why.reason,
                params: why.params,
            }),
        ),
    };
    check(&config)?;
    Ok(Proposal {
        config,
        source,
        sheets,
        reason,
    })
}

/// One breakpoint per chosen device preset: its id, its name, the preset that sizes it.
fn device_views(chosen: &[String]) -> Result<Vec<Value>, StepFailure> {
    if chosen.is_empty() {
        return Err(failure(NO_DEVICES_KEY, &[]));
    }
    let presets = crate::emulation::device_list();
    let mut views: Vec<Value> = Vec::new();
    for id in chosen.iter().take(MAX_DEVICES) {
        let preset = presets
            .iter()
            .find(|preset| &preset.id == id)
            .ok_or_else(|| failure(UNKNOWN_DEVICE_KEY, &[("device", id.clone())]))?;
        if views.iter().any(|view| view["id"] == preset.id.as_str()) {
            continue;
        }
        views.push(json!({
            "id": preset.id,
            "label": preset.label.clone().unwrap_or_else(|| preset.id.clone()),
            "device": preset.id,
        }));
    }
    Ok(views)
}

/// `config` passes the checks a written project config must pass.
fn check(config: &Value) -> Result<(), StepFailure> {
    crate::settings_window::write::validate_project(config, None, "").map_err(|issues| {
        let path = issues
            .first()
            .map(|issue| issue.path.clone())
            .unwrap_or_default();
        failure(INVALID_KEY, &[("path", path)])
    })
}

/// Writes `config` into the project at `root` as `rbrowser init` does; never over a config.
///
/// # Errors
///
/// A config exists by now, or the file cannot be written.
pub fn write(root: &Path, config: &Value) -> Result<PathBuf, StepFailure> {
    let path = ConfigFile::Project.path(root);
    if path.exists() {
        return Err(failure(EXISTS_KEY, &[("path", path.display().to_string())]));
    }
    check(config)?;
    rb_config::write_config_file(root, ConfigFile::Project, config)
        .map_err(|error| failure(WRITE_KEY, &[("message", error.to_string())]))?;
    Ok(path)
}

/// The proposals shown: only the last one can be written, once, by its number.
#[derive(Debug, Default)]
pub struct Proposals {
    next: u32,
    last: Option<(u32, PathBuf, Value)>,
}

impl Proposals {
    /// No proposal yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: 0,
            last: None,
        }
    }

    /// Keeps `config` for `root` as the last proposal; its number.
    pub fn keep(&mut self, root: PathBuf, config: Value) -> u32 {
        self.next = self.next.wrapping_add(1);
        self.last = Some((self.next, root, config));
        self.next
    }

    /// The last proposal when it is `id`, taken out.
    pub fn take(&mut self, id: u32) -> Option<(PathBuf, Value)> {
        match self.last.take() {
            Some((kept, root, config)) if kept == id => Some((root, config)),
            other => {
                self.last = other;
                None
            }
        }
    }
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
