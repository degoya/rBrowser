//! The snippet catalogue (RB-070-04): every snippet the plugin can run, from three sources.
//!
//! - **`user`**: the person's own, the setting `snippets.library` (user scope, protected from
//!   MCP): no repository can plant one, so they run in any workspace.
//! - **`project`**: `.rbrowser/snippets/*.js`, checked in with the project - code the project
//!   brings, so read only under workspace trust: the host refuses the folder to every plugin
//!   until a person trusted the workspace (`plugins.error.untrusted_workspace`), and the trust
//!   dialog names it. `snippets.projectEnabled` switches the source off.
//! - **`local`**: what the panel saved, `.rbrowser/out/snippets/library.json` - inside the
//!   project tree, where a repository could check one in, so it is held to the same trust: the
//!   answer for the project folder tells whether the workspace is trusted ([`probe`]).
//!
//! A snippet whose header does not read is listed with its error and never runs.

use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::{Map, Value, json};

use crate::config::Settings;
use crate::header::{self, MAX_NAME_CHARS, World};
use crate::run::RunTarget;

/// The project's snippets.
pub const PROJECT_DIR: &str = ".rbrowser/snippets";
/// The saved snippets, relative to the plugin's out folder and to the project.
pub const LIBRARY_FILE: &str = "library.json";
pub const LIBRARY_PATH: &str = ".rbrowser/out/snippets/library.json";
/// The longest snippet.
pub const MAX_CODE_CHARS: usize = 100_000;
/// The most snippets read from one source.
pub const MAX_PER_SOURCE: usize = 200;
/// The most code one listing carries; past it the code is left out (`codeOmitted`).
pub const MAX_LISTED_CODE_BYTES: usize = 2 * 1024 * 1024;

/// Where a snippet comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    User,
    Project,
    Local,
}

impl Source {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "user" => Some(Self::User),
            "project" => Some(Self::Project),
            "local" => Some(Self::Local),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
            Self::Local => "local",
        }
    }
}

/// One snippet, its header applied over the settings' defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct Snippet {
    pub source: Source,
    pub name: String,
    pub description: String,
    pub world: World,
    pub target: RunTarget,
    pub timeout_ms: u32,
    pub code: String,
    /// The project file it was read from.
    pub file: Option<String>,
    /// Why it cannot run: `{ "code", "params" }`.
    pub error: Option<Value>,
}

/// Whether a source was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Loaded,
    /// The workspace is not trusted.
    Untrusted,
    /// `snippets.projectEnabled` is off.
    Disabled,
    /// No project is open.
    NoProject,
}

impl Access {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Loaded => "loaded",
            Self::Untrusted => "untrusted",
            Self::Disabled => "disabled",
            Self::NoProject => "noProject",
        }
    }
}

/// The workspace's trust as the host's answer for the project folder tells it, and the folder's
/// names when it could be listed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Probe {
    Trusted(Vec<String>),
    Untrusted,
    NoProject,
}

/// Asks the host for the project folder: trusted (its names; none when it does not exist),
/// untrusted, or no project. Any other refusal counts as untrusted.
pub fn probe(host: &mut dyn PluginHost) -> Probe {
    match host.project_list(PROJECT_DIR) {
        Ok(names) => Probe::Trusted(names),
        Err(failure) => match failure.code.as_deref() {
            Some(codes::NOT_FOUND) => Probe::Trusted(Vec::new()),
            Some(codes::NOT_AVAILABLE) => Probe::NoProject,
            _ => Probe::Untrusted,
        },
    }
}

/// Every snippet and whether the project's and the saved ones were read.
#[derive(Clone, Debug, PartialEq)]
pub struct Catalog {
    pub snippets: Vec<Snippet>,
    pub project: Access,
    pub local: Access,
}

fn error(name: &str, params: &[(&str, String)]) -> Value {
    let params: Map<String, Value> = params
        .iter()
        .map(|(key, value)| ((*key).to_owned(), json!(value)))
        .collect();
    json!({ "code": crate::key(name), "params": params })
}

/// A snippet of `source` named `name` (the header's name for a project file) from `code`.
#[must_use]
pub fn snippet(
    source: Source,
    name: &str,
    code: &str,
    settings: &Settings,
    file: Option<&str>,
) -> Snippet {
    let mut snippet = Snippet {
        source,
        name: name.to_owned(),
        description: String::new(),
        world: settings.default_world,
        target: RunTarget::All,
        timeout_ms: settings.default_timeout_ms,
        code: code.to_owned(),
        file: file.map(str::to_owned),
        error: None,
    };
    if code.chars().count() > MAX_CODE_CHARS {
        snippet.code = String::new();
        snippet.error = Some(error(
            "errors.code_too_long",
            &[("limit", MAX_CODE_CHARS.to_string())],
        ));
        return snippet;
    }
    match header::parse(code) {
        Ok(header) => {
            if source == Source::Project
                && let Some(name) = header.name
            {
                snippet.name = name;
            }
            snippet.description = header.description.unwrap_or_default();
            snippet.world = header.world.unwrap_or(snippet.world);
            snippet.target = header.target.unwrap_or_default();
            snippet.timeout_ms = header.timeout_ms.unwrap_or(snippet.timeout_ms);
        }
        Err(bad) => snippet.error = Some(error("errors.header", &[("tag", bad.tag)])),
    }
    snippet
}

/// The `{ name, code }` entries of a library (`snippets.library` or the saved file).
fn entries(source: Source, list: &[Value], settings: &Settings, into: &mut Vec<Snippet>) {
    for entry in list.iter().take(MAX_PER_SOURCE) {
        let name = entry["name"]
            .as_str()
            .and_then(|name| header::line_text(name, MAX_NAME_CHARS));
        let (Some(name), Some(code)) = (name, entry["code"].as_str()) else {
            continue;
        };
        into.push(snippet(source, &name, code, settings, None));
    }
}

