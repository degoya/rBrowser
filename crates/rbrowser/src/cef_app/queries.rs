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
        let known = KnownBrowsers {
            shell: SHELL_BROWSER.load(Ordering::Acquire),
            pages: settings_window::browser_id(),
            palette: page_windows::browser_id(PageWindow::Palette),
            panels: page_windows::browser_id(PageWindow::Panels),
            panels_dock: super::panel_dock::browser_id(),
            history: page_windows::browser_id(PageWindow::History),
            status_bar: super::status_bar::browser_id(),
            board: canvas_board::browser_id(),
            gallery: super::gallery_window::browser_id(),
        };
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
