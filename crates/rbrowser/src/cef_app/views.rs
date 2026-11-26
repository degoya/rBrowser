//! The CEF objects of the window: the UI-thread registry of views, the delegates that size
//! and report them, and the client whose handlers feed the host. Everything here runs on the
//! CEF UI thread unless a handler says otherwise.

use std::cell::RefCell;
use std::collections::HashMap;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, OnceLock};

use cef::wrapper::message_router::{
    BrowserSideRouter, MessageRouterBrowserSide, MessageRouterBrowserSideHandlerCallbacks,
    MessageRouterConfig,
};
use cef::*;
use rb_engine::{CellRect, ViewId};

use super::context_menu::{ViewContextMenu, ViewMenu};
use super::keys::ViewKeys;
use crate::control::Core;
use crate::host::Host;
use crate::host::focus_cycle::FocusStop;
use crate::shell::{PageOwner, navigation_refused};

/// The host, once CEF is up.
pub static HOST: OnceLock<Arc<Host>> = OnceLock::new();
/// The core (commands, keys, settings), once CEF is up.
pub static CORE: OnceLock<Arc<Core>> = OnceLock::new();
/// The browser id of the shell view; 0 until it exists.
pub static SHELL_BROWSER: AtomicI32 = AtomicI32::new(0);
/// The browser side of the `cefQuery` router.
static ROUTER: OnceLock<Arc<BrowserSideRouter>> = OnceLock::new();

/// The host, when CEF is up.
pub fn host() -> Option<&'static Arc<Host>> {
    HOST.get()
}

/// The core, when CEF is up.
pub fn core() -> Option<&'static Arc<Core>> {
    CORE.get()
}

/// The browser side of the message router.
pub fn router() -> &'static Arc<BrowserSideRouter> {
    ROUTER.get_or_init(|| BrowserSideRouter::new(MessageRouterConfig::default()))
}

/// The CEF objects of the window. UI thread only.
#[derive(Default)]
pub struct UiState {
    /// The main window.
    pub window: Option<Window>,
    /// The shell view on top.
    pub shell: Option<BrowserView>,
    /// The panel the grid is built in.
    pub grid: Option<Panel>,
    /// The grid's views.
    pub views: HashMap<ViewId, BrowserView>,
}

thread_local! {
    /// The window's objects, owned by the UI thread.
    pub static UI: RefCell<UiState> = RefCell::new(UiState::default());
}

/// `view`'s rectangle in the window's client area (DIP): screen bounds minus the client
/// origin (ADR 0001, Decision 2).
pub fn client_rect(view: &View) -> Option<CellRect> {
    let bounds = view.bounds_in_screen();
    let origin = view.window()?.client_area_bounds_in_screen();
    Some(CellRect {
        x: bounds.x - origin.x,
        y: bounds.y - origin.y,
        width: u32::try_from(bounds.width).unwrap_or(0),
        height: u32::try_from(bounds.height).unwrap_or(0),
    })
}

fn text(value: &CefStringUserfree) -> String {
    CefString::from(value).to_string()
}

wrap_panel_delegate! {
    // A panel of the grid plan: its preferred size is the fixed part inside it; the grid
    // root also reports the grid area.
    pub struct SizedPanel {
        size: Size,
        reports_area: bool,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            self.size.clone()
        }

        fn on_layout_changed(&self, view: Option<&mut View>, _new_bounds: Option<&Rect>) {
            if !self.reports_area {
                return;
            }
            if let (Some(host), Some(rect)) = (host(), view.and_then(|view| client_rect(view))) {
                host.on_grid_area(rect);
            }
        }
    }

    impl PanelDelegate {}
}

/// The shell view's height in DIP: one toolbar row, two while it wraps (RB-080-52), and the
/// activity bar below them while it stands there (RB-090-67).
fn shell_height() -> i32 {
    i32::try_from(crate::activity_bar::shell_view_height()).unwrap_or(44)
}

/// A strip's size for a view delegate: `width` x `height` with no side 0, because CEF takes a
/// size with a side of 0 for empty and falls back to the view's own (0 for a browser view) -
/// the toolbar had no height (`crate::fit::strip_size`).
pub fn strip(width: i32, height: i32) -> Size {
    let (width, height) = crate::fit::strip_size(width, height);
    Size { width, height }
}

/// A panel with the preferred size `width` x `height`.
pub fn sized_panel(width: u32, height: u32, reports_area: bool) -> Option<Panel> {
    let size = Size {
        width: i32::try_from(width).unwrap_or(i32::MAX),
        height: i32::try_from(height).unwrap_or(i32::MAX),
    };
    let mut delegate = SizedPanel::new(size, reports_area);
    panel_create(Some(&mut delegate))
}

