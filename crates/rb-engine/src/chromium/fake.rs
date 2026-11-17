//! A scripted [`RenderBackend`] for the tests: answers 404 for a path containing `missing`,
//! fails a URL's first attempts as asked, takes `delay` per navigation and counts how many
//! pages load at once. Like CEF, `open` creates the view at once and is ready after
//! `open_delay`; `close` takes `close_delay` before the view is gone.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;

use super::hidden::{Loaded, RenderBackend, RenderId, RenderSpec, WaitSpec, lock};
use crate::EngineError;

#[derive(Default)]
pub(crate) struct FakeBackend {
    next: AtomicU32,
    pub(crate) delay: Duration,
    pub(crate) open_delay: Duration,
    pub(crate) close_delay: Duration,
    /// URL -> attempts that fail before one succeeds.
    pub(crate) failing: Mutex<BTreeMap<String, u32>>,
    pub(crate) views: Mutex<BTreeMap<RenderId, (RenderSpec, String)>>,
    pub(crate) opened: AtomicUsize,
    pub(crate) closed: Mutex<Vec<RenderId>>,
    loading: AtomicUsize,
    pub(crate) most_at_once: AtomicUsize,
    /// (host, when) of every navigation.
    pub(crate) visits: Mutex<Vec<(String, Instant)>>,
}

impl FakeBackend {
    pub(crate) fn with_delay(delay: Duration) -> Self {
        Self {
            delay,
            ..Self::default()
        }
    }

    /// A backend whose views are ready `open` after they were created and gone `close` after
    /// they were asked to close.
    pub(crate) fn with_open_close(open: Duration, close: Duration) -> Self {
        Self {
            open_delay: open,
            close_delay: close,
            ..Self::default()
        }
    }

    pub(crate) fn fail(&self, url: &str, times: u32) {
        lock(&self.failing).insert(url.to_owned(), times);
    }
}

fn host(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or_default()
        .to_owned()
}

#[async_trait]
impl RenderBackend for FakeBackend {
    async fn open(&self, spec: &RenderSpec) -> Result<RenderId, EngineError> {
        let id = RenderId(self.next.fetch_add(1, Ordering::Relaxed) + 1);
        self.opened.fetch_add(1, Ordering::Relaxed);
        lock(&self.views).insert(id, (spec.clone(), String::new()));
        tokio::time::sleep(self.open_delay).await;
        Ok(id)
    }

    async fn navigate(&self, view: RenderId, url: &str) -> Result<Loaded, EngineError> {
        lock(&self.visits).push((host(url), Instant::now()));
        let now = self.loading.fetch_add(1, Ordering::SeqCst) + 1;
        self.most_at_once.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(self.delay).await;
        self.loading.fetch_sub(1, Ordering::SeqCst);
        {
            let mut failing = lock(&self.failing);
            if let Some(left) = failing.get_mut(url)
                && *left > 0
            {
                *left -= 1;
                return Err(EngineError::Invalid(format!("{url} failed")));
            }
        }
        if let Some(entry) = lock(&self.views).get_mut(&view) {
            entry.1 = url.to_owned();
        }
        Ok(Loaded {
            status: Some(if url.contains("missing") { 404 } else { 200 }),
            final_url: None,
        })
    }

    async fn wait(&self, _view: RenderId, _wait: &WaitSpec) -> Result<(), EngineError> {
        Ok(())
    }

    async fn screenshot(&self, view: RenderId, full_page: bool) -> Result<Vec<u8>, EngineError> {
        let views = lock(&self.views);
        let (spec, url) = views.get(&view).ok_or(EngineError::Closed)?;
        Ok(format!("{url}@{}@{full_page}", spec.breakpoint).into_bytes())
    }

    async fn close(&self, view: RenderId) {
        tokio::time::sleep(self.close_delay).await;
        lock(&self.views).remove(&view);
        lock(&self.closed).push(view);
    }
}
