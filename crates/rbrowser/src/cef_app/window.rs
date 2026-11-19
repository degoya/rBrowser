//! The main window: the shell view on top, the content area below - the grid and, when docked,
//! DevTools (`devtools.rs`) - and the status bar at the bottom (`status_bar.rs`, RB-080-15);
//! neither strip ever overlaps a view (vertical box, shell and bar flex 0, content flex 1).

use std::os::raw::c_int;

use cef::*;

use super::views::{UI, browser_view, sized_panel};
use crate::devtools::focus::KeySource;
use crate::shell::SHELL_URL;

/// The window's first size in DIP.
const INITIAL_SIZE: Size = Size {
    width: 1600,
    height: 1000,
};

wrap_window_delegate! {
    struct MainWindow {
        title: String,
        background: bool,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            INITIAL_SIZE
        }
    }

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            let Some(window) = window else {
                return;
            };
            // The label strips hear the right mouse button from here on (RB-080-46).
            super::strip_clicks::install();
            let root = UI.with(|ui| {
                let ui = ui.borrow();
                let (Some(shell), Some(grid)) = (&ui.shell, &ui.grid) else {
                    return None;
                };
                let root = sized_panel(0, 0, false)?;
                let layout = root.set_to_box_layout(Some(&BoxLayoutSettings {
                    horizontal: 0,
                    cross_axis_alignment: AxisAlignment::STRETCH,
                    ..Default::default()
                }))?;
                let mut shell = View::from(shell);
                root.add_child_view(Some(&mut shell));
                layout.set_flex_for_view(Some(&mut shell), 0);
                // RB-010-12 wraps the grid for zoom to fit and pan (`frame.rs`); docked
                // DevTools sit below or beside it in the content area (RB-010-06), docked
                // plugin panels beside that (RB-080-57).
                let devtools = super::devtools::content(super::frame::wrap_grid(grid))?;
                let mut content = super::panel_dock::content(devtools)?;
                root.add_child_view(Some(&mut content));
                layout.set_flex_for_view(Some(&mut content), 1);
                // The status bar (RB-080-15): the grid ends above it.
                if let Some(mut bar) = super::status_bar::create() {
                    root.add_child_view(Some(&mut bar));
                    layout.set_flex_for_view(Some(&mut bar), 0);
                }
                Some(root)
            });
            if let Some(root) = root {
                window.add_child_view(Some(&mut View::from(&root)));
            } else {
                tracing::error!("the window's views could not be built");
            }
            window.set_title(Some(&CefString::from(self.title.as_str())));
            super::icon::apply(window);
            super::theme::adopt(window);
            super::activate::show(window, self.background);
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
            tracing::info!("the main window was destroyed; the app ends");
            super::strip_clicks::uninstall();
            // Release every CEF object before the message loop ends and CEF shuts down: a
            // thread-local still holding one is dropped after `CefShutdown` (RB-080-91).
            super::teardown::release();
            quit_message_loop();
        }

        // The cef crate's defaults are 0 (CEF's own are true): without these the main window
        // could not be resized, maximised or minimised (owner, build ed2fd51).
        fn can_resize(&self, _window: Option<&mut Window>) -> i32 {
            1
        }

        fn can_maximize(&self, _window: Option<&mut Window>) -> i32 {
            1
        }

        fn can_minimize(&self, _window: Option<&mut Window>) -> i32 {
            1
        }

        fn can_close(&self, _window: Option<&mut Window>) -> i32 {
            // Logged, so a diagnostics ZIP shows what closed the window (a close button, or a
            // browser's DoClose falling through to its top-level window).
            tracing::info!("the main window is asked to close");
            // The browsers close now: a session they end is not attached again (RB-080-91).
            if let Some(host) = super::views::host() {
                host.begin_shutdown();
            }
            // The pages window and the page windows close with the main one (RB-010-08).
            super::settings_window::close();
            super::page_windows::close_all();
            // So do the report windows (RB-091-02) and the sitemap gallery's (RB-090-83).
            super::report_window::close_all();
            super::gallery_window::close_all();
            // Popped-out views go back into this window first, so they close with it (RB-080-14).
            super::pop_out::close_all();
            1
        }

        fn window_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }

        // The window moved, was resized, minimized or restored: the external views' windows
        // follow their cells (RB-080-39).
        fn on_window_bounds_changed(&self, _window: Option<&mut Window>, _new_bounds: Option<&Rect>) {
            super::embed::place_all();
        }

        // A key the focused view did not handle. A browser's own keyboard handler saw it first
        // (`keys.rs`); with the focus on the window, a panel or an overlay without a browser it
        // is the shell's, so the leader works there too (RB-010-17).
        fn on_key_event(&self, window: Option<&mut Window>, event: Option<&KeyEvent>) -> c_int {
            let in_browser = window
                .and_then(|window| window.focused_view())
                .is_some_and(|view| view.as_browser_view().is_some());
            if in_browser {
                return 0;
            }
            super::keys::pre_key(KeySource::Shell, event)
        }
    }
}

/// Creates the shell view, the grid panel and the window titled `title`, focused unless
/// `background`. UI thread, once.
pub fn open(title: &str, background: bool) {
    let shell = browser_view(None, SHELL_URL, None);
    let grid = sized_panel(0, 0, true);
    if shell.is_none() || grid.is_none() {
        tracing::error!("the shell view or the grid panel could not be created");
    }
    UI.with(|ui| {
        let mut ui = ui.borrow_mut();
        ui.shell = shell;
        ui.grid = grid;
    });
    let mut delegate = MainWindow::new(title.to_owned(), background);
    let window = window_create_top_level(Some(&mut delegate));
    UI.with(|ui| ui.borrow_mut().window = window);
}

/// The shell view's height changed - the toolbar wraps into a second row or is back in one
/// (RB-080-52, `crate::toolbar_rows`), or the text size changed (RB-090-26, with the status
/// bar's): the window's box asks the shell and the bar for their sizes again and moves the
/// content down or up; the grid reports its new area itself. UI thread.
pub fn relayout_shell() {
    UI.with(|ui| {
        let ui = ui.borrow();
        let Some(shell) = &ui.shell else {
            return;
        };
        let view = View::from(shell);
        view.invalidate_layout();
        super::status_bar::invalidate();
        if let Some(root) = view.parent_view().and_then(|parent| parent.as_panel()) {
            root.layout();
        }
        // A popup that hangs below the toolbar follows it.
        super::page_windows::reposition();
        // In the log, so a diagnostics ZIP shows whether the window followed the toolbar.
        tracing::info!(
            height = crate::activity_bar::shell_view_height(),
            laid_out = view.bounds().height,
            "the shell view's height follows the toolbar's rows"
        );
    });
}
