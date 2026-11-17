//! The canvas board (RB-080-04, `crate::layout::canvas_board`), CEF side: one Alloy browser view
//! per window showing [`BOARD_URL`], laid into the empty grid panel in canvas mode, so it fills
//! the grid area behind the views' overlays. CEF Views panels get no mouse events, hence a
//! browser view, like the DevTools splitter (`splitter.rs`). It queries the core under the shell's
//! checks, narrowed to the board's own requests (`queries.rs`, `settings_window::guard`), gets
//! the core's events like every shell page, stays on the shell's pages, opens no popups and
//! hands its keys to the leader as the shell's. Made once and kept for the window's life; the
//! grid takes it out of the panel for every other mode. A click on the active view's label
//! opens the size quick pick under it, as the grid's strips do (RB-080-22). UI thread.

use std::cell::RefCell;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicI32, Ordering};

use cef::wrapper::message_router::MessageRouterBrowserSideHandlerCallbacks;
use cef::*;
use rb_engine::ViewId;
use rb_ipc::MapRect;

use super::keys::BoardKeys;
use super::views::router;
use crate::layout::canvas_board::BOARD_URL;
use crate::shell::{PageOwner, navigation_refused};

/// The board's browser id; 0 until it exists.
static BOARD_BROWSER: AtomicI32 = AtomicI32::new(0);

thread_local! {
    /// The board, once made.
    static BOARD: RefCell<Option<BrowserView>> = const { RefCell::new(None) };
}

/// The board's browser id, 0 before it exists (any thread).
pub fn browser_id() -> i32 {
    BOARD_BROWSER.load(Ordering::Acquire)
}

/// Lays the board into `panel` (the grid panel, empty in canvas mode), filling it; made on the
/// first call.
pub fn attach(panel: &Panel) {
    let board = BOARD.with(|board| {
        let mut board = board.borrow_mut();
        if board.is_none() {
            *board = create();
        }
        board.clone()
    });
    let Some(board) = board else {
        tracing::error!("the canvas board could not be created");
        return;
    };
    let mut view = View::from(&board);
    super::grid::detach(&view);
    panel.set_to_fill_layout();
    panel.add_child_view(Some(&mut view));
}

/// Runs `script` (a core -> shell event) in the board's page, once it exists.
pub fn send(script: &str) {
    let frame = BOARD.with(|board| {
        board
            .borrow()
            .as_ref()
            .and_then(BrowserView::browser)
            .and_then(|browser| browser.main_frame())
    });
    if let Some(frame) = frame {
        frame.execute_java_script(
            Some(&CefString::from(script)),
            Some(&CefString::from(BOARD_URL)),
            0,
        );
    }
}

/// Opens the size quick pick of `view` under `label`, the strip the board draws at that
/// rectangle from its top-left corner (RB-080-22): the board's place on the screen puts the
/// menu where a grid strip puts its own. Nothing without a board.
pub fn show_size_pick(view: ViewId, label: MapRect) {
    let board = BOARD.with(|board| board.borrow().clone());
    let Some(board) = board else {
        return;
    };
    let board = View::from(&board);
    let Some(window) = board.window() else {
        return;
    };
    let bounds = board.bounds_in_screen();
    let point = Point {
        x: bounds.x.saturating_add(label.x),
        y: bounds
            .y
            .saturating_add(label.y)
            .saturating_add(i32::try_from(label.height).unwrap_or(i32::MAX)),
    };
    super::cell_labels::show_quick_pick_at(view, &window, point);
}

/// Opens the engine menu of `view` under the right end of `label`, the strip the board draws
/// at that rectangle from its top-left corner (RB-080-33). Nothing without a board.
pub fn show_engine_pick(view: ViewId, label: MapRect) {
    let board = BOARD.with(|board| board.borrow().clone());
    let Some(board) = board else {
        return;
    };
    let board = View::from(&board);
    let Some(window) = board.window() else {
        return;
    };
    let bounds = board.bounds_in_screen();
    let point = Point {
        x: bounds
            .x
            .saturating_add(label.x)
            .saturating_add(i32::try_from(label.width).unwrap_or(i32::MAX)),
        y: bounds
            .y
            .saturating_add(label.y)
            .saturating_add(i32::try_from(label.height).unwrap_or(i32::MAX)),
    };
    super::engine_badge::show_pick_at(view, &window, point);
}

