//! `checks.export` (RB-092-05, `Leader K B`, the palette, the findings panel's Export button, the
//! MCP tool `checks_export`): writes the last check run as SARIF, JUnit, HTML or JSON
//! (`format`, else `checks.export.defaultFormat`) to `path` (a relative path below the export
//! folder, an absolute one inside `.rbrowser/out/` or the user's export folder,
//! [`super::target`]), or to `<run-id>.<extension>` in the export folder; answers
//! `{ path, format }` and names the file in the status bar. No run yet: refused with `checks.export.error.no_run`.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_engine::ViewHost;
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_plugin_types::FindingsFile;
use serde::Deserialize;
use serde_json::{Value, json};

use super::target::{self, PathError};
use super::{ExportContext, ExportFormat, extension, format_name, parse_format};
use crate::control::Core;

/// The command.
pub const EXPORT_COMMAND: &str = "checks.export";
/// Its MCP tool.
pub const EXPORT_TOOL: &str = "checks_export";
/// The key after `Leader K`: B for the report's file (`K E` is the error report plugin's).
const EXPORT_KEY: char = 'B';
/// The refusal without a run to export.
pub const NO_RUN_KEY: &str = "checks.export.error.no_run";
/// The status bar's word on a written export.
pub const WRITTEN_KEY: &str = "checks.export.notice.written";

/// The arguments of `checks.export` and `checks_export`.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportArgs {
    /// `sarif`, `junit`, `html` or `json`; `checks.export.defaultFormat` when absent.
    #[serde(default)]
    pub format: Option<String>,
    /// Where to write (see the module's account); the export folder when absent.
    #[serde(default)]
    pub path: Option<String>,
}

/// Why an export did not happen.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// The arguments or the path are refused.
    #[error("{0}")]
    Invalid(String),
    /// There is nowhere to write, or the write failed.
    #[error("{0}")]
    Failed(String),
}

impl From<ExportError> for CommandError {
    fn from(error: ExportError) -> Self {
        match error {
            ExportError::Invalid(text) => CommandError::InvalidArgs(text),
            ExportError::Failed(text) => CommandError::Failed(text),
        }
    }
}

/// The arguments in `args` (`null` is none), the format named.
///
/// # Errors
///
/// An unknown field or format.
pub fn parse_args(args: &Value) -> Result<(Option<ExportFormat>, Option<String>), ExportError> {
    let args: ExportArgs = if args.is_null() {
        ExportArgs::default()
    } else {
        serde_json::from_value(args.clone())
            .map_err(|error| ExportError::Invalid(error.to_string()))?
    };
    let format = match args.format.as_deref() {
        None => None,
        Some(name) => Some(parse_format(name).ok_or_else(|| {
            ExportError::Invalid(format!(
                "format must be sarif, junit, html or json, not {name:?}"
            ))
        })?),
    };
    Ok((format, args.path))
}

/// Where an export goes.
pub enum Destination {
    /// The command's and an agent's: checked against the allowed folders ([`target`]).
    Checked(Option<String>),
    /// The person's own file (`rbrowser check --out`, checked by the instance protocol).
    Own(PathBuf),
}

/// The texts of the core keys a run's failures and skips name (`plugins.error.*`,
/// `checks.skip.*`), added to `messages` so the export says them as the panel does.
pub fn with_core_texts(
    file: &FindingsFile,
    messages: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut all = messages.clone();
    let core = rb_i18n::core_catalogue();
    let keys = file
        .failures
        .iter()
        .filter_map(|failure| failure.code.clone())
        .chain(
            file.skipped
                .iter()
                .map(|skip| format!("checks.skip.{}", skip.reason)),
        );
    for key in keys {
        if let Entry::Vacant(slot) = all.entry(key)
            && let Some(text) = core.get(slot.key())
        {
            slot.insert(text.to_owned());
        }
    }
    all
}

/// The format in effect: `asked`, else the settings' (the project's over the user's).
pub fn format_of(core: &Core, asked: Option<ExportFormat>) -> ExportFormat {
    let project = core.host().config().checks.findings.export;
    let user = core.user_settings().checks.export;
    asked
        .or(project.over(&user).default_format)
        .unwrap_or_default()
}

