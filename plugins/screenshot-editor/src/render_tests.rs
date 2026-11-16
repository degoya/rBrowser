use serde_json::json;
use tiny_skia::{Color, Pixmap};

use super::*;
use crate::model::validate;
use crate::testing::every_tool;

fn white(width: u32, height: u32) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).expect("pixmap");
    pixmap.fill(Color::WHITE);
    pixmap
}

fn annotations(value: serde_json::Value) -> Vec<Annotation> {
    let mut list: Vec<Annotation> = serde_json::from_value(value).expect("annotations");
    validate(&mut list).expect("valid");
    list
}

fn get(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * pixmap.width() + x) * 4) as usize;
    let d = pixmap.data();
    [d[i], d[i + 1], d[i + 2], d[i + 3]]
}

#[test]
fn each_of_the_nine_tools_leaves_a_mark() {
    let tools = every_tool();
    let list = tools.as_array().expect("list");
    assert_eq!(list.len(), 9);
    // A striped image, so the redaction changes something too.
    let striped = || {
        let mut pixmap = white(40, 40);
        for y in 0..40 {
            for x in (0..40).step_by(2) {
                let i = ((y * 40 + x) * 4) as usize;
                pixmap.data_mut()[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
        pixmap
    };
    for tool in list {
        let kind = tool["type"].as_str().expect("type");
        let one = annotations(json!([tool]));
        let before = striped();
        let after = render(before.clone(), &one).expect(kind);
        if kind == "crop" {
            let cropped = render(
                before.clone(),
                &annotations(json!([{ "type": "crop", "x": 3, "y": 4, "w": 10, "h": 7 }])),
            )
            .expect("crop");
            assert_eq!((cropped.width(), cropped.height()), (10, 7));
            assert_eq!(get(&cropped, 0, 0), get(&before, 3, 4));
        } else {
            assert_ne!(after.data(), before.data(), "{kind} drew nothing");
        }
    }
}

#[test]
fn a_crop_outside_the_image_is_refused() {
    let refused = render(
        white(10, 10),
        &annotations(json!([{ "type": "crop", "x": 20, "y": 20, "w": 5, "h": 5 }])),
    )
    .expect_err("outside");
    assert_eq!(refused.param("reason"), Some("crop"));
    assert_eq!(refused.param("index"), Some("0"));
}

#[test]
fn the_crop_applies_last_whatever_its_place() {
    let list = annotations(json!([
        { "type": "crop", "x": 0, "y": 0, "w": 10, "h": 10 },
        { "type": "rect", "x": 0, "y": 0, "w": 30, "h": 30, "color": "#000000", "width": 1, "fill": true }
    ]));
    let out = render(white(30, 30), &list).expect("render");
    assert_eq!((out.width(), out.height()), (10, 10));
    assert_eq!(get(&out, 5, 5), [0, 0, 0, 255]);
}

#[test]
fn a_highlight_is_translucent() {
    let out = render(
        white(10, 10),
        &annotations(
            json!([{ "type": "highlight", "x": 0, "y": 0, "w": 10, "h": 10, "color": "#0000ff" }]),
        ),
    )
    .expect("render");
    let [r, g, b, a] = get(&out, 5, 5);
    // White under blue at 89/255: 255 - 89 = 166 in red and green.
    assert!(
        (165..=167).contains(&r) && (165..=167).contains(&g),
        "{r} {g}"
    );
    assert_eq!((b, a), (255, 255));
}

#[test]
fn a_redaction_after_text_hides_the_text() {
    let text =
        json!({ "type": "text", "x": 2, "y": 2, "text": "secret", "color": "#000000", "size": 14 });
    let black = json!({ "type": "redact", "x": 0, "y": 0, "w": 80, "h": 30, "mode": "black" });
    let out = render(white(80, 30), &annotations(json!([text, black]))).expect("render");
    assert!(
        out.data()
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 0, 0, 255])
    );
    let pixelated = json!({ "type": "redact", "x": 0, "y": 0, "w": 80, "h": 30, "block": 40 });
    let out = render(white(80, 30), &annotations(json!([text, pixelated]))).expect("render");
    // Two blocks of 40 x 30, each one grey.
    let left = get(&out, 0, 0);
    assert!((0..40).all(|x| (0..30).all(|y| get(&out, x, y) == left)));
}

#[test]
fn a_filled_rectangle_is_a_solid_bar() {
    let out = render(
        white(20, 20),
        &annotations(json!([{ "type": "rect", "x": 15, "y": 15, "w": -10, "h": -10, "color": "#000000", "width": 1, "fill": true }])),
    )
    .expect("render");
    assert_eq!(get(&out, 7, 7), [0, 0, 0, 255]);
    assert_eq!(get(&out, 2, 2), [255, 255, 255, 255]);
}

#[test]
fn an_arrow_ends_in_a_head_at_its_tip() {
    let out = render(
        white(60, 20),
        &annotations(json!([{ "type": "arrow", "x1": 2, "y1": 10, "x2": 58, "y2": 10, "color": "#000000", "width": 2 }])),
    )
    .expect("render");
    // The head is 12 px long and 12 px wide at its base: dark above and below the shaft there.
    assert!(get(&out, 48, 6)[0] < 128 && get(&out, 48, 14)[0] < 128);
    // The shaft is thin before it.
    assert!(get(&out, 20, 6)[0] > 200);
    // A marker has its number in white inside the circle.
    let out = render(
        white(40, 40),
        &annotations(json!([{ "type": "marker", "x": 20, "y": 20, "number": 8, "color": "#000000", "size": 24 }])),
    )
    .expect("render");
    assert_eq!(get(&out, 20 - 16, 20), [0, 0, 0, 255]);
    let white_inside = (12..28).any(|x| (12..28).any(|y| get(&out, x, y)[0] > 200));
    assert!(white_inside);
}
