//! The panel's Variables tab (RB-096-11): `.rbrowser/api/http-client.env.json` read and written
//! whole, `$shared` and each environment a table; a row marked secret lives in the private values
//! (`api-client.secrets` in `config.local.json`) instead, never in the checked-in file. The
//! private values reach the panel as names only; a change to them is a patch (a value set, a
//! name or a whole environment dropped, an environment deleted or renamed), and the core writes the result
//! as the settings window does. A save over a file that changed since the panel read it is
//! refused (`env_changed`), so a teammate's edit pulled meanwhile is never lost unsaid.
//!
//! "Set value" for a `{{$secret name}}` ([`set_secret`]) writes one private value; [`missing_secrets`]
//! says which of a request's `$secret` names have none in the private values nor as
//! `RB_SECRET_<NAME>` ([`super::resolve`] reads them in that order).

use std::collections::BTreeMap;
use std::path::Path;

use rb_api_model::EnvFile;
use rb_api_model::env::SHARED;
use rb_api_model::vars::is_name;
use rb_plugin_types::{Failure, FailureKind};
use serde_json::{Map, Value, json};

use super::error;
use super::ops::{Context, environment_of, layers};
use super::protocol::SecretText;
use super::resolve::{EnvReader, private_secret, secret_variable};
use super::store::Store;

/// The longest private value set from the panel, in bytes (the setting holds 8192 characters).
pub const MAX_VALUE_BYTES: usize = 8 * 1024;
/// The most `$secret` names one check asks about.
pub const MAX_NAMES: usize = 100;

/// The private values' patch, by environment: `null` drops the environment, a name to `null`
/// drops the name, a name to a text sets it.
pub type PrivatePatch = BTreeMap<String, Option<BTreeMap<String, Option<SecretText>>>>;

/// A save of the Variables tab.
#[derive(Clone, Debug, PartialEq)]
pub struct Wanted {
    /// The whole environment file to write.
    pub file: Value,
    /// The file as the panel read it (`null` for none): the save is refused when it changed.
    pub base: Value,
    /// The private values' patch.
    pub private: PrivatePatch,
    /// Environments renamed, old to new: their private values go along, before the patch.
    pub renamed: BTreeMap<String, String>,
    /// Environments deleted: their private values go first, before the renames, so another
    /// environment may take a deleted one's name in the same save.
    pub deleted: Vec<String>,
}

fn bad_variable(name: &str) -> Failure {
    error(FailureKind::BadData, "bad_variable").with_param("name", rb_util::text::cut(name, 80))
}

fn environment_ok(name: &str) -> bool {
    name == SHARED || (!name.is_empty() && name.len() <= 80 && !name.chars().any(char::is_control))
}

fn unreadable(reason: &str) -> Failure {
    error(FailureKind::BadData, "env_unreadable")
        .with_param("reason", rb_util::text::cut(reason, 200))
}

/// The private values as an object of environments (an empty one for none).
fn private_map(ctx: &Context) -> Map<String, Value> {
    match &ctx.secrets {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    }
}

/// The tab's content: the file as JSON, the private names by environment, each rBrowser
/// environment's URL (its `{{baseUrl}}`).
///
/// # Errors
///
/// [`Store::env_json`]'s.
pub fn read(ctx: &Context, store: &Store) -> Result<Value, Failure> {
    let file = store.env_json()?;
    let private: Map<String, Value> = private_map(ctx)
        .into_iter()
        .map(|(environment, names)| {
            let names: Vec<String> = names
                .as_object()
                .map(|names| names.keys().cloned().collect())
                .unwrap_or_default();
            (environment, json!(names))
        })
        .collect();
    let base_urls: Map<String, Value> = ctx
        .environments
        .iter()
        .map(|(id, url)| (id.clone(), json!(url.trim_end_matches('/'))))
        .collect();
    Ok(json!({ "file": file, "private": private, "baseUrls": base_urls }))
}

