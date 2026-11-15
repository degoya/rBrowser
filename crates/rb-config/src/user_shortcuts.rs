//! The shortcut sections of the user settings, `shortcuts` (RB-010-17) and
//! `breakpointShortcuts` (RB-010-16); split out of `user_settings.rs` for its length.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `shortcuts` (RB-010-17).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ShortcutSettings {
    /// How long the leader waits for the next key, in ms; `0` (the default): until `Escape` or
    /// the next key, the which-key bar staying up (RB-080-18).
    #[schemars(range(min = 0, max = 10000))]
    pub leader_timeout_ms: u32,
    /// rBrowser's shortcuts are off while DevTools have focus.
    pub disable_in_devtools: bool,
    /// The user's bindings by command id, in the text form of `rb-commands`; they replace the
    /// command's defaults. An empty list unbinds the command.
    pub bindings: BTreeMap<String, Vec<String>>,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            leader_timeout_ms: 0,
            disable_in_devtools: true,
            bindings: BTreeMap::new(),
        }
    }
}

/// `breakpointShortcuts` (RB-010-16).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct BreakpointShortcutSettings {
    /// A modifier (`Shift` or `Ctrl`) that, held with a breakpoint shortcut that does not
    /// contain it already (a project's own `shortcut`) or with a click in the size quick pick,
    /// opens a new view instead of changing the active one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_view_modifier: Option<NewViewModifier>,
    /// How long the overlay naming the breakpoint stays, in ms.
    pub overlay_ms: u32,
}

/// `breakpointShortcuts.newViewModifier`. No `Alt`: with the `Ctrl` of a breakpoint key it is
/// AltGr on German, French and Spanish keyboards; no `Win`: Windows keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum NewViewModifier {
    /// `Shift`.
    Shift,
    /// `Ctrl`.
    Ctrl,
}

impl Default for BreakpointShortcutSettings {
    fn default() -> Self {
        Self {
            new_view_modifier: None,
            overlay_ms: 1200,
        }
    }
}
