//! Screenshots (RB-050-03): the commands `capture.*`, the MCP tool `screenshot`'s model
//! ([`tool`]) and the capture of the grid's views over their CDP sessions ([`cdp`]), never a
//! read of the screen. File names, the folder, the delay steps and the overview image are
//! `rb_visual`'s.
//!
//! Every capture runs on a page prepared by [`clean`] (RB-080-12): banners hidden, animations
//! frozen, as `capture.*` of Cookie banners & animations, the session's toggles
//! ([`clean_defs`]) and the call's `hideBanners`/`freezeAnimations` say.
//!
//! A command answers at once (`{"started": true}`) and the capture runs on the host's runtime:
//! the pick (element without a selector), the delay, the views in layout order; a notification
//! names the folder, or the error. [`screenshot`] is the same run for the MCP tool and the CLI,
//! which wait for its answer.

pub mod cdp;
pub mod chromium;
pub mod clean;
pub mod clean_defs;
pub mod full_page;
pub mod latest;
pub mod lazy;
pub mod lazy_script;
pub mod pick;
pub mod run;
#[cfg(test)]
pub(crate) mod scripted;
pub mod tool;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rb_commands::CommandError;
use rb_config::EngineKind;
use rb_engine::{EngineError, Target, ViewHost, ViewId};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_visual::overview::parse_color;
use rb_visual::{CaptureSettings, CleanOverrides, CleanPlan, Delay, VisualError};
use serde_json::{Value, json};

pub use cdp::{CaptureTarget, Shot};
pub use tool::{
    CaptureMode, PointArg, ScreenshotAnswer, ScreenshotArgs, ScreenshotRequest, ShotAnswer,
};

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;
use chromium::HostShooter;
use run::{RunContext, ViewMeta};

/// The commands of this module, in the order of the job file.
pub const COMMANDS: [&str; 6] = [
    "capture.views",
    "capture.full_page",
    "capture.all_viewport",
    "capture.element",
    "capture.overview",
    "capture.delay_cycle",
];
/// The notice after a capture; placeholders `{count}`, `{folder}`.
pub const SAVED_KEY: &str = "capture.notice.saved";
/// The notice that a capture failed; placeholder `{error}`.
pub const FAILED_KEY: &str = "capture.notice.failed";
/// The notice before a delayed capture; placeholder `{seconds}`.
pub const COUNTDOWN_KEY: &str = "capture.notice.countdown";
/// The hint of the pick mode.
pub const PICK_KEY: &str = "capture.notice.pick";
/// The notice after `capture.delay_cycle`; placeholder `{seconds}`.
pub const DELAY_KEY: &str = "capture.notice.delay";
/// The notice that a capture without a delay started (RB-090-85).
pub const CAPTURING_KEY: &str = "capture.notice.capturing";
/// The notice that a full page was cut at the image's limits (RB-099-11); placeholders
/// `{label}`, `{height}` (the image's rows).
pub const TRUNCATED_KEY: &str = "capture.notice.truncated";
/// The settings section the notices link to.
const SETTINGS_KEY: &str = "capture";

/// The delay `capture.delay_cycle` chose; `capture.defaultDelay` until the first cycle.
static DELAY: Mutex<Option<Delay>> = Mutex::new(None);
/// What `capture.toggle_hide_banners` and `capture.toggle_freeze` chose for the session
/// (RB-080-12); the settings until the first toggle.
static SESSION_CLEAN: Mutex<CleanOverrides> = Mutex::new(CleanOverrides {
    hide_banners: None,
    freeze_animations: None,
});

/// Why a capture gave nothing.
#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    /// The arguments do not fit.
    #[error("{0}")]
    Invalid(String),
    /// No view to capture.
    #[error("there is no view to capture")]
    NoViews,
    /// Files need an open project.
    #[error("open a project to save screenshots")]
    NoProject,
    /// Every view failed; the first reason.
    #[error("{0}")]
    Failed(String),
    /// The browser side.
    #[error(transparent)]
    Engine(#[from] EngineError),
    /// Naming, folder or image.
    #[error(transparent)]
    Visual(#[from] VisualError),
}

impl From<CaptureError> for CommandError {
    fn from(error: CaptureError) -> Self {
        match error {
            CaptureError::Invalid(text) => CommandError::InvalidArgs(text),
            other => CommandError::Failed(other.to_string()),
        }
    }
}

/// The preset of each capture command; `None` for an id that is not one.
fn preset(id: &str) -> Option<ScreenshotArgs> {
    let (target, mode, overview) = match id {
        "capture.views" => (None, CaptureMode::Viewport, false),
        "capture.full_page" => (Some(Target::All), CaptureMode::FullPage, false),
        "capture.all_viewport" => (Some(Target::All), CaptureMode::Viewport, false),
        "capture.element" => (None, CaptureMode::Element, false),
        "capture.overview" => (Some(Target::All), CaptureMode::Viewport, true),
        _ => return None,
    };
    Some(ScreenshotArgs {
        target,
        mode: Some(mode),
        overview: Some(overview),
        ..ScreenshotArgs::default()
    })
}

/// Runs the command `id` when it is this module's (`capture.*`); `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    if id == "capture.delay_cycle" {
        return Some(Ok(cycle_delay(core)));
    }
    let preset = preset(id)?;
    Some(start(core, args, &preset))
}

