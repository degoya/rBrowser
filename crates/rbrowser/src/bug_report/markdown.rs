//! `report.md` of a bug report (RB-093-04): a Markdown file to paste into a ticket, its images
//! by paths relative to the file. Everything a page wrote - console texts, addresses, titles -
//! stands in code spans or fenced code blocks whose fence is longer than any backtick run inside,
//! so a hostile page cannot inject Markdown (an image that calls home from the ticket, a link)
//! or HTML into the ticket. The texts are already masked ([`super::mask`]); the words of the
//! report come from `text` (`report.md.<key>` of the catalogue in the app).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use super::collect::{ConsoleProblem, FailedRequest, Level};
use super::facts::ViewFacts;

/// A word of the report: the key after `report.md.` and its placeholders.
pub type Text<'a> = &'a dyn Fn(&str, &[(&str, &str)]) -> String;

/// One image of the report.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Image {
    /// The view; `None` for the overview.
    pub view: Option<u32>,
    /// The path relative to `report.md`, `/`-separated.
    pub path: Option<String>,
    /// Why there is no image.
    pub error: Option<String>,
}

/// What `report.md` shows; a part the settings leave out is `None` or empty.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// When it was made, as the folder name says it.
    pub created: String,
    /// The rBrowser version (part `version`).
    pub version: Option<String>,
    /// The environment's id and address (part `environment`).
    pub environment: Option<(String, String)>,
    /// The person's or agent's note, masked.
    pub note: Option<String>,
    /// The views.
    pub views: Vec<ViewFacts>,
    /// Pages and titles (part `environment`).
    pub show_pages: bool,
    /// Breakpoint, size and emulation (part `emulation`).
    pub show_emulation: bool,
    /// The engines (part `version`).
    pub show_engines: bool,
    /// One per view (part `screenshots`).
    pub images: Vec<Image>,
    /// The overview (part `overview`).
    pub overview: Option<Image>,
    /// The console problems (part `console`).
    pub console: Option<Vec<ConsoleProblem>>,
    /// The failed requests (part `network`).
    pub requests: Option<Vec<FailedRequest>>,
}

/// `report` as Markdown.
pub fn render(report: &Report, text: Text) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# {}\n", plain(&text("title", &[])));
    let _ = writeln!(out, "- {}: {}", text("created", &[]), code(&report.created));
    if let Some(version) = &report.version {
        let _ = writeln!(out, "- {}: {}", text("version", &[]), code(version));
    }
    if let Some((id, url)) = &report.environment {
        let _ = writeln!(
            out,
            "- {}: {} {}",
            text("environment", &[]),
            code(id),
            code(url)
        );
    }
    if let Some(note) = &report.note {
        let _ = write!(out, "\n## {}\n\n{}", text("note", &[]), block(note));
    }
    let _ = writeln!(out, "\n## {}", text("views", &[]));
    for view in &report.views {
        view_section(&mut out, report, view, text);
    }
    if let Some(overview) = &report.overview {
        let _ = writeln!(out, "\n## {}\n", text("overview", &[]));
        image(&mut out, overview, &text("overview", &[]), text);
    }
    if let Some(console) = &report.console {
        let count = console.len().to_string();
        let _ = writeln!(out, "\n## {}\n", text("console", &[("count", &count)]));
        if console.is_empty() {
            let _ = writeln!(out, "{}", text("console_none", &[]));
        } else {
            let lines: Vec<String> = console
                .iter()
                .map(|line| console_line(line, text))
                .collect();
            out.push_str(&block(&lines.join("\n")));
        }
    }
    if let Some(requests) = &report.requests {
        let count = requests.len().to_string();
        let _ = writeln!(out, "\n## {}\n", text("requests", &[("count", &count)]));
        if requests.is_empty() {
            let _ = writeln!(out, "{}", text("requests_none", &[]));
        } else {
            let lines: Vec<String> = requests.iter().map(request_line).collect();
            out.push_str(&block(&lines.join("\n")));
        }
    }
    out
}

