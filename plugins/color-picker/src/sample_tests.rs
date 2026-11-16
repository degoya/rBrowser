use serde_json::json;

use super::*;
use crate::pixels::tests::png_base64;
use crate::testing::FakeHost;

const BLUE: [u8; 3] = [0x1e, 0x88, 0xe5];

/// A view scrolled by 300 px with the fixture's blue area everywhere.
fn host(dpr: f64) -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "chromium").view(2, "webkit");
    host.evaluate.insert(
        1,
        json!({ "scrollX": 0, "scrollY": 300, "width": 375, "height": 667, "dpr": dpr })
            .to_string(),
    );
    // The clip is 11 CSS px at DPR 1, 5 at DPR 3: the loupe's 11 device pixels.
    let side = (if dpr == 1.0 { 11.0 } else { 5.0 * dpr }).round() as u32;
    host.answer(
        "Page.captureScreenshot",
        json!({ "data": png_base64(side, side, |_, _| BLUE) }),
    );
    host
}

#[test]
fn the_tool_samples_the_scrolled_document_at_dpr_1_and_3() {
    for dpr in [1.0, 3.0] {
        let mut host = host(dpr);
        let answer = tool(&mut host, &json!({ "target": 1, "x": 100, "y": 50 })).expect("sample");
        let view = &answer["views"][0];
        assert_eq!(view["color"]["hex"], "#1e88e5", "dpr {dpr}");
        assert_eq!(view["value"], "#1e88e5");
        assert_eq!(view["dpr"], dpr);
        assert_eq!(view["view"], 1);
        assert!(view.get("grid").is_none());
        // MCP's color_sample has no page script to show the window image's marker: a clip,
        // in document coordinates (y + scrollY).
        let (_, method, params) = &host.cdp_sent[0];
        assert_eq!(method, "Page.captureScreenshot");
        let half = if dpr == 1.0 { 5.0 } else { 2.0 };
        assert_eq!(params["clip"]["y"], 350.0 - half);
        assert_eq!(params["clip"]["scale"], 1);
        assert_eq!(params["format"], "png");
    }
}

#[test]
fn a_point_outside_the_viewport_and_other_engines_are_answered_per_view() {
    let mut host = host(1.0);
    let answer = tool(&mut host, &json!({ "target": "all", "x": 400, "y": 50 })).expect("views");
    assert_eq!(
        answer["views"][0]["error"],
        "plugin.color-picker.errors.outside"
    );
    assert_eq!(
        answer["views"][1]["error"],
        "plugin.color-picker.errors.unsupported_engine"
    );
    assert!(host.cdp_sent.is_empty());
}

#[test]
fn coordinates_must_be_numbers_in_range() {
    let mut host = host(1.0);
    for args in [
        json!({ "y": 1 }),
        json!({ "x": -1, "y": 1 }),
        json!({ "x": 1, "y": "2" }),
    ] {
        let refused = tool(&mut host, &args).expect_err("refused");
        assert_eq!(
            refused.code.as_deref(),
            Some("plugins.error.invalid"),
            "{args}"
        );
    }
}

#[test]
fn the_loupe_grid_is_hex_or_null() {
    let mut host = host(1.0);
    let port = Viewport {
        width: 375.0,
        height: 667.0,
        dpr: 1.0,
        ..Viewport::default()
    };
    let sample = capture(&mut host, 1, &port, (2.0, 2.0), 11, &Read::Clip).expect("sample");
    let value = to_json(&sample, Format::Oklch, true);
    let grid = value["grid"].as_array().expect("grid");
    assert_eq!(grid.len(), 121);
    assert_eq!(grid[60], "#1e88e5");
    // Left of the document there is nothing.
    assert_eq!(grid[0], Value::Null);
    assert!(
        value["value"]
            .as_str()
            .is_some_and(|text| text.starts_with("oklch("))
    );
}
