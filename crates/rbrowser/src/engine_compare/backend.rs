//! The shots of an external engine (Firefox, WebKit) as a [`RenderBackend`], so an engine
//! comparison takes them with the same pool and batch as the Chromium shot in its hidden render
//! view. A view is a window of the engine's own at the breakpoint's size and DPR - never a grid
//! cell, never synced: it is opened straight from the engine, outside the external windows the
//! grid shows, under an id of a range of its own ([`FIRST_VIEW_ID`]) far above the grid's.
//!
//! The engines' navigation does not wait for the page's `load`, so a load waits for the commit
//! (at most [`NAVIGATED_WAIT`]) and the wait step is the settle pause (`engineCompare.waitMs`);
//! network idle and wait selectors are Chromium's.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use rb_config::Breakpoint;
use rb_engine::chromium::hidden::{Loaded, RenderBackend, RenderId, RenderSpec, WaitSpec};
use rb_engine::{
    Engine, EngineError, HandleEvent, ScreenshotRequest, ViewHandle, ViewId, ViewSpec,
};

use crate::cdp::lock;

/// The first view id of a comparison's views: the grid counts up from 1 and never gets here.
pub const FIRST_VIEW_ID: u32 = 0xFFF0_0000;
/// The longest a load waits for the engine to report the commit; the settle pause follows.
pub const NAVIGATED_WAIT: Duration = Duration::from_secs(15);

/// One external engine's views for a comparison of one breakpoint.
pub struct EngineBackend {
    engine: Arc<dyn Engine>,
    breakpoint: Breakpoint,
    title: String,
    next: AtomicU32,
    views: Mutex<HashMap<RenderId, Arc<dyn ViewHandle>>>,
}

impl EngineBackend {
    /// Views of `engine` at `breakpoint`, their windows titled `title`.
    pub fn new(engine: Arc<dyn Engine>, breakpoint: Breakpoint, title: String) -> Self {
        Self {
            engine,
            breakpoint,
            title,
            next: AtomicU32::new(FIRST_VIEW_ID),
            views: Mutex::new(HashMap::new()),
        }
    }

    fn handle(&self, view: RenderId) -> Result<Arc<dyn ViewHandle>, EngineError> {
        lock(&self.views)
            .get(&view)
            .cloned()
            .ok_or(EngineError::UnknownView(ViewId(view.0)))
    }
}

#[async_trait]
impl RenderBackend for EngineBackend {
    async fn open(&self, spec: &RenderSpec) -> Result<RenderId, EngineError> {
        let id = self.next.fetch_add(1, Ordering::AcqRel);
        let view = ViewSpec {
            view: Some(ViewId(id)),
            breakpoint: self.breakpoint.clone(),
            emulation: spec.emulation.clone(),
            url: String::new(),
            title: self.title.clone(),
            window: None,
            cell: None,
        };
        let handle = self.engine.create_view(&view).await?;
        lock(&self.views).insert(RenderId(id), handle);
        Ok(RenderId(id))
    }

    async fn navigate(&self, view: RenderId, url: &str) -> Result<Loaded, EngineError> {
        let handle = self.handle(view)?;
        let mut events = handle.events();
        handle.navigate(url).await?;
        let committed = async {
            while let Some(event) = events.recv().await {
                match event {
                    HandleEvent::Navigated { .. } => return Ok(()),
                    HandleEvent::Exited { .. } => return Err(EngineError::Closed),
                    HandleEvent::Updated | HandleEvent::WindowReplaced => {}
                }
            }
            Err(EngineError::Closed)
        };
        // An engine that reports no commit in time still gets its settle pause and its shot.
        if let Ok(outcome) = tokio::time::timeout(NAVIGATED_WAIT, committed).await {
            outcome?;
        }
        Ok(Loaded::default())
    }

    async fn wait(&self, view: RenderId, wait: &WaitSpec) -> Result<(), EngineError> {
        self.handle(view)?;
        tokio::time::sleep(wait.delay).await;
        Ok(())
    }

    async fn screenshot(&self, view: RenderId, full_page: bool) -> Result<Vec<u8>, EngineError> {
        let shot = self
            .handle(view)?
            .screenshot(ScreenshotRequest { full_page })
            .await?;
        base64::engine::general_purpose::STANDARD
            .decode(shot.png_base64.as_bytes())
            .map_err(|error| EngineError::Invalid(format!("the screenshot is no base64: {error}")))
    }

    async fn close(&self, view: RenderId) {
        let handle = lock(&self.views).remove(&view);
        if let Some(handle) = handle
            && let Err(error) = handle.close().await
        {
            tracing::debug!(view = view.0, %error, "a comparison view did not close");
        }
    }
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
