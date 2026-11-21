//! One recording of several views at once over the app's CDP connection: `Page.startScreencast`
//! per view session (`jpeg`, the quality, `maxWidth`/`maxHeight` = [`capture_size`]),
//! every `Page.screencastFrame` of a recorded session acked at once with
//! `Page.screencastFrameAck` (Chromium sends no next frame before), decoded and handed to the
//! view's writer, which paces it to a constant frame rate ([`super::pacing`]) and writes it to
//! the view's [`FrameSink`] (ffmpeg). Start and stop are shared by all views.
//!
//! Everything runs as tasks on the host's runtime - one dispatcher for the frame tap, one
//! writer per view - so nothing waits on the UI thread. A writer that falls behind loses frames
//! (the one held is shown longer); the ack never waits for it, so a slow encoder never slows the
//! page (RB-090-52). The frames a view's writer could not take are counted and named in its
//! result ([`RecordedFile::dropped`]) and the log. The recording ends at
//! [`Recording::stop`] or by itself after the maximum duration; then the screencasts stop, the
//! writers write the last frame up to the stop time and the encoders finish their files. A writer
//! that has not finished [`RecorderOptions::stop_timeout`] after the stop is given up (its
//! encoder stopped) with an error for its view, so a recording always ends.
//!
//! The screencast captures the view's whole widget, not the page (acceptance 2026-11-17: the
//! page small in the top left on white). Below a scale of 1 Chromium sizes the widget to the
//! unscaled viewport (`Emulation.setDeviceMetricsOverride` without `dontSetVisibleSize`,
//! `crate::fit::DeviceMetrics::to_params`) and paints the page into its top-left `scale` share;
//! the cell shows that share, the rest is never on screen. [`page_share`] is that share, the
//! encoder cuts it out (`super::encoder::EncodeSpec::page_share`), and [`capture_size`] asks for
//! a frame large enough that the cut keeps every pixel the view paints. The views on screen are
//! left as they are.
//!
//! A pause ([`Recording::pauser`], RB-090-68) is left out of every file: each writer's pacer
//! skips the paused time, and the maximum duration counts recorded time only.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use base64::Engine as _;
use rb_engine::ViewId;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::{mpsc, watch};

use super::encoder::{FrameSink, MIN_PAGE_SHARE};
use super::pacing::{Pacer, Pauses};
use super::pause::Pauser;
use crate::cdp::CdpRouter;

/// The screencast's frame event.
pub const FRAME_EVENT: &str = "Page.screencastFrame";
/// Frame events waiting for the dispatcher.
const TAP_CAPACITY: usize = 512;
/// Frames waiting for one view's writer.
const WRITER_CAPACITY: usize = 8;
/// The longer side of a screencast frame at most, in device pixels (Chromium sends at most the
/// widget's own size anyway).
pub const MAX_CAPTURE_SIDE: u32 = 8192;
/// How long the writers may take after the stop: the encoder's finish and some frames.
pub const STOP_TIMEOUT: Duration = Duration::from_secs(
    super::encoder::FINISH_TIMEOUT.as_secs() + 6 * super::encoder::WRITE_TIMEOUT.as_secs(),
);

/// One view to record.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewPlan {
    /// The view.
    pub view: ViewId,
    /// Its flat CDP session.
    pub session: String,
    /// The frame width in device pixels (the viewport times the DPR, bounded).
    pub width: u32,
    /// The frame height in device pixels.
    pub height: u32,
    /// The share of each screencast frame's width and height the page fills, from the top
    /// left ([`page_share`] of the view's scale).
    pub page_share: f64,
    /// The file it is encoded to.
    pub file: PathBuf,
}

