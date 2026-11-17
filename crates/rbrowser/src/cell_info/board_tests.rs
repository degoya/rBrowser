use rb_config::CellAddress;
use rb_ipc::{BoardItem, ShellToCore, ViewMetrics};
use serde_json::json;

use super::*;
use crate::cell_info::{Piece, infos_of, publish};
use crate::layout::test_support::{config, setup, view_ids};

fn select_board(core: &Core) {
    crate::presets::run_command(core, "layout.select_preset", &json!({ "id": "board" }))
        .expect("a presets command")
        .expect("selects");
}

fn lcp(view: ViewId) -> ViewMetrics {
    serde_json::from_value(json!({
        "view": view.0,
        "vitals": [{ "metric": "lcp", "value": 4200.0, "rating": "poor", "highlight": true }]
    }))
    .expect("metrics")
}

/// The board's item of `view`.
fn board_item(core: &Core, view: ViewId) -> BoardItem {
    let host = core.host();
    let state = crate::layout::ipc::mode_state(host, &host.shell_state().modes).expect("a mode");
    state
        .board
        .expect("the canvas has a board")
        .items
        .into_iter()
        .find(|item| item.view == view)
        .expect("the view is on the board")
}

fn piece(text: &str, action: PieceAction) -> Option<Piece> {
    Some(Piece {
        text: text.to_owned(),
        tooltip: format!("{text} details"),
        action,
    })
}

// RB-090-69: canvas mode draws its labels on the board; they show the very pieces the grid's
// strips show, from the same data and the same `ui.cellLabel` settings.
#[tokio::test]
async fn the_board_shows_the_pieces_the_strips_show() {
    let setup = setup(config());
    let ids = view_ids(&setup);
    let core = &setup.core;
    select_board(core);
    let mut user = core.user_settings();
    user.ui.cell_label.address = CellAddress::Always;
    core.replace_user_settings(user);
    core.host()
        .update_shell_state(|shell| shell.metrics = vec![lcp(ids[1])]);
    publish(core);
    let strips = infos_of(core);
    for view in &ids {
        let strip = strips
            .iter()
            .find(|info| info.view == *view)
            .expect("the strip's pieces");
        let item = board_item(core, *view);
        assert_eq!(item.address, strip.address.as_ref().map(board_piece));
        assert_eq!(item.engine_status, strip.engine.as_ref().map(board_piece));
    }
    // RB-090-71: the vitals are the vitals bar's below the view, not a piece of the label.
    let item = serde_json::to_value(board_item(core, ids[1])).expect("the item");
    assert!(item.get("vitals").is_none(), "{item}");
}

// A click on a board piece does what a click on the strip's piece does; the core reads it from
// its own pieces, and a piece no longer shown makes the view active as a click on the label.
#[test]
fn a_click_does_what_the_piece_says() {
    let view = ViewId(1);
    let infos = vec![CellInfo {
        view,
        address: None,
        engine: piece("Firefox: closed", PieceAction::Restart),
        engine_in_badge: false,
    }];
    assert_eq!(
        action_of(&infos, view, LabelPiece::EngineStatus),
        PieceAction::Restart
    );
    assert_eq!(
        action_of(&infos, view, LabelPiece::Address),
        PieceAction::Activate
    );
    assert_eq!(
        action_of(&infos, ViewId(9), LabelPiece::EngineStatus),
        PieceAction::Activate
    );
}

#[tokio::test]
async fn a_click_on_a_board_piece_goes_through_the_boards_requests() {
    let setup = setup(config());
    let ids = view_ids(&setup);
    let core = &setup.core;
    select_board(core);
    publish(core);
    let target = ids[2];
    if core.host().active_view() == Some(target) {
        core.host().set_active_view(ids[0]).expect("activates");
    }
    let message = ShellToCore::CanvasPiece {
        view: target,
        piece: LabelPiece::Address,
    };
    let answer = crate::layout::canvas_board::ipc(core, &message).expect("the board's request");
    assert!(answer.is_ok(), "{answer:?}");
    assert_eq!(core.host().active_view(), Some(target));
    let unknown = ShellToCore::CanvasPiece {
        view: ViewId(9999),
        piece: LabelPiece::Address,
    };
    let answer = crate::layout::canvas_board::ipc(core, &unknown).expect("the board's request");
    assert!(answer.is_err(), "a view that does not exist is refused");
}
