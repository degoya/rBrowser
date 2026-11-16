//! Views and the host that owns them: view ids, view state, emulation, the grid, and the
//! [`ViewHost`] trait - the operations other jobs call on the CEF app (contract 0.1,
//! `docs/contracts-0.1.md`).
//!
//! RB-010-04 implements [`ViewHost`] in `crates/rbrowser` (CEF, grid, CDP pipe). RB-010-05
//! (emulation), RB-010-06 (DevTools), RB-010-07 (shell), RB-010-12 (presets, environments) and
//! RB-010-16 (breakpoint shortcuts) call it through an `Arc<dyn ViewHost>` and never touch CEF.
//!
//! The engine contract (RB-040-01): [`Engine`] and [`ViewHandle`] ([`engine`]), what an engine
//! did with an emulation ([`EmulationReport`]), Chromium over the host ([`chromium`]), the
//! stand-in for engines that have not landed ([`unsupported`]), the external windows of Firefox
//! and WebKit ([`external`]) and the doubles and conformance checks for tests (`test_support`,
//! built for tests and with the feature `test-support` only). [`chromium`] also holds the hidden
//! render views and the batch that drives them (RB-050-05).

pub mod chromium;
mod emulation;
pub mod emulation_extras;
pub mod emulation_report;
pub mod engine;
pub mod external;
mod grid;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod unsupported;

use std::fmt;

use async_trait::async_trait;
use rb_config::{Breakpoint, DevtoolsMode, Layout};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

pub use emulation::{Emulation, EmulationChange, EmulationSetArgs, resolve_emulation};
pub use emulation_report::{EmulationReport, FieldReport, FieldSupport};
pub use engine::{
    Button, CellApplied, Engine, EngineHealth, HandleEvent, HandleEvents, InputEvent, Phase,
    Screenshot, ScreenshotRequest, ViewHandle, ViewSpec, engine_name,
};
pub use grid::{CellRect, GridCell, compute_grid};
pub use rb_config::EngineKind;

/// A view's id: unique for the lifetime of the instance, never reused. Serialized as a number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ViewId(pub u32);

impl fmt::Display for ViewId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The target of an action, as in every MCP tool: `"all"` or a view id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    /// Every view.
    All,
    /// One view.
    View(ViewId),
}

impl Serialize for Target {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Target::All => serializer.serialize_str("all"),
            Target::View(id) => id.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Target {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Id(u32),
            Text(String),
        }
        match Repr::deserialize(deserializer)? {
            Repr::Id(id) => Ok(Target::View(ViewId(id))),
            Repr::Text(text) if text == "all" => Ok(Target::All),
            Repr::Text(text) => Err(serde::de::Error::custom(format!(
                "target must be \"all\" or a view id, not \"{text}\""
            ))),
        }
    }
}

/// What the shell, MCP and commands know about a view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewInfo {
    /// The id.
    pub id: ViewId,
    /// The label shown in the cell (breakpoint label, or the size for a free size).
    pub label: String,
    /// The breakpoint the view shows now; `None` after a free size (`500x800`, +/-1 px).
    pub breakpoint: Option<String>,
    /// The breakpoint of the view's cell - what `view.reset_size` returns to.
    pub cell_breakpoint: Option<String>,
    /// The engine.
    pub engine: EngineKind,
    /// Viewport width in CSS px.
    pub width: u32,
    /// Viewport height in CSS px.
    pub height: u32,
    /// Device pixel ratio.
    pub dpr: f64,
    /// The current scale of the view into its cell.
    pub scale: f64,
    /// The current URL.
    pub url: String,
    /// The page title.
    pub title: String,
    /// Whether a load is in progress.
    pub loading: bool,
    /// The session the view belongs to.
    pub session: String,
    /// The cell of the grid; `None` for a view outside it (external window, focus switcher).
    pub cell: Option<GridCell>,
}

/// A change of the views, pushed to subscribers (the shell's state, MCP notifications).
#[derive(Clone, Debug, PartialEq)]
pub enum ViewEvent {
    /// A view was created.
    Opened(ViewId),
    /// A view was closed.
    Closed(ViewId),
    /// A view navigated (committed).
    Navigated {
        /// The view.
        view: ViewId,
        /// The new URL.
        url: String,
    },
    /// A view's title, loading state, size, emulation or cell changed.
    Updated(ViewId),
    /// The active view changed.
    ActiveChanged(Option<ViewId>),
    /// The grid was rebuilt (layout or preset changed, window resized).
    LayoutChanged,
    /// Something the shell shows besides the views changed: workspace trust, environment,
    /// preset or focus, the inspected view (contract 0.1-w1b).
    StateChanged,
}

