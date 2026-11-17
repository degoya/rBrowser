//! RB-090-31: the calibration of diag12 (2026-11-16, Windows, DPR 1.5, the canvas layout at 45 %,
//! page zoom RB-090-20). A window image of the viewport's size at its DPR held the page drawn at
//! the canvas's scale in its top left corner; the image's size was taken for the map and the
//! loupe read the page 2.2 times further right and down. Red before the fix: the wide view was
//! read at 3.75 px per CSS px, the phone not at all.

use super::tests::{Shot, shot};
use super::*;
use crate::shot::mapping;

/// The canvas's zoom of diag12 times the page's DPR at page zoom 250 %: 0.45 x 3.75.
const DRAWN: f64 = 0.45 * 3.75;

/// diag12, view 1 (a 375 px phone at page zoom 250 %, 23:35:34): `the window image of view 1
/// (563 x 992 px for 150 x 264 CSS px at DPR 3.75) shows 4 marker-like boxes but no two of them
/// map the viewport into it at about 3.7533 px per CSS px: boxes [3,3 41x20; 199,3 41x20; 3,413
/// 41x20; 199,413 41x20], markers drawn at [0,0; 116.26667,0; 0,242.66667; 116.26667,242.66667]
/// CSS px`. The four boxes are the markers at 1.6875 px per CSS px from the image's corner.
fn phone() -> Shot {
    let mut view = shot((563, 992), (150.0, 264.0), (0.0, 0.0), DRAWN);
    let (right, bottom) = (116.266_670_227_050_78, 242.666_671_752_929_7);
    view.boxes = vec![(0.0, 0.0), (right, 0.0), (0.0, bottom), (right, bottom)];
    view
}

/// diag12, view 8 (2320 px wide at page zoom 250 %, 23:34:49 to 23:35:22), the owner's wrong
/// pick: `view 8 is read from its window image (3480 x 2171 px for 928 x 579 CSS px at DPR 3.75):
/// the viewport at 3.7500 px per CSS px from (0, 0) by the image's size` - the page drawn at
/// 1.6875 in the image's top left 1566 x 977 px, the rest no page.
fn wide() -> Shot {
    let mut view = shot((3480, 2171), (928.0, 579.0), (0.0, 0.0), DRAWN);
    let right = 894.133_361_816_406_3;
    view.boxes = vec![(0.0, 0.0), (right, 0.0), (0.0, 563.0), (right, 563.0)];
    view
}

#[test]
fn the_phone_s_four_markers_map_it_at_the_canvas_s_scale() {
    let view = phone();
    assert_eq!(view.bands().len(), 4, "{:?}", view.bands());
    let (map, _) = view.calibrate(3.75).expect("mapped by its markers");
    assert!((map.scale - DRAWN).abs() < 0.01, "{}", map.scale);
    assert!(map.left.abs() <= 1 && map.top.abs() <= 1, "{map:?}");
    assert_eq!((map.width, map.height), (563, 992));
}

#[test]
fn the_wide_view_is_read_where_its_page_is_drawn_not_at_its_dpr() {
    let view = wide();
    let (map, _) = view.calibrate(3.75).expect("mapped by its markers");
    assert!((map.scale - DRAWN).abs() < 0.01, "{}", map.scale);
    assert!(map.left.abs() <= 1 && map.top.abs() <= 1, "{map:?}");
    // The cursor on the fingers (the owner's screenshot): inside the page's 1566 x 977 px, where
    // the image's size put it at (2645, 1041), in the white beyond the page.
    let (x, y) = mapping(&map).point((705.0, 277.0));
    assert!(
        (1180..=1200).contains(&x) && (460..=476).contains(&y),
        "{x}, {y}"
    );
}

#[test]
fn an_image_of_the_viewport_s_size_whose_marker_is_elsewhere_is_not_read() {
    // One marker seen, drawn smaller than the image's size puts it: not the viewport at its DPR,
    // and one marker maps nothing - the status bar says so rather than show a wrong colour.
    let mut view = wide();
    view.hidden = vec![1, 2, 3];
    assert_eq!(view.bands().len(), 1);
    let refused = view.calibrate(3.75).expect_err("not mapped");
    assert!(
        refused.contains("shows 1 of the 4 markers, not where the image's size puts one"),
        "{refused}"
    );
}

#[test]
fn an_image_of_the_viewport_s_size_without_a_marker_is_still_mapped_by_it() {
    // RB-090-19 (diag11) holds: no box seen, nothing denies the image's size.
    let mut view = wide();
    view.hidden = vec![0, 1, 2, 3];
    let (map, exact) = view.calibrate(3.75).expect("mapped by its size");
    assert_eq!((map.left, map.top, map.scale), (0, 0, 3.75));
    assert!(!exact);
}

#[test]
fn a_viewport_of_whole_css_px_at_a_page_zoom_has_the_image_s_size() {
    // `3480 x 2171 px for 1546 x 965 CSS px at DPR 2.25` (23:34:13): 3480 / 2.25 = 1546.67, the
    // page reports whole CSS px - the image is up to one of them off, not one image pixel.
    assert!(direct((1546.0, 965.0, 2.25), (3480, 2171)).is_some());
    assert!(direct((150.0, 264.0, 3.75), (563, 992)).is_some());
    // diag11's `567 x 746 px for 375 x 497` stays another size.
    assert!(direct((375.0, 497.0, 1.5), (567, 746)).is_none());
    // Below a DPR of 1 one image pixel.
    assert!(direct((7680.0, 4277.0, 0.375), (2880, 1604)).is_some());
    assert!(direct((7680.0, 4277.0, 0.375), (2882, 1604)).is_none());
}
