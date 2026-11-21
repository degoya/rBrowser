//! The sitemap gallery's window (RB-090-83, `crate::sitemap::gallery`): a top-level window of its
//! own, never in the grid's views. One window: the next gallery asked for loads into it and
//! brings it to the front.
//!
//! It shows the gallery page of the shell's pages (RB-090-86,
//! `rbrowser://ui/settings.html#/sitemap-gallery?run=<id>`, Nuxt UI) with the pages window's
//! client (`settings_window.rs`): it stays on the shell's pages, opens no popups and has no
//! rBrowser shortcuts. Its queries are narrowed to reading its run, the theme and the text size
//! and to `app.open_link` ([`crate::settings_window::guard::gallery_may_send`], its browser id
//! [`browser_id`]); its images come from `rbrowser://sitemap/` (`sitemap_scheme.rs`), which
//! serves this window's main frame alone and only the shots of the run it shows ([`shown_run`]).
//! UI thread, but for the reads of the browser id and the run on CEF's IO thread.

use std::cell::RefCell;
use std::os::raw::c_int;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, Ordering};

use cef::*;

use super::settings_window::PagesClient;
use crate::cdp::lock;
use crate::sitemap::gallery::GalleryOpen;

/// The window's first size in DIP.
const SIZE: Size = Size {
    width: 1200,
    height: 860,
};

/// The gallery window's browser id; 0 while it is closed.
static GALLERY_BROWSER: AtomicI32 = AtomicI32::new(0);
/// The run the window shows: the image route serves its shots alone.
static SHOWN: Mutex<Option<String>> = Mutex::new(None);

struct Slot {
    window: Option<Window>,
    view: Option<BrowserView>,
}

thread_local! {
    /// The gallery window, while it stands.
    static GALLERY: RefCell<Option<Slot>> = const { RefCell::new(None) };
}

/// The gallery window's browser id, 0 while it is closed (any thread).
pub fn browser_id() -> i32 {
    GALLERY_BROWSER.load(Ordering::Acquire)
}

/// The run the gallery window shows, while it is open (any thread).
pub fn shown_run() -> Option<String> {
    lock(&SHOWN).clone()
}

/// Shows `gallery`: in the open window, or in a new one.
pub fn open(gallery: GalleryOpen) {
    let shown = GALLERY.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|slot| (slot.window.clone(), slot.view.clone()))
    });
    // First: the image route reads the run for the page's first image.
    *lock(&SHOWN) = Some(gallery.run_id.clone());
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
    let mut client = PagesClient::new();
    let mut delegate = GalleryBrowser::new();
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
        Some(&BrowserSettings::default()),
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

/// Lets go of the window: no browser id, no run, an empty slot.
fn forget() {
    GALLERY_BROWSER.store(0, Ordering::Release);
    lock(&SHOWN).take();
    GALLERY.with(|slot| slot.borrow_mut().take());
}

/// Empties the slot (`super::teardown`): its window and view must not outlive CefShutdown.
pub fn release() {
    forget();
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
            forget();
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
    struct GalleryBrowser {}

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn on_browser_created(
            &self,
            _browser_view: Option<&mut BrowserView>,
            browser: Option<&mut Browser>,
        ) {
            if let Some(browser) = browser {
                GALLERY_BROWSER.store(browser.identifier(), Ordering::Release);
            }
        }

        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}
