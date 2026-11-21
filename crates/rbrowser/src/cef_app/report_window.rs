//! The report window (RB-091-02): a plugin's HTML report in a top-level window of its own, as
//! [`crate::report_view`] describes. Its client has no message router - the report's origin gets
//! no `cefQuery` anyway (`render.rs`) - no keyboard handler (no rBrowser shortcuts) and no page
//! bridge; it loads only report addresses in its main frame. A link the person clicks, in the
//! page or as a popup, goes to `app.open_link` (http and https only, its throttle and the
//! person's `ui.openLinksIn`); any other navigation and every popup is refused.
//!
//! A window's browser starts empty: once it exists, its id joins the report browsers the report
//! route answers ([`windows::register_browser`]) and only then it loads its report, so the route
//! never has to answer a browser it does not know yet. [`windows::choose`] says where a report
//! goes: the one report window for [`Placement::Single`], a new window for
//! [`Placement::PerReport`] up to [`windows::MAX_WINDOWS`], past that the oldest. A window that
//! closes revokes its report's address and leaves the report browsers. UI thread.

use std::cell::RefCell;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicU64, Ordering};

use cef::*;
use serde_json::json;

use crate::report_view::ReportOpen;
use crate::report_view::ask::Placement;
use crate::report_view::tokens::is_report_url;
use crate::report_view::windows::{self, Open, Target};

