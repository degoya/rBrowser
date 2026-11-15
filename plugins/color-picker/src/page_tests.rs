use serde_json::json;

use super::*;
use crate::pixels::tests::png_base64;
use crate::testing::FakeHost;

/// A view of 40 x 30 CSS px at DPR 1, grey all over.
fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(4, "chromium");
    host.answer(
        "Page.captureScreenshot",
        json!({ "data": png_base64(40, 30, |_, _| [0x76, 0x76, 0x76]) }),
    );
    host
}

fn message(message: Value) -> Value {
    json!({ "view": 4, "message": message })
}

#[test]
fn config_answers_the_settings() {
    let answer = handle(&mut host(), &message(json!({ "type": "config" }))).expect("config");
    assert_eq!(answer["loupeSize"], 11);
    assert_eq!(answer["format"], "hex");
}

#[test]
fn the_per_position_sample_route_is_gone() {
    // RB-080-56: a screenshot per pointer position made the views flicker; the page asks for a
    // tile, MCP's color_sample goes through sample::tool.
    let mut host = host();
    let point =
        json!({ "type": "sample", "x": 10, "y": 20, "scrollX": 0, "scrollY": 100, "dpr": 3 });
    let refused = handle(&mut host, &message(point)).expect_err("no route");
    assert_eq!(refused.code.as_deref(), Some("plugins.error.invalid"));
    assert_eq!(screenshots(&host), 0);
}

#[test]
fn taking_posts_to_the_panel_and_stops_every_pipette() {
    let mut host = host();
    let take = json!({ "type": "take", "slot": "background", "x": 10, "y": 20, "width": 40,
                       "height": 30, "dpr": 1, "scrollX": 0, "scrollY": 0, "map": map(40, 30, 1.0) });
    let answer = handle(&mut host, &message(take)).expect("take");
    let posts = answer["$post"].as_array().expect("posts");
    assert_eq!(posts[0]["target"], "panel");
    assert_eq!(posts[0]["message"]["slot"], "background");
    assert_eq!(posts[0]["message"]["view"], 4);
    assert_eq!(posts[0]["message"]["color"]["rgb"], "rgb(118, 118, 118)");
    // No `exact: false` from the page script: nothing said of the display's colours.
    assert!(posts[0]["message"].get("managed").is_none());
    assert_eq!(
        posts[1],
        json!({ "target": "all", "message": { "type": "stop" } })
    );
    // The status bar names the colour taken (RB-080-64).
    assert_eq!(posts[2]["target"], "status");
    assert_eq!(
        posts[2]["message"]["key"],
        "plugin.color-picker.notice.picked"
    );
    assert_eq!(posts[2]["message"]["params"]["value"], "#767676");
}

#[test]
fn esc_in_a_view_stops_every_pipette_and_tells_the_panel() {
    let mut host = host();
    let answer = handle(&mut host, &message(json!({ "type": "stopped" }))).expect("stopped");
    let posts = answer["$post"].as_array().expect("posts");
    assert_eq!(
        posts[0],
        json!({ "target": "all", "message": { "type": "stop" } })
    );
    assert_eq!(
        posts[1],
        json!({ "target": "panel", "message": { "type": "stopped" } })
    );
    assert_eq!(
        posts[2]["message"]["key"],
        "plugin.color-picker.notice.stopped"
    );
    assert_eq!(screenshots(&host), 0);
}

#[test]
fn a_chosen_element_is_measured_in_every_view() {
    let mut host = host();
    host.evaluate
        .insert(4, json!({ "elements": [] }).to_string());
    let answer = handle(
        &mut host,
        &message(json!({ "type": "chosen", "selector": "main > p:nth-of-type(2)" })),
    )
    .expect("measured");
    assert_eq!(answer["selector"], "main > p:nth-of-type(2)");
    assert_eq!(answer["views"][0]["rating"], "none");
}