fn view_section(out: &mut String, report: &Report, view: &ViewFacts, text: Text) {
    let id = view.id.to_string();
    let _ = writeln!(
        out,
        "\n### {} {}\n",
        text("view", &[("id", &id)]),
        code(&view.label)
    );
    if report.show_pages {
        let _ = writeln!(out, "- {}: {}", text("page", &[]), code(&view.url));
        let _ = writeln!(out, "- {}: {}", text("page_title", &[]), code(&view.title));
    }
    if report.show_emulation {
        let size = format!("{} x {} @{}x", view.width, view.height, view.dpr);
        let breakpoint = view.breakpoint.as_deref().unwrap_or("-");
        let _ = writeln!(
            out,
            "- {}: {} {}",
            text("breakpoint", &[]),
            code(breakpoint),
            code(&size)
        );
        if !view.emulation.is_empty() {
            let fields: Vec<String> = view
                .emulation
                .iter()
                .map(|(name, value)| format!("{name}: {value}"))
                .collect();
            let _ = writeln!(
                out,
                "- {}: {}",
                text("emulation", &[]),
                code(&fields.join("; "))
            );
        }
    }
    if report.show_engines {
        let _ = writeln!(out, "- {}: {}", text("engine", &[]), code(&view.engine));
    }
    if let Some(shot) = report.images.iter().find(|shot| shot.view == Some(view.id)) {
        out.push('\n');
        image(out, shot, &view.label, text);
    }
}

fn image(out: &mut String, shot: &Image, alt: &str, text: Text) {
    match (&shot.path, &shot.error) {
        (Some(path), _) => {
            let _ = writeln!(out, "![{}]({})", alt_text(alt), link_path(path));
        }
        (None, error) => {
            let reason = error.as_deref().unwrap_or("-");
            let _ = writeln!(out, "{}", text("no_image", &[("reason", &plain(reason))]));
        }
    }
}

fn console_line(line: &ConsoleProblem, text: Text) -> String {
    let level = match line.level {
        Level::Error => text("level_error", &[]),
        Level::Warning => text("level_warning", &[]),
    };
    let mut shown = format!(
        "[{level}] x{} ({}) {}",
        line.count,
        views(&line.views),
        one_line(&line.text)
    );
    if let Some(source) = &line.source {
        let _ = write!(shown, "\n    {}", one_line(source));
    }
    shown
}

fn request_line(request: &FailedRequest) -> String {
    let outcome = match (request.status, &request.failure) {
        (Some(status), _) => status.to_string(),
        (None, Some(failure)) => one_line(failure),
        (None, None) => "-".to_owned(),
    };
    format!(
        "{outcome} {} {} x{} ({})",
        request.method,
        one_line(&request.url),
        request.count,
        views(&request.views)
    )
}

fn views(ids: &BTreeSet<u32>) -> String {
    let ids: Vec<String> = ids.iter().map(u32::to_string).collect();
    format!("views {}", ids.join(", "))
}

/// `text` on one line: line breaks and other control characters as spaces.
fn one_line(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// `text` without the characters Markdown or HTML act on, for a heading or a sentence.
fn plain(text: &str) -> String {
    one_line(text)
        .chars()
        .filter(|c| !"`*_[]()<>#!|\\".contains(*c))
        .collect()
}

/// `text` as an inline code span, its fence longer than any backtick run inside.
pub fn code(text: &str) -> String {
    let text = one_line(text);
    if text.trim().is_empty() {
        return "`-`".to_owned();
    }
    let fence = "`".repeat(longest_run(&text) + 1);
    let pad = if text.starts_with('`') || text.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{fence}{pad}{text}{pad}{fence}")
}

/// `text` as a fenced code block, its fence longer than any backtick run inside.
pub fn block(text: &str) -> String {
    let lines: Vec<String> = text.lines().map(|line| line.replace('\r', "")).collect();
    let text = lines.join("\n");
    let fence = "`".repeat(longest_run(&text).max(2) + 1);
    format!("{fence}text\n{text}\n{fence}\n")
}

fn longest_run(text: &str) -> usize {
    text.split(|c: char| c != '`')
        .map(str::len)
        .max()
        .unwrap_or(0)
}

/// An image's alt text: no brackets, no control characters.
fn alt_text(text: &str) -> String {
    let alt = plain(text);
    if alt.trim().is_empty() {
        "image".to_owned()
    } else {
        alt
    }
}

/// A relative path as a Markdown link target: what would end or break it percent-encoded.
fn link_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '(' => out.push_str("%28"),
            ')' => out.push_str("%29"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '[' => out.push_str("%5B"),
            ']' => out.push_str("%5D"),
            '"' => out.push_str("%22"),
            '\\' => out.push('/'),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests;
