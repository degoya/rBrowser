//! RB-090-40, the owner's acceptance of 2026-11-16 (beta build, Windows, display DPR 1.5; diag13,
//! `logs/rbrowser.log`, 13:15-13:21Z). The boxes are the log's own (`[x,y wxh; ...]`), the
//! markers' frames read back from them at the scale the boxes' sizes and spans give. Red before
//! the fix: at canvas zoom 75 % an image of the viewport at its DPR was mapped by its size,
//! because the corner box of a page drawn at 1.125 is close enough in place and size to the
//! marker 1.5 puts there (`1.5000 px per CSS px from (-6, -3)`: the loupe 1.33 times too far); at
//! 25 % a view cut on the left was mapped from the corner by two of its four boxes (`from (604,
//! 1)`), because a map before the image's corner was taken only at the image's own scale; two
//! corners that a pair of either side explains alike were taken for the left ones.

use super::clipped_tests::{host, near, pointed, request, truth};
use super::tests::{MANAGED, shot};
use super::*;
use crate::page::handle;
use crate::shot::mapping;

/// The boxes as the log lists them: `x,y wxh`.
fn logged(boxes: &[(u32, u32, u32, u32)]) -> Vec<Band> {
    boxes
        .iter()
        .map(|&(left, top, width, height)| Band {
            left,
            right: left + width,
            top,
            bottom: top + height,
            colours: MANAGED,
        })
        .collect()
}

/// The view `lg` (1024 x 653 CSS px at DPR 1.5) in a window image of 1536 x 980 px, drawn at
/// canvas zoom 75 % (1.125 px per CSS px) and cut on the right by the canvas area: its corners
/// (the right ones 46.5 CSS px in for the scroll bar, the bottom ones 15.66), the pair beside the
/// pointer at `pair`.
fn lg(pair: (f64, f64)) -> Marker {
    vec![
        (0.0, 0.0),
        (977.5, 0.0),
        (0.0, 637.34),
        (977.5, 637.34),
        pair,
        (pair.0 + 64.0, pair.1),
    ]
}

const LG_PORT: (f64, f64, f64) = (1024.0, 653.0, 1.5);
const LG_IMAGE: (u32, u32) = (1536, 980);

#[test]
fn diag13_s_lg_view_at_canvas_zoom_75_is_read_at_1_125_not_by_the_image_s_size() {
    // 13:20:12 (the owner's screen `owner-loupe-displaced-75.png`) and 13:15:27.
    let reads = [
        (
            logged(&[
                (2, 2, 27, 14),
                (38, 500, 27, 13),
                (110, 500, 27, 13),
                (2, 719, 27, 14),
            ]),
            lg((31.8, 442.2)),
            (78.0, 450.0),
        ),
        (
            logged(&[
                (2, 2, 27, 14),
                (107, 227, 27, 14),
                (179, 227, 27, 14),
                (2, 719, 27, 14),
            ]),
            lg((93.1, 200.0)),
            (139.0, 208.0),
        ),
    ];
    for (bands, boxes, pointer) in reads {
        let (map, _) = calibrate(&bands, &boxes, LG_PORT, LG_IMAGE).expect("mapped");
        assert!((map.scale - 1.125).abs() < 0.01, "{map:?}");
        assert!(map.left.abs() <= 1 && map.top.abs() <= 1, "{map:?}");
        // The pick under the pointer: the image pixel in the middle of its CSS pixel.
        let wanted = (
            ((pointer.0 + 0.5_f64) * 1.125).floor() as i64,
            ((pointer.1 + 0.5_f64) * 1.125).floor() as i64,
        );
        let found = mapping(&map).point(pointer);
        assert!(near(found, wanted), "{pointer:?}: {found:?} for {wanted:?}");
    }
}

#[test]
fn diag13_s_mobile_view_at_canvas_zoom_75_is_read_at_0_9_not_by_the_image_s_size() {
    // 13:15:31: `563 x 1022 px for 469 x 852 CSS px at DPR 1.2` read `at 1.2000 px per CSS px
    // from (-5, -3)`; its six boxes, 22 x 10 px, are markers at 0.9 = 0.75 x 1.2.
    let bands = logged(&[
        (1, 2, 22, 10),
        (381, 2, 22, 10),
        (299, 104, 23, 10),
        (357, 104, 22, 10),
        (1, 754, 22, 10),
        (381, 754, 22, 10),
    ]);
    let boxes = vec![
        (0.0, 0.0),
        (422.67, 0.0),
        (0.0, 836.34),
        (422.67, 836.34),
        (331.0, 113.1),
        (395.0, 113.1),
    ];
    let (map, _) = calibrate(&bands, &boxes, (469.0, 852.0, 1.2), (563, 1022)).expect("mapped");
    assert!((map.scale - 0.9).abs() < 0.01, "{map:?}");
    assert!(map.left.abs() <= 1 && map.top.abs() <= 1, "{map:?}");
}

