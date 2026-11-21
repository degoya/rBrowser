//! The gallery of a run: one self-contained `index.html` beside `run.json` - a contact sheet of
//! pages by breakpoints, filters by breakpoint and by status, a large view of each shot with the
//! next and previous one, and the list of errors (HTTP status 400 and above, failed shots,
//! redirects).
//!
//! Everything in a sitemap is somebody else's text, so **every** URL, label and reason is
//! escaped, a link is made only of an `http(s)` URL, and an image path only of plain names (the
//! page's slug and the breakpoint's file name, as `rb_visual::sitemap_store` wrote them). The
//! page runs no script at all: the filters are radio buttons and CSS (`main:has(#radio:checked)
//! .sheet ...` - the radios sit in their fieldsets, the sheet beside them in `<main>`), the large
//! view is a `:target` a thumbnail links to, and the Content Security Policy allows no script, no
//! frame and images from the run's own folder only. Every control is a link, a radio or a label,
//! so the keyboard reaches all of it. The thumbnails are as wide as the plugin's setting
//! `thumbnailWidth` says ([`thumbnail_width`], RB-090-86).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use rb_plugin_api::html::escape;
use serde::Deserialize;

use crate::url::Url;

pub use crate::thumbnail::{SCHEMA, default_thumbnail_width, thumbnail_width};

/// The texts of the page and their English defaults; `args.labels` may replace each (the core
/// passes the catalogue's translations, keys `plugin.sitemap-capture.gallery.<name>`).
pub const LABELS: [(&str, &str); 22] = [
    ("title", "Sitemap capture"),
    ("summary", "{pages} pages in {breakpoints} breakpoints"),
    ("breakpoints", "Breakpoints"),
    ("all_breakpoints", "All"),
    ("status", "Status"),
    ("all_status", "All"),
    ("status_ok", "OK"),
    ("status_problems", "Problems"),
    ("page", "Page"),
    ("shot_alt", "{url} at {breakpoint}"),
    ("pending", "Not captured yet"),
    ("failed", "Failed: {error}"),
    ("errors", "Errors"),
    ("no_errors", "No errors."),
    ("kind_http", "HTTP {status}"),
    ("kind_failed", "Failed"),
    ("kind_redirect", "Redirect"),
    ("redirected_to", "to {url}"),
    ("close", "Close"),
    ("previous", "Previous"),
    ("next", "Next"),
    ("compare", "{status} {percent} %"),
];

/// A breakpoint as `run.json` has it.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Breakpoint {
    pub name: String,
    pub label: String,
    pub width: u32,
    pub height: u32,
}

/// A visual-regression comparison of a shot.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Compare {
    pub status: String,
    pub ratio: f64,
}

/// A shot as `run.json` has it.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Shot {
    /// `pending`, `done` or `failed`.
    pub state: String,
    pub error: Option<String>,
    pub compare: Option<Compare>,
}

/// A page as `run.json` has it.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Page {
    pub url: String,
    pub slug: String,
    pub status: Option<u16>,
    pub final_url: Option<String>,
    pub shots: BTreeMap<String, Shot>,
}

/// `run.json` as far as the gallery reads it (`rb_visual::sitemap::SitemapRun`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Run {
    pub run_id: String,
    pub source: String,
    pub state: String,
    pub breakpoints: Vec<Breakpoint>,
    pub pages: Vec<Page>,
}

/// `text` for an element or a quoted attribute: every control character replaced by U+FFFD
/// (a sitemap's text is one line), then escaped ([`rb_plugin_api::html::escape`]).
#[must_use]
pub fn sanitized(text: &str) -> String {
    let plain: String = text
        .chars()
        .map(|c| if c.is_control() { '\u{fffd}' } else { c })
        .collect();
    escape(&plain)
}

