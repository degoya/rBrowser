//! The commands of the bug report (RB-093-04): `report.create` (`Leader C U`; `C E` is the
//! screenshot editor's), the MCP tool `bug_report`, and `report.open_folder` (`Leader C Y`), the
//! status bar's "Open folder" of the last report; the same message's "Copy Markdown" is the
//! status bar's own (`super::preview`, the core has no clipboard). A report answers its paths
//! only - never its content. On the CEF UI thread (the shell's `runCommand`) the report starts
//! and a notice names it when written; on a thread of the runtime (a shortcut, MCP) the command
//! waits for it - and for the person's word in the preview while `report.preview` is on.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_engine::ViewId;
use rb_ipc::{CoreToShell, Notification, NotificationLevel, StatusAction, StatusCopy};
use serde::Deserialize;
use serde_json::{Value, json};

use super::run::{self, ReportError, Written};
use super::{CREATE, MAX_NOTE_CHARS, OPEN_FOLDER, TOOL};
use crate::cdp::lock;
use crate::control::Core;

/// The key after `Leader C` of `report.create`: U, as in bUg (`C E` is the screenshot editor's).
const CREATE_KEY: char = 'U';
/// The key after `Leader C` of `report.open_folder`.
const OPEN_KEY: char = 'Y';
/// The notice that a report is being made.
pub const STARTED_KEY: &str = "report.notice.started";
/// The notice of a written report; placeholder `{path}`.
pub const WRITTEN_KEY: &str = "report.notice.written";
/// The notice that no report was written; placeholder `{error}`.
pub const FAILED_KEY: &str = "report.notice.failed";
/// The notice that the person cancelled a report in the preview.
pub const CANCELLED_KEY: &str = "report.notice.cancelled";
/// The status bar button that opens the report's folder.
pub const OPEN_LABEL_KEY: &str = "report.action.open_folder";
/// The status bar button that copies the report's Markdown.
pub const COPY_LABEL_KEY: &str = "report.action.copy_markdown";
/// The refusal of `report.open_folder` before the first report.
pub const NONE_KEY: &str = "report.error.none";
/// The screenshot editor's command (`report.annotate`).
pub const EDITOR_COMMAND: &str = "plugin.screenshot_editor.open_last";
/// The settings category the notices link to (Capture, where Bug report is a section).
const SETTINGS_KEY: &str = "capture";
/// The longest note taken, in bytes, before it is cut to [`MAX_NOTE_CHARS`] characters.
const MAX_NOTE_BYTES: usize = 4 * MAX_NOTE_CHARS;

/// `report.md` of the last report of this run.
static LAST: Mutex<Option<PathBuf>> = Mutex::new(None);

/// The views a report names: one id, several, or `"all"`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Targets {
    /// One view.
    One(u32),
    /// These views.
    Many(Vec<u32>),
    /// `"all"`.
    Named(String),
}

/// The arguments of `report.create` and `bug_report`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateArgs {
    /// The views; every view when absent.
    #[serde(default)]
    pub target: Option<Targets>,
    /// A sentence on what went wrong, masked like the rest.
    #[serde(default)]
    pub note: Option<String>,
}

impl CreateArgs {
    /// The views named (`None`: every view).
    ///
    /// # Errors
    ///
    /// A name that is not `"all"`, or an empty list.
    pub fn views(&self) -> Result<Option<Vec<ViewId>>, String> {
        match &self.target {
            None => Ok(None),
            Some(Targets::Named(name)) if name.eq_ignore_ascii_case("all") => Ok(None),
            Some(Targets::Named(name)) => Err(format!(
                "target is \"all\", a view id or a list of view ids, not {name:?}"
            )),
            Some(Targets::One(id)) => Ok(Some(vec![ViewId(*id)])),
            Some(Targets::Many(ids)) if ids.is_empty() => Err("target names no view".to_owned()),
            Some(Targets::Many(ids)) => Ok(Some(ids.iter().copied().map(ViewId).collect())),
        }
    }
}

/// The arguments in `args` (`null` is none).
///
/// # Errors
///
/// An unknown field, a wrong type, a note too long.
pub fn parse_args(args: &Value) -> Result<CreateArgs, String> {
    let parsed: CreateArgs = if args.is_null() {
        CreateArgs::default()
    } else {
        serde_json::from_value(args.clone()).map_err(|error| error.to_string())?
    };
    if parsed
        .note
        .as_ref()
        .is_some_and(|note| note.len() > MAX_NOTE_BYTES)
    {
        return Err(format!("note is at most {MAX_NOTE_CHARS} characters"));
    }
    parsed.views()?;
    Ok(parsed)
}

/// The answer of a written report: its paths, never its content.
pub fn answer(written: &Written) -> Value {
    json!({
        "path": written.folder.display().to_string(),
        "markdown": written.markdown.display().to_string(),
        "zip": written.zip.display().to_string(),
    })
}

impl From<ReportError> for CommandError {
    fn from(error: ReportError) -> Self {
        match error {
            ReportError::Invalid(text) => CommandError::InvalidArgs(text),
            ReportError::Failed(text) => CommandError::Failed(text),
            ReportError::Cancelled => CommandError::Failed(rb_i18n::t(CANCELLED_KEY)),
        }
    }
}

