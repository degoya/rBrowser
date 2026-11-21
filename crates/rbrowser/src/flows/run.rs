//! One run of one flow, without the app around it: the trust gate, the flow's text, the views,
//! an endpoint per view, sync paused, the sidecar's `flows.run`, the skipped views added, and
//! `result.json` written. The app's side - proxies, sidecar, sync - comes in through
//! [`FlowDeps`], so the tests run it against fakes and the scripted browser. A flow that is no
//! file of the flows folder - a stored recording played back (RB-090-66) - comes in memory
//! ([`InlineFlow`]) and runs the same way, behind the same trust gate.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use rb_config::FlowSettings;
use rb_engine::{ViewId, ViewInfo};
use serde_json::{Value, json};
use tokio::sync::{mpsc, watch};

use super::FlowError;
use super::catalog;
use super::gate;
use super::plan::{
    Runnable, engine_name, missing_breakpoints, nothing_to_play, plan, without_hidden,
};
use super::result::{
    ErrorInfo, RESULT_VERSION, RunResult, ViewResult, ViewStatus, run_dir, valid_run_id, write,
};

/// How much longer than its budget a run may take before rBrowser gives up on it; more than the
/// sidecar's own grace (15 s, `sidecar/src/modules/flows/real.ts`), so the sidecar stops a late
/// worker and answers the views it finished first.
pub const RUN_GRACE: Duration = Duration::from_secs(30);
/// What each round of views may take past its timeout in the worker: the failure screenshot and
/// the close (`VIEW_OVERHEAD_MS`, `sidecar/src/modules/flows/result.ts`).
pub const VIEW_OVERHEAD: Duration = Duration::from_secs(10);
/// The message of the views of a cancelled run.
pub const CANCELLED: &str = "the run was cancelled";
/// The folder under the app's local folder ([`rb_config::local_dir`]) that holds each run's
/// worker folder `<runId>`: its temp folder and screenshots on a local drive, never on the
/// project's side - a WSL share refused the worker's removals (RB-090-72). The sidecar removes
/// the run's folder after the run (`sidecar/src/modules/flows/tmp.ts`).
pub const FLOWS_TMP_DIR: &str = "flows-tmp";

/// What a run is about: the project and the app's state when it starts.
#[derive(Clone, Debug)]
pub struct Setup {
    /// The project root, as opened (`\\wsl.localhost\...` included).
    pub root: PathBuf,
    /// `automation.flows`.
    pub settings: FlowSettings,
    /// Every view, in grid order.
    pub views: Vec<ViewInfo>,
    /// The page target of each view that has one.
    pub targets: BTreeMap<ViewId, String>,
    /// The views a layout mode placed off screen: skipped (RB-090-84).
    pub hidden: BTreeSet<ViewId>,
    /// The `env` global: the active environment's URL, else the start URL.
    pub env: Option<String>,
    /// The trust store (`trust.json`).
    pub trust_store: Option<PathBuf>,
    /// When it starts, RFC 3339 UTC.
    pub started_at: String,
    /// `<local>/`[`FLOWS_TMP_DIR`]; `None` lets the sidecar pick its own local folder.
    pub tmp_root: Option<PathBuf>,
}

/// What a run is asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunRequest {
    /// The flow's name.
    pub name: String,
    /// The views asked for; the flow's `// @views` or all when `None`.
    pub views: Option<Vec<ViewId>>,
    /// The run's id (also its folder).
    pub run_id: String,
    /// The flow's text in memory instead of `<flows>/<name>.ts`.
    pub inline: Option<InlineFlow>,
}

/// A flow that lives in no file of the flows folder: a stored recording rendered as a flow by
/// the `recorder-export` plugin (`recorder.play`, RB-090-66).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineFlow {
    /// The flow's text.
    pub source: String,
    /// What results and errors name as its file: the recording's, relative to the root.
    pub file: String,
    /// The breakpoints it runs in when no views are asked for and its text names none.
    pub views: Vec<String>,
    /// How long the flow waits on purpose - a recording played at the `recorded` pace waits its
    /// pauses (RB-090-84): added to the flow's timeout, so the waits take no time from its steps.
    pub pause_ms: u64,
}

/// The flow a run takes: its text, its file as errors and as `result.json` name it, and the
/// views it names itself.
struct Source {
    text: String,
    display: String,
    relative: String,
    head: catalog::Head,
}

