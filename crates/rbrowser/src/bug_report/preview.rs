//! The look at a bug report's texts before it is saved (RB-093-04, review condition "privacy"):
//! with `report.preview` on (the person's setting, never MCP's) a gathered and masked report
//! opens the pages window at [`ROUTE`] (`ui/src/bugreport/`), which shows `report.md` as it would
//! be saved with Save and Cancel; [`ask`] waits for the word - a cancel, the window closed, a
//! newer report or [`PREVIEW_TIMEOUT`] drop it. Only the pages window decides
//! ([`is_decision_request`], `settings_window::guard`). One report waits at a time.
//!
//! The status bar's "Copy Markdown" asks here too (`getMarkdown`): the core has no clipboard, so
//! the bar copies the masked text itself. Only the reports of this run are known, by their
//! folder's name ([`Saved`]); never a path the shell names.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use rb_ipc::{
    BugReportAnswer, BugReportRequest, CoreToShell, IpcErrorCode, ReportPreview, ShellToCore,
    parse_request,
};

use crate::cdp::lock;
use crate::control::Core;
use crate::ipc::Failure;

/// The pages window's route of the preview.
pub const ROUTE: &str = "bug-report";
/// How long a preview waits for the person before the report is dropped.
pub const PREVIEW_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// How many saved reports' texts "Copy Markdown" can still copy.
pub const MAX_SAVED: usize = 8;
/// The refusal of a copy whose report this run no longer knows.
pub const GONE_KEY: &str = "report.error.copy_gone";

/// What the asker hears: `true` to save.
pub type Decided = Box<dyn FnOnce(bool) + Send>;

/// A report to show: everything of [`ReportPreview`] but its number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    /// The report's folder.
    pub folder: String,
    /// `report.md` as it would be saved.
    pub markdown: String,
    /// The screenshots taken.
    pub images: u32,
    /// `report.annotate`.
    pub annotate: bool,
}

/// The preview waiting for the person, at most one.
#[derive(Default)]
pub struct Previews {
    next: u32,
    open: Option<(ReportPreview, Decided)>,
}

impl Previews {
    /// Opens a preview of `draft`; `done` hears the word once. One waiting before is cancelled.
    pub fn ask(&mut self, draft: Draft, done: Decided) -> ReportPreview {
        self.forget(None);
        self.next = self.next.wrapping_add(1).max(1);
        let preview = ReportPreview {
            preview_id: self.next,
            folder: draft.folder,
            markdown: draft.markdown,
            images: draft.images,
            annotate: draft.annotate,
        };
        self.open = Some((preview.clone(), done));
        preview
    }

    /// The preview waiting, for the page.
    #[must_use]
    pub fn current(&self) -> Option<ReportPreview> {
        self.open.as_ref().map(|(preview, _)| preview.clone())
    }

    /// Takes preview `preview_id` to answer it; `None` when it is not the one waiting.
    pub fn decide(&mut self, preview_id: u32) -> Option<Decided> {
        match &self.open {
            Some((preview, _)) if preview.preview_id == preview_id => {
                self.open.take().map(|(_, done)| done)
            }
            _ => None,
        }
    }

    /// Cancels preview `preview_id`, or whichever waits for `None`.
    pub fn forget(&mut self, preview_id: Option<u32>) {
        let waiting = self.open.as_ref().map(|(preview, _)| preview.preview_id);
        if waiting.is_none() || (preview_id.is_some() && preview_id != waiting) {
            return;
        }
        if let Some((_, done)) = self.open.take() {
            done(false);
        }
    }
}

/// The masked `report.md` of the newest saved reports, by their folder's name.
#[derive(Default)]
pub struct Saved {
    items: VecDeque<(String, String)>,
}

impl Saved {
    /// Keeps `text` as report `report`'s; the oldest goes past [`MAX_SAVED`].
    pub fn remember(&mut self, report: String, text: String) {
        self.items.retain(|(name, _)| *name != report);
        self.items.push_back((report, text));
        while self.items.len() > MAX_SAVED {
            self.items.pop_front();
        }
    }

    /// The text of report `report`, when this run saved it and still keeps it.
    #[must_use]
    pub fn text_of(&self, report: &str) -> Option<&str> {
        self.items
            .iter()
            .find(|(name, _)| name == report)
            .map(|(_, text)| text.as_str())
    }
}

/// One window per process: the waiting preview and the saved texts live beside the core.
static PREVIEWS: Mutex<Previews> = Mutex::new(Previews {
    next: 0,
    open: None,
});
static SAVED: Mutex<Saved> = Mutex::new(Saved {
    items: VecDeque::new(),
});

/// Shows `draft` in the pages window and waits for the person: `true` to save. Cancelled,
/// closed, replaced or after [`PREVIEW_TIMEOUT`]: `false`.
pub async fn ask(core: &Core, draft: Draft) -> bool {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let done: Decided = Box::new(move |save| {
        let _ = sender.send(save);
    });
    let preview_id = lock(&PREVIEWS).ask(draft, done).preview_id;
    tracing::info!(preview_id, "a bug report waits for the person's word");
    core.ui()
        .open_settings_window(crate::shell::page_url(ROUTE));
    let save = matches!(
        tokio::time::timeout(PREVIEW_TIMEOUT, receiver).await,
        Ok(Ok(true))
    );
    if !save {
        lock(&PREVIEWS).forget(Some(preview_id));
    }
    save
}

/// Keeps the masked `text` of the saved report `report` for "Copy Markdown".
pub fn remember(report: String, text: String) {
    lock(&SAVED).remember(report, text);
}

/// The pages window closed: a preview waiting in it is cancelled.
pub fn pages_window_closed() {
    lock(&PREVIEWS).forget(None);
}

/// Whether `request` is a decision on a preview (`bugReport`, `decide`): the pages window's only.
pub fn is_decision_request(request: &str) -> bool {
    matches!(
        parse_request(request),
        Ok(ShellToCore::BugReport {
            request: BugReportRequest::Decide { .. }
        })
    )
}

/// Answers `message` when it is this module's (`bugReport`); `None` otherwise.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let ShellToCore::BugReport { request } = message else {
        return None;
    };
    let answer = |answer: BugReportAnswer| CoreToShell::BugReport { answer };
    Some(match request {
        BugReportRequest::GetPreview => Ok(lock(&PREVIEWS)
            .current()
            .map_or(CoreToShell::Ack, |preview| {
                answer(BugReportAnswer::Preview(preview))
            })),
        BugReportRequest::Decide { preview_id, save } => {
            let done = lock(&PREVIEWS).decide(*preview_id).ok_or_else(|| {
                (
                    IpcErrorCode::NotFound,
                    format!("no bug report preview {preview_id} waits"),
                )
            });
            done.map(|done| {
                done(*save);
                core.ui().close_settings_window();
                CoreToShell::Ack
            })
        }
        BugReportRequest::GetMarkdown { report } => lock(&SAVED)
            .text_of(report)
            .map(|text| {
                answer(BugReportAnswer::Markdown {
                    report: report.clone(),
                    text: text.to_owned(),
                })
            })
            .ok_or_else(|| (IpcErrorCode::NotFound, rb_i18n::t(GONE_KEY))),
    })
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod tests;
