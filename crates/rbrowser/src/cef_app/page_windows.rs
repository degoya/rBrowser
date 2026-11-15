//! The page windows (`crate::page_window`): the findings window (RB-060-01), the command
//! palette's popup (RB-060-11) and the message history's popup on the status bar (RB-080-15). Like the pages window (`settings_window.rs`) they show the shell's
//! pages, query the core under the same checks (`queries.rs`: a known browser, its main frame,
//! a page of `rbrowser://ui/`), stay on the shell's pages and open no popups - they use the pages
//! window's client. Neither covers the toolbar strip nor resizes the grid.
//!
//! The palette is a frameless menu window of the main window (`get_parent_window` with
//! `is_menu`, which may take the keyboard), right below the toolbar. It hides when it loses the
//! focus, and when it ran or was left with Escape (`dismissPalette`), and is shown again for the
//! next open, so it opens without a page load. UI thread.

use std::cell::RefCell;
use std::collections::HashMap;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicI32, Ordering};

use cef::*;

use super::settings_window::PagesClient;
use super::views::UI;
use crate::page_window::{
    Generations, HISTORY_SIZE, PALETTE_SIZE, PANELS_SIZE, PageWindow, ScreenRect, history_bounds,
    palette_bounds,
};
use crate::shell::PAGES_URL;

/// The palette's browser id; 0 while it is closed.
static PALETTE_BROWSER: AtomicI32 = AtomicI32::new(0);
/// The plugin panels window's browser id; 0 while it is closed (RB-060-12).
static PANELS_BROWSER: AtomicI32 = AtomicI32::new(0);
/// The message history's browser id; 0 while it is closed (RB-080-15).
static HISTORY_BROWSER: AtomicI32 = AtomicI32::new(0);

#[derive(Default)]
struct Slot {
    window: Option<Window>,
    view: Option<BrowserView>,
}

thread_local! {
    static SLOTS: RefCell<HashMap<PageWindow, Slot>> = RefCell::new(HashMap::new());
    /// The current window of each kind: a window that closes lets go of its slot at once, and
    /// its late end spares the window opened after it (RB-080-57).
    static GENERATIONS: RefCell<Generations<PageWindow>> = RefCell::new(Generations::default());
}

fn is_current(kind: PageWindow, generation: u64) -> bool {
    GENERATIONS.with(|generations| generations.borrow().is_current(kind, generation))
}

fn id_of(window: PageWindow) -> &'static AtomicI32 {
    match window {
        PageWindow::Palette => &PALETTE_BROWSER,
        PageWindow::Panels => &PANELS_BROWSER,
        PageWindow::History => &HISTORY_BROWSER,
    }
}

/// `window`'s browser id, 0 while it is closed (any thread).
pub fn browser_id(window: PageWindow) -> i32 {
    id_of(window).load(Ordering::Acquire)
}

/// The browser `browser` closed: whichever page window it was no longer queries.
pub fn forget(browser: i32) {
    for window in PageWindow::ALL {
        // Only the window whose id it is; another one's stays.
        let _ = id_of(window).compare_exchange(browser, 0, Ordering::AcqRel, Ordering::Acquire);
    }
}

fn window_of(kind: PageWindow) -> Option<Window> {
    SLOTS.with(|slots| {
        slots
            .borrow()
            .get(&kind)
            .and_then(|slot| slot.window.clone())
    })
}

fn view_of(kind: PageWindow) -> Option<BrowserView> {
    SLOTS.with(|slots| slots.borrow().get(&kind).and_then(|slot| slot.view.clone()))
}

fn main_window() -> Option<Window> {
    UI.with(|ui| ui.borrow().window.clone())
}

/// A popup's place in screen DIP: the palette's below the main window's toolbar, the message
/// history's on the status bar.
fn popup_bounds(kind: PageWindow) -> Option<Rect> {
    let client = main_window()?.client_area_bounds_in_screen();
    let client = ScreenRect {
        x: client.x,
        y: client.y,
        width: client.width,
        height: client.height,
    };
    // Below the toolbar, two rows high while it wraps (RB-080-52).
    let shell_height = crate::toolbar_rows::height();
    let bounds = match kind {
        PageWindow::History => history_bounds(client, shell_height, super::status_bar::height()),
        _ => palette_bounds(client, shell_height),
    };
    Some(Rect {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
    })
}

/// Opens `kind` at `url`, or shows it, brings it to the front and focuses its page. An open
/// panels window keeps its page (its panels, the findings among them, RB-080-69) and hears the
/// `select` event instead; the palette starts afresh on the `uiCommand` the core sends with the
/// open.
pub fn open(kind: PageWindow, url: &str) {
    if let Some(window) = window_of(kind) {
        if kind.is_popup()
            && let Some(bounds) = popup_bounds(kind)
        {
            window.set_bounds(Some(&bounds));
        }
        window.show();
        window.activate();
        if let Some(view) = view_of(kind) {
            view.request_focus();
        }
        return;
    }
    let generation = GENERATIONS.with(|generations| generations.borrow_mut().open(kind));
    let mut client = PagesClient::new();
    let mut delegate = PageBrowser::new(kind, generation);
    let view = browser_view_create(
        Some(&mut client),
        Some(&CefString::from(url)),
        Some(&BrowserSettings::default()),
        None,
        None,
        Some(&mut delegate),
    );
    if view.is_none() {
        tracing::error!(?kind, "a page window's view could not be created");
        return;
    }
    SLOTS.with(|slots| slots.borrow_mut().entry(kind).or_default().view = view);
    let mut delegate = PageWindowDelegate::new(kind, rb_i18n::t(kind.title_key()), generation);
    let window = window_create_top_level(Some(&mut delegate));
    SLOTS.with(|slots| slots.borrow_mut().entry(kind).or_default().window = window);
}

