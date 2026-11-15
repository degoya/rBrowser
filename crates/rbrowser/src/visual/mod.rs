//! Visual regression (RB-050-04): the commands `visual.compare`, `visual.baseline_update` and
//! `visual.lfs_setup`, the MCP tools' models ([`tool`]) and the runs over the grid's views
//! ([`run`]). Pixel diff, baselines, metadata and the LFS check are `rb_visual`'s; the findings
//! come from the first-party check plugin `visual-regression`, which gets the comparisons as its
//! arguments ([`findings`]), and go to the findings panel like any check run.
//!
//! A command answers at once (`{"started": true}`) and the run goes on the core's runtime, one
//! at a time; a notification names the outcome and the folder of the diff images (below the
//! screenshot folder, `visual/<run>/<page>/<breakpoint>-diff.png`). [`compare`] and [`update`]
//! are the same runs for the MCP tools, which wait for their answer. Saving baselines checks
//! Git LFS (`visual.lfsCheck`) and names the set-up command when it is missing; only
//! `visual.lfs_setup`, the person's own command, changes `.gitattributes` and the Git
//! configuration. Baselines come from the project and only produce findings: no trust gate.

pub mod findings;
pub mod lfs;
pub mod run;
pub mod tool;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rb_commands::CommandError;
use rb_config::EngineKind;
use rb_engine::{EngineError, Target, ViewHost};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_visual::{BaselineMeta, BaselineStore, SlugStrategy, Viewport, VisualError, meta};
use serde_json::{Value, json};

use crate::about::{BUILD, split_cef};
use crate::capture::chromium::HostShooter;
use crate::control::Core;
use crate::host::Host;
use run::{RunSetup, ViewTarget, VisualOptions};
use tool::{CompareAnswer, CompareArgs, LfsAnswer, UpdateAnswer, UpdateArgs};

/// The commands of this module.
pub const COMMANDS: [&str; 3] = [COMPARE_COMMAND, UPDATE_COMMAND, LFS_SETUP_COMMAND];
/// Compare the views with their baselines.
pub const COMPARE_COMMAND: &str = "visual.compare";
/// Store the views as the new baselines.
pub const UPDATE_COMMAND: &str = "visual.baseline_update";
/// Set the project up for LFS baselines.
pub const LFS_SETUP_COMMAND: &str = "visual.lfs_setup";
/// The notice after a compare; placeholders `{regressions}`, `{count}`, `{folder}`.
pub const COMPARED_KEY: &str = "visual.notice.compared";
/// The notice of unreliable comparisons; placeholder `{count}`.
pub const UNRELIABLE_KEY: &str = "visual.notice.unreliable";
/// The notice after a baseline update; placeholders `{count}`, `{failed}`.
pub const UPDATED_KEY: &str = "visual.notice.updated";
/// The notice that a run failed; placeholder `{error}`.
pub const FAILED_KEY: &str = "visual.notice.failed";
/// The notice that Git LFS does not keep the baselines.
pub const LFS_MISSING_KEY: &str = "visual.notice.lfs_missing";
/// The notice after the LFS set-up.
pub const LFS_DONE_KEY: &str = "visual.notice.lfs_done";
/// The notice that the LFS set-up failed; placeholder `{error}`.
pub const LFS_FAILED_KEY: &str = "visual.notice.lfs_failed";
/// The refusal while a run is in progress.
pub const BUSY_KEY: &str = "visual.error.busy";
/// The settings section the notices link to.
const SETTINGS_KEY: &str = "visual";

static BUSY: AtomicBool = AtomicBool::new(false);

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

/// Why a run gave nothing.
#[derive(Debug, thiserror::Error)]
pub enum VisualRunError {
    /// Baselines need an open project.
    #[error("open a project to use baselines")]
    NoProject,
    /// No view in the target.
    #[error("there is no view to compare")]
    NoViews,
    /// Another run is in progress.
    #[error("{key}", key = BUSY_KEY)]
    Busy,
    /// The browser side.
    #[error(transparent)]
    Engine(#[from] EngineError),
    /// Folder, file or image.
    #[error(transparent)]
    Visual(#[from] VisualError),
    /// A task ended without an answer.
    #[error("{0}")]
    Failed(String),
}

/// Runs the command `id` when it is this module's (`visual.*`); `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    let started = match id {
        COMPARE_COMMAND => CompareArgs::parse(args)
            .map_err(CommandError::InvalidArgs)
            .and_then(|args| {
                start(core, move |core| async move {
                    report_compare(&core, &args).await
                })
            }),
        UPDATE_COMMAND => UpdateArgs::parse(args)
            .map_err(CommandError::InvalidArgs)
            .and_then(|args| {
                start(core, move |core| async move {
                    report_update(&core, &args).await
                })
            }),
        LFS_SETUP_COMMAND => start(core, |core| async move { lfs_setup(&core).await }),
        _ => return None,
    };
    Some(started.map(|()| json!({ "started": true })))
}