/// Writes `file` (with the texts of its message keys) in `format` to `destination`; the path.
///
/// # Errors
///
/// The path is refused, there is nowhere to write, or the write failed.
pub fn export(
    core: &Core,
    file: &FindingsFile,
    messages: &BTreeMap<String, String>,
    format: ExportFormat,
    destination: Destination,
) -> Result<PathBuf, ExportError> {
    let host = core.host();
    let engines: BTreeMap<u32, String> = host
        .views()
        .iter()
        .map(|view| (view.id.0, view.engine.name().to_owned()))
        .collect();
    let messages = with_core_texts(file, messages);
    let context = ExportContext {
        messages: &messages,
        engines: &engines,
        text: &report_text,
    };
    let body = super::render(file, format, &context)
        .map_err(|error| ExportError::Failed(error.to_string()))?;
    let default_name = format!("{}.{}", file.run_id, extension(format));
    match destination {
        Destination::Own(path) => {
            target::write_file(&path, &body).map_err(|error| failed_write(&path, &error))?;
            Ok(path)
        }
        Destination::Checked(requested) => {
            let project = host.config().checks.findings.export;
            let user = core.user_settings().checks.export;
            let root = host.project_root();
            let bases = target::bases(root.as_deref(), project.dir.as_deref(), user.dir.as_deref())
                .map_err(path_error)?;
            let path =
                target::resolve(&bases, requested.as_deref(), &default_name).map_err(path_error)?;
            target::write_inside(&bases, &path, &body)
                .map_err(|error| failed_write(&path, &error))?;
            Ok(path)
        }
    }
}

/// A report text from the core catalogue (`checks.export.report.<key>`, [`super::REPORT_TEXTS`];
/// the literal prefix lets the i18n lint see the family).
fn report_text(key: &str, args: &[(&str, &str)]) -> String {
    rb_i18n::t_with(&format!("checks.export.report.{key}"), args)
}

fn path_error(error: PathError) -> ExportError {
    match error {
        PathError::NoBase => ExportError::Failed(rb_i18n::t("checks.export.error.no_project")),
        PathError::Refused(reason) => ExportError::Invalid(rb_i18n::t_with(
            "checks.export.error.path",
            &[("reason", &reason)],
        )),
    }
}

fn failed_write(path: &Path, error: &std::io::Error) -> ExportError {
    let reason = format!("{}: {error}", path.display());
    ExportError::Failed(rb_i18n::t_with(
        "checks.export.error.write",
        &[("reason", &reason)],
    ))
}

/// `checks.export` on the last run: the path written and the format, named in the status bar.
///
/// # Errors
///
/// See [`export`]; no run yet.
pub fn run(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let (asked, path) = parse_args(args)?;
    let (file, messages) =
        super::super::last_run().ok_or_else(|| CommandError::Failed(NO_RUN_KEY.to_owned()))?;
    let format = format_of(core, asked);
    let written = export(core, &file, &messages, format, Destination::Checked(path))?;
    let shown = written.display().to_string();
    core.notify(CoreToShell::Notification(Notification {
        level: NotificationLevel::Success,
        message_key: WRITTEN_KEY.to_owned(),
        args: BTreeMap::from([("path".to_owned(), shown.clone())]),
        settings_key: None,
    }));
    Ok(json!({ "path": shown, "format": format_name(format) }))
}

/// The definition; title key `commands.checks.export`.
///
/// # Errors
///
/// An id the registry would refuse (a test holds it valid).
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    Ok(vec![CommandDef {
        id: CommandId::new(EXPORT_COMMAND)?,
        title_key: format!("commands.{EXPORT_COMMAND}"),
        group: Group::Checks,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![Key::Char(Group::Checks.leader_key()), Key::Char(EXPORT_KEY)],
        })],
        mcp: McpCoverage::Covered(EXPORT_TOOL.to_owned()),
    }])
}

/// Registers the command with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| registry.register(def, handler(Arc::downgrade(core))).err())
        .collect()
}

fn handler(core: Weak<Core>) -> Handler {
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        run(&core, &args)
    })
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
