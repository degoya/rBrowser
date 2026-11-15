//! The status bar at the bottom of the main window (RB-080-15): a shell page of its own
//! (`status-bar.html`) in a browser view below the content area, [`STATUS_BAR_HEIGHT`] DIP high.
//! It never overlaps a view: the window is a vertical box of toolbar, content and bar, and the
//! bar's delegate holds its height with a size that has no side 0 (`views::strip`). It has the
//! pages' handlers (`settings_window.rs`): it stays on the shell's pages, opens no popups and
//! queries the core under the same checks (`queries.rs`, [`browser_id`]); and the shell's keys,
//! so the leader works while it has the focus. [`send`] brings it the core's events. Hidden
//! (`ui.statusBar.visible`), the box gives its room to the content. UI thread.

use std::cell::RefCell;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicI32, Ordering};

use cef::wrapper::message_router::MessageRouterBrowserSideHandlerCallbacks;
use cef::*;

use super::keys::ViewKeys;
use super::settings_window::{PagesLifeSpan, PagesRequests};
use super::take_focus::TakeFocus;
use super::views::{router, strip};
use crate::host::focus_cycle::FocusStop;
use crate::shell::{STATUS_BAR_HEIGHT, STATUS_BAR_URL};

/// The bar's browser id; 0 until it exists.
static BAR_BROWSER: AtomicI32 = AtomicI32::new(0);

thread_local! {
    /// The bar's view, once the window built it.
    static BAR: RefCell<Option<BrowserView>> = const { RefCell::new(None) };
}

/// The bar's browser id, 0 until it exists (any thread).
pub fn browser_id() -> i32 {
    BAR_BROWSER.load(Ordering::Acquire)
}

fn bar_height() -> i32 {
    i32::try_from(STATUS_BAR_HEIGHT).unwrap_or(24)
}

fn view() -> Option<BrowserView> {
    BAR.with(|bar| bar.borrow().clone())
}

/// The bar's height while it is shown, else 0 (where the message history stands).
pub fn height() -> u32 {
    let shown = view().is_some_and(|view| View::from(&view).is_visible() != 0);
    if shown { STATUS_BAR_HEIGHT } else { 0 }
}

/// The window of the bar and the screen point (DIP) `x`/`y` from the bar's top-left corner
/// names (its zoom presets, RB-090-18); `None` while the bar is hidden or has no window.
pub fn screen_point(x: i32, y: i32) -> Option<(Window, Point)> {
    let view = View::from(&view()?);
    if view.is_visible() == 0 {
        return None;
    }
    let window = view.window()?;
    let bounds = view.bounds_in_screen();
    let point = Point {
        x: bounds.x.saturating_add(x),
        y: bounds.y.saturating_add(y),
    };
    tracing::info!(
        bar_x = bounds.x,
        bar_y = bounds.y,
        bar_width = bounds.width,
        x,
        y,
        "the status bar's point in the screen"
    );
    Some((window, point))
}

/// The bar's view for the window's box, shown as the user settings say.
pub fn create() -> Option<View> {
    let mut client = BarClient::new();
    let mut delegate = BarBrowser::new();
    let bar = browser_view_create(
        Some(&mut client),
        Some(&CefString::from(STATUS_BAR_URL)),
        Some(&BrowserSettings::default()),
        None,
        None,
        Some(&mut delegate),
    )?;
    let visible = super::start::LAUNCH
        .get()
        .is_none_or(|launch| launch.setup.user.ui.status_bar.visible);
    let view = View::from(&bar);
    view.set_visible(c_int::from(visible));
    BAR.with(|slot| *slot.borrow_mut() = Some(bar));
    Some(view)
}

/// Shows or hides the bar; the window's box lays its children out again.
pub fn set_visible(visible: bool) {
    let Some(bar) = view() else {
        return;
    };
    let view = View::from(&bar);
    if (view.is_visible() != 0) == visible {
        return;
    }
    view.set_visible(c_int::from(visible));
    if let Some(root) = view.parent_view().and_then(|parent| parent.as_panel()) {
        root.layout();
    }
}

/// The bar's browser, for the Tab order (`take_focus.rs`, RB-090-23).
pub(super) fn browser() -> Option<Browser> {
    view().and_then(|bar| bar.browser())
}

/// Runs `script` (a core -> shell event) in the bar's page.
pub fn send(script: &str) {
    let frame = view()
        .and_then(|bar| bar.browser())
        .and_then(|browser| browser.main_frame());
    if let Some(frame) = frame {
        frame.execute_java_script(
            Some(&CefString::from(script)),
            Some(&CefString::from(STATUS_BAR_URL)),
            0,
        );
    }
}

wrap_browser_view_delegate! {
    // Alloy style like every view of the app (ADR 0001, Decision 2); a strip of the bar's height
    // whatever the box asks (preferred, minimum and height for a width), like the toolbar.
    struct BarBrowser {}

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            strip(0, bar_height())
        }

        fn minimum_size(&self, _view: Option<&mut View>) -> Size {
            strip(0, bar_height())
        }

        fn height_for_width(&self, _view: Option<&mut View>, _width: c_int) -> c_int {
            bar_height()
        }
    }

    impl BrowserViewDelegate {
        fn on_browser_created(
            &self,
            _browser_view: Option<&mut BrowserView>,
            browser: Option<&mut Browser>,
        ) {
            if let Some(browser) = browser {
                BAR_BROWSER.store(browser.identifier(), Ordering::Release);
            }
        }

        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

wrap_client! {
    // The pages' handlers (router, navigation guard, no popups) and the shell's keys.
    struct BarClient {}

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

        fn keyboard_handler(&self) -> Option<KeyboardHandler> {
            Some(ViewKeys::new(None))
        }

        // Tab past the bar's last control goes on to the toolbar (RB-090-23).
        fn focus_handler(&self) -> Option<FocusHandler> {
            Some(TakeFocus::new(FocusStop::StatusBar))
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

/// Lets the bar's view go (the main window was destroyed, RB-080-91).
pub fn release() {
    BAR.with(|bar| bar.borrow_mut().take());
}
