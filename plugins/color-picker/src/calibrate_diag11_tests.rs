//! RB-090-19: the calibration of diag11 (2026-11-15, Windows, an owner's project, DPR 1.5). Red before
//! the fix (the job's log keeps the run): the false pair at 0.5538 was taken, an image of the
//! viewport at its DPR without both markers was not mapped, and a colourful page filled the finder
//! before the bottom markers.

use super::tests::{MANAGED, PAGE, corners, shot};
use super::*;

/// A box of a marker's shape at `scale` image px per CSS px whose middle is the image point
/// `middle`, in the display's colours.
fn band_at(middle: (f64, f64), scale: f64) -> Band {
    let half = (MARKER_SIZE.0 * scale / 2.0, MARKER_SIZE.1 * scale / 2.0);
    Band {
        left: (middle.0 - half.0).round() as u32,
        right: (middle.0 + half.0).round() as u32,
        top: (middle.1 - half.1).round() as u32,
        bottom: (middle.1 + half.1).round() as u32,
        colours: MANAGED,
    }
}

/// diag11, build 7f54ac3: `view 3 is read from its window image (1536 x 2019 px for 1024 x 1346
/// CSS px at DPR 1.5): the viewport at 0.5538 px per CSS px from (0, 0)`. The top left marker
/// where DPR 1.5 puts it, and a box of the page where a scale of 0.5538 puts the bottom right one.
fn diag11_false_pair(port: (f64, f64)) -> (Vec<Band>, Marker) {
    let boxes = corners(port.0, port.1);
    let first = css_middle(boxes[0]);
    let last = css_middle(boxes[3]);
    let marker = (first.0 * 1.5, first.1 * 1.5);
    let wrong = 0.5538;
    let page = (
        marker.0 + (last.0 - first.0) * wrong,
        marker.1 + (last.1 - first.1) * wrong,
    );
    (vec![band_at(marker, 1.5), band_at(page, wrong)], boxes)
}

#[test]
fn a_false_band_at_0_5538_is_never_taken_for_an_image_of_viewport_times_dpr() {
    // The pair measured 0.5538 and was taken; the image is the viewport at DPR 1.5 exactly, which
    // maps it without any marker.
    let port = (1024.0, 1346.0);
    let (bands, boxes) = diag11_false_pair(port);
    let (map, _) = calibrate(&bands, &boxes, (port.0, port.1, 1.5), (1536, 2019)).expect("mapped");
    assert_eq!(
        map,
        Map {
            left: 0,
            top: 0,
            scale: 1.5,
            width: 1536,
            height: 2019
        }
    );
}

#[test]
fn a_scale_far_from_the_image_s_is_never_taken() {
    // The same pair in an image a few pixels wider than the viewport at 1.5 (not mapped by its
    // size): 0.5538 is far from the 1.5 the image's size gives, so it is refused.
    let port = (1024.0, 1346.0);
    let (bands, boxes) = diag11_false_pair(port);
    let measured = calibrate(&bands, &boxes, (port.0, port.1, 1.5), (1542, 2019));
    assert!(
        measured.is_err() || measured.is_ok_and(|(map, _)| (map.scale - 1.5).abs() < 0.02),
        "a wrong scale was taken"
    );
}

#[test]
fn every_image_of_diag11_that_is_the_viewport_at_its_dpr_is_mapped_without_markers() {
    // diag11, 15:22-15:30: every pick failed (`shows 0 of the two markers`, `shows 1 of the two
    // markers`, `shows 2 marker-like boxes but none maps the viewport into it`); each image was the
    // viewport at DPR 1.5, give or take the half pixel Chromium rounds.
    for (image, port) in [
        ((1152, 914), (768.0, 609.0)),
        ((3000, 1827), (2000.0, 1218.0)),
        ((563, 954), (375.0, 636.0)),
        ((1152, 3333), (768.0, 2222.0)),
        ((1536, 4476), (1024.0, 2984.0)),
        ((3000, 1905), (2000.0, 1270.0)),
        ((563, 1628), (375.0, 1085.0)),
        ((1152, 1511), (768.0, 1007.0)),
    ] {
        let boxes = corners(port.0, port.1);
        let (map, exact) = calibrate(&[], &boxes, (port.0, port.1, 1.5), image).expect("mapped");
        assert_eq!((map.left, map.top, map.scale), (0, 0, 1.5), "{image:?}");
        assert_eq!((map.width, map.height), image);
        // No marker seen: the colours are not said to be the page's.
        assert!(!exact);
    }
    // More than a pixel off is not the viewport at its DPR: diag11's `567 x 746 px for 375 x 497`.
    assert!(direct((375.0, 497.0, 1.5), (567, 746)).is_none());
    assert!(direct((375.0, 497.0, 1.5), (563, 746)).is_some());
}

