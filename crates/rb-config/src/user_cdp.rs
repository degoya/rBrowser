//! `cdp` of the user settings: the external CDP proxy (RB-030-06), moved out of
//! `user_settings.rs` and re-exported there.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `cdp`: the external CDP proxy.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct CdpSettings {
    /// Off by default.
    pub enabled: bool,
    /// A fixed port; random when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}