impl Source {
    fn of(setup: &Setup, request: &RunRequest) -> Result<Self, FlowError> {
        if let Some(inline) = &request.inline {
            let mut head = catalog::parse_head(&inline.source);
            // Only a flow file on disk may load a login state: a recording's rendered text is no
            // trusted flow (security review w28).
            head.storage_state = None;
            if head.views.is_empty() {
                head.views.clone_from(&inline.views);
            }
            return Ok(Self {
                text: inline.source.clone(),
                display: inline.file.clone(),
                relative: inline.file.clone(),
                head,
            });
        }
        let (path, text) = catalog::read(&setup.root, &setup.settings.dir, &request.name)?;
        Ok(Self {
            head: catalog::parse_head(&text),
            display: display_path(&path),
            relative: catalog::relative(&setup.root, &path),
            text,
        })
    }
}

/// The CDP endpoint of one view, kept open while its guard lives.
pub struct Endpoint {
    /// `{ "url", "headers" }` for `connectOverCDP`.
    pub value: Value,
    /// Stops the endpoint when dropped.
    pub guard: Box<dyn Send + Sync>,
}

/// What a run needs of the app.
#[async_trait]
pub trait FlowDeps: Send + Sync {
    /// One endpoint per view of `run` (in its order) that shows only that view's page;
    /// `targets` are the pages of every view of the grid.
    fn open_endpoints(
        &self,
        run: &[Runnable],
        targets: &[String],
    ) -> Result<Vec<Endpoint>, FlowError>;

    /// Stops syncing input for `views` until the guard drops.
    fn pause_sync(&self, views: &[ViewId]) -> Box<dyn Send + Sync>;

    /// Loads the login state file `name` (`// @storageState`, under `.rbrowser/out/`) into the
    /// sessions of `views` (RB-094-08).
    async fn load_state(&self, _name: &str, _views: &[ViewId]) -> Result<(), FlowError> {
        Ok(())
    }

    /// The sidecar's `flows.run` with `params`, its `$/progress` sent to `progress`.
    async fn run_flow(
        &self,
        params: Value,
        timeout: Duration,
        progress: mpsc::UnboundedSender<Value>,
    ) -> Result<Value, FlowError>;
}

/// The flow's file as errors name it: the Linux path for a project in WSL.
pub fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    match rb_wsl::path::parse_unc(&text) {
        Ok(wsl) => wsl.linux_path,
        Err(_) => text.into_owned(),
    }
}

/// The longest a run of `views` views may take at `parallel` with `timeout_ms` each: per round
/// one timeout and [`VIEW_OVERHEAD`], then [`RUN_GRACE`].
pub fn budget(views: usize, parallel: u32, timeout_ms: u32) -> Duration {
    let lanes = match usize::try_from(parallel) {
        Ok(0) | Err(_) => views,
        Ok(lanes) => lanes.min(views),
    }
    .max(1);
    let rounds = u32::try_from(views.div_ceil(lanes)).unwrap_or(u32::MAX);
    (Duration::from_millis(u64::from(timeout_ms)) + VIEW_OVERHEAD).saturating_mul(rounds)
        + RUN_GRACE
}

/// A flow's `timeout_ms` with the `pause_ms` it waits on purpose on top, at most `u32::MAX`.
fn with_pauses(timeout_ms: u32, pause_ms: u64) -> u32 {
    u32::try_from(u64::from(timeout_ms).saturating_add(pause_ms)).unwrap_or(u32::MAX)
}

