//! The middle mouse button pans the canvas (RB-080-50), as Space + drag does: held and dragged
//! anywhere over the canvas - the free board, a view, a label or a footer - it moves the camera,
//! and the page under the pointer never sees the drag, so Chromium's autoscroll does not start.
//! The window hears the button itself (`cef_app/board_pan.rs`: the views are native and the
//! board lies behind them); this is the gesture without a window, in screen DIP:
//!
//! - a press over the canvas is taken from the page ([`Step::Begin`]);
//! - once the pointer is more than [`CLICK_SLOP`] DIP from the press it is a drag: each move
//!   shows the canvas there ([`show`], a frame of `super::canvas_live`), the release keeps the
//!   camera ([`keep`]), Escape puts it back ([`Step::Cancel`]);
//! - a press let go within the slop was a click: the page gets it after all ([`Step::Click`]) -
//!   a link opens in the background, as without the pan.
//!
//! `layoutModes.middleButtonPan` off leaves the button to the pages.

use rb_commands::CommandError;

use super::canvas::Camera;
use super::canvas_board::{MAX_DRAG, panned};
use super::canvas_live;
use crate::control::Core;

/// How far the pointer may move with the button held and the press still be a click, in DIP.
pub const CLICK_SLOP: i32 = 4;

/// The camera `from` after a drag of `dx`/`dy` screen DIP: the canvas follows the pointer.
fn dragged(from: Camera, dx: i32, dy: i32) -> Camera {
    let held = |delta: i32| delta.clamp(-MAX_DRAG, MAX_DRAG);
    panned(from, (held(dx), held(dy)))
}

/// A frame of the drag: the canvas `dx`/`dy` screen DIP from where the camera was `from` when
/// it began. Nothing is kept.
pub fn show(core: &Core, from: Camera, dx: i32, dy: i32) -> Result<(), CommandError> {
    canvas_live::show(core, dragged(from, dx, dy))
}

/// The drag ended `dx`/`dy` from where it began: the camera is kept.
pub fn keep(core: &Core, from: Camera, dx: i32, dy: i32) -> Result<(), CommandError> {
    canvas_live::keep(core, dragged(from, dx, dy))
}

/// What the window does with a mouse message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing: the message is the page's.
    Pass,
    /// A press over the canvas: taken from the page. Whether it is a drag or a click shows.
    Begin,
    /// The message is taken from the page and nothing else happens (the release after a
    /// cancelled drag).
    Take,
    /// The canvas follows: `dx`/`dy` from the press. The message is passed on.
    Move { dx: i32, dy: i32 },
    /// The drag ended `dx`/`dy` from the press and is kept; `taken` when the message (the
    /// button's release) is taken from the page.
    End { dx: i32, dy: i32, taken: bool },
    /// The press was let go where it went down: the page gets a middle click at the press
    /// after all. The release itself is taken.
    Click { at: (i32, i32) },
    /// Escape during a drag: the canvas goes back. The message is passed on.
    Cancel,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Idle,
    /// The button is down at the point; not a drag yet.
    Pressed((i32, i32)),
    /// A drag from the point.
    Dragging((i32, i32)),
    /// The drag was cancelled; the button is still down and its release is still ours.
    Spent,
}

/// The middle button's gesture, from the press to the release.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiddlePan {
    phase: Phase,
}

impl MiddlePan {
    /// Whether the button is down in a gesture of ours.
    pub fn active(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// The button went down at `at` (a double click's second press too); `on_canvas` when the
    /// pointer is over the canvas in canvas mode with the setting on.
    pub fn down(&mut self, at: (i32, i32), on_canvas: bool) -> Step {
        if !on_canvas {
            self.phase = Phase::Idle;
            return Step::Pass;
        }
        self.phase = Phase::Pressed(at);
        Step::Begin
    }

    /// The pointer moved to `at`. `held` says whether the button is still down - a release the
    /// window did not hear (outside it) ends the gesture at the next move; `escape` whether
    /// Escape is down.
    pub fn moved(&mut self, at: (i32, i32), held: bool, escape: bool) -> Step {
        match self.phase {
            Phase::Idle => Step::Pass,
            Phase::Spent => {
                if !held {
                    self.phase = Phase::Idle;
                }
                Step::Pass
            }
            Phase::Pressed(start) => {
                if !held || escape {
                    // Nothing was shown: nothing to keep or to put back. An unheard release
                    // gives the page no click - the pointer is somewhere else by now.
                    self.phase = if held { Phase::Spent } else { Phase::Idle };
                    return Step::Pass;
                }
                let (dx, dy) = delta(start, at);
                if dx.abs() <= CLICK_SLOP && dy.abs() <= CLICK_SLOP {
                    return Step::Pass;
                }
                self.phase = Phase::Dragging(start);
                Step::Move { dx, dy }
            }
            Phase::Dragging(start) => {
                let (dx, dy) = delta(start, at);
                if escape {
                    self.phase = if held { Phase::Spent } else { Phase::Idle };
                    Step::Cancel
                } else if held {
                    Step::Move { dx, dy }
                } else {
                    self.phase = Phase::Idle;
                    Step::End {
                        dx,
                        dy,
                        taken: false,
                    }
                }
            }
        }
    }

    /// The button was let go at `at`.
    pub fn up(&mut self, at: (i32, i32)) -> Step {
        let phase = std::mem::take(&mut self.phase);
        match phase {
            Phase::Idle => Step::Pass,
            Phase::Spent => Step::Take,
            Phase::Pressed(start) => Step::Click { at: start },
            Phase::Dragging(start) => {
                let (dx, dy) = delta(start, at);
                Step::End {
                    dx,
                    dy,
                    taken: true,
                }
            }
        }
    }
}

fn delta(start: (i32, i32), at: (i32, i32)) -> (i32, i32) {
    (at.0.saturating_sub(start.0), at.1.saturating_sub(start.1))
}

#[cfg(test)]
#[path = "middle_pan_tests.rs"]
mod tests;
