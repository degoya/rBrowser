//! The setup wizard (RB-091-05): optional, once per person, every step skippable. It lives in
//! the pages window (`#/setup`, `ui/src/firstrun/`): welcome, plugins (RB-091-04's
//! `available` / `installChosen`), project (the config `rbrowser init` writes, shown before it
//! is written), basics (theme, layout, sync), done.
//!
//! - The very first start opens it by itself ([`state::decide`], `first-run.json` beside the
//!   user settings); a project without config says "Set up project" once in the status bar,
//!   whose button opens the project step. Never a reminder, never a badge.
//! - `app.setup_wizard` (`Leader , W`, the palette, Settings > General > Start) opens it again;
//!   with `ui.firstRunWizard` off nothing opens it - neither the start nor the command.
//! - `preview`, `write` and `basics` run on the runtime's blocking pool one after the other
//!   ([`lane`]) and answer with the request's number, which the page matches.
//! - MCP can neither open nor answer it: the command is refused to MCP ([`refuse_mcp`]) and the
//!   `setup` messages come from the pages window only (`settings_window::guard`).

pub mod basics;
mod lane;
pub mod project;
pub mod state;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_config::ConfigFile;
use rb_ipc::{
    CoreToShell, IpcErrorCode, Notification, NotificationLevel, SetupAnswer, SetupLayout,
    SetupProject, SetupRequest, SetupStep, ShellToCore, StatusAction, parse_request,
};
use serde_json::{Value, json};

use crate::cdp::lock;
use crate::control::Core;
use crate::ipc::Failure;
use project::{Preview, Proposals, StepFailure};

/// The command that opens the wizard.
pub const COMMAND: &str = "app.setup_wizard";
/// Its leader key in the settings group (`Leader , W`).
pub const LEADER_KEY: char = 'W';
/// Why the command has no MCP tool.
pub const MCP_OMITTED: &str = "a person's dialog: MCP can neither open nor answer the setup wizard";
/// The pages window's route.
pub const ROUTE: &str = "setup";
/// The project step, the `step` argument of the command and the route's key.
pub const PROJECT_STEP: &str = "project";
/// The command's refusal while `ui.firstRunWizard` is off.
pub const OFF_KEY: &str = "firstrun.error.off";
/// The hint for a project without config.
pub const HINT_KEY: &str = "firstrun.hint.project";
/// Its button.
pub const HINT_ACTION_KEY: &str = "firstrun.hint.action";
/// The setting the hint links to.
pub const SETTING_KEY: &str = "ui.firstRunWizard";
/// `basics` without a project to write into.
pub const NO_PROJECT_KEY: &str = "firstrun.basics.error.no_project";
/// `basics` could not read or write the config.
pub const BASICS_FAILED_KEY: &str = "firstrun.basics.error.failed";

/// The proposals of the project step; only the last is written.
static PROPOSALS: Mutex<Proposals> = Mutex::new(Proposals::new());
/// The folder the wizard wrote a config into since the app started.
static WRITTEN: Mutex<Option<PathBuf>> = Mutex::new(None);

/// The command definition: `Leader , W`, no MCP tool.
///
/// # Errors
///
/// A malformed id, which only a broken build meets.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    Ok(vec![CommandDef {
        id: CommandId::new(COMMAND)?,
        title_key: format!("commands.{COMMAND}"),
        group: Group::Settings,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![
                Key::Char(Group::Settings.leader_key()),
                Key::Char(LEADER_KEY),
            ],
        })],
        mcp: McpCoverage::Omitted(MCP_OMITTED.to_owned()),
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
        open(&core, &args)
    })
}

/// Opens the wizard, at the project step with `{ "step": "project" }`; refused while
/// `ui.firstRunWizard` is off.
///
/// # Errors
///
/// [`OFF_KEY`] while the setting is off.
pub fn open(core: &Core, args: &Value) -> Result<Value, CommandError> {
    if !core.user_settings().ui.first_run_wizard {
        return Err(CommandError::Failed(OFF_KEY.to_owned()));
    }
    let step = args.get("step").and_then(Value::as_str);
    core.ui()
        .open_settings_window(crate::shell::page_url(&route(step)));
    Ok(Value::Null)
}

/// The route of `step`: the project step's key, else the wizard's start.
#[must_use]
pub fn route(step: Option<&str>) -> String {
    match step {
        Some(PROJECT_STEP) => format!("{ROUTE}/{PROJECT_STEP}"),
        _ => ROUTE.to_owned(),
    }
}

