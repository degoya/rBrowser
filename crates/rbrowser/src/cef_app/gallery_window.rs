//! The sitemap gallery's window (RB-090-83, `crate::sitemap::gallery`): a run's `index.html` in a
//! top-level window of its own, never in the grid's views. One window: the next gallery asked
//! for loads into it and brings it to the front. Its client has no message router, no keyboard
//! handler (no rBrowser shortcuts) and no page bridge, JavaScript is off (the gallery has none),
//! and every request it makes - the main frame and every image, style or font - reaches only
//! what lies in the run's folder ([`GalleryOpen::may_load`]): a gallery a repository put there
//! cannot reach a share (an NTLM hash), the web or another file (security review, wave
//! 090-w21). A link the person clicks, in the page or as a popup, goes to `app.open_link` (http
//! and https only); anything else is refused. The policy is the gallery's own `<meta>` one -
//! `file:` answers carry no headers to add one to - and JavaScript is off whatever it says. UI
//! thread, but for the requests' check on CEF's IO thread.

use std::cell::RefCell;
use std::os::raw::c_int;
use std::sync::Mutex;

use cef::*;

use crate::cdp::lock;
use crate::sitemap::gallery::GalleryOpen;

/// The window's first size in DIP.
const SIZE: Size = Size {
    width: 1200,
    height: 860,
};

struct Slot {
    window: Option<Window>,
    view: Option<BrowserView>,
}

/// The gallery shown now: what its requests are held to, read on the UI and the IO thread.
static SHOWN: Mutex<Option<GalleryOpen>> = Mutex::new(None);

thread_local! {
    /// The gallery window, while it stands.
    static GALLERY: RefCell<Option<Slot>> = const { RefCell::new(None) };
}

fn text(value: &CefStringUserfree) -> String {
    CefString::from(value).to_string()
}

/// Whether the gallery shown now may load `url`: a navigation or any other request.
fn may_load(url: &str) -> bool {
    lock(&SHOWN)
        .as_ref()
        .is_some_and(|shown| shown.may_load(url))
}

/// Shows `gallery`: in the open window, or in a new one.
pub fn open(gallery: GalleryOpen) {
    let shown = GALLERY.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|slot| (slot.window.clone(), slot.view.clone()))
    });
    // First: the guard reads the gallery's folder for its first request.
    *lock(&SHOWN) = Some(gallery.clone());
    if let Some((window, view)) = shown {
        if let Some(frame) = view
            .as_ref()
            .and_then(|view| view.browser())
            .and_then(|browser| browser.main_frame())
        {
            frame.load_url(Some(&CefString::from(gallery.url.as_str())));
        }
        if let Some(window) = window {
            window.set_title(Some(&CefString::from(gallery.title.as_str())));
            window.show();
            window.activate();
        }
        return;
    }
    let mut client = GalleryClient::new();
    let mut delegate = GalleryBrowser::new();
    let settings = BrowserSettings {
        // The gallery has no script; a file put in its place runs none either.
        javascript: State::DISABLED,
        ..Default::default()
    };
    let title = gallery.title.clone();
    let url = gallery.url.clone();
    GALLERY.with(|slot| {
        *slot.borrow_mut() = Some(Slot {
            window: None,
            view: None,
        });
    });
    let view = browser_view_create(
        Some(&mut client),
        Some(&CefString::from(url.as_str())),
        Some(&settings),
        None,
        None,
        Some(&mut delegate),
    );
    if view.is_none() {
        tracing::error!("the gallery window's view could not be created");
        GALLERY.with(|slot| slot.borrow_mut().take());
        lock(&SHOWN).take();
        return;
    }
    GALLERY.with(|slot| {
        if let Some(slot) = slot.borrow_mut().as_mut() {
            slot.view = view;
        }
    });
    let mut delegate = GalleryWindowDelegate::new(title);
    let window = window_create_top_level(Some(&mut delegate));
    GALLERY.with(|slot| {
        if let Some(slot) = slot.borrow_mut().as_mut() {
            slot.window = window;
        }
    });
}

fn view() -> Option<BrowserView> {
    GALLERY.with(|slot| slot.borrow().as_ref().and_then(|slot| slot.view.clone()))
}

