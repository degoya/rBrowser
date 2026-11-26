//! The lazy-loading pre-pass of a full-page capture (RB-099-28): images below the viewport that
//! load lazily (`loading="lazy"`, an `IntersectionObserver` or scroll-driven loader) are not
//! there until the page has been scrolled to them, so a full page taken at once missed them.
//! Before the shots, [`prepass`] walks the page to its end one viewport at a time ([`next_stop`]),
//! so the page sees real scroll positions, goes back to where it was, and waits for the images
//! still loading - all within [`DEADLINE`]. The capture measures the page again afterwards
//! (content loaded on the way lengthens it) and plans its sections as before (RB-099-11). A
//! deadline that runs out costs no screenshot: the capture is taken as it is and its PNG says so
//! ([`Prepass::mark`], `imagesPending` in the answer).
//!
//! The walk is no input for the sync: every scroll of the pre-pass sets the sync agent's echo
//! marker first ([`super::lazy_script::step`]), the way back one that holds over the shot
//! ([`SHOT_MARKER_MS`]), so no other view follows. After the capture [`put_back`] scrolls the
//! view to where it was once more (a viewport grown to the page's height leaves it at the top)
//! with a short marker. The scripts run in the sync agent's world of the view when it has one,
//! else - and when that world gives no position (its context went with a navigation) - in
//! rBrowser's capture world ([`run`]); a pre-pass that cannot run at all marks the capture as
//! well: its lazy images may be missing.

use std::time::Duration;

use async_trait::async_trait;
use rb_engine::EngineError;
use serde_json::{Value, json};
use tokio::time::{Instant, timeout, timeout_at};

use super::full_page::MAX_IMAGE_ROWS;
use super::lazy_script;
use crate::cdp::CdpRouter;

/// The longest the pre-pass of one capture may take: the walk and the wait for the images.
pub const DEADLINE: Duration = Duration::from_secs(4);
/// The most scroll stops of one walk.
pub const MAX_STOPS: usize = 256;
/// The walk goes no further down than an image of a full page may reach (at a ratio of 1).
pub const WALK_LIMIT: f64 = MAX_IMAGE_ROWS;
/// The echo marker of one scroll of the walk, in ms.
pub const STEP_MARKER_MS: u32 = 1000;
/// The echo marker of the way back before the shot, in ms: it holds while the viewport grows to
/// the page's height and back (the sync agent's longest).
pub const SHOT_MARKER_MS: u32 = 10_000;
/// The longest the way back may take, also past the deadline.
const RETURN_TIMEOUT: Duration = Duration::from_secs(2);

/// A scroll position in CSS px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scroll {
    /// Across.
    pub x: f64,
    /// Down.
    pub y: f64,
}

/// What the page answers to a step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seen {
    /// Where it is.
    pub at: Scroll,
    /// Its scroll height in CSS px.
    pub height: f64,
    /// Its viewport's height in CSS px.
    pub viewport: f64,
}

/// What the pre-pass needs of a page.
#[async_trait]
pub trait LazyPage: Send + Sync {
    /// Sets the echo marker for `marker_ms`, scrolls to `to` (none: stays) and waits for the
    /// frames that follow; where the page is then.
    async fn step(&self, to: Option<Scroll>, marker_ms: u32) -> Result<Seen, EngineError>;
    /// Waits at most `wait` for the images still loading; how many still are.
    async fn images(&self, wait: Duration) -> Result<u32, EngineError>;
}

/// What a pre-pass did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Prepass {
    /// Where the page was before; `None`: the page gave no position, nothing was scrolled.
    pub start: Option<Scroll>,
    /// Images may be missing: the deadline ran out (the walk was cut or images were still
    /// loading), or the pre-pass could not run ([`run`]).
    pub timed_out: bool,
}

impl Prepass {
    /// `png` marked as captured with images still loading when the deadline ran out.
    pub fn mark(&self, png: Vec<u8>) -> Vec<u8> {
        if !self.timed_out {
            return png;
        }
        tracing::warn!(
            "images were still loading at the deadline; the full page is taken as it is"
        );
        let deadline = DEADLINE.as_millis().to_string();
        rb_visual::note::with_note(&png, rb_visual::note::IMAGES_PENDING_KEY, &deadline)
            .unwrap_or(png)
    }
}

/// The walk's next scroll top after `previous` (none yet: the top) in a `viewport` of a page
/// `height` CSS px tall: one viewport further, at most the last top (`height - viewport`, the
/// height no more than [`WALK_LIMIT`]); `None` at the end, for a page within its viewport and
/// without a usable viewport.
pub fn next_stop(previous: Option<f64>, viewport: f64, height: f64) -> Option<f64> {
    if !(viewport.is_finite() && viewport >= 1.0 && height.is_finite()) {
        return None;
    }
    let end = (height.min(WALK_LIMIT) - viewport).max(0.0);
    match previous {
        _ if end < 1.0 => None,
        None => Some(0.0),
        Some(at) if at + 0.5 >= end => None,
        Some(at) => Some((at + viewport).min(end)),
    }
}

