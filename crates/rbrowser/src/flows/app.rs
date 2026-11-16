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
use rb_engine::{ViewHost as _, ViewId, ViewInfo};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot, watch};

use super::plan::Runnable;
use super::result::{RESULT_FILE, RunResult, run_dir, run_id, valid_run_id};
use super::run::{Endpoint, FlowDeps, RunRequest, Setup, execute_recorded};
use super::sync_pause::SyncPause;
use super::{
    CANCEL, FlowError, OPEN_RESULTS, RERUN_LAST, RUN, RUN_ALL, catalog, endpoints, gate, queue,
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
}

/// The run going on.
struct Running {
    run_id: String,
    flow: String,
    cancel: watch::Sender<bool>,
    statuses: BTreeMap<u32, String>,
}

/// The last run, for `flows.rerun_last` and `flows.open_results`.
struct Last {
    name: String,
    views: Option<Vec<ViewId>>,
    dir: PathBuf,
}

struct State {
    running: Option<Running>,
    last: Option<Last>,
}

static STATE: Mutex<State> = Mutex::new(State {
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
        env,
        trust_store: crate::trust::store_path(core),
        started_at: crate::trust::rfc3339(SystemTime::now()),
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
    {
        let mut state = lock(&STATE);
        if let Some(running) = &state.running {
            return Err(FlowError::Busy(running.flow.clone()));
        }
        state.running = Some(Running {
            run_id: id.clone(),
            flow: name.to_owned(),
            cancel,
            statuses: BTreeMap::new(),
        });
    }
    let request = RunRequest {
        name: name.to_owned(),
        views,
        run_id: id.clone(),
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
    core.runtime().clone().spawn(async move {
        let (progress, mut statuses) = mpsc::unbounded_channel::<Value>();
        let follow = tokio::spawn(async move {
            while let Some(status) = statuses.recv().await {
                record_status(&status);
            }
        });
        let outcome = execute_recorded(&setup, &request, &deps, cancelled, progress).await;
        follow.abort();
        finish(&core, &setup, &request, &outcome);
        let _ = sender.send(outcome);
    });
    Ok(Started { run_id: id, done })
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
    {
        let mut state = lock(&STATE);
        state.running = None;
        state.last = Some(Last {
            name: request.name.clone(),
            views: request.views.clone(),
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
}

/// The views of a command's `views` argument: ids, or breakpoint ids and labels (every view
/// showing one); `None` when absent.
///
/// # Errors
///
/// [`FlowError::NoViews`] for a name no view has.
pub fn resolve_views(views: &[ViewInfo], value: &Value) -> Result<Option<Vec<ViewId>>, FlowError> {
    let Some(items) = value.as_array() else {
        return Ok(None);
    };
    let mut ids = Vec::new();
    for item in items {
        let text = item.as_str().map(str::trim);
        let id = item.as_u64().or_else(|| text.and_then(|t| t.parse().ok()));
        let found: Vec<ViewId> = views
            .iter()
            .filter(|view| match (id, text) {
                (Some(id), _) => u64::from(view.id.0) == id,
                (None, Some(name)) => [view.breakpoint.as_deref(), view.cell_breakpoint.as_deref()]
                    .into_iter()
                    .flatten()
                    .chain([view.label.as_str()])
                    .any(|candidate| candidate.eq_ignore_ascii_case(name)),
                (None, None) => false,
            })
            .map(|view| view.id)
            .collect();
        if found.is_empty() {
            return Err(FlowError::NoViews(format!("there is no view {item}")));
        }
        for id in found {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    Ok(Some(ids))
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
            RUN_ALL => run_all(&core).map_err(failed),
            RERUN_LAST => {
                let last = lock(&STATE)
                    .last
                    .as_ref()
                    .map(|last| (last.name.clone(), last.views.clone()));
                let (name, views) = last.ok_or(FlowError::NothingYet).map_err(failed)?;
                let started = start(&core, &name, views, None).map_err(failed)?;
                Ok(json!({ "runId": started.run_id }))
            }
            CANCEL => Ok(json!({ "cancelled": cancel() })),
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

/// `flows.run_all`: every flow, one after the other, in its own views.
fn run_all(core: &Arc<Core>) -> Result<Value, FlowError> {
    let setup = setup(core)?;
    let names: Vec<String> = catalog::list(&setup.root, &setup.settings.dir)?
        .into_iter()
        .map(|flow| flow.name)
        .collect();
    let Some((first, rest)) = names.split_first() else {
        return Err(FlowError::NoName(Vec::new()));
    };
    let since = queue::cancels();
    let started = start(core, first, None, None)?;
    let rest = rest.to_vec();
    let core = Arc::clone(core);
    core.runtime().clone().spawn(async move {
        let _ = started.done.await;
        let cancelled = move || queue::cancelled_since(since);
        queue::run_queue(rest, cancelled, |name| start(&core, name, None, None)).await;
    });
    Ok(json!({ "flows": names }))
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

fn notification(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> CoreToShell {
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
