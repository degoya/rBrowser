//! The project's rule file `.rbrowser/htmlvalidate.json` (RB-094-06): a checked-in subset of
//! html-validate's configuration - the levels and options of the plugin's own html-validate rules
//! (`rules::all`), nothing else. It takes precedence over `html-validate.disabledRules`: a rule
//! it sets to a level runs at that level even when the setting switches it off, a rule it sets
//! `off` never runs; and it is the one way to switch on an opt-in rule (`no-unknown-attributes`).
//!
//! **Only under workspace trust.** The file is a project's, so it counts only once a person
//! trusted the workspace. The host gates `project.read` by trust for the folders the trust hash
//! covers (`.rbrowser/plugins/` among them), not for this file, and tells a plugin nothing else
//! about trust; so [`trusted`] asks the host's own gate - a listing of `.rbrowser/plugins/`
//! refused with `plugins.error.untrusted_workspace` is an untrusted workspace, and any other
//! refusal counts as untrusted too. An untrusted project's file is never parsed.
//!
//! **Never code.** html-validate's configuration can name plugins, presets (`extends`), element
//! tables and transformers, which load code; only `rules` (and `$schema`) is read, every other
//! key refuses the file. A rule is one of the plugin's html-validate rules, its level `off`,
//! `warn`, `error` or 0 to 2, its options a small JSON object of plain values. The file is at
//! most [`MAX_FILE_BYTES`].

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, FailureKind, PluginHost, Severity, codes};
use serde_json::{Map, Value};

use crate::{key, rules};

/// The project-relative path of the rule file.
pub const PROJECT_FILE: &str = ".rbrowser/htmlvalidate.json";
/// A folder the host reads only under workspace trust: the probe of [`trusted`].
pub const TRUST_PROBE: &str = ".rbrowser/plugins";
/// The largest rule file read.
pub const MAX_FILE_BYTES: usize = 64 * 1024;
/// The most rules the file sets.
pub const MAX_PROJECT_RULES: usize = 128;
/// The most keys of one options object.
pub const MAX_OPTION_KEYS: usize = 32;
/// The longest string of an option.
pub const MAX_OPTION_TEXT: usize = 512;
/// The most items of an option list.
pub const MAX_OPTION_ITEMS: usize = 64;
/// How deep options nest: the options object, an object in it, one more.
pub const MAX_OPTION_DEPTH: usize = 3;

/// What became of the rule file in a run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    /// There is none (or no project).
    #[default]
    None,
    /// There is one, but the workspace is not trusted: not applied.
    Untrusted,
    /// Applied.
    Applied,
}

impl State {
    /// The name in the MCP answer.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Untrusted => "untrusted",
            Self::Applied => "applied",
        }
    }
}

/// A rule's level, as html-validate takes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Off,
    Warn,
    Error,
}

impl Level {
    /// The severity of its messages; `None` for off.
    #[must_use]
    pub const fn severity(self) -> Option<Severity> {
        match self {
            Self::Off => None,
            Self::Warn => Some(Severity::Warning),
            Self::Error => Some(Severity::Error),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

fn read_level(value: &Value) -> Option<Level> {
    match value {
        Value::String(text) => match text.as_str() {
            "off" => Some(Level::Off),
            "warn" => Some(Level::Warn),
            "error" => Some(Level::Error),
            _ => None,
        },
        Value::Number(number) => match number.as_u64() {
            Some(0) => Some(Level::Off),
            Some(1) => Some(Level::Warn),
            Some(2) => Some(Level::Error),
            _ => None,
        },
        _ => None,
    }
}

/// The level of an html-validate rule configuration: `"error"`, `2` or `["error", {...}]`.
#[must_use]
pub fn level_of(config: &Value) -> Option<Level> {
    match config {
        Value::Array(items) => items.first().and_then(read_level),
        other => read_level(other),
    }
}

/// The rules a project's file sets: by rule id, the level and html-validate's configuration
/// (`"warn"` or `["warn", {options}]`).
pub type Rules = BTreeMap<String, (Level, Value)>;

/// The rule file of a run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectRules {
    pub state: State,
    rules: Rules,
}

impl ProjectRules {
    /// An applied file setting `rules`.
    #[must_use]
    pub const fn applied(rules: Rules) -> Self {
        Self {
            state: State::Applied,
            rules,
        }
    }

    /// The level the file sets for rule `id`.
    #[must_use]
    pub fn level(&self, id: &str) -> Option<Level> {
        self.rules.get(id).map(|(level, _)| *level)
    }

    /// The configuration the file sets for rule `id`.
    #[must_use]
    pub fn config(&self, id: &str) -> Option<&Value> {
        self.rules.get(id).map(|(_, config)| config)
    }
}

fn refused(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, &key("errors", "project_rules"))
        .with_param("reason", reason.chars().take(300).collect::<String>())
}

fn short(text: &str) -> String {
    text.chars().take(64).collect()
}

fn check_options(value: &Value, depth: usize, at: &str) -> Result<(), Failure> {
    let deep = || refused(&format!("the options of `{at}` nest too deep"));
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
        Value::String(text) if text.chars().count() <= MAX_OPTION_TEXT => Ok(()),
        Value::String(_) => Err(refused(&format!(
            "an option of `{at}` is longer than {MAX_OPTION_TEXT} characters"
        ))),
        Value::Array(items) => {
            if items.len() > MAX_OPTION_ITEMS {
                return Err(refused(&format!(
                    "an option list of `{at}` has more than {MAX_OPTION_ITEMS} items"
                )));
            }
            if depth >= MAX_OPTION_DEPTH {
                return Err(deep());
            }
            items
                .iter()
                .try_for_each(|item| check_options(item, depth + 1, at))
        }
        Value::Object(map) => {
            if depth >= MAX_OPTION_DEPTH {
                return Err(deep());
            }
            if map.len() > MAX_OPTION_KEYS {
                return Err(refused(&format!(
                    "the options of `{at}` have more than {MAX_OPTION_KEYS} keys"
                )));
            }
            for (name, item) in map {
                let named = !name.is_empty()
                    && name.len() <= 64
                    && name.starts_with(|c: char| c.is_ascii_alphabetic())
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
                if !named {
                    return Err(refused(&format!(
                        "`{}` is no option name of `{at}`",
                        short(name)
                    )));
                }
                check_options(item, depth + 1, at)?;
            }
            Ok(())
        }
    }
}

