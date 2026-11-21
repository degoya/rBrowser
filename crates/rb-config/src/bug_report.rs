//! `report`: Capture > Bug report (RB-093-04) - which parts the bug report bundle holds
//! (`report.include`) and whether the screenshot editor opens on its images
//! (`report.annotate`). Read by `crates/rbrowser/src/bug_report/`. Re-exported from the crate
//! root.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One part of a bug report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ReportPart {
    /// A screenshot of each view, password fields blacked out.
    Screenshots,
    /// One image of every view side by side.
    Overview,
    /// The console errors and warnings and the uncaught exceptions, deduplicated.
    Console,
    /// The failed requests (status 400 and above, aborted), without headers or bodies.
    Network,
    /// The page address and the environment.
    Environment,
    /// Breakpoint, size and emulation of each view.
    Emulation,
    /// The rBrowser version and the engines.
    Version,
}

impl ReportPart {
    /// Every part, in the order the report shows them.
    pub const ALL: [ReportPart; 7] = [
        ReportPart::Screenshots,
        ReportPart::Overview,
        ReportPart::Console,
        ReportPart::Network,
        ReportPart::Environment,
        ReportPart::Emulation,
        ReportPart::Version,
    ];
}

/// `report`: Capture > Bug report (RB-093-04).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct BugReportConfig {
    /// The parts a bug report holds; every part by default.
    #[schemars(extend("uniqueItems" = true))]
    pub include: Vec<ReportPart>,
    /// Open the screenshot editor on the report's newest image once it is written.
    pub annotate: bool,
}

impl Default for BugReportConfig {
    fn default() -> Self {
        Self {
            include: ReportPart::ALL.to_vec(),
            annotate: false,
        }
    }
}

impl BugReportConfig {
    /// Whether the report holds `part`.
    #[must_use]
    pub fn has(&self, part: ReportPart) -> bool {
        self.include.contains(&part)
    }
}

#[cfg(test)]
#[path = "bug_report_tests.rs"]
mod tests;
