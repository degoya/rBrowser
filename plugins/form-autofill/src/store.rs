//! The presets taken from a form (RB-090-70): the plugin's own file
//! `.rbrowser/out/form-autofill/presets.json` - written only through `out.write`, so nothing
//! outside the plugin's folder, and read back through `project.read`. A preset has the shape of a
//! settings profile (`{"name", "values"}`, values by field name or id) and is used as one: a fill
//! naming it, or `activeProfile`, takes it when no settings profile has that name. A locator that
//! names a secret (`src/secret.rs`) is never stored, whoever saves - the panel or MCP.
//!
//! The file also carries the request of the command `capture` (`capture-request.json`: the view
//! and the time, no value), so the panel that the command opens - and that could not hear its post
//! yet - captures that view once it is up.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, codes};
use serde_json::{Map, Value, json};

use crate::args::bad;
use crate::key;
use crate::secret::secret_text;
use crate::settings::{MAX_VALUE, MAX_VALUES, Profile, Settings, profile};

/// The presets' file, relative to the plugin's out folder (`out.write`).
pub const FILE: &str = "presets.json";
/// The same file as `project.read` names it.
pub const READ_PATH: &str = ".rbrowser/out/form-autofill/presets.json";
/// The capture request's file (`out.write`) and its read path.
pub const REQUEST_FILE: &str = "capture-request.json";
pub const REQUEST_PATH: &str = ".rbrowser/out/form-autofill/capture-request.json";
/// How long a capture request waits for the panel, in seconds.
pub const REQUEST_SECONDS: u64 = 30;
/// The most presets stored.
pub const MAX_PRESETS: usize = 50;
/// The longest preset name, in characters (as a settings profile's).
pub const MAX_NAME: usize = 80;
/// The longest locator, in bytes (as a settings profile's).
pub const MAX_LOCATOR: usize = 200;

/// The arguments of `autofill_save`: the name, the values by locator and whether a preset of
/// that name is replaced.
#[derive(Clone, Debug, PartialEq)]
pub struct SaveArgs {
    pub name: String,
    pub values: BTreeMap<String, String>,
    pub replace: bool,
}

pub(crate) fn name_of(value: Option<&Value>) -> Result<String, Failure> {
    let name = value.and_then(Value::as_str).map(str::trim).unwrap_or("");
    if name.is_empty() || name.chars().count() > MAX_NAME || name.chars().any(char::is_control) {
        return Err(bad("name"));
    }
    Ok(name.to_owned())
}

fn values_of(value: Option<&Value>) -> Result<BTreeMap<String, String>, Failure> {
    let map = value
        .and_then(Value::as_object)
        .ok_or_else(|| bad("values"))?;
    if map.len() > MAX_VALUES {
        return Err(bad("values"));
    }
    map.iter()
        .map(|(locator, value)| match value.as_str() {
            Some(text)
                if !locator.is_empty()
                    && locator.len() <= MAX_LOCATOR
                    && text.chars().count() <= MAX_VALUE =>
            {
                Ok((locator.clone(), text.to_owned()))
            }
            _ => Err(bad("values")),
        })
        .collect()
}

impl SaveArgs {
    /// # Errors
    ///
    /// `plugins.error.invalid` with `reason` naming the argument that does not read.
    pub fn parse(args: &str) -> Result<Self, Failure> {
        let value: Value = serde_json::from_str(args).map_err(|_| bad("args"))?;
        let map = value.as_object().ok_or_else(|| bad("args"))?;
        let replace = match map.get("replace") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(replace)) => *replace,
            Some(_) => return Err(bad("replace")),
        };
        Ok(Self {
            name: name_of(map.get("name"))?,
            values: values_of(map.get("values"))?,
            replace,
        })
    }
}

/// The presets of the file's `bytes`, each as it reads, and whether every entry read whole: the
/// file is JSON, `presets` a list of at most [`MAX_PRESETS`], each entry as [`profile`] takes it
/// without dropping or trimming anything.
fn parse(bytes: &[u8]) -> (Vec<Profile>, bool) {
    let value: Value = serde_json::from_slice(bytes).unwrap_or(Value::Null);
    let Some(list) = value["presets"].as_array() else {
        return (Vec::new(), false);
    };
    let mut whole = list.len() <= MAX_PRESETS;
    let mut presets = Vec::new();
    for entry in list {
        match profile(entry) {
            Some(preset)
                if entry["name"].as_str() == Some(preset.name.as_str())
                    && entry["values"].as_object().map(Map::len) == Some(preset.values.len()) =>
            {
                presets.push(preset);
            }
            Some(preset) => {
                whole = false;
                presets.push(preset);
            }
            None => whole = false,
        }
    }
    presets.truncate(MAX_PRESETS);
    (presets, whole)
}

fn file(host: &mut dyn PluginHost) -> Result<Option<Vec<u8>>, Failure> {
    match host.project_read(READ_PATH) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(failure) if failure.code.as_deref() == Some(codes::NOT_FOUND) => Ok(None),
        Err(failure) => Err(failure),
    }
}

/// The stored presets, all of them: none while the file is missing.
///
/// # Errors
///
/// `plugin.form-autofill.errors.presets_unreadable` when the file does not read whole (not JSON,
/// an entry of another shape, more than [`MAX_PRESETS`]) - so a save never writes it anew from
/// part of it; the host's failure when it cannot be read at all (no project open).
pub fn read(host: &mut dyn PluginHost) -> Result<Vec<Profile>, Failure> {
    let Some(bytes) = file(host)? else {
        return Ok(Vec::new());
    };
    match parse(&bytes) {
        (presets, true) => Ok(presets),
        (_, false) => Err(
            Failure::coded(FailureKind::BadData, &key("errors.presets_unreadable"))
                .with_param("file", READ_PATH),
        ),
    }
}

