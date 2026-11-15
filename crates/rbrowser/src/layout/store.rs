//! A mode's fields written into the config (RB-080-04): the split ratio and sides after the
//! divider moved or the sides swapped, the canvas items after a view was placed, the horizontal
//! parameters given to `layout.mode.horizontal`. Scroll, pan and the minimap stay in the session.
//!
//! They go to the active preset when it has this mode; otherwise to the top-level layout's
//! `layout.modes.<mode>` - rBrowser makes no preset of its own (owner, 2026-11-12: presets named
//! `split` or `canvas` beside the mode buttons looked like modes). It is written to the file
//! that defines it - `config.json` when it is there, else `config.local.json` (positions are
//! personal until moved into the project file) - checked as the settings window checks a change
//! and written in the canonical form. The watcher reads the config again. "Save layout as
//! preset" ([`save_preset`]) names its file itself.

use std::path::Path;

use rb_commands::CommandError;
use rb_config::{ConfigFile, LayoutMode, ModeFields, ProjectConfig};
use serde_json::{Map, Value, json};

use crate::control::Core;
use crate::settings_window::{user_file, write};

/// The config key the preset lives under.
const PRESETS: [&str; 2] = ["layouts", "presets"];
/// The config key of the top-level layout's mode fields (`layout.modes.<mode>`).
const MODES: [&str; 2] = ["layout", "modes"];

/// Where a mode's fields are kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// The preset of this id, which has the mode.
    Preset(String),
    /// The top-level layout's `layout.modes`.
    TopLevel,
}

impl Target {
    /// What the commands answer as `persisted`: the preset's id, or `"layout"`.
    pub fn label(&self) -> String {
        match self {
            Target::Preset(id) => id.clone(),
            Target::TopLevel => "layout".to_owned(),
        }
    }

    /// The JSON path of the fields' object in a config file.
    fn path(&self, mode: LayoutMode) -> Vec<String> {
        match self {
            Target::Preset(id) => PRESETS
                .iter()
                .map(|key| (*key).to_owned())
                .chain([id.clone()])
                .collect(),
            Target::TopLevel => MODES
                .iter()
                .copied()
                .chain([mode_name(mode)])
                .map(str::to_owned)
                .collect(),
        }
    }
}

/// Where the fields of `mode` live for the selected `preset`: the preset when it has that mode,
/// else the top-level layout.
pub fn target(config: &ProjectConfig, preset: Option<&str>, mode: LayoutMode) -> Target {
    match preset {
        Some(id)
            if config
                .layouts
                .presets
                .get(id)
                .is_some_and(|found| found.mode == mode) =>
        {
            Target::Preset(id.to_owned())
        }
        _ => Target::TopLevel,
    }
}

/// The fields of `mode` kept for the selected `preset` (see [`target`]); none when none are.
pub fn stored(config: &ProjectConfig, preset: Option<&str>, mode: LayoutMode) -> ModeFields {
    match target(config, preset, mode) {
        Target::Preset(id) => config
            .layouts
            .presets
            .get(&id)
            .map(|preset| preset.modes.clone())
            .unwrap_or_default(),
        Target::TopLevel => config.layout.modes.get(mode).cloned().unwrap_or_default(),
    }
}

/// Writes `fields` (camelCase keys; `null` removes one) into `target` for `mode`; answers the
/// target's label. Fails without an open project or when the result does not pass the config
/// check.
pub fn persist(
    core: &Core,
    target: &Target,
    mode: LayoutMode,
    fields: &Map<String, Value>,
) -> Result<String, CommandError> {
    let path = target.path(mode);
    let mode_key = matches!(target, Target::Preset(_)).then_some(mode);
    write_into(core, None, &path, |document| {
        patch(document, &path, mode_key, fields);
    })?;
    Ok(target.label())
}

