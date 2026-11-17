use super::*;
use serde_json::json;

#[test]
fn the_state_is_camel_case_and_leaves_out_what_is_unset() {
    let state = ModeState {
        split: Some(SplitState {
            ratio_permille: 600,
            vertical: true,
            step_permille: 50,
        }),
        ..ModeState::default()
    };
    assert_eq!(
        serde_json::to_value(&state).expect("serializes"),
        json!({ "split": { "ratioPermille": 600, "vertical": true, "stepPermille": 50 } })
    );
}

#[test]
fn the_minimap_reads_back() {
    let value = json!({ "canvas": {
        "position": "start",
        "items": [{ "view": 3, "breakpoint": "mobile",
                    "rect": { "x": -8, "y": 0, "width": 200, "height": 420 }, "visible": false }],
        "viewport": { "x": 0, "y": 0, "width": 1600, "height": 900 }
    } });
    let state: ModeState = serde_json::from_value(value).expect("reads");
    let canvas = state.canvas.expect("minimap");
    assert_eq!(canvas.position, MinimapPosition::Start);
    assert_eq!(canvas.items[0].view, ViewId(3));
    assert_eq!(canvas.items[0].rect.x, -8);
}

#[test]
fn the_board_lists_each_label_on_the_board() {
    let state = ModeState {
        board: Some(BoardState {
            space_pan: true,
            wheel_zoom: true,
            camera: BoardCamera {
                x: -40,
                y: 0,
                zoom_permille: 500,
            },
            level_permille: 250,
            items: vec![BoardItem {
                view: ViewId(2),
                label: MapRect {
                    x: 10,
                    y: -4,
                    width: 200,
                    height: 24,
                },
                rect: MapRect {
                    x: 10,
                    y: 20,
                    width: 200,
                    height: 400,
                },
                visible: false,
                text: "Mobile  393 \u{d7} 852 @3x".to_owned(),
                page_zoom: None,
                address: None,
                engine_status: None,
            }],
        }),
        ..ModeState::default()
    };
    let value = serde_json::to_value(&state).expect("serializes");
    assert_eq!(
        value,
        json!({ "board": { "spacePan": true, "wheelZoom": true, "items": [{
            "view": 2,
            "label": { "x": 10, "y": -4, "width": 200, "height": 24 },
            "rect": { "x": 10, "y": 20, "width": 200, "height": 400 },
            "visible": false,
            "text": "Mobile  393 \u{d7} 852 @3x"
        }], "camera": { "x": -40, "y": 0, "zoomPermille": 500 }, "levelPermille": 250 } })
    );
    let back: ModeState = serde_json::from_value(value).expect("reads back");
    assert_eq!(back, state);
    // A board of an older core has no level: 0.
    let old = json!({ "spacePan": false, "items": [] });
    let board: BoardState = serde_json::from_value(old).expect("reads");
    assert_eq!(board.level_permille, 0);
}

#[test]
fn a_drag_phase_is_lower_case() {
    for (phase, text) in [
        (DragPhase::Move, "move"),
        (DragPhase::End, "end"),
        (DragPhase::Cancel, "cancel"),
    ] {
        assert_eq!(
            serde_json::to_value(phase).expect("serializes"),
            json!(text)
        );
    }
}