/// A file or folder name `rb_visual` writes: ASCII letters, digits, `.`, `_`, `-`, no leading dot.
fn plain(name: &str) -> bool {
    (1..=80).contains(&name.len())
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// The texts in effect.
struct Texts(BTreeMap<String, String>);

impl Texts {
    fn new(overrides: &BTreeMap<String, String>) -> Self {
        Self(
            LABELS
                .iter()
                .map(|(key, text)| {
                    let text = overrides.get(*key).map_or(*text, String::as_str);
                    ((*key).to_owned(), text.to_owned())
                })
                .collect(),
        )
    }

    /// The text `key` with its `{name}` placeholders filled, escaped.
    fn get(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut text = self.0.get(key).cloned().unwrap_or_default();
        for (name, value) in args {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        sanitized(&text)
    }
}

/// A link to `url` when it is an `http(s)` URL, else the escaped text.
fn link(url: &str) -> String {
    match Url::parse(url) {
        Some(parsed) => {
            let href = sanitized(&parsed.to_string());
            format!(
                r#"<a href="{href}" rel="noreferrer noopener">{}</a>"#,
                sanitized(url)
            )
        }
        None => sanitized(url),
    }
}

/// What went wrong with a page, if anything: the kind and the detail.
fn problems(page: &Page, texts: &Texts) -> Vec<(String, String)> {
    let mut found = Vec::new();
    if let Some(status) = page.status.filter(|status| *status >= 400) {
        found.push((
            texts.get("kind_http", &[("status", &status.to_string())]),
            String::new(),
        ));
    }
    if let Some(target) = page
        .final_url
        .as_deref()
        .filter(|target| *target != page.url)
    {
        found.push((
            texts.get("kind_redirect", &[]),
            texts.get("redirected_to", &[("url", target)]),
        ));
    }
    for (breakpoint, shot) in &page.shots {
        if shot.state == "failed" {
            let error = shot.error.as_deref().unwrap_or_default();
            found.push((
                texts.get("kind_failed", &[]),
                format!("{}: {}", sanitized(breakpoint), sanitized(error)),
            ));
        }
    }
    found
}

/// The page's look: the app's (Nuxt UI, `primary` teal and `neutral` slate, the shell's contrast
/// steps), light and dark by the system's choice, so the export and the app's gallery page look
/// alike (RB-090-86).
const STYLE: &str = ":root{--bg:#fff;--elevated:#f8fafc;--text:#334155;--strong:#0f172a;--muted:#475569;\
--border:#e2e8f0;--primary:#115e59;--error:#b91c1c;--error-bg:#fef2f2;--focus:#115e59;color-scheme:light}\
@media (prefers-color-scheme:dark){:root{--bg:#0f172a;--elevated:#1e293b;--text:#cbd5e1;--strong:#fff;\
--muted:#94a3b8;--border:#1e293b;--primary:#2dd4bf;--error:#fca5a5;--error-bg:#450a0a;--focus:#2dd4bf;color-scheme:dark}}\
body{font:14px/1.5 system-ui,-apple-system,'Segoe UI',sans-serif;margin:0;padding:16px;color:var(--text);background:var(--bg)}\
header{padding:0 0 8px}h1{font-size:18px;font-weight:600;color:var(--strong);margin:0 0 4px}h2{font-size:16px;font-weight:600;\
color:var(--strong)}header p{margin:0;color:var(--muted)}a{color:var(--primary)}.filters{display:flex;flex-wrap:wrap;\
gap:6px 14px;align-items:center;margin:8px 0;border:0;padding:0}.filters legend{font-weight:500;color:var(--strong);padding:0;\
margin-right:8px;float:left}.filters input{accent-color:var(--primary);margin:0 4px 0 8px}table{border-collapse:collapse}\
.sheet{overflow-x:auto;border:1px solid var(--border);border-radius:6px}.sheet table{width:100%}.sheet thead{background:var(--elevated)}\
.sheet th,.sheet td{border-top:1px solid var(--border);padding:8px 12px;vertical-align:top;text-align:left}\
.sheet thead th{font-weight:500;color:var(--strong);border-top:0}.sheet small{color:var(--muted)}\
.sheet th[scope=row]{max-width:260px;word-break:break-all;font-weight:normal}.sheet img{height:auto;display:block;\
cursor:zoom-in;border:1px solid var(--border);border-radius:2px}.badge{display:inline-block;padding:0 6px;border-radius:4px;\
font-size:12px;background:var(--error-bg);color:var(--error)}#errors td{padding:2px 8px 2px 0}\
.view{display:none}.view:target{display:flex;flex-direction:column;position:fixed;inset:0;background:var(--bg);\
padding:16px;overflow:auto;z-index:1}.view img{max-width:100%;height:auto}.view nav{display:flex;gap:16px;margin-bottom:8px}\
a:focus-visible,input:focus-visible+label{outline:2px solid var(--focus);outline-offset:2px;border-radius:2px}";

/// The id of the large view of page `page` in breakpoint `index`.
fn view_id(page: usize, index: usize) -> String {
    format!("v-{page}-{index}")
}

/// The gallery of `run` with thumbnails of the default width.
#[must_use]
pub fn render(run: &Run, labels: &BTreeMap<String, String>) -> String {
    render_sized(run, labels, default_thumbnail_width())
}

/// The gallery of `run` with thumbnails `thumbnail_width` CSS px wide.
#[must_use]
pub fn render_sized(run: &Run, labels: &BTreeMap<String, String>, thumbnail_width: u32) -> String {
    let texts = Texts::new(labels);
    let breakpoints: Vec<&Breakpoint> = run
        .breakpoints
        .iter()
        .filter(|breakpoint| plain(&breakpoint.name))
        .collect();
    let mut html = String::with_capacity(4096 + run.pages.len() * 1024);
    let _ = write!(
        html,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src 'self' file:; style-src 'unsafe-inline'; form-action 'none'; base-uri 'none'\">\
<meta name=\"referrer\" content=\"no-referrer\"><meta name=\"viewport\" content=\"width=device-width\">\
<title>{} - {}</title><style>{STYLE}.sheet img{{width:{thumbnail_width}px}}",
        texts.get("title", &[]),
        sanitized(&run.run_id),
    );
    // The radios and the sheet share `<main>`, not a parent: the rule starts there.
    for index in 0..breakpoints.len() {
        let _ = write!(
            html,
            "main:has(#bp-{index}:checked) .sheet .cell:not(.bp-{index}){{display:none}}"
        );
    }
    html.push_str(
        "main:has(#st-ok:checked) .sheet tr.problem,main:has(#st-problem:checked) .sheet tr.fine{display:none}</style></head><body>",
    );
    let pages = run.pages.len().to_string();
    let count = breakpoints.len().to_string();
    let _ = write!(
        html,
        "<header><h1>{}</h1><p>{} &middot; {} &middot; {}</p><p>{}</p></header><main>",
        texts.get("title", &[]),
        sanitized(&run.source),
        sanitized(&run.run_id),
        sanitized(&run.state),
        texts.get("summary", &[("pages", &pages), ("breakpoints", &count)]),
    );
    let _ = write!(
        html,
        "<fieldset class=\"filters\"><legend>{}</legend><input type=\"radio\" name=\"bp\" id=\"bp-all\" checked><label for=\"bp-all\">{}</label>",
        texts.get("breakpoints", &[]),
        texts.get("all_breakpoints", &[]),
    );
    for (index, breakpoint) in breakpoints.iter().enumerate() {
        let _ = write!(
            html,
            "<input type=\"radio\" name=\"bp\" id=\"bp-{index}\"><label for=\"bp-{index}\">{}</label>",
            sanitized(label(breakpoint)),
        );
    }
    let _ = write!(
        html,
        "</fieldset><fieldset class=\"filters\"><legend>{}</legend>\
<input type=\"radio\" name=\"st\" id=\"st-all\" checked><label for=\"st-all\">{}</label>\
<input type=\"radio\" name=\"st\" id=\"st-ok\"><label for=\"st-ok\">{}</label>\
<input type=\"radio\" name=\"st\" id=\"st-problem\"><label for=\"st-problem\">{}</label></fieldset>",
        texts.get("status", &[]),
        texts.get("all_status", &[]),
        texts.get("status_ok", &[]),
        texts.get("status_problems", &[]),
    );
    sheet(&mut html, run, &breakpoints, &texts);
    errors(&mut html, run, &texts);
    views(&mut html, run, &breakpoints, &texts);
    html.push_str("</main></body></html>\n");
    html
}

fn label(breakpoint: &Breakpoint) -> &str {
    if breakpoint.label.is_empty() {
        &breakpoint.name
    } else {
        &breakpoint.label
    }
}

fn sheet(html: &mut String, run: &Run, breakpoints: &[&Breakpoint], texts: &Texts) {
    let _ = write!(
        html,
        "<div class=\"sheet\"><table><thead><tr><th scope=\"col\">{}</th>",
        texts.get("page", &[]),
    );
    for (index, breakpoint) in breakpoints.iter().enumerate() {
        let _ = write!(
            html,
            "<th scope=\"col\" class=\"cell bp-{index}\">{} <small>{}&times;{}</small></th>",
            sanitized(label(breakpoint)),
            breakpoint.width,
            breakpoint.height,
        );
    }
    html.push_str("</tr></thead><tbody>");
    for (number, page) in run.pages.iter().enumerate() {
        let class = if problems(page, texts).is_empty() {
            "fine"
        } else {
            "problem"
        };
        let badge = page
            .status
            .filter(|status| *status >= 400)
            .map(|status| format!(" <span class=\"badge\">{status}</span>"))
            .unwrap_or_default();
        let _ = write!(
            html,
            "<tr class=\"{class}\"><th scope=\"row\">{}{badge}</th>",
            link(&page.url)
        );
        for (index, breakpoint) in breakpoints.iter().enumerate() {
            let _ = write!(html, "<td class=\"cell bp-{index}\">");
            let shot = page.shots.get(&breakpoint.name);
            match shot.map(|shot| shot.state.as_str()) {
                Some("done") if plain(&page.slug) => {
                    let alt = texts.get(
                        "shot_alt",
                        &[("url", &page.url), ("breakpoint", label(breakpoint))],
                    );
                    let _ = write!(
                        html,
                        "<a href=\"#{}\"><img src=\"{}/{}.png\" alt=\"{alt}\" loading=\"lazy\"></a>",
                        view_id(number, index),
                        page.slug,
                        breakpoint.name,
                    );
                    if let Some(compare) = shot.and_then(|shot| shot.compare.as_ref()) {
                        let percent = format!("{:.2}", compare.ratio * 100.0);
                        let _ = write!(
                            html,
                            "<span class=\"badge\">{}</span>",
                            texts.get(
                                "compare",
                                &[("status", &compare.status), ("percent", &percent)]
                            )
                        );
                    }
                }
                Some("failed") => {
                    let error = shot
                        .and_then(|shot| shot.error.as_deref())
                        .unwrap_or_default();
                    html.push_str(&texts.get("failed", &[("error", error)]));
                }
                _ => html.push_str(&texts.get("pending", &[])),
            }
            html.push_str("</td>");
        }
        html.push_str("</tr>");
    }
    html.push_str("</tbody></table></div>");
}

fn errors(html: &mut String, run: &Run, texts: &Texts) {
    let _ = write!(
        html,
        "<section id=\"errors\"><h2>{}</h2>",
        texts.get("errors", &[])
    );
    let mut rows = String::new();
    for page in &run.pages {
        for (kind, detail) in problems(page, texts) {
            let _ = write!(
                rows,
                "<tr><td>{}</td><td>{kind}</td><td>{detail}</td></tr>",
                link(&page.url)
            );
        }
    }
    if rows.is_empty() {
        let _ = write!(html, "<p>{}</p>", texts.get("no_errors", &[]));
    } else {
        let _ = write!(html, "<table>{rows}</table>");
    }
    html.push_str("</section>");
}

fn views(html: &mut String, run: &Run, breakpoints: &[&Breakpoint], texts: &Texts) {
    let shots: Vec<(usize, usize)> = run
        .pages
        .iter()
        .enumerate()
        .filter(|(_, page)| plain(&page.slug))
        .flat_map(|(number, page)| {
            breakpoints
                .iter()
                .enumerate()
                .filter(move |(_, breakpoint)| {
                    page.shots
                        .get(&breakpoint.name)
                        .is_some_and(|shot| shot.state == "done")
                })
                .map(move |(index, _)| (number, index))
        })
        .collect();
    for (at, (number, index)) in shots.iter().enumerate() {
        let page = &run.pages[*number];
        let breakpoint = breakpoints[*index];
        let alt = texts.get(
            "shot_alt",
            &[("url", &page.url), ("breakpoint", label(breakpoint))],
        );
        let _ = write!(
            html,
            "<section class=\"view\" id=\"{}\" aria-label=\"{alt}\"><nav><a href=\"#\">{}</a>",
            view_id(*number, *index),
            texts.get("close", &[]),
        );
        if let Some((previous_page, previous_index)) = at.checked_sub(1).map(|before| shots[before])
        {
            let _ = write!(
                html,
                "<a href=\"#{}\">{}</a>",
                view_id(previous_page, previous_index),
                texts.get("previous", &[])
            );
        }
        if let Some((next_page, next_index)) = shots.get(at + 1) {
            let _ = write!(
                html,
                "<a href=\"#{}\">{}</a>",
                view_id(*next_page, *next_index),
                texts.get("next", &[])
            );
        }
        let _ = write!(
            html,
            "</nav><p>{} &middot; {}</p><img src=\"{}/{}.png\" alt=\"{alt}\" loading=\"lazy\"></section>",
            link(&page.url),
            sanitized(label(breakpoint)),
            page.slug,
            breakpoint.name,
        );
    }
}

#[cfg(test)]
#[path = "gallery_tests.rs"]
mod tests;