/// Closes the gallery window (the main window closes, the app ends).
pub fn close_all() {
    let window = GALLERY.with(|slot| slot.borrow().as_ref().and_then(|slot| slot.window.clone()));
    if let Some(window) = window {
        window.close();
    }
}

/// Empties the slot (`super::teardown`): its window and view must not outlive CefShutdown.
pub fn release() {
    GALLERY.with(|slot| slot.borrow_mut().take());
    lock(&SHOWN).take();
}

/// A web address the person clicked: through `app.open_link`, which takes http and https only.
fn open_outside(url: String) {
    super::report_window::open_outside(url);
}

wrap_window_delegate! {
    struct GalleryWindowDelegate {
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
            let view = view();
            if let Some(view) = &view {
                window.add_child_view(Some(&mut View::from(view)));
            }
            window.set_title(Some(&CefString::from(self.title.as_str())));
            super::icon::apply(window);
            super::theme::adopt(window);
            window.center_window(Some(&SIZE));
            window.show();
            if let Some(view) = view {
                view.request_focus();
            }
        }

        fn on_theme_colors_changed(&self, window: Option<&mut Window>, _chrome_theme: c_int) {
            if let Some(window) = window {
                super::theme::colors_changed(window);
            }
        }

        fn on_window_destroyed(&self, _window: Option<&mut Window>) {
            GALLERY.with(|slot| slot.borrow_mut().take());
            lock(&SHOWN).take();
        }

        fn can_resize(&self, _window: Option<&mut Window>) -> c_int {
            1
        }

        fn can_maximize(&self, _window: Option<&mut Window>) -> c_int {
            1
        }

        fn can_minimize(&self, _window: Option<&mut Window>) -> c_int {
            1
        }

        fn can_close(&self, _window: Option<&mut Window>) -> c_int {
            1
        }

        fn window_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

wrap_browser_view_delegate! {
    // Alloy style like every view of the app (ADR 0001, Decision 2).
    struct GalleryBrowser {}

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

wrap_client! {
    // No message router, no keyboard handler, no page bridge: the guard on what it loads and
    // the popups it refuses.
    struct GalleryClient {}

    impl Client {
        fn request_handler(&self) -> Option<RequestHandler> {
            Some(GalleryRequests::new())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(GalleryPopups::new())
        }
    }
}

wrap_request_handler! {
    struct GalleryRequests {}

    impl RequestHandler {
        // Every request of the window, not only its navigations (IO thread).
        fn resource_request_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
            _is_navigation: c_int,
            _is_download: c_int,
            _request_initiator: Option<&CefString>,
            _disable_default_handling: Option<&mut c_int>,
        ) -> Option<ResourceRequestHandler> {
            Some(GalleryResources::new())
        }

        fn on_before_browse(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            user_gesture: c_int,
            is_redirect: c_int,
        ) -> c_int {
            let url = request.map(|request| text(&request.url())).unwrap_or_default();
            let main = frame.as_ref().is_some_and(|frame| frame.is_main() != 0);
            if main && is_redirect == 0 && may_load(&url) {
                return 0;
            }
            if main && user_gesture != 0 {
                open_outside(url);
            } else {
                tracing::info!(main, "a gallery's navigation was refused");
            }
            1
        }
    }
}

wrap_resource_request_handler! {
    struct GalleryResources {}

    impl ResourceRequestHandler {
        fn on_before_resource_load(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _callback: Option<&mut Callback>,
        ) -> ReturnValue {
            let url = request.map(|request| text(&request.url())).unwrap_or_default();
            if may_load(&url) {
                ReturnValue::CONTINUE
            } else {
                // The address is the repository's: only that something was refused is logged.
                tracing::info!("a gallery's request outside its folder was refused");
                ReturnValue::CANCEL
            }
        }
    }
}

wrap_life_span_handler! {
    struct GalleryPopups {}

    impl LifeSpanHandler {
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: c_int,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            user_gesture: c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut c_int>,
        ) -> c_int {
            // The gallery opens no window of its own; a clicked link goes to the person's choice.
            if user_gesture != 0
                && let Some(url) = target_url
            {
                open_outside(url.to_string());
            }
            1
        }
    }
}