#[test]
fn the_board_requests_parse() {
    assert_eq!(
        crate::parse_request(
            r#"{ "type": "canvasDrag", "view": 3, "dx": -12, "dy": 40, "phase": "move" }"#
        )
        .expect("parses"),
        crate::ShellToCore::CanvasDrag {
            view: ViewId(3),
            dx: -12,
            dy: 40,
            phase: DragPhase::Move,
        }
    );
    assert_eq!(
        crate::parse_request(r#"{ "type": "canvasPan", "dx": 5, "dy": 0, "phase": "end" }"#)
            .expect("parses"),
        crate::ShellToCore::CanvasPan {
            dx: 5,
            dy: 0,
            phase: DragPhase::End,
        }
    );
    assert_eq!(
        crate::parse_request(r#"{ "type": "canvasStep", "view": 1, "dx": 0, "dy": -1 }"#)
            .expect("parses"),
        crate::ShellToCore::CanvasStep {
            view: ViewId(1),
            dx: 0,
            dy: -1,
        }
    );
    assert_eq!(
        crate::parse_request(r#"{ "type": "canvasSizePick", "view": 2 }"#).expect("parses"),
        crate::ShellToCore::CanvasSizePick {
            view: ViewId(2),
            force: false
        }
    );
    assert!(
        crate::parse_request(
            r#"{ "type": "canvasDrag", "view": 3, "dx": 1.5, "dy": 0, "phase": "move" }"#
        )
        .is_err(),
        "a delta is whole DIP"
    );
    assert!(
        crate::parse_request(r#"{ "type": "canvasPan", "dx": 1, "dy": 0, "phase": "fling" }"#)
            .is_err()
    );
}

// RB-090-28: a label on the board shows its view's page zoom like a grid strip's badge - the
// percent while the page is zoomed, nothing at 100 % (and from a core without the field).
#[test]
fn a_board_item_carries_its_page_zoom_while_zoomed() {
    let item = BoardItem {
        view: ViewId(3),
        label: MapRect {
            x: 0,
            y: 0,
            width: 200,
            height: 24,
        },
        rect: MapRect {
            x: 0,
            y: 24,
            width: 200,
            height: 400,
        },
        visible: true,
        text: "Tablet".to_owned(),
        page_zoom: Some(110),
        address: None,
        engine_status: None,
    };
    let value = serde_json::to_value(&item).expect("serializes");
    assert_eq!(value["pageZoom"], json!(110));
    let back: BoardItem = serde_json::from_value(value.clone()).expect("reads back");
    assert_eq!(back, item);
    let mut unzoomed = value;
    unzoomed
        .as_object_mut()
        .expect("an object")
        .remove("pageZoom");
    let back: BoardItem = serde_json::from_value(unzoomed).expect("reads without the field");
    assert_eq!(back.page_zoom, None);
    let shown = serde_json::to_value(&back).expect("serializes");
    assert!(shown.get("pageZoom").is_none(), "{shown}");
}

// RB-090-69: the board's label shows the pieces a grid strip shows beside the name - the
// address and the external engine's window (the Web Vitals went to the vitals bar, RB-090-71) -
// each with its text and its details; a piece not shown is left out (and a core without the
// fields reads as none).
#[test]
fn a_board_item_carries_the_strip_pieces_it_shows() {
    let item: BoardItem = serde_json::from_value(json!({
        "view": 3,
        "label": { "x": 0, "y": 0, "width": 200, "height": 24 },
        "rect": { "x": 0, "y": 24, "width": 200, "height": 400 },
        "visible": true,
        "text": "Tablet"
    }))
    .expect("reads without the pieces");
    assert_eq!((&item.address, &item.engine_status), (&None, &None));
    let item = BoardItem {
        engine_status: Some(BoardPiece {
            text: "Firefox: closed".to_owned(),
            tooltip: "Click to open it again".to_owned(),
        }),
        ..item
    };
    let value = serde_json::to_value(&item).expect("serializes");
    assert_eq!(value["engineStatus"]["text"], json!("Firefox: closed"));
    assert!(value.get("address").is_none(), "{value}");
    let back: BoardItem = serde_json::from_value(value).expect("reads back");
    assert_eq!(back, item);
}

// RB-090-69: a click on a piece of the board's label names the view and the piece; the core
// looks up what it does.
#[test]
fn a_click_on_a_board_piece_names_the_view_and_the_piece() {
    for (name, piece) in [
        ("address", LabelPiece::Address),
        ("engineStatus", LabelPiece::EngineStatus),
    ] {
        assert_eq!(
            crate::parse_request(&format!(
                r#"{{ "type": "canvasPiece", "view": 2, "piece": "{name}" }}"#
            ))
            .expect("parses"),
            crate::ShellToCore::CanvasPiece {
                view: ViewId(2),
                piece
            }
        );
    }
    assert!(
        crate::parse_request(r#"{ "type": "canvasPiece", "view": 2, "piece": "zoom" }"#).is_err()
    );
}
