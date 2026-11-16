//! RB-090-40: a view the window's edge or the canvas area cuts (owner, 2026-11-16, test ZIP
//! `49c267d`, Windows, DPR 1.5; diag12 23:34-23:41Z). The window image is the window's pixels in
//! the view's rectangle, that rectangle moved to start at the window's top left corner where the
//! view begins above or left of it; where the view is cut off it shows the toolbar or the board.
//! diag12, view 6 at 23:34:37 (its place `x: 738, y: -45` with a label of 20 DIP: the view 25 DIP
//! above the window): `the window image of view 6 (1296 x 807 px for 768 x 478 CSS px at DPR
//! 3.75) shows 1 of the 4 markers: boxes [3,747 41x20]` - the bottom left marker, which the
//! image's scale of 1.6875 puts at 784, 37 px (25 DIP) higher. Red before the fix: a map may not
//! start before the image's corner (`pair_map`), the page script drew no markers where the view
//! is seen, and an image of the viewport at its DPR was read by its size from its top rows alone.

use serde_json::{Value, json};

use super::tests::{MANAGED, Shot, shot};
use crate::page::handle;
use crate::pixels::tests::png_base64;
use crate::shot::mapping;
use crate::testing::FakeHost;

/// The two markers page/markers.js draws beside the pointer at `at`: frames 64 CSS px apart, the
/// pair centred on it and kept inside the viewport `port`.
fn beside(port: (f64, f64), at: (f64, f64)) -> [(f64, f64); 2] {
    let x = (at.0 - 46.0).clamp(0.0, port.0 - 92.0);
    let y = (at.1 - 8.0).clamp(0.0, port.1 - 16.0);
    [(x, y), (x + 64.0, y)]
}

/// `view` with the two markers beside the pointer at `at` after its four corners.
pub(super) fn pointed(mut view: Shot, at: (f64, f64)) -> Shot {
    view.boxes.extend(beside(view.port, at));
    view
}

/// The image pixel the shot shows the CSS pixel `at` at: what the pick must read.
pub(super) fn truth(view: &Shot, at: (f64, f64)) -> (i64, i64) {
    let axis = |css: f64, offset: f64| ((css.floor() + 0.5) * view.scale + offset).floor() as i64;
    (axis(at.0, view.offset.0), axis(at.1, view.offset.1))
}

/// Whether two image points are at most a pixel apart: a scaled marker's edges blend, its middle
/// is known to half a pixel.
pub(super) fn near(found: (i64, i64), wanted: (i64, i64)) -> bool {
    (found.0 - wanted.0).abs() <= 1 && (found.1 - wanted.1).abs() <= 1
}

/// diag12, view 6 at 23:34:37: 25 DIP above the window (37 px at 1.5), only 77 DIP of it inside
/// the canvas area on the right (115 px), the toolbar and the area's top edge over its top 70 px.
fn view_6() -> Shot {
    let mut view = shot((1296, 807), (768.0, 478.0), (0.0, -37.0), 1296.0 / 768.0);
    view.window = Some([0.0, 70.0, 115.0, 807.0]);
    view
}

#[test]
fn diag12_s_view_6_with_one_corner_seen_is_still_refused_by_its_corners_alone() {
    // What the owner saw: one box, nothing to map by - the status bar, never a wrong colour.
    let view = view_6();
    assert_eq!(view.bands().len(), 1, "{:?}", view.bands());
    let refused = view.calibrate(3.75).expect_err("not mapped");
    assert!(refused.contains("shows 1 of the 4 markers"), "{refused}");
}

#[test]
fn diag12_s_view_6_is_mapped_by_a_corner_and_the_markers_beside_the_pointer() {
    let at = (40.0, 300.0);
    let view = pointed(view_6(), at);
    let (map, _) = view.calibrate(3.75).expect("mapped");
    assert!((map.scale - 1296.0 / 768.0).abs() < 1e-9, "{map:?}");
    assert!(near((map.left, map.top), (0, -37)), "{map:?}");
    for point in [at, (5.0, 120.0), (60.0, 470.0)] {
        let found = mapping(&map).point(point);
        assert!(near(found, truth(&view, point)), "{point:?}: {found:?}");
    }
}

