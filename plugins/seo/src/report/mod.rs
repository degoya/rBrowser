//! The report of a run, written through `out-write` to `.rbrowser/out/seo/<run>/report.json`
//! and `report.html`: the pages, the findings with their English text, the image list.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, Finding, PluginHost, Severity, ViewId};
use serde::Serialize;

use crate::audit::PageSummary;
use crate::hreflang::matrix::SetReport;
use crate::i18n;
use crate::parity::ParityReport;
use crate::rules::images::{ImageEntry, ok_count};
use crate::run::{Outcome, Scope, Summary};

pub mod html;
pub mod sets;

/// The report format's version.
pub const REPORT_VERSION: u32 = 1;

/// One finding of the report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReportFinding {
    pub rule: String,
    pub severity: Severity,
    /// The message key (`plugin.seo.findings.*`).
    pub message: String,
    /// The message in English.
    pub text: String,
    pub params: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl ReportFinding {
    #[must_use]
    pub fn of(finding: &Finding) -> Self {
        Self {
            rule: finding.rule.clone(),
            severity: finding.severity,
            text: i18n::text(&finding.message, &finding.params),
            message: finding.message.clone(),
            params: finding.params.iter().cloned().collect(),
            view: finding.view,
            url: finding.url.clone(),
            selector: finding.selector.clone(),
            path: finding.path.clone(),
        }
    }
}

/// "X of Y images ok" and the list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Images {
    pub total: usize,
    pub ok: usize,
    pub list: Vec<ImageEntry>,
}

/// `report.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub version: u32,
    pub run: String,
    pub started_at: u64,
    pub url: String,
    pub scope: Scope,
    pub rendered: bool,
    pub summary: Summary,
    pub pages: Vec<PageSummary>,
    pub images: Images,
    /// The language set and its reciprocity matrix (scope `hreflang`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hreflang: Option<SetReport>,
    /// The views compared (`parity`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parity: Option<ParityReport>,
    pub findings: Vec<ReportFinding>,
}

impl Report {
    #[must_use]
    pub fn of(outcome: &Outcome, run: &str, started_at: u64) -> Self {
        let mut findings: Vec<ReportFinding> =
            outcome.findings.iter().map(ReportFinding::of).collect();
        findings.sort_by_key(|finding| std::cmp::Reverse(finding.severity));
        Self {
            version: REPORT_VERSION,
            run: run.to_owned(),
            started_at,
            url: outcome.url.clone(),
            scope: outcome.scope,
            rendered: outcome.rendered,
            summary: Summary::of(&outcome.findings),
            pages: outcome.pages.clone(),
            images: Images {
                total: outcome.images.len(),
                ok: ok_count(&outcome.images),
                list: outcome.images.clone(),
            },
            hreflang: outcome.hreflang.clone(),
            parity: outcome.parity.clone(),
            findings,
        }
    }
}

/// Days since 1970-01-01 as (year, month, day), proleptic Gregorian (H. Hinnant's algorithm).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The run's folder name: `YYYYMMDD-HHMMSS` in UTC, and the view (`-v3`) when one was checked,
/// so the runs of several views in the same second do not meet.
#[must_use]
pub fn run_id(now: u64, view: Option<ViewId>) -> String {
    let seconds = i64::try_from(now).unwrap_or(0);
    let (year, month, day) = civil(seconds.div_euclid(86_400));
    let rest = seconds.rem_euclid(86_400);
    let mut id = format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    );
    if let Some(view) = view {
        id.push_str(&format!("-v{view}"));
    }
    id
}

/// Writes `report.json` and `report.html` below `<run>/`; answers their paths in the project.
///
/// # Errors
///
/// The host refused or failed a write.
pub fn write(host: &mut dyn PluginHost, report: &Report) -> Result<[String; 2], Failure> {
    let json = serde_json::to_vec_pretty(report).unwrap_or_default();
    let json_path = format!("{}/report.json", report.run);
    let html_path = format!("{}/report.html", report.run);
    host.out_write(&json_path, &json)?;
    host.out_write(&html_path, html::render(report).as_bytes())?;
    Ok([
        format!(".rbrowser/out/seo/{json_path}"),
        format!(".rbrowser/out/seo/{html_path}"),
    ])
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
