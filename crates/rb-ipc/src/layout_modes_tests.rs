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
