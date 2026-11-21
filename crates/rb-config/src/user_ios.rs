//! `engines.ios`: Mobile Safari on iOS over `safaridriver`, macOS only (RB-095-04) - in the iOS
//! Simulator when Xcode is installed, else on an iPhone or iPad connected over USB. On Windows
//! and Linux the keys are kept and do nothing.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `engines.ios`: iOS views over `safaridriver`, macOS only (RB-095-04).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct IosSettings {
    /// Where an iOS view runs: the simulator when Xcode is installed, else a connected device
    /// (`auto`); only the iOS Simulator (`simulator`, needs Xcode); only a connected iPhone or
    /// iPad (`device`).
    #[schemars(
        regex(pattern = r"^(auto|simulator|device)$"),
        extend("enum" = ["auto", "simulator", "device"])
    )]
    pub mode: String,
    /// The simulator model an iOS breakpoint without `iosDevice` runs on (`iPhone 16`).
    pub default_simulator: String,
    /// How long a simulator may take to boot, in seconds.
    #[schemars(range(min = 10, max = 600))]
    pub boot_timeout_sec: u32,
}

impl Default for IosSettings {
    fn default() -> Self {
        Self {
            mode: "auto".to_owned(),
            default_simulator: "iPhone 16".to_owned(),
            boot_timeout_sec: 120,
        }
    }
}

#[cfg(test)]
#[path = "user_ios_tests.rs"]
mod tests;
