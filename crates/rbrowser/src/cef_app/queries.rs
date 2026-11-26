//! The browser side of `cefQuery`: only the shell's own pages reach the core - the shell view,
//! the status bar (RB-080-15), the pages window (RB-010-08), the page windows and the canvas
//! board (RB-080-04, only its own requests) and the sitemap gallery window (RB-090-86, only its
//! own), each from its main frame on `rbrowser://ui/`
//! ([`crate::settings_window::guard`]); any other browser or frame is refused with 403, whatever
//! it sends.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use cef::wrapper::message_router::{
    BrowserSideCallback, BrowserSideHandler, MessageRouterBrowserSide,
};
use cef::*;
use rb_ipc::IpcErrorCode;

use super::views::{SHELL_BROWSER, core, router};
use super::{canvas_board, page_windows, settings_window};
use crate::cdp::lock;
use crate::page_window::PageWindow;
use crate::settings_window::guard::{KnownBrowsers, QueryOrigin, request_allowed};

/// A shell page that may query the core, in its own browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShellPage {
    Shell,
    Pages,
    Palette,
    Panels,
    PanelsDock,
    History,
    StatusBar,
    Board,
    Gallery,
}

#[cfg(test)]
impl ShellPage {
    /// Every page, for the tests.
    const ALL: [ShellPage; 9] = [
        ShellPage::Shell,
        ShellPage::Pages,
        ShellPage::Palette,
        ShellPage::Panels,
        ShellPage::PanelsDock,
        ShellPage::History,
        ShellPage::StatusBar,
        ShellPage::Board,
        ShellPage::Gallery,
    ];
}

/// The browsers allowed to query, each page's id from `id_of` (0 while it has no browser).
fn known_browsers(id_of: impl Fn(ShellPage) -> i32) -> KnownBrowsers {
    KnownBrowsers {
        shell: id_of(ShellPage::Shell),
        pages: id_of(ShellPage::Pages),
        palette: id_of(ShellPage::Palette),
        panels: id_of(ShellPage::Panels),
        panels_dock: id_of(ShellPage::PanelsDock),
        history: id_of(ShellPage::History),
        status_bar: id_of(ShellPage::StatusBar),
        board: id_of(ShellPage::Board),
        gallery: id_of(ShellPage::Gallery),
    }
}

/// The browser of `page` now; 0 while it has none.
fn browser_id_of(page: ShellPage) -> i32 {
    match page {
        ShellPage::Shell => SHELL_BROWSER.load(Ordering::Acquire),
        ShellPage::Pages => settings_window::browser_id(),
        ShellPage::Palette => page_windows::browser_id(PageWindow::Palette),
        ShellPage::Panels => page_windows::browser_id(PageWindow::Panels),
        ShellPage::PanelsDock => super::panel_dock::browser_id(),
        ShellPage::History => page_windows::browser_id(PageWindow::History),
        ShellPage::StatusBar => super::status_bar::browser_id(),
        ShellPage::Board => canvas_board::browser_id(),
        ShellPage::Gallery => super::gallery_window::browser_id(),
    }
}

struct ShellQueries;

impl BrowserSideHandler for ShellQueries {
    fn on_query_str(
        &self,
        browser: Option<Browser>,
        frame: Option<Frame>,
        _query_id: i64,
        request: &str,
        _persistent: bool,
        callback: Arc<Mutex<dyn BrowserSideCallback>>,
    ) -> bool {
        let known = known_browsers(browser_id_of);
        let url = frame
            .as_ref()
            .map(|frame| CefString::from(&frame.url()).to_string())
            .unwrap_or_default();
        let origin = QueryOrigin {
            browser: browser.as_ref().map_or(0, |browser| browser.identifier()),
            main_frame: frame.as_ref().is_some_and(|frame| frame.is_main() != 0),
            url: &url,
        };
        if !request_allowed(origin, known, request) {
            lock(&callback).failure(
                IpcErrorCode::Refused as i32,
                "only the shell may query the core",
            );
            return true;
        }
        let Some(core) = core() else {
            lock(&callback).failure(IpcErrorCode::Internal as i32, "the core is not ready");
            return true;
        };
        // A `runCommand` answers from a worker thread; the callback posts its answer to the UI
        // thread itself.
        crate::ipc::dispatch_to(core, request, move |answer| {
            let callback = lock(&callback);
            match answer {
                Ok(answer) => callback.success_str(&answer),
                Err((code, message)) => callback.failure(code as i32, &message),
            }
        });
        true
    }
}

/// Routes the shell's queries to [`crate::ipc::dispatch`]. UI thread.
pub fn install() {
    if router()
        .add_handler(Arc::new(ShellQueries), false)
        .is_none()
    {
        tracing::error!("the shell query handler could not be installed");
    }
}

#[cfg(test)]
#[path = "queries_tests.rs"]
mod tests;
