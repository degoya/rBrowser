//! The browsers behind the render views (RB-090-83): windowless (off-screen) Chromium browsers
//! the window makes on its UI thread (`cef_app/render_views.rs`), never hidden CDP targets - a
//! hidden target has no compositor, so `Page.captureScreenshot` never answered there (the
//! owner's log, build b9dd1d3: every shot ran into the router's 30 s).
//!
//! The window makes a browser at its marker URL ([`RenderView::marker`], in the request context
//! of its session) and says when it exists ([`RenderViews::open`]); the backend
//! (`super::backend`) then finds its CDP target among `Target.getTargets` by that URL
//! ([`target_showing`]), hides it at once and attaches. A render view closes through its
//! browser's host ([`RenderViews::close`]); a full-page shot grows it to the page's height and
//! back ([`RenderViews::resize`], `super::render_surface`). [`backend_for`] gives the host's
//! window these browsers; the sitemap capture and the environment comparison both take it. A marker
//! ([`next_marker`]) is a number of one counter of the process and 128 random bits, so two runs
//! at once never share one and no page can guess one: a grid page that opens a window at a
//! marker it made up opens one no render view looks for. And a target with an opener (a popup)
//! or in another browser context than its session's is never taken for a render view
//! (security review, wave 090-w21).

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use rb_config::EngineKind;
use rb_engine::{EngineError, ViewHost as _};
use serde_json::{Value, json};

use super::backend::{CdpRenderBackend, MARKER_PREFIX, SessionTarget};
use crate::cdp::CdpRouter;
use crate::host::Host;

/// How long the window may take to make a render view's browser, and the browser to list it.
pub const FIND_DEADLINE: Duration = Duration::from_secs(10);
/// How often the targets are asked for a render view's marker, or for a closed one.
pub const FIND_POLL: Duration = Duration::from_millis(50);
/// How long a closed render view may stay listed before it is no longer waited for.
pub const CLOSE_WAIT: Duration = Duration::from_secs(2);

/// A windowless browser for the window to make.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderView {
    /// The URL it starts with (`super::backend::MARKER_PREFIX` and its number): how its CDP
    /// target is found.
    pub marker: String,
    /// The session whose request context it lives in (`default`: the global one).
    pub session: String,
    /// Its size in DIP, the breakpoint's viewport; never 0.
    pub width: u32,
    pub height: u32,
    /// The device scale factor its render handler reports (RB-090-90): the breakpoint's pixel
    /// ratio, at least 1 and at most [`MAX_SCALE`]. The emulation asks the compositor for that
    /// ratio; a windowless browser at 1 painted a surface too small for it, which the shot
    /// repeated (the iPad mini at 2: the page twice side by side).
    pub scale: f32,
}

/// The highest device scale factor of a render view: the highest pixel ratio a breakpoint has.
pub const MAX_SCALE: f32 = 10.0;

impl RenderView {
    /// A render view at `marker` in `session`, `width` x `height` (a side of 0 becomes 1) at
    /// pixel ratio `dpr` (none, `0`, or below 1: a scale of 1; a smaller ratio needs no smaller
    /// surface).
    #[must_use]
    pub fn new(marker: &str, session: &str, width: u32, height: u32, dpr: f64) -> Self {
        // In [1, MAX_SCALE] already, so the narrowing loses nothing a scale factor needs.
        let scale = if dpr.is_finite() {
            dpr.clamp(1.0, f64::from(MAX_SCALE)) as f32
        } else {
            1.0
        };
        Self {
            marker: marker.to_owned(),
            session: session.to_owned(),
            width: width.max(1),
            height: height.max(1),
            scale,
        }
    }
}

/// What [`RenderViews::open`] calls once: `Ok` when the browser exists, else why not.
pub type RenderViewMade = Box<dyn FnOnce(Result<(), String>) + Send>;

/// The size in DIP a resize waits for.
#[derive(Clone, Copy)]
struct Wanted {
    width: u32,
    height: u32,
}