pub use super::label_strip::cell_label;

wrap_browser_view_delegate! {
    // A browser view: the shell (`view: None`) or a grid view. Alloy style, because Chrome
    // style allows one browser view per window (ADR 0001, Decision 2).
    pub struct BrowserDelegate {
        view: Option<ViewId>,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            match self.view {
                // Never a side of 0: CEF would take the size for empty (`strip`).
                None => strip(0, shell_height()),
                Some(_) => Size {
                    width: 0,
                    height: 0,
                },
            }
        }

        // Older box layouts ask a child's height for its width (a browser view's web contents
        // answer 0), and a box layout shrinks a child no further than its minimum; with the
        // non-empty preferred size (`strip`) all three hold the shell at its height.
        fn height_for_width(&self, _view: Option<&mut View>, _width: c_int) -> c_int {
            if self.view.is_none() { shell_height() } else { 0 }
        }

        fn minimum_size(&self, _view: Option<&mut View>) -> Size {
            if self.view.is_none() {
                strip(0, shell_height())
            } else {
                Size {
                    width: 0,
                    height: 0,
                }
            }
        }

        fn on_layout_changed(&self, view: Option<&mut View>, new_bounds: Option<&Rect>) {
            if self.view.is_none() {
                // A shell without height shows no toolbar (owner acceptance 0.1).
                let height = new_bounds.map_or(0, |bounds| bounds.height);
                if height <= 0 {
                    tracing::warn!(height, "the shell view was laid out without height");
                }
                return;
            }
            let (Some(id), Some(host)) = (self.view, host()) else {
                return;
            };
            if let Some(rect) = view.and_then(|view| client_rect(view)) {
                host.on_view_bounds(id, rect);
            }
            // An external view's window follows its cell (RB-080-39).
            super::embed::place_view(id);
        }
    }

    impl BrowserViewDelegate {
        fn on_browser_created(
            &self,
            _browser_view: Option<&mut BrowserView>,
            browser: Option<&mut Browser>,
        ) {
            if let (None, Some(browser)) = (self.view, browser) {
                SHELL_BROWSER.store(browser.identifier(), Ordering::Release);
            }
        }

        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

/// A browser view showing `url`: the shell for `None`, else the grid view `view`; in `context`,
/// else the global request context (a named session's, RB-020-07).
pub fn browser_view(
    view: Option<ViewId>,
    url: &str,
    mut context: Option<RequestContext>,
) -> Option<BrowserView> {
    let mut client = ViewClient::new(view, view.map(ViewMenu::for_view));
    let mut delegate = BrowserDelegate::new(view);
    browser_view_create(
        Some(&mut client),
        Some(&CefString::from(url)),
        Some(&BrowserSettings::default()),
        None,
        context.as_mut(),
        Some(&mut delegate),
    )
}

wrap_client! {
    // The handlers of one view (`None`: the shell).
    pub struct ViewClient {
        view: Option<ViewId>,
        menu: Option<ViewMenu>,
    }

    impl Client {
        // The shell's console reaches the app's log; a grid view reports its address and title.
        fn display_handler(&self) -> Option<DisplayHandler> {
            match self.view {
                None => Some(super::console::ShellConsole::new()),
                Some(_) => Some(ViewDisplay::new(self.view)),
            }
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(ViewLoad::new(self.view))
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(ViewLifeSpan::new(self.view))
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(ViewRequests::new(self.view))
        }

        fn keyboard_handler(&self) -> Option<KeyboardHandler> {
            Some(ViewKeys::new(self.view))
        }

        // A click into a grid view makes it the active view (RB-020-02); Tab past the end of a
        // view or the toolbar goes on to the next view of the Tab order (RB-090-23).
        fn focus_handler(&self) -> Option<FocusHandler> {
            Some(match self.view {
                Some(view) => super::view_focus::ViewFocus::new(view),
                None => super::take_focus::TakeFocus::new(FocusStop::Toolbar),
            })
        }

        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            self.menu.as_ref().map(ViewContextMenu::of)
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

wrap_display_handler! {
    struct ViewDisplay {
        view: Option<ViewId>,
    }

    impl DisplayHandler {
        fn on_address_change(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            url: Option<&CefString>,
        ) {
            let (Some(view), Some(frame), Some(url), Some(host)) = (self.view, frame, url, host())
            else {
                return;
            };
            if frame.is_main() != 0 {
                host.on_address_change(view, &url.to_string());
            }
        }

        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let (Some(view), Some(title), Some(host)) = (self.view, title, host()) {
                host.on_title_change(view, &title.to_string());
            }
        }
    }
}

wrap_load_handler! {
    struct ViewLoad {
        view: Option<ViewId>,
    }

    impl LoadHandler {
        fn on_loading_state_change(
            &self,
            _browser: Option<&mut Browser>,
            is_loading: c_int,
            _can_go_back: c_int,
            _can_go_forward: c_int,
        ) {
            if let (Some(view), Some(host)) = (self.view, host()) {
                host.on_loading_state(view, is_loading != 0);
            }
        }

        fn on_load_end(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _http_status_code: c_int,
        ) {
            let (Some(frame), Some(host)) = (frame, host()) else {
                return;
            };
            if frame.is_main() == 0 {
                return;
            }
            match self.view {
                Some(view) => {
                    host.on_load_end(view, &text(&frame.url()));
                    // The page zoom it loaded at (RB-090-20).
                    super::page_zoom::loaded(view, browser);
                }
                // The shell is ready: give it the state once; events follow, and what waited
                // for the shell (binding notices) is shown.
                None => {
                    if let Some(script) = crate::ipc::state_event(host) {
                        frame.execute_java_script(Some(&CefString::from(script.as_str())), None, 0);
                    }
                    super::zoom::shell_loaded(browser);
                }
            }
        }

        fn on_load_error(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            error_code: Errorcode,
            error_text: Option<&CefString>,
            failed_url: Option<&CefString>,
        ) {
            if self.view.is_none() && frame.is_some_and(|frame| frame.is_main() != 0) {
                let (url, error) = (failed_url.map(ToString::to_string), error_text.map(ToString::to_string));
                tracing::error!(code = error_code.get_raw(), ?url, ?error, "the shell did not load");
            }
        }
    }
}

wrap_life_span_handler! {
    struct ViewLifeSpan {
        view: Option<ViewId>,
    }

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
            client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut c_int>,
        ) -> c_int {
            // The shell opens no windows. A grid view's popup opens as CEF's own window with a
            // client of its own: it is no grid view, so its address, title and loads are not
            // reported as the opener's, it gets no rBrowser shortcuts and no `cefQuery` route,
            // and it never loads a shell page. Its own popups inherit that client.
            let Some(view) = self.view else {
                return 1;
            };
            if let Some(client) = client {
                *client = Some(super::popup::PopupClient::new(view));
            }
            0
        }

        // CEF's default (false) "will send the standard close notification to the browser's
        // top-level parent window (... CefWindowDelegate::CanClose() callback from Views)"
        // (`cef_life_span_handler.h`, DoClose): the main window, which quits the app. A view or
        // the shell never closes its window; the grid drops a view by releasing its detached
        // BrowserView (`grid.rs`), and the app's end tears the hierarchy down without DoClose.
        fn do_close(&self, _browser: Option<&mut Browser>) -> c_int {
            1
        }

        fn on_before_close(&self, browser: Option<&mut Browser>) {
            // Its requests waiting in rBrowser's sign-in dialog end with it (RB-099-04).
            super::auth::view_closed(browser.as_deref());
            router().on_before_close(browser.map(|browser| browser.clone()));
        }
    }
}

