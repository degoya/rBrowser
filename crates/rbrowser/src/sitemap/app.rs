//! Sitemap capture in the window: the commands, the MCP host tools' entry points and the shell.
//! A run gets the bundled plugin `sitemap-capture` ([`BundledTools`]), render views as windowless
//! browsers of the window driven over the app's CDP connection (`super::render_views`) and the
//! window as its follower ([`ShellEvents`]: the toolbar's progress through the shell state after
//! every shot, one notice when it ends); a start says so (`super::feedback`). Everything runs on
//! the core's runtime; the grid's views are not touched, so the grid stays usable during a run.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, Weak};

use rb_commands::CommandError;
use rb_ipc::{CoreToShell, Notification, NotificationLevel, SitemapRunState, SitemapStatus};
use rb_visual::sitemap::{Progress, RunState};
use rb_visual::sitemap_store::OUT_DIR;
use rb_visual::{BaselineStore, DiffOptions};
use serde_json::{Value, json};

use super::defs::{CANCEL, CAPTURE, GALLERY, PANEL};
use super::plan::{self, PlanError};
use super::runs::{Ended, NewRun, PluginTools, RunError, RunEvents, Runs, Services, Setup, Start};
use super::settings::Effective;
use super::sink::CompareWith;
use super::tool::{
    CANCEL_TOOL, CAPTURE_TOOL, CancelAnswer, CaptureArgs, CaptureStarted, GALLERY_TOOL, RunArgs,
    STATUS_TOOL, StatusAnswer,
};
use super::{feedback, gallery};
use crate::control::Core;

/// The plugin that finds the pages and renders the gallery.
pub const PLUGIN_ID: &str = "sitemap-capture";
/// The notice of a finished run; placeholders `{done}`, `{failed}`, `{total}`, `{folder}`.
pub const FINISHED_KEY: &str = "sitemap.notice.finished";
/// The notice of a cancelled run; placeholders `{done}`, `{total}`, `{runId}`.
pub const CANCELLED_KEY: &str = "sitemap.notice.cancelled";
/// The notice of a run that could not start; placeholder `{error}`.
pub const FAILED_KEY: &str = "sitemap.notice.failed";
/// The settings section the notices link to.
const SETTINGS_KEY: &str = "sitemap";

/// The runs of this process (one window).
pub fn runs() -> &'static Arc<Runs> {
    static RUNS: OnceLock<Arc<Runs>> = OnceLock::new();
    RUNS.get_or_init(Runs::new)
}

/// The bundled plugin's tools.
pub struct BundledTools;

impl PluginTools for BundledTools {
    fn discover(&self, args: &Value) -> Result<Value, String> {
        crate::analyzer::plugins::bundled()
            .call_tool(PLUGIN_ID, "discover", args)
            .map_err(|error| error.to_string())
    }

    fn gallery(&self, run: &Value) -> Result<String, String> {
        let answer = crate::analyzer::plugins::bundled()
            .call_tool(PLUGIN_ID, "gallery", &json!({ "run": run }))
            .map_err(|error| error.to_string())?;
        answer
            .get("html")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "plugins.error.bad_output".to_owned())
    }
}

/// The window as a run's follower.
pub struct ShellEvents {
    core: Weak<Core>,
    root: PathBuf,
}

/// The shell's view of a run.
pub fn shell_status(
    run_id: &str,
    state: RunState,
    progress: Progress,
    folder: String,
) -> SitemapStatus {
    SitemapStatus {
        run_id: run_id.to_owned(),
        state: match state {
            RunState::Running => SitemapRunState::Running,
            RunState::Cancelled => SitemapRunState::Cancelled,
            RunState::Finished => SitemapRunState::Finished,
        },
        total: progress.total,
        done: progress.done,
        failed: progress.failed,
        eta_seconds: (state == RunState::Running)
            .then_some(progress.eta_seconds)
            .flatten(),
        folder,
        gallery: None,
    }
}

impl ShellEvents {
    fn folder(&self, run_id: &str) -> String {
        let mut folder = self.root.clone();
        folder.extend(OUT_DIR);
        folder.push(run_id);
        folder.display().to_string()
    }
}

impl RunEvents for ShellEvents {
    fn progress(&self, run_id: &str, progress: Progress) {
        let Some(core) = self.core.upgrade() else {
            return;
        };
        let status = shell_status(run_id, RunState::Running, progress, self.folder(run_id));
        core.host()
            .update_shell_state(|shell| shell.sitemap = Some(status));
    }

    fn ended(&self, ended: &Ended) {
        let Some(core) = self.core.upgrade() else {
            return;
        };
        let folder = ended.folder.display().to_string();
        let mut status = shell_status(&ended.run_id, ended.state, ended.progress, folder.clone());
        status.gallery = ended
            .gallery
            .as_ref()
            .map(|path| path.display().to_string());
        core.host()
            .update_shell_state(|shell| shell.sitemap = Some(status));
        let progress = ended.progress;
        let (level, key, args) = if ended.state == RunState::Cancelled {
            let args = [
                ("done", progress.done.to_string()),
                ("total", progress.total.to_string()),
                ("runId", ended.run_id.clone()),
            ];
            (NotificationLevel::Info, CANCELLED_KEY, args)
        } else {
            let level = if progress.failed == 0 {
                NotificationLevel::Info
            } else {
                NotificationLevel::Warning
            };
            let args = [
                ("done", progress.done.to_string()),
                ("failed", progress.failed.to_string()),
                ("folder", folder),
            ];
            (level, FINISHED_KEY, args)
        };
        let mut args: Vec<(&str, String)> = args.to_vec();
        args.push(("total", progress.total.to_string()));
        core.notify(notice(level, key, &args));
    }
}

