//! The environment comparison in the window: the commands, the MCP host tools' entry points and
//! the notices when a run starts (RB-090-85) and ends. A run gets the sitemap capture's render
//! views (windowless browsers driven over the app's CDP connection,
//! `crate::sitemap::render_views`); everything runs on the core's runtime, the diff and the
//! reading of a `report.json` (up to 64 MiB) on blocking threads, and the grid's views are not
//! touched - the grid stays usable.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock, Weak};

use rb_commands::CommandError;
use rb_engine::chromium::hidden::RenderSpec;
use rb_engine::{Target, ViewHost as _};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};

use super::defs::{CANCEL, LIVE, REPORT, RUN};
use super::live::live;
use super::names::path_folder;
use super::plan::{self, PlanError};
use super::report::{EnvRun, RunEnvironment, RunPath, Summary};
use super::runs::{NewRun, RunError, RunEvents, Runs, Services};
use super::tool::{
    CANCEL_TOOL, COMPARE_TOOL, CancelAnswer, CompareArgs, LIVE_TOOL, LiveArgs, RESULT_TOOL,
    ResultArgs, Started,
};
use crate::control::Core;

/// The notice of a started run; placeholder `{shots}` (RB-090-85).
pub const STARTED_KEY: &str = "envcompare.notice.started";
/// The notice of a finished run; placeholders `{differing}`, `{diffs}`, `{folder}`.
pub const FINISHED_KEY: &str = "envcompare.notice.finished";
/// The notice of a run that could not start; placeholder `{error}`.
pub const FAILED_KEY: &str = "envcompare.notice.failed";
/// The notice of a cancelled run; placeholders `{done}`, `{shots}`, `{folder}`.
pub const CANCELLED_KEY: &str = "envcompare.notice.cancelled";
/// The notice of a report that could not be opened; placeholder `{error}`.
pub const REPORT_FAILED_KEY: &str = "envcompare.notice.report_failed";
/// The settings section the notices link to.
const SETTINGS_KEY: &str = "env-compare";

/// The runs of this process (one window).
pub fn runs() -> &'static Arc<Runs> {
    static RUNS: OnceLock<Arc<Runs>> = OnceLock::new();
    RUNS.get_or_init(Runs::new)
}

fn notice(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect::<BTreeMap<_, _>>(),
        settings_key: Some(SETTINGS_KEY.to_owned()),
    })
}

/// The window as a run's follower: one notice when it ends.
struct ShellEvents(Weak<Core>);

impl RunEvents for ShellEvents {
    fn progress(&self, run_id: &str, done: usize, total: usize) {
        tracing::debug!(run_id, done, total, "an environment comparison took a shot");
    }

    fn ended(&self, summary: &Summary) {
        let Some(core) = self.0.upgrade() else {
            return;
        };
        if summary.state == rb_visual::sitemap::RunState::Cancelled {
            let args = [
                ("done", summary.shots_done.to_string()),
                ("shots", summary.shots.to_string()),
                ("folder", summary.folder.clone()),
            ];
            core.notify(notice(NotificationLevel::Info, CANCELLED_KEY, &args));
            return;
        }
        let level = if summary.differing == 0 && summary.shots_failed == 0 {
            NotificationLevel::Info
        } else {
            NotificationLevel::Warning
        };
        let args = [
            ("differing", summary.differing.to_string()),
            ("diffs", summary.pairs.len().to_string()),
            ("folder", summary.folder.clone()),
        ];
        core.notify(notice(level, FINISHED_KEY, &args));
    }
}

