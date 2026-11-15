//! [`UnsupportedEngine`]: the stand-in for an engine whose implementation has not landed
//! (Firefox until RB-040-02, WebKit until RB-040-04) or cannot run on this machine. It is always
//! [`EngineHealth::Unavailable`] and opens no view, so a breakpoint with that engine shows "not
//! available yet" instead of failing.

use std::sync::Arc;

use async_trait::async_trait;
use rb_config::EngineKind;

use crate::EngineError;
use crate::engine::{Engine, EngineHealth, ViewHandle, ViewSpec};
use crate::external::not_available_yet;

/// An engine that cannot run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsupportedEngine {
    kind: EngineKind,
    reason: String,
}

impl UnsupportedEngine {
    /// `kind` is not available yet.
    pub fn not_yet(kind: EngineKind) -> Self {
        Self::because(kind, not_available_yet(kind))
    }

    /// `kind` is not available, for `reason` (shown to the user).
    pub fn because(kind: EngineKind, reason: String) -> Self {
        Self { kind, reason }
    }

    /// Why it is not available.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[async_trait]
impl Engine for UnsupportedEngine {
    fn kind(&self) -> EngineKind {
        self.kind
    }

    fn external(&self) -> bool {
        self.kind != EngineKind::Chromium
    }

    fn health(&self) -> EngineHealth {
        EngineHealth::Unavailable(self.reason.clone())
    }

    async fn create_view(&self, _spec: &ViewSpec) -> Result<Arc<dyn ViewHandle>, EngineError> {
        Err(EngineError::Unavailable {
            engine: self.kind,
            reason: self.reason.clone(),
        })
    }

    async fn shutdown(&self) {}
}

#[cfg(test)]
#[path = "unsupported_tests.rs"]
mod tests;
