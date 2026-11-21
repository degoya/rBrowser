//! `report` of the user settings (RB-093-04): where a bug report is written and whether the
//! person sees its texts before - the person's own choice, never a project's: a checked-in
//! config could otherwise send a report with the page's state out of the project, or skip the
//! look before it is saved. Both keys are protected from MCP (`annotate::USER_KEYS`, the
//! refusal list of `config_set`). The parts and the editor are the project's `report.include`
//! and `report.annotate` (`bug_report.rs`). Re-exported from the crate root.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `report`: Capture > Bug report, the person's part.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct UserReportSettings {
    /// The folder a bug report is written into, each in a folder of its own named by the time;
    /// empty: the project's `.rbrowser/out/reports/`. An absolute path of an existing folder.
    pub folder: String,
    /// Show the report's texts after masking before it is saved, with Save and Cancel; a
    /// cancelled report leaves nothing behind.
    pub preview: bool,
}

impl Default for UserReportSettings {
    fn default() -> Self {
        Self {
            folder: String::new(),
            preview: true,
        }
    }
}

impl UserReportSettings {
    /// The folder the person chose, trimmed; `None` for the project's.
    #[must_use]
    pub fn chosen_folder(&self) -> Option<&str> {
        Some(self.folder.trim()).filter(|folder| !folder.is_empty())
    }
}

#[cfg(test)]
#[path = "user_report_tests.rs"]
mod tests;