/// Starts a comparison: the MCP tool `env_compare` and the command `env.compare.run`.
///
/// # Errors
///
/// No project, a call that cannot be planned, or another run in progress.
pub async fn start(core: &Arc<Core>, args: CompareArgs) -> Result<Started, RunError> {
    let clean = args.clean();
    let host = core.host().clone();
    let root = host.project_root().ok_or(RunError::NoProject)?;
    let config = host.config();
    let environments = plan::environments(
        &config.environments,
        &args.environments,
        &config.env_compare.environments,
    )?;
    let views = host.views();
    let targets: Vec<_> = args
        .views
        .iter()
        .filter_map(|id| views.iter().find(|view| view.id.0 == *id))
        .collect();
    let target_breakpoints: Vec<String> = targets
        .iter()
        .filter_map(|view| {
            view.cell_breakpoint
                .clone()
                .or_else(|| view.breakpoint.clone())
        })
        .collect();
    let ids = plan::breakpoint_ids(
        &args.breakpoints,
        &target_breakpoints,
        &config.env_compare.breakpoints,
    );
    let devices = crate::emulation::device_list();
    let trusted = host.shell_state().trusted;
    let breakpoints =
        crate::sitemap::plan::breakpoints(&plan::capture_args(ids), &config, &devices, trusted)
            .map_err(|error| PlanError::Breakpoints(error.to_string()))?;
    let specs: Vec<RenderSpec> = breakpoints
        .run
        .iter()
        .filter_map(|breakpoint| breakpoints.specs.get(&breakpoint.name).cloned())
        .collect();
    let mut paths: Vec<String> = Vec::new();
    if args.paths.is_empty() {
        let active = host.active_view();
        let shown = targets
            .first()
            .copied()
            .or_else(|| views.iter().find(|view| Some(view.id) == active))
            .or_else(|| views.first());
        paths.push(shown.map_or_else(
            || "/".to_owned(),
            |view| plan::view_path(&view.url, &config.environments),
        ));
    } else {
        for path in &args.paths {
            let path = plan::path_of(path, &config.environments)?;
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    let effective = plan::effective(&config, &args);
    let run = EnvRun {
        max_ratio: effective.max_ratio,
        environments: environments
            .iter()
            .map(|environment| RunEnvironment {
                id: environment.id.clone(),
                url: environment.url.clone(),
                color: environment.color.clone(),
            })
            .collect(),
        breakpoints: breakpoints.run,
        paths: paths
            .iter()
            .enumerate()
            .map(|(index, path)| RunPath {
                path: path.clone(),
                folder: path_folder(index, path),
            })
            .collect(),
        ..EnvRun::default()
    };
    let new = NewRun {
        root,
        now_unix: crate::sitemap::runs::now_unix(),
        run,
        specs,
        batch: effective.batch,
        options: effective.options,
        texts: Arc::new(|key: &str| rb_i18n::t(&format!("envcompare.report.{key}"))),
    };
    let services = Services {
        backend: Arc::new(
            crate::sitemap::render_views::backend_for(&host)
                .with_clean(crate::capture::clean_plan(core, clean)),
        ),
        events: Arc::new(ShellEvents(Arc::downgrade(core))),
    };
    let started = runs().start(new, services).await?;
    core.notify(started_notice(&started));
    Ok(started)
}

/// The notice that `started` runs: the person sees the comparison begin (RB-090-85).
fn started_notice(started: &Started) -> CoreToShell {
    notice(
        NotificationLevel::Info,
        STARTED_KEY,
        &[("shots", started.shots.to_string())],
    )
}

/// The state and diffs of a run: the MCP tool `env_compare_result`. A finished run's
/// `report.json` (up to 64 MiB) is read and parsed on a blocking thread, never on the caller's.
///
/// # Errors
///
/// No project, or no such run.
pub async fn result(core: &Core, args: ResultArgs) -> Result<Summary, RunError> {
    let root = core.host().project_root().ok_or(RunError::NoProject)?;
    tokio::task::spawn_blocking(move || runs().status(&root, args.run_id.as_deref()))
        .await
        .map_err(|error| RunError::Failed(error.to_string()))?
}

/// Cancels a run: the MCP tool `env_compare_cancel` and the command `env.compare.cancel`.
///
/// # Errors
///
/// No project, or no such run.
pub fn cancel(core: &Core, args: &ResultArgs) -> Result<CancelAnswer, RunError> {
    let root = core.host().project_root().ok_or(RunError::NoProject)?;
    let (run_id, cancelled) = runs().cancel(&root, args.run_id.as_deref())?;
    Ok(CancelAnswer { run_id, cancelled })
}

/// `path` as a `file:` URL (a Windows drive, a UNC share or a POSIX path).
pub fn file_url(path: &str) -> String {
    let slashed = path.replace('\\', "/");
    let encode = |text: &str| -> String {
        let mut out = String::with_capacity(text.len());
        for byte in text.bytes() {
            if byte.is_ascii_alphanumeric() || b"-._~/:".contains(&byte) {
                out.push(char::from(byte));
            } else {
                out.push_str(&format!("%{byte:02X}"));
            }
        }
        out
    };
    if let Some(share) = slashed.strip_prefix("//") {
        return format!("file://{}", encode(share));
    }
    let drive = slashed.as_bytes();
    if drive.len() >= 2 && drive[0].is_ascii_alphabetic() && drive[1] == b':' {
        return format!("file:///{}", encode(&slashed));
    }
    format!("file://{}", encode(&slashed))
}

/// `env.compare.report`: the report of the run (the latest when none is named) in the active
/// view; the run's summary.
async fn open_report(core: &Core, args: ResultArgs) -> Result<Summary, String> {
    let summary = result(core, args)
        .await
        .map_err(|error| error.to_string())?;
    let report = summary
        .report
        .as_deref()
        .ok_or_else(|| "env_compare.error.no_report".to_owned())?;
    let host = core.host();
    let view = host
        .active_view()
        .or_else(|| host.views().first().map(|view| view.id))
        .ok_or_else(|| "there is no view".to_owned())?;
    host.navigate(Target::View(view), &file_url(report))
        .map_err(|error| error.to_string())?;
    Ok(summary)
}

fn to_value(answer: &impl serde::Serialize) -> Result<Value, CommandError> {
    serde_json::to_value(answer).map_err(|error| CommandError::Failed(error.to_string()))
}

/// Runs the command `id` (registered by `super::defs`). `env.compare.run` and
/// `env.compare.report` answer at once and go on on the core's runtime (a command may come from
/// the CEF UI thread, which never reads a `report.json`); a start or an opening that fails is a
/// notice.
pub fn run(core: &Core, id: &str, args: &Value) -> Result<Value, CommandError> {
    match id {
        LIVE => {
            let args = LiveArgs::parse(args).map_err(CommandError::InvalidArgs)?;
            to_value(&live(core, &args).map_err(|error| CommandError::Failed(error.to_string()))?)
        }
        RUN => {
            let args = CompareArgs::parse(args).map_err(CommandError::InvalidArgs)?;
            let core = core
                .arc()
                .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
            let runtime = core.runtime().clone();
            runtime.spawn(async move {
                if let Err(error) = start(&core, args).await {
                    tracing::warn!(
                        code = error.code(),
                        "an environment comparison did not start"
                    );
                    let args = [("error", rb_diag::redact::redact_text(&error.to_string()))];
                    core.notify(notice(NotificationLevel::Error, FAILED_KEY, &args));
                }
            });
            Ok(json!({ "started": true }))
        }
        REPORT => {
            let args = ResultArgs::parse(args).map_err(CommandError::InvalidArgs)?;
            let core = core
                .arc()
                .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
            let runtime = core.runtime().clone();
            runtime.spawn(async move {
                if let Err(error) = open_report(&core, args).await {
                    tracing::warn!("the environment comparison report was not opened");
                    let args = [("error", rb_diag::redact::redact_text(&error))];
                    core.notify(notice(NotificationLevel::Error, REPORT_FAILED_KEY, &args));
                }
            });
            Ok(json!({ "opening": true }))
        }
        CANCEL => {
            let args = ResultArgs::parse(args).map_err(CommandError::InvalidArgs)?;
            to_value(&cancel(core, &args).map_err(|error| CommandError::Failed(error.to_string()))?)
        }
        other => Err(CommandError::UnknownCommand(other.to_owned())),
    }
}

/// The MCP tools the host runs here.
pub const TOOLS: [&str; 4] = [COMPARE_TOOL, RESULT_TOOL, LIVE_TOOL, CANCEL_TOOL];

fn invalid(message: String) -> rb_mcp::ToolFailure {
    rb_mcp::ToolFailure::new("args.invalid", message)
}

fn tool_answer(answer: &impl serde::Serialize) -> Result<Value, rb_mcp::ToolFailure> {
    serde_json::to_value(answer)
        .map_err(|error| rb_mcp::ToolFailure::new("mcp.internal", error.to_string()))
}

/// A run error as an MCP failure: its stable code, the message masked.
pub fn tool_failure(error: &RunError) -> rb_mcp::ToolFailure {
    rb_mcp::ToolFailure::new(
        error.code(),
        rb_diag::redact::redact_text(&error.to_string()),
    )
}

/// Runs the MCP tool `name` (one of [`TOOLS`]) with `args`; the run on the core's runtime.
///
/// # Errors
///
/// Arguments that do not read, the run's refusal, or an unknown name.
pub async fn host_tool(
    core: &Arc<Core>,
    name: &str,
    args: Value,
) -> Result<Value, rb_mcp::ToolFailure> {
    match name {
        COMPARE_TOOL => {
            let args = CompareArgs::parse(&args).map_err(invalid)?;
            let run = core.clone();
            let started = core
                .runtime()
                .spawn(async move { start(&run, args).await })
                .await
                .map_err(|error| rb_mcp::ToolFailure::new("mcp.internal", error.to_string()))?
                .map_err(|error| tool_failure(&error))?;
            tool_answer(&started)
        }
        RESULT_TOOL => {
            let args = ResultArgs::parse(&args).map_err(invalid)?;
            tool_answer(
                &result(core, args)
                    .await
                    .map_err(|error| tool_failure(&error))?,
            )
        }
        CANCEL_TOOL => {
            let args = ResultArgs::parse(&args).map_err(invalid)?;
            tool_answer(&cancel(core, &args).map_err(|error| tool_failure(&error))?)
        }
        LIVE_TOOL => {
            let args = LiveArgs::parse(&args).map_err(invalid)?;
            tool_answer(&live(core, &args).map_err(|error| tool_failure(&error))?)
        }
        other => Err(rb_mcp::ToolFailure::new(
            "tool.unknown",
            format!("the host runs no tool `{other}`"),
        )),
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
