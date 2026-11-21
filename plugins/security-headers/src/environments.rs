//! The project's environments (`environments` of `.rbrowser/config.json`, RB-010-12): which one a
//! URL belongs to, and the same page in another one (its origin, the URL's path kept - as the
//! toolbar's switch keeps it).
//!
//! Read through `project-read`, which never hands out `config.local.json`: an environment only
//! the machine-local config adds is not known here. The file is the project's, not a foreign
//! server's, but it is held to bounds all the same: at most [`MAX_ENVIRONMENTS`], ids of
//! [`MAX_ID_CHARS`] characters, `http(s)` URLs only.

use rb_plugin_api::PluginHost;
use rb_util::url::origin_of;
use serde_json::Value;

/// The project config, relative to the project root.
pub const CONFIG_PATH: &str = ".rbrowser/config.json";
/// The most environments read.
pub const MAX_ENVIRONMENTS: usize = 16;
/// The longest environment id read.
pub const MAX_ID_CHARS: usize = 64;

/// One environment: its id and origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    pub id: String,
    /// `scheme://host[:port]`, lower case, the scheme's default port dropped.
    pub origin: String,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.chars().count() <= MAX_ID_CHARS
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// The environments of the config text `bytes`; anything malformed is left out.
#[must_use]
pub fn parse(bytes: &[u8]) -> Vec<Environment> {
    let Ok(config) = serde_json::from_slice::<Value>(bytes) else {
        return Vec::new();
    };
    let Some(list) = config.get("environments").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut environments: Vec<Environment> = Vec::new();
    for entry in list {
        if environments.len() >= MAX_ENVIRONMENTS {
            break;
        }
        let id = entry.get("id").and_then(Value::as_str).unwrap_or_default();
        let origin = entry.get("url").and_then(Value::as_str).and_then(origin_of);
        if let Some(origin) = origin
            && valid_id(id)
            && !environments.iter().any(|seen| seen.id == id)
        {
            environments.push(Environment {
                id: id.to_owned(),
                origin,
            });
        }
    }
    environments
}

/// The project's environments; none when the plugin cannot read the config.
pub fn read(host: &mut dyn PluginHost) -> Vec<Environment> {
    host.project_read(CONFIG_PATH)
        .map(|bytes| parse(&bytes))
        .unwrap_or_default()
}

/// The environment whose origin `url` has.
#[must_use]
pub fn of_url<'a>(environments: &'a [Environment], url: &str) -> Option<&'a Environment> {
    let origin = origin_of(url)?;
    environments
        .iter()
        .find(|environment| environment.origin == origin)
}

/// The path, query included, of `url` (`/` when it has none), its fragment dropped.
#[must_use]
pub fn path_of(url: &str) -> String {
    let without_fragment = url.split('#').next().unwrap_or_default();
    let rest = without_fragment
        .split_once("://")
        .map_or("", |(_, rest)| rest);
    match rest.find(['/', '?']) {
        Some(at) => {
            let path = rest.get(at..).unwrap_or("/");
            if path.starts_with('?') {
                format!("/{path}")
            } else {
                path.to_owned()
            }
        }
        None => "/".to_owned(),
    }
}

/// The page `path` (from [`path_of`]) in `environment`.
#[must_use]
pub fn rebase(environment: &Environment, path: &str) -> String {
    format!("{}{path}", environment.origin)
}

#[cfg(test)]
#[path = "environments_tests.rs"]
mod tests;
