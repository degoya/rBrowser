//! The findings export (RB-092-05): a check run ([`FindingsFile`]) as SARIF 2.1.0 ([`sarif`]),
//! JUnit XML ([`junit`]), one offline HTML file ([`html`]) or its own JSON. The findings
//! panel's Export button and the palette run the command `checks.export` on the last run
//! ([`command`], also the MCP tool `checks_export`), `rbrowser check` runs the checks in the
//! project's running instance and exports that run ([`check`]). Where a file may go is
//! [`target`]: an agent's path stays below `.rbrowser/out/` or the person's export folder.
//!
//! The renderers are pure: the texts of the findings' message keys (the plugins' locales, as
//! the panel gets them), the engine of each view and the report's own texts come in through an
//! [`ExportContext`].

pub mod check;
pub mod command;
pub mod html;
pub mod junit;
pub mod sarif;
pub mod target;
#[cfg(test)]
mod testing;

use std::collections::{BTreeMap, BTreeSet};

pub use rb_config::{FindingsExportFormat as ExportFormat, FindingsFailOn};
use rb_plugin_types::{FailOn, FindingMessage, FindingsFile, MergedFinding, ViewId};

/// The prefix of the report's own texts in the core catalogue (`checks.export.report.title`).
pub const REPORT_TEXTS: &str = "checks.export.report.";

/// A report text: the key below [`REPORT_TEXTS`] and its arguments.
pub type ReportText<'a> = &'a dyn Fn(&str, &[(&str, &str)]) -> String;

/// What the renderers need beside the run.
pub struct ExportContext<'a> {
    /// The texts of the message keys the findings name, unformatted (`checks::run::messages`);
    /// a key without a text is shown as the key.
    pub messages: &'a BTreeMap<String, String>,
    /// The engine of each view (`chromium`), as far as the window knows it.
    pub engines: &'a BTreeMap<ViewId, String>,
    /// The report's own texts by key below [`REPORT_TEXTS`].
    pub text: ReportText<'a>,
}

impl ExportContext<'_> {
    /// A finding's sentence: its message's text with the parameters filled in.
    pub fn message(&self, message: &FindingMessage) -> String {
        let Some(text) = self.messages.get(&message.key) else {
            return message.key.clone();
        };
        let args: Vec<(&str, &str)> = message
            .params
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        rb_i18n::format_message(text, &args)
    }

    /// The name view `view` of `file` goes by in a report: its breakpoint id, else the text
    /// `view` (`View 3`).
    pub fn view_name(&self, file: &FindingsFile, view: ViewId) -> String {
        file.views
            .iter()
            .find(|checked| checked.id == view)
            .and_then(|checked| checked.breakpoint.clone())
            .unwrap_or_else(|| (self.text)("view", &[("id", &view.to_string())]))
    }

    /// The engines of `finding`'s views, each once, in the order of its views.
    pub fn engines_of(&self, finding: &MergedFinding) -> Vec<String> {
        let mut seen = BTreeSet::new();
        finding
            .views
            .iter()
            .filter_map(|view| self.engines.get(view))
            .filter(|engine| seen.insert(engine.as_str()))
            .cloned()
            .collect()
    }
}

/// The file extension of `format`, without the dot.
pub const fn extension(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Sarif => "sarif",
        ExportFormat::Junit => "xml",
        ExportFormat::Html => "html",
        ExportFormat::Json => "json",
    }
}

/// The name of `format` in settings, arguments and the command line (`sarif`).
pub const fn format_name(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Sarif => "sarif",
        ExportFormat::Junit => "junit",
        ExportFormat::Html => "html",
        ExportFormat::Json => "json",
    }
}

/// The format named `name` (`sarif`, `junit`, `html`, `json`); `None` for anything else.
pub fn parse_format(name: &str) -> Option<ExportFormat> {
    [
        ExportFormat::Sarif,
        ExportFormat::Junit,
        ExportFormat::Html,
        ExportFormat::Json,
    ]
    .into_iter()
    .find(|format| format_name(*format) == name)
}

/// The threshold of `rbrowser check` for the setting's value.
pub const fn fail_on(setting: FindingsFailOn) -> FailOn {
    match setting {
        FindingsFailOn::Error => FailOn::Error,
        FindingsFailOn::Warning => FailOn::Warning,
        FindingsFailOn::Info => FailOn::Info,
    }
}

/// `file` in `format`, as the text of the file to write.
///
/// # Errors
///
/// The run does not serialize (JSON and SARIF); never for a run read from a `findings.json`.
pub fn render(
    file: &FindingsFile,
    format: ExportFormat,
    context: &ExportContext<'_>,
) -> Result<String, serde_json::Error> {
    Ok(match format {
        ExportFormat::Sarif => {
            let mut text = serde_json::to_string_pretty(&sarif::render(file, context))?;
            text.push('\n');
            text
        }
        ExportFormat::Junit => junit::render(file, context),
        ExportFormat::Html => html::render(file, context),
        ExportFormat::Json => {
            let mut text = serde_json::to_string_pretty(file)?;
            text.push('\n');
            text
        }
    })
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
