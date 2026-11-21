//! The app side of the findings (RB-060-01). `checks.run_all` (`Leader K K`, the MCP tool
//! `checks_run`) and the panel's `runChecks` run every loaded check plugin over the views of the
//! target with `rb-plugin-host`'s check runner ([`run`]): the settings are the project's
//! `checks` keys over the user's, the run is written to `.rbrowser/out/checks/<run-id>/` when a
//! project is open, pushed to the findings panel as `findings` with the texts of its message
//! keys, and `checks.run_all` answers `ChecksRunAnswer`. `highlightFinding` marks a finding's
//! element in the views it names ([`highlight`]).
//!
//! The plugins run on a thread of their own (`rb-checks`), never on the UI thread, one run at a
//! time; no check plugin loaded is an empty run, not an error. While it goes the panel hears of
//! every plugin x view job and gets the findings of each plugin as it is through;
//! `checks.cancel` stops it with what it found so far ([`progress`], RB-080-89). A run of chosen
//! checks over the views and page of the last run is merged into it ([`selection`], RB-094-17).

pub mod cancel;
pub mod catalog;
pub mod export;
pub mod highlight;
pub mod merge;
pub mod progress;
pub mod reported;
pub mod run;
pub mod selection;

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime};

use rb_commands::CommandError;
use rb_engine::{EngineError, ViewHost, ViewId};
use rb_ipc::{CoreToShell, IpcErrorCode, Notification, NotificationLevel, ShellToCore};
use rb_plugin_host::{
    Cancel, ChecksRunAnswer, ChecksRunArgs, ChecksSettings, RunControl, RunOutcome, RunRequest,
    estimate_ms,
};
use rb_plugin_types::FindingsFile;
use serde_json::{Value, json};

use crate::analyzer::plugins::{self, NoRuntime};
use crate::cdp::lock;
use crate::control::Core;
use crate::ipc::Failure;

/// The command of a check run.
pub const RUN_ALL_COMMAND: &str = "checks.run_all";
/// The i18n key of the refusal while a run is in progress.
pub const BUSY_KEY: &str = "checks.error.busy";
/// The i18n key of the notice when a run could not be written.
pub const NOT_WRITTEN_KEY: &str = "checks.notice.not_written";

static BUSY: AtomicBool = AtomicBool::new(false);
/// The last run as it went to the shell, for a findings window that opens later.
static LAST: Mutex<Option<CoreToShell>> = Mutex::new(None);

/// Whether a run is in progress (the context menu disables "Run all checks" then, RB-080-43).
pub fn is_busy() -> bool {
    BUSY.load(Ordering::Acquire)
}

/// Holds [`BUSY`] while one run is in progress.
struct Running;

impl Running {
    fn start() -> Option<Self> {
        BUSY.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self)
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

/// Why a run did not start.
#[derive(Debug, thiserror::Error)]
enum StartError {
    #[error("{key}", key = BUSY_KEY)]
    Busy,
    #[error("the window is closed")]
    Closed,
    #[error("the check thread did not start: {0}")]
    Thread(std::io::Error),
}

/// One run, prepared on the calling thread: what to run, with which settings, where to write.
struct Job {
    request: RunRequest,
    settings: ChecksSettings,
    root: Option<PathBuf>,
    /// What the project turned off, named in the panel.
    project_off: rb_ipc::ProjectOff,
    /// Whether a run of chosen checks merges into the last run (`runChecks`, `checks.run_all`);
    /// `visual.compare`'s requested run replaces it.
    merge: bool,
}

/// Runs the command `id` when it is this module's (`checks.run_all`); `None` otherwise.
/// `checks.cancel` registers its own handler ([`cancel`]).
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    (id == RUN_ALL_COMMAND).then(|| run_all(core, args))
}

/// Answers `message` when it is this module's (`runChecks`, `highlightFinding`,
/// `getFindings`); `None` otherwise.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let started = match message {
        ShellToCore::GetFindings => {
            return Some(Ok(lock(&LAST).clone().unwrap_or(CoreToShell::Ack)));
        }
        ShellToCore::GetChecksProgress => {
            return Some(Ok(
                progress::last_progress().map_or(CoreToShell::Ack, CoreToShell::ChecksProgress)
            ));
        }
        ShellToCore::RunChecks {
            target,
            plugins,
            url,
        } => start_from_shell(
            core,
            &json!({ "target": target, "plugins": plugins, "url": url }),
        ),
        ShellToCore::HighlightFinding {
            finding_id,
            selector,
            views,
        } => start_highlight(core, finding_id, selector, views),
        _ => return None,
    };
    Some(started.map(|()| CoreToShell::Ack))
}

/// `checks.run_all`: runs and waits for the run (the caller is a command thread, not the UI).
fn run_all(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let args = parse(args).map_err(CommandError::InvalidArgs)?;
    let outcome = run_blocking(core, args)?;
    serde_json::to_value(ChecksRunAnswer::from_outcome(&outcome))
        .map_err(|error| CommandError::Failed(error.to_string()))
}