/// The settings in effect: the project's `capture` section over the user's.
pub fn settings(core: &Core) -> CaptureSettings {
    let project = serde_json::to_value(&core.host().config().capture).ok();
    let user = serde_json::to_value(&core.user_settings().capture).ok();
    let (settings, problems) = CaptureSettings::resolve(project.as_ref(), user.as_ref());
    for problem in problems {
        tracing::warn!(%problem, "a capture setting was not taken");
    }
    settings
}

/// The session's word on hiding and freezing (the toggles).
pub fn session_clean() -> CleanOverrides {
    *lock(&SESSION_CLEAN)
}

/// Sets the session's word on hiding and freezing (the toggles).
pub fn set_session_clean(overrides: CleanOverrides) {
    *lock(&SESSION_CLEAN) = overrides;
}

/// What a capture does to the page (RB-080-12): `call` over the session's toggles over the
/// settings; the deprecated `sitemap.hideSelectors` still adds its selectors.
pub fn clean_plan(core: &Core, call: CleanOverrides) -> CleanPlan {
    let extra = core.host().config().sitemap.hide_selectors.clone();
    settings(core).clean.plan(&extra, call.or(session_clean()))
}

fn current_delay(settings: &CaptureSettings) -> Delay {
    lock(&DELAY).unwrap_or(settings.default_delay)
}

fn cycle_delay(core: &Core) -> Value {
    let next = {
        let mut delay = lock(&DELAY);
        let next = delay.unwrap_or(settings(core).default_delay).next();
        *delay = Some(next);
        next
    };
    core.notify(notice(
        NotificationLevel::Info,
        DELAY_KEY,
        &[("seconds", next.secs().to_string())],
    ));
    json!({ "delaySeconds": next.secs() })
}

/// Starts the capture of a command and answers at once; its start (RB-090-85) and its result
/// come as notifications.
fn start(core: &Core, args: &Value, preset: &ScreenshotArgs) -> Result<Value, CommandError> {
    let args: ScreenshotArgs = match args {
        Value::Null => ScreenshotArgs::default(),
        args => serde_json::from_value(args.clone())
            .map_err(|error| CommandError::InvalidArgs(error.to_string()))?,
    };
    let settings = settings(core);
    let request = args
        .or(preset)
        .request(
            settings.element_padding,
            current_delay(&settings).duration(),
            true,
        )
        .map_err(CommandError::InvalidArgs)?;
    let core = core
        .arc()
        .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
    let delay_ms = u64::try_from(request.delay.as_millis()).unwrap_or(u64::MAX);
    if let Some(started) = capturing_notice(&request) {
        core.notify(started);
    }
    let runtime = core.host().runtime().clone();
    runtime.spawn(async move {
        match screenshot(&core, &request).await {
            Ok(answer) => {
                let count = answer.paths().len().to_string();
                let folder = answer.folder.clone().unwrap_or_default();
                core.notify(notice(
                    NotificationLevel::Info,
                    SAVED_KEY,
                    &[("count", count), ("folder", folder)],
                ));
                for cut in truncated_notices(&answer) {
                    core.notify(cut);
                }
            }
            Err(error) => {
                tracing::warn!(%error, "the screenshot failed");
                core.notify(notice(
                    NotificationLevel::Error,
                    FAILED_KEY,
                    &[("error", error.to_string())],
                ));
            }
        }
    });
    Ok(json!({ "started": true, "delayMs": delay_ms }))
}

/// A warning for each image that ends before its page does (RB-099-11).
fn truncated_notices(answer: &ScreenshotAnswer) -> Vec<CoreToShell> {
    answer
        .shots
        .iter()
        .filter(|shot| shot.truncated)
        .map(|shot| {
            notice(
                NotificationLevel::Warning,
                TRUNCATED_KEY,
                &[
                    ("label", shot.label.clone()),
                    ("height", shot.height.to_string()),
                ],
            )
        })
        .collect()
}

