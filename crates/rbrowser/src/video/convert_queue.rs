//! The Videos panel's exports and conversions (RB-090-52), one at a time in the background: the
//! panel's `convertVideo` queues a job ([`enqueue`]), a worker on the host's runtime runs them in
//! order ([`super::convert_run::make`], ffmpeg below the app's priority), the status bar says how
//! far it is (a notice at start, at each quarter and at the end) and the panel shows the share
//! (`videosChanged` at every tenth). The list carries what waits and runs ([`snapshot`]); a
//! recording with a job is not deleted meanwhile ([`busy_recordings`]). The app's end cancels
//! the running job - ffmpeg stopped, its half-written file removed - and drops the rest
//! ([`shutdown`]).

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rb_ipc::{CoreToShell, Notification, NotificationLevel, VideoConversion, VideoTarget};
use tokio::sync::watch;

use super::convert::{ConvertError, Encoder, mp4_encoder};
use super::convert_run::{CANCELLED, EncodeWith, Job, make, plan};
use super::encoder::{encoder_threads, logical_cores};
use super::ffmpeg;
use super::registry::Registry;
use crate::cdp::lock;
use crate::control::Core;

/// A job started; placeholders `{name}`, `{target}`.
pub const STARTED_KEY: &str = "video.notice.convert_started";
/// A job waits for others; placeholders `{target}`, `{count}`.
pub const WAITING_KEY: &str = "video.notice.convert_waiting";
/// A quarter is done; placeholders `{target}`, `{percent}`.
pub const PROGRESS_KEY: &str = "video.notice.convert_progress";
/// The result is there; placeholders `{name}`, `{target}`.
pub const DONE_KEY: &str = "video.notice.convert_done";
/// A job failed; placeholders `{target}`, `{error}`.
pub const FAILED_KEY: &str = "video.notice.convert_failed";

/// The jobs of the process.
static QUEUE: Mutex<Queue> = Mutex::new(Queue::new());

/// The running job.
#[derive(Debug)]
struct Running {
    job: Job,
    percent: Option<u8>,
    cancel: watch::Sender<bool>,
}

/// What waits and runs; the worker is the one that takes from it.
#[derive(Debug, Default)]
pub(crate) struct Queue {
    waiting: VecDeque<Job>,
    running: Option<Running>,
    worker: bool,
    closed: bool,
}

impl Queue {
    const fn new() -> Self {
        Self {
            waiting: VecDeque::new(),
            running: None,
            worker: false,
            closed: false,
        }
    }

    /// Queues `job`; answers how many run or wait before it.
    pub(crate) fn push(&mut self, job: Job) -> Result<usize, ConvertError> {
        if self.closed {
            return Err(ConvertError::Ffmpeg("rBrowser is closing".to_owned()));
        }
        let same = |other: &Job| other.recording == job.recording && other.target == job.target;
        if self
            .running
            .as_ref()
            .is_some_and(|running| same(&running.job))
            || self.waiting.iter().any(same)
        {
            return Err(ConvertError::Queued);
        }
        let ahead = self.waiting.len() + usize::from(self.running.is_some());
        self.waiting.push_back(job);
        Ok(ahead)
    }

    /// The next job and its cancel signal, while none runs.
    pub(crate) fn next(&mut self) -> Option<(Job, watch::Receiver<bool>)> {
        if self.running.is_some() || self.closed {
            return None;
        }
        let job = self.waiting.pop_front()?;
        let (cancel, cancelled) = watch::channel(false);
        self.running = Some(Running {
            job: job.clone(),
            percent: None,
            cancel,
        });
        Some((job, cancelled))
    }

    /// The running job is at `percent`.
    pub(crate) fn progress(&mut self, percent: u8) {
        if let Some(running) = self.running.as_mut() {
            running.percent = Some(percent);
        }
    }

    /// The running job ended.
    pub(crate) fn done(&mut self) {
        self.running = None;
    }

