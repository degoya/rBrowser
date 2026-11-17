use std::time::Duration;

use serde_json::json;

use super::*;

fn parse(value: serde_json::Value) -> ScreenshotArgs {
    serde_json::from_value(value).expect("valid arguments")
}

#[test]
fn the_arguments_are_camel_case_and_strict() {
    let args = parse(json!({
        "target": "all", "mode": "fullPage", "overview": true, "delayMs": 5000, "save": false
    }));
    assert_eq!(args.target, Some(Target::All));
    assert_eq!(args.mode, Some(CaptureMode::FullPage));
    assert!(serde_json::from_value::<ScreenshotArgs>(json!({ "nope": 1 })).is_err());
    let one = parse(json!({ "target": 3, "mode": "element", "selector": "#a", "padding": 8 }));
    assert_eq!(one.target, Some(Target::View(ViewId(3))));
}

#[test]
fn the_defaults_fill_what_is_left_out() {
    let preset = ScreenshotArgs {
        target: Some(Target::All),
        mode: Some(CaptureMode::FullPage),
        ..ScreenshotArgs::default()
    };
    let args = parse(json!({ "mode": "viewport" })).or(&preset);
    assert_eq!(args.target, Some(Target::All));
    assert_eq!(args.mode, Some(CaptureMode::Viewport));
    let request = ScreenshotArgs::default()
        .request(16, Duration::from_secs(5), false)
        .expect("valid");
    assert_eq!(request.target, None);
    assert_eq!(request.mode, CaptureMode::Viewport);
    assert_eq!(request.padding, 16);
    assert_eq!(request.delay, Duration::from_secs(5));
    assert!(request.save);
    assert!(!request.overview);
}

#[test]
fn an_explicit_delay_wins_over_the_cycle() {
    let request = parse(json!({ "delayMs": 250 }))
        .request(16, Duration::from_secs(10), false)
        .expect("valid");
    assert_eq!(request.delay, Duration::from_millis(250));
}

#[test]
fn bad_combinations_are_refused() {
    let refused = |value: serde_json::Value, pick: bool| {
        parse(value).request(16, Duration::ZERO, pick).is_err()
    };
    assert!(
        refused(json!({ "mode": "element" }), false),
        "no selector over MCP"
    );
    assert!(
        !refused(json!({ "mode": "element" }), true),
        "a command may pick"
    );
    assert!(
        refused(json!({ "mode": "element", "target": "all" }), true),
        "pick needs one view"
    );
    assert!(refused(
        json!({ "mode": "element", "selector": "  " }),
        false
    ));
    assert!(refused(
        json!({ "mode": "element", "selector": "a".repeat(2000) }),
        false
    ));
    assert!(
        refused(json!({ "selector": "#a" }), false),
        "selector without element"
    );
    assert!(refused(
        json!({ "mode": "element", "selector": "#a", "overview": true }),
        false
    ));
    assert!(refused(json!({ "padding": 10_000 }), false));
    assert!(refused(json!({ "delayMs": 600_000 }), false));
}

/// The context menu's "Element under the cursor" (RB-080-43): the point names the element, so
/// nobody picks - over MCP too.
#[test]
fn a_point_names_the_element_instead_of_a_pick_or_a_selector() {
    let request = parse(json!({ "mode": "element", "target": 2, "at": { "x": 12, "y": 40 } }))
        .request(16, Duration::ZERO, false)
        .expect("a point needs no pick");
    assert_eq!(
        request.at,
        Some(PointArg {
            x: 12,
            y: 40,
            dip: false
        })
    );
    let dip = parse(json!({ "mode": "element", "at": { "x": 1, "y": 2, "dip": true } }))
        .request(16, Duration::ZERO, false)
        .expect("a DIP point");
    assert_eq!(dip.at.map(|point| point.dip), Some(true));
    assert_eq!(request.selector, None);
    let refused =
        |value: serde_json::Value| parse(value).request(16, Duration::ZERO, true).is_err();
    assert!(
        refused(json!({ "mode": "element", "selector": "#a", "at": { "x": 1, "y": 2 } })),
        "a selector or a point, not both"
    );
    assert!(
        refused(json!({ "mode": "viewport", "at": { "x": 1, "y": 2 } })),
        "a point is only for mode element"
    );
    assert!(
        refused(json!({ "mode": "element", "target": "all", "at": { "x": 1, "y": 2 } })),
        "a point is in one view"
    );
    assert!(
        refused(json!({ "mode": "element", "at": { "x": -1, "y": 2 } })),
        "a negative coordinate"
    );
    let preset = ScreenshotArgs {
        mode: Some(CaptureMode::Element),
        ..ScreenshotArgs::default()
    };
    let merged = parse(json!({ "at": { "x": 3, "y": 4 } })).or(&preset);
    assert_eq!(
        merged.at,
        Some(PointArg {
            x: 3,
            y: 4,
            dip: false
        })
    );
}

#[test]
fn the_answer_lists_paths_and_leaves_out_the_empty() {
    let answer = ScreenshotAnswer {
        shots: vec![ShotAnswer {
            view: Some(ViewId(1)),
            label: "Mobile".to_owned(),
            width: 1179,
            height: 2556,
            path: Some("/p/a.png".to_owned()),
            ..ShotAnswer::default()
        }],
        overview: Some(ShotAnswer {
            label: "overview".to_owned(),
            path: Some("/p/overview.png".to_owned()),
            ..ShotAnswer::default()
        }),
        folder: Some("/p".to_owned()),
    };
    assert_eq!(answer.paths(), vec!["/p/a.png", "/p/overview.png"]);
    let value = serde_json::to_value(&answer).expect("serialises");
    assert_eq!(value["shots"][0]["view"], 1);
    assert!(value["shots"][0].get("pngBase64").is_none());
    assert!(value["overview"].get("view").is_none());
}

#[test]
fn hide_banners_and_freeze_animations_reach_the_request() {
    let request = parse(json!({ "hideBanners": false, "freezeAnimations": true }))
        .request(16, Duration::ZERO, false)
        .expect("valid");
    assert_eq!(request.clean.hide_banners, Some(false));
    assert_eq!(request.clean.freeze_animations, Some(true));
    let unset = ScreenshotArgs::default()
        .request(16, Duration::ZERO, false)
        .expect("valid");
    assert_eq!(unset.clean, rb_visual::CleanOverrides::default());
}