/// The notice that a command's capture started, so a full page of every view does not run
/// unseen (RB-090-85); none where another notice names the start - the countdown of a delay,
/// the pick of an element.
fn capturing_notice(request: &ScreenshotRequest) -> Option<CoreToShell> {
    let named = !request.delay.is_zero() || matches!(request.mode, CaptureMode::Element);
    (!named).then(|| notice(NotificationLevel::Info, CAPTURING_KEY, &[]))
}

/// Captures what `request` asks for and waits for it: the MCP tool, the CLI and the commands.
pub async fn screenshot(
    core: &Arc<Core>,
    request: &ScreenshotRequest,
) -> Result<ScreenshotAnswer, CaptureError> {
    let host = core.host().clone();
    let settings = settings(core);
    let ids = target_ids(&host, request)?;
    let folder = match (request.save, &request.folder, host.project_root()) {
        (false, _, _) => None,
        (true, Some(folder), _) => Some(folder.clone()),
        (true, None, Some(root)) => Some(settings.output_dir(&root)?),
        (true, None, None) => return Err(CaptureError::NoProject),
    };
    let mut plan = clean_plan(core, request.clean);
    plan.mask = request.mask_passwords;
    let shooter = HostShooter::new(host.clone()).with_clean(plan);
    let shot = match (request.mode, &request.selector) {
        (CaptureMode::Viewport, _) => Shot::Viewport,
        (CaptureMode::FullPage, _) => Shot::FullPage,
        (CaptureMode::Element, Some(selector)) => Shot::Element {
            selector: selector.clone(),
            padding: request.padding,
        },
        (CaptureMode::Element, None) => {
            let [view] = ids.as_slice() else {
                return Err(CaptureError::Invalid(
                    "picking an element works in one view".to_owned(),
                ));
            };
            let backend_node_id = match request.at {
                // The context menu named the point: the element under it (RB-080-43).
                Some(point) => shooter.node_at(*view, point).await?,
                None => {
                    core.notify(notice(NotificationLevel::Info, PICK_KEY, &[]));
                    shooter.pick(*view).await?
                }
            };
            Shot::Picked {
                backend_node_id,
                padding: request.padding,
            }
        }
    };
    if !request.delay.is_zero() {
        let seconds = request.delay.as_millis().div_ceil(1000).to_string();
        core.notify(notice(
            NotificationLevel::Info,
            COUNTDOWN_KEY,
            &[("seconds", seconds)],
        ));
        tokio::time::sleep(request.delay).await;
    }
    // The grid may have changed during the pick or the delay: the views as they are now.
    let views = metas(&host, &target_ids(&host, request)?);
    let env = host.shell_state().environment;
    let env_color = env.as_deref().and_then(|id| {
        host.config()
            .environments
            .iter()
            .find(|environment| environment.id == id)
            .and_then(|environment| environment.color.as_deref().and_then(parse_color))
    });
    let context = RunContext {
        env,
        env_color,
        pattern: settings.path_pattern.clone(),
        overview_labels: settings.overview_labels,
        folder,
        stamp: crate::local_time::stamp_now(),
    };
    let answer = run::run(
        &shooter,
        &views,
        &shot,
        request.mode,
        request.overview,
        &context,
    )
    .await?;
    // MCP's `rbrowser://screenshots/latest` (RB-095-01): the project's screenshot folder only.
    if request.folder.is_none()
        && let Some(root) = host.project_root()
    {
        latest::remember(&root, &answer);
    }
    Ok(answer)
}

/// The ids `request` names: its views, else its target's.
fn target_ids(host: &Host, request: &ScreenshotRequest) -> Result<Vec<ViewId>, CaptureError> {
    Ok(match &request.views {
        Some(views) => views.clone(),
        None => host.target_views(request.target)?,
    })
}

/// The views `ids` in layout order, with what a run needs of each.
fn metas(host: &Arc<Host>, ids: &[ViewId]) -> Vec<ViewMeta> {
    host.views()
        .into_iter()
        .filter(|info| ids.contains(&info.id))
        .map(|info| {
            let height = host
                .capture_target(info.id)
                .map(|target| target.height)
                .filter(|height| *height > 0)
                .unwrap_or(info.height);
            ViewMeta {
                id: info.id,
                name: info
                    .breakpoint
                    .clone()
                    .unwrap_or_else(|| format!("{}x{height}", info.width)),
                label: info.label,
                width: info.width,
                height,
                dpr: info.dpr,
                engine: match info.engine {
                    EngineKind::Chromium => "chromium",
                    EngineKind::Firefox => "firefox",
                    EngineKind::Webkit => "webkit",
                    EngineKind::Android => "android",
                    EngineKind::Safari => "safari",
                    EngineKind::Ios => "ios",
                }
                .to_owned(),
            }
        })
        .collect()
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
