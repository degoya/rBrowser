//! An environment renamed in the settings takes its references along (RB-094-24): the project
//! config names environments by id in `basicAuth.environments` and `envCompare.environments`,
//! the API client keys its environments by the same id (`api-client.secrets` in
//! `config.local.json`, `.rbrowser/api/http-client.env.json`), and the shell remembers the one
//! in effect by id. Without this a renamed environment lost its
//! Basic auth - its old id stayed listed - and the view fell back to the start environment.
//!
//! A rename is a write of `environments` where an entry keeps its place and its URL and only its
//! id changes to one that was not there. A new id with another URL is another environment: it
//! takes no Basic auth along to a host the person never stored a credential for. The
//! credentials themselves are bound to the project and the environment's origin
//! (`basic_auth::store::target_name`), not to its id, so they need nothing.

use rb_config::ConfigFile;
use serde_json::Value;

use super::values::ProjectDocuments;
use crate::control::Core;

/// The settings key of the environments.
pub const ENVIRONMENTS_KEY: &str = "environments";

/// The paths, in a project config file, of the lists that name environments by id.
const REFERENCES: [[&str; 2]; 2] = [
    ["basicAuth", "environments"],
    ["envCompare", "environments"],
];

/// The paths of the objects keyed by environment id (the API client's private values in
/// `config.local.json`).
const KEYED: [[&str; 2]; 1] = [["api-client", "secrets"]];

/// `(old id, new id)` of every environment `after` renames from `before` (both the
/// `environments` array as JSON; anything else renames nothing).
pub fn renames(before: &Value, after: &Value) -> Vec<(String, String)> {
    let (Some(before), Some(after)) = (before.as_array(), after.as_array()) else {
        return Vec::new();
    };
    if before.len() != after.len() {
        return Vec::new();
    }
    let id = |entry: &Value| entry.get("id").and_then(Value::as_str).map(str::to_owned);
    let url = |entry: &Value| entry.get("url").cloned();
    let had = |wanted: &str| {
        before
            .iter()
            .any(|entry| id(entry).as_deref() == Some(wanted))
    };
    let has = |wanted: &str| {
        after
            .iter()
            .any(|entry| id(entry).as_deref() == Some(wanted))
    };
    before
        .iter()
        .zip(after)
        .filter_map(|(old, new)| {
            let (old_id, new_id) = (id(old)?, id(new)?);
            (old_id != new_id
                && url(old) == url(new)
                && !has(old_id.as_str())
                && !had(new_id.as_str()))
            .then_some((old_id, new_id))
        })
        .collect()
}

/// Puts the new id in place of the old one in every reference list of `document`; whether it
/// changed anything.
pub fn rename_in(document: &mut Value, renames: &[(String, String)]) -> bool {
    let mut changed = false;
    for [section, field] in REFERENCES {
        let Some(list) = document
            .get_mut(section)
            .and_then(|section| section.get_mut(field))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        for entry in list.iter_mut() {
            let Some(listed) = entry.as_str() else {
                continue;
            };
            if let Some((_, new)) = renames.iter().find(|(old, _)| old == listed) {
                *entry = Value::String(new.clone());
                changed = true;
            }
        }
    }
    for [section, field] in KEYED {
        if let Some(map) = document
            .get_mut(section)
            .and_then(|section| section.get_mut(field))
        {
            changed |= rename_keys(map, renames);
        }
    }
    changed
}

/// Renames the keys of the object `map` in place, keeping their order; a new id that is a key
/// already is left alone (nothing is written over). Whether it changed anything.
pub fn rename_keys(map: &mut Value, renames: &[(String, String)]) -> bool {
    let Some(object) = map.as_object() else {
        return false;
    };
    let wanted = |key: &str| {
        renames
            .iter()
            .find(|(old, new)| old == key && !object.contains_key(new))
            .map(|(_, new)| new.clone())
    };
    if !object.keys().any(|key| wanted(key.as_str()).is_some()) {
        return false;
    }
    let renamed: serde_json::Map<String, Value> = object
        .iter()
        .map(|(key, value)| {
            (
                wanted(key.as_str()).unwrap_or_else(|| key.clone()),
                value.clone(),
            )
        })
        .collect();
    *map = Value::Object(renamed);
    true
}

/// The checked-in API environments (`.rbrowser/api/http-client.env.json`) follow the renames;
/// a file that does not read or write is logged and left as it is.
pub fn rename_api_environments(root: &std::path::Path, renames: &[(String, String)]) {
    if renames.is_empty() {
        return;
    }
    let store = crate::api_client::store::Store::new(root);
    let mut value = match store.env_json() {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(?error, "the API environments do not read; not renamed");
            return;
        }
    };
    if rename_keys(&mut value, renames)
        && let Err(error) = store.write_env_json(&value)
    {
        tracing::warn!(?error, "the renamed API environments were not written");
    }
}

/// What [`carry`] gives back: the document, the other project file to write with it, the renames.
pub type Carried = (Value, Option<(ConfigFile, Value)>, Vec<(String, String)>);

/// The write of `key` to `file` with the references of the environments it renames carried
/// along: in `document`, and in the other project file - written with it as `also` - when that
/// one names them. Returns the renames as well, for the environment in effect.
pub fn carry(
    key: &str,
    value: &Value,
    project: &ProjectDocuments,
    file: ConfigFile,
    mut document: Value,
    mut also: Option<(ConfigFile, Value)>,
) -> Carried {
    if key != ENVIRONMENTS_KEY {
        return (document, also, Vec::new());
    }
    let renamed = renames(&project.effective[ENVIRONMENTS_KEY], value);
    if renamed.is_empty() {
        return (document, also, renamed);
    }
    rename_in(&mut document, &renamed);
    also = match also.take() {
        Some((other_file, mut other)) => {
            rename_in(&mut other, &renamed);
            Some((other_file, other))
        }
        None => {
            let (other_file, other) = match file {
                ConfigFile::Project => (ConfigFile::Local, project.local_file.clone()),
                ConfigFile::Local => (ConfigFile::Project, Some(project.project_file.clone())),
            };
            other.and_then(|mut other| {
                rename_in(&mut other, &renamed).then_some((other_file, other))
            })
        }
    };
    (document, also, renamed)
}

/// The environment in effect follows its rename, so the views stay where they are instead of
/// falling back to the start environment when the config is applied again.
pub fn follow_in_effect(core: &Core, renamed: &[(String, String)]) {
    if renamed.is_empty() {
        return;
    }
    core.host().update_shell_state(|shell| {
        let next = shell
            .environment
            .as_deref()
            .and_then(|current| renamed.iter().find(|(old, _)| old == current))
            .map(|(_, new)| new.clone());
        if next.is_some() {
            shell.environment = next;
        }
    });
}

#[cfg(test)]
#[path = "env_rename_tests.rs"]
mod tests;
