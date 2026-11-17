//! The pieces on the canvas board (RB-090-69): canvas mode draws its labels in a page of its own
//! (`ui/src/canvas/CanvasBoard.vue`), so each board item carries the pieces the grid's strips
//! show - the same [`CellInfo`], built from the same data and `ui.cellLabel` settings - and a
//! click on one does what a click on the strip's piece does ([`run`]). The action is looked up
//! here, from the pieces the core published last, never taken from the page.

use rb_commands::CommandError;
use rb_engine::{ViewHost as _, ViewId};
use rb_ipc::{BoardPiece, LabelPiece, ShellToCore};
use serde_json::json;

use super::{CellInfo, Piece, PieceAction};
use crate::control::Core;
use crate::engines::{RAISE, RESTART};

/// `piece` as the board draws it: its text and its details.
#[must_use]
pub fn board_piece(piece: &Piece) -> BoardPiece {
    BoardPiece {
        text: piece.text.clone(),
        tooltip: piece.tooltip.clone(),
    }
}

/// The `kind` piece of `info`.
#[must_use]
pub fn piece_of(info: &CellInfo, kind: LabelPiece) -> Option<&Piece> {
    match kind {
        LabelPiece::Address => info.address.as_ref(),
        LabelPiece::EngineStatus => info.engine.as_ref(),
    }
}

/// The `kind` piece of `view` among `infos`, as the board draws it; `None` while not shown.
#[must_use]
pub fn board_piece_of(infos: &[CellInfo], view: ViewId, kind: LabelPiece) -> Option<BoardPiece> {
    infos
        .iter()
        .find(|info| info.view == view)
        .and_then(|info| piece_of(info, kind))
        .map(board_piece)
}

/// What a click on the `kind` piece of `view` does; a piece no longer shown makes the view
/// active, as a click on its label.
#[must_use]
pub fn action_of(infos: &[CellInfo], view: ViewId, kind: LabelPiece) -> PieceAction {
    infos
        .iter()
        .find(|info| info.view == view)
        .and_then(|info| piece_of(info, kind))
        .map_or(PieceAction::Activate, |piece| piece.action)
}

/// A click on the board's `kind` piece of `view` (`canvasPiece`).
///
/// # Errors
///
/// [`CommandError::InvalidArgs`] for a view that does not exist.
pub fn click(core: &Core, view: ViewId, kind: LabelPiece) -> Result<(), CommandError> {
    let host = core.host();
    if !host.views().iter().any(|info| info.id == view) {
        return Err(CommandError::InvalidArgs(format!("no view {view}")));
    }
    run(core, view, action_of(&host.cell_infos(), view, kind));
    Ok(())
}

/// Runs `action` for `view` - the grid strip's piece (`cef_app/cell_info.rs`), the board's, the
/// engine badge's menu and the vitals bar (`cef_app/cell_footers.rs`, RB-090-71) alike: make
/// the view active, restart its engine, raise the external windows or mark the element behind a
/// vital.
pub fn run(core: &Core, view: ViewId, action: PieceAction) {
    let (id, args) = match action {
        PieceAction::Activate => {
            if let Err(error) = core.host().set_active_view(view) {
                tracing::warn!(%view, %error, "the clicked view could not be activated");
            }
            return;
        }
        PieceAction::Restart => (RESTART, json!({ "view": view.0 })),
        PieceAction::Raise => (RAISE, json!({})),
        PieceAction::Mark(metric) => {
            // The message the shell's vitals badge sent: the core marks the element.
            let message = ShellToCore::HighlightVital { view, metric };
            if let Some(Err((_, error))) = crate::vitals::ipc(core, &message) {
                tracing::debug!(%view, %error, "no element to mark");
            }
            return;
        }
    };
    core.run_command_async(id, args, move |result| {
        if let Err(error) = result {
            tracing::warn!(%view, command = id, %error, "the label strip's command failed");
        }
    });
}

#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
