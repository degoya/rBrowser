//! The render views' browsers (RB-090-83, `crate::sitemap::render_views`): windowless
//! (off-screen) Chromium browsers, so a sitemap capture's or an environment comparison's pages
//! get a compositor and `Page.captureScreenshot` its frames - the hidden CDP targets before had
//! none. Each is made at its marker URL in the request context of its session, sized to its
//! breakpoint's viewport ([`RenderSize`]); the frames CEF paints (`OnPaint`) are dropped once
//! their height is noted. A full-page shot grows the view to the page's height and back
//! ([`resize`], RB-090-88): the size the render handler reports is one [`ViewSize`] per view,
//! `WasResized` makes CEF ask for it again, `Invalidate` paints a new frame, and the first frame
//! that tall answers the resize - the shot copies that frame, no longer the top viewport repeated
//! down the page. It is never in the grid and gets no input: its client has no
//! keyboard handler, no message router and no page bridge, refuses every popup and keeps the
//! shell's pages out (`super::popup::PopupRequests`). Its screen's scale is the breakpoint's
//! pixel ratio (`screen_info`, RB-090-90): the emulation asks the compositor for that ratio, and
//! a surface painted at 1 came out repeated in a shot at 2. `windowless_rendering_enabled` is
//! set in the CEF settings (`super::run`). UI thread.

use std::cell::RefCell;
use std::collections::HashMap;
use std::os::raw::c_int;
use std::sync::Arc;

use cef::*;

use crate::sitemap::render_views::{RenderView, RenderViewMade, ViewSize};

/// The background a page without one of its own is painted on: opaque white, as in a window
/// (an off-screen browser paints transparent otherwise, and the PNG would be).
const OPAQUE_WHITE: u32 = 0xFFFF_FFFF;

/// An open render view: its browser and the size its render handler reports.
struct Open {
    browser: Browser,
    size: Arc<ViewSize>,
}

thread_local! {
    /// Each open render view, by its marker URL.
    static BROWSERS: RefCell<HashMap<String, Open>> = RefCell::new(HashMap::new());
}

/// Makes the windowless browser of `view` and calls `made`.
pub fn open(view: RenderView, made: RenderViewMade) {
    let mut context = if view.session == crate::plugins::consent::PROBE_SESSION {
        // The consent probe's own context; without one, no browser (never the global context).
        let Some(fresh) = crate::sessions::cef::fresh_request_context() else {
            made(Err("no request context for the consent probe".to_owned()));
            return;
        };
        Some(fresh)
    } else {
        crate::sessions::cef::request_context(&view.session)
    };
    // No parent window: there is no window whose close notification this browser could send.
    let window_info = WindowInfo::default().set_as_windowless(no_parent());
    let settings = BrowserSettings {
        background_color: OPAQUE_WHITE,
        ..Default::default()
    };
    let size = Arc::new(ViewSize::new(view.width, view.height));
    let mut client = RenderClient::new(size.clone(), view.scale);
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
            let open = Open { browser, size };
            BROWSERS.with(|browsers| browsers.borrow_mut().insert(view.marker, open));
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

/// Gives the browser that started at `marker` the size `width` x `height` DIP and has it paint
/// a frame; the first frame that tall calls `painted`. A browser that is gone answers why.
pub fn resize(marker: &str, width: u32, height: u32, painted: RenderViewMade) {
    let open = BROWSERS.with(|browsers| {
        browsers
            .borrow()
            .get(marker)
            .map(|open| (open.browser.clone(), open.size.clone()))
    });
    let Some((host, size)) = open.and_then(|(browser, size)| Some((browser.host()?, size))) else {
        painted(Err("the render view is closed".to_owned()));
        return;
    };
    size.resize(width, height, painted);
    // CEF asks the render handler for the view's rectangle again, then paints at that size.
    host.was_resized();
    host.invalidate(PaintElementType::VIEW);
}

/// Closes the browser that started at `marker`.
pub fn close(marker: &str) {
    let open = BROWSERS.with(|browsers| browsers.borrow_mut().remove(marker));
    if let Some(open) = open {
        shut(&open);
    }
}

/// Closes `open`'s browser; a resize still waiting gets no frame.
fn shut(open: &Open) {
    open.size.closed();
    if let Some(host) = open.browser.host() {
        host.close_browser(1);
    }
}

/// Closes every render view and lets go of its browser (`super::teardown`): none may outlive
/// CefShutdown.
pub fn release() {
    let views: Vec<Open> = BROWSERS.with(|browsers| {
        browsers
            .borrow_mut()
            .drain()
            .map(|(_, open)| open)
            .collect()
    });
    for open in &views {
        shut(open);
    }
}

/// No window handle: a null `HWND` on Windows, a null `NSView` on macOS.
#[cfg(windows)]
fn no_parent() -> sys::cef_window_handle_t {
    sys::cef_window_handle_t::default()
}

/// No window handle: a null `HWND` on Windows, a null `NSView` on macOS.
#[cfg(target_os = "macos")]
fn no_parent() -> sys::cef_window_handle_t {
    std::ptr::null_mut()
}

/// No window handle: X11 window 0 on Linux (RB-097-05).
#[cfg(target_os = "linux")]
fn no_parent() -> sys::cef_window_handle_t {
    0
}

/// A side in DIP as CEF takes it: at least 1.
fn side(dip: u32) -> c_int {
    c_int::try_from(dip.max(1)).unwrap_or(c_int::MAX)
}

wrap_client! {
    // The render view's client: its size and scale, the guard against the shell's pages, no
    // popups.
    struct RenderClient {
        size: Arc<ViewSize>,
        scale: f32,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(RenderSize::new(self.size.clone(), self.scale))
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
    // The view's size as its rectangle: the breakpoint's viewport, the page's height during a
    // full-page shot; its scale as the screen's (RB-090-90). A frame is dropped once its height
    // is noted - the screenshot comes over CDP.
    struct RenderSize {
        size: Arc<ViewSize>,
        scale: f32,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(rect) = rect {
                let (width, height) = self.size.get();
                *rect = Rect {
                    x: 0,
                    y: 0,
                    width: side(width),
                    height: side(height),
                };
            }
        }

        // The breakpoint's pixel ratio as the screen's scale: the compositor paints the surface
        // at the ratio the emulation asks for, so a shot at 2 is not the surface at 1 repeated.
        fn screen_info(
            &self,
            _browser: Option<&mut Browser>,
            screen_info: Option<&mut ScreenInfo>,
        ) -> c_int {
            let Some(info) = screen_info else {
                return 0;
            };
            let (width, height) = self.size.get();
            let rect = Rect {
                x: 0,
                y: 0,
                width: side(width),
                height: side(height),
            };
            info.device_scale_factor = self.scale;
            info.depth = 24;
            info.depth_per_component = 8;
            info.is_monochrome = 0;
            info.rect = rect.clone();
            info.available_rect = rect;
            1
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            _buffer: *const u8,
            width: c_int,
            height: c_int,
        ) {
            // Device pixels: `ViewSize` measures them in DIP by the frame's width.
            if type_ == PaintElementType::VIEW {
                let pixels = |side: c_int| u32::try_from(side).unwrap_or(0);
                self.size.painted(pixels(width), pixels(height));
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
