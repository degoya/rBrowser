//! The MCP tool `seo_check` and the commands `plugin.seo.check` (`Ctrl+Shift+Space K S`),
//! `plugin.seo.hreflang` (`K H`, the scope `hreflang`) and `plugin.seo.parity` (`K P`,
//! `parity: true`): one run, its report written, its findings reported to the findings panel,
//! the answer in short. RB-080-11: `groups` narrows a run to rule groups;
//! `plugin.seo.check_wellknown` (`K W`) is `groups: ["wellknown"]`.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::config::Config;
use crate::report::{self, Report, ReportFinding};
use crate::rules::{GROUPS, of_groups};
use crate::run::{self, Request, Scope};
use crate::url::MAX_URL_CHARS;

/// The most findings the answer lists; the report holds them all.
pub const MAX_ANSWERED: usize = 200;
/// The most missing pairs of the matrix the answer lists; the report holds them all.
pub const MAX_PAIRS: usize = 50;

/// The arguments of `seo_check`.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct Args {
    url: Option<String>,
    scope: Option<String>,
    max_pages: Option<u64>,
    parity: Option<bool>,
    /// The rule groups to run (`wellknown`, `social`, ...); every group when absent.
    groups: Option<Vec<String>>,
    /// `"all"` or a view id, as the host resolves MCP's `target`.
    target: Option<Value>,
}

pub(crate) fn bad_args(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, "plugin.seo.errors.bad_args").with_param("reason", reason)
}

/// `args` (JSON, empty for `{}`) with `key` set to `value` unless it is set already: a
/// command's fixed argument (`scope: "hreflang"`) under what its caller sent.
#[must_use]
pub fn with(args: &str, key: &str, value: Value) -> String {
    let mut object = if args.trim().is_empty() {
        serde_json::Map::new()
    } else {
        match serde_json::from_str::<Value>(args) {
            Ok(Value::Object(object)) => object,
            // Left as it is, so the check answers it with `bad_args`.
            _ => return args.to_owned(),
        }
    };
    object.entry(key).or_insert(value);
    Value::Object(object).to_string()
}

/// The view to compare: the target's, else the active one.
pub(crate) fn view_of(
    host: &mut dyn PluginHost,
    target: Option<&Value>,
) -> Result<Option<ViewId>, Failure> {
    match target {
        Some(Value::Number(number)) => number
            .as_u64()
            .and_then(|id| ViewId::try_from(id).ok())
            .map(Some)
            .ok_or_else(|| bad_args("target")),
        Some(Value::String(all)) if all == "all" => Ok(host.active_view().ok().flatten()),
        Some(_) => Err(bad_args("target")),
        None => Ok(host.active_view().ok().flatten()),
    }
}

/// Runs `seo_check` with `args` (JSON); answers JSON.
///
/// # Errors
///
/// The arguments are malformed (`plugin.seo.errors.bad_args`), or the run failed.
pub fn check(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let value: Value = if args.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(args).map_err(|_| bad_args("json"))?
    };
    let parsed: Args = serde_json::from_value(value.clone()).map_err(|_| bad_args("fields"))?;
    if parsed
        .url
        .as_ref()
        .is_some_and(|url| url.len() > MAX_URL_CHARS)
    {
        return Err(bad_args("url"));
    }
    if parsed.max_pages == Some(0) {
        return Err(bad_args("maxPages"));
    }
    let scope = match parsed.scope.as_deref() {
        None | Some("page") => Scope::Page,
        Some("crawl") => Scope::Crawl,
        Some("hreflang") => Scope::Hreflang,
        Some(_) => return Err(bad_args("scope")),
    };
    let rules = match &parsed.groups {
        None => Vec::new(),
        Some(groups)
            if !groups.is_empty()
                && groups.iter().all(|group| GROUPS.contains(&group.as_str())) =>
        {
            of_groups(groups)
        }
        Some(_) => return Err(bad_args("groups")),
    };
    let config = Config::read(host, &value);
    let view = view_of(host, parsed.target.as_ref())?;
    let request = Request {
        url: parsed.url,
        scope,
        view,
        rules,
        parity: parsed.parity.unwrap_or(false),
    };
    let started = host.now_unix_seconds();
    let outcome = run::run(host, &config, &request)?;
    let run_id = report::run_id(started, view);
    let report = Report::of(&outcome, &run_id, started);
    let files = match report::write(host, &report) {
        Ok(paths) => Some(paths),
        Err(failure) => {
            host.log(
                "warn",
                &format!(
                    "seo: report not written: {}",
                    failure.code().unwrap_or("unknown")
                ),
            );
            None
        }
    };
    if !outcome.findings.is_empty()
        && let Err(failure) = host.report_findings(&outcome.findings)
    {
        host.log(
            "warn",
            &format!(
                "seo: findings not reported: {}",
                failure.code().unwrap_or("unknown")
            ),
        );
    }
    let findings: Vec<&ReportFinding> = report.findings.iter().take(MAX_ANSWERED).collect();
    let answer = json!({
        "run": run_id,
        "url": report.url,
        "scope": report.scope,
        "rendered": report.rendered,
        "pages": report.pages.len(),
        "summary": report.summary,
        "images": { "total": report.images.total, "ok": report.images.ok },
        "hreflang": report.hreflang.as_ref().map(|set| json!({
            "pages": set.pages.len(),
            "missing": set.missing.len(),
            "pairs": set.missing.iter().take(MAX_PAIRS).collect::<Vec<_>>(),
        })),
        "parity": report.parity.as_ref().map(|parity| json!({
            "reference": parity.reference,
            "views": parity.views.len(),
        })),
        "findings": findings,
        "truncated": report.findings.len() > MAX_ANSWERED,
        "report": files.map(|[json, html]| json!({ "json": json, "html": html })),
    });
    Ok(answer.to_string())
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