    /// The app ends: the running job is told to stop, the waiting ones go, nothing new starts.
    pub(crate) fn close(&mut self) {
        self.closed = true;
        self.waiting.clear();
        if let Some(running) = &self.running {
            let _ = running.cancel.send(true);
        }
    }

    /// The jobs as the panel lists them, the running one first.
    pub(crate) fn snapshot(&self) -> Vec<VideoConversion> {
        let entry = |job: &Job, running: bool, percent: Option<u8>| VideoConversion {
            recording: job.recording.clone(),
            file: job.file.clone(),
            target: job.target.clone(),
            to: job.to,
            running,
            percent,
        };
        self.running
            .iter()
            .map(|running| entry(&running.job, true, running.percent))
            .chain(self.waiting.iter().map(|job| entry(job, false, None)))
            .collect()
    }

    /// The recordings with a job, each once.
    pub(crate) fn busy(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .running
            .iter()
            .map(|running| running.job.recording.clone())
            .chain(self.waiting.iter().map(|job| job.recording.clone()))
            .collect();
        ids.dedup();
        ids
    }
}

/// The quarter (25, 50, 75) `now` reached that `before` had not; the end has its own notice.
#[must_use]
pub fn quarter_crossed(before: Option<u8>, now: u8) -> Option<u8> {
    let quarter = |percent: u8| percent / 25;
    let reached = quarter(now.min(99));
    (reached > before.map_or(0, quarter) && reached > 0).then_some(reached * 25)
}

/// The exports and conversions waiting and running, for the panel's list.
#[must_use]
pub fn snapshot() -> Vec<VideoConversion> {
    lock(&QUEUE).snapshot()
}

/// The recordings with an export or conversion: not deleted while it runs or waits.
#[must_use]
pub fn busy_recordings() -> Vec<String> {
    lock(&QUEUE).busy()
}

/// The app ends: the running job stops and removes its half-written file, the rest is dropped.
pub fn shutdown() {
    lock(&QUEUE).close();
}

/// A status bar notice without a settings link.
fn status(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect::<BTreeMap<_, _>>(),
        settings_key: None,
    })
}

/// Queues making video `file` of recording `id` of `core`'s project into `to`, the registry in
/// `dir`, and starts the worker when none runs.
///
/// # Errors
///
/// MP4 on a platform without an H.264 encoder, no project, or what [`plan`] and the queue
/// refuse.
pub fn enqueue(
    core: &Core,
    dir: Option<&Path>,
    id: &str,
    file: &str,
    to: VideoTarget,
) -> Result<(), ConvertError> {
    if to == VideoTarget::Mp4 && mp4_encoder().is_none() {
        return Err(ConvertError::Unsupported);
    }
    let Some(worker) = core.arc() else {
        return Err(ConvertError::Ffmpeg("the window is closed".to_owned()));
    };
    let root = core
        .host()
        .project_root()
        .ok_or(super::library::LibraryError::NotFound)?;
    let recorded = dir
        .map(|dir| Registry::for_project(dir, &root).load())
        .unwrap_or_default();
    let recording = super::app::running_folder()
        .and_then(|folder| folder.file_name()?.to_str().map(str::to_owned));
    let running: Vec<&str> = recording.as_deref().into_iter().collect();
    let job = plan(&root, &recorded, id, file, to, &running)?;
    let (name, target) = (job.file.clone(), job.target.clone());
    let (ahead, start) = {
        let mut queue = lock(&QUEUE);
        let ahead = queue.push(job)?;
        let start = !queue.worker;
        queue.worker = true;
        (ahead, start)
    };
    tracing::info!(recording = %id, file = %name, result = %target, ahead, "video conversion queued");
    if ahead == 0 {
        core.notify(status(
            NotificationLevel::Info,
            STARTED_KEY,
            &[("name", name), ("target", target)],
        ));
    } else {
        core.notify(status(
            NotificationLevel::Info,
            WAITING_KEY,
            &[("target", target), ("count", ahead.to_string())],
        ));
    }
    if start {
        let dir = dir.map(Path::to_path_buf);
        core.host()
            .runtime()
            .spawn(async move { work(worker, dir).await });
    }
    Ok(())
}

