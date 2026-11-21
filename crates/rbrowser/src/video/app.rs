//! The recording of the window: one at a time, shared by the commands and MCP. [`start`] picks
//! the ffmpeg, plans the views (their session and frame size: the viewport they show times the
//! DPR, at most [`MAX_SIDE`] px on the longer side, and the share of the screencast frame their
//! page fills, [`plan_frame`]), starts one encoder per view and the [`Recording`]; a
//! task on the host's runtime waits for its end - a stop or the maximum duration - hides the
//! views it showed again, keeps the result for `status` and tells the person. Hidden views are
//! shown for the recording: a throttled view paints no frame.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rb_config::EngineKind;
use rb_engine::{Target, ViewHost as _, ViewId, ViewInfo};
use rb_ipc::NotificationLevel;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::VideoError;
use super::encoder::{
    EncodeSpec, FfmpegSink, FrameSink, WebmCodec, encoder_threads, even, logical_cores,
};
use super::ffmpeg::{self, Ffmpeg, notice};
use super::recorder::{
    RecordedFile, RecorderOptions, Recording, STOP_TIMEOUT, ViewPlan, page_share, wall_clock,
};
use super::settings::{VideoSettings, file_names, make_run_folder};
use crate::cdp::lock;
use crate::control::Core;

/// The MCP tool.
pub const TOOL_NAME: &str = "record_video";
/// The longer side of a frame at most, in device pixels.
pub const MAX_SIDE: u32 = 3840;
/// A recording started; placeholders `{count}`, `{seconds}` (the maximum).
pub const STARTED_KEY: &str = "video.notice.started";
/// The files are written; placeholders `{count}`, `{folder}`.
pub const SAVED_KEY: &str = "video.notice.saved";
/// A recording failed; placeholder `{error}`.
pub const FAILED_KEY: &str = "video.notice.failed";
/// A view that cannot be recorded; placeholders `{view}`, `{engine}`.
pub const UNAVAILABLE_KEY: &str = "video.notice.unavailable";

/// The running recording.
static ACTIVE: Mutex<Option<Active>> = Mutex::new(None);
/// A start in progress (it awaits the browser).
static STARTING: AtomicBool = AtomicBool::new(false);
/// The last finished recording.
static LAST: Mutex<Option<Arc<Finished>>> = Mutex::new(None);
/// Numbers the recordings.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct Active {
    id: u64,
    started: Instant,
    views: Vec<ViewId>,
    folder: PathBuf,
    max: Duration,
    stop: watch::Sender<bool>,
    done: watch::Receiver<Option<Arc<Finished>>>,
    pauser: super::pause::Pauser,
}

/// A finished recording.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finished {
    /// The folder of the files.
    pub folder: String,
    /// One per view.
    pub files: Vec<RecordedFile>,
}

/// What to record.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartRequest {
    /// `"all"` or a view id; the active view when absent.
    #[serde(default)]
    pub target: Option<Target>,
    /// These views (MCP resolves a breakpoint id to them); before `target`.
    #[serde(default)]
    pub views: Option<Vec<ViewId>>,
}

/// The answer of a start.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Started {
    /// The views recorded.
    pub views: Vec<ViewId>,
    /// The folder the files go to.
    pub folder: String,
    /// The recording stops by itself after this.
    pub max_duration_sec: u64,
    /// The WebM codec: VP9, or VP8 where the ffmpeg has no VP9 (RB-090-52).
    pub codec: WebmCodec,
    /// The ffmpeg used.
    pub ffmpeg: Ffmpeg,
}

/// The state for `record_video status`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// A recording runs.
    pub recording: bool,
    /// Its views.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<ViewId>,
    /// Seconds since its start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_sec: Option<u64>,
    /// Its folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// It is paused (RB-090-68): the files leave the pause out.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub paused: bool,
    /// It stops by itself after this.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_duration_sec: Option<u64>,
    /// The ffmpeg a recording would use; `null` when none is there yet.
    pub ffmpeg: Option<Ffmpeg>,
    /// The last finished recording.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<Arc<Finished>>,
}

