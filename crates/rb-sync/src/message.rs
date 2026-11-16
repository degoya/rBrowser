//! The envelopes around the events: what the agent sends over the binding
//! ([`AgentMessage`]), the commands the core evaluates in the agent's world
//! ([`AgentCommand`]), their answers ([`CommandResult`]) and the agent's configuration
//! ([`AgentConfig`], from `sync.*`). The page API's bus messages and page state are
//! [`crate::bus`] (RB-020-08).
//!
//! A binding payload comes from a page's renderer and is parsed as untrusted input:
//! [`parse_message`] refuses payloads over [`MAX_MESSAGE_BYTES`] and other protocol versions,
//! and its errors never quote the payload.

use serde::{Deserialize, Serialize};

use crate::bus::{BusMessage, BusPayload, PageState};
use crate::event::{ChangeData, ElementTarget, PROTOCOL_VERSION, ScrollData, SyncEvent};
use crate::framework::FrameworkMessage;
use crate::vitals::VitalsMessage;

/// The largest binding payload the core reads.
pub const MAX_MESSAGE_BYTES: usize = 256 * 1024;
/// [`AgentConfig::scroll_debounce_ms`] by default (`sync.scrollDebounceMs`, RB-020-04).
pub const DEFAULT_SCROLL_DEBOUNCE_MS: u32 = 150;

fn default_scroll_debounce_ms() -> u32 {
    DEFAULT_SCROLL_DEBOUNCE_MS
}

/// The agent started in a document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ready {
    /// The protocol version.
    pub v: u32,
    /// Whether the agent runs in the top frame.
    pub top: bool,
    /// The CSS paths of the iframes from the top document down, when every hop is same-origin;
    /// otherwise the core knows the frame from the CDP session and context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<Vec<String>>,
    /// The document's URL.
    pub url: String,
}

/// A captured event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventMessage {
    /// The protocol version.
    pub v: u32,
    /// Whether the agent runs in the top frame.
    pub top: bool,
    /// As [`Ready::frame`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<Vec<String>>,
    /// Counts up from 1 per document.
    pub seq: u64,
    /// When, ms since the epoch (`Date.now()` in the page).
    pub time: u64,
    /// The event.
    pub event: SyncEvent,
}

/// A tool that syncs the views by itself (RB-020-06).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DetectedTool {
    /// Browsersync's client (its ghost mode replays clicks, scrolls and forms).
    Browsersync,
}

/// The agent found a tool in its document that syncs by itself (RB-020-06).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Detected {
    /// The protocol version.
    pub v: u32,
    /// Whether the agent runs in the top frame.
    pub top: bool,
    /// As [`Ready::frame`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<Vec<String>>,
    /// The tool.
    pub tool: DetectedTool,
}

/// Agent -> core: the binding's payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum AgentMessage {
    /// The agent started.
    Ready(Ready),
    /// An event was captured.
    Event(EventMessage),
    /// A page sent a bus message (RB-020-08).
    Bus(BusMessage),
    /// A tool that syncs by itself is in the page (RB-020-06).
    Detected(Detected),
    /// The top document's Web Vitals (RB-060-10).
    Vitals(VitalsMessage),
    /// The top document's frameworks, Vue and React (RB-080-24).
    Framework(FrameworkMessage),
}

impl AgentMessage {
    /// The protocol version the message carries.
    pub fn version(&self) -> u32 {
        match self {
            Self::Ready(ready) => ready.v,
            Self::Event(event) => event.v,
            Self::Bus(bus) => bus.v,
            Self::Detected(detected) => detected.v,
            Self::Vitals(vitals) => vitals.v,
            Self::Framework(framework) => framework.v,
        }
    }
}

/// What the agent captures (`AgentConfig.capture`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureSwitches {
    /// Pointer down/up, click, double click, context menu (`sync.click`).
    pub click: bool,
    /// Hover (`sync.hover`).
    pub hover: bool,
    /// Scroll and wheel (`sync.scroll`).
    pub scroll: bool,
    /// Key down/up (`sync.keyboard`).
    pub keyboard: bool,
    /// Input and submit (`sync.input`).
    pub input: bool,
    /// Change of form controls (`sync.forms`, RB-020-05).
    pub forms: bool,
    /// Focus (`sync.focus`, RB-020-05).
    pub focus: bool,
}

/// The agent's configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfig {
    /// Only the active view captures.
    pub active: bool,
    /// Per kind of event.
    pub capture: CaptureSwitches,
    /// The least time between two hover events (`sync.hoverThrottleMs`).
    pub hover_throttle_ms: u32,
    /// How long scrolling rests before its end position is sent once more, freshly anchored
    /// (`sync.scrollDebounceMs`; 0: never; RB-020-04).
    #[serde(default = "default_scroll_debounce_ms")]
    pub scroll_debounce_ms: u32,
}