/// Why a view operation failed.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The contract stub: the implementing job has not landed yet.
    #[error("not implemented yet (contract 0.1 stub)")]
    NotImplemented,
    /// No view has this id.
    #[error("unknown view {0}")]
    UnknownView(ViewId),
    /// The engine of the view cannot do this (Firefox has no reduced-motion emulation).
    #[error("{feature} is not available in {engine:?}")]
    Unsupported {
        /// The view's engine.
        engine: EngineKind,
        /// What was asked for.
        feature: String,
    },
    /// A CDP command failed; `code` and `message` are CDP's.
    #[error("CDP {method} failed ({code}): {message}")]
    Cdp {
        /// The CDP method.
        method: String,
        /// CDP's error code.
        code: i64,
        /// CDP's error message.
        message: String,
    },
    /// The browser or its connection is gone (shutdown, crash).
    #[error("the browser is not available")]
    Closed,
    /// The engine cannot run: not implemented yet, not installed (RB-040-01).
    #[error("{reason}")]
    Unavailable {
        /// The engine.
        engine: EngineKind,
        /// Why, for the user; names the engine.
        reason: String,
    },
    /// Invalid input (a cell outside the grid, a zero size).
    #[error("invalid: {0}")]
    Invalid(String),
}

/// The operations of the CEF app on its views. RB-010-04 implements it; the methods return
/// quickly and never block the UI thread (CEF work is posted to the UI thread, CDP work runs on
/// tokio).
#[async_trait]
pub trait ViewHost: Send + Sync {
    /// Every view, in grid order.
    fn views(&self) -> Vec<ViewInfo>;

    /// One view.
    fn view(&self, view: ViewId) -> Option<ViewInfo>;

    /// The active view: focused or last clicked (RB-010-16).
    fn active_view(&self) -> Option<ViewId>;

    /// Makes `view` the active view.
    fn set_active_view(&self, view: ViewId) -> Result<(), EngineError>;

    /// Sets `view` to `breakpoint`: size and full emulation (resolved with RB-010-05's
    /// [`resolve_emulation`]), without reload; sync stays on.
    fn set_breakpoint(&self, view: ViewId, breakpoint: &Breakpoint) -> Result<(), EngineError>;

    /// Sets a free size (`500x800`, +/-1 px), keeping the rest of the emulation; `dpr` `None`
    /// keeps the current one.
    fn set_size(
        &self,
        view: ViewId,
        width: u32,
        height: u32,
        dpr: Option<f64>,
    ) -> Result<(), EngineError>;

    /// Applies a resolved emulation to `view`, live where CDP allows. The host remembers it and
    /// re-applies it after every CDP reconnect and resize; it is the only owner of emulation.
    fn apply_emulation(&self, view: ViewId, emulation: &Emulation) -> Result<(), EngineError>;

    /// The emulation `view` has now.
    fn emulation(&self, view: ViewId) -> Option<Emulation>;

    /// Opens a new view with `breakpoint` outside the grid order (the modifier of RB-010-16).
    fn open_view(&self, breakpoint: &Breakpoint) -> Result<ViewId, EngineError>;

    /// Rebuilds the grid for `layout` (a preset; focus mode is a 1 x 1 layout). Views whose
    /// cell keeps its breakpoint are not reloaded.
    fn set_layout(&self, layout: &Layout) -> Result<(), EngineError>;

    /// Loads `url` in the target views.
    fn navigate(&self, target: Target, url: &str) -> Result<(), EngineError>;

    /// Reloads the target views; `ignore_cache` bypasses the cache.
    fn reload(&self, target: Target, ignore_cache: bool) -> Result<(), EngineError>;

    /// History back in the target views.
    fn go_back(&self, target: Target) -> Result<(), EngineError>;

    /// History forward in the target views.
    fn go_forward(&self, target: Target) -> Result<(), EngineError>;

    /// Opens DevTools for `view`, docked or as a window (RB-010-06).
    fn open_devtools(&self, view: ViewId, mode: DevtoolsMode) -> Result<(), EngineError>;

    /// Sends one CDP command to `view`'s target over the pipe and returns its `result`.
    /// `Input.*` coordinates are CSS px here; the host scales them (ADR 0001, Decision 3).
    async fn cdp_send(
        &self,
        view: ViewId,
        method: &str,
        params: Value,
    ) -> Result<Value, EngineError>;

    /// A receiver of every [`ViewEvent`] from now on.
    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<ViewEvent>;
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