/// Video settings in effect: the project's `capture.video` over the user's, the user's tools.
pub fn settings(core: &Core) -> VideoSettings {
    let user = core.user_settings();
    VideoSettings::resolve(
        &core.host().config().capture.video,
        &user.capture.video,
        &user.tools,
    )
}

/// The folder of the running recording; `None` without one (the Videos panel cannot delete
/// it, RB-110-12).
pub fn running_folder() -> Option<PathBuf> {
    lock(&ACTIVE).as_ref().map(|active| active.folder.clone())
}

/// The running recording's pauser (RB-090-68); `None` without one.
pub fn running_pauser() -> Option<super::pause::Pauser> {
    lock(&ACTIVE).as_ref().map(|active| active.pauser.clone())
}

/// Whether a recording runs.
pub fn is_recording() -> bool {
    lock(&ACTIVE).is_some()
}

/// The views of the running recording; `None` without one (the context menu's Record/Stop
/// texts, RB-080-43 - a lock read, no ffmpeg lookup).
pub fn recording_views() -> Option<Vec<ViewId>> {
    lock(&ACTIVE).as_ref().map(|active| active.views.clone())
}

/// The frame size of a view: `width` x `height` CSS px at `dpr`, the longer side at most
/// [`MAX_SIDE`], both even.
pub fn frame_size(width: u32, height: u32, dpr: f64) -> (u32, u32) {
    let dpr = if dpr.is_finite() && dpr > 0.0 {
        dpr
    } else {
        1.0
    };
    let w = f64::from(width.max(1)) * dpr;
    let h = f64::from(height.max(1)) * dpr;
    let scale = (f64::from(MAX_SIDE) / w.max(h)).min(1.0);
    (
        even((w * scale).round() as u32),
        even((h * scale).round() as u32),
    )
}

/// What `view` is recorded at: the viewport it shows now (`width` x `height` CSS px - the height
/// its cell gives it, not the breakpoint's nominal one, which it does not show) at its DPR
/// ([`frame_size`]), and the share of each screencast frame its page fills ([`page_share`] of its
/// scale). Planned from the breakpoint's height, a frame of another aspect was stretched into
/// it (acceptance 2026-11-17: `desktop.webm` with a sample aspect ratio of 1353:1160).
pub fn plan_frame(view: &ViewInfo) -> (u32, u32, f64) {
    let (width, height) = frame_size(view.width, view.height, view.dpr);
    (width, height, page_share(view.scale))
}

/// Starts a recording of `request`'s views.
///
/// # Errors
///
/// One runs already, no ffmpeg (the consent notice is shown), no project, no Chromium view, or
/// the folder or an encoder could not be made.
pub async fn start(core: &Arc<Core>, request: &StartRequest) -> Result<Started, VideoError> {
    if is_recording() || STARTING.swap(true, Ordering::AcqRel) {
        return Err(VideoError::Busy);
    }
    let result = start_inner(core, request).await;
    STARTING.store(false, Ordering::Release);
    result
}

