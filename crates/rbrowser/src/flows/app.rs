//! The app's side of flows: the run that is going on and the last one, starting a run on the
//! core's runtime (from a command, the instance channel of `rbrowser flow run`, or MCP), the
//! command handlers, and the notices in the shell. One run at a time: two flows in the same views
//! would act on each other.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use rb_commands::{CommandError, Handler};
use rb_engine::{ViewHost as _, ViewId};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot, watch};

use super::plan::Runnable;
pub use super::plan::resolve_views;
use super::result::{RESULT_FILE, RunResult, run_dir, run_id, valid_run_id};
use super::run::{Endpoint, FlowDeps, InlineFlow, RunRequest, Setup, execute_recorded};
use super::sync_pause::SyncPause;
use super::{
    CANCEL, FlowError, OPEN_RESULTS, PANEL, RERUN_LAST, RUN, RUN_ALL, catalog, endpoints, gate,
    queue,
};
use crate::cdp::lock;
use crate::control::Core;

/// The i18n keys of the notices; `{flow}`, `{views}`, `{passed}`, `{failed}`, `{skipped}`,
/// `{dir}`, `{flows}` and `{error}` are placeholders.
pub mod notice {
    /// A run started.
    pub const STARTED: &str = "flows.notice.started";
    /// Every view passed (or was skipped).
    pub const PASSED: &str = "flows.notice.passed";
    /// A view failed or timed out.
    pub const FAILED: &str = "flows.notice.failed";
    /// The run did not start or broke off.
    pub const ERROR: &str = "flows.notice.error";
    /// The workspace is not trusted.
    pub const UNTRUSTED: &str = "flows.notice.untrusted";
    /// `flows.run` without a name in a project with several flows.
    pub const PICK: &str = "flows.notice.pick";
    /// The project has no flows.
    pub const NONE: &str = "flows.notice.none";
    /// `flows.run_all` stopped at a flow that did not start; `{flow}`, `{error}`, `{flows}`.
    pub const QUEUE_STOPPED: &str = "flows.notice.queue_stopped";
}

/// The run going on; the Flows panel reads it ([`super::panel`]).
pub(super) struct Running {
    pub(super) run_id: String,
    pub(super) flow: String,
    cancel: watch::Sender<bool>,
    pub(super) statuses: BTreeMap<u32, String>,
    /// When it started, ms since the epoch.
    pub(super) started_at_ms: u64,
}

/// The last run, for `flows.rerun_last` and `flows.open_results`.
pub(super) struct Last {
    name: String,
    views: Option<Vec<ViewId>>,
    /// A played recording's flow (RB-090-66), so `flows.rerun_last` plays it again.
    inline: Option<InlineFlow>,
    pub(super) dir: PathBuf,
}

pub(super) struct State {
    pub(super) running: Option<Running>,
    pub(super) last: Option<Last>,
}

pub(super) static STATE: Mutex<State> = Mutex::new(State {
    running: None,
    last: None,
});

/// A run that started: its id, and its result once it ends.
pub struct Started {
    /// The run's id, also its folder under `.rbrowser/out/flows/`.
    pub run_id: String,
    /// The result (or why there is none).
    pub done: oneshot::Receiver<Result<RunResult, FlowError>>,
}

/// What a run starts from: the project and the app's state now.
///
/// # Errors
///
/// [`FlowError::NoProject`].
pub fn setup(core: &Core) -> Result<Setup, FlowError> {
    let host = core.host();
    let root = host.project_root().ok_or(FlowError::NoProject)?;
    let views = host.views();
    let targets = views
        .iter()
        .filter_map(|view| host.target_id(view.id).map(|target| (view.id, target)))
        .collect();
    let hidden = views.iter().map(|view| view.id);
    let hidden = hidden.filter(|id| host.is_off_screen(*id)).collect();
    let config = host.config();
    let environment = host.shell_state().environment;
    let env = environment
        .and_then(|id| {
            config
                .environments
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.url.clone())
        })
        .or(config.start_url);
    Ok(Setup {
        root,
        settings: core.user_settings().automation.flows,
        views,
        targets,
        hidden,
        env,
        trust_store: crate::trust::store_path(core),
        started_at: crate::trust::rfc3339(SystemTime::now()),
        tmp_root: rb_config::local_dir().map(|dir| dir.join(super::run::FLOWS_TMP_DIR)),
    })
}

