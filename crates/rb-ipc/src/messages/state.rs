//! The state of the window ([`StateSnapshot`] and its parts) and one setting's value.

use rb_config::{Environment, LayoutMode, SyncConfig};
use rb_engine::external::ExternalView;
use rb_engine::{ViewId, ViewInfo};
use rb_settings::Origin;
use rb_sync::ViewDesync;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::payloads::{
    BookmarkSummary, BreakpointSummary, DetectedBreakpoint, DevtoolsState, RecorderStatus,
    SessionInfo,
};

/// The state of the window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateSnapshot {
    /// The open project; `None` before one is opened.
    pub project: Option<ProjectInfo>,
    /// Every view, in grid order.
    pub views: Vec<ViewInfo>,
    /// The active view.
    pub active_view: Option<ViewId>,
    /// The URL of the address bar (the active view's, else the first view's).
    pub url: String,
    /// The current environment id.
    pub environment: Option<String>,
    /// The project's environments.
    pub environments: Vec<Environment>,
    /// Layout preset and mode.
    pub layout: LayoutState,
    /// What is synced.
    pub sync: SyncState,
    /// The project's breakpoints in config order (focus switcher, quick pick; contract 0.1-w1b).
    pub breakpoints: Vec<BreakpointSummary>,
    /// The project's sessions, `default` first (RB-020-07).
    #[serde(default)]
    pub sessions: Vec<SessionInfo>,
    /// The analyser's last CSS breakpoints by width (`Leader B 1...9`, RB-010-13, RB-010-16).
    pub detected: Vec<DetectedBreakpoint>,
    /// The view the DevTools inspect and where they are; `None` while closed (RB-010-06).
    pub devtools: Option<DevtoolsState>,
    /// Firefox and WebKit views with their cell badge; empty without the tiles (RB-040-01).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub externals: Vec<ExternalView>,
    /// The running recording: the toolbar's indicator; absent while none runs (RB-050-01).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorder: Option<RecorderStatus>,
    /// The project's bookmarks with their URLs in the current environment (RB-070-06).
    #[serde(default)]
    pub bookmarks: Vec<BookmarkSummary>,
    /// The window's last sitemap capture (RB-050-05); absent before the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sitemap: Option<crate::SitemapStatus>,
    /// The Web Vitals and resources per view, for the cell labels; empty while both are off
    /// (RB-060-10).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<crate::ViewMetrics>,
    /// The frameworks each view's page runs, for the DevTools offer; empty while none is
    /// found or both are off (RB-080-24).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frameworks: Vec<crate::ViewFrameworks>,
    /// The running video recording: the activity bar's entry and the toolbar's indicator;
    /// absent while none runs (RB-090-67).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<crate::VideoActivity>,
    /// Where the activity bar has room and which entries are folded; absent while it tells
    /// nothing (RB-090-67).
    #[serde(default, skip_serializing_if = "crate::ActivityBarState::is_empty")]
    pub activity_bar: crate::ActivityBarState,
    /// The views' clock and animation rates while either is changed: the activity bar's entry
    /// (RB-094-07); absent while the time is real and every animation runs at 100 %.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_control: Option<crate::TimeControlActivity>,
    /// The plugin calls running that reported progress, by plugin id: the activity bar's
    /// plugins entry (RB-093-01); absent while none does.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugin_progress: Vec<crate::PluginProgress>,
    /// The views with forced pseudo-class states and the states: the activity bar's entry
    /// with Reset (RB-094-16); absent while no view has one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forced_states: Vec<crate::ViewForcedStates>,
    /// The devices the main menu and the toolbar's "+" offer (RB-094-20); absent until the
    /// window's engines started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devices: Option<crate::DevicesMenu>,
}

/// The open project.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    /// The folder name.
    pub name: String,
    /// The project root, as the user sees it (`\\wsl.localhost\...` included).
    pub root: String,
    /// Whether the workspace is trusted.
    pub trusted: bool,
}

/// The layout part of the state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutState {
    /// The current preset; `None` for the top-level `layout`.
    pub preset: Option<String>,
    /// Every preset id, in config order.
    pub presets: Vec<String>,
    /// Grid, focus, horizontal, split or canvas.
    pub mode: LayoutMode,
    /// Focus mode: the breakpoint shown (RB-010-12).
    pub focus: Option<String>,
    /// Horizontal, split and canvas: what the toolbar's controls show (RB-080-04).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modes: Option<crate::ModeState>,
    /// The breakpoints whose views are not open and have a cell in the grid in effect
    /// (`layout.hidden`, RB-080-41): the "+" menu offers them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<String>,
    /// The minimap switched on or off for this session (`layout.minimap.toggle`), in every
    /// mode; `None` while the setting `layoutModes.minimap` holds (RB-090-82).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimap: Option<bool>,
}

/// The sync part of the state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    /// The global switch.
    pub enabled: bool,
    /// The kinds of sync.
    pub features: SyncConfig,
    /// The views taken out of sync (`Leader S X`; RB-020-02).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded: Vec<ViewId>,
    /// The views that could not follow: the desync badge in the grid gap (RB-020-02).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub desynced: Vec<ViewDesync>,
    /// The views whose navigation is decoupled (`syncNavigation: false`, `Leader S U`): their
    /// label strip is their own address field (RB-020-06).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub navigation_decoupled: Vec<ViewId>,
    /// The site whose Browsersync switched sync off; the shell offers "sync anyway" while sync
    /// is off (RB-020-06).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browsersync: Option<String>,
    /// `Leader S N` is armed: the active view's next click is not synced (RB-020-05).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub skip_next_click: bool,
}

/// One setting's effective value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingValue {
    /// The dotted key.
    pub key: String,
    /// The value.
    pub value: Value,
    /// Where it comes from (the badge).
    pub origin: Origin,
}
