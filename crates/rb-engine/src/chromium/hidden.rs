//! Hidden render views: what one is opened with ([`RenderSpec`]), what a page is waited for
//! ([`WaitSpec`]), the operations of the browser side ([`RenderBackend`]: open, navigate, wait,
//! screenshot, close) and the pool that keeps at most `limit` of them open and hands an idle one
//! with the same spec out again ([`RenderPool`]). A lease that is dropped without
//! [`RenderPool::release`] leaves its view an orphan, which the next [`RenderPool::acquire`] (or
//! [`RenderPool::shutdown`]) closes.
//!
//! A batch is cancelled by dropping its futures, so the pool's own opens and closes run in
//! tasks of their own: a close always finishes, and a view that opens after its caller gave up
//! (cancelled, or timed out) is closed, never left behind.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot};

use crate::{Emulation, EngineError};

/// The most render views a pool may keep open.
pub const MAX_RENDER_VIEWS: usize = 8;

/// A render view, unique for the browser side's lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RenderId(pub u32);

/// What a render view is opened with.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderSpec {
    /// The breakpoint's name (for the log).
    pub breakpoint: String,
    /// Its emulation: size, DPR, mobile, touch, user agent, ...
    pub emulation: Emulation,
    /// The named session (cookie and storage context, RB-020-07); `default` for the grid's.
    pub session: String,
}

/// What a page is waited for after its `load`.
#[derive(Clone, Debug, PartialEq)]
pub struct WaitSpec {
    /// No request for a moment (Chromium's `networkIdle` lifecycle event).
    pub network_idle: bool,
    /// An element that must be there.
    pub selector: Option<String>,
    /// A further pause.
    pub delay: Duration,
    /// The longest a page may take from navigation to its screenshot.
    pub timeout: Duration,
    /// Scroll to the end and back, so lazy content loads.
    pub auto_scroll: bool,
}

impl Default for WaitSpec {
    fn default() -> Self {
        Self {
            network_idle: true,
            selector: None,
            delay: Duration::ZERO,
            timeout: Duration::from_secs(30),
            auto_scroll: true,
        }
    }
}

/// What a navigation gave.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Loaded {
    /// The main document's HTTP status (none for `file:` or a cached page).
    pub status: Option<u16>,
    /// Where the page ended, when that is not the URL asked for.
    pub final_url: Option<String>,
}

/// The browser side of render views. Every method returns without blocking the UI thread.
#[async_trait]
pub trait RenderBackend: Send + Sync {
    /// Opens a hidden view with `spec`'s emulation and session.
    async fn open(&self, spec: &RenderSpec) -> Result<RenderId, EngineError>;
    /// Loads `url` and waits for its `load`.
    async fn navigate(&self, view: RenderId, url: &str) -> Result<Loaded, EngineError>;
    /// Waits as `wait` says and scrolls.
    async fn wait(&self, view: RenderId, wait: &WaitSpec) -> Result<(), EngineError>;
    /// A PNG of the viewport, or of the whole page (on a page the backend prepares for the
    /// capture, RB-080-12).
    async fn screenshot(&self, view: RenderId, full_page: bool) -> Result<Vec<u8>, EngineError>;
    /// Closes the view; one that is gone already is no error.
    async fn close(&self, view: RenderId);
}

pub(crate) use rb_util::lock;

#[derive(Default)]
struct Views {
    /// Open and not leased, with the spec they were opened with.
    idle: Vec<(RenderSpec, RenderId)>,
    /// Leases dropped without a release.
    orphans: Vec<RenderId>,
    /// Every open view, leased or idle.
    open: usize,
}

/// At most `limit` render views open; idle ones are handed out again for the same spec.
pub struct RenderPool {
    backend: Arc<dyn RenderBackend>,
    permits: Arc<Semaphore>,
    limit: usize,
    views: Arc<Mutex<Views>>,
}

/// One render view, leased from the pool.
pub struct RenderLease {
    id: Option<RenderId>,
    spec: RenderSpec,
    views: Arc<Mutex<Views>>,
    _permit: OwnedSemaphorePermit,
}

impl RenderLease {
    /// The view.
    ///
    /// # Panics
    ///
    /// Never: the id is taken only by [`RenderPool::release`], which consumes the lease.
    #[must_use]
    pub fn id(&self) -> RenderId {
        self.id.expect("a lease holds its view until released")
    }

    /// The spec it was opened with.
    #[must_use]
    pub fn spec(&self) -> &RenderSpec {
        &self.spec
    }
}

impl Drop for RenderLease {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            lock(&self.views).orphans.push(id);
        }
    }
}

/// A view the open task hands its caller; when it is dropped untaken (the caller gave up while
/// it was on its way), it becomes an orphan.
struct Opened {
    id: Option<RenderId>,
    views: Arc<Mutex<Views>>,
}

