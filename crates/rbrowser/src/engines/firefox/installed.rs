//! The installed Firefox editions in the app (RB-080-32): what `rb_bidi::launcher::find_installed`
//! finds, shaped for the settings field `engines.firefox.path` (the options the window offers,
//! the placeholder, a configured path that is gone), for MCP `engines_list` and for
//! `rbrowser engines firefox --list`. A read only: nothing here starts Firefox or writes a
//! setting. The shaping takes the list and the world as parameters, so it is tested as is.

use std::path::Path;

use rb_bidi::launcher::{Installed, Kind};
use rb_mcp::ToolFailure;
use serde::Serialize;
use serde_json::{Value, json};

use crate::control::Core;

/// The host tool that lists the installations.
pub const LIST_TOOL: &str = "engines_list";

/// What the settings window shows at `engines.firefox.path` beside the schema.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldData {
    /// The installations, in search order: the first is what an empty field starts.
    pub options: Vec<FieldOption>,
    /// The configured path, when it names no file any more.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing: Option<String>,
}

/// One choice of the field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FieldOption {
    /// The executable, what the setting takes.
    pub value: String,
    /// `<edition> <version> - <path>`.
    pub label: String,
}

/// The edition's name for the person.
pub fn kind_name(kind: Kind) -> String {
    rb_i18n::t(&format!("engines.firefox.kind.{}", kind.id()))
}

/// `<edition> <version> - <path>`, the version left out when it is unknown; only the
/// edition's name is a text of the catalogue.
pub fn label_of(installed: &Installed) -> String {
    let path = installed.path.display();
    let kind = kind_name(installed.kind);
    match &installed.version {
        Some(version) => format!("{kind} {version} - {path}"),
        None => format!("{kind} - {path}"),
    }
}

/// The configured path when it is set and names no file.
fn missing_of(configured: Option<&str>, exists: impl Fn(&Path) -> bool) -> Option<String> {
    configured
        .map(str::trim)
        .filter(|path| !path.is_empty() && !exists(Path::new(path)))
        .map(str::to_owned)
}

/// The field's data for `installed` and the configured path.
pub fn field_data(
    installed: &[Installed],
    configured: Option<&str>,
    exists: impl Fn(&Path) -> bool,
) -> FieldData {
    FieldData {
        options: installed
            .iter()
            .map(|found| FieldOption {
                value: found.path.display().to_string(),
                label: label_of(found),
            })
            .collect(),
        missing: missing_of(configured, exists),
    }
}

/// `{ "firefox": { "configured": <path>|null, "missing"?: true, "installed": [...] } }`, the
/// answer of `engines_list` and of `--json` on the command line.
pub fn list_json(
    installed: &[Installed],
    configured: Option<&str>,
    exists: impl Fn(&Path) -> bool,
) -> Value {
    let configured = configured.map(str::trim).filter(|path| !path.is_empty());
    let mut firefox = json!({
        "configured": configured,
        "installed": installed,
    });
    if let (Some(_), Some(object)) = (missing_of(configured, exists), firefox.as_object_mut()) {
        object.insert("missing".to_owned(), Value::Bool(true));
    }
    json!({ "firefox": firefox })
}

/// The lines of `rbrowser engines firefox --list`: `configured\t<ok|missing>\t<path>` when a
/// path is set, then `<kind>\t<version|->\t<path>` per installation.
pub fn text_lines(
    installed: &[Installed],
    configured: Option<&str>,
    exists: impl Fn(&Path) -> bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(path) = configured.map(str::trim).filter(|path| !path.is_empty()) {
        let state = if exists(Path::new(path)) {
            "ok"
        } else {
            "missing"
        };
        lines.push(format!("configured\t{state}\t{path}"));
    }
    lines.extend(installed.iter().map(|found| {
        format!(
            "{}\t{}\t{}",
            found.kind.id(),
            found.version.as_deref().unwrap_or("-"),
            found.path.display()
        )
    }));
    lines
}

/// The field's data for `core`'s settings, as the settings window's schema payload carries it.
pub fn field_for(core: &Core) -> Value {
    let settings = core.user_settings();
    let data = field_data(
        &installed(),
        settings.engines.firefox_executable(),
        Path::is_file,
    );
    serde_json::to_value(data).unwrap_or_else(|_| json!({ "options": [] }))
}

/// Answers `engines_list`.
///
/// # Errors
///
/// `invalid_arguments` when the call carries arguments (it takes none).
pub fn host_tool(core: &Core, args: &Value) -> Result<Value, ToolFailure> {
    if args.as_object().is_some_and(|object| !object.is_empty()) {
        return Err(ToolFailure::new(
            "invalid_arguments",
            "engines_list takes no arguments",
        ));
    }
    let settings = core.user_settings();
    // A tokio worker answers MCP: the enumeration may run here, and the cache follows.
    Ok(list_json(
        &refresh(),
        settings.engines.firefox_executable(),
        Path::is_file,
    ))
}

/// The installations as last enumerated (`None` before the first [`refresh`]): the settings
/// answer runs on the UI thread and never walks the registry or the disk itself.
#[cfg(not(test))]
static CACHE: std::sync::Mutex<Option<Vec<Installed>>> = std::sync::Mutex::new(None);

/// Every Firefox installed on this machine, in search order, as last enumerated; empty before
/// the first [`refresh`].
#[cfg(not(test))]
pub fn installed() -> Vec<Installed> {
    crate::cdp::lock(&CACHE).clone().unwrap_or_default()
}

/// Enumerates the machine (the registry, the disk) and keeps the result for [`installed`];
/// blocking, so off the UI thread: [`refresh_in_background`], or a tokio worker (MCP).
#[cfg(not(test))]
pub fn refresh() -> Vec<Installed> {
    let found = rb_bidi::launcher::find_installed();
    *crate::cdp::lock(&CACHE) = Some(found.clone());
    found
}

#[cfg(test)]
pub fn refresh() -> Vec<Installed> {
    installed()
}

/// Enumerates on a blocking thread of the core's runtime: at the core's start and after every
/// settings answer, so the window shows the machine as of its last opening at the latest.
pub fn refresh_in_background(core: &Core) {
    drop(core.runtime().spawn_blocking(refresh));
}

/// Unit tests never read the machine: the list a test hands in, else none.
#[cfg(test)]
pub(crate) static TEST_INSTALLED: std::sync::Mutex<Vec<Installed>> =
    std::sync::Mutex::new(Vec::new());

#[cfg(test)]
pub fn installed() -> Vec<Installed> {
    crate::cdp::lock(&TEST_INSTALLED).clone()
}

#[cfg(test)]
#[path = "installed_tests.rs"]
mod tests;
