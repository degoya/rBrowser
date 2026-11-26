//! The [`Shooter`] of the grid's Chromium views: the host's session of each view, its metrics
//! sent again after the capture, and a hidden (out of sight, throttled) view shown to the
//! browser for the capture and hidden again afterwards. Each capture runs on a page prepared by
//! [`super::clean`] (RB-080-12: banners hidden, animations frozen), undone right after; a full
//! page's lazy-loading pre-pass runs in the view's sync agent world (RB-099-28). Firefox and
//! WebKit views capture through their engines once those exist (RB-040-01).

use std::sync::Arc;

use async_trait::async_trait;
use rb_config::EngineKind;
use rb_engine::{EngineError, ViewHost, ViewId};
use rb_visual::CleanPlan;
use serde_json::json;

use super::cdp::{CaptureTarget, FixedSurface, Shot, capture_in};
use super::clean;
use super::pick::{PICK_TIMEOUT, pick};
use super::run::Shooter;
use super::tool::PointArg;
use crate::emulation::cdp::media_params;
use crate::host::Host;

/// One capture at a time: a capture overrides the view's device metrics and sends the host's
/// back afterwards, so two at once would interleave their `setDeviceMetricsOverride`s.
static SHOOTING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Captures the host's views over CDP.
pub struct HostShooter {
    host: Arc<Host>,
    clean: CleanPlan,
}

impl HostShooter {
    /// A shooter for `host`'s views that leaves the pages as they are.
    pub fn new(host: Arc<Host>) -> Self {
        Self {
            host,
            clean: CleanPlan::default(),
        }
    }

    /// The same shooter, each capture on a page prepared as `plan` says.
    #[must_use]
    pub fn with_clean(mut self, plan: CleanPlan) -> Self {
        self.clean = plan;
        self
    }

    /// Lets the user click an element in `view`; its backend node id.
    pub async fn pick(&self, view: ViewId) -> Result<i64, EngineError> {
        let target = self.target(view)?;
        pick(self.host.cdp(), &target.session, PICK_TIMEOUT).await
    }

    /// The backend node id of the element at `point` of `view` (CSS px of the page's viewport,
    /// or DIP of its widget with `dip`; the context menu's click, RB-080-43), never one of
    /// rBrowser's own page hosts (RB-090-51).
    pub async fn node_at(&self, view: ViewId, point: PointArg) -> Result<i64, EngineError> {
        use rb_engine::ViewHost as _;
        let target = self.target(view)?;
        let scale = if point.dip {
            self.host.view(view).map_or(1.0, |info| info.scale)
        } else {
            1.0
        };
        let point = crate::devtools::inspect::Point {
            x: f64::from(point.x),
            y: f64::from(point.y),
        };
        crate::devtools::inspect::node_at(self.host.cdp(), &target.session, point, scale).await
    }

    fn target(&self, view: ViewId) -> Result<CaptureTarget, EngineError> {
        let info = self.host.view(view).ok_or(EngineError::UnknownView(view))?;
        if info.engine != EngineKind::Chromium {
            return Err(EngineError::Unsupported {
                engine: info.engine,
                feature: "screenshot".to_owned(),
            });
        }
        self.host
            .capture_target(view)
            .ok_or_else(|| EngineError::Invalid(format!("view {view} is not attached yet")))
    }
}

#[async_trait]
impl Shooter for HostShooter {
    async fn shoot(&self, view: ViewId, shot: &Shot, scale: f64) -> Result<Vec<u8>, EngineError> {
        let _one_at_a_time = SHOOTING.lock().await;
        let target = self.target(view)?;
        // A throttled view paints no frame: lift it for the capture.
        if target.hidden {
            self.host.ui().set_hidden(view, false);
        }
        let host = self.host.clone();
        let restore = move || host.capture_target(view).map(|now| now.metrics);
        let media = self.host.emulation(view).map_or_else(
            || json!({ "media": "", "features": [] }),
            |emulation| media_params(&emulation),
        );
        let cdp = self.host.cdp();
        // A full page's lazy-loading pre-pass scrolls in the sync agent's world: the agent sends
        // none of it to the other views (RB-099-28).
        let agent = self.host.sync_agents().top_context(view).ok();
        let shot = capture_in(cdp, &target, shot, scale, &restore, &FixedSurface, agent);
        let result = clean::around(cdp, &target.session, &self.clean, &media, shot).await;
        let still_hidden = self.host.capture_target(view).is_some_and(|now| now.hidden);
        if target.hidden && still_hidden {
            self.host.ui().set_hidden(view, true);
        }
        if result.is_err() {
            // Whatever the capture left behind, the host sends the view's emulation again.
            self.host.reapply_emulation(view);
        }
        result
    }
}

#[cfg(test)]
#[path = "chromium_tests.rs"]
mod tests;
