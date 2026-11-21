//! Checks > Findings, the export (RB-092-05): `checks.export.defaultFormat`,
//! `checks.export.failOn` and `checks.export.dir`, which the project config and the user settings
//! may both hold (the project wins per key, as the rest of `checks`). Read by the findings
//! export of `crates/rbrowser` (`checks::export`): the panel's Export, the command
//! `checks.export`, the MCP tool `checks_export` and `rbrowser check`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A file format of the findings export.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FindingsExportFormat {
    /// SARIF 2.1.0: one run per check plugin, for code-scanning viewers.
    #[default]
    Sarif,
    /// JUnit XML: one test suite per breakpoint, one test case per rule.
    Junit,
    /// One HTML file that opens offline.
    Html,
    /// `findings.json` as the run wrote it.
    Json,
}

/// From which severity on a finding fails `rbrowser check` (exit code 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FindingsFailOn {
    /// Errors only.
    #[default]
    Error,
    /// Warnings and errors.
    Warning,
    /// Every finding.
    Info,
}

/// `checks.export`. A key not set is `None`, so the project can win per key.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct FindingsExportSettings {
    /// The format of the findings panel's Export button, of the command `checks.export` and of
    /// `rbrowser check` without `--format`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("default" = "sarif"))]
    pub default_format: Option<FindingsExportFormat>,
    /// From which severity on `rbrowser check` without `--fail-on` exits with 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("default" = "error"))]
    pub fail_on: Option<FindingsFailOn>,
    /// The folder exports are written to; empty for `.rbrowser/out/exports` in the project. A
    /// relative folder lies below `.rbrowser/out/`; only the user settings may name an absolute
    /// folder. MCP writes only there or below `.rbrowser/out/`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
}

impl FindingsExportSettings {
    /// These values (the project's) over `user`: a key set here wins.
    #[must_use]
    pub fn over(&self, user: &FindingsExportSettings) -> FindingsExportSettings {
        FindingsExportSettings {
            default_format: self.default_format.or(user.default_format),
            fail_on: self.fail_on.or(user.fail_on),
            dir: self.dir.clone().or_else(|| user.dir.clone()),
        }
    }
}

#[cfg(test)]
#[path = "findings_export_tests.rs"]
mod tests;
