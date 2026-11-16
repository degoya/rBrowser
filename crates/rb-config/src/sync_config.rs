//! `sync`: what is replayed in the other views (RB-020-*), split from [`crate::model`] and
//! re-exported there.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `sync`: what is replayed in the other views (RB-020-*). All on by default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncConfig {
    /// Navigation.
    pub navigation: bool,
    /// Scrolling.
    pub scroll: bool,
    /// Clicks.
    pub click: bool,
    /// Text input.
    pub input: bool,
    /// Keyboard.
    pub keyboard: bool,
    /// Hover.
    pub hover: bool,
    /// Form submits and non-GET navigations run only in the active view.
    pub leader_only_submit: bool,
    /// With `leader_only_submit`, every session has a leader of its own (RB-020-07).
    pub leader_per_session: bool,
    /// A click into a view (it gets the keyboard focus) makes it the active view, the leader;
    /// off (the default), only a click on its label does (RB-020-02).
    pub activate_on_focus: bool,
    /// The least time between two hover events of the sync agent, in ms (RB-020-01).
    #[schemars(range(max = 2000))]
    pub hover_throttle_ms: u32,
    /// How long a view may take to replay an event before it is desynced, in ms (RB-020-02).
    #[schemars(range(min = 100, max = 5000))]
    pub desync_timeout_ms: u32,
    /// Right clicks are replayed, so the page's context menu opens in every view (RB-020-03). Off
    /// by default: the browser's own menu would open in every view at once.
    pub context_menu: bool,
    /// The state of form controls (select, radio, checkbox, range, ...) follows a change in the
    /// active view (RB-020-05).
    pub forms: bool,
    /// The focused field follows the active view's focus (RB-020-05). Off by default.
    pub focus: bool,
    /// How long input sync has to take the other views to the active view's page before they
    /// load its URL, in ms (RB-020-06).
    #[schemars(range(min = 100, max = 10000))]
    pub navigation_settle_ms: u32,
    /// A different fragment (`#part`) counts as a different page for navigation sync
    /// (RB-020-06).
    pub navigation_include_hash: bool,
    /// Switch sync off when a page carries Browsersync's client, whose ghost mode syncs by
    /// itself (RB-020-06).
    pub detect_browsersync: bool,
    /// How scroll sync moves the other views (RB-020-04).
    pub scroll_mode: ScrollMode,
    /// How long scrolling rests before its end position is sent once more, freshly anchored, in
    /// ms; 0 never (RB-020-04).
    #[schemars(range(max = 2000))]
    pub scroll_debounce_ms: u32,
}

/// How scroll sync moves the other views (`sync.scrollMode`, RB-020-04).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ScrollMode {
    /// The same content at the same edge: an anchor element and its offset, set directly.
    #[default]
    Anchor,
    /// Real wheel events over the same element, for pages that take over scrolling.
    Wheel,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            navigation: true,
            scroll: true,
            click: true,
            input: true,
            keyboard: true,
            hover: true,
            leader_only_submit: true,
            leader_per_session: true,
            activate_on_focus: false,
            hover_throttle_ms: 100,
            desync_timeout_ms: 750,
            context_menu: false,
            forms: true,
            focus: false,
            navigation_settle_ms: 1000,
            navigation_include_hash: false,
            detect_browsersync: true,
            scroll_mode: ScrollMode::Anchor,
            scroll_debounce_ms: 150,
        }
    }
}
