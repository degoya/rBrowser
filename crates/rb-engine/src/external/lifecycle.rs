//! The life of one external view inside [`ExternalWindows`] (a child of `manager`): the start
//! with its reason, the handle once the window is open, the watch of its events, its end -
//! closed by the person it stays closed, crashed it restarts after the backoff while
//! `engines.external.restartOnCrash` is on (RB-080-38).

use std::sync::{Arc, Weak};
use std::time::Instant;

use rb_util::lock;

use super::{ExternalWindows, StartReason, not_available_yet};
use crate::ViewId;
use crate::emulation_report::EmulationReport;
use crate::engine::{EngineHealth, HandleEvent, HandleEvents, ViewHandle, ViewSpec, engine_name};
use crate::external::registry::{ExitAction, ExternalStatus};

impl ExternalWindows {
    pub(super) fn spawn_start(&self, view: ViewId, reason: StartReason) {
        if let Some(me) = self.me.upgrade() {
            self.runtime
                .spawn(async move { me.start(view, reason).await });
        }
    }

    /// Closes `handle` - the old window of `view`, if it has one - and only then starts the
    /// view again: never two windows per view (RB-080-38).
    pub(super) fn spawn_close_then_start(
        &self,
        view: ViewId,
        handle: Option<Arc<dyn ViewHandle>>,
        reason: StartReason,
    ) {
        if let Some(me) = self.me.upgrade() {
            self.runtime.spawn(async move {
                if let Some(handle) = handle {
                    close(handle).await;
                }
                me.start(view, reason).await;
            });
        }
    }

    /// Starts `view` for `reason`: its engine if needed, then its window with its last URL.
    /// One start per view at a time; a start asked for while one is under way is not dropped
    /// but runs after it (the window that one opened closes first), so a restart requested
    /// while the view starts - or while a start that ended "unavailable" winds down - always
    /// happens. Several requests meanwhile are one start.
    pub(super) async fn start(self: Arc<Self>, view: ViewId, reason: StartReason) {
        {
            let mut starting = lock(&self.starting);
            if let Some(queued) = starting.get_mut(&view) {
                *queued = Some(reason);
                return;
            }
            starting.insert(view, None);
        }
        let mut reason = reason;
        loop {
            self.start_once(view, reason).await;
            let queued = {
                let mut starting = lock(&self.starting);
                let queued = starting.get_mut(&view).and_then(Option::take);
                if queued.is_none() {
                    starting.remove(&view);
                }
                queued
            };
            let Some(next) = queued else {
                return;
            };
            // The window the start under way opened is not the one asked for: it goes first,
            // never two windows per view (RB-080-38).
            let opened = lock(&self.handles).remove(&view);
            if let Some(handle) = opened {
                close(handle).await;
            }
            reason = next;
        }
    }

    /// Starts `view` until it ran on the engine the registry names: a view whose engine changed
    /// while it started starts again on the new one.
    async fn start_once(self: &Arc<Self>, view: ViewId, reason: StartReason) {
        let mut reason = reason;
        while self.start_on_current_engine(view, reason).await {
            reason = StartReason::EngineChanged;
        }
    }

    /// One start; `true` when the view's engine changed meanwhile, so it must start again.
    async fn start_on_current_engine(self: &Arc<Self>, view: ViewId, reason: StartReason) -> bool {
        let Some(mut spec) = lock(&self.specs).get(&view).cloned() else {
            return false;
        };
        let Some(known) = self.view(view) else {
            return false;
        };
        let kind = known.engine;
        let engine = lock(&self.engines).get(&kind).cloned();
        let engine = match engine.map(|engine| (engine.health(), engine)) {
            None => {
                lock(&self.registry).unavailable(view, not_available_yet(kind));
                self.changed();
                return false;
            }
            Some((EngineHealth::Unavailable(reason), _)) => {
                lock(&self.registry).unavailable(view, reason);
                self.changed();
                return false;
            }
            Some((_, engine)) => engine,
        };
        tracing::info!(
            %view,
            engine = engine_name(kind),
            breakpoint = %known.breakpoint,
            %reason,
            "external window starts"
        );
        lock(&self.registry).starting(view);
        self.changed();
        spec.url = known.url;
        if spec.window.is_none() {
            let placing = lock(&self.placing).clone();
            let index = self.list().iter().position(|entry| entry.view == view);
            spec.window = known
                .window
                .or_else(|| self.rect_for(&placing, view, index.unwrap_or(0), true));
        }
        let created = engine.create_view(&spec).await;
        let moved = self.view(view).is_some_and(|now| now.engine != kind);
        match created {
            Ok(handle) if moved => close(handle).await,
            Ok(handle) => self.opened(view, &spec, handle).await,
            Err(_) if moved => {}
            Err(error) => {
                tracing::warn!(
                    %view,
                    engine = engine_name(kind),
                    %error,
                    "external view did not start; it stays so until opened again"
                );
                lock(&self.registry).failed(view, error.to_string());
                self.changed();
            }
        }
        moved
    }

