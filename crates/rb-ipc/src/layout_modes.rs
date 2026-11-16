//! The layout modes' part of the shell state (RB-080-04): what the toolbar's controls show -
//! the split's divider, the strip's scroll, the canvas minimap - and what the canvas board (the
//! page behind the canvas's views) draws, with the phase of its drags. Mirrored in
//! `ui/src/ipc/layout.ts`.

use rb_config::MinimapPosition;
use rb_engine::ViewId;
use serde::{Deserialize, Serialize};

/// The state of the mode in effect; one of the parts is set.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeState {
    /// Split: the divider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split: Option<SplitState>,
    /// Horizontal: the scroll.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strip: Option<StripState>,
    /// Canvas: the minimap's content; `None` while the minimap is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas: Option<MinimapState>,
    /// Canvas: the labels the board draws behind the views.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<BoardState>,
}

/// The split's divider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitState {
    /// The primary side's share in per mille (100 to 900).
    pub ratio_permille: u32,
    /// `true` for a vertical divider (primary left).
    pub vertical: bool,
    /// The keyboard step in per mille.
    pub step_permille: u32,
}

/// The strip's scroll.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StripState {
    /// The view at the left edge, 0-based.
    pub first: u32,
    /// The largest useful `first`.
    pub last_first: u32,
    /// The views in the strip.
    pub count: u32,
}

/// The minimap: every view's place and the visible part, in canvas DIP (rounded).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinimapState {
    /// Where the minimap sits in the toolbar strip.
    pub position: MinimapPosition,
    /// The views.
    pub items: Vec<MinimapItem>,
    /// What the grid area shows.
    pub viewport: MapRect,
}

/// One view on the minimap.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinimapItem {
    /// The view.
    pub view: ViewId,
    /// Its breakpoint (the accessible name of its rectangle).
    pub breakpoint: String,
    /// The view and its label band.
    pub rect: MapRect,
    /// Shown in the grid area.
    pub visible: bool,
}

/// A rectangle in canvas DIP.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapRect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

/// The canvas board: the page that fills the grid area behind the canvas's views, draws their
/// label strips and takes the drags (a label moves its view, the empty board pans).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardState {
    /// `layoutModes.spacePan`: Space held turns any drag on the board into a pan.
    pub space_pan: bool,
    /// `layoutModes.wheelZoom`: Ctrl + wheel on the board zooms the canvas (RB-080-50).
    #[serde(default)]
    pub wheel_zoom: bool,
    /// The views, in grid order.
    pub items: Vec<BoardItem>,
    /// The camera: what the board's top-left corner shows, so the board turns a click into a
    /// canvas point (RB-080-41).
    #[serde(default)]
    pub camera: BoardCamera,
    /// The canvas zoom as the status bar shows it (RB-080-94): the active view's page zoom in
    /// per mille (camera x item zoom; 1000 = 100 %).
    #[serde(default)]
    pub level_permille: u32,
}

/// The canvas camera as the board needs it: the canvas point at the board's top-left corner,
/// rounded, and the zoom in per mille (screen DIP per canvas DIP).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardCamera {
    /// Canvas x at the board's left edge.
    pub x: i32,
    /// Canvas y at the board's top edge.
    pub y: i32,
    /// The zoom in per mille, at least 1.
    pub zoom_permille: u32,
}

impl Default for BoardCamera {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            zoom_permille: 1000,
        }
    }
}

/// One view on the board, in DIP from the board's (the grid area's) top-left corner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardItem {
    /// The view.
    pub view: ViewId,
    /// Its label strip.
    pub label: MapRect,
    /// The view below it.
    pub rect: MapRect,
    /// Shown; a hidden view (not wholly inside the area) shows only its outline on the board.
    pub visible: bool,
    /// The strip's text, as the grid's strips show it: the breakpoint, its size and its
    /// pixel ratio (`Mobile  393 x 852 @3x`, RB-080-22).
    pub text: String,
}

/// Where a drag on the canvas board is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DragPhase {
    /// Still dragging: shown live, nothing kept.
    Move,
    /// Let go: the result is kept.
    End,
    /// Escape (or a lost pointer): everything goes back to where the drag began.
    Cancel,
}

#[cfg(test)]
#[path = "layout_modes_tests.rs"]
mod tests;