/// Runs `request` in `setup` and writes its `result.json`; `cancel` turning `true` ends it.
///
/// # Errors
///
/// Before any view runs: [`FlowError::Untrusted`], a missing or unreadable flow, no views, an
/// endpoint that cannot be opened, or a `result.json` that cannot be written.
pub async fn execute(
    setup: &Setup,
    request: &RunRequest,
    deps: &dyn FlowDeps,
    mut cancel: watch::Receiver<bool>,
    progress: mpsc::UnboundedSender<Value>,
) -> Result<RunResult, FlowError> {
    let started = Instant::now();
    let store = setup.trust_store.as_deref();
    let hash = gate::trusted_hash(&setup.root, store)?;
    let flow = Source::of(setup, request)?;
    gate::still_trusted(&setup.root, store, &hash)?;
    if !valid_run_id(&request.run_id) {
        return Err(FlowError::Io(format!("{:?} is no run id", request.run_id)));
    }
    let planned = plan(
        &setup.views,
        request.views.as_deref(),
        &flow.head.views,
        |view| setup.targets.get(&view).cloned(),
    )
    .map_err(|error| match (error, &request.inline) {
        (FlowError::NoViews(_), Some(inline))
            if request.views.is_none() && !inline.views.is_empty() =>
        {
            FlowError::NoViews(format!(
                "the recording's breakpoints {} show in no view of the grid",
                inline.views.join(", ")
            ))
        }
        (error, _) => error,
    })?;
    // RB-090-84: the views a layout hides are skipped, the breakpoints no view shows named; views
    // asked for by id or name play even when hidden.
    let (planned, skipped_breakpoints) = match &request.views {
        Some(_) => (planned, Vec::new()),
        None => (
            without_hidden(planned, &setup.hidden),
            missing_breakpoints(&setup.views, &flow.head.views),
        ),
    };
    // A run with no view to play fails; it never passes having played nothing.
    if planned.run.is_empty() {
        return Err(FlowError::NoViews(nothing_to_play(&planned.skipped)));
    }
    announce(&progress, &planned.run, &planned.skipped);
    // The activity bar's "step n/m" (RB-090-68): `super::step_progress::TOTAL_STEPS`.
    if let Some(total) = super::run_steps::total_steps(&flow.text) {
        let _ = progress.send(json!({ "totalSteps": total }));
    }
    let timeout_ms = with_pauses(
        flow.head.timeout_ms.unwrap_or(setup.settings.timeout_ms),
        request.inline.as_ref().map_or(0, |inline| inline.pause_ms),
    );
    let dir = run_dir(&setup.root, &request.run_id);
    let file = flow.display.clone();
    let ran = if planned.run.is_empty() {
        Vec::new()
    } else {
        let targets: Vec<String> = setup.targets.values().cloned().collect();
        let endpoints = deps.open_endpoints(&planned.run, &targets)?;
        let ids: Vec<ViewId> = planned.run.iter().map(|run| run.view.id).collect();
        if let Some(name) = &flow.head.storage_state {
            deps.load_state(name, &ids).await?;
        }
        let _sync = (!setup.settings.sync_during_run).then(|| deps.pause_sync(&ids));
        let params = params(
            setup,
            request,
            &flow.text,
            &file,
            &planned.run,
            &endpoints,
            &dir,
            timeout_ms,
        );
        let timeout = budget(planned.run.len(), setup.settings.parallel, timeout_ms);
        let answer = tokio::select! {
            answer = deps.run_flow(params, timeout, progress) => Some(answer),
            _ = cancel.wait_for(|cancelled| *cancelled) => None,
        };
        drop(endpoints);
        match answer {
            Some(Ok(answer)) => views_of(&answer, &planned.run, &file),
            Some(Err(error)) => failed_all(&planned.run, &error.to_string(), &file),
            None => failed_all(&planned.run, CANCELLED, &file),
        }
    };
    let mut views: Vec<ViewResult> = ran.into_iter().chain(planned.skipped).collect();
    let order = |id: u32| setup.views.iter().position(|view| view.id.0 == id);
    views.sort_by_key(|view| order(view.view_id));
    let result = RunResult {
        version: RESULT_VERSION,
        run_id: request.run_id.clone(),
        flow: request.name.clone(),
        file: flow.relative,
        started_at: setup.started_at.clone(),
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        passed: !RunResult::failed(&views),
        views,
        skipped_breakpoints,
        error: None,
    };
    write(&dir, &result)?;
    Ok(result)
}

