use serde_json::json;

use super::*;
use crate::testing::every_tool;

fn parse(value: serde_json::Value) -> Vec<Annotation> {
    serde_json::from_value(value).expect("annotations")
}

#[test]
fn every_tool_reads_and_writes_the_same_json() {
    let annotations = parse(every_tool());
    assert_eq!(annotations.len(), 9);
    let again: Vec<Annotation> =
        serde_json::from_value(serde_json::to_value(&annotations).expect("json")).expect("back");
    assert_eq!(again, annotations);
    let mut checked = annotations.clone();
    assert_eq!(validate(&mut checked), Ok(()));
}

#[test]
fn colours_are_hash_and_six_hex_digits() {
    assert_eq!(Rgb::parse("#FfA500"), Some(Rgb(255, 165, 0)));
    assert_eq!(String::from(Rgb(1, 2, 255)), "#0102ff");
    for bad in ["red", "#fff", "#gggggg", "ff0000", "#ff00001", "#ff 000"] {
        assert_eq!(Rgb::parse(bad), None, "{bad}");
    }
    let refused: Result<Vec<Annotation>, _> = serde_json::from_value(json!([
        { "type": "line", "x1": 0, "y1": 0, "x2": 1, "y2": 1, "color": "red", "width": 1 }
    ]));
    assert!(refused.is_err());
}

#[test]
fn unknown_tools_and_fields_are_refused() {
    for bad in [
        json!([{ "type": "circle", "x": 0, "y": 0 }]),
        json!([{ "type": "crop", "x": 0, "y": 0, "w": 1, "h": 1, "script": "x" }]),
        json!([{ "x": 0, "y": 0, "w": 1, "h": 1 }]),
    ] {
        let refused: Result<Vec<Annotation>, _> = serde_json::from_value(bad.clone());
        assert!(refused.is_err(), "{bad}");
    }
}

#[test]
fn a_redaction_defaults_to_a_mosaic_of_twelve_pixels() {
    let annotations = parse(json!([{ "type": "redact", "x": 0, "y": 0, "w": 5, "h": 5 }]));
    assert_eq!(
        annotations[0],
        Annotation::Redact {
            x: 0.0,
            y: 0.0,
            w: 5.0,
            h: 5.0,
            mode: RedactMode::Pixelate,
            block: DEFAULT_BLOCK
        }
    );
}

#[test]
fn a_rectangle_dragged_up_and_left_is_normalised() {
    let mut annotations = parse(json!([
        { "type": "redact", "x": 30, "y": 20, "w": -10, "h": -5, "mode": "black" }
    ]));
    assert_eq!(validate(&mut annotations), Ok(()));
    let Annotation::Redact { x, y, w, h, .. } = annotations[0] else {
        panic!("redact");
    };
    assert_eq!((x, y, w, h), (20.0, 15.0, 10.0, 5.0));
}

#[test]
fn values_out_of_range_name_the_annotation_and_the_reason() {
    let cases = [
        (
            json!({ "type": "line", "x1": 0, "y1": 0, "x2": 1e6, "y2": 1, "color": "#000000", "width": 1 }),
            "coordinate",
        ),
        (
            json!({ "type": "line", "x1": 0, "y1": 0, "x2": 1, "y2": 1, "color": "#000000", "width": 0.5 }),
            "width",
        ),
        (
            json!({ "type": "rect", "x": 0, "y": 0, "w": 1, "h": 1, "color": "#000000", "width": 65 }),
            "width",
        ),
        (
            json!({ "type": "text", "x": 0, "y": 0, "text": "  ", "color": "#000000", "size": 12 }),
            "text",
        ),
        (
            json!({ "type": "text", "x": 0, "y": 0, "text": "a\nb", "color": "#000000", "size": 12 }),
            "text",
        ),
        (
            json!({ "type": "text", "x": 0, "y": 0, "text": "ok", "color": "#000000", "size": 300 }),
            "size",
        ),
        (
            json!({ "type": "marker", "x": 0, "y": 0, "number": 10000, "color": "#000000", "size": 12 }),
            "number",
        ),
        (
            json!({ "type": "redact", "x": 0, "y": 0, "w": 5, "h": 5, "block": 129 }),
            "block",
        ),
        (
            json!({ "type": "freehand", "points": [], "color": "#000000", "width": 2 }),
            "points",
        ),
    ];
    for (annotation, reason) in cases {
        let mut annotations = parse(json!([
            { "type": "crop", "x": 0, "y": 0, "w": 1, "h": 1 },
            annotation
        ]));
        let failure = validate(&mut annotations).expect_err(reason);
        assert_eq!(
            failure.code(),
            Some("plugin.screenshot-editor.errors.bad_annotation")
        );
        assert_eq!(failure.param("index"), Some("1"), "{reason}");
        assert_eq!(failure.param("reason"), Some(reason));
    }
}

#[test]
fn one_crop_at_most_and_a_bounded_number_of_points() {
    let mut two = parse(json!([
        { "type": "crop", "x": 0, "y": 0, "w": 1, "h": 1 },
        { "type": "crop", "x": 0, "y": 0, "w": 2, "h": 2 }
    ]));
    assert_eq!(
        validate(&mut two).expect_err("two crops").param("reason"),
        Some("crop")
    );
    let points: Vec<[f32; 2]> = (0..MAX_POINTS + 1).map(|i| [i as f32, 0.0]).collect();
    let mut long = vec![Annotation::Freehand {
        points,
        color: Rgb(0, 0, 0),
        width: 2.0,
    }];
    assert_eq!(
        validate(&mut long).expect_err("points").param("reason"),
        Some("points")
    );
    let mut many = vec![
        Annotation::Crop {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0
        };
        MAX_ANNOTATIONS + 1
    ];
    assert_eq!(
        validate(&mut many).expect_err("count").param("reason"),
        Some("count")
    );
}

#[test]
fn the_document_keeps_its_fields_in_camel_case() {
    let document = Document {
        version: DOCUMENT_VERSION,
        image: ".rbrowser/out/screenshots/a/mobile.png".to_owned(),
        width: 40,
        height: 30,
        annotations: parse(every_tool()),
    };
    let text = serde_json::to_string(&document).expect("json");
    assert!(text.starts_with(r#"{"version":1,"image":".rbrowser/out/screenshots/a/mobile.png","width":40,"height":30,"annotations":[{"type":"rect""#), "{text}");
    let back: Document = serde_json::from_str(&text).expect("back");
    assert_eq!(back, document);
}

/// Charge 9 review L3: a mosaic of 4 px blocks leaves text readable; a smaller block than
/// [`BLOCK`]'s minimum is raised to it, so a saved document or an MCP call still renders.
#[test]
fn a_redaction_block_below_eight_pixels_is_raised_to_eight() {
    assert_eq!(BLOCK.0, 8);
    for small in [0, 2, 4, 7] {
        let mut annotations = parse(json!([
            { "type": "redact", "x": 0, "y": 0, "w": 5, "h": 5, "block": small }
        ]));
        validate(&mut annotations).expect("valid");
        let Annotation::Redact { block, .. } = &annotations[0] else {
            panic!("a redaction");
        };
        assert_eq!(*block, 8, "{small}");
    }
}