/// Checks the tab's save and answers the private values to keep and the file to write after
/// them ([`commit`]): a row moved to the private values is never lost from both.
///
/// # Errors
///
/// `env_changed`, `env_unreadable` (not an environment file), `bad_variable` (a private name,
/// value or environment that cannot be one), `exists` (an environment renamed onto one the
/// private values hold that is neither renamed away nor deleted itself).
pub fn save(ctx: &Context, store: &Store, wanted: Wanted) -> Result<(Value, Value), Failure> {
    let current = store.env_json()?;
    if current != wanted.base {
        return Err(error(FailureKind::BadData, "env_changed"));
    }
    EnvFile::from_json(&wanted.file).map_err(|reason| unreadable(&reason))?;
    let mut private = private_map(ctx);
    // The deleted blocks go before the renames (code review w33, item 3).
    for environment in &wanted.deleted {
        private.remove(environment);
    }
    // Every renamed block is taken out first, so a swap moves both (the w32 review).
    let mut moved = Vec::new();
    for (from, to) in &wanted.renamed {
        if !environment_ok(to) {
            return Err(bad_variable(to));
        }
        moved.push((to, private.remove(from)));
    }
    for (to, block) in moved {
        if private.contains_key(to) {
            return Err(error(FailureKind::BadData, "exists")
                .with_param("name", rb_util::text::cut(to, 80)));
        }
        if let Some(block) = block {
            private.insert(to.clone(), block);
        }
    }
    for (environment, patch) in wanted.private {
        if !environment_ok(&environment) {
            return Err(bad_variable(&environment));
        }
        let Some(patch) = patch else {
            private.remove(&environment);
            continue;
        };
        let block = private
            .entry(environment)
            .or_insert_with(|| Value::Object(Map::new()));
        let Value::Object(block) = block else {
            return Err(unreadable("a private environment is not an object"));
        };
        for (name, value) in patch {
            if !is_name(&name) {
                return Err(bad_variable(&name));
            }
            match value {
                Some(SecretText(value)) if value.len() <= MAX_VALUE_BYTES => {
                    block.insert(name, Value::String(value));
                }
                Some(_) => return Err(bad_variable(&name)),
                None => {
                    block.remove(&name);
                }
            }
        }
    }
    private.retain(|_, block| block.as_object().is_none_or(|names| !names.is_empty()));
    let private = Value::Object(private);
    EnvFile::from_json(&private).map_err(|reason| unreadable(&reason))?;
    Ok((private, wanted.file))
}

/// Keeps a save's private values (`keep`, the window's write to `config.local.json`), then
/// writes its environment file below `root`: the file only once the values are kept, so a row
/// moved from the file to the private values stays in the file when keeping them fails. When
/// the file then is not written, the private values are kept as they were (`previous`) again
/// (code review w33, item 2): a row moved the other way is not lost, a rename not half done.
///
/// # Errors
///
/// `keep`'s, `no_project` for a file without a project, [`Store::write_env_json`]'s.
pub fn commit(
    root: Option<&Path>,
    previous: &Value,
    secrets: Option<&Value>,
    file: Option<&Value>,
    mut keep: impl FnMut(&Value) -> Result<(), Failure>,
) -> Result<(), Failure> {
    if let Some(secrets) = secrets {
        keep(secrets)?;
    }
    let Some(file) = file else {
        return Ok(());
    };
    let written = root
        .ok_or_else(|| error(FailureKind::NotFound, "no_project"))
        .and_then(|root| Store::new(root).write_env_json(file));
    if let Err(failure) = written {
        if secrets.is_some()
            && let Err(back) = keep(previous)
        {
            tracing::warn!(
                code = back.code().unwrap_or(""),
                "the private values were not put back after the environment file failed"
            );
        }
        return Err(failure);
    }
    Ok(())
}

/// The private values with `name` set to `value` in `environment` (`$shared` without one).
///
/// # Errors
///
/// `bad_variable` for a name or environment that cannot be one, or a value too long.
pub fn set_secret(
    ctx: &Context,
    name: &str,
    value: &str,
    environment: Option<&str>,
) -> Result<Value, Failure> {
    let name = name.trim();
    let environment = environment
        .filter(|e| !e.trim().is_empty())
        .unwrap_or(SHARED);
    if !is_name(name) || value.len() > MAX_VALUE_BYTES {
        return Err(bad_variable(name));
    }
    if !environment_ok(environment) {
        return Err(bad_variable(environment));
    }
    let mut private = private_map(ctx);
    let block = private
        .entry(environment.to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    let Value::Object(block) = block else {
        return Err(unreadable("a private environment is not an object"));
    };
    block.insert(name.to_owned(), Value::String(value.to_owned()));
    let private = Value::Object(private);
    EnvFile::from_json(&private).map_err(|reason| unreadable(&reason))?;
    Ok(private)
}

/// Which of `names` (`{{$secret name}}`) have no value in `environment` (the views' without
/// one): not in the private values, not as `RB_SECRET_<NAME>`.
///
/// # Errors
///
/// `too_many_names`, and [`layers`]'.
pub fn missing_secrets(
    ctx: &Context,
    store: &Store,
    names: &[String],
    environment: Option<String>,
    env: &EnvReader,
) -> Result<Vec<String>, Failure> {
    if names.len() > MAX_NAMES {
        return Err(error(FailureKind::Limit, "too_many_names").with_param("most", MAX_NAMES));
    }
    let layers = layers(ctx, store, environment_of(ctx, environment))?;
    Ok(names
        .iter()
        .filter(|name| {
            private_secret(&layers, name).is_none()
                && secret_variable(name)
                    .and_then(|variable| env(&variable))
                    .is_none()
        })
        .cloned()
        .collect())
}

#[cfg(test)]
#[path = "variables_tests.rs"]
mod tests;
