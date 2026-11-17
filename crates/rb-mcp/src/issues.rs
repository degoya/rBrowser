//! The `Audits` issues the error report needs (RB-080-09): Chromium reports CSP violations,
//! mixed content and deprecations as issues - deprecations no longer reach the console - so the
//! host keeps those three codes beside the console messages, their URLs masked like every
//! other text of the buffers ([`crate::logs`]).

use serde::Serialize;
use serde_json::Value;

use crate::logs::text_of;

/// The `Audits` issue codes kept; every other issue is ignored.
pub const ISSUE_CODES: &[&str] = &[
    "ContentSecurityPolicyIssue",
    "MixedContentIssue",
    "DeprecationIssue",
];

/// One `Audits` issue of a code of [`ISSUE_CODES`].
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueEntry {
    /// `ContentSecurityPolicyIssue`, `MixedContentIssue` or `DeprecationIssue`.
    pub code: String,
    /// The violated directive (CSP), the resolution status (mixed content: `MixedContentBlocked`,
    /// `MixedContentAutomaticallyUpgraded`, `MixedContentWarning`) or the deprecation's type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// The violation type (CSP: `kURLViolation`, `kInlineViolation`, ...) or the resource type
    /// (mixed content).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// The blocked (CSP) or insecure (mixed content) URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Where it happened: the source location's script or document (CSP, deprecation), the
    /// main resource (mixed content).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// The line (1-based), where known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    /// The column (1-based), where known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
    /// A report-only CSP: reported, not enforced.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub report_only: bool,
    /// When the host received it, milliseconds since the Unix epoch (issues carry no time).
    pub time: f64,
}

/// An `Audits.InspectorIssue` of a code of [`ISSUE_CODES`], its URLs masked.
pub(crate) fn issue(issue: &Value) -> Option<IssueEntry> {
    let code = issue["code"]
        .as_str()
        .filter(|code| ISSUE_CODES.contains(code))?;
    let details = &issue["details"];
    let (kind, detail, url, location) = match code {
        "ContentSecurityPolicyIssue" => {
            let csp = &details["contentSecurityPolicyIssueDetails"];
            (
                &csp["violatedDirective"],
                &csp["contentSecurityPolicyViolationType"],
                &csp["blockedURL"],
                &csp["sourceCodeLocation"],
            )
        }
        "MixedContentIssue" => {
            let mixed = &details["mixedContentIssueDetails"];
            (
                &mixed["resolutionStatus"],
                &mixed["resourceType"],
                &mixed["insecureURL"],
                &Value::Null,
            )
        }
        _ => {
            let deprecation = &details["deprecationIssueDetails"];
            (
                &deprecation["type"],
                &Value::Null,
                &Value::Null,
                &deprecation["sourceCodeLocation"],
            )
        }
    };
    let source_url = match code {
        "MixedContentIssue" => text_of(&details["mixedContentIssueDetails"]["mainResourceURL"]),
        _ => text_of(&location["url"]),
    };
    Some(IssueEntry {
        code: code.to_owned(),
        kind: text_of(kind),
        detail: text_of(detail),
        url: text_of(url).filter(|url| !url.is_empty()),
        source_url: source_url.filter(|url| !url.is_empty()),
        line: location["lineNumber"].as_u64().map(|line| line + 1),
        column: location["columnNumber"].as_u64().map(|column| column + 1),
        report_only: details["contentSecurityPolicyIssueDetails"]["isReportOnly"]
            .as_bool()
            .unwrap_or(false),
        time: now_ms(),
    })
}

fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64() * 1000.0)
}