/// Runs the queued jobs one after the other until none is left.
async fn work(core: Arc<Core>, dir: Option<PathBuf>) {
    loop {
        let next = {
            let mut queue = lock(&QUEUE);
            let next = queue.next();
            if next.is_none() {
                queue.worker = false;
            }
            next
        };
        let Some((job, mut cancel)) = next else {
            return;
        };
        let result = run_one(&core, &job, dir.as_deref(), &mut cancel).await;
        lock(&QUEUE).done();
        let target = job.target.clone();
        match result {
            Ok(()) => core.notify(status(
                NotificationLevel::Success,
                DONE_KEY,
                &[("name", job.file.clone()), ("target", target)],
            )),
            Err(ConvertError::Ffmpeg(error)) if error == CANCELLED => {
                tracing::info!(result = %job.target, "video conversion cancelled");
            }
            Err(error) => {
                tracing::warn!(%error, result = %job.target, "video conversion failed");
                core.notify(status(
                    NotificationLevel::Error,
                    FAILED_KEY,
                    &[("target", target), ("error", error.to_string())],
                ));
            }
        }
        core.send(&CoreToShell::VideosChanged);
    }
}

/// One job: the ffmpeg and encoder in effect, then [`make`] with the progress reported.
async fn run_one(
    core: &Arc<Core>,
    job: &Job,
    dir: Option<&Path>,
    cancel: &mut watch::Receiver<bool>,
) -> Result<(), ConvertError> {
    let settings = super::app::settings(core);
    let Some(found) = ffmpeg::locate(&settings, ffmpeg::default_store().as_ref()) else {
        if let Ok(entry) = crate::tools::ffmpeg_entry() {
            core.notify(ffmpeg::needed_notice(&entry));
        }
        return Err(ConvertError::Ffmpeg(
            super::VideoError::FfmpegMissing.to_string(),
        ));
    };
    ffmpeg::probe(&found).await.map_err(ConvertError::Ffmpeg)?;
    let encoder = match job.to {
        VideoTarget::Mp4 => {
            let name = mp4_encoder().ok_or(ConvertError::Unsupported)?;
            let known = ffmpeg::encoders(&found.path)
                .await
                .map_err(ConvertError::Ffmpeg)?;
            if !known.contains(name) {
                return Err(ConvertError::Ffmpeg(format!(
                    "the ffmpeg at {} has no {name}, which rBrowser exports MP4 with",
                    found.path.display()
                )));
            }
            Encoder::Mp4(name)
        }
        VideoTarget::Webm => Encoder::Webm(
            ffmpeg::webm_codec(&found)
                .await
                .map_err(ConvertError::Ffmpeg)?,
        ),
    };
    // Beside a recording's encoders: all of them together at most half the cores.
    let recording = super::app::recording_views().map_or(0, |views| views.len());
    let with = EncodeWith {
        encoder,
        quality: settings.quality,
        threads: encoder_threads(logical_cores(), 1 + recording),
    };
    let notifier = Arc::clone(core);
    let target = job.target.clone();
    let mut before: Option<u8> = None;
    let mut report = move |percent: u8| {
        lock(&QUEUE).progress(percent);
        if before.is_none_or(|before| before / 10 != percent / 10) {
            notifier.send(&CoreToShell::VideosChanged);
        }
        if let Some(quarter) = quarter_crossed(before, percent) {
            notifier.notify(status(
                NotificationLevel::Info,
                PROGRESS_KEY,
                &[("target", target.clone()), ("percent", quarter.to_string())],
            ));
        }
        before = Some(percent);
    };
    make(job, &found.path, with, dir, cancel, &mut report)
        .await
        .map(|_| ())
}

#[cfg(test)]
#[path = "convert_queue_tests.rs"]
mod tests;
