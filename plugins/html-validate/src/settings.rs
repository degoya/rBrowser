//! The plugin's settings (`settings.schema.json`, Settings > Checks > HTML validation): the
//! source validated (served or rendered, RB-094-06), the engine, the Nu HTML Checker's address,
//! rules switched off and the lowest severity reported. A trusted project's rule file
//! (`crate::project`) is added to them for a run and takes precedence over the rules off.
//! Read through `settings.get`, held to the schema's bounds again here - a value that does not fit
//! fails the run with the key named.
//!
//! The address is a user setting only (`x-scope: ["user"]`): a project's checked-in config cannot
//! send its pages' source anywhere. It is an `http` or `https` URL with a host, no credentials, no
//! query and no fragment; [`nu_endpoint`] adds the `/?out=json` the checker answers JSON to.

use rb_plugin_api::{Failure, FailureKind, PluginHost, Severity};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::key;
use crate::project::{Level, ProjectRules};

/// The most rules `disabledRules` lists.
pub const MAX_RULES: usize = 64;
/// The longest address.
pub const MAX_ADDRESS: usize = 2048;

/// Which validator checks the source.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Engine {
    /// html-validate, offline in the view's isolated world.
    #[default]
    Builtin,
    /// The Nu HTML Checker at `nuAddress`.
    Nu,
}

impl Engine {
    /// The name in settings and answers.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Nu => "nu",
        }
    }
}

/// Which source of the page is validated.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// The document as the server sent it, as the W3C validator reads it: findings by line.
    #[default]
    Raw,
    /// The DOM after rendering, serialized in the view: findings by selector.
    Rendered,
}

impl Source {
    /// The name in settings and answers.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Rendered => "rendered",
        }
    }
}

/// The effective settings of one run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub source: Source,
    pub engine: Engine,
    /// The address as set, trimmed; `None` when empty. Checked by `check_address` when a run
    /// uses the Nu engine.
    pub nu_address: Option<String>,
    pub disabled_rules: Vec<String>,
    pub min_severity: Severity,
    /// The project's rule file, when the run read one (`project::load`); not a setting.
    pub project: ProjectRules,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            source: Source::Raw,
            engine: Engine::Builtin,
            nu_address: None,
            disabled_rules: Vec::new(),
            min_severity: Severity::Warning,
            project: ProjectRules::default(),
        }
    }
}

/// `plugin.html-validate.errors.bad_setting` for key `name`.
pub fn bad_setting(name: &str, reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, &key("errors", "bad_setting"))
        .with_param("key", name)
        .with_param("reason", reason)
}

fn read<T: DeserializeOwned>(
    host: &mut dyn PluginHost,
    name: &str,
    fallback: T,
) -> Result<T, Failure> {
    match host.setting(name) {
        None => Ok(fallback),
        Some(text) if text.trim() == "null" => Ok(fallback),
        Some(text) => {
            serde_json::from_str(&text).map_err(|error| bad_setting(name, &error.to_string()))
        }
    }
}

/// Holds an address to its form; `None` for an empty one. The refusal never repeats the address,
/// which could hold a password.
///
/// # Errors
///
/// The address is no `http`/`https` URL with a host, or has credentials, a query or a fragment.
pub fn check_address(address: &str) -> Result<Option<String>, Failure> {
    let address = address.trim();
    if address.is_empty() {
        return Ok(None);
    }
    let refused = |reason: &str| bad_setting("nuAddress", reason);
    if address.len() > MAX_ADDRESS {
        return Err(refused("longer than 2048 characters"));
    }
    if address.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(refused("holds whitespace or control characters"));
    }
    let lower = address.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .ok_or_else(|| refused("only http and https addresses"))?;
    let authority = rest.split('/').next().unwrap_or_default();
    if authority.is_empty() {
        return Err(refused("no host"));
    }
    if authority.contains('@') {
        return Err(refused("no user name or password in the address"));
    }
    if rest.contains(['?', '#']) {
        return Err(refused("no query or fragment"));
    }
    Ok(Some(address.to_owned()))
}

/// The URL the checker is asked at: the address with a closing `/` and `?out=json`.
#[must_use]
pub fn nu_endpoint(address: &str) -> String {
    let slash = if address.ends_with('/') { "" } else { "/" };
    format!("{address}{slash}?out=json")
}

impl Settings {
    /// The effective settings; the schema's default for a key the host has no value for.
    ///
    /// # Errors
    ///
    /// A value that does not fit its key ([`bad_setting`]).
    pub fn read(host: &mut dyn PluginHost) -> Result<Self, Failure> {
        let defaults = Self::default();
        let address: String = read(host, "nuAddress", String::new())?;
        let settings = Self {
            source: read(host, "source", defaults.source)?,
            engine: read(host, "engine", defaults.engine)?,
            // Checked when the Nu engine runs (`Plan::new`): a bad address never stops builtin.
            nu_address: Some(address.trim())
                .filter(|address| !address.is_empty())
                .map(str::to_owned),
            disabled_rules: read(host, "disabledRules", defaults.disabled_rules)?,
            min_severity: read(host, "minSeverity", defaults.min_severity)?,
            project: ProjectRules::default(),
        };
        if settings.disabled_rules.len() > MAX_RULES {
            return Err(bad_setting("disabledRules", "more than 64 rules"));
        }
        if let Some(id) = settings
            .disabled_rules
            .iter()
            .find(|id| !rb_plugin_api::check::valid_rule_id(id) || id.contains(char::is_uppercase))
        {
            return Err(bad_setting(
                "disabledRules",
                &format!(
                    "`{}` is no rule id",
                    id.chars().take(64).collect::<String>()
                ),
            ));
        }
        Ok(settings)
    }

    /// Whether a message of `severity` is reported.
    #[must_use]
    pub fn keeps(&self, severity: Severity) -> bool {
        severity >= self.min_severity
    }

    /// Whether rule `id` is switched off: by the project's rule file when it sets the rule
    /// (`off`, or a level that switches it on), else by `disabledRules`.
    #[must_use]
    pub fn disabled(&self, id: &str) -> bool {
        match self.project.level(id) {
            Some(level) => level == Level::Off,
            None => self.disabled_rules.iter().any(|rule| rule == id),
        }
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
