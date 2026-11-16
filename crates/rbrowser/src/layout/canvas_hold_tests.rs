use rb_engine::{CellRect, ViewId};

use super::*;

fn held(number: u32, x: i32) -> Held {
    Held {
        number,
        area: CellRect {
            x: 0,
            y: 44,
            width: 1600,
            height: 900,
        },
        placed: vec![PlacedView {
            view: ViewId(1),
            outer: CellRect {
                x,
                y: 60,
                width: 200,
                height: 420,
            },
            visible: true,
        }],
    }
}

#[test]
fn each_canvas_gets_a_number_of_its_own() {
    let mut hold = CanvasHold::default();
    let first = hold.number();
    let second = hold.number();
    assert_ne!(first, second);
    hold.next = u32::MAX;
    assert_eq!(hold.number(), 0, "the numbers wrap and never panic");
}

// RB-090-55: the views go where the board drew their frames - the canvas it painted, not a
// newer one still on its way, and never an older one after a newer one.
#[test]
fn the_canvas_painted_is_placed_and_every_older_one_is_dropped() {
    let mut hold = CanvasHold::default();
    for (number, x) in [(1, 10), (2, 20), (3, 30)] {
        hold.hold(held(number, x));
    }
    assert_eq!(hold.painted(2), Some(held(2, 20)));
    assert_eq!(hold.painted(1), None, "older than the one placed");
    assert_eq!(hold.painted(2), None, "placed once");
    assert_eq!(hold.painted(9), None, "never held");
    assert_eq!(hold.painted(3), Some(held(3, 30)));
    assert_eq!(hold.painted(3), None);
}

#[test]
fn a_board_that_falls_behind_keeps_only_the_newest_canvases() {
    let mut hold = CanvasHold::default();
    let count = u32::try_from(MAX_HELD).expect("a small number") + 3;
    for number in 1..=count {
        hold.hold(held(number, 0));
    }
    assert_eq!(hold.held.len(), MAX_HELD);
    assert_eq!(hold.painted(1), None, "the oldest went first");
    assert!(hold.painted(count).is_some());
}

#[test]
fn a_camera_kept_or_put_back_ends_the_trial_and_every_hold() {
    let mut hold = CanvasHold::default();
    let tried = Camera {
        x: 5.0,
        y: 6.0,
        zoom: 0.25,
    };
    assert_eq!(hold.live(), None);
    hold.try_out(tried);
    hold.hold(held(1, 0));
    assert_eq!(hold.live(), Some(tried));
    hold.settle();
    assert_eq!(hold.live(), None);
    assert_eq!(
        hold.painted(1),
        None,
        "a frame painted after the end moves nothing"
    );
}
