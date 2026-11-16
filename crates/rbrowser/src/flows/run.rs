//! One run of one flow, without the app around it: the trust gate, the flow's text, the views,
//! an endpoint per view, sync paused, the sidecar's `flows.run`, the skipped views added, and
//! `result.json` written. The app's side - proxies, sidecar, sync - comes in through
//! [`FlowDeps`], so the tests run it against fakes and the scripted browser.

use std::collections::BTreeMap;
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
use super::plan::{Runnable, engine_name, plan};
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
    /// The `env` global: the active environment's URL, else the start URL.
    pub env: Option<String>,
    /// The trust store (`trust.json`).
    pub trust_store: Option<PathBuf>,
    /// When it starts, RFC 3339 UTC.
    pub started_at: String,
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
    let (path, source) = catalog::read(&setup.root, &setup.settings.dir, &request.name)?;
    gate::still_trusted(&setup.root, store, &hash)?;
    if !valid_run_id(&request.run_id) {
        return Err(FlowError::Io(format!("{:?} is no run id", request.run_id)));
    }
    let head = catalog::parse_head(&source);
    let planned = plan(
        &setup.views,
        request.views.as_deref(),
        &head.views,
        |view| setup.targets.get(&view).cloned(),
    )?;
    let timeout_ms = head.timeout_ms.unwrap_or(setup.settings.timeout_ms);
    let dir = run_dir(&setup.root, &request.run_id);
    let file = display_path(&path);
    let ran = if planned.run.is_empty() {
        Vec::new()
    } else {
        let targets: Vec<String> = setup.targets.values().cloned().collect();
        let endpoints = deps.open_endpoints(&planned.run, &targets)?;
        let ids: Vec<ViewId> = planned.run.iter().map(|run| run.view.id).collect();
        let _sync = (!setup.settings.sync_during_run).then(|| deps.pause_sync(&ids));
        let params = params(
            setup,
            request,
            &source,
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
        file: catalog::relative(&setup.root, &path),
        started_at: setup.started_at.clone(),
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        passed: !RunResult::failed(&views),
        views,
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
                file: format!(
                    "{}/{}.ts",
                    setup.settings.dir.trim_end_matches('/'),
                    request.name
                ),
                started_at: setup.started_at.clone(),
                duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                passed: false,
                views: Vec::new(),
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
        "parallel": setup.settings.parallel,
        "screenshotOnFailure": setup.settings.screenshot_on_failure,
        "outDir": dir.to_string_lossy(),
        "projectRoot": setup.root.to_string_lossy(),
    })
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
        line: None,
        column: None,
        file: file.to_owned(),
    });
    view
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