/// The stored preset `name`, from every entry of the file that reads (a fill only reads).
#[must_use]
pub fn find(host: &mut dyn PluginHost, name: &str) -> Option<Profile> {
    let bytes = file(host).ok()??;
    parse(&bytes)
        .0
        .into_iter()
        .find(|preset| preset.name == name)
}

/// Why nothing can be stored now, for an answer: the failure's code and parameters.
#[must_use]
pub fn failure_json(failure: &Failure) -> Value {
    json!({ "code": failure.code, "params": failure.params })
}

pub(crate) fn summaries(presets: &[Profile]) -> Value {
    json!(
        presets
            .iter()
            .map(|preset| json!({
                "name": preset.name,
                "fields": preset.values.len(),
                "values": preset.values,
            }))
            .collect::<Vec<_>>()
    )
}

/// The answer of `autofill_presets`: every stored preset with its values; `available` false with
/// the reason's code as `error` when nothing can be stored (no project open, a file that does not
/// read whole).
///
/// # Errors
///
/// None so far; a store that cannot be read is an empty list with its reason.
pub fn list(host: &mut dyn PluginHost) -> Result<Value, Failure> {
    Ok(match read(host) {
        Ok(presets) => {
            json!({ "presets": summaries(&presets), "available": true, "file": READ_PATH })
        }
        Err(failure) => json!({ "presets": [], "available": false, "file": READ_PATH,
                                "error": failure.code, "params": failure.params }),
    })
}

pub(crate) fn write(host: &mut dyn PluginHost, presets: &[Profile]) -> Result<(), Failure> {
    let file = json!({
        "version": 1,
        "presets": presets
            .iter()
            .map(|preset| json!({ "name": preset.name, "values": preset.values }))
            .collect::<Vec<_>>(),
    });
    let text = serde_json::to_string_pretty(&file)
        .map_err(|_| Failure::coded(FailureKind::Other, codes::BAD_OUTPUT))?;
    host.out_write(FILE, text.as_bytes())
}

pub(crate) fn refused(kind: FailureKind, name: &str, preset: &str) -> Failure {
    Failure::coded(kind, &key(&format!("errors.{name}"))).with_param("name", preset)
}

/// Saves `args` as a preset: never over a settings profile of that name, over a stored one only
/// with `replace`, at most [`MAX_PRESETS`]; a locator that names a secret is dropped.
///
/// # Errors
///
/// `plugin.form-autofill.errors.preset_in_settings`, `preset_exists`, `presets_full`,
/// `preset_empty`; the host's failure when the store cannot be read or written.
pub fn save(host: &mut dyn PluginHost, args: &SaveArgs) -> Result<Value, Failure> {
    let settings = Settings::read(host);
    if settings.profile(&args.name).is_some() {
        return Err(refused(
            FailureKind::BadData,
            "preset_in_settings",
            &args.name,
        ));
    }
    let (values, dropped): (BTreeMap<_, _>, BTreeMap<_, _>) = args
        .values
        .clone()
        .into_iter()
        .partition(|(locator, _)| !secret_text(locator));
    if values.is_empty() {
        return Err(refused(FailureKind::BadData, "preset_empty", &args.name));
    }
    let mut presets = read(host)?;
    let at = presets.iter().position(|preset| preset.name == args.name);
    match at {
        Some(_) if !args.replace => {
            return Err(refused(FailureKind::BadData, "preset_exists", &args.name));
        }
        None if presets.len() >= MAX_PRESETS => {
            return Err(refused(FailureKind::Limit, "presets_full", &args.name));
        }
        _ => {}
    }
    let preset = Profile {
        name: args.name.clone(),
        values,
    };
    let fields = preset.values.len();
    match at {
        Some(at) => presets[at] = preset,
        None => presets.push(preset),
    }
    write(host, &presets)?;
    Ok(json!({
        "saved": args.name,
        "replaced": at.is_some(),
        "fields": fields,
        "dropped": dropped.keys().collect::<Vec<_>>(),
        "file": READ_PATH,
        "$post": [{ "target": "panel", "message": { "type": "presets", "presets": summaries(&presets) } }],
    }))
}

/// Notes that the command `capture` asked for `view`, for the panel it opens.
///
/// # Errors
///
/// The host's failure when nothing can be written (no project open).
pub fn request_capture(host: &mut dyn PluginHost, view: ViewId) -> Result<(), Failure> {
    let time = host.now_unix_seconds();
    let text = json!({ "view": view, "time": time }).to_string();
    host.out_write(REQUEST_FILE, text.as_bytes())
}

/// The view a fresh capture request names, taken once: the request is cleared as it is read.
#[must_use]
pub fn take_request(host: &mut dyn PluginHost) -> Option<ViewId> {
    let bytes = host.project_read(REQUEST_PATH).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    let request = value.as_object().filter(|map| !map.is_empty())?;
    // Cleared before it is used: a request is never taken twice, even if this call fails later.
    host.out_write(
        REQUEST_FILE,
        Value::Object(Map::new()).to_string().as_bytes(),
    )
    .ok()?;
    let view = request
        .get("view")?
        .as_u64()
        .and_then(|id| ViewId::try_from(id).ok())?;
    let time = request.get("time")?.as_u64()?;
    let now = host.now_unix_seconds();
    (time <= now && now - time <= REQUEST_SECONDS).then_some(view)
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