#[test]
fn a_view_cut_on_any_side_reads_its_visible_points() {
    // (cut, image, viewport, DPR, drawn scale, offset, the window's part of the image, pointer)
    let own = |image: u32, port: f64| f64::from(image) / port;
    let cases = [
        // diag12, view 8 at 23:37:39 (canvas 60 %, page zoom 125 %): `2085 x 1308 px for 1856 x
        // 1164 CSS px at DPR 1.875 shows 1 of the 2 markers: boxes [182,2 28x13]` - its top right
        // marker, 1861 px left of where the image's size puts it: the view begins 1241 DIP left
        // of the window.
        (
            "left, canvas 60 %, page zoom",
            (2085, 1308),
            (1856.0, 1164.0),
            1.875,
            own(2085, 1856.0),
            (-1861.0, 0.0),
            [5.0, 0.0, 224.0, 1308.0],
            (1760.0, 500.0),
        ),
        (
            "right and bottom, canvas 45 %",
            (1296, 807),
            (768.0, 478.0),
            3.75,
            own(1296, 768.0),
            (0.0, 0.0),
            [0.0, 0.0, 600.0, 500.0],
            (200.0, 150.0),
        ),
        (
            "top, the grid scrolled, 100 %",
            (1536, 1050),
            (1024.0, 700.0),
            1.5,
            1.5,
            (0.0, -60.0),
            [0.0, 70.0, 1536.0, 1050.0],
            (500.0, 400.0),
        ),
        (
            "top and left, canvas 100 %, page zoom 150 %",
            (3480, 2171),
            (1546.0, 965.0),
            2.25,
            2.25,
            (-900.0, -300.0),
            [0.0, 0.0, 3480.0, 2171.0],
            (900.0, 600.0),
        ),
    ];
    for (cut, image, port, dpr, scale, offset, window, at) in cases {
        let mut view = pointed(shot(image, port, offset, scale), at);
        view.window = Some(window);
        let (map, _) = view
            .calibrate(dpr)
            .unwrap_or_else(|why| panic!("{cut}: {why}"));
        assert_eq!((map.width, map.height), image, "{cut}");
        let wanted = (offset.0 as i64, offset.1 as i64);
        assert!(near((map.left, map.top), wanted), "{cut}: {map:?}");
        // The pointer and points around it, all where the view is seen.
        for point in [at, (at.0 - 30.0, at.1 + 40.0), (at.0 + 20.0, at.1 - 50.0)] {
            let found = mapping(&map).point(point);
            assert!(
                near(found, truth(&view, point)),
                "{cut}: {point:?}: {found:?}"
            );
        }
    }
}

#[test]
fn a_few_pixels_cut_off_an_image_of_the_viewport_at_its_dpr_are_not_read_as_its_corner() {
    // Its bottom markers 3 px higher are near enough to agree with the image's size; the pick
    // would read 3 px too low.
    let mut view = shot((1536, 1050), (1024.0, 700.0), (0.0, -3.0), 1.5);
    view.window = Some([0.0, 0.0, 1536.0, 1050.0]);
    view.hidden = vec![0, 1];
    let (map, _) = view.calibrate(1.5).expect("mapped");
    assert_eq!((map.left, map.top, map.scale), (0, -3, 1.5));
}

