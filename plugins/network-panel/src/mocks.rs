//! Mock files: `.rbrowser/mocks/*.json` (the project's, checked in, project content under
//! workspace trust; the folder is the setting `mocksDir`) and the plugin's own in
//! `.rbrowser/out/network-panel/mocks/*.json` (rules added over MCP or the panel, HAR imports).
//! Both have one form:
//!
//! ```json
//! { "rules": [
//!   { "id": "user-500", "enabled": true,
//!     "match": { "urlPattern": "*/api/user", "regex": false, "method": "GET", "resourceType": "XHR" },
//!     "mock": { "status": 500, "headers": { "content-type": "application/json" },
//!               "body": { "error": "down" }, "delayMs": 800 } },
//!   { "id": "no-tracking", "match": { "urlPattern": "*/tracking.js" }, "block": true },
//!   { "id": "header", "match": { "urlPattern": "*/api/*" },
//!     "rewrite": { "headers": { "x-test": "1" } } } ] }
//! ```
//!
//! A mock's body is `body` (text, or JSON written out), `bodyBase64`, or `bodyFile` - a path
//! relative to the mocks folder, never absolute, never `..`. A rule's id within the host is
//! `<file>#<id>` (`own:<file>#<id>` for the plugin's own files). The host validates the rules
//! again; this module only makes them. The kinds - the extended rewrite, `mapLocal` and
//! `mapRemote` of RB-080-37 among them - are [`crate::kinds`].

use base64::Engine as _;
use rb_plugin_api::PluginHost;
use serde_json::{Value, json};

pub use crate::kinds::{Body, Kind};

/// The plugin's own mock files, relative to its out folder.
pub const OWN_DIR: &str = "mocks";
/// The plugin's own mock files, relative to the project.
pub const OWN_PATH: &str = ".rbrowser/out/network-panel/mocks";
/// The file rules added over MCP and the panel go to.
pub const ADDED_FILE: &str = "added.json";
/// The most mock files read per folder.
pub const MAX_FILES: usize = 64;
/// The largest mock body, in bytes.
pub const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

/// One rule of a mock file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockRule {
    /// The id within its file.
    pub id: String,
    pub enabled: bool,
    pub url_pattern: String,
    pub regex: bool,
    pub method: Option<String>,
    pub resource_type: Option<String>,
    pub kind: Kind,
}

/// One mock file, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockFile {
    /// Relative to the project.
    pub path: String,
    /// The id prefix of its rules (`api.json`, `own:added.json`).
    pub prefix: String,
    /// The plugin's own file (it may change it).
    pub owned: bool,
    pub rules: Vec<MockRule>,
    /// What did not read, one line each.
    pub problems: Vec<String>,
}

impl MockFile {
    /// `<prefix>#<id>`.
    #[must_use]
    pub fn qualified(&self, rule: &MockRule) -> String {
        format!("{}#{}", self.prefix, rule.id)
    }
}

/// Whether `path` is relative and plain: `/`-separated segments, none empty, `.` or `..`, no
/// backslash, no drive, no control character.
#[must_use]
pub fn relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 512
        && !path.contains(['\\', ':'])
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

/// Whether `id` may name a rule in a file.
#[must_use]
pub fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

