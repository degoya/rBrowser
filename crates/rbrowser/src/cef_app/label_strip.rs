//! The label strip of a grid cell: the native button over a view that names its breakpoint and
//! size. A click makes the view active; a click on the active view's strip opens its size
//! quick pick (`cell_labels.rs`). Split from `views.rs`, which re-exports [`cell_label`]. UI
//! thread.

use cef::*;
use rb_engine::{ViewHost, ViewId};

use super::views::{host, strip};

wrap_button_delegate! {
    // The label strip of a cell; a click makes its view active.
    pub struct CellLabel {
        view: ViewId,
        height: i32,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            strip(0, self.height)
        }

        // The theme's ground is over the strip's - it came into a window, or the window's
        // theme changed: the leader's mark again (RB-080-47).
        fn on_theme_changed(&self, view: Option<&mut View>) {
            let label = view
                .and_then(|view| view.as_button())
                .and_then(|button| button.as_label_button());
            if let Some(label) = label {
                super::active_label::theme_changed(self.view, &label);
            }
        }
    }

    impl ButtonDelegate {
        fn on_button_pressed(&self, button: Option<&mut Button>) {
            let Some(host) = host() else {
                return;
            };
            // A click on the active view's strip opens its size quick pick (RB-010-16).
            let was_active = host.active_view() == Some(self.view);
            if let Err(error) = host.set_active_view(self.view) {
                tracing::warn!(%error, "the clicked view could not be activated");
            } else if was_active && let Some(button) = button {
                super::cell_labels::show_quick_pick(self.view, button);
            }
        }
    }
}

/// The label strip of `view`.
pub fn cell_label(view: ViewId, label: &str, height: u32) -> Option<LabelButton> {
    let mut delegate = CellLabel::new(view, i32::try_from(height).unwrap_or(i32::MAX));
    label_button_create(Some(&mut delegate), Some(&CefString::from(label)))
}
