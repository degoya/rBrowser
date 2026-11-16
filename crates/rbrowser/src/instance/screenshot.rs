//! `rbrowser screenshot` in the running instance (RB-050-03): the request is checked again, the
//! views the command line names are resolved against the grid, the folder it gives is taken as
//! written, and the capture runs on the host's runtime - never on the UI thread, never on the
//! channel's - within the request's budget ([`protocol::screenshot_budget`]).

use std::sync::Arc;
use std::time::Duration;

use rb_engine::{ViewHost as _, ViewId};
use serde_json::Value;

use super::protocol;
use crate::capture::{self, ScreenshotAnswer, ScreenshotArgs, ScreenshotRequest};
use crate::cli::screenshot::select_views;
use crate::control::Core;

/// What the instance captures with: the window's core, or a fake in the tests.
pub trait Capturer {
    /// The views in layout order: id and breakpoint.
    fn views(&self) -> Vec<(ViewId, Option<String>)>;
    /// `capture.elementPadding` in effect.
    fn padding(&self) -> u32;
    /// Captures `request`, waiting at most `budget`.
    fn capture(
        &self,
        request: ScreenshotRequest,
        budget: Duration,
    ) -> Result<ScreenshotAnswer, String>;
}

/// Captures what a `screenshot` request asks for and answers the [`ScreenshotAnswer`] as JSON.
/// Files only (no base64 over the channel), no pick mode (nobody clicks for a command line).
pub fn run(
    capturer: &dyn Capturer,
    args: &ScreenshotArgs,
    views: &[String],
    out: Option<&str>,
) -> Result<Value, String> {
    protocol::check_screenshot(args, views, out)?;
    let folder = out.map(protocol::check_out_dir).transpose()?;
    let mut request = args.request(capturer.padding(), Duration::ZERO, false)?;
    request.save = true;
    request.views = select_views(&capturer.views(), views)?;
    request.folder = folder;
    let answer = capturer.capture(request, protocol::screenshot_budget(args))?;
    serde_json::to_value(&answer).map_err(|error| error.to_string())
}

/// The window's core as a [`Capturer`].
pub struct CoreCapturer(pub Arc<Core>);

impl Capturer for CoreCapturer {
    fn views(&self) -> Vec<(ViewId, Option<String>)> {
        self.0
            .host()
            .views()
            .into_iter()
            .map(|info| (info.id, info.breakpoint))
            .collect()
    }

    fn padding(&self) -> u32 {
        capture::settings(&self.0).element_padding
    }

    fn capture(
        &self,
        request: ScreenshotRequest,
        budget: Duration,
    ) -> Result<ScreenshotAnswer, String> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let core = self.0.clone();
        self.0.host().runtime().spawn(async move {
            let result = capture::screenshot(&core, &request)
                .await
                .map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
        receiver
            .recv_timeout(budget)
            .map_err(|_| "the capture did not finish in time".to_owned())?
    }
}

#[cfg(test)]
#[path = "screenshot_tests.rs"]
mod tests;
