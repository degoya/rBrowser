//! The bug report bundle (RB-093-04): one command - `report.create` (`Leader C U`), the page
//! menu's Capture > Bug report, the palette, the MCP tool `bug_report` - writes everything a
//! ticket needs to `.rbrowser/out/reports/<time>/`: a screenshot of each view with the password
//! fields blacked out, the overview, the console errors and warnings and the failed requests of
//! the views (deduplicated), page, environment, breakpoint, size and emulation of each view,
//! engine and rBrowser version - as `report.md` with relative image paths and as `report.zip`.
//! `report.include` picks the parts, `report.annotate` opens the screenshot editor afterwards;
//! the person's `report.folder` puts the report elsewhere, and with `report.preview` on (the
//! default) the person reads the masked texts before anything is saved - Cancel leaves nothing
//! behind. The status message of a saved report opens its folder or copies its Markdown.
//!
//! **Privacy** (review condition "high"): every text passes [`mask::Masker`] - the views' cookie
//! values and emulated header values as known secrets, credential headers in any form,
//! `rb_diag`'s rules, every address's query values, fragment and user info - and the finished
//! Markdown passes it once more. Request and response headers and bodies are never read. The
//! MCP tool answers the folder's path, never the content. Nothing leaves the machine.
//!
//! | Part | Module |
//! | --- | --- |
//! | Masking | [`mask`] |
//! | The problems of the views | [`collect`] |
//! | The state of each view | [`facts`] |
//! | `report.md` | [`markdown`] |
//! | Folder and ZIP | [`bundle`] |
//! | The preview before saving, "Copy Markdown" | [`preview`] |
//! | Gathering from the window | [`run`] |
//! | The commands | [`command`] |

pub mod bundle;
pub mod collect;
pub mod command;
pub mod facts;
pub mod markdown;
pub mod mask;
pub mod preview;
pub mod run;

use rb_config::{BugReportConfig, ReportPart};
use serde_json::Value;

use collect::collect;
use facts::{ViewFacts, emulation_lines};
use markdown::{Image, Report, Text};
use mask::Masker;

/// The command that makes a report.
pub const CREATE: &str = "report.create";
/// The command that opens the last report's folder.
pub const OPEN_FOLDER: &str = "report.open_folder";
/// The MCP tool of [`CREATE`].
pub const TOOL: &str = "bug_report";
/// The longest note kept, in characters.
pub const MAX_NOTE_CHARS: usize = 2000;

/// One view as the window knows it, nothing masked yet.
#[derive(Clone, Debug, PartialEq)]
pub struct RawView {
    /// The view's id.
    pub id: u32,
    /// The cell's label.
    pub label: String,
    /// The breakpoint; none after a free size.
    pub breakpoint: Option<String>,
    /// Viewport width in CSS px.
    pub width: u32,
    /// Viewport height in CSS px.
    pub height: u32,
    /// Device pixel ratio.
    pub dpr: f64,
    /// `chromium`, `firefox` or `webkit`.
    pub engine: String,
    /// The page's address.
    pub url: String,
    /// The page's title.
    pub title: String,
    /// The JSON of its `rb_engine::Emulation`; `null` when unknown.
    pub emulation: Value,
}

/// What a report is made of, nothing masked yet but the images.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gathered {
    /// The folder's name (`ui.fileNameTime`).
    pub created: String,
    /// The rBrowser version.
    pub version: String,
    /// The environment's id and address.
    pub environment: Option<(String, String)>,
    /// The caller's note.
    pub note: Option<String>,
    /// The views, in layout order.
    pub views: Vec<RawView>,
    /// The window's buffers (`crate::plugins::console::current`).
    pub feed: Value,
    /// One image per view, paths relative to the report.
    pub images: Vec<Image>,
    /// The overview.
    pub overview: Option<Image>,
    /// `report.*`.
    pub settings: BugReportConfig,
}

/// The values a report hides wherever they appear: `cookies` (the views' cookie values) and the
/// extra header values of each view's emulation.
pub fn secrets(cookies: Vec<String>, views: &[RawView]) -> Vec<String> {
    let mut all = cookies;
    for view in views {
        all.extend(facts::header_values(&view.emulation));
    }
    all
}

/// The report of `gathered`, every text masked by `masker`, the parts of `report.include` only.
pub fn compose(gathered: &Gathered, masker: &Masker) -> Report {
    let has = |part: ReportPart| gathered.settings.has(part);
    let ids: Vec<u32> = gathered.views.iter().map(|view| view.id).collect();
    let problems = collect(&gathered.feed, &ids, masker);
    let views = gathered
        .views
        .iter()
        .map(|view| ViewFacts {
            id: view.id,
            label: masker.text(&view.label),
            breakpoint: view.breakpoint.as_deref().map(|id| masker.text(id)),
            width: view.width,
            height: view.height,
            dpr: view.dpr,
            engine: view.engine.clone(),
            url: masker.text(&view.url),
            title: masker.text(&view.title),
            emulation: if has(ReportPart::Emulation) {
                emulation_lines(&view.emulation, masker)
            } else {
                Vec::new()
            },
        })
        .collect();
    let masked_image = |image: &Image| Image {
        error: image.error.as_deref().map(|error| masker.text(error)),
        ..image.clone()
    };
    Report {
        created: gathered.created.clone(),
        version: has(ReportPart::Version).then(|| gathered.version.clone()),
        environment: gathered
            .environment
            .as_ref()
            .filter(|_| has(ReportPart::Environment))
            .map(|(id, url)| (masker.text(id), masker.text(url))),
        note: gathered
            .note
            .as_deref()
            .map(str::trim)
            .filter(|note| !note.is_empty())
            .map(|note| masker.text(&note.chars().take(MAX_NOTE_CHARS).collect::<String>())),
        views,
        show_pages: has(ReportPart::Environment),
        show_emulation: has(ReportPart::Emulation),
        show_engines: has(ReportPart::Version),
        images: if has(ReportPart::Screenshots) {
            gathered.images.iter().map(masked_image).collect()
        } else {
            Vec::new()
        },
        overview: gathered
            .overview
            .as_ref()
            .filter(|_| has(ReportPart::Overview))
            .map(masked_image),
        console: has(ReportPart::Console).then(|| problems.console.clone()),
        requests: has(ReportPart::Network).then(|| problems.requests.clone()),
    }
}

/// `report.md` of `report`, masked once more as a whole.
pub fn markdown(report: &Report, masker: &Masker, text: Text) -> String {
    masker.text(&markdown::render(report, text))
}

/// A word of `report.md` from the core catalogue (`report.md.<key>`; the literal prefix lets the
/// i18n lint see the family).
pub fn report_text(key: &str, args: &[(&str, &str)]) -> String {
    rb_i18n::t_with(&format!("report.md.{key}"), args)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
