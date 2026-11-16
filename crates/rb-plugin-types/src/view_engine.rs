//! Which views a plugin's page services reach (RB-080-98): `evaluate` and `cdp` serve Chromium
//! views only. One check, so the host's refusal and a check plugin's own answer for a Firefox or
//! WebKit view are the same [`Failure::engine_unserved`], which the check runner shows as a view
//! skipped for its engine.

use crate::{Failure, FailureKind, ViewId, ViewInfo, codes};

/// The engine whose views the page services reach.
const CHROMIUM: &str = "chromium";

impl ViewInfo {
    /// `Ok` for a Chromium view; [`Failure::engine_unserved`] naming `service` for another.
    ///
    /// # Errors
    ///
    /// The view runs another engine.
    pub fn require_chromium(&self, service: &str) -> Result<(), Failure> {
        if self.engine == CHROMIUM {
            Ok(())
        } else {
            Err(Failure::engine_unserved(service, self.id, &self.engine))
        }
    }

    /// The view `id` of `views` when it is a Chromium view.
    ///
    /// # Errors
    ///
    /// `plugins.error.not_found` when no view has `id`; [`Failure::engine_unserved`] when it
    /// runs another engine.
    pub fn chromium_of(views: &[Self], id: ViewId, service: &str) -> Result<Self, Failure> {
        let view = views
            .iter()
            .find(|view| view.id == id)
            .ok_or_else(|| Failure::coded(FailureKind::NotFound, codes::NOT_FOUND))?;
        view.require_chromium(service)?;
        Ok(view.clone())
    }
}

#[cfg(test)]
#[path = "view_engine_tests.rs"]
mod tests;