/// [`execute`]; when it fails before its views, a `result.json` that says why (no views,
/// `passed: false`), so `rbrowser flow run` waiting for this run id learns it at once instead of
/// after the whole budget. A run id that names no folder gets none.
///
/// # Errors
///
/// Those of [`execute`].
pub async fn execute_recorded(
    setup: &Setup,
    request: &RunRequest,
    deps: &dyn FlowDeps,
    cancel: watch::Receiver<bool>,
    progress: mpsc::UnboundedSender<Value>,
) -> Result<RunResult, FlowError> {
    let started = Instant::now();
    let outcome = execute(setup, request, deps, cancel, progress).await;
    if let Err(error) = &outcome
        && valid_run_id(&request.run_id)
    {
        let dir = run_dir(&setup.root, &request.run_id);
        if super::result::read(&dir).is_none() {
            let result = RunResult {
                version: RESULT_VERSION,
                run_id: request.run_id.clone(),
                flow: request.name.clone(),
                file: request.inline.as_ref().map_or_else(
                    || {
                        format!(
                            "{}/{}.ts",
                            setup.settings.dir.trim_end_matches('/'),
                            request.name
                        )
                    },
                    |inline| inline.file.clone(),
                ),
                started_at: setup.started_at.clone(),
                duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                passed: false,
                views: Vec::new(),
                skipped_breakpoints: Vec::new(),
                error: Some(rb_diag::redact(&error.to_string())),
            };
            if let Err(written) = write(&dir, &result) {
                tracing::warn!(%written, "the result of a failed flow run could not be written");
            }
        }
    }
    outcome
}

#[expect(
    clippy::too_many_arguments,
    reason = "one call site; the parts of one request"
)]
fn params(
    setup: &Setup,
    request: &RunRequest,
    source: &str,
    file: &str,
    run: &[Runnable],
    endpoints: &[Endpoint],
    dir: &Path,
    timeout_ms: u32,
) -> Value {
    let views: Vec<Value> = run
        .iter()
        .zip(endpoints)
        .map(|(runnable, endpoint)| {
            let view = &runnable.view;
            json!({
                "id": view.id.0,
                "label": view.label,
                "width": view.width,
                "height": view.height,
                "engine": engine_name(view.engine),
                "breakpoint": view.breakpoint,
                "endpoint": endpoint.value,
            })
        })
        .collect();
    json!({
        "runId": request.run_id,
        "flow": { "name": request.name, "source": source, "file": file },
        "views": views,
        "env": setup.env,
        "timeoutMs": timeout_ms,
        // The sidecar holds it to `timeoutMs` (RB-090-76).
        "actionTimeoutMs": setup.settings.action_timeout_ms,
        "parallel": setup.settings.parallel,
        "screenshotOnFailure": setup.settings.screenshot_on_failure,
        "outDir": dir.to_string_lossy(),
        "projectRoot": setup.root.to_string_lossy(),
        "tmpDir": setup
            .tmp_root
            .as_ref()
            .map(|root| root.join(&request.run_id).to_string_lossy().into_owned()),
    })
}

/// Names every view of the run at its start: each that runs `queued`, each left out `skipped`,
/// so the Flows panel shows the whole run before the sidecar reports (RB-090-71).
fn announce(progress: &mpsc::UnboundedSender<Value>, run: &[Runnable], skipped: &[ViewResult]) {
    let waiting = run.iter().map(|runnable| (runnable.view.id.0, "queued"));
    let left_out = skipped.iter().map(|view| (view.view_id, "skipped"));
    for (view, status) in waiting.chain(left_out) {
        let _ = progress.send(json!({ "viewId": view, "status": status }));
    }
}

/// The sidecar's views, each run view once (a missing one failed).
fn views_of(answer: &Value, run: &[Runnable], file: &str) -> Vec<ViewResult> {
    let reported: Vec<ViewResult> = answer
        .get("views")
        .cloned()
        .and_then(|views| serde_json::from_value(views).ok())
        .unwrap_or_default();
    run.iter()
        .map(|runnable| {
            reported
                .iter()
                .find(|view| view.view_id == runnable.view.id.0)
                .cloned()
                .unwrap_or_else(|| {
                    failed(runnable, "the sidecar reported nothing for this view", file)
                })
        })
        .collect()
}

fn failed_all(run: &[Runnable], message: &str, file: &str) -> Vec<ViewResult> {
    run.iter()
        .map(|runnable| failed(runnable, message, file))
        .collect()
}

fn failed(runnable: &Runnable, message: &str, file: &str) -> ViewResult {
    let mut view = super::plan::skipped(&runnable.view, "");
    view.status = ViewStatus::Failed;
    view.reason = None;
    view.error = Some(ErrorInfo {
        message: message.to_owned(),
        stack: String::new(),
        step: None,
        step_id: None,
        summary: None,
        line: None,
        column: None,
        file: file.to_owned(),
    });
    view
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "run_hidden_tests.rs"]
mod hidden_tests;

#[cfg(test)]
#[path = "run_state_tests.rs"]
mod state_tests;