#[test]
fn a_cut_view_drawn_smaller_than_its_image_is_read_before_its_corner() {
    // diag13 (13:19:55, canvas zoom 25 %, `calibrate_diag13_tests.rs`): the window moves the
    // rectangle of a view drawn smaller than its image too. The earlier rule - a map before the
    // image's corner only at the image's own scale - read that view from the corner by two of its
    // four boxes. Two corners and the pair beside the pointer at 0.8 of the image's scale, 300 px
    // left of the image: all four agree with the map before the corner.
    let scale = 0.8 * 1296.0 / 768.0;
    let at = (400.0, 200.0);
    let mut view = pointed(shot((1296, 807), (768.0, 478.0), (-300.0, 0.0), scale), at);
    view.window = Some([0.0, 0.0, 1296.0, 807.0]);
    assert!(view.bands().len() >= 2, "{:?}", view.bands());
    let (map, _) = view.calibrate(3.75).expect("mapped");
    assert!(near((map.left, map.top), (-300, 0)), "{map:?}");
    for point in [at, (700.0, 50.0), (500.0, 450.0)] {
        let found = mapping(&map).point(point);
        assert!(near(found, truth(&view, point)), "{point:?}: {found:?}");
    }
}

/// The calibrate request of view 4 for `view`'s viewport and markers.
pub(super) fn request(view: &Shot, dpr: f64, pointed: bool) -> Value {
    json!({ "view": 4, "message": {
        "type": "calibrate", "x": 500, "y": 400, "width": view.port.0, "height": view.port.1,
        "dpr": dpr, "scrollX": 0, "scrollY": 0, "pointed": pointed,
        "marker": view.boxes.iter().map(|(x, y)| [*x, *y]).collect::<Vec<_>>(),
    } })
}

pub(super) fn host(view: &Shot) -> FakeHost {
    let mut host = FakeHost::default();
    host.view(4, "chromium");
    let data = png_base64(view.image.0, view.image.1, |x, y| view.pixel(x, y));
    host.answer("Page.captureScreenshot", json!({ "data": data }));
    host
}

#[test]
fn a_grid_view_scrolled_above_the_window_is_not_read_by_its_size() {
    // Red before the fix: only the image's top rows were searched - the toolbar there, no box -
    // and its size mapped it from its corner, every pick 60 px below the point.
    let mut view = shot((1536, 1050), (1024.0, 700.0), (0.0, -60.0), 1.5);
    view.window = Some([0.0, 70.0, 1536.0, 1050.0]);
    let mut host = host(&view);
    let answer = handle(&mut host, &request(&view, 1.5, false)).expect("answered");
    assert_eq!(
        answer["map"],
        json!({ "left": 0, "top": -60, "scale": 1.5, "width": 1536, "height": 1050 }),
        "{answer}"
    );
    assert_eq!(answer["exact"], false);
    // One window image, no clip.
    assert_eq!(host.cdp_sent.len(), 1);
    assert!(host.cdp_sent[0].2.get("clip").is_none());
}

#[test]
fn without_a_marker_or_the_pointer_in_the_view_its_size_maps_nothing() {
    // The view cut above and below: no corner seen. Without the person's pointer in it (the
    // pipette's start, the active view at its middle) the image's size could be 60 px off: the
    // page script measures again where the pointer comes in.
    let mut view = shot((1536, 1050), (1024.0, 700.0), (0.0, -60.0), 1.5);
    view.window = Some([0.0, 70.0, 1536.0, 900.0]);
    let mut host = host(&view);
    let answer = handle(&mut host, &request(&view, 1.5, false)).expect("answered");
    assert_eq!(answer, json!({ "map": null, "reason": "markers" }));
    assert!(
        host.logged
            .iter()
            .any(|(_, line)| line.contains("the pointer is not in the view")),
        "{:?}",
        host.logged
    );
    // With the pointer in it no box at all is a page whose markers are not seen (diag11, a
    // `filter` on the page): its size maps it, as before.
    let mut unseen = shot((1536, 1050), (1024.0, 700.0), (0.0, 0.0), 1.5);
    unseen.colours = [MANAGED[1], MANAGED[1]];
    let mut host = self::host(&unseen);
    let answer = handle(&mut host, &request(&unseen, 1.5, true)).expect("answered");
    assert_eq!(
        answer["map"],
        json!({ "left": 0, "top": 0, "scale": 1.5, "width": 1536, "height": 1050 }),
        "{answer}"
    );
}