/// Refuses the command to an MCP client: the wizard is a person's.
///
/// # Errors
///
/// [`CommandError::Failed`] for [`COMMAND`].
pub fn refuse_mcp(id: &str) -> Result<(), CommandError> {
    if id == COMMAND {
        return Err(CommandError::Failed(format!(
            "{COMMAND} is a person's dialog: {MCP_OMITTED}"
        )));
    }
    Ok(())
}

/// Whether `request` is one of the wizard's (`setup`): the pages window's only.
#[must_use]
pub fn is_setup_request(request: &str) -> bool {
    matches!(parse_request(request), Ok(ShellToCore::Setup { .. }))
}

/// The core is up: the very first start opens the wizard, a project without config is hinted
/// once ([`state::decide`]). Runs before the open project is recorded as a recent one, so a
/// settings file that exists was there before this start.
pub fn on_start(core: &Core) {
    let Some(settings) = core.user_settings_path() else {
        return;
    };
    let path = state::path_for(settings);
    let root = core.host().project_root();
    let key = root.as_deref().map(crate::instance::file::key);
    let project = root
        .as_deref()
        .zip(key.as_deref())
        .map(|(root, key)| state::ProjectAtStart {
            key,
            has_config: ConfigFile::Project.path(root).is_file(),
        });
    let input = state::StartInput {
        enabled: core.user_settings().ui.first_run_wizard,
        settings_file_exists: settings.is_file(),
        project,
    };
    let (action, next) = state::decide(&input, state::load(&path));
    if let Some(next) = next
        && let Err(error) = state::save(&path, &next)
    {
        // Not remembered, it would show at every start: nothing shows.
        tracing::warn!(%error, "the first-run state was not saved; the wizard stays closed");
        return;
    }
    match action {
        state::StartAction::OpenWizard => {
            core.ui()
                .open_settings_window(crate::shell::page_url(ROUTE));
        }
        state::StartAction::ShowHint => hint(core, root.as_deref()),
        state::StartAction::Nothing => {}
    }
}

/// "Set up project" in the status bar, its button opens the project step.
fn hint(core: &Core, root: Option<&Path>) {
    let name = root
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    core.notify_with_action(
        Notification {
            level: NotificationLevel::Info,
            message_key: HINT_KEY.to_owned(),
            args: [("project".to_owned(), name)].into(),
            settings_key: Some(SETTING_KEY.to_owned()),
        },
        StatusAction {
            command: COMMAND.to_owned(),
            label_key: HINT_ACTION_KEY.to_owned(),
            label_args: BTreeMap::new(),
            args: json!({ "step": PROJECT_STEP }),
        },
    );
}

/// Answers the `setup` messages; `None` for any other.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let ShellToCore::Setup { request } = message else {
        return None;
    };
    Some(match request {
        SetupRequest::Info => Ok(CoreToShell::Setup { answer: info(core) }),
        SetupRequest::Close => {
            core.ui().close_settings_window();
            Ok(CoreToShell::Ack)
        }
        work => in_background(core, work.clone()).map(|()| CoreToShell::Ack),
    })
}

/// What the wizard asks first.
#[must_use]
pub fn info(core: &Core) -> SetupAnswer {
    let project = core.host().project_root().map(|root| SetupProject {
        has_config: ConfigFile::Project.path(&root).is_file(),
        root: root.to_string_lossy().into_owned(),
    });
    SetupAnswer::Info {
        enabled: core.user_settings().ui.first_run_wizard,
        project,
        written: lock(&WRITTEN)
            .as_ref()
            .map(|root| root.to_string_lossy().into_owned()),
    }
}

/// Runs a step on the blocking lane ([`lane`]: the runtime's blocking pool, one step after the
/// other in the order they came) and sends its answer, with the request's number, as a `setup`
/// event.
fn in_background(core: &Core, request: SetupRequest) -> Result<(), Failure> {
    let core = core
        .arc()
        .ok_or_else(|| (IpcErrorCode::Internal, "the window is closed".to_owned()))?;
    let runtime = core.runtime().clone();
    lane::run_in_order(&runtime, move || {
        let answer = answer(&core, &request);
        core.send(&CoreToShell::Setup { answer });
    });
    Ok(())
}