/// Runs `args` on the thread `rb-checks` and waits for the run - shown, written and remembered
/// as the last run like every other (`checks.run_all`, `rbrowser check`, RB-092-05). The caller
/// is a command or instance thread, never the UI.
///
/// # Errors
///
/// An unknown view (`InvalidArgs`), another run in progress or no runtime (`Failed`).
pub(crate) fn run_blocking(core: &Core, args: ChecksRunArgs) -> Result<RunOutcome, CommandError> {
    let job = prepare(core, args).map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
    start(core, job)
        .map_err(|error| CommandError::Failed(error.to_string()))?
        .join()
        .map_err(|_| CommandError::Failed("the check run ended without an answer".to_owned()))?
        .map_err(|error| CommandError::Failed(error.to_string()))
}

/// The last run as the panel got it, and the texts of its message keys (RB-092-05); `None`
/// before the first run or when it does not read back.
pub(crate) fn last_run() -> Option<(FindingsFile, BTreeMap<String, String>)> {
    match lock(&LAST).clone()? {
        CoreToShell::Findings { run, messages, .. } => serde_json::from_value(run)
            .ok()
            .map(|file| (file, messages)),
        _ => None,
    }
}

/// `runChecks`: checked and started here, answered at once; the run follows as `findings`.
fn start_from_shell(core: &Core, args: &Value) -> Result<(), Failure> {
    let args = parse(args).map_err(|error| (IpcErrorCode::BadRequest, error))?;
    let job = prepare(core, args).map_err(|error| {
        let code = match error {
            EngineError::UnknownView(_) => IpcErrorCode::NotFound,
            _ => IpcErrorCode::BadRequest,
        };
        (code, error.to_string())
    })?;
    match start(core, job) {
        Ok(_) => Ok(()),
        Err(StartError::Busy) => Err((IpcErrorCode::Refused, BUSY_KEY.to_owned())),
        Err(error) => Err((IpcErrorCode::Internal, error.to_string())),
    }
}

/// The arguments of `checks_run`, checked; none is every plugin over every view.
fn parse(args: &Value) -> Result<ChecksRunArgs, String> {
    if args.is_null() {
        return Ok(ChecksRunArgs::default());
    }
    ChecksRunArgs::parse(args).map_err(|failure| failure.to_string())
}

/// The run of `args`: the target's views (an unknown view is refused), the settings in effect
/// and the project to write to.
fn prepare(core: &Core, args: ChecksRunArgs) -> Result<Job, EngineError> {
    let host = core.host();
    let views = host
        .target_views(Some(run::engine_target(args.target)))?
        .into_iter()
        .filter_map(|id| host.view(id))
        .map(|info| run::run_view(&info))
        .collect();
    let (project, user) = (host.config().checks.findings, core.user_settings().checks);
    Ok(Job {
        request: run::request(args, views, SystemTime::now()),
        settings: run::settings(&project, &user),
        root: host.project_root(),
        project_off: run::project_off(&project, &user),
        merge: true,
    })
}

/// Starts `job` on the thread `rb-checks`, which borrows the core's runtime for the plugin
/// calls, unless a run is in progress.
fn start(core: &Core, job: Job) -> Result<JoinHandle<Result<RunOutcome, NoRuntime>>, StartError> {
    let running = Running::start().ok_or(StartError::Busy)?;
    let core = core.arc().ok_or(StartError::Closed)?;
    let runtime = core.runtime().clone();
    std::thread::Builder::new()
        .name("rb-checks".to_owned())
        .spawn(move || {
            let _running = running;
            let _runtime = runtime.enter();
            execute(&core, job)
        })
        .map_err(StartError::Thread)
}

