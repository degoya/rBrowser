//! The report as one HTML file (`.rbrowser/out/error-report/report.html`): standalone, no script,
//! no network (its own CSP), every text escaped. Its words come from the plugin's English
//! catalogue (`locales/en.json`, compiled in), so they stay with the plugin's other texts.

use std::fmt::Write as _;

use rb_plugin_api::html::escape;
use serde_json::Value;

use crate::PLUGIN_ID;
use crate::classify::Category;
use crate::report::{Entry, Report, view_name};

const CATALOGUE: &str = include_str!("../locales/en.json");

const STYLE: &str = "body{font:14px/1.45 system-ui,sans-serif;margin:24px;color:#1f1f1f}\
h2{margin-top:28px;border-bottom:1px solid #dadce0}li{margin:8px 0}\
.severity{font-weight:600}.error{color:#b3261e}.warning{color:#7a4a00}.info{color:#0b57d0}\
.muted{color:#5f6368}pre{white-space:pre-wrap;background:#f6f8fa;padding:6px}\
code{word-break:break-all}";

/// The plugin's English texts.
pub struct Texts(Value);

impl Texts {
    /// The compiled-in catalogue.
    #[must_use]
    pub fn load() -> Self {
        Self(serde_json::from_str(CATALOGUE).unwrap_or(Value::Null))
    }

    /// The text `plugin.<id>.<key>` with `{name}` replaced from `params`; the key itself when
    /// the catalogue has none.
    #[must_use]
    pub fn t(&self, key: &str, params: &[(&str, String)]) -> String {
        let mut node = &self.0["plugin"][PLUGIN_ID];
        for part in key.split('.') {
            node = &node[part];
        }
        let mut text = node.as_str().unwrap_or(key).to_owned();
        for (name, value) in params {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }
}

/// `seconds` since the Unix epoch as `YYYY-MM-DD HH:MM` (UTC).
#[must_use]
pub fn utc(seconds: u64) -> String {
    let days = i64::try_from(seconds / 86_400).unwrap_or(0);
    let rest = seconds % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        rest / 3600,
        rest % 3600 / 60
    )
}

fn entry(out: &mut String, texts: &Texts, report: &Report, entry: &Entry) {
    let severity = entry.severity.as_str();
    let names: Vec<String> = entry
        .views()
        .into_iter()
        .map(|id| view_name(&report.views, id))
        .collect();
    let _ = write!(
        out,
        "<li><span class=\"severity {severity}\">{}</span> <code>{}</code> <span class=\"muted\">{} - {}</span>",
        escape(&texts.t(&format!("severities.{severity}"), &[])),
        escape(&entry.text),
        escape(&names.join(", ")),
        escape(&texts.t("html.count", &[("count", entry.count().to_string())])),
    );
    if let Some(only) = &entry.only_in {
        let text = texts.t("html.only", &[("views", only.join(", "))]);
        let _ = write!(out, "<br><strong>{}</strong>", escape(&text));
    }
    if let Some(source) = &entry.source {
        let mut place = source.url.clone();
        for part in [source.line, source.column].into_iter().flatten() {
            let _ = write!(place, ":{part}");
        }
        let _ = write!(
            out,
            "<br>{}: <code>{}</code>",
            escape(&texts.t("html.source", &[])),
            escape(&place)
        );
    }
    if let Some(request) = &entry.request {
        let status = request.status.map(|s| s.to_string()).unwrap_or_default();
        let kind = request.resource_type.clone().unwrap_or_default();
        let _ = write!(
            out,
            "<br>{}: <code>{} {} {}</code>",
            escape(&texts.t("html.request", &[])),
            escape(&request.method),
            escape(&status),
            escape(&kind)
        );
    }
    if let Some(detail) = &entry.detail {
        let _ = write!(
            out,
            "<br>{}: <code>{}</code>",
            escape(&texts.t("html.detail", &[])),
            escape(detail)
        );
    }
    if let Some(stack) = &entry.stack {
        let _ = write!(
            out,
            "<details><summary>{}</summary><pre>{}</pre></details>",
            escape(&texts.t("html.stack", &[])),
            escape(stack)
        );
    }
    out.push_str("</li>");
}

/// The report as an HTML page, generated at `now` (seconds since the epoch).
#[must_use]
pub fn render(report: &Report, now: u64) -> String {
    let texts = Texts::load();
    let title = escape(&texts.t("html.title", &[]));
    let mut out = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\">\
<title>{title}</title><style>{STYLE}</style></head><body><h1>{title}</h1>"
    );
    let generated = texts.t(
        "html.generated",
        &[
            ("time", utc(now)),
            ("count", report.capturing.len().to_string()),
        ],
    );
    let _ = write!(out, "<p class=\"muted\">{}</p>", escape(&generated));
    let views: Vec<String> = report
        .views
        .iter()
        .map(|v| {
            format!(
                "{} ({}, {} px, {})",
                v.label, v.breakpoint, v.width, v.engine
            )
        })
        .collect();
    let _ = write!(
        out,
        "<p>{}: {}</p>",
        escape(&texts.t("html.views", &[])),
        escape(&views.join("; "))
    );
    if report.entries.is_empty() {
        let _ = write!(out, "<p>{}</p>", escape(&texts.t("html.none", &[])));
    }
    for category in Category::ALL {
        let entries: Vec<&Entry> = report
            .entries
            .iter()
            .filter(|e| e.category == category)
            .collect();
        if entries.is_empty() {
            continue;
        }
        let heading = texts.t(&format!("rules.{}", category.key_name()), &[]);
        let _ = write!(out, "<h2>{} ({})</h2><ul>", escape(&heading), entries.len());
        for item in entries {
            entry(&mut out, &texts, report, item);
        }
        out.push_str("</ul>");
    }
    out.push_str("</body></html>\n");
    out
}

#[cfg(test)]
#[path = "html_tests.rs"]
mod tests;