/// The answer to a `preview`, `write` or `basics` request, on this thread, with its number.
pub fn answer(core: &Core, request: &SetupRequest) -> SetupAnswer {
    match request {
        SetupRequest::Preview {
            request_id,
            root,
            url,
            template,
            devices,
        } => {
            let root = PathBuf::from(root);
            let preview = Preview {
                root: &root,
                url: url.as_deref().filter(|url| !url.trim().is_empty()),
                template: *template,
                devices,
            };
            match project::propose(&preview, crate::analyzer::init_config) {
                Ok((config, source)) => SetupAnswer::Proposal {
                    request_id: *request_id,
                    proposal_id: lock(&PROPOSALS).keep(root.clone(), config.clone()),
                    root: root.to_string_lossy().into_owned(),
                    config,
                    source,
                },
                Err(failure) => failed(*request_id, SetupStep::Preview, failure),
            }
        }
        SetupRequest::Write {
            request_id,
            proposal_id,
        } => write(core, *request_id, *proposal_id),
        SetupRequest::Basics {
            request_id,
            layout,
            sync,
        } => basics(core, *request_id, *layout, *sync),
        SetupRequest::Info | SetupRequest::Close => info(core),
    }
}

fn failed(request_id: u32, step: SetupStep, (key, args): StepFailure) -> SetupAnswer {
    SetupAnswer::Failed {
        request_id,
        step,
        message_key: key.to_owned(),
        args,
    }
}

fn is_open_project(core: &Core, root: &Path) -> bool {
    core.host()
        .project_root()
        .is_some_and(|open| crate::instance::file::key(&open) == crate::instance::file::key(root))
}

/// Writes the shown proposal; the open project reloads its config, another folder becomes a
/// recent project that `project.open_recent` opens.
fn write(core: &Core, request_id: u32, proposal_id: u32) -> SetupAnswer {
    let Some((root, config)) = lock(&PROPOSALS).take(proposal_id) else {
        return failed(
            request_id,
            SetupStep::Write,
            (project::NO_PROPOSAL_KEY, BTreeMap::new()),
        );
    };
    let path = match project::write(&root, &config) {
        Ok(path) => path,
        Err(failure) => return failed(request_id, SetupStep::Write, failure),
    };
    let open = is_open_project(core, &root);
    if open {
        if let Err(error) = crate::commands::reload_config(core.host()) {
            tracing::warn!(%error, "the config the wizard wrote did not load");
        }
    } else {
        crate::instance::recent::record(core, &root);
    }
    *lock(&WRITTEN) = Some(root.clone());
    SetupAnswer::Written {
        request_id,
        root: root.to_string_lossy().into_owned(),
        path: path.to_string_lossy().into_owned(),
        open,
    }
}

/// Writes the layout and sync into the project the wizard set up, else the open project with a
/// config.
fn basics(
    core: &Core,
    request_id: u32,
    layout: Option<SetupLayout>,
    sync: Option<bool>,
) -> SetupAnswer {
    let has_config = |root: &PathBuf| ConfigFile::Project.path(root).is_file();
    let target = lock(&WRITTEN)
        .clone()
        .filter(has_config)
        .or_else(|| core.host().project_root().filter(has_config));
    let Some(root) = target else {
        return failed(
            request_id,
            SetupStep::Basics,
            (NO_PROJECT_KEY, BTreeMap::new()),
        );
    };
    let path = ConfigFile::Project.path(&root);
    let mut document = match rb_config::read_json(&path) {
        Ok(document) => document,
        Err(error) => return basics_failed(request_id, &error.to_string()),
    };
    basics::apply(&mut document, layout, sync);
    if let Err(issues) = crate::settings_window::write::validate_project(&document, None, "") {
        let path = issues
            .first()
            .map(|issue| issue.path.clone())
            .unwrap_or_default();
        return basics_failed(request_id, &path);
    }
    if let Err(error) = rb_config::write_config_file(&root, ConfigFile::Project, &document) {
        return basics_failed(request_id, &error.to_string());
    }
    if is_open_project(core, &root)
        && let Err(error) = crate::commands::reload_config(core.host())
    {
        tracing::warn!(%error, "the config the wizard changed did not load");
    }
    SetupAnswer::Applied {
        request_id,
        root: root.to_string_lossy().into_owned(),
    }
}

fn basics_failed(request_id: u32, message: &str) -> SetupAnswer {
    failed(
        request_id,
        SetupStep::Basics,
        (
            BASICS_FAILED_KEY,
            [("message".to_owned(), message.to_owned())].into(),
        ),
    )
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