/// Why a requested run (`visual.compare`'s) did not run.
#[derive(Debug, thiserror::Error)]
pub(crate) enum RequestError {
    /// Another check run is in progress.
    #[error("{key}", key = BUSY_KEY)]
    Busy,
    /// The thread has no runtime.
    #[error(transparent)]
    NoRuntime(#[from] NoRuntime),
}

/// Runs `request` as a check run on the calling thread (a blocking one of the runtime, never the
/// UI or an async task) with the settings in effect: written, pushed to the panel, answered.
/// `visual.compare` (RB-050-04) reports its comparisons through it.
///
/// # Errors
///
/// Another check run is in progress ([`RequestError::Busy`], `checks.error.busy`), or the
/// thread has no runtime ([`NoRuntime`]): nothing ran.
pub(crate) fn run_request(core: &Core, request: RunRequest) -> Result<FindingsFile, RequestError> {
    // One check run at a time: the cancel, the progress and the last run are the app's one.
    let _running = Running::start().ok_or(RequestError::Busy)?;
    let host = core.host();
    let (project, user) = (host.config().checks.findings, core.user_settings().checks);
    let job = Job {
        request,
        settings: run::settings(&project, &user),
        root: host.project_root(),
        project_off: run::project_off(&project, &user),
        merge: false,
    };
    Ok(execute(core, job)?.file)
}

/// Runs `job` with the bundled check plugins - the panel told as it goes, the status bar at its
/// start and end - writes it, pushes it to the panel; the run.
fn execute(core: &Core, job: Job) -> Result<RunOutcome, NoRuntime> {
    let trusted = core.host().shell_state().trusted;
    let sources = run::check_sources(&plugins::bundled().report.loaded, trusted);
    let Job {
        request,
        settings,
        root,
        project_off,
        merge,
    } = job;
    let plugins: Vec<String> = sources
        .iter()
        .map(|source| source.plugin_id().to_owned())
        .filter(|id| {
            request
                .plugins
                .as_ref()
                .is_none_or(|asked| asked.contains(id))
        })
        .collect();
    let views = request.views.len();
    let breakpoints: BTreeMap<u32, String> = request
        .views
        .iter()
        .filter_map(|view| Some((view.id, view.breakpoint.clone()?)))
        .collect();
    // A run of chosen checks merges into the last run as it goes and at its end (RB-094-17).
    let base = if merge {
        let last = selection::shown(lock(&LAST).as_ref());
        selection::Base::of(last, progress::last_progress().as_ref(), &request, &plugins)
            .map(Arc::new)
    } else {
        None
    };
    let cancel = Arc::new(Cancel::default());
    let sink = core.arc().map(|arc| {
        let estimate = estimate_ms(&progress::timings(), &plugins, settings.parallel());
        progress::PanelSink::start(
            arc,
            request.run_id.clone(),
            breakpoints,
            estimate,
            project_off.clone(),
            Arc::clone(&cancel),
            base.clone(),
        )
    });
    let control = RunControl {
        cancel,
        sink: sink
            .as_ref()
            .map(|sink| Arc::new(progress::Shared(Arc::clone(sink))) as _),
    };
    progress::notice(
        core,
        NotificationLevel::Info,
        progress::STARTED_KEY,
        &[
            ("plugins", plugins.len().to_string()),
            ("views", views.max(1).to_string()),
        ],
    );
    let started = Instant::now();
    let ran = plugins::block_on(run::execute(
        &sources,
        &settings,
        request,
        root.as_deref(),
        &control,
    ));
    let (outcome, not_written) = match ran {
        Ok(ran) => ran,
        Err(error) => {
            if let Some(sink) = &sink {
                sink.close();
            }
            return Err(error);
        }
    };
    if let Some(sink) = &sink {
        sink.finish(&outcome);
    }
    progress::remember_timings(&outcome);
    if let Some(failure) = not_written {
        tracing::warn!(%failure, "the check run was not written");
        core.notify(CoreToShell::Notification(Notification {
            level: NotificationLevel::Warning,
            message_key: NOT_WRITTEN_KEY.to_owned(),
            args: Default::default(),
            settings_key: None,
        }));
    }
    let file = &outcome.file;
    let texts = |of: &FindingsFile| run::messages(of, rb_i18n::plugin_texts);
    let (shown, messages) = match &base {
        Some(base) => {
            let (merged, messages) = base.run(file, texts);
            (Cow::Owned(merged), messages)
        }
        None => (Cow::Borrowed(file), texts(file)),
    };
    match serde_json::to_value(&*shown) {
        Ok(run) => {
            let message = CoreToShell::Findings {
                run,
                messages,
                project_off,
                partial: false,
            };
            core.send(&message);
            *lock(&LAST) = Some(message);
        }
        Err(error) => tracing::warn!(%error, "the check run cannot be shown"),
    }
    let (level, key) = if outcome.cancelled {
        (NotificationLevel::Info, progress::CANCELLED_KEY)
    } else {
        (NotificationLevel::Success, progress::FINISHED_KEY)
    };
    progress::notice(
        core,
        level,
        key,
        &[
            ("count", file.summary.total.to_string()),
            ("seconds", progress::seconds_text(started.elapsed())),
        ],
    );
    Ok(outcome)
}

/// `highlightFinding`: checked here, the CDP calls on the runtime (answered at once).
fn start_highlight(
    core: &Core,
    finding_id: &str,
    selector: &str,
    views: &[ViewId],
) -> Result<(), Failure> {
    let host = core.host().clone();
    let known: Vec<ViewId> = host.views().iter().map(|view| view.id).collect();
    highlight::validate(finding_id, selector, views, &known)
        .map_err(|error| (IpcErrorCode::BadRequest, error.to_string()))?;
    let (selector, views) = (selector.to_owned(), views.to_vec());
    core.runtime().spawn(async move {
        for (view, outcome) in highlight::highlight_all(&*host, &views, &selector).await {
            match outcome {
                Ok(true) => {}
                Ok(false) => tracing::debug!(%view, "the finding's element is not in the view"),
                // The selector is a plugin's text: not logged.
                Err(error) => tracing::debug!(%view, %error, "the finding was not marked"),
            }
        }
    });
    Ok(())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
