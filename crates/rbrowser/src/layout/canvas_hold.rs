//! The views wait for their frames (RB-090-55). The window moves the canvas's views at once,
//! while the canvas board draws their frames, labels and the frame around their footers in a
//! page of its own, frames later - the views ran ahead of their frames during every zoom. With
//! a board that answers, each canvas the board is sent (`canvasLive`) carries a number, the
//! views' places for it are held under that number, and they go to the window once the board
//! says it painted that canvas (`canvasPainted`): the newest canvas painted, never an older one
//! after it. A board that does not answer keeps nothing for longer than [`HOLD_MS`].
//!
//! Beside the holds lies the camera a gesture is trying out, or the place a label dragged tries
//! out for its view (RB-094-18): every state the core sends while the gesture lasts draws the
//! board's frames for it (`super::canvas_board::board_state`), so a state does not take the
//! frames back to the camera or the place kept before the gesture.

use std::collections::VecDeque;

use rb_engine::CellRect;

use super::canvas::Camera;
use crate::host::ui::PlacedView;

/// How long the views wait for a board that does not answer, in ms.
pub const HOLD_MS: u64 = 100;
/// The most canvases held at once; a board that falls behind loses the oldest.
pub const MAX_HELD: usize = 8;

/// The places of the views for one canvas the board was sent.
#[derive(Clone, Debug, PartialEq)]
pub struct Held {
    /// The canvas's number (`canvasLive`'s `seq`).
    pub number: u32,
    /// The grid area the places were computed for.
    pub area: CellRect,
    /// Where each view goes.
    pub placed: Vec<PlacedView>,
}

/// The canvas place a label dragged tries out for its view (RB-094-18).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moved {
    /// The view's place in the canvas's order.
    pub index: usize,
    /// Its left edge in canvas DIP.
    pub x: i32,
    /// Its top edge in canvas DIP.
    pub y: i32,
}

/// The canvas on trial and the places waiting for the board; part of the host's state.
#[derive(Clone, Debug, Default)]
pub struct CanvasHold {
    /// The camera a gesture is trying out; `None` outside a gesture.
    live: Option<Camera>,
    /// The place a label dragged tries out; `None` outside a drag.
    moved: Option<Moved>,
    /// The number given last.
    next: u32,
    /// The places waiting for the board, oldest first.
    held: VecDeque<Held>,
}

impl CanvasHold {
    /// The camera a gesture is trying out.
    pub fn live(&self) -> Option<Camera> {
        self.live
    }

    /// A gesture tries `camera` out.
    pub fn try_out(&mut self, camera: Camera) {
        self.live = Some(camera);
    }

    /// The place a label dragged tries out.
    pub fn moved(&self) -> Option<Moved> {
        self.moved
    }

    /// A label dragged tries `moved` out.
    pub fn try_moved(&mut self, moved: Moved) {
        self.moved = Some(moved);
    }

    /// The number of the next canvas the board is sent.
    pub fn number(&mut self) -> u32 {
        self.next = self.next.wrapping_add(1);
        self.next
    }

    /// Holds `held` until the board painted it; past [`MAX_HELD`] the oldest goes.
    pub fn hold(&mut self, held: Held) {
        if self.held.len() >= MAX_HELD {
            self.held.pop_front();
        }
        self.held.push_back(held);
    }

    /// The board painted the canvas `number`: its places, once; every older hold goes with it.
    /// `None` for a number placed already, dropped or never held.
    pub fn painted(&mut self, number: u32) -> Option<Held> {
        let index = self.held.iter().position(|held| held.number == number)?;
        self.held.drain(..=index).next_back()
    }

    /// The views were placed for good (a camera kept, a cancel, another mode): the trial and
    /// every hold end.
    pub fn settle(&mut self) {
        self.live = None;
        self.moved = None;
        self.held.clear();
    }
}

#[cfg(test)]
#[path = "canvas_hold_tests.rs"]
mod tests;