/// `flow_list`: the folder, whether flows may run, every flow, the run going on and the last.
///
/// # Errors
///
/// [`FlowError::NoProject`], [`FlowError::BadDir`] or [`FlowError::Io`].
pub fn list(core: &Core) -> Result<Value, FlowError> {
    let setup = setup(core)?;
    let flows = catalog::list(&setup.root, &setup.settings.dir)?;
    let trusted = gate::trusted_hash(&setup.root, setup.trust_store.as_deref()).is_ok();
    let state = lock(&STATE);
    let running = state.running.as_ref().map(|running| {
        json!({ "runId": running.run_id, "flow": running.flow, "views": running.statuses })
    });
    let last = state.last.as_ref().map(|last| {
        json!({ "flow": last.name, "resultFile": last.dir.join(RESULT_FILE).to_string_lossy() })
    });
    Ok(json!({
        "dir": setup.settings.dir,
        "trusted": trusted,
        "blockedReason": (!trusted).then(|| FlowError::Untrusted.to_string()),
        "flows": flows,
        "running": running,
        "last": last,
    }))
}

/// Starts the flow `name` in `views` (its own when `None`) on the core's runtime.
///
/// # Errors
///
/// [`FlowError::NoProject`], [`FlowError::Untrusted`] (the shell is told), [`FlowError::Busy`],
/// or a run id that names no folder.
pub fn start(
    core: &Arc<Core>,
    name: &str,
    views: Option<Vec<ViewId>>,
    asked_id: Option<String>,
) -> Result<Started, FlowError> {
    begin(core, name, views, asked_id, None)
}

/// Starts `flow`, named `name` - a stored recording rendered as a flow (`recorder.play`,
/// RB-090-66) - in `views` (its breakpoints' when `None`), as [`start`] starts a flow file:
/// the same trust gate, progress, notices, cancel and `result.json`.
///
/// # Errors
///
/// Those of [`start`].
pub fn start_inline(
    core: &Arc<Core>,
    name: &str,
    flow: InlineFlow,
    views: Option<Vec<ViewId>>,
) -> Result<Started, FlowError> {
    begin(core, name, views, None, Some(flow))
}

fn begin(
    core: &Arc<Core>,
    name: &str,
    views: Option<Vec<ViewId>>,
    asked_id: Option<String>,
    inline: Option<InlineFlow>,
) -> Result<Started, FlowError> {
    let setup = setup(core)?;
    if let Err(error) = gate::trusted_hash(&setup.root, setup.trust_store.as_deref()) {
        core.notify(notification(
            NotificationLevel::Warning,
            notice::UNTRUSTED,
            &[],
        ));
        return Err(error);
    }
    let id = match asked_id {
        Some(id) if valid_run_id(&id) => id,
        Some(id) => return Err(FlowError::Io(format!("{id:?} is no run id"))),
        None => {
            let random = crate::secret::random_hex(3).map_err(|e| FlowError::Io(e.to_string()))?;
            run_id(&setup.started_at, &random)
        }
    };
    let (cancel, cancelled) = watch::channel(false);
    // No recording may start between the check and the claim (RB-090-71).
    super::exclusive::exclusively(|| {
        let recording =
            crate::recorder::for_host(core.host()).and_then(|recorder| recorder.taking_steps());
        gate::not_while_recording(recording.as_deref())?;
        let mut state = lock(&STATE);
        if let Some(running) = &state.running {
            return Err(FlowError::Busy(running.flow.clone()));
        }
        state.running = Some(Running {
            run_id: id.clone(),
            flow: name.to_owned(),
            cancel,
            statuses: BTreeMap::new(),
            started_at_ms: epoch_ms(SystemTime::now()),
        });
        Ok(())
    })?;
    let request = RunRequest {
        name: name.to_owned(),
        views,
        run_id: id.clone(),
        inline,
    };
    let (sender, done) = oneshot::channel();
    let deps = AppDeps {
        core: Arc::clone(core),
    };
    let core = Arc::clone(core);
    core.notify(notification(
        NotificationLevel::Info,
        notice::STARTED,
        &[("flow", name.to_owned())],
    ));
    core.send(&CoreToShell::FlowRunChanged);
    core.runtime().clone().spawn(async move {
        let (progress, statuses) = mpsc::unbounded_channel::<Value>();
        let follow = super::step_progress::follow(&core, &request, statuses, record_status);
        // `automation.flows.hiddenViews: layout` shows the run's layout first (RB-090-84).
        let shown = super::run_layout::show(&core).await;
        let setup = if shown.switched() {
            self::setup(&core).unwrap_or(setup)
        } else {
            setup
        };
        let outcome = execute_recorded(&setup, &request, &deps, cancelled, progress).await;
        follow.abort();
        finish(&core, &setup, &request, &outcome);
        if let Some(replaced) = shown.into_restore() {
            super::run_layout::restore(&core, replaced);
        }
        let _ = sender.send(outcome);
    });
    Ok(Started { run_id: id, done })
}

