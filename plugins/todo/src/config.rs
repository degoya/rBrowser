//! The plugin's settings (`settings.schema.json`, Settings > Project > Todo): which files are
//! todo files, which folders are never searched, and the timestamp of `@done(...)`. The panel
//! reads its own keys (`hideDone`, `hideArchive`, `refreshSeconds`) from the settings message.

use rb_plugin_api::PluginHost;
use serde_json::Value;

use crate::clock::Format;

/// `todo.files` without a setting.
pub const DEFAULT_FILES: &str = "**/*.todo";
/// `todo.exclude` without a setting: dependencies, version control and build output - folders
/// no one keeps todos in, and the largest of a project (code review RB-110-13).
pub const DEFAULT_EXCLUDE: [&str; 13] = [
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    ".nuxt",
    ".output",
    ".svelte-kit",
    "vendor",
    ".venv",
    "__pycache__",
];
/// The most globs and folder names each list keeps.
pub const MAX_LIST: usize = 20;
/// The longest glob or folder name kept, in characters.
const MAX_ENTRY_CHARS: usize = 200;

/// The settings in effect.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// `todo.files`: globs a todo file's project-relative path matches.
    pub files: Vec<String>,
    /// `todo.exclude`: folder names never searched, compared without regard to case.
    pub exclude: Vec<String>,
    /// `todo.timestampFormat`.
    pub format: Format,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            files: vec![DEFAULT_FILES.to_owned()],
            exclude: DEFAULT_EXCLUDE
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
            format: Format::default(),
        }
    }
}

fn value(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

/// A list of non-empty strings, capped; `None` when the value is no list.
fn strings(value: Option<Value>) -> Option<Vec<String>> {
    let Value::Array(items) = value? else {
        return None;
    };
    Some(
        items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|item| !item.is_empty() && item.chars().count() <= MAX_ENTRY_CHARS)
            .take(MAX_LIST)
            .map(str::to_owned)
            .collect(),
    )
}

impl Settings {
    /// The settings the host answers; the default for each key it has not or that does not read.
    #[must_use]
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let mut settings = Self::default();
        if let Some(files) = strings(value(host, "files")).filter(|files| !files.is_empty()) {
            settings.files = files;
        }
        if let Some(exclude) = strings(value(host, "exclude")) {
            settings.exclude = exclude;
        }
        if let Some(format) = value(host, "timestampFormat")
            .as_ref()
            .and_then(Value::as_str)
            .and_then(Format::parse)
        {
            settings.format = format;
        }
        settings
    }

    /// Whether a folder named `name` is never searched.
    #[must_use]
    pub fn excluded(&self, name: &str) -> bool {
        self.exclude
            .iter()
            .any(|excluded| excluded.eq_ignore_ascii_case(name))
    }

    /// Whether a glob of `files` names the folder `path` (project-relative) outright, before its
    /// first wildcard (`.plans/*.todo` names `.plans`): searched though hidden or git-ignored.
    #[must_use]
    pub fn names_folder(&self, path: &str) -> bool {
        let folder = format!("{path}/");
        self.files.iter().any(|glob| {
            let literal = glob.split(['*', '?']).next().unwrap_or_default();
            literal.starts_with(&folder)
        })
    }

    /// Whether the project-relative `path` is a todo file of the settings.
    #[must_use]
    pub fn wanted(&self, path: &str) -> bool {
        self.files
            .iter()
            .any(|glob| crate::glob::matches(glob, path))
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