#[test]
fn messages_that_do_not_read_are_refused() {
    let mut host = host();
    for args in [
        json!({ "message": { "type": "config" } }),
        message(json!({ "type": "launch" })),
        message(
            json!({ "type": "tile", "x": 1, "y": 1, "width": 40, "height": 30, "dpr": 40, "scrollX": 0, "scrollY": 0 }),
        ),
        message(
            json!({ "type": "tile", "x": 1, "y": 1, "width": 40, "height": 30, "dpr": 1, "scrollY": 0 }),
        ),
        message(
            json!({ "type": "tile", "x": 1, "y": 1, "width": 0, "height": 30, "dpr": 1, "scrollX": 0, "scrollY": 0 }),
        ),
        message(
            json!({ "type": "tile", "x": 1, "y": 1, "height": 30, "dpr": 1, "scrollX": 0, "scrollY": 0 }),
        ),
        message(
            json!({ "type": "take", "slot": "border", "x": 1, "y": 1, "width": 40, "height": 30, "dpr": 1, "scrollX": 0, "scrollY": 0 }),
        ),
        message(json!({ "type": "chosen" })),
        message(
            json!({ "type": "tile", "x": 1, "y": 1, "width": 40, "height": 30, "dpr": 1, "scrollX": 0, "scrollY": 0, "clip": "yes" }),
        ),
        // Neither a map nor a clip; a map that does not read.
        message(
            json!({ "type": "tile", "x": 1, "y": 1, "width": 40, "height": 30, "dpr": 1, "scrollX": 0, "scrollY": 0 }),
        ),
        message(
            json!({ "type": "tile", "x": 1, "y": 1, "width": 40, "height": 30, "dpr": 1, "scrollX": 0, "scrollY": 0, "map": { "left": 0, "top": 0, "scale": 0, "width": 40, "height": 30 } }),
        ),
        message(
            json!({ "type": "tile", "x": 1, "y": 1, "width": 40, "height": 30, "dpr": 1, "scrollX": 0, "scrollY": 0, "map": { "left": 0.5, "top": 0, "scale": 1, "width": 40, "height": 30 } }),
        ),
        message(
            json!({ "type": "tile", "x": 1, "y": 1, "width": 40, "height": 30, "dpr": 1, "scrollX": 0, "scrollY": 0, "map": { "left": -2, "top": 0, "scale": 1, "width": 40, "height": 30 } }),
        ),
        message(
            json!({ "type": "calibrate", "x": 1, "y": 1, "width": 40, "height": 30, "dpr": 1, "scrollX": 0, "scrollY": 0, "marker": [[1, 2]] }),
        ),
        message(
            json!({ "type": "calibrate", "x": 1, "y": 1, "width": 40, "height": 30, "dpr": 1, "scrollX": 0, "scrollY": 0, "marker": [[1, 2], [-3, 2]] }),
        ),
    ] {
        let refused = handle(&mut host, &args).expect_err("refused");
        assert_eq!(
            refused.code.as_deref(),
            Some("plugins.error.invalid"),
            "{args}"
        );
    }
}

fn screenshots(host: &FakeHost) -> usize {
    host.cdp_sent
        .iter()
        .filter(|sent| sent.1 == "Page.captureScreenshot")
        .count()
}

/// The map of a window image that is the viewport of `width` x `height` CSS px at `scale`.
pub(crate) fn map(width: u32, height: u32, scale: f64) -> Value {
    json!({ "left": 0, "top": 0, "scale": scale,
            "width": (f64::from(width) * scale).round(), "height": (f64::from(height) * scale).round() })
}

fn tile_message(x: u32, y: u32, width: u32, height: u32, dpr: f64) -> Value {
    message(
        json!({ "type": "tile", "x": x, "y": y, "width": width, "height": height, "dpr": dpr,
                "scrollX": 0, "scrollY": 0, "map": map(width, height, dpr) }),
    )
}

pub(crate) fn rgb_of(answer: &Value) -> Vec<u8> {
    base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        answer["rgb"].as_str().expect("rgb"),
    )
    .expect("base64")
}