fn epoch_ms(at: SystemTime) -> u64 {
    at.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Ends the run going on and a `flows.run_all` queue; `false` when none runs.
pub fn cancel() -> bool {
    queue::note_cancel();
    lock(&STATE)
        .running
        .as_ref()
        .is_some_and(|running| running.cancel.send(true).is_ok())
}

fn record_status(status: &Value) {
    let (Some(view), Some(text)) = (
        status.get("viewId").and_then(Value::as_u64),
        status.get("status").and_then(Value::as_str),
    ) else {
        return;
    };
    if let (Some(running), Ok(view)) = (lock(&STATE).running.as_mut(), u32::try_from(view)) {
        running.statuses.insert(view, text.to_owned());
    }
}

fn finish(
    core: &Core,
    setup: &Setup,
    request: &RunRequest,
    outcome: &Result<RunResult, FlowError>,
) {
    super::report::log(&request.name, outcome, |view| {
        super::report::shown_now(core, view)
    });
    {
        let mut state = lock(&STATE);
        state.running = None;
        state.last = Some(Last {
            name: request.name.clone(),
            views: request.views.clone(),
            inline: request.inline.clone(),
            dir: run_dir(&setup.root, &request.run_id),
        });
    }
    let flow = ("flow", request.name.clone());
    let message = match outcome {
        Ok(result) => {
            let (passed, failed, skipped) = result.counts();
            let failing: Vec<&str> = result
                .views
                .iter()
                .filter(|view| view.error.is_some())
                .map(|view| view.label.as_str())
                .collect();
            let dir = format!("{}/{}", super::result::OUT_DIR, result.run_id);
            let args = [
                flow,
                ("passed", passed.to_string()),
                ("failed", failed.to_string()),
                ("skipped", skipped.to_string()),
                ("views", failing.join(", ")),
                ("dir", dir),
            ];
            if result.passed {
                notification(NotificationLevel::Info, notice::PASSED, &args)
            } else {
                notification(NotificationLevel::Error, notice::FAILED, &args)
            }
        }
        Err(error) => notification(
            NotificationLevel::Error,
            notice::ERROR,
            &[flow, ("error", rb_diag::redact(&error.to_string()))],
        ),
    };
    core.notify(message);
    for skipped in outcome.iter().flat_map(super::report::skip_notices) {
        core.notify(skipped);
    }
    core.send(&CoreToShell::FlowRunChanged);
}

/// The handler of the command `id`.
pub fn handler(id: &str, core: Weak<Core>) -> Handler {
    let id = id.to_owned();
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        let failed = |error: FlowError| CommandError::Failed(error.to_string());
        match id.as_str() {
            RUN => {
                let name = match args.get("name").and_then(Value::as_str) {
                    Some(name) => name.to_owned(),
                    None => only_flow(&core).map_err(failed)?,
                };
                let views = resolve_views(&core.host().views(), &args["views"]).map_err(failed)?;
                let run_id = args.get("runId").and_then(Value::as_str).map(str::to_owned);
                let started = start(&core, &name, views, run_id).map_err(failed)?;
                Ok(json!({ "runId": started.run_id }))
            }
            RUN_ALL => queue::run_all(&core).map_err(failed),
            RERUN_LAST => {
                let last = lock(&STATE)
                    .last
                    .as_ref()
                    .map(|last| (last.name.clone(), last.views.clone(), last.inline.clone()));
                let (name, views, inline) = last.ok_or(FlowError::NothingYet).map_err(failed)?;
                let started = begin(&core, &name, views, None, inline).map_err(failed)?;
                Ok(json!({ "runId": started.run_id }))
            }
            CANCEL => Ok(json!({ "cancelled": cancel() })),
            PANEL => {
                // A person's (RB-090-71): a folded dock opens.
                let flows = crate::panel_dock::core_panels::FLOWS;
                crate::panel_dock::core_panels::open_unfolded(&core, flows);
                Ok(Value::Null)
            }
            OPEN_RESULTS => {
                let dir = lock(&STATE).last.as_ref().map(|last| last.dir.clone());
                let file = dir
                    .ok_or(FlowError::NothingYet)
                    .map_err(failed)?
                    .join(RESULT_FILE);
                core.runtime().spawn_blocking(move || {
                    if let Err(error) = rb_diag::reveal(&file) {
                        tracing::warn!(%error, "the flow results could not be shown");
                    }
                });
                Ok(Value::Null)
            }
            other => Err(CommandError::UnknownCommand(other.to_owned())),
        }
    })
}

