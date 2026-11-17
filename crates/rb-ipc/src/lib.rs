//! The protocol between the shell UI (Vue, its own CEF view) and the Rust core (contract 0.1,
//! `docs/contracts-0.1.md`). TypeScript mirror: `ui/src/ipc/protocol.ts`.
//!
//! **Transport.**
//! - Shell -> core: CEF's message router. The shell calls
//!   `window.cefQuery({ request: JSON.stringify(msg), onSuccess, onFailure })` with a
//!   [`ShellToCore`]. The core answers each query once: `onSuccess(json)` with an encoded
//!   [`CoreToShell`] ([`encode_response`]), or `onFailure(code, message)` with an
//!   [`IpcErrorCode`].
//! - Core -> shell: events. The core runs [`event_script`] in the shell's frame
//!   (`ExecuteJavaScript`), which dispatches `new CustomEvent("rb-ipc", { detail: msg })` on
//!   `window`; the shell listens with `window.addEventListener("rb-ipc", ...)`.
//!
//! The message types and the three transport functions are the contract (implemented here, as
//! they define the wire format). RB-010-07 implements the router handler, the dispatch and the
//! shell side.

mod activity;
mod checks;
mod frameworks;
mod layout_modes;
mod lists;
mod messages;
mod metrics;
mod native_menu;
mod payloads;
mod portal;
mod recordings;
mod sitemap;
mod status;
mod videos;

use serde::Serialize;

pub use activity::{Activity, ActivityBarState, VideoActivity};
pub use checks::{CheckJob, CheckJobState, ChecksProgress};
pub use frameworks::ViewFrameworks;
pub use layout_modes::{
    BoardCamera, BoardItem, BoardPiece, BoardState, DragPhase, LabelPiece, MapRect, MinimapItem,
    MinimapState, ModeState, SplitState, StripState,
};
pub use lists::{FlowList, FlowSummary, ProjectShortcut, ProjectShortcutList};
pub use messages::{
    CoreToShell, LayoutState, ProjectInfo, SettingValue, ShellToCore, StateSnapshot, SyncFeature,
    SyncState,
};
pub use metrics::{Rating, ResourceReading, Unavailable, ViewMetrics, ViewThrottle, VitalReading};
pub use native_menu::NativeMenuRow;
pub use payloads::{
    AboutInfo, AnalyzerHint, BindingCheck, BookmarkSummary, BoundaryKind, BreakpointProposal,
    BreakpointSummary, CommandInfo, DetectedBreakpoint, DevtoolsState, ExtensionPrompt,
    ExtensionPromptKind, Notification, NotificationLevel, PathKind, PluginGrantDetails, ProjectOff,
    ProposedBreakpoint, RecorderPhase, RecorderStatus, SessionInfo, SettingIssue, StylesheetSource,
    TrustDecision, TrustInfo, TrustPrompt, TrustStatus, TrustedProject, WebStoreExtension,
};
pub use portal::{PortalDevice, PortalPairing, PortalState};
pub use rb_commands::{WhichKey, WhichKeyEntry};
pub use rb_config::trust::{BlockedItem, ProtectedKind};
pub use rb_engine::ViewId;
pub use rb_settings::Scope;
pub use recordings::{
    FlowRunState, FlowRunning, FlowViewStatus, RecordingEntry, RecordingExport,
    RecordingExportFile, RecordingExportFormat, RecordingList,
};
pub use sitemap::{SitemapRunState, SitemapStatus};
pub use status::{StatusAction, StatusMessage};
pub use videos::{VideoConversion, VideoFile, VideoList, VideoRecording, VideoTarget};

/// The protocol version; raised by the coordinator on an incompatible change.
pub const PROTOCOL_VERSION: u32 = 1;
/// The message router's query function in the shell's `window`.
pub const QUERY_FUNCTION: &str = "cefQuery";
/// The message router's cancel function in the shell's `window`.
pub const QUERY_CANCEL_FUNCTION: &str = "cefQueryCancel";
/// The name of the `CustomEvent` that carries core -> shell messages.
pub const EVENT_NAME: &str = "rb-ipc";

/// The error code of a failed query (`onFailure(code, message)`), HTTP-like.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum IpcErrorCode {
    /// The request is not valid JSON or not a known message.
    BadRequest = 400,
    /// Refused (a protected setting, a scope the key does not allow).
    Refused = 403,
    /// The command, view, preset or setting does not exist.
    NotFound = 404,
    /// The core failed.
    Internal = 500,
    /// The handler is not implemented yet.
    NotImplemented = 501,
}

/// A failed IPC operation.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IpcError {
    /// The request could not be parsed.
    #[error("bad request: {0}")]
    BadRequest(String),
    /// A message could not be encoded.
    #[error("cannot encode message: {0}")]
    Encode(String),
}

impl IpcError {
    /// The code for `onFailure`.
    pub fn code(&self) -> IpcErrorCode {
        match self {
            IpcError::BadRequest(_) => IpcErrorCode::BadRequest,
            IpcError::Encode(_) => IpcErrorCode::Internal,
        }
    }
}

/// Parses the `request` string of a `cefQuery`.
pub fn parse_request(request: &str) -> Result<ShellToCore, IpcError> {
    serde_json::from_str(request).map_err(|error| IpcError::BadRequest(error.to_string()))
}

/// The `onSuccess` string of a query's answer.
pub fn encode_response(message: &CoreToShell) -> Result<String, IpcError> {
    encode(message)
}

/// The JavaScript that delivers `message` to the shell as an `rb-ipc` event. JSON is valid
/// JavaScript, so the message is embedded as an object literal.
pub fn event_script(message: &CoreToShell) -> Result<String, IpcError> {
    let json = encode(message)?;
    Ok(format!(
        "window.dispatchEvent(new CustomEvent(\"{EVENT_NAME}\",{{detail:{json}}}));"
    ))
}

fn encode<T: Serialize>(message: &T) -> Result<String, IpcError> {
    serde_json::to_string(message).map_err(|error| IpcError::Encode(error.to_string()))
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