/// Core -> agent, evaluated as `__rbAgent.command(<json>)` in the agent's world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "lowercase")]
pub enum AgentCommand {
    /// Replaces the configuration.
    Configure {
        /// The new configuration.
        config: AgentConfig,
    },
    /// The replay marker: nothing is captured for `ms` (0 ends it; at most 10 s), so the
    /// engine's replayed input causes no echo.
    Replay {
        /// How long.
        ms: u32,
    },
    /// Resolves a target to a viewport point.
    Resolve {
        /// The target.
        target: ElementTarget,
    },
    /// Scroll sync (RB-020-04): brings the content the leader shows at a container's edge to the
    /// same edge here (`Done`; `NotFound` when the container is missing). Sets its own replay
    /// marker.
    Scroll(ScrollData),
    /// The point of `target` inside the viewport, never scrolling; the viewport's centre when
    /// it is missing (`Ok`; the wheel replay of RB-020-04).
    Point {
        /// The target.
        target: ElementTarget,
    },
    /// Sets a form control to the leader's state (RB-020-05); `Debug` masks the value.
    Apply(ChangeData),
    /// Focuses an element without scrolling (RB-020-05).
    Focus {
        /// The element.
        target: ElementTarget,
    },
    /// Is the agent there?
    Ping,
    /// The page API's state of the view (RB-020-08): the agent keeps it and hands it to the
    /// page's main world.
    Page {
        /// The state.
        page: PageState,
    },
    /// A bus message for the page (RB-020-08); delivered only while the bus is on and, when
    /// `origin` is given, only to a page of that origin.
    Bus {
        /// The view that sent it; `None` for the core (MCP's `bus_send`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<u32>,
        /// The sender's origin; `None` delivers to any page.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin: Option<String>,
        /// The message type.
        r#type: String,
        /// The payload, serialised once for every view ([`crate::bus::check_payload`]).
        payload: BusPayload,
    },
    /// Moves the document to `url` through the history API, without a reload: `pushState`
    /// and a `popstate` for the page's router, or the hash for a fragment (RB-020-06). Refused
    /// for another origin.
    Navigate {
        /// The URL, of the document's origin.
        url: String,
    },
}

/// The answer to an [`AgentCommand`]. `Ok`, `NotFound` and `NotVisible` answer `Resolve`;
/// `Apply` and `Focus` answer `Done`, `NotFound` or `NotVisible`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CommandResult {
    /// Done.
    Done,
    /// The target is at (`x`, `y`), CSS pixels of the agent's frame viewport.
    Ok {
        /// Horizontal.
        x: f64,
        /// Vertical.
        y: f64,
        /// Whether the element was scrolled into view first.
        scrolled: bool,
    },
    /// No element matches.
    NotFound,
    /// The element exists but is not rendered (`display:none`, no box).
    NotVisible,
    /// The command was refused.
    Error {
        /// Why.
        message: String,
    },
}

/// A binding payload the core does not take.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    /// Over [`MAX_MESSAGE_BYTES`].
    #[error("sync agent message of {0} bytes is over the limit")]
    TooLarge(usize),
    /// Not a message of this protocol (the position only, never the payload).
    #[error("sync agent message is invalid ({category} at line {line}, column {column})")]
    Invalid {
        /// syntax, data or eof.
        category: &'static str,
        /// Line of the error.
        line: usize,
        /// Column of the error.
        column: usize,
    },
    /// Another protocol version.
    #[error("sync agent protocol version {0} is not {PROTOCOL_VERSION}")]
    Version(u32),
}

/// Parses one binding payload.
pub fn parse_message(payload: &str) -> Result<AgentMessage, ProtocolError> {
    if payload.len() > MAX_MESSAGE_BYTES {
        return Err(ProtocolError::TooLarge(payload.len()));
    }
    let message: AgentMessage = serde_json::from_str(payload).map_err(|error| {
        let category = match error.classify() {
            serde_json::error::Category::Io => "io",
            serde_json::error::Category::Syntax => "syntax",
            serde_json::error::Category::Data => "data",
            serde_json::error::Category::Eof => "eof",
        };
        ProtocolError::Invalid {
            category,
            line: error.line(),
            column: error.column(),
        }
    })?;
    match message.version() {
        PROTOCOL_VERSION => Ok(message),
        other => Err(ProtocolError::Version(other)),
    }
}

#[cfg(test)]
#[path = "message_tests.rs"]
mod tests;