/// The only flow of the project; with none or several the shell says what to do.
fn only_flow(core: &Core) -> Result<String, FlowError> {
    let setup = setup(core)?;
    let flows = catalog::list(&setup.root, &setup.settings.dir)?;
    match flows.as_slice() {
        [only] => Ok(only.name.clone()),
        [] => {
            core.notify(notification(
                NotificationLevel::Info,
                notice::NONE,
                &[("dir", setup.settings.dir.clone())],
            ));
            Err(FlowError::NoName(Vec::new()))
        }
        several => {
            let names: Vec<String> = several.iter().map(|flow| flow.name.clone()).collect();
            core.notify(notification(
                NotificationLevel::Info,
                notice::PICK,
                &[("flows", names.join(", "))],
            ));
            Err(FlowError::NoName(names))
        }
    }
}

/// The app as a run needs it.
struct AppDeps {
    core: Arc<Core>,
}

#[async_trait]
impl FlowDeps for AppDeps {
    fn open_endpoints(
        &self,
        run: &[Runnable],
        targets: &[String],
    ) -> Result<Vec<Endpoint>, FlowError> {
        let host = self.core.host();
        let hooks = |filter: &'static dyn rb_cdp::TargetFilter| crate::cdp_proxy::Hooks {
            filter,
            ..crate::cdp_proxy::hooks(host)
        };
        endpoints::open(self.core.runtime(), host.cdp(), &hooks, run, targets)
    }

    /// The run's views leave the window's SyncHub until the guard drops ([`SyncPause`]).
    fn pause_sync(&self, views: &[ViewId]) -> Box<dyn Send + Sync> {
        match crate::sync_hub::for_host(self.core.host()) {
            Some(sync) => Box::new(SyncPause::new(sync, views)),
            None => Box::new(()),
        }
    }

    async fn run_flow(
        &self,
        params: Value,
        timeout: Duration,
        progress: mpsc::UnboundedSender<Value>,
    ) -> Result<Value, FlowError> {
        let sidecar = crate::sidecar::shared()
            .ok_or_else(|| FlowError::Sidecar("the Node sidecar is not available".to_owned()))?;
        sidecar
            .request_with_progress("flows.run", params, Some(timeout), progress)
            .await
            .map_err(|error| FlowError::Sidecar(rb_diag::redact(&error.to_string())))
    }
}

pub(super) fn notification(
    level: NotificationLevel,
    key: &str,
    args: &[(&str, String)],
) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
        settings_key: None,
    })
}