impl Wanted {
    /// Whether a frame of `width_px` x `height_px` device pixels is at least this tall. The
    /// frame's height in DIP is its pixels scaled by its width (a resize keeps the width), so a
    /// frame of the old size never counts at any scale factor; the slack covers both sides
    /// rounded to whole pixels (code review, wave 090-w23).
    fn reached_by(self, width_px: u32, height_px: u32) -> bool {
        if width_px == 0 {
            return false;
        }
        let (width, height) = (u64::from(self.width), u64::from(self.height));
        let (width_px, height_px) = (u64::from(width_px), u64::from(height_px));
        height_px * width + height + width_px >= height * width_px
    }
}

/// A render view's size, which its window's render handler reports (`cef_app/render_views.rs`),
/// and the wait of a resize for the first frame painted that tall (RB-090-88).
pub struct ViewSize {
    width: AtomicU32,
    height: AtomicU32,
    /// The size in DIP a frame must reach, and whom to tell.
    waiting: Mutex<Option<(Wanted, RenderViewMade)>>,
}

impl ViewSize {
    /// A view of `width` x `height` DIP (a side of 0 becomes 1).
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: AtomicU32::new(width.max(1)),
            height: AtomicU32::new(height.max(1)),
            waiting: Mutex::new(None),
        }
    }

    /// The width and the height in DIP.
    #[must_use]
    pub fn get(&self) -> (u32, u32) {
        (
            self.width.load(Ordering::Acquire),
            self.height.load(Ordering::Acquire),
        )
    }

    /// Takes `width` x `height` DIP; `painted` is answered by the first frame at least that tall
    /// ([`ViewSize::painted`]). A resize still waiting is answered with an error first.
    pub fn resize(&self, width: u32, height: u32, painted: RenderViewMade) {
        let wanted = Wanted {
            width: width.max(1),
            height: height.max(1),
        };
        self.width.store(wanted.width, Ordering::Release);
        self.height.store(wanted.height, Ordering::Release);
        let earlier = crate::cdp::lock(&self.waiting).replace((wanted, painted));
        if let Some((_, earlier)) = earlier {
            earlier(Err("the render view was resized again".to_owned()));
        }
    }

    /// A frame of `width` x `height` device pixels was painted (at a scale factor above 1 more
    /// than DIP); answers the waiting resize when it is as tall in DIP.
    pub fn painted(&self, width: u32, height: u32) {
        let done = {
            let mut waiting = crate::cdp::lock(&self.waiting);
            match waiting.as_ref() {
                Some((wanted, _)) if wanted.reached_by(width, height) => waiting.take(),
                _ => None,
            }
        };
        if let Some((_, painted)) = done {
            painted(Ok(()));
        }
    }

    /// The view closed: a resize still waiting gets no frame.
    pub fn closed(&self) {
        let waiting = crate::cdp::lock(&self.waiting).take();
        if let Some((_, painted)) = waiting {
            painted(Err("the render view is closed".to_owned()));
        }
    }
}

/// The window's side of the render views.
pub trait RenderViews: Send + Sync {
    /// Makes the browser `view` asks for and calls `made`; returns at once.
    fn open(&self, view: RenderView, made: RenderViewMade);
    /// Gives the browser that started at `marker` the size `width` x `height` DIP and calls
    /// `painted` once it painted a frame at least that tall (RB-090-88, a full-page shot); a
    /// browser that is gone answers why.
    fn resize(&self, marker: String, width: u32, height: u32, painted: RenderViewMade);
    /// Closes the browser that started at `marker`; one that is gone already is no error.
    fn close(&self, marker: String);
}

/// A marker URL no render view of this process had and no page can guess: [`MARKER_PREFIX`],
/// the next number of one counter for every backend (a sitemap capture and an environment
/// comparison may run at once, and a closed view may still be listed for a moment), `-` and 128
/// random bits in hex.
///
/// # Errors
///
/// No randomness: no render view is made with a marker a page could guess.
pub fn next_marker() -> Result<String, EngineError> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce)
        .map_err(|_| EngineError::Invalid("no randomness for a render view".to_owned()))?;
    let number = NEXT.fetch_add(1, Ordering::Relaxed);
    Ok(format!("{MARKER_PREFIX}{number}-{}", rb_util::hex(&nonce)))
}

