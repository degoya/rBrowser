//! The grid's strips follow Windows' text size (RB-090-26, [`crate::text_scale`]): the label
//! strip's height for every layout, and the grid sent again when the size changes, with its
//! views kept.

use rb_engine::ViewEvent;

use super::{Host, LABEL_HEIGHT};

/// The height of the label strip above each view, in DIP at the text size in effect.
#[must_use]
pub fn label_height() -> u32 {
    crate::text_scale::dip(LABEL_HEIGHT)
}

impl Host {
    /// The text size changed: the grid - or the layout mode's places - is sent again from the
    /// state in effect, with the strips' new heights (label strips, breakpoint bars in the
    /// cells and in the popped-out views' windows), and the views get new metrics. Nothing is
    /// rebuilt and no layout or mode is written, so a change that comes in between stays.
    /// Nothing while no project is open.
    pub fn text_scale_changed(&self) {
        if self.state().order.is_empty() {
            return;
        }
        self.regrid("the text size changed".to_owned());
        let views = self.state().order.clone();
        for view in views {
            self.spawn_apply(view, false);
        }
        self.emit(ViewEvent::LayoutChanged);
    }
}