wrap_request_handler! {
    struct ViewRequests {
        view: Option<ViewId>,
    }

    impl RequestHandler {
        fn on_before_browse(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _user_gesture: c_int,
            _is_redirect: c_int,
        ) -> c_int {
            let url = request.map(|request| text(&request.url())).unwrap_or_default();
            let main = frame.as_ref().is_some_and(|frame| frame.is_main() != 0);
            // The shell stays on its own pages; no grid view ever loads them.
            let owner = match self.view {
                None => PageOwner::Shell,
                Some(_) => PageOwner::View,
            };
            if navigation_refused(owner, main, &url) {
                tracing::warn!(view = ?self.view, "a navigation across the shell boundary was refused");
                return 1;
            }
            router().on_before_browse(
                browser.map(|browser| browser.clone()),
                frame.map(|frame| frame.clone()),
            );
            0
        }

        // A Basic auth challenge of an environment with a stored credential (RB-094-08).
        fn auth_credentials(
            &self,
            browser: Option<&mut Browser>,
            origin_url: Option<&CefString>,
            is_proxy: c_int,
            host: Option<&CefString>,
            port: c_int,
            realm: Option<&CefString>,
            scheme: Option<&CefString>,
            callback: Option<&mut AuthCallback>,
        ) -> c_int {
            let asked = super::auth::Asked { origin_url, is_proxy, host, port, realm, scheme };
            let requester = self.view.map(crate::basic_auth::later::Requester::View);
            super::auth::answer(requester, browser, &asked, callback)
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