/// How a recording runs.
#[derive(Clone)]
pub struct RecorderOptions {
    /// Frames per second of the files.
    pub fps: u32,
    /// The JPEG quality of the frames, 1 to 100.
    pub quality: u32,
    /// The recording stops by itself after this.
    pub max_duration: Duration,
    /// Now, in seconds since the Unix epoch - the clock of the frames' timestamps.
    pub clock: Arc<dyn Fn() -> f64 + Send + Sync>,
    /// How long the writers may take after the stop ([`STOP_TIMEOUT`]).
    pub stop_timeout: Duration,
}

/// Seconds since the Unix epoch, as `metadata.timestamp` counts them.
pub fn wall_clock() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64())
}

/// The share of a screencast frame's width and height the page of a view at `scale` fills, from
/// its top-left corner: `scale` below 1 (at least [`MIN_PAGE_SHARE`]), else all of it; an
/// unusable scale counts as 1.
#[must_use]
pub fn page_share(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        scale.clamp(MIN_PAGE_SHARE, 1.0)
    } else {
        1.0
    }
}

/// The frame the screencast is asked for: the planned `width` x `height` over the page's
/// `share`, so the share keeps the planned size's pixels where the view paints them; the longer
/// side at most [`MAX_CAPTURE_SIDE`], the aspect kept.
#[must_use]
pub fn capture_size(width: u32, height: u32, share: f64) -> (u32, u32) {
    let share = page_share(share);
    let w = f64::from(width.max(1)) / share;
    let h = f64::from(height.max(1)) / share;
    let fit = (f64::from(MAX_CAPTURE_SIDE) / w.max(h)).min(1.0);
    // At most MAX_CAPTURE_SIDE: always inside u32.
    ((w * fit).round() as u32, (h * fit).round() as u32)
}

/// What one view gave.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedFile {
    /// The view.
    pub view: ViewId,
    /// The file.
    pub path: String,
    /// The frames written (at the frame rate).
    pub frames: u64,
    /// The length of the file in seconds.
    pub seconds: f64,
    /// The screencast frames the encoder did not take in time and that were left out (the frame
    /// before them shown longer); the page never waited for them (RB-090-52).
    pub dropped: u64,
    /// Why the file is missing or incomplete.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One decoded frame.
struct Frame {
    jpeg: Vec<u8>,
    timestamp: f64,
}

/// Where a recorded session's frames go: its writer, and the count of those it could not take.
struct Route {
    session: String,
    writer: mpsc::Sender<Frame>,
    dropped: Arc<AtomicU64>,
}

/// A running recording.
pub struct Recording {
    stop: watch::Sender<bool>,
    done: watch::Receiver<Option<Arc<Vec<RecordedFile>>>>,
    pauser: Pauser,
}

