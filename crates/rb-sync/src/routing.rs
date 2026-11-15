//! [`EngineRouter`]: the hub's [`Replay`] across engines (RB-040-01). Each view's events go to
//! the replay of the view's engine - Chromium's (RB-020-03) for the grid, Firefox's and
//! WebKit's (RB-040-02, RB-040-04) for the external windows. A view whose external window has
//! ended is [`ViewRoute::Stopped`]: its events are skipped as
//! [`ReplayOutcome::Unsupported`], so it is not desynced for being away, and it follows again
//! once it runs. A view the router was not told about is Chromium's, as before engines.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rb_engine::{EngineKind, ViewId};

use crate::replay::{NoReplay, Replay, ReplayOutcome, ReplayRequest, lock};

/// Where one view's events go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewRoute {
    /// To the replay of this engine.
    Engine(EngineKind),
    /// Nowhere: the view's external window has ended.
    Stopped,
}

/// The replays of the engines, chosen per view.
pub struct EngineRouter {
    engines: Mutex<HashMap<EngineKind, Arc<dyn Replay>>>,
    views: Mutex<HashMap<ViewId, ViewRoute>>,
}

impl Default for EngineRouter {
    /// Every engine without a replay ([`NoReplay`]).
    fn default() -> Self {
        Self::new(Arc::new(NoReplay))
    }
}

impl EngineRouter {
    /// A router whose Chromium replay is `chromium`.
    pub fn new(chromium: Arc<dyn Replay>) -> Self {
        Self {
            engines: Mutex::new(HashMap::from([(EngineKind::Chromium, chromium)])),
            views: Mutex::new(HashMap::new()),
        }
    }

    /// `engine`'s replay from now on.
    pub fn set_engine(&self, engine: EngineKind, replay: Arc<dyn Replay>) {
        lock(&self.engines).insert(engine, replay);
    }

    /// Replaces every view's route; views not named are Chromium's.
    pub fn set_routes(&self, routes: impl IntoIterator<Item = (ViewId, ViewRoute)>) {
        *lock(&self.views) = routes.into_iter().collect();
    }

    /// `view`'s route.
    pub fn route(&self, view: ViewId) -> ViewRoute {
        lock(&self.views)
            .get(&view)
            .copied()
            .unwrap_or(ViewRoute::Engine(EngineKind::Chromium))
    }
}

#[async_trait]
impl Replay for EngineRouter {
    async fn dispatch(&self, request: ReplayRequest) -> ReplayOutcome {
        let ViewRoute::Engine(engine) = self.route(request.target) else {
            return ReplayOutcome::Unsupported;
        };
        let replay = lock(&self.engines).get(&engine).cloned();
        match replay {
            Some(replay) => replay.dispatch(request).await,
            None => ReplayOutcome::Unsupported,
        }
    }
}

#[cfg(test)]
#[path = "routing_tests.rs"]
mod tests;
