//! The bug report's messages (RB-093-04): the preview of a report before it is saved (the
//! route `#/bug-report` of the pages window) and the status bar's "Copy Markdown" - the core
//! has no clipboard, so the status bar asks for the masked text and copies it itself. Inside
//! `ShellToCore::BugReport` and `CoreToShell::BugReport`; mirrored in `ui/src/ipc/bugReport.ts`.

use serde::{Deserialize, Serialize};

/// A request of the bug report's pages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum BugReportRequest {
    /// The report waiting for the person's word; answered with `bugReport` (`preview`), `ack`
    /// when none waits.
    GetPreview,
    /// The person's word on preview `preview_id`: save the report, or drop it with its
    /// screenshots. Answered with `ack`; only the pages window sends it.
    Decide {
        /// The preview answered.
        preview_id: u32,
        /// Save it.
        save: bool,
    },
    /// The masked `report.md` of report `report` - its folder's name, as the status message's
    /// copy button names it; only a report of this run is known. Answered with `bugReport`
    /// (`markdown`).
    GetMarkdown {
        /// The report's folder name.
        report: String,
    },
}

/// The core's answer to a [`BugReportRequest`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BugReportAnswer {
    /// The report waiting for the person's word.
    Preview(ReportPreview),
    /// The masked `report.md` of a saved report.
    Markdown {
        /// The report's folder name.
        report: String,
        /// `report.md`.
        text: String,
    },
}

/// A report gathered and masked, not saved yet: what the preview shows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportPreview {
    /// The preview's number, for the decision.
    pub preview_id: u32,
    /// The report's folder.
    pub folder: String,
    /// `report.md` as it will be saved: every text masked.
    pub markdown: String,
    /// The screenshots taken (password fields blacked out), the overview counted.
    pub images: u32,
    /// `report.annotate`: the screenshot editor opens once the report is saved.
    pub annotate: bool,
}

#[cfg(test)]
#[path = "bug_report_tests.rs"]
mod tests;
