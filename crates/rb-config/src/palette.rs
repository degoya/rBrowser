//! `palette` of the user settings: the command palette (Settings > Keyboard Shortcuts > Command
//! palette, RB-060-11), read by the shell (`ui/src/palette/constants.ts`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `palette`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct PaletteSettings {
    /// How many recently used entries the palette lists first; 0 keeps no history.
    #[schemars(range(max = 50))]
    pub recent_count: u32,
    /// The palette shows the description of the selected entry.
    pub show_descriptions: bool,
    /// `>` lists only commands, `@` only views, `#` only settings; off, they are searched as
    /// text.
    pub prefixes: bool,
}

impl Default for PaletteSettings {
    fn default() -> Self {
        Self {
            recent_count: 8,
            show_descriptions: true,
            prefixes: true,
        }
    }
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;
