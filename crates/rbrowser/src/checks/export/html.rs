//! A check run as one HTML file that opens offline: no script, no image, no stylesheet or font
//! from anywhere - the style is inline and a Content-Security-Policy of `default-src 'none'`
//! holds the page to it, so opening it sends no request. One table of the findings, errors
//! first, with the rule, the sentence, the page, the element, the breakpoints and engines and a
//! rule's help page (a link only to an `http(s)` page). Every text from the run is escaped
//! (`rb_plugin_types::html::escape`); the page's own texts come from the core catalogue
//! (`checks.export.report.*`) through the context.

use std::fmt::Write as _;

use rb_plugin_types::{FindingsFile, MergedFinding, html::escape};

use super::{ExportContext, sarif};

/// The page's Content-Security-Policy: nothing loads, the inline style applies.
pub const CSP: &str = "default-src 'none'; style-src 'unsafe-inline'";

const STYLE: &str = "body{font:14px system-ui,sans-serif;margin:16px;color:#1f2328;background:#fff}\
h1{font-size:20px}table{border-collapse:collapse;width:100%}th,td{border:1px solid #d0d7de;\
padding:4px 8px;text-align:left;vertical-align:top}th{background:#f6f8fa}code{font-size:12px;\
word-break:break-all}tr.error td:first-child{color:#cf222e;font-weight:600}tr.warning \
td:first-child{color:#9a6700;font-weight:600}tr.info td:first-child{color:#0969da}\
@media (prefers-color-scheme:dark){body{color:#e6edf3;background:#0d1117}th{background:#161b22}\
th,td{border-color:#30363d}}";

/// The columns, by text key.
const COLUMNS: [&str; 8] = [
    "severity",
    "rule",
    "message",
    "url",
    "element",
    "breakpoints",
    "engines",
    "help",
];

/// The page of `file`.
pub fn render(file: &FindingsFile, context: &ExportContext<'_>) -> String {
    let say = |key: &str, args: &[(&str, &str)]| escape(&(context.text)(key, args));
    let title = format!("{} {}", say("title", &[]), escape(&file.run_id));
    let mut page = String::new();
    let _ = write!(
        page,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{CSP}\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{title}</title><style>{STYLE}</style></head><body><h1>{title}</h1>"
    );
    let summary = &file.summary;
    let _ = write!(
        page,
        "<p>{}</p><p>{}: {}</p>",
        say(
            "summary",
            &[
                ("total", &summary.total.to_string()),
                ("error", &summary.error.to_string()),
                ("warning", &summary.warning.to_string()),
                ("info", &summary.info.to_string()),
            ],
        ),
        say("plugins", &[]),
        escape(&file.plugins.join(", "))
    );
    if !file.failures.is_empty() {
        let _ = write!(page, "<h2>{}</h2><ul>", say("failures", &[]));
        for failure in &file.failures {
            let _ = write!(
                page,
                "<li>{}</li>",
                escape(&sarif::failure_text(failure, context))
            );
        }
        page.push_str("</ul>");
    }
    let _ = write!(page, "<h2>{}</h2>", say("findings", &[]));
    if file.findings.is_empty() {
        let _ = write!(page, "<p>{}</p>", say("none", &[]));
    } else {
        page.push_str("<table><thead><tr>");
        for column in COLUMNS {
            let _ = write!(page, "<th>{}</th>", say(column, &[]));
        }
        page.push_str("</tr></thead><tbody>");
        let mut findings: Vec<&MergedFinding> = file.findings.iter().collect();
        // Errors first; the rule and the run's order within a severity.
        findings.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.rule_id.cmp(&b.rule_id)));
        for finding in findings {
            row(&mut page, file, finding, context);
        }
        page.push_str("</tbody></table>");
    }
    page.push_str("</body></html>\n");
    page
}

fn row(
    page: &mut String,
    file: &FindingsFile,
    finding: &MergedFinding,
    context: &ExportContext<'_>,
) {
    let severity = finding.severity.as_str();
    let element = finding
        .element
        .as_ref()
        .and_then(|element| element.selector.as_deref().or(element.path.as_deref()))
        .map_or_else(String::new, |element| {
            format!("<code>{}</code>", escape(element))
        });
    let names: Vec<String> = finding
        .views
        .iter()
        .map(|view| context.view_name(file, *view))
        .collect();
    let help = finding
        .help_url
        .as_deref()
        .filter(|url| sarif::is_web(url))
        .map_or_else(String::new, |url| {
            format!(
                "<a href=\"{}\" rel=\"noreferrer\">{}</a>",
                escape(url),
                escape(&(context.text)("help", &[]))
            )
        });
    let _ = write!(
        page,
        "<tr class=\"finding {severity}\"><td>{}</td><td><code>{}</code></td><td>{}</td>\
         <td>{}</td><td>{element}</td><td>{}</td><td>{}</td><td>{help}</td></tr>",
        escape(&(context.text)(&format!("level.{severity}"), &[])),
        escape(&finding.rule_id),
        escape(&context.message(&finding.message)),
        escape(finding.url.as_deref().unwrap_or_default()),
        escape(&names.join(", ")),
        escape(&context.engines_of(finding).join(", "))
    );
}

#[cfg(test)]
#[path = "html_tests.rs"]
mod tests;