/// The target showing `marker` on `cdp` (in `context` when given), asked for until
/// [`FIND_DEADLINE`]: the browser lists its marker once it has committed it.
///
/// # Errors
///
/// The browser does not answer, or lists no such target in time.
pub async fn find_target(
    cdp: &CdpRouter,
    marker: &str,
    context: Option<&str>,
) -> Result<String, EngineError> {
    let deadline = tokio::time::Instant::now() + FIND_DEADLINE;
    loop {
        let targets = cdp.call("Target.getTargets", json!({}), None).await?;
        if let Some(target) = target_showing(&targets, marker, context) {
            return Ok(target);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(EngineError::Invalid(format!(
                "no CDP target shows {marker}"
            )));
        }
        tokio::time::sleep(FIND_POLL).await;
    }
}

/// Forgets the closed `target` as hidden once `cdp` no longer lists it (at most
/// [`CLOSE_WAIT`]); one still listed then stays hidden.
pub async fn forget_when_closed(cdp: &CdpRouter, target: &str) {
    let deadline = tokio::time::Instant::now() + CLOSE_WAIT;
    loop {
        match cdp.call("Target.getTargets", json!({}), None).await {
            Ok(targets) if !lists(&targets, target) => {
                crate::devtools::hidden::forget(target);
                return;
            }
            Ok(_) => {}
            Err(error) => {
                tracing::debug!(%error, "whether a render view closed is unknown");
                return;
            }
        }
        if tokio::time::Instant::now() >= deadline {
            tracing::warn!("a render view was not closed in time; it stays hidden");
            return;
        }
        tokio::time::sleep(FIND_POLL).await;
    }
}

/// The id of the target in `targets` (a `Target.getTargets` answer) whose URL is `marker`: no
/// popup (an `openerId`), and in `context` when one is given.
#[must_use]
pub fn target_showing(targets: &Value, marker: &str, context: Option<&str>) -> Option<String> {
    let text = |info: &Value, key: &str| info.get(key).and_then(Value::as_str).map(str::to_owned);
    infos(targets)
        .find(|info| {
            text(info, "url").as_deref() == Some(marker)
                && text(info, "openerId").is_none_or(|opener| opener.is_empty())
                && context.is_none_or(|context| {
                    text(info, "browserContextId").as_deref() == Some(context)
                })
        })
        .and_then(|info| info.get("targetId").and_then(Value::as_str))
        .map(str::to_owned)
}

/// Whether `targets` (a `Target.getTargets` answer) still lists `target`.
#[must_use]
pub fn lists(targets: &Value, target: &str) -> bool {
    infos(targets).any(|info| info.get("targetId").and_then(Value::as_str) == Some(target))
}

fn infos(targets: &Value) -> impl Iterator<Item = &Value> {
    targets
        .get("targetInfos")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// The host's window as the render views' side: every call is posted to its UI thread.
struct HostViews(Weak<Host>);

impl RenderViews for HostViews {
    fn open(&self, view: RenderView, made: RenderViewMade) {
        match self.0.upgrade() {
            Some(host) => host.ui().open_render_view(view, made),
            None => made(Err("the window is closed".to_owned())),
        }
    }

    fn resize(&self, marker: String, width: u32, height: u32, painted: RenderViewMade) {
        match self.0.upgrade() {
            Some(host) => host.ui().resize_render_view(marker, width, height, painted),
            None => painted(Err("the window is closed".to_owned())),
        }
    }

    fn close(&self, marker: String) {
        if let Some(host) = self.0.upgrade() {
            host.ui().close_render_view(marker);
        }
    }
}

/// A Chromium view's target of `host`'s grid showing a session: a session's render views live
/// in its context, and a named session no view shows is refused, never swapped for the default
/// one.
fn session_target(host: &Arc<Host>) -> SessionTarget {
    let host = Arc::downgrade(host);
    Box::new(move |session| {
        let host = host.upgrade()?;
        host.views()
            .into_iter()
            .filter(|view| view.engine == EngineKind::Chromium && view.session == session)
            .find_map(|view| host.target_id(view.id))
    })
}

/// The render views of `host`'s window over its CDP connection.
pub fn backend_for(host: &Arc<Host>) -> CdpRenderBackend {
    CdpRenderBackend::new(
        host.cdp().clone(),
        session_target(host),
        Arc::new(HostViews(Arc::downgrade(host))),
    )
}

#[cfg(test)]
#[path = "render_views_tests.rs"]
mod tests;
