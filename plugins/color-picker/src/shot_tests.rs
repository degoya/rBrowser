use serde_json::{Value, json};

use crate::calibrate::{MARKER_FRAME, MARKER_SIZE};
use crate::page::handle;
use crate::page::tests::rgb_of;
use crate::pixels::Rgb;
use crate::pixels::tests::png_base64;
use crate::testing::FakeHost;

const MANAGED: [Rgb; 2] = [[231, 70, 251], [124, 248, 59]];
const PAGE: Rgb = [174, 215, 237];
const BLUE: Rgb = [0x1e, 0x88, 0xe5];

/// The markers' frames where page/picker.js draws them: the viewport's four corners.
fn corners(port: (f64, f64)) -> [(f64, f64); 4] {
    let (right, bottom) = (
        port.0 - MARKER_SIZE.0 - 2.0 * MARKER_FRAME,
        port.1 - MARKER_SIZE.1 - 2.0 * MARKER_FRAME,
    );
    [(0.0, 0.0), (right, 0.0), (0.0, bottom), (right, bottom)]
}

/// A window image of `image` pixels showing a viewport of `port` CSS px from the image pixel
/// `offset` at `scale`, with the page script's framed markers in its corners in `colours` (or
/// none) and blue at the CSS pixel `blue`; dark around the viewport.
fn window(
    image: (u32, u32),
    port: (f64, f64),
    offset: (u32, u32),
    scale: f64,
    colours: Option<[Rgb; 2]>,
    blue: (f64, f64),
) -> String {
    png_base64(image.0, image.1, |x, y| {
        let css = |pixel: u32, from: u32| (f64::from(pixel) + 0.5 - f64::from(from)) / scale;
        let (cx, cy) = (css(x, offset.0), css(y, offset.1));
        if x < offset.0 || y < offset.1 || cx >= port.0 || cy >= port.1 {
            return [31, 31, 31];
        }
        if let Some(colours) = colours {
            for (left, top) in corners(port) {
                let (dx, dy) = (cx - left - MARKER_FRAME, cy - top - MARKER_FRAME);
                if (0.0..MARKER_SIZE.0).contains(&dx) && (0.0..MARKER_SIZE.1).contains(&dy) {
                    return colours[usize::from(dx >= MARKER_SIZE.0 / 2.0)];
                }
                let frame = -MARKER_FRAME..MARKER_SIZE.0 + MARKER_FRAME;
                if frame.contains(&dx)
                    && (-MARKER_FRAME..MARKER_SIZE.1 + MARKER_FRAME).contains(&dy)
                {
                    return [0, 0, 0];
                }
            }
        }
        if cx.floor() == blue.0 && cy.floor() == blue.1 {
            BLUE
        } else {
            PAGE
        }
    })
}

fn host(shot: String) -> FakeHost {
    let mut host = FakeHost::default();
    host.view(4, "chromium");
    host.answer("Page.captureScreenshot", json!({ "data": shot }));
    host
}

fn calibrate_message(port: (f64, f64), dpr: f64) -> Value {
    json!({ "view": 4, "message": {
        "type": "calibrate", "x": 10, "y": 10, "width": port.0, "height": port.1, "dpr": dpr,
        "scrollX": 0, "scrollY": 0,
        "marker": corners(port).map(|(x, y)| [x, y]), "pointed": true,
    } })
}

#[test]
fn the_window_image_of_diag9_is_read_without_a_clip() {
    // RB-080-77, red before the fix (probe-rb-080-77-red.patch in the job's log): diag9's line
    // "the window image of view 3 (635 x 365 px for 1024 x 588 CSS px) does not show the marker
    // where the viewport maps it: [Some([231, 70, 251]), Some([124, 248, 59])]; a clip instead" -
    // the markers were there, in the display's colours, and every read became a clip.
    let port = (1024.0, 588.0);
    let mut host = host(window(
        (635, 365),
        port,
        (0, 0),
        635.0 / 1024.0,
        Some(MANAGED),
        (-1.0, -1.0),
    ));
    let answer = handle(&mut host, &calibrate_message(port, 1.0)).expect("calibrated");
    assert_eq!(
        answer["map"],
        json!({ "left": 0, "top": 0, "scale": 635.0 / 1024.0, "width": 635, "height": 365 })
    );
    assert_eq!(answer["exact"], false);
    // One window image; no clip.
    assert_eq!(host.cdp_sent.len(), 1);
    assert_eq!(host.cdp_sent[0].2["fromSurface"], false);
    assert!(host.cdp_sent[0].2.get("clip").is_none());
    // A person's log names how the view is read and that its colours are the display's.
    assert_eq!(host.logged.len(), 1);
    assert_eq!(host.logged[0].0, "info");
    assert!(
        host.logged[0].1.contains(
            "view 4 is read from its window image (635 x 365 px for 1024 x 588 CSS px at DPR 1)"
        ),
        "{}",
        host.logged[0].1
    );
    assert!(
        host.logged[0]
            .1
            .contains("the display's colours (#ff00ff, #00ff00 read #e746fb, #7cf83b)"),
        "{}",
        host.logged[0].1
    );
}