fn project_snippets(
    host: &mut dyn PluginHost,
    names: &[String],
    settings: &Settings,
    into: &mut Vec<Snippet>,
) {
    let mut files: Vec<&String> = names
        .iter()
        .filter(|name| name.ends_with(".js") && !name.contains('/') && name.len() > 3)
        .collect();
    files.sort();
    for file in files.into_iter().take(MAX_PER_SOURCE) {
        let stem = &file[..file.len() - 3];
        let path = format!("{PROJECT_DIR}/{file}");
        let mut entry = match host.project_read(&path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(code) => snippet(Source::Project, stem, &code, settings, Some(file)),
                Err(_) => snippet(Source::Project, stem, "", settings, Some(file)),
            },
            Err(failure) => {
                let code = failure
                    .code
                    .unwrap_or_else(|| codes::HOST_FAILED.to_owned());
                let mut entry = snippet(Source::Project, stem, "", settings, Some(file));
                entry.error = Some(json!({ "code": code, "params": {} }));
                entry
            }
        };
        if entry.error.is_none() && entry.code.trim().is_empty() {
            entry.error = Some(error("errors.empty", &[]));
        }
        into.push(entry);
    }
}

/// The saved snippets' entries as they read; none for a file that is missing or does not read.
pub fn saved_entries(host: &mut dyn PluginHost) -> Vec<Value> {
    host.project_read(LIBRARY_PATH)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|library| library["snippets"].as_array().cloned())
        .unwrap_or_default()
}

/// Marks every snippet after the first of a name in its source.
fn mark_duplicates(snippets: &mut [Snippet]) {
    for index in 1..snippets.len() {
        let (before, rest) = snippets.split_at_mut(index);
        let current = &mut rest[0];
        if current.error.is_none()
            && before
                .iter()
                .any(|other| other.source == current.source && other.name == current.name)
        {
            current.error = Some(error("errors.duplicate_name", &[]));
        }
    }
}

/// The catalogue as the host and the settings give it now.
pub fn load(host: &mut dyn PluginHost, settings: &Settings) -> Catalog {
    let mut snippets = Vec::new();
    entries(Source::User, &settings.library, settings, &mut snippets);
    let (project, local) = match probe(host) {
        Probe::NoProject => (Access::NoProject, Access::NoProject),
        Probe::Untrusted => (Access::Untrusted, Access::Untrusted),
        Probe::Trusted(names) => {
            let project = if settings.project_enabled {
                project_snippets(host, &names, settings, &mut snippets);
                Access::Loaded
            } else {
                Access::Disabled
            };
            let saved = saved_entries(host);
            entries(Source::Local, &saved, settings, &mut snippets);
            (project, Access::Loaded)
        }
    };
    mark_duplicates(&mut snippets);
    Catalog {
        snippets,
        project,
        local,
    }
}

impl Catalog {
    /// The snippet `name` of `source`, or of any source when it is the only one of that name.
    ///
    /// # Errors
    ///
    /// `errors.not_found` (`errors.project_untrusted` when the project's snippets were not read
    /// for want of trust), `errors.ambiguous`, `errors.invalid_snippet` for one that cannot run.
    pub fn find(&self, source: Option<Source>, name: &str) -> Result<&Snippet, Failure> {
        let matches: Vec<&Snippet> = self
            .snippets
            .iter()
            .filter(|snippet| {
                snippet.name == name && source.is_none_or(|source| snippet.source == source)
            })
            .collect();
        let found = match matches.as_slice() {
            [] => {
                let untrusted = self.project == Access::Untrusted
                    && source.is_none_or(|source| source != Source::User);
                let key = if untrusted {
                    "errors.project_untrusted"
                } else {
                    "errors.not_found"
                };
                return Err(crate::coded(FailureKind::NotFound, key)
                    .with_param("name", crate::text::cut(name, MAX_NAME_CHARS)));
            }
            [one] => *one,
            many if many.iter().all(|snippet| snippet.source == many[0].source) => many[0],
            _ => {
                return Err(crate::coded(FailureKind::BadData, "errors.ambiguous")
                    .with_param("name", crate::text::cut(name, MAX_NAME_CHARS)));
            }
        };
        if found.error.is_some() {
            return Err(crate::coded(FailureKind::BadData, "errors.invalid_snippet")
                .with_param("name", crate::text::cut(name, MAX_NAME_CHARS)));
        }
        Ok(found)
    }

    /// The listing: `{ snippets: [...], project, local }`, with each snippet's code when
    /// `with_code` (up to [`MAX_LISTED_CODE_BYTES`] in all).
    #[must_use]
    pub fn to_json(&self, with_code: bool) -> Value {
        let mut budget = MAX_LISTED_CODE_BYTES;
        let snippets: Vec<Value> = self
            .snippets
            .iter()
            .map(|snippet| {
                let mut entry = json!({
                    "source": snippet.source.as_str(),
                    "name": snippet.name,
                    "description": snippet.description,
                    "world": snippet.world.as_str(),
                    "target": snippet.target.as_str(),
                    "timeoutMs": snippet.timeout_ms,
                });
                if let Some(file) = &snippet.file {
                    entry["file"] = json!(file);
                }
                if let Some(error) = &snippet.error {
                    entry["error"] = error.clone();
                }
                if with_code {
                    if snippet.code.len() <= budget {
                        budget -= snippet.code.len();
                        entry["code"] = json!(snippet.code);
                    } else {
                        entry["codeOmitted"] = json!(true);
                    }
                }
                entry
            })
            .collect();
        json!({ "snippets": snippets, "project": self.project.as_str(), "local": self.local.as_str() })
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