impl Drop for Opened {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            lock(&self.views).orphans.push(id);
        }
    }
}

impl RenderPool {
    /// A pool over `backend` of at most `limit` views (1 to [`MAX_RENDER_VIEWS`]).
    #[must_use]
    pub fn new(backend: Arc<dyn RenderBackend>, limit: usize) -> Arc<Self> {
        let limit = limit.clamp(1, MAX_RENDER_VIEWS);
        Arc::new(Self {
            backend,
            permits: Arc::new(Semaphore::new(limit)),
            limit,
            views: Arc::new(Mutex::new(Views::default())),
        })
    }

    /// The backend, for the steps on a leased view.
    #[must_use]
    pub fn backend(&self) -> &Arc<dyn RenderBackend> {
        &self.backend
    }

    /// How many views are open now.
    #[must_use]
    pub fn open_views(&self) -> usize {
        lock(&self.views).open
    }

    /// A view with `spec`: an idle one opened with it, else a new one (closing an idle one of
    /// another spec when the pool is full). Waits while `limit` views are leased.
    ///
    /// # Errors
    ///
    /// The pool was shut down, or the backend could not open a view.
    pub async fn acquire(&self, spec: &RenderSpec) -> Result<RenderLease, EngineError> {
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| EngineError::Closed)?;
        let orphans = {
            let mut views = lock(&self.views);
            let orphans = std::mem::take(&mut views.orphans);
            views.open = views.open.saturating_sub(orphans.len());
            orphans
        };
        for orphan in orphans {
            self.close_detached(orphan).await;
        }
        let (reuse, evict) = {
            let mut views = lock(&self.views);
            match views.idle.iter().position(|(idle, _)| idle == spec) {
                Some(at) => (Some(views.idle.remove(at).1), None),
                None if views.open >= self.limit && !views.idle.is_empty() => {
                    views.open -= 1;
                    (None, Some(views.idle.remove(0).1))
                }
                None => (None, None),
            }
        };
        if let Some(evicted) = evict {
            self.close_detached(evicted).await;
        }
        let id = match reuse {
            Some(id) => id,
            None => self.open_detached(spec).await?,
        };
        Ok(RenderLease {
            id: Some(id),
            spec: spec.clone(),
            views: self.views.clone(),
            _permit: permit,
        })
    }

    /// Gives `lease` back: kept for the next page when `healthy`, else closed.
    pub async fn release(&self, mut lease: RenderLease, healthy: bool) {
        let Some(id) = lease.id.take() else {
            return;
        };
        if healthy {
            lock(&self.views).idle.push((lease.spec.clone(), id));
        } else {
            {
                let mut views = lock(&self.views);
                views.open = views.open.saturating_sub(1);
            }
            self.close_detached(id).await;
        }
    }

    /// Opens a view with `spec` in a task of its own and counts it. When this caller is gone
    /// before the view is there, the task closes it again.
    async fn open_detached(&self, spec: &RenderSpec) -> Result<RenderId, EngineError> {
        let (sender, receiver) = oneshot::channel::<Result<Opened, EngineError>>();
        let (backend, views, spec) = (self.backend.clone(), self.views.clone(), spec.clone());
        tokio::spawn(async move {
            let opened = backend.open(&spec).await.map(|id| {
                lock(&views).open += 1;
                Opened {
                    id: Some(id),
                    views: views.clone(),
                }
            });
            if let Err(Ok(mut late)) = sender.send(opened)
                && let Some(id) = late.id.take()
            {
                {
                    let mut views = lock(&views);
                    views.open = views.open.saturating_sub(1);
                }
                backend.close(id).await;
            }
        });
        let mut opened = receiver.await.map_err(|_| EngineError::Closed)??;
        opened.id.take().ok_or(EngineError::Closed)
    }

    /// Closes `id` in a task of its own, so the close finishes even when this caller is
    /// dropped; waits for it otherwise.
    async fn close_detached(&self, id: RenderId) {
        let backend = self.backend.clone();
        // A close that panicked is no view this pool can do more about.
        let _ = tokio::spawn(async move { backend.close(id).await }).await;
    }

    /// Closes every idle and orphaned view and takes no new lease.
    pub async fn shutdown(&self) {
        self.permits.close();
        let closing: Vec<RenderId> = {
            let mut views = lock(&self.views);
            let mut ids: Vec<RenderId> = views.idle.drain(..).map(|(_, id)| id).collect();
            ids.append(&mut views.orphans);
            views.open = views.open.saturating_sub(ids.len());
            ids
        };
        for id in closing {
            self.close_detached(id).await;
        }
    }
}

#[cfg(test)]
#[path = "hidden_tests.rs"]
mod tests;