/// `layout.save_preset`: `preset` (a whole preset object) as `layouts.presets.<id>` of `file`,
/// replacing a preset of that id there. Nothing is written when the result does not pass the
/// config check.
pub fn save_preset(
    core: &Core,
    id: &str,
    file: ConfigFile,
    preset: &Value,
) -> Result<(), CommandError> {
    let path: Vec<String> = PRESETS.iter().map(|key| (*key).to_owned()).collect();
    let mut entry = Map::new();
    entry.insert(id.to_owned(), preset.clone());
    write_into(core, Some(file), &path, |document| {
        patch(document, &path, None, &entry);
    })
    .map(|_| ())
}

/// Applies `edit` to the config file `file` - else to the one that has `path` (`config.json`
/// when it is there, else `config.local.json`) - checks the merged result as the settings window
/// does and writes it; answers the file written. `view.set_engine` writes a breakpoint's engine
/// through it too (RB-080-33).
pub(crate) fn write_into(
    core: &Core,
    file: Option<ConfigFile>,
    path: &[String],
    edit: impl FnOnce(&mut Value),
) -> Result<ConfigFile, CommandError> {
    let host = core.host();
    let root = host
        .project_root()
        .ok_or_else(|| CommandError::Failed("no project is open".to_owned()))?;
    let project_path = ConfigFile::Project.path(&root);
    let local_path = ConfigFile::Local.path(&root);
    let mut project = read_or_empty(&project_path)?;
    let mut local = read_or_empty(&local_path)?;
    let file = file.unwrap_or(if at(&project, path).is_some() {
        ConfigFile::Project
    } else {
        ConfigFile::Local
    });
    let in_project = file == ConfigFile::Project;
    let file_path = if in_project {
        &project_path
    } else {
        &local_path
    };
    if !user_file::writable(file_path) {
        return Err(CommandError::Failed(format!(
            "{} cannot be read, so it is not written over",
            file.file_name()
        )));
    }
    edit(if in_project { &mut project } else { &mut local });
    write::validate_project(&project, Some(&local), "").map_err(|issues| {
        let problems: Vec<String> = issues
            .iter()
            .map(|issue| format!("{} {}", issue.path, issue.message_key))
            .collect();
        CommandError::InvalidArgs(format!(
            "the layout does not pass the config check: {}",
            problems.join("; ")
        ))
    })?;
    let document = if in_project { &project } else { &local };
    rb_config::write_config_file(&root, file, document)
        .map_err(|error| CommandError::Failed(error.to_string()))?;
    Ok(file)
}

/// The config name of `mode`.
pub fn mode_name(mode: LayoutMode) -> &'static str {
    match mode {
        LayoutMode::Grid => "grid",
        LayoutMode::Focus => "focus",
        LayoutMode::Horizontal => "horizontal",
        LayoutMode::Split => "split",
        LayoutMode::Canvas => "canvas",
    }
}

/// Writes `fields` into the object at `path` of `document` (a `null` field is removed), and
/// `mode` as its `"mode"` when given (a preset). Missing objects on the way are created.
pub fn patch(
    document: &mut Value,
    path: &[String],
    mode: Option<LayoutMode>,
    fields: &Map<String, Value>,
) {
    if !document.is_object() {
        *document = json!({});
    }
    let mut node = document;
    for key in path {
        let Some(object) = node.as_object_mut() else {
            return;
        };
        let entry = object.entry(key.clone()).or_insert_with(|| json!({}));
        if !entry.is_object() {
            *entry = json!({});
        }
        node = entry;
    }
    let Some(object) = node.as_object_mut() else {
        return;
    };
    if let Some(mode) = mode {
        object.insert("mode".to_owned(), Value::from(mode_name(mode)));
    }
    for (key, value) in fields {
        if value.is_null() {
            object.remove(key);
        } else {
            object.insert(key.clone(), value.clone());
        }
    }
}

/// The value at `path` of `document`, when there is one.
fn at<'a>(document: &'a Value, path: &[String]) -> Option<&'a Value> {
    path.iter().try_fold(document, |node, key| node.get(key))
}

/// The JSON of `path`; an empty object when there is no such file.
fn read_or_empty(path: &Path) -> Result<Value, CommandError> {
    if !path.is_file() {
        return Ok(json!({}));
    }
    rb_config::read_json(path).map_err(|error| CommandError::Failed(error.to_string()))
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
