//! The `logs` section of the user settings (RB-010-10), beside `user_settings.rs`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `logs`: the log file and the diagnostics bundle (RB-010-10). The defaults are those of
/// `rb_diag::LogSettings`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct LogSettings {
    /// The level of the log file: `error`, `warn`, `info`, `debug` or `trace` (`RUST_LOG`, when
    /// set, wins).
    #[schemars(extend("enum" = ["error", "warn", "info", "debug", "trace"]))]
    pub level: String,
    /// The size in MiB at which the log file rotates.
    #[schemars(range(min = 1, max = 500))]
    pub max_file_size_mb: u32,
    /// The log files kept, the current one included.
    #[schemars(range(min = 1, max = 50))]
    pub max_files: u32,
    /// The newest crash dumps the diagnostics bundle takes; 0 takes none.
    #[schemars(range(max = 20))]
    pub crash_dumps_in_bundle: u32,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            level: "info".to_owned(),
            max_file_size_mb: 10,
            max_files: 5,
            crash_dumps_in_bundle: 3,
        }
    }
}
