//! Synchronisation between the views (`docs/sync-protocol.md`): the sync agent's protocol
//! (RB-020-01) - what the agent in each view's isolated world reports, the commands the core
//! sends it, and its configuration - the SyncHub (RB-020-02): routing the active view's
//! events to the others ([`Hub`], [`SyncHub`]), the leader model for form submits
//! ([`leader`]), the desync state ([`desync`]) and the interface to the engines' replay
//! ([`Replay`], RB-020-03), routed per view to its engine's replay ([`EngineRouter`],
//! RB-040-01), the page API's message bus (RB-020-08, [`bus`]), navigation
//! sync as a fallback (RB-020-06, [`NavigationSync`], [`nav_url`]) and form sync's masking for
//! MCP and the recorder (RB-020-05, [`forms`]). The recorder (RB-050-01), fed through
//! [`Observer`], is [`recorder`].

pub mod bus;
pub mod desync;
pub mod driver;
pub mod event;
pub mod forms;
pub mod framework;
pub mod hub;
mod hub_types;
pub mod leader;
pub mod loading;
pub mod message;
pub mod nav_url;
pub mod navigation;
pub mod recorder;
pub mod replay;
pub mod routing;
pub mod switches;
pub mod vitals;

pub use bus::{
    BusError, BusMessage, BusPayload, BusSettings, PAGE_API_GLOBAL, PAGE_BOOT_PARAM, PageEmulation,
    PageInfo, PageState, RateLimiter,
};
pub use desync::{DesyncReason, DesyncTable, ViewDesync};
pub use driver::{DEFAULT_TIMEOUT, Observer, OnChange, QUEUE_DEPTH, SyncHub};
pub use event::{
    AGENT_GLOBAL, BINDING_NAME, BOOT_GLOBAL, ChangeData, ElementSelector, ElementTarget, HoverData,
    InputData, KeyData, MOD_ALT, MOD_CTRL, MOD_META, MOD_SHIFT, MouseButton, MouseData, Offset,
    PROTOCOL_VERSION, PointerData, PointerPhase, ScrollData, SubmitData, SyncEvent, TargetData,
    WORLD_NAME, WheelData,
};
pub use forms::{MASK, is_file_skip, is_sensitive, redact};
pub use framework::{Framework, FrameworkInfo, FrameworkMessage};
pub use hub::{Hub, HubError, HubStatus, Navigation, Route, SkipReason};
pub use leader::{FOLLOW_WINDOW, Follow, LeaderOnly, SubmitPolicy};
pub use loading::{LOAD_START_GRACE, LoadChange, LoadTable};
pub use message::{
    AgentCommand, AgentConfig, AgentMessage, AgentScrollMode, CaptureSwitches, CommandResult,
    DEFAULT_SCROLL_DEBOUNCE_MS, Detected, DetectedTool, EventMessage, MAX_MESSAGE_BYTES,
    ProtocolError, Ready, parse_message,
};
pub use navigation::{
    DEFAULT_SETTLE, FollowMode, Follower, NavFollow, NavOptions, NavigationSync, SETTLE_ROUNDS,
    Settle, SettleOutcome,
};
pub use replay::{NoReplay, RecordingReplay, Replay, ReplayOutcome, ReplayRequest};
pub use routing::{EngineRouter, ViewRoute};
pub use switches::{SyncKind, SyncSwitches, event_type, marks_desync, selector_of, target_of};
pub use vitals::{ClsValue, InpValue, LcpValue, VitalMetric, VitalsError, VitalsMessage};