/// A report window's first size in DIP.
const SIZE: Size = Size {
    width: 1100,
    height: 820,
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct Slot {
    id: u64,
    single: bool,
    window: Option<Window>,
    view: Option<BrowserView>,
    /// Its browser's id, once created.
    browser: Option<i32>,
    url: String,
}

thread_local! {
    static SLOTS: RefCell<Vec<Slot>> = const { RefCell::new(Vec::new()) };
}

fn text(value: &CefStringUserfree) -> String {
    CefString::from(value).to_string()
}

/// Shows `report` where [`windows::choose`] says.
pub fn open(report: ReportOpen) {
    let open: Vec<Open> = SLOTS.with(|slots| {
        slots
            .borrow()
            .iter()
            .map(|slot| Open {
                id: slot.id,
                single: slot.single,
            })
            .collect()
    });
    if let Target::Reuse(id) = windows::choose(&open, report.placement)
        && show_in(id, &report)
    {
        return;
    }
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let mut client = ReportClient::new();
    let mut delegate = ReportBrowser::new(id);
    // No address yet: the browser loads its report once it is a known report browser.
    let view = browser_view_create(
        Some(&mut client),
        None,
        Some(&BrowserSettings::default()),
        None,
        None,
        Some(&mut delegate),
    );
    if view.is_none() {
        tracing::error!("the report window's view could not be created");
        crate::report_view::revoke(&report.url);
        return;
    }
    SLOTS.with(|slots| {
        slots.borrow_mut().push(Slot {
            id,
            single: report.placement == Placement::Single,
            window: None,
            view,
            browser: None,
            url: report.url.clone(),
        });
    });
    let mut delegate = ReportWindowDelegate::new(id, report.title);
    let window = window_create_top_level(Some(&mut delegate));
    SLOTS.with(|slots| {
        if let Some(slot) = slots.borrow_mut().iter_mut().find(|slot| slot.id == id) {
            slot.window = window;
        }
    });
}

/// Loads `report` into the open window `id`; false when it is gone.
fn show_in(id: u64, report: &ReportOpen) -> bool {
    let found = SLOTS.with(|slots| {
        slots
            .borrow_mut()
            .iter_mut()
            .find(|slot| slot.id == id)
            .map(|slot| {
                let before = std::mem::replace(&mut slot.url, report.url.clone());
                (slot.window.clone(), slot.view.clone(), before)
            })
    });
    let Some((window, view, before)) = found else {
        return false;
    };
    crate::report_view::revoke(&before);
    // A browser not created yet loads the slot's address when it is.
    if let Some(frame) = view
        .as_ref()
        .and_then(|view| view.browser())
        .and_then(|browser| browser.main_frame())
    {
        frame.load_url(Some(&CefString::from(report.url.as_str())));
    }
    if let Some(window) = window {
        window.set_title(Some(&CefString::from(report.title.as_str())));
        window.show();
        window.activate();
    }
    if let Some(view) = view {
        view.request_focus();
    }
    true
}

/// Browser `browser` of window `id` exists: it becomes a report browser, then loads its report.
fn browser_created(id: u64, browser: &Browser) {
    let browser_id = browser.identifier();
    windows::register_browser(browser_id);
    let url = SLOTS.with(|slots| {
        let mut slots = slots.borrow_mut();
        let slot = slots.iter_mut().find(|slot| slot.id == id)?;
        slot.browser = Some(browser_id);
        Some(slot.url.clone())
    });
    if let (Some(url), Some(frame)) = (url, browser.main_frame()) {
        frame.load_url(Some(&CefString::from(url.as_str())));
    }
}

fn view_of(id: u64) -> Option<BrowserView> {
    SLOTS.with(|slots| {
        slots
            .borrow()
            .iter()
            .find(|slot| slot.id == id)
            .and_then(|slot| slot.view.clone())
    })
}

/// Closes every report window (the main window closes, the app ends).
pub fn close_all() {
    let windows: Vec<Window> = SLOTS.with(|slots| {
        slots
            .borrow()
            .iter()
            .filter_map(|slot| slot.window.clone())
            .collect()
    });
    for window in windows {
        window.close();
    }
}

/// Empties the slots (`super::teardown`): their windows and views must not outlive CefShutdown.
pub fn release() {
    SLOTS.with(|slots| slots.borrow_mut().clear());
}

/// A link the person clicked in a report: through `app.open_link`, off the UI thread.
pub(super) fn open_outside(url: String) {
    let Some(core) = super::views::core().cloned() else {
        return;
    };
    let runtime = core.runtime().clone();
    runtime.spawn_blocking(move || {
        if let Err(error) = crate::open_link::open(&core, json!({ "url": url })) {
            tracing::debug!(%error, "a report's link was not opened");
        }
    });
}

wrap_window_delegate! {
    struct ReportWindowDelegate {
        id: u64,
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
            let view = view_of(self.id);
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
            let gone = SLOTS.with(|slots| {
                let mut slots = slots.borrow_mut();
                let at = slots.iter().position(|slot| slot.id == self.id)?;
                Some(slots.remove(at))
            });
            if let Some(slot) = gone {
                crate::report_view::revoke(&slot.url);
                if let Some(browser) = slot.browser {
                    windows::forget_browser(browser);
                }
            }
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

        // macOS draws the close, minimise and zoom buttons only on this answer; CEF's default
        // is "not frameless", the cef crate's 0 - a window without them cannot be closed there
        // (RB-090-98). Ignored on Windows.
        fn with_standard_window_buttons(&self, _window: Option<&mut Window>) -> c_int {
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
    struct ReportBrowser {
        id: u64,
    }

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn on_browser_created(
            &self,
            _browser_view: Option<&mut BrowserView>,
            browser: Option<&mut Browser>,
        ) {
            if let Some(browser) = browser {
                browser_created(self.id, browser);
            }
        }

        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

wrap_client! {
    // No message router, no keyboard handler, no page bridge: the guard on what it loads and
    // the popups it refuses.
    struct ReportClient {}

    impl Client {
        fn request_handler(&self) -> Option<RequestHandler> {
            Some(ReportRequests::new())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(ReportPopups::new())
        }
    }
}

wrap_request_handler! {
    struct ReportRequests {}

    impl RequestHandler {
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
            if main && is_redirect == 0 && is_report_url(&url) {
                return 0;
            }
            if !main && (url == "about:blank" || url == "about:srcdoc") {
                return 0;
            }
            if main && user_gesture != 0 {
                open_outside(url);
            } else {
                tracing::info!(main, "a report's navigation was refused");
            }
            1
        }
    }
}

wrap_life_span_handler! {
    struct ReportPopups {}

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
            // A report opens no window of its own; a clicked link goes to the person's choice.
            if user_gesture != 0
                && let Some(url) = target_url
            {
                open_outside(url.to_string());
            }
            1
        }
    }
}
