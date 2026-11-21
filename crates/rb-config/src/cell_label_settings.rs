//! `ui.cellLabel` (RB-090-27): what a view's label strip shows beside its name and size - the
//! state of an external engine's window and the view's address. The Web Vitals moved to a bar
//! of their own under the view (RB-090-71, `ui.viewFooter.vitals`); an older file's
//! `ui.cellLabel.vitals` is still read without an error and not written again.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `ui.cellLabel`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct CellLabelSettings {
    /// The label strip of a Firefox or WebKit view says how its engine's window stands
    /// (starting, in its cell, engine stopped, ...), in the engine badge beside the engine's
    /// name (`Firefox · window closed`), the reason in the tooltip; the badge's menu then offers
    /// to restart a stopped engine or bring its window to the front. Off, the engine badge and
    /// MCP's `views_list` still name the engine.
    pub engine_status: bool,
    /// When the label strip shows the view's address: `differs` while it is not the active
    /// view's address (a view with its own navigation, sync off, a view that could not follow),
    /// `always` for every view, `off` never.
    pub address: CellAddress,
    /// A click on the address in the label strip of a view with its own navigation (decoupled
    /// with `Leader S U` or `syncNavigation: false`) opens an address field there, as does
    /// `view.edit_address` (`Leader S L`): Enter loads the address in that view only, Escape
    /// cancels (RB-092-03). Off, the address only shows and its click makes the view active.
    pub address_editable: bool,
}

impl Default for CellLabelSettings {
    fn default() -> Self {
        Self {
            engine_status: true,
            address: CellAddress::Differs,
            address_editable: true,
        }
    }
}

/// `ui.cellLabel.address`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CellAddress {
    /// Never.
    Off,
    /// While the view's address is not the active view's.
    #[default]
    Differs,
    /// For every view.
    Always,
}

#[cfg(test)]
#[path = "cell_label_settings_tests.rs"]
mod tests;
