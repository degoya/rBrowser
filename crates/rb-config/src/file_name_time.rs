//! `ui.fileNameTime`: the time zone of the names a person reads - screenshot and video run
//! folders, a recording's default name, the sitemap and environment-compare runs (RB-090-69).
//! Timestamps stored in JSON stay UTC either way.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The time zone of the names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FileNameTime {
    /// The system's local time, daylight saving time included.
    #[default]
    Local,
    /// Coordinated Universal Time.
    Utc,
}

#[cfg(test)]
#[path = "file_name_time_tests.rs"]
mod tests;
