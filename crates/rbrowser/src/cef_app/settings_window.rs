//! The pages window (RB-010-08, contract 0.1-w1b): a top-level window of its own with the
//! shell's pages (`rbrowser://ui/settings.html`: the settings, the trust dialog, the analyser's
//! wizard), so nothing larger than the toolbar ever covers the grid. It gets the core's events
//! like the shell, may query the core like the shell (`queries.rs`, [`browser_id`]), stays on
//! the shell's pages, opens no popups and has no rBrowser shortcuts (the shortcut editor
//! records keys there). UI thread.

use std::cell::RefCell;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicI32, Ordering};

use cef::wrapper::message_router::MessageRouterBrowserSideHandlerCallbacks;
use cef::*;

use super::views::router;
use crate::shell::{PAGES_URL, PageOwner, navigation_refused};

/// The pages window's browser id; 0 while it is closed.
static PAGES_BROWSER: AtomicI32 = AtomicI32::new(0);

/// The window's first size in DIP.
const SIZE: Size = Size {
    width: 1080,
    height: 760,
};

#[derive(Default)]
struct Pages {
    window: Option<Window>,
    view: Option<BrowserView>,
}

thread_local! {
    static PAGES: RefCell<Pages> = RefCell::new(Pages::default());
}

/// The pages window's browser id, 0 while it is closed (any thread).
pub fn browser_id() -> i32 {
    PAGES_BROWSER.load(Ordering::Acquire)
}

/// The pages window's browser, while it is open.
pub fn browser() -> Option<Browser> {
    PAGES.with(|pages| pages.borrow().view.as_ref().and_then(BrowserView::browser))
}

/// Opens the pages window at `url`, or brings it to the front and loads `url` there (a route
/// change only, when the page is loaded already).
pub fn open(url: &str) {
    let open = PAGES.with(|pages| pages.borrow().window.clone());
    if let Some(window) = open {
        if let Some(frame) = browser().and_then(|browser| browser.main_frame()) {
            frame.load_url(Some(&CefString::from(url)));
        }
        window.show();
        window.activate();
        return;
    }
    let mut client = PagesClient::new();
    let mut delegate = PagesBrowser::new();
    let view = browser_view_create(
        Some(&mut client),
        Some(&CefString::from(url)),
        Some(&BrowserSettings::default()),
        None,
        None,
        Some(&mut delegate),
    );
    if view.is_none() {
        tracing::error!("the settings window's view could not be created");
        return;
    }
    PAGES.with(|pages| pages.borrow_mut().view = view);
    let mut window = PagesWindow::new(rb_i18n::t("settings.window.title"));
    let window = window_create_top_level(Some(&mut window));
    PAGES.with(|pages| pages.borrow_mut().window = window);
}

/// Runs `script` (a core -> shell event) in the pages window's page, when it is open.
pub fn send(script: &str) {
    if let Some(frame) = browser().and_then(|browser| browser.main_frame()) {
        frame.execute_java_script(
            Some(&CefString::from(script)),
            Some(&CefString::from(PAGES_URL)),
            0,
        );
    }
}

/// Closes the pages window (the main window closes, the app ends).
pub fn close() {
    if let Some(window) = PAGES.with(|pages| pages.borrow().window.clone()) {
        window.close();
    }
}