/// A notice of sitemap capture, linked to its settings.
pub(super) fn notice(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> CoreToShell {
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

/// Starts (or, with `resume`, continues) a capture: the MCP tool `sitemap_capture` and the
/// command `sitemap.capture`. Answers once the pages are found; the person sees the start
/// (`super::feedback`).
///
/// # Errors
///
/// No project, a call that cannot become a run, another run in progress, the plugin failed.
pub async fn capture(core: &Arc<Core>, args: CaptureArgs) -> Result<CaptureStarted, RunError> {
    let clean = args.clean();
    let host = core.host().clone();
    let root = host.project_root().ok_or(RunError::NoProject)?;
    let config = host.config();
    let effective = Effective::new(&config.sitemap, &args);
    let trusted = host.shell_state().trusted;
    let devices = crate::emulation::device_list();
    let start = match &args.resume {
        Some(run_id) => {
            // The run's own breakpoints: every one the config has now, by file name.
            let every = CaptureArgs {
                session: args.session.clone(),
                ..CaptureArgs::default()
            };
            let breakpoints = plan::breakpoints(&every, &config, &devices, trusted).or_else(
                |error| match error {
                    PlanError::NoBreakpoints => Ok(plan::Breakpoints::default()),
                    other => Err(other),
                },
            )?;
            Start::Resume {
                run_id: run_id.clone(),
                specs: breakpoints.specs,
            }
        }
        None => {
            let breakpoints = plan::breakpoints(&args, &config, &devices, trusted)?;
            let (call, project, folder) = (args.clone(), config.clone(), root.clone());
            let max_bytes = effective.max_bytes;
            let source = tokio::task::spawn_blocking(move || {
                plan::source(&call, &project, &folder, max_bytes)
            })
            .await
            .map_err(|error| RunError::Failed(error.to_string()))??;
            let rewrite = plan::rewrite(&args, &config, &source)?;
            let (limits, selection) = effective.discover_args();
            let mut discover =
                json!({ "source": source.source, "limits": limits, "selection": selection });
            if let Some(rewrite) = rewrite {
                discover["rewrite"] = rewrite;
            }
            Start::New(NewRun {
                discover,
                source: source.label,
                env: args.env.clone(),
                breakpoints,
            })
        }
    };
    let compare = args.compare.then(|| {
        let visual = crate::visual::options(&config.visual, None);
        CompareWith {
            store: BaselineStore::new(&root),
            strategy: visual.slug,
            options: DiffOptions {
                threshold: visual.threshold,
                include_aa: false,
            },
            machine: crate::visual::machine_meta(super::runs::now_unix()),
            full_page: config.sitemap.full_page,
        }
    });
    let setup = Setup {
        root: root.clone(),
        now_unix: super::runs::now_unix(),
        batch: effective.batch,
        full_page: config.sitemap.full_page,
        compare,
    };
    let services = Services {
        tools: Arc::new(BundledTools),
        backend: Arc::new(
            super::render_views::backend_for(&host)
                .with_clean(crate::capture::clean_plan(core, clean))
                .with_shot_timeout(effective.shot_timeout),
        ),
        events: Arc::new(ShellEvents {
            core: Arc::downgrade(core),
            root,
        }),
    };
    let started = runs().start(setup, start, services).await?;
    feedback::started(core, &started);
    Ok(started)
}

/// The status of a run: the MCP tool `sitemap_status`.
///
/// # Errors
///
/// No project, or no such run.
pub fn status(core: &Core, args: &RunArgs) -> Result<StatusAnswer, RunError> {
    let root = core.host().project_root().ok_or(RunError::NoProject)?;
    runs().status(&root, args.run_id.as_deref())
}

/// Cancels a run: the MCP tool `sitemap_cancel` and the command `sitemap.cancel`.
///
/// # Errors
///
/// No project, or no such run.
pub fn cancel(core: &Core, args: &RunArgs) -> Result<CancelAnswer, RunError> {
    let root = core.host().project_root().ok_or(RunError::NoProject)?;
    let (run_id, cancelled) = runs().cancel(&root, args.run_id.as_deref())?;
    Ok(CancelAnswer { run_id, cancelled })
}

fn to_value(answer: &impl serde::Serialize) -> Result<Value, CommandError> {
    serde_json::to_value(answer).map_err(|error| CommandError::Failed(error.to_string()))
}

fn failed(error: &RunError) -> CommandError {
    CommandError::Failed(error.to_string())
}

/// Runs the command `id` (registered by `super::defs`). `sitemap.capture` answers at once and
/// goes on on the core's runtime (a command may come from the CEF UI thread); a start that fails
/// is a notice, a start while a capture runs shows that one (`super::feedback::busy`).
pub fn run(core: &Core, id: &str, args: &Value) -> Result<Value, CommandError> {
    match id {
        CAPTURE => {
            let args = CaptureArgs::parse(args).map_err(CommandError::InvalidArgs)?;
            let core = core
                .arc()
                .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
            let runtime = core.runtime().clone();
            runtime.spawn(async move {
                match capture(&core, args).await {
                    Ok(_) => {}
                    Err(RunError::Busy) => feedback::busy(&core),
                    Err(error) => {
                        tracing::warn!(code = error.code(), "a sitemap capture did not start");
                        let args = [("error", error.to_string())];
                        core.notify(notice(NotificationLevel::Error, FAILED_KEY, &args));
                    }
                }
            });
            Ok(json!({ "started": true }))
        }
        CANCEL => {
            let args = RunArgs::parse(args).map_err(CommandError::InvalidArgs)?;
            to_value(&cancel(core, &args).map_err(|error| failed(&error))?)
        }
        GALLERY => {
            let args = RunArgs::parse(args).map_err(CommandError::InvalidArgs)?;
            let opened = gallery::open(core, &args, gallery::Caller::Person);
            to_value(&opened.map_err(|error| failed(&error))?)
        }
        PANEL => {
            core.send(&CoreToShell::UiCommand {
                id: PANEL.to_owned(),
                args: None,
            });
            let args = RunArgs::parse(args).map_err(CommandError::InvalidArgs)?;
            match status(core, &args) {
                Ok(answer) => to_value(&answer),
                Err(RunError::NotFound(_) | RunError::NoProject) => Ok(Value::Null),
                Err(error) => Err(failed(&error)),
            }
        }
        other => Err(CommandError::UnknownCommand(other.to_owned())),
    }
}

/// How long finding the pages may take for a command-line call (the plugin's own time limit).
pub const DISCOVER_BUDGET: std::time::Duration = std::time::Duration::from_secs(300);

/// [`capture`] for the instance channel (`rbrowser capture sitemap`), from a thread that is no
/// async task: run on the core's runtime and waited for, at most [`DISCOVER_BUDGET`]; the answer
/// is the `CaptureStarted`.
///
/// # Errors
///
/// The run's refusal (its code first), or no answer in time.
pub fn capture_blocking(core: &Arc<Core>, args: CaptureArgs) -> Result<Value, String> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let run = core.clone();
    core.runtime().spawn(async move {
        let started = capture(&run, args).await;
        let _ = sender.send(started);
    });
    let started = receiver
        .recv_timeout(DISCOVER_BUDGET)
        .map_err(|_| "the sitemap was not read in time".to_owned())?
        .map_err(|error| rb_diag::redact::redact_text(&error.to_string()))?;
    serde_json::to_value(started).map_err(|error| error.to_string())
}

/// The MCP tools the host runs here.
pub const TOOLS: [&str; 4] = [CAPTURE_TOOL, STATUS_TOOL, CANCEL_TOOL, GALLERY_TOOL];

fn invalid(message: String) -> rb_mcp::ToolFailure {
    rb_mcp::ToolFailure::new("invalid_arguments", message)
}

fn tool_answer(answer: &impl serde::Serialize) -> Result<Value, rb_mcp::ToolFailure> {
    serde_json::to_value(answer)
        .map_err(|error| rb_mcp::ToolFailure::new("internal", error.to_string()))
}

/// Runs the MCP tool `name` (one of [`TOOLS`]) with `args`; the capture on the core's runtime.
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
        CAPTURE_TOOL => {
            let args = CaptureArgs::parse(&args).map_err(invalid)?;
            let run = core.clone();
            let started = core
                .runtime()
                .spawn(async move { capture(&run, args).await })
                .await
                .map_err(|error| rb_mcp::ToolFailure::new("internal", error.to_string()))?
                .map_err(|error| tool_failure(&error))?;
            tool_answer(&started)
        }
        STATUS_TOOL => {
            let args = RunArgs::parse(&args).map_err(invalid)?;
            tool_answer(&status(core, &args).map_err(|error| tool_failure(&error))?)
        }
        CANCEL_TOOL => {
            let args = RunArgs::parse(&args).map_err(invalid)?;
            tool_answer(&cancel(core, &args).map_err(|error| tool_failure(&error))?)
        }
        GALLERY_TOOL => {
            let args = RunArgs::parse(&args).map_err(invalid)?;
            let opened = gallery::open(core, &args, gallery::Caller::Agent);
            tool_answer(&opened.map_err(|error| tool_failure(&error))?)
        }
        other => Err(rb_mcp::ToolFailure::new(
            "unknown_tool",
            format!("the host runs no tool `{other}`"),
        )),
    }
}

/// A run error as an MCP failure: its stable code, the message masked.
pub fn tool_failure(error: &RunError) -> rb_mcp::ToolFailure {
    rb_mcp::ToolFailure::new(
        error.code(),
        rb_diag::redact::redact_text(&error.to_string()),
    )
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