#[test]
fn the_bottom_marker_is_found_below_a_colourful_page() {
    // A page's pictures give many one-row runs of magenta beside green above the bottom marker;
    // the first 16 of them filled the finder and the marker was never kept.
    let width = 420;
    let mut finder = Finder::default();
    let paint = |y: u32| -> Vec<Rgb> {
        (0..width)
            .map(|x| {
                let noise = y < 40 && x >= y * 10 && x < y * 10 + 6;
                let marker = |left: u32, top: u32| {
                    (top..top + 12).contains(&y) && (left..left + 24).contains(&x)
                };
                if noise {
                    MARKER[usize::from(x >= y * 10 + 3)]
                } else if marker(10, 100) {
                    MARKER[usize::from(x >= 22)]
                } else if marker(380, 300) {
                    MARKER[usize::from(x >= 392)]
                } else {
                    PAGE
                }
            })
            .collect()
    };
    for y in 0..320 {
        finder.row(y, &paint(y));
    }
    let found: Vec<(u32, u32)> = finder
        .markers()
        .iter()
        .map(|band| (band.left, band.top))
        .collect();
    assert_eq!(found, vec![(10, 100), (380, 300)]);
}

#[test]
fn any_two_of_the_four_markers_map_a_scaled_view() {
    // diag9's canvas zoom: `635 x 365 px for 1024 x 588 CSS px`. The bottom markers covered (a
    // strip over the view's bottom, a part of the view off the screen), or two diagonal ones.
    let scale = 635.0 / 1024.0;
    for hidden in [vec![2, 3], vec![0, 3], vec![1, 2], vec![0, 1]] {
        let mut view = shot((635, 365), (1024.0, 588.0), (0.0, 0.0), scale);
        view.hidden = hidden.clone();
        let (map, _) = view.calibrate(1.0).expect("mapped");
        assert_eq!((map.left, map.top), (0, 0), "{hidden:?}");
        assert!(
            (map.scale - scale).abs() < 1e-9,
            "{hidden:?}: {}",
            map.scale
        );
    }
}

#[test]
fn a_box_of_another_size_is_no_marker() {
    // A box where a marker would be at the image's scale but three times its size: a page's
    // magenta-and-green picture, not the marker.
    let (port, image) = ((1024.0, 588.0), (635, 365));
    let scale = 635.0 / 1024.0;
    let boxes = corners(port.0, port.1);
    let at = |index: usize| {
        let (x, y) = css_middle(boxes[index]);
        (x * scale, y * scale)
    };
    let dims = (port.0, port.1, 1.0);
    let wrong = [band_at(at(0), scale), band_at(at(3), 3.0 * scale)];
    assert!(calibrate(&wrong, &boxes, dims, image).is_err());
    let right = [band_at(at(0), scale), band_at(at(3), scale)];
    let (map, _) = calibrate(&right, &boxes, dims, image).expect("mapped");
    assert_eq!((map.left, map.top), (0, 0));
}

#[test]
fn a_marker_on_a_magenta_bar_is_still_found() {
    // A page's bar along the viewport's bottom in a magenta of its own (a breakpoint helper, a
    // cookie banner): the black frame keeps it from running into the bottom markers.
    let mut view = shot((635, 365), (1024.0, 588.0), (0.0, 0.0), 635.0 / 1024.0);
    view.bar = Some(MANAGED[0]);
    assert_eq!(view.bands().len(), 4);
    let (map, _) = view.calibrate(1.0).expect("mapped");
    assert!((map.scale - 635.0 / 1024.0).abs() < 1e-9, "{}", map.scale);
}

#[test]
fn markers_the_page_drew_outside_the_viewport_are_not_looked_for() {
    // A `transform` or `filter` on `<html>` makes `fixed` relative to the document: the page
    // script reports the bottom markers at the document's end.
    let mut view = shot((635, 365), (1024.0, 588.0), (0.0, 0.0), 635.0 / 1024.0);
    view.hidden = vec![2, 3];
    view.boxes[2].1 = 4000.0;
    view.boxes[3].1 = 4000.0;
    let (map, _) = view.calibrate(1.0).expect("mapped by the top two");
    assert_eq!((map.left, map.top), (0, 0));
    view.boxes[1].1 = 4000.0;
    let refused = view.calibrate(1.0).expect_err("one corner left");
    assert!(
        refused.contains("has 1 of the page's markers inside the viewport"),
        "{refused}"
    );
}