async fn start_inner(core: &Arc<Core>, request: &StartRequest) -> Result<Started, VideoError> {
    let settings = settings(core);
    let Some(ffmpeg) = ffmpeg::locate(&settings, ffmpeg::default_store().as_ref()) else {
        if let Ok(entry) = crate::tools::ffmpeg_entry() {
            core.notify(ffmpeg::needed_notice(&entry));
        }
        return Err(VideoError::FfmpegMissing);
    };
    ffmpeg::probe(&ffmpeg).await.map_err(VideoError::Ffmpeg)?;
    // Before anything is made: an ffmpeg without VP9 and VP8 says so now (RB-090-52).
    let codec = ffmpeg::webm_codec(&ffmpeg)
        .await
        .map_err(VideoError::Ffmpeg)?;
    let host = core.host().clone();
    let root = host.project_root().ok_or(VideoError::NoProject)?;
    let ids = match &request.views {
        Some(views) => views.clone(),
        None => host.target_views(request.target)?,
    };
    let mut plans = Vec::new();
    let mut names = Vec::new();
    let mut hidden = Vec::new();
    for info in host
        .views()
        .into_iter()
        .filter(|info| ids.contains(&info.id))
    {
        if info.engine != EngineKind::Chromium {
            let engine = format!("{:?}", info.engine).to_lowercase();
            core.notify(notice(
                NotificationLevel::Warning,
                UNAVAILABLE_KEY,
                &[("view", info.label.clone()), ("engine", engine)],
            ));
            continue;
        }
        let Some(target) = host.capture_target(info.id) else {
            continue;
        };
        let (width, height, page_share) = plan_frame(&info);
        if target.hidden {
            hidden.push(info.id);
        }
        names.push((
            info.id,
            info.breakpoint
                .clone()
                .unwrap_or_else(|| format!("{}x{}", info.width, info.height)),
        ));
        plans.push(ViewPlan {
            view: info.id,
            session: target.session,
            width,
            height,
            page_share,
            file: PathBuf::new(),
        });
    }
    if plans.is_empty() {
        return Err(VideoError::NoViews);
    }
    let stamp = crate::local_time::stamp_now();
    let folder = tokio::task::spawn_blocking(move || make_run_folder(&root, &stamp))
        .await
        .map_err(|error| VideoError::Failed(error.to_string()))?
        .map_err(|error| VideoError::Failed(error.to_string()))?;
    let files = file_names(&names);
    // All encoders together at most half the logical cores (RB-090-52).
    let threads = encoder_threads(logical_cores(), plans.len());
    let mut views: Vec<(ViewPlan, Box<dyn FrameSink>)> = Vec::new();
    for mut plan in plans {
        let name = files
            .iter()
            .find(|(view, _)| *view == plan.view)
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| format!("{}.video", plan.view.0));
        plan.file = folder.join(name);
        let spec = EncodeSpec {
            codec,
            fps: settings.fps,
            quality: settings.quality,
            width: plan.width,
            height: plan.height,
            threads,
            page_share: plan.page_share,
        };
        let sink =
            FfmpegSink::spawn(&ffmpeg.path, &spec, &plan.file).map_err(VideoError::Failed)?;
        views.push((plan, Box::new(sink)));
    }
    for view in &hidden {
        host.ui().set_hidden(*view, false);
    }
    let recorded: Vec<ViewId> = views.iter().map(|(plan, _)| plan.view).collect();
    let recording = Recording::start(
        host.cdp().clone(),
        views,
        RecorderOptions {
            fps: settings.fps,
            quality: settings.quality,
            max_duration: settings.max_duration,
            clock: Arc::new(wall_clock),
            stop_timeout: STOP_TIMEOUT,
        },
    )
    .await;
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (done_tx, done) = watch::channel(None);
    *lock(&ACTIVE) = Some(Active {
        id,
        started: Instant::now(),
        views: recorded.clone(),
        folder: folder.clone(),
        max: settings.max_duration,
        stop: recording.stopper(),
        done,
        pauser: recording.pauser(),
    });
    // The activity bar's entry and the toolbar's indicator (RB-090-67).
    crate::activity_bar::publish_video(
        &host,
        Some((
            recorded.clone(),
            crate::status::now_ms(),
            settings.max_duration.as_secs(),
        )),
    );
    supervise(core.clone(), recording, id, folder.clone(), hidden, done_tx);
    core.notify(notice(
        NotificationLevel::Info,
        STARTED_KEY,
        &[
            ("count", recorded.len().to_string()),
            ("seconds", settings.max_duration.as_secs().to_string()),
        ],
    ));
    Ok(Started {
        views: recorded,
        folder: folder.display().to_string(),
        max_duration_sec: settings.max_duration.as_secs(),
        codec,
        ffmpeg,
    })
}