#[test]
fn a_tile_is_cut_from_the_view_as_it_is_shown_never_a_clip() {
    // RB-080-64: a clipped screenshot - and, under the emulation of a view scaled into its cell,
    // one from the surface - makes the view draw another picture for a frame. The tile is cut
    // from the window's image of the whole viewport. Red before the fix: a 59 x 59 CSS px clip.
    let mut host = FakeHost::default();
    host.view(4, "chromium");
    // A viewport of 400 x 300 CSS px at DPR 2: an image of 800 x 600, blue at the CSS pixel
    // (100, 50) - the image pixels 200 and 201.
    let shot = png_base64(800, 600, |x, y| {
        if x / 2 == 100 && y / 2 == 50 {
            [0x1e, 0x88, 0xe5]
        } else {
            [255; 3]
        }
    });
    host.answer("Page.captureScreenshot", json!({ "data": shot }));
    let answer = handle(&mut host, &tile_message(100, 50, 400, 300, 2.0)).expect("tile");
    assert_eq!(screenshots(&host), 1);
    let params = &host.cdp_sent[0].2;
    assert!(params.get("clip").is_none(), "{params}");
    assert_eq!(params["fromSurface"], false);
    assert_eq!(params["captureBeyondViewport"], false);
    assert_eq!(params["format"], "png");
    // The image pixel in the middle of the CSS pixel (100, 50) is (201, 101); 58 on each side.
    assert_eq!(
        (answer["left"].as_u64(), answer["top"].as_u64()),
        (Some(143), Some(43))
    );
    assert_eq!(
        (answer["offsetX"].as_i64(), answer["offsetY"].as_i64()),
        (Some(0), Some(0))
    );
    assert_eq!(answer["scale"], 2.0);
    assert_eq!(
        (answer["width"].as_u64(), answer["height"].as_u64()),
        (Some(117), Some(117))
    );
    assert_eq!(
        (
            answer["imageWidth"].as_u64(),
            answer["imageHeight"].as_u64()
        ),
        (Some(800), Some(600))
    );
    let rgb = rgb_of(&answer);
    assert_eq!(rgb.len(), 117 * 117 * 3);
    let at = (58 * 117 + 58) * 3;
    assert_eq!(&rgb[at..at + 3], &[0x1e, 0x88, 0xe5]);
    assert!(answer.to_string().len() <= 64 * 1024);
    // Every screenshot is in the log: their number is what a flickering view is traced by. At
    // debug: MCP loops (color_sample) must not fill the rotating log.
    assert_eq!(host.logged.len(), 1);
    assert_eq!(host.logged[0].0, "debug");
    assert!(
        host.logged[0]
            .1
            .contains("view 4 read from its window image")
    );
}

#[test]
fn a_pipette_reads_clips_where_the_settings_ask_for_them() {
    let mut host = host();
    let mut clip = tile_message(10, 10, 40, 30, 1.0);
    clip["message"]["map"] = Value::Null;
    clip["message"]["clip"] = json!(true);
    let answer = handle(&mut host, &clip).expect("tile");
    assert_eq!(screenshots(&host), 1);
    assert!(host.cdp_sent[0].2.get("clip").is_some());
    assert!(answer.get("left").is_none());
}

#[test]
fn a_window_image_of_another_size_than_measured_is_refused() {
    // The cell changed (canvas zoom, a resize without a resize event): the page script measures
    // again; no clip, which would flicker.
    let mut host = host();
    let refused = handle(&mut host, &tile_message(10, 10, 80, 30, 1.0)).expect_err("refused");
    assert_eq!(
        refused.params.get("reason").map(String::as_str),
        Some("window_size")
    );
    assert_eq!(screenshots(&host), 1);
    assert!(
        host.logged
            .iter()
            .any(|(_, line)| line.contains("not the 80 x 30 px measured"))
    );
}

#[test]
fn a_failed_window_image_is_no_clip_either() {
    let mut host = host();
    host.queued.insert(
        "Page.captureScreenshot".to_owned(),
        [Err(rb_plugin_api::Failure::coded(
            rb_plugin_api::FailureKind::Other,
            "plugins.error.host_failed",
        ))]
        .into(),
    );
    let refused = handle(&mut host, &tile_message(10, 10, 40, 30, 1.0)).expect_err("failed");
    assert_eq!(refused.code.as_deref(), Some("plugins.error.host_failed"));
    assert_eq!(screenshots(&host), 1);
    assert!(host.logged.iter().any(|(_, line)| {
        line.contains("window image of view 4 failed: plugins.error.host_failed")
    }));
}