wrap_window_delegate! {
    struct PagesWindow {
        title: String,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            SIZE
        }
    }

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            let Some(window) = window else {
                return;
            };
            window.set_to_fill_layout();
            if let Some(view) = PAGES.with(|pages| pages.borrow().view.clone()) {
                window.add_child_view(Some(&mut View::from(&view)));
            }
            window.set_title(Some(&CefString::from(self.title.as_str())));
            super::icon::apply(window);
            super::theme::adopt(window);
            window.center_window(Some(&SIZE));
            window.show();
        }

        // Windows switched its theme and CEF reset the colours: rBrowser's again (RB-080-95).
        fn on_theme_colors_changed(
            &self,
            window: Option<&mut Window>,
            _chrome_theme: ::std::os::raw::c_int,
        ) {
            if let Some(window) = window {
                super::theme::colors_changed(window);
            }
        }

        fn on_window_destroyed(&self, _window: Option<&mut Window>) {
            PAGES_BROWSER.store(0, Ordering::Release);
            PAGES.with(|pages| *pages.borrow_mut() = Pages::default());
        }

        fn can_close(&self, _window: Option<&mut Window>) -> i32 {
            1
        }

        // The cef crate's defaults are 0 (CEF's own are true).
        fn can_resize(&self, _window: Option<&mut Window>) -> i32 {
            1
        }

        fn can_maximize(&self, _window: Option<&mut Window>) -> i32 {
            1
        }

        fn can_minimize(&self, _window: Option<&mut Window>) -> i32 {
            1
        }

        fn window_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

wrap_browser_view_delegate! {
    // Alloy style like every view of the app (ADR 0001, Decision 2).
    struct PagesBrowser {}

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn on_browser_created(
            &self,
            _browser_view: Option<&mut BrowserView>,
            browser: Option<&mut Browser>,
        ) {
            if let Some(browser) = browser {
                PAGES_BROWSER.store(browser.identifier(), Ordering::Release);
            }
        }

        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

wrap_client! {
    // The pages window's handlers: the message router, the navigation guard, no popups. The page
    // windows (`page_windows.rs`) use them as well, the status bar (`status_bar.rs`) with the
    // shell's keys.
    pub(super) struct PagesClient {}

    impl Client {
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(super::console::ShellConsole::new())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(PagesLifeSpan::new())
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(PagesRequests::new())
        }

        fn on_process_message_received(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            source_process: ProcessId,
            message: Option<&mut ProcessMessage>,
        ) -> c_int {
            c_int::from(router().on_process_message_received(
                browser.map(|browser| browser.clone()),
                frame.map(|frame| frame.clone()),
                source_process,
                message.map(|message| message.clone()),
            ))
        }
    }
}

wrap_life_span_handler! {
    pub(super) struct PagesLifeSpan {}

    impl LifeSpanHandler {
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: c_int,
            _target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut c_int>,
        ) -> c_int {
            // The pages open no windows.
            1
        }

        fn on_before_close(&self, browser: Option<&mut Browser>) {
            // Only the window whose browser closed stops querying; the client is shared.
            let id = browser.as_ref().map_or(0, |browser| browser.identifier());
            let _ = PAGES_BROWSER.compare_exchange(id, 0, Ordering::AcqRel, Ordering::Acquire);
            super::page_windows::forget(id);
            super::panel_dock::forget(id);
            router().on_before_close(browser.map(|browser| browser.clone()));
        }
    }
}

wrap_request_handler! {
    pub(super) struct PagesRequests {}

    impl RequestHandler {
        fn on_before_browse(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _user_gesture: c_int,
            _is_redirect: c_int,
        ) -> c_int {
            let url = request
                .map(|request| CefString::from(&request.url()).to_string())
                .unwrap_or_default();
            let main = frame.as_ref().is_some_and(|frame| frame.is_main() != 0);
            // The pages stay on the shell's pages, like the shell view.
            if navigation_refused(PageOwner::Shell, main, &url) {
                tracing::warn!("a navigation of the settings window off its pages was refused");
                return 1;
            }
            router().on_before_browse(
                browser.map(|browser| browser.clone()),
                frame.map(|frame| frame.clone()),
            );
            0
        }

        fn on_render_process_terminated(
            &self,
            browser: Option<&mut Browser>,
            _status: TerminationStatus,
            _error_code: c_int,
            _error_string: Option<&CefString>,
        ) {
            router().on_render_process_terminated(browser.map(|browser| browser.clone()));
        }
    }
}
