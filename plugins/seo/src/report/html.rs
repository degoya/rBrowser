//! `report.html`: one static page - no script, no request (`default-src 'none'`), every text
//! escaped; the labels from the plugin's catalogue. Severity is spelled out, never only a
//! colour.

use std::fmt::Write as _;

use rb_plugin_api::Severity;
pub use rb_plugin_api::html::escape;

use crate::i18n;
use crate::report::Report;

fn label(key: &str) -> String {
    escape(&i18n::text(&format!("plugin.seo.report.{key}"), &[]))
}

fn severity(severity: Severity) -> String {
    label(&format!("severity.{}", severity.as_str()))
}

const STYLE: &str = "body{font:14px/1.5 system-ui,sans-serif;margin:24px;color:#1a1a1a;background:#fff}\
table{border-collapse:collapse;width:100%;margin:8px 0 24px}th,td{border:1px solid #c8c8c8;padding:4px 8px;\
text-align:left;vertical-align:top}th{background:#f0f0f0}code{font-size:12px;word-break:break-all}\
.error{color:#9b1c1c;font-weight:600}.warning{color:#7a4a00;font-weight:600}.info{color:#1d4f91}\
caption{text-align:left;font-weight:600;padding:4px 0}td.missing{color:#9b1c1c;font-weight:600}\
td.own,td.unknown{color:#4a4a4a}";

/// The report as HTML.
#[must_use]
pub fn render(report: &Report) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\">\
<title>{title}</title><style>{STYLE}</style></head><body><h1>{title}</h1>",
        title = label("title"),
    );
    let summary = &report.summary;
    let _ = write!(
        out,
        "<p><code>{url}</code> - {run} <code>{id}</code></p><p>{errors}: {e}, {warnings}: {w}, \
{infos}: {i}; {images}: {ok} / {total}</p>",
        url = escape(&report.url),
        run = label("run"),
        id = escape(&report.run),
        errors = label("severity.error"),
        e = summary.error,
        warnings = label("severity.warning"),
        w = summary.warning,
        infos = label("severity.info"),
        i = summary.info,
        images = label("images_ok"),
        ok = report.images.ok,
        total = report.images.total,
    );
    let _ = write!(
        out,
        "<h2>{}</h2><table><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr>",
        label("pages"),
        label("url"),
        label("status"),
        label("indexable"),
        label("page_title"),
    );
    for page in &report.pages {
        let _ = write!(
            out,
            "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(&page.final_url),
            page.status
                .map_or_else(|| "-".to_owned(), |status| status.to_string()),
            label(if page.indexable { "yes" } else { "no" }),
            escape(page.title.as_deref().unwrap_or_default()),
        );
    }
    out.push_str("</table>");
    if let Some(set) = &report.hreflang {
        super::sets::hreflang(&mut out, set);
    }
    if let Some(parity) = &report.parity {
        super::sets::parity(&mut out, parity);
    }
    let _ = write!(
        out,
        "<h2>{}</h2><table><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr>",
        label("findings"),
        label("severity_column"),
        label("finding"),
        label("url"),
        label("element"),
    );
    for finding in &report.findings {
        let _ = write!(
            out,
            "<tr><td class=\"{class}\">{severity}</td><td>{text}<br><code>{rule}</code></td>\
<td><code>{url}</code></td><td><code>{path}</code></td></tr>",
            class = finding.severity.as_str(),
            severity = severity(finding.severity),
            text = escape(&finding.text),
            rule = escape(&finding.rule),
            url = escape(finding.url.as_deref().unwrap_or_default()),
            path = escape(finding.path.as_deref().unwrap_or_default()),
        );
    }
    let _ = write!(
        out,
        "</table><h2>{}</h2><table><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr>",
        label("images"),
        label("src"),
        label("alt"),
        label("status"),
        label("element"),
    );
    for image in &report.images.list {
        let status = if image.status.is_empty() {
            label("ok")
        } else {
            escape(&image.status.join(", "))
        };
        let alt = image.alt.as_deref().map_or_else(
            || label("no_alt"),
            |alt| format!("<code>{}</code>", escape(alt)),
        );
        let _ = write!(
            out,
            "<tr><td><code>{}</code></td><td>{alt}</td><td>{status}</td><td><code>{}</code></td></tr>",
            escape(&image.src),
            escape(&image.path),
        );
    }
    out.push_str("</table></body></html>\n");
    out
}