#[test]
fn the_loupe_is_cut_around_the_pointer_at_a_dpr_and_an_offset() {
    // RB-080-77: the tile around the CSS pixel under the pointer of a viewport drawn at 1.5 image
    // px per CSS px from the image pixel (6, 4); its middle is that pixel, its crosshair cell.
    let port = (400.0, 300.0);
    let image = (620, 470);
    let shot = |markers| window(image, port, (6, 4), 1.5, markers, (123.0, 77.0));
    let mut measured = host(shot(Some(MANAGED)));
    let answer = handle(&mut measured, &calibrate_message(port, 1.5)).expect("calibrated");
    assert_eq!(
        answer["map"],
        json!({ "left": 6, "top": 4, "scale": 1.5, "width": 620, "height": 470 })
    );
    let mut host = host(shot(None));
    let tile = json!({ "view": 4, "message": {
        "type": "tile", "x": 123.4, "y": 77.9, "width": 400, "height": 300, "dpr": 1.5,
        "scrollX": 0, "scrollY": 0, "map": answer["map"],
    } });
    let answer = handle(&mut host, &tile).expect("tile");
    // The CSS pixel (123, 77): its middle at 6 + 123.5 * 1.5 = 191.25, 4 + 77.5 * 1.5 = 120.25.
    let (left, top) = (
        answer["left"].as_u64().expect("left"),
        answer["top"].as_u64().expect("top"),
    );
    assert_eq!((left, top), (191 - 58, 120 - 58));
    assert_eq!(
        (answer["offsetX"].as_i64(), answer["offsetY"].as_i64()),
        (Some(6), Some(4))
    );
    let rgb = rgb_of(&answer);
    let width = usize::try_from(answer["width"].as_u64().expect("width")).expect("small");
    let middle = (58 * width + 58) * 3;
    assert_eq!(&rgb[middle..middle + 3], &BLUE);
    // The CSS pixel is 1.5 image px (190 and 191): the one right of the middle is the page.
    assert_eq!(&rgb[middle - 3..middle], &BLUE);
    assert_eq!(&rgb[middle + 3..middle + 6], &PAGE);
}

#[test]
fn a_window_image_without_the_markers_is_not_read() {
    // diag9: often the page's colour where the markers should be - the frame before they were
    // drawn. Not a clip, which flickers: the page script asks again, then says so.
    let port = (1024.0, 588.0);
    let mut host = host(window(
        (635, 365),
        port,
        (0, 0),
        635.0 / 1024.0,
        None,
        (-1.0, -1.0),
    ));
    let answer = handle(&mut host, &calibrate_message(port, 1.0)).expect("answered");
    assert_eq!(answer, json!({ "map": null, "reason": "markers" }));
    assert_eq!(host.cdp_sent.len(), 1);
    assert!(host.logged.iter().any(|(level, line)| {
        level == "info"
            && line.contains(
                "(635 x 365 px for 1024 x 588 CSS px at DPR 1) shows 0 of the 4 markers: boxes [], markers drawn at [0,0; 996,0; 0,572; 996,572] CSS px",
            )
    }), "{:?}", host.logged);
}

