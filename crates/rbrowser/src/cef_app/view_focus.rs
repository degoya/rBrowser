//! The focus handler of a grid view (RB-020-02): a click into the view (it gets the keyboard
//! focus) makes it the active view, as a click on its label strip does - "the view you work in
//! is the active view". UI thread.
//!
//! A replayed input (CDP `Input.dispatch*`) reaches the page without moving the native focus, so
//! a follower never takes the lead this way. The active view getting the focus back (the window
//! activated again, `set_active_view` focusing it) changes nothing (`Host::on_view_focused`).
//!
//! A page loading in the view asks for the focus first (`on_set_focus`, CEF's navigation
//! source): in focus mode it is refused, so the toolbar keeps the keys while its arrow keys
//! step the breakpoints (`host::keeps_focus`, RB-080-17).
//!
//! Tab past the page's last control (Shift+Tab past its first) hands the focus on to the status
//! bar (the toolbar) - `take_focus.rs`, RB-090-23.

use std::os::raw::c_int;

use cef::*;
use rb_engine::ViewId;

use super::take_focus::take_focus;
use super::views::{core, host};
use crate::host::focus_cycle::FocusStop;
use crate::host::{FocusRequest, keeps_focus};

wrap_focus_handler! {
    pub(super) struct ViewFocus {
        view: ViewId,
    }

    impl FocusHandler {
        fn on_set_focus(&self, _browser: Option<&mut Browser>, source: FocusSource) -> c_int {
            let request = if source == FocusSource::NAVIGATION {
                FocusRequest::Navigation
            } else {
                FocusRequest::System
            };
            let (Some(host), Some(core)) = (host(), core()) else {
                return 0;
            };
            let mode = host.shell_state().mode;
            c_int::from(keeps_focus(request, mode, &core.user_settings().focus_mode))
        }

        fn on_take_focus(&self, _browser: Option<&mut Browser>, next: c_int) {
            take_focus(FocusStop::ActiveView, next);
        }

        fn on_got_focus(&self, _browser: Option<&mut Browser>) {
            if let Some(host) = host() {
                host.on_view_focused(self.view);
            }
        }
    }
}
