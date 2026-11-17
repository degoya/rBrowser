//! A stored preset deleted or renamed (RB-090-71): the tools `delete` and `rename`, which the
//! panel's preset choice calls and MCP reaches as `autofill_delete` and `autofill_rename`. Both
//! touch only the plugin's own file (`src/store.rs`), never a settings profile; a file that does
//! not read whole is never written anew, as a save never writes it. Both post the presets left
//! to the panel, so every open panel offers the same choice.

use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde_json::{Value, json};

use crate::args::bad;
use crate::settings::{Profile, Settings};
use crate::store::{self, READ_PATH, name_of, refused, summaries};

/// The arguments of `delete` (`name`) and `rename` (`name` and `to`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameArgs {
    pub name: String,
    pub to: Option<String>,
}

impl NameArgs {
    /// # Errors
    ///
    /// `plugins.error.invalid` with `reason` naming the argument that does not read.
    pub fn parse(args: &str) -> Result<Self, Failure> {
        let value: Value = serde_json::from_str(args).map_err(|_| bad("args"))?;
        let map = value.as_object().ok_or_else(|| bad("args"))?;
        let to = match map.get("to") {
            None | Some(Value::Null) => None,
            Some(to) => Some(name_of(Some(to)).map_err(|_| bad("to"))?),
        };
        Ok(Self {
            name: name_of(map.get("name"))?,
            to,
        })
    }
}

/// The stored presets and where `name` is among them.
fn find(host: &mut dyn PluginHost, name: &str) -> Result<(Vec<Profile>, usize), Failure> {
    let presets = store::read(host)?;
    let at = presets
        .iter()
        .position(|preset| preset.name == name)
        .ok_or_else(|| refused(FailureKind::NotFound, "preset_unknown", name))?;
    Ok((presets, at))
}

/// The panel's post of the presets now stored.
fn post(presets: &[Profile]) -> Value {
    json!([{ "target": "panel", "message": { "type": "presets", "presets": summaries(presets) } }])
}

/// Deletes the stored preset `args.name`.
///
/// # Errors
///
/// `plugin.form-autofill.errors.preset_unknown` (no stored preset of that name - a settings
/// profile is none), `presets_unreadable`; the host's failure when the store cannot be read or
/// written.
pub fn delete(host: &mut dyn PluginHost, args: &NameArgs) -> Result<Value, Failure> {
    let (mut presets, at) = find(host, &args.name)?;
    presets.remove(at);
    store::write(host, &presets)?;
    Ok(json!({ "deleted": args.name, "file": READ_PATH, "$post": post(&presets) }))
}

/// Renames the stored preset `args.name` to `args.to`, in its place: never onto another stored
/// preset or a settings profile's name. The same name again changes nothing.
///
/// # Errors
///
/// `plugins.error.invalid` (no `to`), `plugin.form-autofill.errors.preset_unknown`,
/// `preset_exists`, `preset_in_settings`, `presets_unreadable`; the host's failure when the store
/// cannot be read or written.
pub fn rename(host: &mut dyn PluginHost, args: &NameArgs) -> Result<Value, Failure> {
    let to = args.to.clone().ok_or_else(|| bad("to"))?;
    let (mut presets, at) = find(host, &args.name)?;
    if to != args.name {
        if Settings::read(host).profile(&to).is_some() {
            return Err(refused(FailureKind::BadData, "preset_in_settings", &to));
        }
        if presets.iter().any(|preset| preset.name == to) {
            return Err(refused(FailureKind::BadData, "preset_exists", &to));
        }
        presets[at].name.clone_from(&to);
        store::write(host, &presets)?;
    }
    Ok(json!({ "renamed": args.name, "to": to, "file": READ_PATH, "$post": post(&presets) }))
}

#[cfg(test)]
#[path = "preset_edit_tests.rs"]
mod tests;