#[test]
fn an_image_of_the_viewport_at_its_dpr_is_read_without_markers() {
    // RB-090-19, diag11: `the window image of view 2 (1152 x 914 px for 768 x 609 CSS px at DPR
    // 1.5) shows 2 marker-like boxes but none maps the viewport into it` - three times, then
    // `unreadable`. Its size maps it where the markers agree or none is seen with the pointer in
    // the view (RB-090-40).
    let port = (768.0, 609.0);
    let image = (1152, 914);
    for (markers, exact) in [
        (None, false),
        (Some(MANAGED), false),
        (Some([[255, 0, 255], [0, 255, 0]]), true),
    ] {
        let mut host = host(window(image, port, (0, 0), 1.5, markers, (-1.0, -1.0)));
        let answer = handle(&mut host, &calibrate_message(port, 1.5)).expect("calibrated");
        assert_eq!(
            answer,
            json!({
                "map": { "left": 0, "top": 0, "scale": 1.5, "width": 1152, "height": 914 },
                "exact": exact,
                "visible": { "left": 0.0, "top": 0.0, "right": 768.0, "bottom": 609.0 }
            })
        );
        assert!(
            host.logged[0].1.contains(
                "view 4 is read from its window image (1152 x 914 px for 768 x 609 CSS px at DPR 1.5): the viewport at 1.5000 px per CSS px from (0, 0) by the image's size"
            ),
            "{}",
            host.logged[0].1
        );
    }
}

#[test]
fn an_image_of_the_viewport_s_size_holding_its_page_smaller_is_read_by_its_markers() {
    // RB-090-31, diag12: on the canvas at 45 % a view's window image had the viewport's size at
    // its DPR and the page drawn at 0.45 of it in its top left corner; only the image's top rows
    // were read and its size taken (3 here), the loupe read twice too far right and down. The
    // markers there deny the size: the whole image is searched and they map it.
    let port = (200.0, 120.0);
    let shot = window((600, 360), port, (0, 0), 1.5, Some(MANAGED), (-1.0, -1.0));
    let mut host = host(shot);
    let answer = handle(&mut host, &calibrate_message(port, 3.0)).expect("calibrated");
    let scale = answer["map"]["scale"].as_f64().expect("mapped");
    assert!((scale - 1.5).abs() < 0.01, "{answer}");
    assert_eq!(
        (&answer["map"]["left"], &answer["map"]["top"]),
        (&json!(0), &json!(0))
    );
    assert_eq!(host.cdp_sent.len(), 1);
    assert!(
        host.logged
            .iter()
            .any(|(_, line)| line.contains("from (0, 0) by its markers [")),
        "{:?}",
        host.logged
    );
}

#[test]
fn a_window_image_larger_than_a_4k_screen_is_not_read() {
    // Review C2 of RB-080-64: inflating it could use up the call's fuel; refused by its header.
    // RB-090-19, diag11: `the window image of view 7 failed: plugins.error.invalid` three times -
    // view 7 is 2000 CSS px wide at DPR 1.5, more than 8 megapixels from 1865 CSS px high; the
    // log said neither the reason nor the size. It names both now, and the status bar the reason.
    let huge = crate::pixels::tests::png_header_base64(3000, 2900, false);
    let mut tall = host(huge);
    let answer = handle(&mut tall, &calibrate_message((2000.0, 1933.0), 1.5)).expect("answered");
    assert_eq!(answer, json!({ "map": null, "reason": "size" }));
    assert!(
        tall.logged.iter().any(|(_, line)| line.contains(
            "the window image of view 4 (3000 x 2900 px for 2000 x 1933 CSS px at DPR 1.5) has more than the 8 megapixels the pipette reads"
        )),
        "{:?}",
        tall.logged
    );
    // An answer that is no PNG names the failure's reason.
    let mut garbled = host("bm90IGEgcG5n".to_owned());
    let answer = handle(&mut garbled, &calibrate_message((40.0, 30.0), 1.0)).expect("answered");
    assert_eq!(answer, json!({ "map": null, "reason": "image" }));
    assert!(
        garbled
            .logged
            .iter()
            .any(|(_, line)| line.contains("failed: plugins.error.invalid (screenshot)")),
        "{:?}",
        garbled.logged
    );
}

/// RB-090-29: measuring a view decodes its window image's base64 once - the size and the marker
/// scan read the same bytes (twice before, a cost on large views, RB-090-19).
#[test]
fn a_calibration_decodes_the_window_image_once() {
    let port = (400.0, 300.0);
    let mut host = host(window(
        (400, 300),
        port,
        (0, 0),
        1.0,
        Some(MANAGED),
        (5.0, 5.0),
    ));
    let viewport = super::Viewport {
        width: port.0,
        height: port.1,
        dpr: 1.0,
        scroll_x: 0.0,
        scroll_y: 0.0,
    };
    let marker = corners(port);
    let decodes = crate::pixels::tests::base64_decodes(|| {
        let _ = super::calibrate_view(&mut host, 4, &viewport, &marker, true);
    });
    assert_eq!(decodes, 1);
}