impl Recording {
    /// Starts the screencasts of `views`, each written to its sink; a view whose screencast
    /// does not start ends with its error at once, the others record.
    pub async fn start(
        cdp: Arc<CdpRouter>,
        views: Vec<(ViewPlan, Box<dyn FrameSink>)>,
        options: RecorderOptions,
    ) -> Self {
        // The tap first: no frame between the start and the subscription is lost.
        let mut tap = cdp.tap(FRAME_EVENT, TAP_CAPACITY);
        let (stop, mut stopped) = watch::channel(false);
        let (finished, done) = watch::channel(None);
        let pauser = Pauser::new(Arc::clone(&options.clock));
        let max_secs = options.max_duration.as_secs_f64();
        let mut routes: Vec<Route> = Vec::new();
        let mut writers = Vec::new();
        for (plan, sink) in views {
            let (frames, inbox) = mpsc::channel(WRITER_CAPACITY);
            let dropped = Arc::new(AtomicU64::new(0));
            let (max_width, max_height) = capture_size(plan.width, plan.height, plan.page_share);
            let started = cdp
                .call(
                    "Page.startScreencast",
                    json!({
                        "format": "jpeg",
                        "quality": options.quality.clamp(1, 100),
                        "maxWidth": max_width,
                        "maxHeight": max_height,
                        "everyNthFrame": 1,
                    }),
                    Some(&plan.session),
                )
                .await;
            let failure = match started {
                Ok(_) => {
                    routes.push(Route {
                        session: plan.session.clone(),
                        writer: frames,
                        dropped: Arc::clone(&dropped),
                    });
                    None
                }
                // The sender is dropped: the writer closes its sink at once.
                Err(error) => Some(error.to_string()),
            };
            let pacer = Pacer::new(options.fps, max_secs);
            let clock = Arc::clone(&options.clock);
            let place = (plan.view, plan.file.display().to_string());
            let counted = Arc::clone(&dropped);
            let pauses = pauser.subscribe();
            let writer = tokio::spawn(write_view(plan, inbox, sink, pacer, clock, pauses, failure));
            writers.push((place, counted, writer));
        }
        let max_duration = options.max_duration;
        let stop_timeout = options.stop_timeout;
        let mut pause_log = pauser.subscribe();
        tokio::spawn(async move {
            let begun = tokio::time::Instant::now();
            let deadline = tokio::time::sleep(max_duration);
            tokio::pin!(deadline);
            // While paused the maximum waits; after a resume it is that much later.
            let (mut paused, mut following) = (false, true);
            loop {
                tokio::select! {
                    _ = stopped.changed() => break,
                    () = &mut deadline, if !paused => break,
                    changed = pause_log.changed(), if following => match changed {
                        Ok(()) => {
                            let pauses = pause_log.borrow_and_update().clone();
                            paused = pauses.is_paused();
                            let held = Duration::try_from_secs_f64(pauses.ended_secs()).unwrap_or_default();
                            if let Some(at) = begun.checked_add(max_duration.saturating_add(held)) {
                                deadline.as_mut().reset(at);
                            }
                        }
                        Err(_) => following = false,
                    },
                    event = tap.recv() => match event {
                        Some(event) => route(&cdp, &routes, event.session_id.as_deref(), &event.params),
                        None => break,
                    },
                }
            }
            for route in &routes {
                // A view that closed meanwhile has nothing left to stop.
                let _ = cdp
                    .call("Page.stopScreencast", json!({}), Some(&route.session))
                    .await;
            }
            drop(routes);
            let deadline = tokio::time::Instant::now() + stop_timeout;
            let mut files = Vec::new();
            for ((view, path), dropped, mut writer) in writers {
                let dropped = dropped.load(Ordering::Relaxed);
                if dropped > 0 {
                    tracing::info!(
                        view = view.0,
                        dropped,
                        "the encoder fell behind; frames were left out, the page never waited"
                    );
                }
                match tokio::time::timeout_at(deadline, &mut writer).await {
                    Ok(Ok(mut file)) => {
                        file.dropped = dropped;
                        files.push(file);
                    }
                    Ok(Err(_)) => {}
                    Err(_) => {
                        // Dropping the writer's sink stops its encoder (`kill_on_drop`).
                        writer.abort();
                        tracing::warn!(view = view.0, "a video writer hung and was given up");
                        files.push(RecordedFile {
                            view,
                            path,
                            frames: 0,
                            seconds: 0.0,
                            dropped,
                            error: Some(format!(
                                "the encoder did not finish within {} s of the stop",
                                stop_timeout.as_secs()
                            )),
                        });
                    }
                }
            }
            let _ = finished.send(Some(Arc::new(files)));
        });
        Self { stop, done, pauser }
    }

    /// Waits until the recording ends by itself (the maximum duration) or by a stop.
    pub async fn finished(&mut self) -> Arc<Vec<RecordedFile>> {
        match self.done.wait_for(Option::is_some).await {
            Ok(files) => files.clone().unwrap_or_default(),
            Err(_) => Arc::default(),
        }
    }

    /// Stops the recording and waits for the files.
    pub async fn stop(mut self) -> Arc<Vec<RecordedFile>> {
        let _ = self.stop.send(true);
        self.finished().await
    }