/// The toolbar's height changed (RB-080-52): every popup stands where it would open now - the
/// palette below the toolbar's last row - whether it is shown or waits hidden.
pub fn reposition() {
    for kind in PageWindow::ALL {
        if kind.is_popup()
            && let (Some(window), Some(bounds)) = (window_of(kind), popup_bounds(kind))
        {
            window.set_bounds(Some(&bounds));
        }
    }
}

/// Hides `kind` and gives the main window the focus back (the palette ran or was left).
pub fn hide(kind: PageWindow) {
    let Some(window) = window_of(kind) else {
        return;
    };
    window.hide();
    if let Some(main) = main_window() {
        main.activate();
    }
}

/// Runs `script` (a core -> shell event) in every open page window.
pub fn send(script: &str) {
    for kind in PageWindow::ALL {
        send_to(kind, script);
    }
}

/// Runs `script` (a core -> shell event) in the page window `kind`, when it is open.
pub fn send_to(kind: PageWindow, script: &str) {
    let frame = view_of(kind)
        .and_then(|view| view.browser())
        .and_then(|browser| browser.main_frame());
    if let Some(frame) = frame {
        frame.execute_java_script(
            Some(&CefString::from(script)),
            Some(&CefString::from(PAGES_URL)),
            0,
        );
    }
}

/// Closes the page window `kind`, when it is open (the plugin panels docked, RB-080-57). It
/// lets go of its slot and its query rights at once: the window closes later, and an open in
/// between makes a new one instead of showing the closing one.
pub fn close(kind: PageWindow) {
    GENERATIONS.with(|generations| generations.borrow_mut().close(kind));
    let slot = SLOTS.with(|slots| slots.borrow_mut().remove(&kind));
    id_of(kind).store(0, Ordering::Release);
    if let Some(window) = slot.and_then(|slot| slot.window) {
        window.close();
    }
}

/// Closes every page window (the main window closes, the app ends).
pub fn close_all() {
    for kind in PageWindow::ALL {
        if let Some(window) = window_of(kind) {
            window.close();
        }
    }
}

fn size_of(kind: PageWindow) -> Size {
    let (width, height) = match kind {
        PageWindow::Palette => PALETTE_SIZE,
        PageWindow::Panels => PANELS_SIZE,
        PageWindow::History => HISTORY_SIZE,
    };
    Size { width, height }
}

wrap_window_delegate! {
    struct PageWindowDelegate {
        kind: PageWindow,
        title: String,
        generation: u64,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            size_of(self.kind)
        }
    }

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            let Some(window) = window else {
                return;
            };
            window.set_to_fill_layout();
            let view = view_of(self.kind);
            if let Some(view) = &view {
                window.add_child_view(Some(&mut View::from(view)));
            }
            window.set_title(Some(&CefString::from(self.title.as_str())));
            super::icon::apply(window);
            super::theme::adopt(window);
            if !self.kind.is_popup() {
                window.center_window(Some(&size_of(self.kind)));
            }
            window.show();
            if let Some(view) = view {
                view.request_focus();
            }
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
            // A window closed by `close` is no longer current: the one opened since stays.
            let current = GENERATIONS.with(|generations| {
                generations.borrow_mut().destroyed(self.kind, self.generation)
            });
            if !current {
                return;
            }
            id_of(self.kind).store(0, Ordering::Release);
            SLOTS.with(|slots| slots.borrow_mut().remove(&self.kind));
            // Closed by hand: a changed `panels.dock` no longer docks it.
            if self.kind == PageWindow::Panels
                && let Some(core) = super::views::core()
            {
                crate::panel_dock::on_window_closed(core);
            }
        }

        // The cef crate's defaults are 0 (CEF's own are true); the palette popup keeps its size.
        fn can_resize(&self, _window: Option<&mut Window>) -> c_int {
            c_int::from(self.kind.resizable())
        }

        fn can_maximize(&self, _window: Option<&mut Window>) -> c_int {
            c_int::from(self.kind.resizable())
        }

        fn can_minimize(&self, _window: Option<&mut Window>) -> c_int {
            c_int::from(self.kind.resizable())
        }

        fn on_window_activation_changed(&self, window: Option<&mut Window>, active: c_int) {
            // The palette goes as soon as anything else has the focus.
            if self.kind.is_popup()
                && active == 0
                && let Some(window) = window
            {
                window.hide();
            }
        }

        fn parent_window(
            &self,
            _window: Option<&mut Window>,
            is_menu: Option<&mut c_int>,
            can_activate_menu: Option<&mut c_int>,
        ) -> Option<Window> {
            if !self.kind.is_popup() {
                return None;
            }
            // A menu of the main window that takes the keyboard (the palette's field).
            if let Some(is_menu) = is_menu {
                *is_menu = 1;
            }
            if let Some(can_activate_menu) = can_activate_menu {
                *can_activate_menu = 1;
            }
            main_window()
        }

        fn initial_bounds(&self, _window: Option<&mut Window>) -> Rect {
            if self.kind.is_popup() {
                popup_bounds(self.kind).unwrap_or_default()
            } else {
                Rect::default()
            }
        }

        fn is_frameless(&self, _window: Option<&mut Window>) -> c_int {
            c_int::from(self.kind.is_popup())
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
    struct PageBrowser {
        kind: PageWindow,
        generation: u64,
    }

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn on_browser_created(
            &self,
            _browser_view: Option<&mut BrowserView>,
            browser: Option<&mut Browser>,
        ) {
            // Only the current window's browser queries; a closing one's never again.
            if let Some(browser) = browser
                && is_current(self.kind, self.generation)
            {
                id_of(self.kind).store(browser.identifier(), Ordering::Release);
            }
        }

        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}