#[test]
fn a_take_by_the_display_s_colours_tells_the_panel() {
    let mut host = host();
    let take = json!({ "type": "take", "x": 10, "y": 20, "width": 40, "height": 30, "dpr": 1,
                       "scrollX": 0, "scrollY": 0, "map": map(40, 30, 1.0), "exact": false });
    let answer = handle(&mut host, &message(take)).expect("take");
    assert_eq!(screenshots(&host), 1);
    assert_eq!(answer["color"]["hex"], "#767676");
    assert_eq!(answer["$post"][0]["message"]["managed"], true);
}

#[test]
fn a_tile_at_the_edge_of_the_image_is_cut_short() {
    let mut host = host();
    let answer = handle(&mut host, &tile_message(39, 0, 40, 30, 1.0)).expect("tile");
    assert_eq!(
        (answer["left"].as_u64(), answer["top"].as_u64()),
        (Some(0), Some(0))
    );
    assert_eq!(
        (answer["width"].as_u64(), answer["height"].as_u64()),
        (Some(40), Some(30))
    );
}

#[test]
fn the_value_of_a_colour_takes_no_screenshot() {
    let mut host = host();
    host.settings
        .insert("defaultFormat".to_owned(), "\"rgb\"".to_owned());
    let answer = handle(
        &mut host,
        &message(json!({ "type": "value", "color": "#1e88e5" })),
    )
    .expect("value");
    assert_eq!(answer["value"], "rgb(30, 136, 229)");
    assert_eq!(screenshots(&host), 0);
    for color in [json!("red"), json!("#1e88e5ff"), json!("#12"), json!(7)] {
        let refused = handle(
            &mut host,
            &message(json!({ "type": "value", "color": color })),
        )
        .expect_err("refused");
        assert_eq!(refused.code.as_deref(), Some("plugins.error.invalid"));
    }
}

#[test]
fn a_view_the_plugin_does_not_read_takes_no_screenshot() {
    // RB-080-77, diag9: a Firefox view's page script asked for window images; CDP answered
    // `plugins.error.not_available` 33 times.
    let mut host = host();
    host.views[0].engine = "firefox".to_owned();
    for request in [
        tile_message(10, 10, 40, 30, 1.0),
        message(
            json!({ "type": "calibrate", "x": 1, "y": 1, "width": 40, "height": 30, "dpr": 1,
                        "scrollX": 0, "scrollY": 0, "marker": [[0, 0], [16, 18]] }),
        ),
    ] {
        let refused = handle(&mut host, &request).expect_err("refused");
        assert_eq!(
            refused.code.as_deref(),
            Some("plugin.color-picker.errors.unsupported_engine")
        );
    }
    assert_eq!(screenshots(&host), 0);
}

#[test]
fn a_view_without_markers_is_named_in_the_status_bar() {
    let mut host = host();
    let answer = handle(&mut host, &message(json!({ "type": "unreadable" }))).expect("notice");
    assert_eq!(answer["$post"][0]["target"], "status");
    assert_eq!(
        answer["$post"][0]["message"]["key"],
        "plugin.color-picker.notice.unreadable"
    );
    assert_eq!(answer["$post"][0]["message"]["params"]["view"], "View 4");
    assert_eq!(screenshots(&host), 0);
}

#[test]
fn the_status_bar_names_why_a_view_is_not_read() {
    // RB-090-19: the reason `calibrate` answered comes back with `unreadable`.
    let mut host = host();
    for (reason, name) in [
        (json!("size"), "unreadable_size"),
        (json!("image"), "unreadable_image"),
        (json!("markers"), "unreadable"),
        (json!("anything"), "unreadable"),
        (json!(7), "unreadable"),
    ] {
        let request = message(json!({ "type": "unreadable", "reason": reason }));
        let answer = handle(&mut host, &request).expect("notice");
        assert_eq!(
            answer["$post"][0]["message"]["key"],
            format!("plugin.color-picker.notice.{name}"),
            "{reason}"
        );
    }
    assert_eq!(screenshots(&host), 0);
}
