//! Sync paused for the views of a flow run (RB-040-07, `automation.flows.syncDuringRun` off): each
//! view of the run leaves the SyncHub (`HostSync::exclude`) while the guard lives, so a flow's
//! clicks and keys are not replayed into the other views - which run the same flow. Dropping the
//! guard - the run's end, its cancel, an aborted task or a panic - takes back exactly the views it
//! excluded; one the person had excluded before stays excluded.

use std::sync::Arc;

use rb_engine::ViewId;

use crate::sync_hub::HostSync;

/// The part of the SyncHub a pause needs.
pub trait SyncSwitch: Send + Sync {
    /// Whether `view` is out of sync now.
    fn is_excluded(&self, view: ViewId) -> bool;

    /// Takes `view` out of sync (`true`) or back in.
    ///
    /// # Errors
    ///
    /// A view the hub does not know.
    fn exclude(&self, view: ViewId, excluded: bool) -> Result<(), String>;
}

impl SyncSwitch for HostSync {
    fn is_excluded(&self, view: ViewId) -> bool {
        self.hub().with(|hub| hub.is_excluded(view))
    }

    fn exclude(&self, view: ViewId, excluded: bool) -> Result<(), String> {
        HostSync::exclude(self, view, excluded).map_err(|error| error.to_string())
    }
}

/// The views a run took out of sync, given back on drop.
pub struct SyncPause {
    sync: Arc<dyn SyncSwitch>,
    paused: Vec<ViewId>,
}

impl SyncPause {
    /// Takes every view of `views` that is in sync out of it.
    pub fn new(sync: Arc<dyn SyncSwitch>, views: &[ViewId]) -> Self {
        let mut paused = Vec::new();
        for &view in views {
            if sync.is_excluded(view) {
                continue;
            }
            match sync.exclude(view, true) {
                Ok(()) => paused.push(view),
                Err(error) => tracing::debug!(%view, %error, "no sync to pause for the flow run"),
            }
        }
        tracing::debug!(?paused, "sync paused for the flow run");
        Self { sync, paused }
    }

    /// The views this pause took out of sync.
    pub fn paused(&self) -> &[ViewId] {
        &self.paused
    }
}

impl Drop for SyncPause {
    fn drop(&mut self) {
        for &view in &self.paused {
            if let Err(error) = self.sync.exclude(view, false) {
                tracing::debug!(%view, %error, "sync not taken back after the flow run");
            }
        }
    }
}

#[cfg(test)]
#[path = "sync_pause_tests.rs"]
mod tests;
