//! The render views' browsers (RB-090-83, `crate::sitemap::render_views`): windowless
//! (off-screen) Chromium browsers, so a sitemap capture's or an environment comparison's pages
//! get a compositor and `Page.captureScreenshot` its frames - the hidden CDP targets before had
//! none. Each is made at its marker URL in the request context of its session, sized to its
//! breakpoint's viewport ([`RenderSize`]); the frames CEF paints (`OnPaint`) are dropped, the
//! render handler's default. It is never in the grid and gets no input: its client has no
//! keyboard handler, no message router and no page bridge, refuses every popup and keeps the
//! shell's pages out (`super::popup::PopupRequests`). `windowless_rendering_enabled` is set in
//! the CEF settings (`super::run`). UI thread.

use std::cell::RefCell;
use std::collections::HashMap;
use std::os::raw::c_int;

use cef::*;

use crate::sitemap::render_views::{RenderView, RenderViewMade};

/// The background a page without one of its own is painted on: opaque white, as in a window
/// (an off-screen browser paints transparent otherwise, and the PNG would be).
const OPAQUE_WHITE: u32 = 0xFFFF_FFFF;

thread_local! {
    /// The browser of each open render view, by its marker URL.
    static BROWSERS: RefCell<HashMap<String, Browser>> = RefCell::new(HashMap::new());
}

/// Makes the windowless browser of `view` and calls `made`.
pub fn open(view: RenderView, made: RenderViewMade) {
    let mut context = crate::sessions::cef::request_context(&view.session);
    // No parent window: there is no window whose close notification this browser could send.
    let window_info = WindowInfo::default().set_as_windowless(no_parent());
    let settings = BrowserSettings {
        background_color: OPAQUE_WHITE,
        ..Default::default()
    };
    let mut client = RenderClient::new(side(view.width), side(view.height));
    let browser = browser_host_create_browser_sync(
        Some(&window_info),
        Some(&mut client),
        Some(&CefString::from(view.marker.as_str())),
        Some(&settings),
        None,
        context.as_mut(),
    );
    match browser {
        Some(browser) => {
            BROWSERS.with(|browsers| browsers.borrow_mut().insert(view.marker, browser));
            made(Ok(()));
        }
        None => {
            tracing::warn!(
                session = %view.session,
                "a render view's windowless browser could not be created"
            );
            made(Err(
                "the render view's windowless browser could not be created".to_owned(),
            ));
        }
    }
}

/// Closes the browser that started at `marker`.
pub fn close(marker: &str) {
    let browser = BROWSERS.with(|browsers| browsers.borrow_mut().remove(marker));
    if let Some(host) = browser.and_then(|browser| browser.host()) {
        host.close_browser(1);
    }
}

/// Closes every render view and lets go of its browser (`super::teardown`): none may outlive
/// CefShutdown.
pub fn release() {
    let browsers: Vec<Browser> = BROWSERS.with(|browsers| {
        browsers
            .borrow_mut()
            .drain()
            .map(|(_, browser)| browser)
            .collect()
    });
    for browser in browsers {
        if let Some(host) = browser.host() {
            host.close_browser(1);
        }
    }
}

/// No window handle: a null `HWND` on Windows, a null `NSView` on macOS.
#[cfg(windows)]
fn no_parent() -> sys::cef_window_handle_t {
    sys::cef_window_handle_t::default()
}

/// No window handle: a null `HWND` on Windows, a null `NSView` on macOS.
#[cfg(not(windows))]
fn no_parent() -> sys::cef_window_handle_t {
    std::ptr::null_mut()
}

/// A side in DIP as CEF takes it: at least 1.
fn side(dip: u32) -> c_int {
    c_int::try_from(dip.max(1)).unwrap_or(c_int::MAX)
}

wrap_client! {
    // The render view's client: its size, the guard against the shell's pages, no popups.
    struct RenderClient {
        width: c_int,
        height: c_int,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(RenderSize::new(self.width, self.height))
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(super::popup::PopupRequests::new())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(RenderLifeSpan::new())
        }
    }
}

wrap_render_handler! {
    // The breakpoint's viewport as the view's rectangle; `OnPaint` keeps its default (the frame
    // is dropped - the screenshot comes over CDP).
    struct RenderSize {
        width: c_int,
        height: c_int,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(rect) = rect {
                *rect = Rect {
                    x: 0,
                    y: 0,
                    width: self.width,
                    height: self.height,
                };
            }
        }
    }
}

wrap_life_span_handler! {
    struct RenderLifeSpan {}

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
            // A page captured opens no window.
            1
        }

        // A windowless browser has no window of its own and none above it (no parent): the
        // close goes on to OnBeforeClose, it never reaches the main window.
        fn do_close(&self, _browser: Option<&mut Browser>) -> c_int {
            0
        }
    }
}