/// Waits for the end of `recording` on the host's runtime and wraps it up.
fn supervise(
    core: Arc<Core>,
    mut recording: Recording,
    id: u64,
    folder: PathBuf,
    hidden: Vec<ViewId>,
    done: watch::Sender<Option<Arc<Finished>>>,
) {
    let runtime = core.host().runtime().clone();
    runtime.spawn(async move {
        let files = recording.finished().await;
        let host = core.host();
        for view in hidden {
            if host.capture_target(view).is_some_and(|now| now.hidden) {
                host.ui().set_hidden(view, true);
            }
        }
        let finished = Arc::new(Finished {
            folder: folder.display().to_string(),
            files: files.to_vec(),
        });
        let ended = {
            let mut active = lock(&ACTIVE);
            let ours = active.as_ref().is_some_and(|running| running.id == id);
            if ours {
                *active = None;
            }
            ours
        };
        if ended {
            crate::activity_bar::publish_video(host, None);
        }
        *lock(&LAST) = Some(Arc::clone(&finished));
        let _ = done.send(Some(Arc::clone(&finished)));
        let saved = finished
            .files
            .iter()
            .filter(|file| file.error.is_none())
            .count();
        let dropped: u64 = finished.files.iter().map(|file| file.dropped).sum();
        tracing::info!(saved, dropped, "the recording ended");
        match finished.files.iter().find_map(|file| file.error.clone()) {
            Some(error) if saved == 0 => {
                tracing::warn!(%error, "the recording gave no file");
                core.notify(notice(
                    NotificationLevel::Error,
                    FAILED_KEY,
                    &[("error", error)],
                ));
            }
            _ => core.notify(notice(
                NotificationLevel::Info,
                SAVED_KEY,
                &[
                    ("count", saved.to_string()),
                    ("folder", finished.folder.clone()),
                ],
            )),
        }
        // The Videos panel's registry, retention and list (RB-110-12): only the files this
        // recording wrote completely.
        let names = finished
            .files
            .iter()
            .filter(|file| file.error.is_none())
            .filter_map(|file| {
                std::path::Path::new(&file.path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned)
            })
            .collect();
        super::panel::on_saved(&core, &folder, names);
    });
}

/// Stops the running recording and waits for its files.
///
/// # Errors
///
/// [`VideoError::NotRecording`].
pub async fn stop() -> Result<Arc<Finished>, VideoError> {
    let (stop, mut done) = {
        let active = lock(&ACTIVE);
        let active = active.as_ref().ok_or(VideoError::NotRecording)?;
        (active.stop.clone(), active.done.clone())
    };
    let _ = stop.send(true);
    let finished = done
        .wait_for(Option::is_some)
        .await
        .map_err(|_| VideoError::Failed("the recording ended without a result".to_owned()))?
        .clone();
    finished.ok_or(VideoError::NotRecording)
}

/// The recording now, the ffmpeg a recording would use and the last result.
pub fn status(core: &Core) -> Status {
    let ffmpeg = ffmpeg::locate(&settings(core), ffmpeg::default_store().as_ref());
    let last = lock(&LAST).clone();
    let active = lock(&ACTIVE);
    match active.as_ref() {
        Some(active) => Status {
            recording: true,
            views: active.views.clone(),
            elapsed_sec: Some(active.started.elapsed().as_secs()),
            folder: Some(active.folder.display().to_string()),
            paused: active.pauser.is_paused(),
            max_duration_sec: Some(active.max.as_secs()),
            ffmpeg,
            last,
        },
        None => Status {
            recording: false,
            views: Vec::new(),
            elapsed_sec: None,
            folder: None,
            paused: false,
            max_duration_sec: None,
            ffmpeg,
            last,
        },
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