/// The board's window and the screen point (DIP) of `x`/`y` from the board's top-left corner:
/// where a right-click on it asks for a menu (RB-080-46). `None` without a board.
pub fn screen_point(x: i32, y: i32) -> Option<(Window, Point)> {
    let board = View::from(&BOARD.with(|board| board.borrow().clone())?);
    let window = board.window()?;
    let bounds = board.bounds_in_screen();
    Some((
        window,
        Point {
            x: bounds.x.saturating_add(x),
            y: bounds.y.saturating_add(y),
        },
    ))
}

/// The board's rectangle in screen DIP and its window's handle, while the board is shown - in
/// canvas mode, when it fills the grid area (RB-080-50: where the middle button pans).
#[cfg_attr(
    target_os = "macos",
    allow(
        dead_code,
        reason = "used by the Windows-only middle-button pan; its macOS twin is a no-op"
    )
)]
pub fn area_in_screen() -> Option<(Rect, usize)> {
    // Asked inside the mouse hook: never wait for a borrow.
    let board = BOARD.with(|board| board.try_borrow().ok().and_then(|board| board.clone()))?;
    let board = View::from(&board);
    let window = board.window()?;
    if board.is_drawn() == 0 {
        return None;
    }
    Some((
        board.bounds_in_screen(),
        super::handle::value(window.window_handle()),
    ))
}

/// Lets the board go (the window closes).
pub fn clear() {
    BOARD.with(|board| board.borrow_mut().take());
}

fn create() -> Option<BrowserView> {
    let mut client = BoardClient::new();
    let mut delegate = BoardDelegate::new();
    browser_view_create(
        Some(&mut client),
        Some(&CefString::from(BOARD_URL)),
        Some(&BrowserSettings::default()),
        None,
        None,
        Some(&mut delegate),
    )
}

wrap_browser_view_delegate! {
    // Alloy style like every view of the app (ADR 0001, Decision 2); its size is the panel's.
    struct BoardDelegate {}

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn on_browser_created(
            &self,
            _browser_view: Option<&mut BrowserView>,
            browser: Option<&mut Browser>,
        ) {
            if let Some(browser) = browser {
                BOARD_BROWSER.store(browser.identifier(), Ordering::Release);
            }
        }

        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

wrap_client! {
    // The board's handlers: its console, the message router, the navigation guard, no popups,
    // and the shell's keys.
    struct BoardClient {}

    impl Client {
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(super::console::ShellConsole::new())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(BoardLifeSpan::new())
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(BoardRequests::new())
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(BoardLoad::new())
        }

        // Focused (after a click on the board), the leader works as in the toolbar, and
        // `Ctrl+0` sets the canvas to 100 % (RB-080-94).
        fn keyboard_handler(&self) -> Option<KeyboardHandler> {
            Some(BoardKeys::new())
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
    struct BoardLifeSpan {}

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
            // The board opens no windows.
            1
        }

        // The board sits in the main window: CEF's default `DoClose() == false` would close
        // that window and quit the app (`cef_life_span_handler.h`, DoClose); the board's view is
        // released from the hierarchy instead.
        fn do_close(&self, _browser: Option<&mut Browser>) -> c_int {
            1
        }

        fn on_before_close(&self, browser: Option<&mut Browser>) {
            let id = browser.as_ref().map_or(0, |browser| browser.identifier());
            let _ = BOARD_BROWSER.compare_exchange(id, 0, Ordering::AcqRel, Ordering::Acquire);
            router().on_before_close(browser.map(|browser| browser.clone()));
        }
    }
}

wrap_load_handler! {
    struct BoardLoad {}

    impl LoadHandler {
        // The board never zooms (RB-080-50): a level Chromium kept for the shell's host goes.
        fn on_load_end(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _http_status_code: c_int,
        ) {
            if frame.is_some_and(|frame| frame.is_main() != 0) {
                super::zoom::reset(browser);
            }
        }
    }
}

wrap_request_handler! {
    struct BoardRequests {}

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
            // The board stays on the shell's pages, like the shell view.
            if navigation_refused(PageOwner::Shell, main, &url) {
                tracing::warn!("a navigation of the canvas board off its page was refused");
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
