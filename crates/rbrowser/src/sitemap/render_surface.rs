//! The painted area of a render view for a full-page shot (RB-090-88): a windowless browser
//! paints only its own rectangle, so with the page's height in the emulation alone the image
//! repeated the top viewport down the page (and showed stripes at a DPR of 2 or 3). The window
//! grows the browser to the page's height (`cef_app/render_views.rs`: the size its render handler
//! reports, `was_resized`, a repaint) and says when it painted a frame that tall; the shot waits
//! for that at most [`FRAME_WAIT`], asks once more, and then fails with its reason - a frame of
//! the old size is never saved as the full page (code review, wave 090-w23). The restore gives
//! the view its breakpoint's size back without waiting.
//!
//! The grown view is bounded (security review, wave 090-w23): at most [`MAX_DEVICE_PX`] DIP tall
//! at any pixel ratio, and at most [`MAX_SHOT_PIXELS`] - width x height x ratio squared, a ratio
//! below 1 counted as 1 - per shot. A page beyond either fails its shot with the reason, in
//! `run.json` like any failed shot, before the view grows.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rb_engine::EngineError;

use super::render_views::RenderViews;
use crate::capture::cdp::{MAX_DEVICE_PX, Surface};

/// The longest wait for a render view's frame at a new size.
pub const FRAME_WAIT: Duration = Duration::from_secs(2);
/// The most pixels of one full-page shot at its pixel ratio: 64 megapixels (3840 x 16384 at 1).
pub const MAX_SHOT_PIXELS: f64 = 64.0 * 1024.0 * 1024.0;

/// Whether a view of `width` x `height` DIP at pixel ratio `dpr` (below 1 or none: 1) stays
/// within [`MAX_SHOT_PIXELS`].
fn within_budget(width: u32, height: u32, dpr: f64) -> bool {
    let ratio = dpr.max(1.0);
    f64::from(width) * f64::from(height) * ratio * ratio <= MAX_SHOT_PIXELS
}

/// The browser at `marker` of `window`, `width` x `height` DIP at its breakpoint's size.
pub struct RenderSurface<'a> {
    pub window: &'a Arc<dyn RenderViews>,
    pub marker: &'a str,
    pub width: u32,
    pub height: u32,
    /// The breakpoint's pixel ratio (`0`: none).
    pub dpr: f64,
}

impl RenderSurface<'_> {
    /// Asks the view for `height` DIP and waits for its frame: `None` when none came in time.
    async fn grow(&self, height: u32) -> Option<Result<(), EngineError>> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.window.resize(
            self.marker.to_owned(),
            self.width,
            height,
            Box::new(move |painted| {
                // The shot gave up waiting already: nothing listens.
                let _ = sender.send(painted);
            }),
        );
        match tokio::time::timeout(FRAME_WAIT, receiver).await {
            Ok(Ok(Ok(()))) => Some(Ok(())),
            Ok(Ok(Err(error))) => Some(Err(EngineError::Invalid(error))),
            Ok(Err(_)) | Err(_) => None,
        }
    }

    /// Gives the view its breakpoint's size back; nothing waits for the frame.
    pub fn shrink(&self) {
        self.window.resize(
            self.marker.to_owned(),
            self.width,
            self.height,
            Box::new(|_| {}),
        );
    }
}

/// A height in CSS px as whole DIP, at least the view's own (`full_page_height` is a whole
/// number below `MAX_DEVICE_PX` already).
fn whole(height: f64, own: u32) -> u32 {
    if height.is_finite() && height > 0.0 {
        (height.ceil().min(f64::from(u32::MAX)) as u32).max(own)
    } else {
        own
    }
}

#[async_trait]
impl Surface for RenderSurface<'_> {
    async fn fit(&self, height: f64) -> Result<(), EngineError> {
        if !height.is_finite() || height > MAX_DEVICE_PX {
            tracing::warn!(height, "a page is too tall for a full-page shot");
            return Err(EngineError::Invalid(format!(
                "the page is too tall for a full-page shot: at most {MAX_DEVICE_PX} px"
            )));
        }
        let height = whole(height, self.height);
        if !within_budget(self.width, height, self.dpr) {
            tracing::warn!(
                width = self.width,
                height,
                dpr = self.dpr,
                "a full-page shot is over the pixel budget"
            );
            return Err(EngineError::Invalid(format!(
                "the full page ({} x {height} px at a pixel ratio of {}) is more than the {} \
                 megapixels a shot may have",
                self.width,
                self.dpr.max(1.0),
                MAX_SHOT_PIXELS / (1024.0 * 1024.0)
            )));
        }
        for _ in 0..2 {
            if let Some(grown) = self.grow(height).await {
                return grown;
            }
        }
        tracing::warn!(
            height,
            "a render view painted no frame at the page's height; the full-page shot fails"
        );
        Err(EngineError::Invalid(format!(
            "the render view painted no frame at the page's height ({height} px) within {} s, \
             asked twice",
            FRAME_WAIT.as_secs()
        )))
    }

    async fn restore(&self) {
        self.shrink();
    }
}

#[cfg(test)]
#[path = "render_surface_tests.rs"]
mod tests;