/// Starts `job` on the core's runtime (never the caller's: a command may come from the CEF UI
/// thread) and answers at once.
fn start<F, Fut>(core: &Core, job: F) -> Result<(), CommandError>
where
    F: FnOnce(Arc<Core>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    if BUSY.load(Ordering::Acquire) {
        return Err(CommandError::Failed(BUSY_KEY.to_owned()));
    }
    let core = core
        .arc()
        .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
    let runtime = core.runtime().clone();
    runtime.spawn(job(core));
    Ok(())
}

/// The settings of Capture > Visual regression in effect, `threshold` overriding
/// `visual.threshold`.
pub fn options(config: &rb_config::VisualConfig, threshold: Option<f64>) -> VisualOptions {
    VisualOptions {
        threshold: threshold.unwrap_or(config.threshold),
        full_page: config.full_page,
        slug: match config.slug_strategy {
            rb_config::SlugStrategy::Path => SlugStrategy::Path,
            rb_config::SlugStrategy::HostPath => SlugStrategy::HostPath,
            rb_config::SlugStrategy::PathQuery => SlugStrategy::PathQuery,
        },
        lfs_check: config.lfs_check,
        fail_on_meta_mismatch: config.fail_on_meta_mismatch,
    }
}

/// This machine and build as `meta.json` records them, at `now` (Unix seconds).
pub fn machine_meta(now: u64) -> BaselineMeta {
    let (cef, chromium) = split_cef(BUILD.cef);
    BaselineMeta {
        machine: meta::machine_name(|name| std::env::var(name).ok()),
        os: std::env::consts::OS.to_owned(),
        engine_version: chromium,
        cef_version: Some(cef),
        app_version: BUILD.version.to_owned(),
        created_at: now,
        ..BaselineMeta::default()
    }
}

fn now() -> std::time::Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
}

/// The views of `target` in layout order, as a run needs them.
fn targets(host: &Arc<Host>, target: Target) -> Result<Vec<ViewTarget>, VisualRunError> {
    let ids = host.target_views(Some(target))?;
    let views: Vec<ViewTarget> = host
        .views()
        .into_iter()
        .filter(|info| ids.contains(&info.id))
        .map(|info| {
            let height = host
                .capture_target(info.id)
                .map(|target| target.height)
                .filter(|height| *height > 0)
                .unwrap_or(info.height);
            ViewTarget {
                id: info.id,
                name: info
                    .breakpoint
                    .clone()
                    .unwrap_or_else(|| format!("{}x{height}", info.width)),
                label: info.label,
                url: info.url,
                engine: match info.engine {
                    EngineKind::Chromium => "chromium",
                    EngineKind::Firefox => "firefox",
                    EngineKind::Webkit => "webkit",
                }
                .to_owned(),
                viewport: Viewport {
                    width: info.width,
                    height,
                },
            }
        })
        .collect();
    if views.is_empty() {
        return Err(VisualRunError::NoViews);
    }
    Ok(views)
}

/// Compares the views of `args` with their baselines, reports the findings and waits: the MCP
/// tool `visual_compare` and the command.
pub async fn compare(
    core: &Arc<Core>,
    args: &CompareArgs,
) -> Result<CompareAnswer, VisualRunError> {
    let _running = Running::start().ok_or(VisualRunError::Busy)?;
    let host = core.host().clone();
    let root = host.project_root().ok_or(VisualRunError::NoProject)?;
    let options = options(&host.config().visual, args.threshold);
    let views = targets(&host, args.target.unwrap_or(Target::All))?;
    let since = now();
    let run_id = rb_plugin_host::run_id(since.as_millis());
    let diff_dir = crate::capture::settings(core)
        .output_dir(&root)?
        .join("visual")
        .join(&run_id);
    let setup = RunSetup {
        options,
        machine: machine_meta(since.as_secs()),
        diff_dir: Some(diff_dir.clone()),
    };
    let shooter =
        HostShooter::new(host.clone()).with_clean(crate::capture::clean_plan(core, args.clean()));
    let comparisons =
        run::compare_views(&shooter, &BaselineStore::new(&root), &views, &setup).await;
    let request = findings::request(
        run_id.clone(),
        since.as_secs(),
        args.target.unwrap_or(Target::All),
        &views,
        &comparisons,
        options.fail_on_meta_mismatch,
    );
    let reporter = core.clone();
    tokio::task::spawn_blocking(move || crate::checks::run_request(&reporter, request))
        .await
        .map_err(|error| VisualRunError::Failed(error.to_string()))?
        .map_err(|error| VisualRunError::Failed(error.to_string()))?;
    let folder = diff_dir.is_dir().then(|| diff_dir.display().to_string());
    Ok(CompareAnswer::of(run_id, comparisons, folder))
}