/// The definitions; title keys `commands.report.create`, `commands.report.open_folder`.
///
/// # Errors
///
/// An id the registry would refuse (a test holds them valid).
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    let leader = |key: char| {
        vec![Binding::Leader(Chord {
            keys: vec![Key::Char(Group::Capture.leader_key()), Key::Char(key)],
        })]
    };
    Ok(vec![
        CommandDef {
            id: CommandId::new(CREATE)?,
            title_key: format!("commands.{CREATE}"),
            group: Group::Capture,
            default_bindings: leader(CREATE_KEY),
            mcp: McpCoverage::Covered(TOOL.to_owned()),
        },
        CommandDef {
            id: CommandId::new(OPEN_FOLDER)?,
            title_key: format!("commands.{OPEN_FOLDER}"),
            group: Group::Capture,
            default_bindings: leader(OPEN_KEY),
            mcp: McpCoverage::Omitted(
                "opens the file manager; bug_report answers the report's path".to_owned(),
            ),
        },
    ])
}

/// Registers the commands with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| {
            let handler = handler(Arc::downgrade(core), def.id.as_str() == CREATE);
            registry.register(def, handler).err()
        })
        .collect()
}

fn handler(core: Weak<Core>, create: bool) -> Handler {
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        if create {
            run_create(&core, &args)
        } else {
            open_folder()
        }
    })
}

/// `report.create`: waits for the report on a thread of the runtime and answers its paths;
/// elsewhere starts it and answers `{ "started": true }`.
///
/// # Errors
///
/// The arguments; on a thread of the runtime also [`run::create`]'s errors.
pub fn run_create(core: &Arc<Core>, args: &Value) -> Result<Value, CommandError> {
    let args = parse_args(args).map_err(CommandError::InvalidArgs)?;
    let views = args.views().map_err(CommandError::InvalidArgs)?;
    core.notify(notice(NotificationLevel::Info, STARTED_KEY, &[]));
    let runtime = core.runtime().clone();
    if tokio::runtime::Handle::try_current().is_err() {
        let core = Arc::clone(core);
        runtime.spawn(async move {
            let result = run::create(&core, views, args.note).await;
            let _ = finish(&core, result);
        });
        return Ok(json!({ "started": true }));
    }
    let result =
        tokio::task::block_in_place(|| runtime.block_on(run::create(core, views, args.note)));
    finish(core, result).map(|written| answer(&written))
}

/// The buttons of a saved report's message: "Open folder" (the last report's) and "Copy
/// Markdown" of this one, named by its folder.
pub fn buttons(folder: &Path) -> (StatusAction, StatusCopy) {
    let action = StatusAction {
        command: OPEN_FOLDER.to_owned(),
        label_key: OPEN_LABEL_KEY.to_owned(),
        label_args: BTreeMap::new(),
        args: Value::Null,
    };
    let copy = StatusCopy {
        label_key: COPY_LABEL_KEY.to_owned(),
        report: report_name(folder),
    };
    (action, copy)
}

/// The report's name: its folder's.
fn report_name(folder: &Path) -> String {
    folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Whether the screenshot editor, which reads `.rbrowser/out/` of the project `root` only, sees
/// the report in `folder`.
pub fn editor_sees(folder: &Path, root: Option<&Path>) -> bool {
    root.is_some_and(|root| folder.starts_with(root.join(".rbrowser").join("out")))
}

/// Remembers and names a written report (with "Open folder" and "Copy Markdown"), opens the
/// screenshot editor when `report.annotate` asks and the editor can see the report; names a
/// failure, a cancel as such.
fn finish(core: &Arc<Core>, result: Result<Written, ReportError>) -> Result<Written, CommandError> {
    match result {
        Ok(written) => {
            *lock(&LAST) = Some(written.markdown.clone());
            super::preview::remember(report_name(&written.folder), written.text.clone());
            let path = written.folder.display().to_string();
            let notification = Notification {
                level: NotificationLevel::Success,
                message_key: WRITTEN_KEY.to_owned(),
                args: BTreeMap::from([("path".to_owned(), path)]),
                settings_key: Some(SETTINGS_KEY.to_owned()),
            };
            let (action, copy) = buttons(&written.folder);
            core.notify_with_copy(notification, action, copy);
            let root = core.host().project_root();
            if core.host().config().report.annotate
                && editor_sees(&written.folder, root.as_deref())
                && core.has_command(EDITOR_COMMAND)
            {
                // Off this thread: the editor's command waits for its plugin.
                core.run_command_async(EDITOR_COMMAND, Value::Null, |result| {
                    if let Err(error) = result {
                        tracing::warn!(%error, "the screenshot editor did not open");
                    }
                });
            }
            Ok(written)
        }
        Err(ReportError::Cancelled) => {
            tracing::info!("a bug report was cancelled in the preview");
            core.notify(notice(NotificationLevel::Info, CANCELLED_KEY, &[]));
            Err(ReportError::Cancelled.into())
        }
        Err(error) => {
            tracing::warn!(%error, "no bug report was written");
            core.notify(notice(
                NotificationLevel::Error,
                FAILED_KEY,
                &[("error", error.to_string())],
            ));
            Err(error.into())
        }
    }
}

/// `report.open_folder`: the last report's folder in the file manager, `report.md` selected.
fn open_folder() -> Result<Value, CommandError> {
    let markdown = lock(&LAST)
        .clone()
        .ok_or_else(|| CommandError::Failed(rb_i18n::t(NONE_KEY)))?;
    rb_diag::reveal(&markdown).map_err(|error| CommandError::Failed(error.to_string()))?;
    Ok(json!({ "path": markdown.display().to_string() }))
}

fn notice(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
        settings_key: Some(SETTINGS_KEY.to_owned()),
    })
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