/// Walks `page` to its end, back to where it was and waits for its images, within `deadline`.
pub async fn prepass(page: &dyn LazyPage, deadline: Duration) -> Prepass {
    let until = Instant::now() + deadline;
    let first = match timeout_at(until, page.step(None, STEP_MARKER_MS)).await {
        Ok(Ok(seen)) => seen,
        Ok(Err(error)) => {
            tracing::debug!(%error, "no lazy-loading pre-pass: the page gave no position");
            return Prepass::default();
        }
        Err(_) => {
            return Prepass {
                start: None,
                timed_out: true,
            };
        }
    };
    let start = first.at;
    let mut timed_out = false;
    let (mut at, mut height, mut stops) = (None, first.height, 0);
    while let Some(stop) = next_stop(at, first.viewport, height) {
        // Cut at its stops the walk is not late: the images loading so far are waited for.
        if stops == MAX_STOPS {
            break;
        }
        if Instant::now() >= until {
            timed_out = true;
            break;
        }
        stops += 1;
        let to = Scroll {
            x: start.x,
            y: stop,
        };
        match timeout_at(until, page.step(Some(to), STEP_MARKER_MS)).await {
            // The page went no further (it does not scroll, or not that far): its end.
            Ok(Ok(seen)) if at.is_some_and(|before| seen.at.y <= before) => break,
            Ok(Ok(seen)) => (at, height) = (Some(seen.at.y), seen.height),
            Ok(Err(error)) => {
                tracing::debug!(%error, "the lazy-loading walk stopped");
                break;
            }
            Err(_) => {
                timed_out = true;
                break;
            }
        }
    }
    // Back where it was, whatever the walk gave, with the marker held over the shot.
    back(page, start, SHOT_MARKER_MS).await;
    if !timed_out {
        let wait = until.saturating_duration_since(Instant::now());
        timed_out = match timeout(wait + RETURN_TIMEOUT, page.images(wait)).await {
            Ok(Ok(pending)) => pending > 0,
            Ok(Err(error)) => {
                tracing::debug!(%error, "the images of the page were not waited for");
                false
            }
            Err(_) => true,
        };
    }
    Prepass {
        start: Some(start),
        timed_out,
    }
}

/// The pre-pass of the page of `session` within [`DEADLINE`]: in `agent`, the sync agent's world
/// of the view (its session and context), else - or when that world gives no position, a stale
/// context - in rBrowser's capture world. A pre-pass that ran in neither counts as images
/// pending. The page it ran on, for [`put_back`].
pub async fn run<'a>(
    cdp: &'a CdpRouter,
    session: &str,
    agent: Option<(String, i64)>,
) -> (Option<CdpPage<'a>>, Prepass) {
    let started = Instant::now();
    if let Some((agent_session, context)) = agent {
        let page = CdpPage {
            cdp,
            session: agent_session,
            context,
        };
        let done = prepass(&page, DEADLINE).await;
        if done.start.is_some() || done.timed_out {
            return (Some(page), done);
        }
        tracing::debug!(
            "the sync agent's world gave no position; the capture world walks the page"
        );
    }
    let not_run = Prepass {
        start: None,
        timed_out: true,
    };
    let Some(page) = CdpPage::capture_world(cdp, session).await else {
        return (None, not_run);
    };
    let done = prepass(&page, DEADLINE.saturating_sub(started.elapsed())).await;
    if done.start.is_none() {
        return (Some(page), not_run);
    }
    (Some(page), done)
}

/// Scrolls `page` back to `start` after the capture, with a short echo marker.
pub async fn put_back(page: &dyn LazyPage, start: Scroll) {
    back(page, start, STEP_MARKER_MS).await;
}

async fn back(page: &dyn LazyPage, start: Scroll, marker_ms: u32) {
    match timeout(RETURN_TIMEOUT, page.step(Some(start), marker_ms)).await {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => tracing::debug!(%error, "the page was not scrolled back"),
        Err(_) => tracing::debug!("the page was not scrolled back in time"),
    }
}

/// A view's page over its flat CDP session, in an isolated world.
pub struct CdpPage<'a> {
    cdp: &'a CdpRouter,
    session: String,
    context: i64,
}

impl<'a> CdpPage<'a> {
    /// The page of `session` in rBrowser's capture world; `None` without it.
    async fn capture_world(cdp: &'a CdpRouter, session: &str) -> Option<Self> {
        match super::clean::world(cdp, session).await {
            Ok(context) => Some(Self {
                cdp,
                session: session.to_owned(),
                context,
            }),
            Err(error) => {
                tracing::debug!(%error, "no isolated world for the lazy-loading pre-pass");
                None
            }
        }
    }

    async fn run(&self, expression: String) -> Result<Value, EngineError> {
        let params = json!({
            "expression": expression,
            "contextId": self.context,
            "returnByValue": true,
            "awaitPromise": true,
        });
        let answer = self
            .cdp
            .call("Runtime.evaluate", params, Some(&self.session))
            .await?;
        if answer.get("exceptionDetails").is_some() {
            return Err(EngineError::Invalid(
                "the lazy-loading pre-pass script threw".to_owned(),
            ));
        }
        Ok(answer
            .pointer("/result/value")
            .cloned()
            .unwrap_or(Value::Null))
    }
}

#[async_trait]
impl LazyPage for CdpPage<'_> {
    async fn step(&self, to: Option<Scroll>, marker_ms: u32) -> Result<Seen, EngineError> {
        let value = self.run(lazy_script::step(to, marker_ms)?).await?;
        lazy_script::seen(&value)
            .ok_or_else(|| EngineError::Invalid("the page gave no scroll position".to_owned()))
    }

    async fn images(&self, wait: Duration) -> Result<u32, EngineError> {
        let value = self.run(lazy_script::images(wait)).await?;
        lazy_script::pending(&value)
            .ok_or_else(|| EngineError::Invalid("the page gave no image count".to_owned()))
    }
}

#[cfg(test)]
#[path = "lazy_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "lazy_capture_tests.rs"]
mod capture_tests;
