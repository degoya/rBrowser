//! `on_take_focus` of the toolbar, the grid views and the status bar (RB-090-23): Tab past a
//! view's last control (Shift+Tab past its first) hands the focus to the next view of the Tab
//! order, `host::focus_cycle`. UI thread.

use std::os::raw::c_int;

use cef::*;
use rb_engine::ViewHost as _;

use super::views::{UI, host};
use crate::host::focus_cycle::{FocusStop, next_stop};

/// Moves the focus on from `from`; `next` is CEF's: 1 for Tab, 0 for Shift+Tab.
pub(super) fn take_focus(from: FocusStop, next: c_int) {
    let present = |stop: FocusStop| match stop {
        FocusStop::Toolbar => true,
        FocusStop::ActiveView => active_view().is_some(),
        FocusStop::StatusBar => super::status_bar::height() > 0,
    };
    let Some(stop) = next_stop(from, next != 0, present) else {
        return;
    };
    let browser = match stop {
        FocusStop::Toolbar => {
            UI.with(|ui| ui.borrow().shell.as_ref().and_then(BrowserView::browser))
        }
        FocusStop::ActiveView => active_view(),
        FocusStop::StatusBar => super::status_bar::browser(),
    };
    if let Some(host) = browser.and_then(|browser| browser.host()) {
        host.set_focus(1);
    }
}

/// The active grid view's browser.
fn active_view() -> Option<Browser> {
    let view = host()?.active_view()?;
    UI.with(|ui| ui.borrow().views.get(&view).and_then(BrowserView::browser))
}

wrap_focus_handler! {
    // The focus handler of a shell page in the Tab order (the toolbar, the status bar).
    pub(super) struct TakeFocus {
        from: FocusStop,
    }

    impl FocusHandler {
        fn on_take_focus(&self, _browser: Option<&mut Browser>, next: c_int) {
            take_focus(self.from, next);
        }
    }
}
