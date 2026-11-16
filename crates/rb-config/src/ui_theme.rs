//! `ui.theme`: the shell's colour theme (RB-010-07).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The shell's colour theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Follow Windows.
    #[default]
    System,
    /// Light.
    Light,
    /// Dark.
    Dark,
}