/// A file name as a rule id prefix: characters outside `[A-Za-z0-9._-]` become `_`.
#[must_use]
pub fn prefix_of(name: &str) -> String {
    name.chars()
        .take(64)
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Rule `index` of a file as it is written.
///
/// # Errors
///
/// What does not read, as one line.
pub fn parse_rule(raw: &Value, index: usize) -> Result<MockRule, String> {
    let id = match &raw["id"] {
        Value::Null => format!("r{index}"),
        Value::String(id) if valid_id(id) => id.clone(),
        _ => {
            return Err(format!(
                "rule {index}: the id is not [A-Za-z0-9._-], 1 to 64"
            ));
        }
    };
    let at = |reason: String| format!("{id}: {reason}");
    let matcher = &raw["match"];
    let url_pattern = matcher["urlPattern"]
        .as_str()
        .filter(|pattern| !pattern.is_empty())
        .ok_or_else(|| at("match.urlPattern is missing".to_owned()))?
        .to_owned();
    let text = |key: &str| matcher[key].as_str().map(str::to_owned);
    Ok(MockRule {
        enabled: raw["enabled"] != Value::Bool(false),
        regex: matcher["regex"] == Value::Bool(true),
        method: text("method").map(|method| method.to_ascii_uppercase()),
        resource_type: text("resourceType"),
        kind: crate::kinds::parse(raw).map_err(at)?,
        url_pattern,
        id,
    })
}

/// A mock file's text, read.
#[must_use]
pub fn parse_file(bytes: &[u8], path: &str, prefix: &str, owned: bool) -> MockFile {
    let mut file = MockFile {
        path: path.to_owned(),
        prefix: prefix.to_owned(),
        owned,
        rules: Vec::new(),
        problems: Vec::new(),
    };
    let rules = match serde_json::from_slice::<Value>(bytes) {
        Ok(Value::Object(map)) => match map.get("rules") {
            Some(Value::Array(rules)) => rules.clone(),
            _ => {
                file.problems.push("no rules list".to_owned());
                return file;
            }
        },
        _ => {
            file.problems.push("no JSON object".to_owned());
            return file;
        }
    };
    for (index, raw) in rules.iter().enumerate() {
        match parse_rule(raw, index) {
            Ok(rule) if file.rules.iter().any(|known| known.id == rule.id) => {
                file.problems.push(format!("{}: the id is taken", rule.id));
            }
            Ok(rule) => file.rules.push(rule),
            Err(problem) => file.problems.push(problem),
        }
    }
    file
}

/// The `*.json` files of `dir`, by name, at most [`MAX_FILES`].
fn json_files(host: &mut dyn PluginHost, dir: &str) -> Vec<String> {
    let mut names: Vec<String> = host
        .project_list(dir)
        .unwrap_or_default()
        .into_iter()
        .filter(|name| name.ends_with(".json") && !name.contains('/'))
        .collect();
    names.sort();
    names.truncate(MAX_FILES);
    names
}

/// The names of the project's mock files (listed even without trust: the panel names them).
#[must_use]
pub fn project_files(host: &mut dyn PluginHost, mocks_dir: &str) -> Vec<String> {
    json_files(host, mocks_dir)
        .into_iter()
        .map(|name| format!("{mocks_dir}/{name}"))
        .collect()
}

/// Every mock file read: the project's, then the plugin's own. Call only under trust.
#[must_use]
pub fn load(host: &mut dyn PluginHost, mocks_dir: &str) -> Vec<MockFile> {
    let mut files = Vec::new();
    for (dir, owned) in [(mocks_dir, false), (OWN_PATH, true)] {
        for name in json_files(host, dir) {
            let path = format!("{dir}/{name}");
            let prefix = if owned {
                format!("own:{}", prefix_of(&name))
            } else {
                prefix_of(&name)
            };
            let file = match host.project_read(&path) {
                Ok(bytes) => parse_file(&bytes, &path, &prefix, owned),
                Err(failure) => MockFile {
                    path: path.clone(),
                    prefix,
                    owned,
                    rules: Vec::new(),
                    problems: vec![failure.code.unwrap_or_else(|| "unreadable".to_owned())],
                },
            };
            files.push(file);
        }
    }
    files
}

/// `rule` of `file` as a host rule (origin `project`), its body read.
///
/// # Errors
///
/// The body file cannot be read or is too large.
pub fn host_rule(
    host: &mut dyn PluginHost,
    file: &MockFile,
    rule: &MockRule,
    mocks_dir: &str,
) -> Result<Value, String> {
    let mut matcher = json!({ "urlPattern": rule.url_pattern, "regex": rule.regex });
    if let Some(method) = &rule.method {
        matcher["method"] = json!(method);
    }
    if let Some(kind) = &rule.resource_type {
        matcher["resourceType"] = json!(kind);
    }
    let body_base64 = match &rule.kind {
        Kind::Mock { body, .. } => {
            let engine = base64::engine::general_purpose::STANDARD;
            match body {
                Body::None => String::new(),
                Body::Text(text) => engine.encode(text.as_bytes()),
                Body::Base64(data) => data.clone(),
                Body::File(relative) => {
                    let bytes = host
                        .project_read(&format!("{mocks_dir}/{relative}"))
                        .map_err(|_| format!("bodyFile {relative} cannot be read"))?;
                    if bytes.len() > MAX_BODY_BYTES {
                        return Err(format!("bodyFile {relative} is larger than 4 MiB"));
                    }
                    engine.encode(bytes)
                }
            }
        }
        _ => String::new(),
    };
    let action = crate::kinds::host_action(&rule.kind, &body_base64);
    Ok(
        json!({ "id": file.qualified(rule), "origin": "project", "match": matcher,
               "action": action }),
    )
}

/// `rule` in the file form, as [`parse_rule`] reads it back.
#[must_use]
pub fn rule_json(rule: &MockRule) -> Value {
    let mut matcher = json!({ "urlPattern": rule.url_pattern });
    if rule.regex {
        matcher["regex"] = json!(true);
    }
    if let Some(method) = &rule.method {
        matcher["method"] = json!(method);
    }
    if let Some(kind) = &rule.resource_type {
        matcher["resourceType"] = json!(kind);
    }
    let mut out = json!({ "id": rule.id, "enabled": rule.enabled, "match": matcher });
    crate::kinds::write_json(&rule.kind, &mut out);
    out
}

/// A file of `rules` in the file form.
#[must_use]
pub fn file_json(rules: &[MockRule]) -> Value {
    json!({ "rules": rules.iter().map(rule_json).collect::<Vec<_>>() })
}

#[cfg(test)]
#[path = "mocks_tests.rs"]
mod tests;