fn rule(id: &str, value: &Value) -> Result<(Level, Value), Failure> {
    let offered = rules::find(id).is_some_and(|rule| !rules::is_nu(&rule.id));
    if !offered {
        return Err(refused(&format!(
            "`{}` is no rule of the plugin's html-validate rules",
            short(id)
        )));
    }
    let bad_level = || refused(&format!("`{id}` has no level off, warn or error"));
    match value {
        Value::Array(items) => {
            let level = items.first().and_then(read_level).ok_or_else(bad_level)?;
            match items.get(1..).unwrap_or_default() {
                [] => Ok((level, Value::String(level.as_str().to_owned()))),
                [Value::Object(options)] => {
                    let options = Value::Object(options.clone());
                    check_options(&options, 0, id)?;
                    Ok((
                        level,
                        Value::Array(vec![Value::String(level.as_str().to_owned()), options]),
                    ))
                }
                _ => Err(refused(&format!(
                    "`{id}` takes a level and at most one options object"
                ))),
            }
        }
        other => {
            let level = read_level(other).ok_or_else(bad_level)?;
            Ok((level, Value::String(level.as_str().to_owned())))
        }
    }
}

/// The rules of a rule file's bytes.
///
/// # Errors
///
/// `errors.project_rules` with the reason: larger than [`MAX_FILE_BYTES`], no JSON object, a key
/// other than `rules` and `$schema`, a rule the plugin does not offer, a level or options outside
/// the subset.
pub fn parse(bytes: &[u8]) -> Result<Rules, Failure> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(refused("larger than 64 KiB"));
    }
    let value: Value =
        serde_json::from_slice(bytes).map_err(|error| refused(&format!("not JSON: {error}")))?;
    let Value::Object(file) = value else {
        return Err(refused("the file is no JSON object"));
    };
    if let Some(other) = file
        .keys()
        .find(|name| *name != "rules" && *name != "$schema")
    {
        return Err(refused(&format!(
            "`{}` is not read: only `rules` (html-validate's plugins, presets, elements and \
             transformers can load code)",
            short(other)
        )));
    }
    let empty = Map::new();
    let rules = match file.get("rules") {
        None => &empty,
        Some(Value::Object(rules)) => rules,
        Some(_) => return Err(refused("`rules` is no object")),
    };
    if rules.len() > MAX_PROJECT_RULES {
        return Err(refused("more than 128 rules"));
    }
    rules
        .iter()
        .map(|(id, value)| rule(id, value).map(|parsed| (id.clone(), parsed)))
        .collect()
}

fn absent(failure: &Failure) -> bool {
    failure.kind == FailureKind::NotFound
        || matches!(
            failure.code(),
            Some(codes::NOT_AVAILABLE | codes::CAPABILITY_DENIED)
        )
}

/// Whether the person trusts the workspace, as the host's trust gate on [`TRUST_PROBE`] says:
/// a listing there (or its folder missing) is trust; its refusal is none.
pub fn trusted(host: &mut dyn PluginHost) -> bool {
    match host.project_list(TRUST_PROBE) {
        Ok(_) => true,
        Err(failure) => {
            failure.code() != Some(codes::UNTRUSTED_WORKSPACE)
                && failure.kind != FailureKind::AccessDenied
        }
    }
}

/// The project's rule file in a run: none, untrusted, or its rules.
///
/// # Errors
///
/// The file does not read or is refused by [`parse`] (`errors.project_rules`).
pub fn load(host: &mut dyn PluginHost) -> Result<ProjectRules, Failure> {
    let bytes = match host.project_read(PROJECT_FILE) {
        Ok(bytes) => bytes,
        Err(failure) if absent(&failure) => return Ok(ProjectRules::default()),
        Err(failure) => {
            return Err(refused(failure.code().unwrap_or(failure.kind.as_str())));
        }
    };
    if !trusted(host) {
        host.log(
            "info",
            &format!("{PROJECT_FILE} not applied: the workspace is untrusted"),
        );
        return Ok(ProjectRules {
            state: State::Untrusted,
            rules: Rules::new(),
        });
    }
    Ok(ProjectRules::applied(parse(&bytes)?))
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
