//! The Firefox engine (RB-040-02): `rb_engine::Engine` over the installed Firefox and BiDi.
//!
//! - One Firefox per project ([`FirefoxEngine`]), started with its first view and quit with
//!   the app; one window (`browsingContext`) per Firefox breakpoint ([`view`]).
//! - Emulation: viewport and DPR (`setViewport`, never awaited for long), user agent, locale,
//!   color scheme, screen, offline, headers; the rest is marked by the report
//!   ([`crate::emulation`]).
//! - The sync agent runs as a preload script in the sandbox `rbrowser` (Chromium's isolated
//!   world) and reports over the `script.message` channel `rb-agent-<view>` ([`agent`]); the
//!   engine hands the reports out ([`FirefoxEngine::reports`]) for the SyncHub.
//! - Replay: [`FirefoxReplay`], the hub's `rb_sync::Replay` for Firefox views, resolves the
//!   target through the agent and replays with `input.performActions` ([`replay`]).

pub mod agent;
mod cookies;
mod engine;
pub mod replay;
mod session;
pub mod view;

#[cfg(test)]
pub(crate) mod test_util;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use rb_engine::{EngineError, EngineKind, ViewId};
use rb_sync::{AgentConfig, AgentMessage};
use serde_json::Value;

use crate::error::BidiError;
use crate::launcher::{self, FirefoxProcess, START_TIMEOUT};
use crate::profile;

pub use cookies::CookieOutcome;
pub use engine::{EmulationFilter, FirefoxEngine};
pub use replay::FirefoxReplay;
pub use session::QUIT_BUDGET;

/// A Firefox to talk to: its session endpoint and, when rBrowser started it, its process.
pub struct Launched {
    /// `ws://127.0.0.1:<port>/session`.
    pub endpoint: String,
    /// The process; quit with the engine.
    pub process: Option<FirefoxProcess>,
}

/// Starts a Firefox (the installed one, or a fake in tests).
#[async_trait]
pub trait Launch: Send + Sync {
    /// Starts it and answers its endpoint.
    async fn launch(&self) -> Result<Launched, BidiError>;
}

/// The installed Firefox with rBrowser's profile.
#[derive(Clone, Debug)]
pub struct InstalledFirefox {
    /// `firefox.exe`.
    pub executable: PathBuf,
    /// The profile folder (rBrowser's, per project, in its local folder).
    pub profile: PathBuf,
    /// `engines.firefox.extraPrefs`.
    pub extra_prefs: BTreeMap<String, Value>,
    /// No tabs, address bar or bookmarks toolbar: the window is the page, for a view in its
    /// grid cell (`engines.external.embed`, RB-080-39).
    pub hide_chrome: bool,
    /// How every window renders CSS px to device px, so the views fit their cells
    /// (`layout.css.devPixelsPerPx`, RB-080-40); `None` the display's own ratio.
    pub render_scale: Option<f64>,
}

#[async_trait]
impl Launch for InstalledFirefox {
    async fn launch(&self) -> Result<Launched, BidiError> {
        profile::prepare(
            &self.profile,
            &self.extra_prefs,
            self.hide_chrome,
            self.render_scale,
        )
        .map_err(|error| BidiError::Launch(error.to_string()))?;
        let process = launcher::launch(&self.executable, &self.profile, START_TIMEOUT).await?;
        Ok(Launched {
            endpoint: process.endpoint().to_owned(),
            process: Some(process),
        })
    }
}

/// How the engine starts Firefox and its agent.
#[derive(Clone)]
pub struct FirefoxConfig {
    /// Starts Firefox.
    pub launch: Arc<dyn Launch>,
    /// The agent bundle (`agent/dist/rb-agent.js`); empty: no agent, no sync.
    pub agent_source: Arc<str>,
    /// The agents' configuration at start.
    pub agent_config: AgentConfig,
}

/// What a Firefox view's agent said.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentReport {
    /// The view.
    pub view: ViewId,
    /// Whether the agent runs in the view's top frame.
    pub top: bool,
    /// The message, parsed as untrusted input.
    pub message: AgentMessage,
}

/// The engine error of a BiDi error.
pub(crate) fn engine_error(error: BidiError) -> EngineError {
    match error {
        BidiError::Closed => EngineError::Closed,
        BidiError::Launch(_)
        | BidiError::TooOld { .. }
        | BidiError::Connect(_)
        | BidiError::NotLoopback => EngineError::Unavailable {
            engine: EngineKind::Firefox,
            reason: error.to_string(),
        },
        other => EngineError::Invalid(other.to_string()),
    }
}
