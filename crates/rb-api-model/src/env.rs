//! The API environments, in the JetBrains HTTP Client's form: `.rbrowser/api/http-client.env.json`
//! is checked in, `{"$shared": {...}, "<environment>": {"name": "value"}}`; the private values
//! (tokens, passwords) have the same form but live in `config.local.json` (the setting
//! `api-client.secrets`), never in a checked-in file. An environment is named like the project's
//! rBrowser environment (its id), so the environment the views show picks the API's too; the
//! rBrowser environment's URL is `{{baseUrl}}` unless the file sets one for it.
//!
//! The order a name is looked up in ([`Layers::variable`]): the private values of the
//! environment, the public ones of the environment, the private `$shared`, the public `$shared`,
//! then `baseUrl` from the rBrowser environment. A private value is a secret.

use std::collections::BTreeMap;

use crate::vars::{Lookup, Value, is_name};

/// The block every environment shares.
pub const SHARED: &str = "$shared";
/// The checked-in environment file, below `.rbrowser/api/`.
pub const ENV_FILE: &str = "http-client.env.json";
/// The variable the rBrowser environment's URL fills.
pub const BASE_URL: &str = "baseUrl";
/// The most environments, and variables per environment.
pub const MAX_ENTRIES: usize = 200;

/// Environments and their variables.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnvFile {
    environments: BTreeMap<String, BTreeMap<String, String>>,
}

impl EnvFile {
    /// The file's text.
    ///
    /// # Errors
    ///
    /// Not JSON, or not of the form ([`EnvFile::from_json`]).
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|error| format!("not JSON: {error}"))?;
        Self::from_json(&value)
    }

    /// The form: an object of environments, each an object of names to a string, number or
    /// boolean; `null` is an empty file.
    ///
    /// # Errors
    ///
    /// Another shape, a name that is no variable's, too many entries.
    pub fn from_json(value: &serde_json::Value) -> Result<Self, String> {
        let environments = match value {
            serde_json::Value::Null => return Ok(Self::default()),
            serde_json::Value::Object(map) => map,
            _ => return Err("not an object of environments".to_owned()),
        };
        if environments.len() > MAX_ENTRIES {
            return Err("too many environments".to_owned());
        }
        let mut out = BTreeMap::new();
        for (environment, variables) in environments {
            let serde_json::Value::Object(variables) = variables else {
                return Err(format!("environment {environment} is not an object"));
            };
            if variables.len() > MAX_ENTRIES {
                return Err(format!("environment {environment} has too many variables"));
            }
            let mut held = BTreeMap::new();
            for (name, value) in variables {
                if !is_name(name) {
                    return Err(format!("{name} is no variable name"));
                }
                let text = match value {
                    serde_json::Value::String(text) => text.clone(),
                    serde_json::Value::Number(number) => number.to_string(),
                    serde_json::Value::Bool(flag) => flag.to_string(),
                    _ => return Err(format!("{name} is no string, number or boolean")),
                };
                held.insert(name.clone(), text);
            }
            out.insert(environment.clone(), held);
        }
        Ok(Self { environments: out })
    }

    /// The environments it names, without `$shared`.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.environments
            .keys()
            .filter(|name| *name != SHARED)
            .cloned()
            .collect()
    }

    /// The value of `name` in `environment`.
    #[must_use]
    pub fn get(&self, environment: &str, name: &str) -> Option<&str> {
        self.environments
            .get(environment)?
            .get(name)
            .map(String::as_str)
    }

    /// The names of `environment` and `$shared`, each once.
    #[must_use]
    pub fn variable_names(&self, environment: Option<&str>) -> Vec<String> {
        let mut names: Vec<String> = [Some(SHARED), environment]
            .into_iter()
            .flatten()
            .filter_map(|block| self.environments.get(block))
            .flat_map(|variables| variables.keys().cloned())
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

/// The variables of one request's send: both files, the environment, the rBrowser URL.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layers {
    /// `.rbrowser/api/http-client.env.json`.
    pub public: EnvFile,
    /// The private values (`api-client.secrets`).
    pub private: EnvFile,
    /// The environment in effect, by name.
    pub environment: Option<String>,
    /// The rBrowser environment's URL: `{{baseUrl}}` unless a file sets it.
    pub base_url: Option<String>,
}

impl Layers {
    /// The names a request may use here, sorted; a secret's name is listed, never its value.
    #[must_use]
    pub fn names(&self) -> Vec<(String, bool)> {
        let environment = self.environment.as_deref();
        let mut names: Vec<(String, bool)> = self
            .public
            .variable_names(environment)
            .into_iter()
            .map(|name| (name, false))
            .collect();
        for name in self.private.variable_names(environment) {
            names.retain(|(known, _)| *known != name);
            names.push((name, true));
        }
        if self.base_url.is_some() && !names.iter().any(|(name, _)| name == BASE_URL) {
            names.push((BASE_URL.to_owned(), false));
        }
        names.sort();
        names
    }
}

impl Lookup for Layers {
    fn variable(&self, name: &str) -> Option<Value> {
        let order = [
            (&self.private, self.environment.as_deref(), true),
            (&self.public, self.environment.as_deref(), false),
            (&self.private, Some(SHARED), true),
            (&self.public, Some(SHARED), false),
        ];
        for (file, block, secret) in order {
            let Some(text) = block.and_then(|block| file.get(block, name)) else {
                continue;
            };
            return Some(if secret {
                Value::secret(text)
            } else {
                Value::plain(text)
            });
        }
        (name == BASE_URL)
            .then(|| self.base_url.clone())
            .flatten()
            .map(Value::plain)
    }

    fn dynamic(&self, _name: &str, _args: &str) -> Option<Value> {
        None
    }
}

#[cfg(test)]
#[path = "env_tests.rs"]
mod tests;