    async fn opened(self: &Arc<Self>, view: ViewId, spec: &ViewSpec, handle: Arc<dyn ViewHandle>) {
        if self.view(view).is_none() {
            // The layout dropped the view while it started.
            close(handle).await;
            return;
        }
        let events = handle.events();
        let unsupported =
            EmulationReport::for_engine(handle.engine(), &spec.emulation).unsupported();
        lock(&self.handles).insert(view, handle.clone());
        {
            let mut registry = lock(&self.registry);
            registry.started(view, unsupported, Instant::now());
            if let Some(rect) = spec.window {
                registry.moved(view, rect);
            }
        }
        self.changed();
        let me = self.me.clone();
        self.runtime
            .spawn(async move { watch(me, view, handle, events).await });
    }

    /// `handle`, the current handle of `view`, ended: by the person's hand (`crashed` false, it
    /// stays closed) or with its engine.
    fn ended(&self, view: ViewId, handle: &Arc<dyn ViewHandle>, crashed: bool) {
        {
            let mut handles = lock(&self.handles);
            match handles.get(&view) {
                Some(current) if Arc::ptr_eq(current, handle) => {
                    handles.remove(&view);
                }
                // A restart replaced it, or the layout dropped the view.
                _ => return,
            }
        }
        // The engine was switched off or went missing: the view says so, not "closed".
        if !crashed
            && let Some(known) = self.view(view)
            && let EngineHealth::Unavailable(reason) = self.engine_health(known.engine)
        {
            tracing::info!(%view, %reason, "external window ended with its engine");
            lock(&self.registry).unavailable(view, reason);
            self.changed();
            return;
        }
        let action = lock(&self.registry).exited(view, crashed, Instant::now());
        tracing::info!(%view, crashed, ?action, "external window ended");
        self.changed();
        match action {
            ExitAction::RestartAfter(wait) => {
                let me = self.me.clone();
                self.runtime.spawn(async move {
                    tokio::time::sleep(wait).await;
                    let Some(me) = me.upgrade() else {
                        return;
                    };
                    let waiting = me
                        .view(view)
                        .is_some_and(|known| known.status == ExternalStatus::Restarting);
                    if waiting {
                        me.start(view, StartReason::Crashed).await;
                    }
                });
            }
            ExitAction::Stay | ExitAction::GiveUp => {}
        }
    }
}

/// Follows `handle`'s events until it ends.
async fn watch(
    me: Weak<ExternalWindows>,
    view: ViewId,
    handle: Arc<dyn ViewHandle>,
    mut events: HandleEvents,
) {
    loop {
        let event = events.recv().await;
        let Some(windows) = me.upgrade() else {
            return;
        };
        match event {
            Some(HandleEvent::Navigated { url }) => lock(&windows.registry).navigated(view, &url),
            Some(HandleEvent::Updated) => {}
            Some(HandleEvent::WindowReplaced) => windows.window_replaced(view),
            Some(HandleEvent::Exited { crashed }) => {
                windows.ended(view, &handle, crashed);
                return;
            }
            // The engine let go of the view without a word: not the person's hand, so it
            // counts as a crash (a restart while `restartOnCrash` is on, three at most).
            None => {
                windows.ended(view, &handle, true);
                return;
            }
        }
    }
}

pub(super) async fn close(handle: Arc<dyn ViewHandle>) {
    if let Err(error) = handle.close().await {
        tracing::debug!(view = %handle.id(), %error, "closing failed");
    }
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
