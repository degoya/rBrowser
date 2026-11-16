//! `ui.newViewSize` (RB-080-46): what the "New size..." picker lists.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `ui.newViewSize`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct NewViewSizeSettings {
    /// The "New size..." picker lists the device presets (the built-in ones and your own,
    /// with their pixel ratio) below the sizes. Off leaves the sizes and "Custom...".
    pub devices: bool,
}

impl Default for NewViewSizeSettings {
    fn default() -> Self {
        Self { devices: true }
    }
}
