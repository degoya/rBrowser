//! A run's failures as findings (RB-096-04): every check that failed and every request that got
//! no answer becomes a finding of the plugin `api-client`, shown in the findings panel over the
//! last run of the other plugins (`crate::checks::reported`, RB-094-14). The core reports them
//! itself as part of the `api-client` capability's work; the person's rule overrides and ignore
//! patterns still hold (`api-client.check-failed`, `api-client.request-failed`). A finding's
//! element is `<request id>#<check number>`, so the same check is the same finding in every run.

use rb_api_model::verdict::Problem;
use rb_plugin_types::{Finding, Severity};

use super::collection::{Ran, Report};

/// The rule of a failed check.
pub const CHECK_FAILED: &str = "check-failed";
/// The rule of a request that got no answer.
pub const REQUEST_FAILED: &str = "request-failed";

fn key(name: &str) -> String {
    format!("plugin.{}.findings.{name}", super::PLUGIN_ID)
}

fn url_of(ran: &Ran) -> String {
    ran.sent_url.clone().unwrap_or_else(|| ran.url.clone())
}

/// The findings of `report`, in its order.
#[must_use]
pub fn of(report: &Report) -> Vec<Finding> {
    let mut out = Vec::new();
    for ran in &report.requests {
        if let Some(failure) = &ran.error {
            out.push(Finding {
                rule: REQUEST_FAILED.to_owned(),
                severity: Severity::Error,
                message: key("request_failed"),
                params: vec![("request".to_owned(), ran.id.clone())],
                // Why, as the panel's report says it: a finding's text takes no raw code.
                data: serde_json::to_string(failure).ok(),
                url: Some(url_of(ran)),
                path: Some(ran.id.clone()),
                ..Finding::default()
            });
            continue;
        }
        for (index, verdict) in ran.checks.iter().enumerate() {
            if verdict.passed {
                continue;
            }
            let mut params = vec![
                ("request".to_owned(), ran.id.clone()),
                ("check".to_owned(), verdict.check.clone()),
            ];
            // What was found, or why nothing was: each problem a text of its own.
            let message = match verdict.problem {
                Some(Problem::Missing) => "check_missing",
                Some(Problem::NotJson) => "check_not_json",
                Some(Problem::BadPath) => "check_bad_path",
                None => {
                    params.push((
                        "actual".to_owned(),
                        verdict.actual.clone().unwrap_or_default(),
                    ));
                    "check_failed"
                }
            };
            out.push(Finding {
                rule: CHECK_FAILED.to_owned(),
                severity: Severity::Error,
                message: key(message),
                params,
                url: Some(url_of(ran)),
                path: Some(format!("{}#{}", ran.id, index + 1)),
                data: serde_json::to_string(verdict).ok(),
                ..Finding::default()
            });
        }
    }
    out
}

#[cfg(test)]
#[path = "findings_tests.rs"]
mod tests;
