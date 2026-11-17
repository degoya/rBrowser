//! The part of the viewport the window image is known to show (RB-090-40, diag13: the loupe cut
//! off by the canvas area's right edge, `owner-loupe-cut-right.png`).

use super::*;
use crate::calibrate::calibrate;
use crate::pixels::Rgb;

/// The display's colours of the owner's machine (diag9).
const MANAGED: [Rgb; 2] = [[231, 70, 251], [124, 248, 59]];
/// A viewport of 1024 x 604 CSS px at DPR 1.5 in an image of its size, and its corners' frames.
const PORT: (f64, f64, f64) = (1024.0, 604.0, 1.5);
const IMAGE: (u32, u32) = (1536, 906);
const CORNERS: [(f64, f64); 4] = [(0.0, 0.0), (996.0, 0.0), (0.0, 588.0), (996.0, 588.0)];

/// The box of the marker whose frame is at `frame`, drawn at 1.5 from the image's corner.
fn drawn(frame: (f64, f64)) -> Band {
    let at = |css: f64| ((css + MARKER_FRAME) * 1.5).round() as u32;
    Band {
        left: at(frame.0),
        right: at(frame.0 + MARKER_SIZE.0),
        top: at(frame.1),
        bottom: at(frame.1 + MARKER_SIZE.1),
        colours: MANAGED,
    }
}

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

fn close(found: Area, wanted: Area) -> bool {
    [
        (found.left, wanted.left),
        (found.top, wanted.top),
        (found.right, wanted.right),
        (found.bottom, wanted.bottom),
    ]
    .iter()
    .all(|(one, other)| (one - other).abs() <= 1.5)
}

#[test]
fn a_view_cut_on_the_right_is_seen_up_to_its_farthest_marker_there() {
    // diag13, 13:20:12: the lg view at 1.125, its right corners cut off by the canvas area; the
    // pair beside the pointer is the farthest right the view is known to be seen.
    let bands = logged(&[
        (2, 2, 27, 14),
        (38, 500, 27, 13),
        (110, 500, 27, 13),
        (2, 719, 27, 14),
    ]);
    let boxes = vec![
        (0.0, 0.0),
        (977.5, 0.0),
        (0.0, 637.34),
        (977.5, 637.34),
        (31.8, 442.2),
        (95.8, 442.2),
    ];
    let (map, _) = calibrate(&bands, &boxes, (1024.0, 653.0, 1.5), (1536, 980)).expect("mapped");
    let area = visible(&map, &bands, &boxes, (1024.0, 653.0));
    let wanted = Area {
        left: 0.0,
        top: 0.0,
        right: 123.8,
        bottom: 653.0,
    };
    assert!(close(area, wanted), "{area:?}");
}

#[test]
fn a_view_before_the_image_s_corner_is_seen_from_the_window_s_edge() {
    // diag13, 13:19:55: the xl view 252 px left of the window at 0.3 - its left corners are
    // before the image, not cut off inside it; the view is seen from 840.8 CSS px.
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
    let (map, _) = calibrate(&bands, &boxes, (2900.0, 1765.0, 1.2), (3480, 2118)).expect("mapped");
    let area = visible(&map, &bands, &boxes, (2900.0, 1765.0));
    let from = -(map.left as f64) / map.scale;
    assert!((from - 840.8).abs() <= 8.0, "{map:?}");
    let wanted = Area {
        left: from,
        top: 0.0,
        right: 2900.0,
        bottom: 1765.0,
    };
    assert!(close(area, wanted), "{area:?}");
}

#[test]
fn a_view_whose_markers_are_all_seen_is_seen_whole() {
    let bands: Vec<Band> = CORNERS.iter().map(|at| drawn(*at)).collect();
    let (map, _) = calibrate(&bands, &CORNERS, PORT, IMAGE).expect("mapped");
    let area = visible(&map, &bands, &CORNERS, (PORT.0, PORT.1));
    let whole = Area {
        left: 0.0,
        top: 0.0,
        right: 1024.0,
        bottom: 604.0,
    };
    assert!(close(area, whole), "{area:?}");
}

#[test]
fn a_view_cut_at_the_bottom_keeps_its_right_side() {
    // The bottom corners cut off, the top right one seen: only the bottom is bounded, by the
    // lowest marker seen (the pair beside the pointer).
    let mut boxes = CORNERS.to_vec();
    boxes.extend([(254.0, 292.0), (318.0, 292.0)]);
    let bands: Vec<Band> = [boxes[0], boxes[1], boxes[4], boxes[5]]
        .iter()
        .map(|at| drawn(*at))
        .collect();
    let (map, _) = calibrate(&bands, &boxes, PORT, IMAGE).expect("mapped");
    let area = visible(&map, &bands, &boxes, (PORT.0, PORT.1));
    let wanted = Area {
        left: 0.0,
        top: 0.0,
        right: 1024.0,
        bottom: 308.0,
    };
    assert!(close(area, wanted), "{area:?}");
}

#[test]
fn without_a_marker_seen_the_image_bounds_it() {
    // RB-090-19: no box at all, the image's size maps it - nothing says where it is cut.
    let (map, _) = calibrate(&[], &CORNERS, PORT, IMAGE).expect("mapped");
    let area = visible(&map, &[], &CORNERS, (PORT.0, PORT.1));
    let whole = Area {
        left: 0.0,
        top: 0.0,
        right: 1024.0,
        bottom: 604.0,
    };
    assert_eq!(area, whole);
}
