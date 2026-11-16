//! The dock's preparation at its marker (its target captured and hidden, the screencast's
//! setting written for the frontend) and its size: the settings', the splitter's while dragged.

use std::sync::Mutex;

use rb_engine::EngineError;
use serde_json::{Value, json};

use super::{DOCK_MARKER_URL, Devtools, Dock, side_of};
use crate::cdp::lock;
use crate::devtools::layout::DockSizes;
use crate::devtools::{hidden, screencast};
use crate::host::ui::{DevtoolsDock, DockSide};

impl Devtools {
    /// The dock finished loading `url`: at the marker, it is prepared and gets the frontend.
    /// The preparation carries the dock's generation: closed and made again before it finishes
    /// (a toggled `devtools.screencast`), its result is dropped and the new dock prepares for
    /// itself.
    pub fn on_dock_loaded(&self, url: &str) {
        if url != DOCK_MARKER_URL {
            return;
        }
        let generation = {
            let inner = lock(&self.inner);
            if inner.dock != Dock::Marker {
                return;
            }
            inner.dock_generation
        };
        let Some(me) = self.me.upgrade() else {
            return;
        };
        self.runtime.spawn(async move {
            let session = match me.prepare_dock().await {
                Ok(session) => Some(session),
                Err(error) => {
                    tracing::warn!(%error, "the DevTools dock was not prepared; its screencast may show");
                    None
                }
            };
            me.dock_prepared(generation, session);
        });
    }

    /// Captures the dock's target by its marker, hides it, and writes the screencast's setting
    /// (`devtools.screencast`) for the frontend: `Page.enable` first, or Blink's page agent
    /// never injects the script (RB-080-21).
    async fn prepare_dock(&self) -> Result<String, EngineError> {
        let cdp = self.host()?.cdp().clone();
        let screencast = lock(&self.inner).screencast;
        let targets = cdp.call("Target.getTargets", json!({}), None).await?;
        let target = targets
            .get("targetInfos")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|info| info.get("url").and_then(Value::as_str) == Some(DOCK_MARKER_URL))
            .and_then(|info| info.get("targetId").and_then(Value::as_str))
            .map(str::to_owned)
            .ok_or_else(|| EngineError::Invalid("no CDP target shows the dock".to_owned()))?;
        hidden::register(&target);
        let attached = cdp
            .call(
                "Target.attachToTarget",
                json!({ "targetId": target, "flatten": true }),
                None,
            )
            .await?;
        let session = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| EngineError::Invalid("attachToTarget gave no session".to_owned()))?
            .to_owned();
        cdp.call("Page.enable", json!({}), Some(&session)).await?;
        cdp.call(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "source": screencast::preference_script(screencast) }),
            Some(&session),
        )
        .await?;
        Ok(session)
    }

    /// The dock of `generation` is prepared: it loads the frontend of what is open now.
    fn dock_prepared(&self, generation: u64, session: Option<String>) {
        let Ok(host) = self.host() else {
            return;
        };
        let sizes = self.dock_sizes();
        let mut inner = lock(&self.inner);
        if inner.dock != Dock::Marker || inner.dock_generation != generation {
            // Closed meanwhile, or closed and made again: the stale session goes with its dock.
            drop(inner);
            self.detach(&host, session);
            return;
        }
        inner.dock = Dock::Ready(session);
        let dock = inner.open.as_ref().and_then(|open| {
            let side = side_of(open.mode)?;
            Some(DevtoolsDock {
                view: open.view,
                side,
                size: sizes.size_for(side),
                frontend_url: open.ticket.as_ref()?.frontend_url().to_owned(),
            })
        });
        if let Some(dock) = dock {
            host.ui().dock_devtools(Some(dock));
        }
    }

    /// Whether the settings want the frontend's screencast (`devtools.screencast`).
    pub(super) fn screencast_wanted(&self) -> bool {
        self.core
            .upgrade()
            .is_some_and(|core| core.user_settings().devtools.screencast)
    }

    /// The dock sizes the settings give.
    pub(super) fn dock_sizes(&self) -> DockSizes {
        self.core
            .upgrade()
            .map(|core| crate::devtools::splitter::sizes_of(&core.user_settings().devtools))
            .unwrap_or_default()
    }

    /// The dock's size when the splitter's drag began.
    pub(crate) fn drag_start(&self) -> &Mutex<Option<u32>> {
        &self.drag_start
    }

    /// Lays the open dock out again at `size` (the splitter while dragged) or at the settings'
    /// size; the side it is on, `None` while nothing is docked.
    pub fn redock(&self, size: Option<u32>) -> Option<DockSide> {
        let host = self.host().ok()?;
        let sizes = self.dock_sizes();
        let inner = lock(&self.inner);
        let open = inner.open.as_ref()?;
        let side = side_of(open.mode)?;
        let frontend_url = match &inner.dock {
            Dock::Closed => return None,
            Dock::Marker => DOCK_MARKER_URL.to_owned(),
            Dock::Ready(_) => open.ticket.as_ref()?.frontend_url().to_owned(),
        };
        let dock = DevtoolsDock {
            view: open.view,
            side,
            size: size.unwrap_or_else(|| sizes.size_for(side)),
            frontend_url,
        };
        drop(inner);
        host.ui().dock_devtools(Some(dock));
        Some(side)
    }
}
