//! A grid view got the keyboard focus (a click into it, `cef_app/view_focus.rs`): with
//! `sync.activateOnFocus` (off by default) it becomes the active view, the leader sync follows
//! (RB-020-02); otherwise only a click on its label strip does that. Before that, a view asks
//! for the focus ([`keeps_focus`]): in focus mode a page loading in the view does not get it,
//! so the toolbar's arrow keys keep stepping the breakpoints (RB-080-17).

use rb_config::{FocusModeSettings, LayoutMode};
use rb_engine::{EngineError, ViewEvent, ViewHost, ViewId};

use super::Host;

/// Where a view's request for the keyboard focus comes from (CEF's focus source).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusRequest {
    /// A page loads in the view: a new view's first page, `navigate`, a bookmark.
    Navigation,
    /// A click into the view, `set_active_view`, the window activated.
    System,
}

/// Whether `request` is refused, so the keyboard focus stays where it is. In focus mode every
/// arrow key of the toolbar opens a new view for the next breakpoint, and the page loading in
/// it took the focus: the toolbar stepped once and lost the keys. With `focusMode.keepFocus`
/// (on by default) a loading page never takes the focus there; a click or an activation
/// always gets it, as every request does in the other modes.
pub fn keeps_focus(request: FocusRequest, mode: LayoutMode, settings: &FocusModeSettings) -> bool {
    settings.keep_focus && request == FocusRequest::Navigation && mode == LayoutMode::Focus
}

impl Host {
    /// Makes `view` the active view; with `focus` the window focuses it too. The focus handler
    /// passes `false`: the view has the focus already, and a focus queued for it would reach the
    /// window after the next view's and take the focus back - the views took the lead from each
    /// other in a loop.
    pub(super) fn activate(&self, view: ViewId, focus: bool) -> Result<(), EngineError> {
        let changed = {
            let mut state = self.state();
            if !state.views.contains_key(&view) {
                return Err(EngineError::UnknownView(view));
            }
            state.active.replace(view) != Some(view)
        };
        if focus {
            self.ui.focus(view);
        }
        if changed {
            // The leader's label strip in the accent colour (RB-080-04).
            self.ui.mark_active(Some(view));
            self.emit(ViewEvent::ActiveChanged(Some(view)));
            // The CPU budget moves off the new leader (RB-020-09).
            self.rethrottle();
        }
        Ok(())
    }

    /// Makes `view` the active view when it is not already, without focusing it again; `true`
    /// when it changed.
    pub fn on_view_focused(&self, view: ViewId) -> bool {
        if self.active_view() == Some(view)
            || !self.with_config(|config| config.sync.activate_on_focus)
        {
            return false;
        }
        match self.activate(view, false) {
            Ok(()) => {
                // Evidence for a focus that moves on its own (a follower taking the lead).
                tracing::info!(%view, "a view got the focus and became the active view");
                true
            }
            Err(error) => {
                tracing::debug!(%error, %view, "a focused view was not made active");
                false
            }
        }
    }
}

#[cfg(test)]
#[path = "focused_tests.rs"]
mod tests;