/// 13:19:55, canvas zoom 25 %: the view `xl` (2900 x 1765 CSS px at DPR 1.2) in a window image
/// of 3480 x 2118 px, drawn at 0.3 px per CSS px and beginning 252 px left of the window - its
/// right corners at x = 604 and the pair beside the pointer left of them, inside the viewport
/// only for a map before the image's corner.
fn xl() -> (Vec<Band>, Marker) {
    let bands = logged(&[
        (604, 2, 8, 3),
        (550, 230, 8, 3),
        (570, 230, 7, 3),
        (604, 527, 8, 3),
    ]);
    let boxes = vec![
        (0.0, 0.0),
        (2853.5, 0.0),
        (0.0, 1749.34),
        (2853.5, 1749.34),
        (2673.5, 760.0),
        (2737.5, 760.0),
    ];
    (bands, boxes)
}

const XL_PORT: (f64, f64, f64) = (2900.0, 1765.0, 1.2);
const XL_IMAGE: (u32, u32) = (3480, 2118);

#[test]
fn diag13_s_xl_view_cut_on_the_left_at_canvas_zoom_25_is_read_before_the_image_s_corner() {
    // The log's map, `0.3001 px per CSS px from (604, 1)`, put the pick 856 px right of the
    // point: two of the four boxes agree with it, all four with the one 252 px before the corner.
    let (bands, boxes) = xl();
    let (map, _) = calibrate(&bands, &boxes, XL_PORT, XL_IMAGE).expect("mapped");
    assert!((map.scale - 0.3).abs() < 0.005, "{map:?}");
    assert!((map.left + 252).abs() <= 2 && map.top.abs() <= 2, "{map:?}");
    let pointer = (2719.5, 768.0);
    let found = mapping(&map).point(pointer);
    assert!((found.0 - 564).abs() <= 2, "{found:?}");
}

#[test]
fn two_corners_that_either_side_s_pair_explains_are_refused() {
    // The xl view's two right corners alone (the pointer not in it yet): its left corners from
    // the image's corner or its right ones 252 px before it - a guess either way.
    let (bands, boxes) = xl();
    let corners_only = [bands[0], bands[3]];
    let refused = calibrate(&corners_only, &boxes[..4], XL_PORT, XL_IMAGE).expect_err("ambiguous");
    assert!(refused.contains("two places"), "{refused}");
}

#[test]
fn diag13_s_200_percent_refusal_is_a_stale_image() {
    // 13:18:37, 1.4 s after canvas zoom 200 %: `563 x 1046 px for 469 x 872 CSS px at DPR 1.2
    // shows 1 of the 6 markers`, the box 58 x 28 px a marker at 2.4 - the page drawn at the new
    // scale into the view's old 563 px (the screen shows it drawn only that wide until a pan).
    // One marker maps nothing: refused, as it must be; the picker has nothing to fix there.
    let bands = logged(&[(402, 482, 58, 28)]);
    let boxes = vec![
        (0.0, 0.0),
        (422.67, 0.0),
        (0.0, 856.34),
        (422.67, 856.34),
        (166.08, 321.58),
        (230.08, 321.58),
    ];
    let refused = calibrate(&bands, &boxes, (469.0, 872.0, 1.2), (563, 1046)).expect_err("stale");
    assert!(refused.contains("shows 1 of the 6 markers"), "{refused}");
}

#[test]
fn a_view_drawn_at_75_percent_and_cut_on_the_right_reads_its_visible_points() {
    // The 13:20:12 case drawn: the canvas area cuts the view 280 px into the image.
    let at = (78.0, 450.0);
    let mut view = pointed(shot(LG_IMAGE, (1024.0, 653.0), (0.0, 0.0), 1.125), at);
    view.window = Some([0.0, 0.0, 280.0, 980.0]);
    let (map, _) = view.calibrate(1.5).expect("mapped");
    assert!((map.scale - 1.125).abs() < 0.01, "{map:?}");
    for point in [at, (20.0, 100.0), (200.0, 600.0)] {
        let found = mapping(&map).point(point);
        assert!(near(found, truth(&view, point)), "{point:?}: {found:?}");
    }
}

#[test]
fn the_calibrate_answer_says_where_the_cut_view_is_seen() {
    // `owner-loupe-cut-right.png`: the loupe went right of the pointer, past the canvas area's
    // edge. The answer names the part of the viewport known to be seen: up to the pair beside
    // the pointer, not the viewport's 1024 CSS px.
    let at = (150.0, 450.0);
    let mut view = pointed(shot(LG_IMAGE, (1024.0, 653.0), (0.0, 0.0), 1.125), at);
    view.window = Some([0.0, 0.0, 280.0, 980.0]);
    let mut host = host(&view);
    let answer = handle(&mut host, &request(&view, 1.5, true)).expect("answered");
    let scale = answer["map"]["scale"].as_f64().expect("a map");
    assert!((scale - 1.125).abs() < 0.01, "{answer}");
    let seen = &answer["visible"];
    let right = seen["right"].as_f64().expect("visible");
    assert!((right - (at.0 + 46.0)).abs() <= 1.5, "{answer}");
    assert_eq!(seen["left"].as_f64(), Some(0.0), "{answer}");
}