/// Stores the views of `args` as the new baselines and checks Git LFS: the MCP tool
/// `visual_baseline_update` and the command.
pub async fn update(core: &Arc<Core>, args: &UpdateArgs) -> Result<UpdateAnswer, VisualRunError> {
    let _running = Running::start().ok_or(VisualRunError::Busy)?;
    let host = core.host().clone();
    let root = host.project_root().ok_or(VisualRunError::NoProject)?;
    let options = options(&host.config().visual, None);
    let views = targets(&host, args.target.unwrap_or(Target::All))?;
    let setup = RunSetup {
        options,
        machine: machine_meta(now().as_secs()),
        diff_dir: None,
    };
    let shooter =
        HostShooter::new(host.clone()).with_clean(crate::capture::clean_plan(core, args.clean()));
    let baselines = run::update_views(&shooter, &BaselineStore::new(&root), &views, &setup).await;
    let checked = if options.lfs_check && baselines.iter().any(|entry| entry.path.is_some()) {
        let status = blocking(move || rb_visual::lfs::check(&root, &lfs::SystemGit)).await?;
        Some(LfsAnswer {
            tracked: status.tracked,
            installed: status.installed,
        })
    } else {
        None
    };
    Ok(UpdateAnswer {
        baselines,
        lfs: checked,
    })
}

async fn blocking<T: Send + 'static>(
    job: impl FnOnce() -> T + Send + 'static,
) -> Result<T, VisualRunError> {
    tokio::task::spawn_blocking(job)
        .await
        .map_err(|error| VisualRunError::Failed(error.to_string()))
}

async fn report_compare(core: &Arc<Core>, args: &CompareArgs) {
    match compare(core, args).await {
        Ok(answer) => {
            let level = if answer.regressions == 0 {
                NotificationLevel::Info
            } else {
                NotificationLevel::Warning
            };
            let args = [
                ("regressions", answer.regressions.to_string()),
                ("count", answer.comparisons.len().to_string()),
                ("folder", answer.folder.clone().unwrap_or_default()),
            ];
            core.notify(notice(level, COMPARED_KEY, &args));
            if answer.unreliable > 0 {
                let args = [("count", answer.unreliable.to_string())];
                core.notify(notice(NotificationLevel::Warning, UNRELIABLE_KEY, &args));
            }
        }
        Err(error) => failed(core, &error),
    }
}

async fn report_update(core: &Arc<Core>, args: &UpdateArgs) {
    match update(core, args).await {
        Ok(answer) => {
            let saved = answer
                .baselines
                .iter()
                .filter(|entry| entry.path.is_some())
                .count();
            let level = if saved == answer.baselines.len() {
                NotificationLevel::Info
            } else {
                NotificationLevel::Warning
            };
            let args = [
                ("count", saved.to_string()),
                ("failed", (answer.baselines.len() - saved).to_string()),
            ];
            core.notify(notice(level, UPDATED_KEY, &args));
            if answer
                .lfs
                .is_some_and(|lfs| !(lfs.tracked && lfs.installed))
            {
                core.notify(notice(NotificationLevel::Warning, LFS_MISSING_KEY, &[]));
            }
        }
        Err(error) => failed(core, &error),
    }
}

/// `visual.lfs_setup`: appends the `.gitattributes` line and runs `git lfs install --local`.
async fn lfs_setup(core: &Arc<Core>) {
    let Some(root) = core.host().project_root() else {
        failed(core, &VisualRunError::NoProject);
        return;
    };
    let done = blocking(move || rb_visual::lfs::set_up(&root, &lfs::SystemGit)).await;
    match done {
        Ok(Ok(_)) => core.notify(notice(NotificationLevel::Info, LFS_DONE_KEY, &[])),
        Ok(Err(error)) => {
            tracing::warn!(%error, "the Git LFS set-up failed");
            let args = [("error", error.to_string())];
            core.notify(notice(NotificationLevel::Error, LFS_FAILED_KEY, &args));
        }
        Err(error) => failed(core, &error),
    }
}

fn failed(core: &Core, error: &VisualRunError) {
    tracing::warn!(%error, "the visual run failed");
    let args = [("error", error.to_string())];
    core.notify(notice(NotificationLevel::Error, FAILED_KEY, &args));
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

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
