//! What the replay needs of a window: [`ReplayPort`], served by the host ([`HostPort`]) and by
//! a timestamping fake in the tests.

use std::sync::{Arc, Weak};

use async_trait::async_trait;
use rb_config::ScrollMode;
use rb_engine::{EngineError, ViewHost, ViewId};
use serde_json::Value;

use crate::host::Host;

/// What a view is, as far as the replay cares.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewTraits {
    /// The view emulates touch (`Emulation.touch`): clicks are taps.
    pub touch: bool,
}

/// The views of a window, for the replay.
#[async_trait]
pub trait ReplayPort: Send + Sync {
    /// `view`'s traits; `None` when it is gone.
    fn traits(&self, view: ViewId) -> Option<ViewTraits>;

    /// `sync.contextMenu`: right clicks are replayed.
    fn context_menu(&self) -> bool;

    /// `sync.scrollMode`: how scroll sync moves the view (RB-020-04).
    fn scroll_mode(&self) -> ScrollMode {
        ScrollMode::Anchor
    }

    /// Evaluates `expression` in the agent's world of `view`'s top frame and answers its value.
    async fn world(&self, view: ViewId, expression: String) -> Result<Value, EngineError>;

    /// Sends `method` to `view`'s page. `Input.*` coordinates are CSS px of the emulated page;
    /// the port scales them into the view.
    async fn send(
        &self,
        view: ViewId,
        method: &'static str,
        params: Value,
    ) -> Result<Value, EngineError>;
}

/// The port of a window's host.
pub struct HostPort {
    host: Weak<Host>,
}

impl HostPort {
    /// The port of `host`.
    pub fn new(host: &Arc<Host>) -> Self {
        Self {
            host: Arc::downgrade(host),
        }
    }

    fn host(&self) -> Result<Arc<Host>, EngineError> {
        self.host.upgrade().ok_or(EngineError::Closed)
    }
}

#[async_trait]
impl ReplayPort for HostPort {
    fn traits(&self, view: ViewId) -> Option<ViewTraits> {
        let host = self.host.upgrade()?;
        let emulation = host.emulation(view)?;
        Some(ViewTraits {
            touch: emulation.touch,
        })
    }

    fn context_menu(&self) -> bool {
        self.host
            .upgrade()
            .is_some_and(|host| host.config().sync.context_menu)
    }

    fn scroll_mode(&self) -> ScrollMode {
        self.host
            .upgrade()
            .map(|host| crate::scroll_sync::mode(&host))
            .unwrap_or_default()
    }

    async fn world(&self, view: ViewId, expression: String) -> Result<Value, EngineError> {
        let host = self.host()?;
        host.sync_agents()
            .evaluate_in_world(view, &expression)
            .await
    }

    async fn send(
        &self,
        view: ViewId,
        method: &'static str,
        params: Value,
    ) -> Result<Value, EngineError> {
        // `cdp_send` multiplies the `Input.*` coordinates by the view's scale (fit::scale_input).
        self.host()?.cdp_send(view, method, params).await
    }
}