    /// A handle that stops the recording from elsewhere.
    pub fn stopper(&self) -> watch::Sender<bool> {
        self.stop.clone()
    }

    /// A handle that pauses and resumes the recording (RB-090-68).
    pub fn pauser(&self) -> Pauser {
        self.pauser.clone()
    }
}

/// Hands one frame event of a recorded session to its writer, then acks it. A frame of
/// another session is someone else's (DevTools' screencast) and left alone.
fn route(cdp: &Arc<CdpRouter>, routes: &[Route], session: Option<&str>, params: &Value) {
    let Some(session) = session else {
        return;
    };
    let Some(route) = routes.iter().find(|route| route.session == session) else {
        return;
    };
    let decoded = params
        .get("data")
        .and_then(Value::as_str)
        .and_then(|data| base64::engine::general_purpose::STANDARD.decode(data).ok());
    let timestamp = params
        .pointer("/metadata/timestamp")
        .and_then(Value::as_f64);
    if let (Some(jpeg), Some(timestamp)) = (decoded, timestamp)
        && route.writer.try_send(Frame { jpeg, timestamp }).is_err()
    {
        // A full writer loses this frame; the one it holds is shown longer. Never wait here:
        // the ack below keeps the page's screencast going.
        route.dropped.fetch_add(1, Ordering::Relaxed);
    }
    let number = params.get("sessionId").cloned().unwrap_or(Value::Null);
    let cdp = Arc::clone(cdp);
    let session = session.to_owned();
    tokio::spawn(async move {
        if let Err(error) = cdp
            .call(
                "Page.screencastFrameAck",
                json!({ "sessionId": number }),
                Some(&session),
            )
            .await
        {
            tracing::debug!(%error, "a screencast frame was not acked");
        }
    });
}

/// One view's writer: paces the frames into the sink until the dispatcher lets go, then
/// writes the last frame up to the stop time and finishes the file.
async fn write_view(
    plan: ViewPlan,
    mut inbox: mpsc::Receiver<Frame>,
    mut sink: Box<dyn FrameSink>,
    mut pacer: Pacer,
    clock: Arc<dyn Fn() -> f64 + Send + Sync>,
    mut pauses: watch::Receiver<Pauses>,
    failure: Option<String>,
) -> RecordedFile {
    let mut error = failure;
    let mut held: Option<Vec<u8>> = None;
    while let Some(frame) = inbox.recv().await {
        if pauses.has_changed().unwrap_or(false) {
            pacer.set_pauses(pauses.borrow_and_update().clone());
        }
        let repeat = pacer.advance(frame.timestamp);
        if error.is_none()
            && let Some(jpeg) = &held
            && let Err(failed) = write_copies(sink.as_mut(), jpeg, repeat).await
        {
            error = Some(failed);
        }
        held = Some(frame.jpeg);
    }
    pacer.set_pauses(pauses.borrow().clone());
    let repeat = pacer.finish(clock());
    if error.is_none() {
        match &held {
            Some(jpeg) => {
                if let Err(failed) = write_copies(sink.as_mut(), jpeg, repeat).await {
                    error = Some(failed);
                }
            }
            None => error = Some("the view painted no frame".to_owned()),
        }
    }
    let finished = sink.finish().await;
    if error.is_none()
        && let Err(failed) = finished
    {
        error = Some(failed);
    }
    RecordedFile {
        view: plan.view,
        path: plan.file.display().to_string(),
        frames: pacer.written(),
        seconds: pacer.duration_secs(),
        dropped: 0,
        error,
    }
}

async fn write_copies(sink: &mut dyn FrameSink, jpeg: &[u8], copies: u64) -> Result<(), String> {
    for _ in 0..copies {
        sink.write(jpeg)
            .await
            .map_err(|error| format!("the encoder took no more frames: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "recorder_tests.rs"]
mod tests;
