use serde_json::json;

use super::*;
use crate::fake::SweepHost;

#[test]
fn without_settings_the_sweep_runs_320_to_1920_in_steps_of_ten_for_every_kind() {
    let mut host = SweepHost::default();
    let settings = Settings::read(&mut host);
    assert_eq!(settings, Settings::default());
    assert_eq!(
        settings.span().expect("a span"),
        Span {
            from: 320,
            to: 1920,
            step: 10
        }
    );
    assert_eq!(settings.kinds, KINDS);
}

#[test]
fn the_settings_are_read_and_a_value_out_of_range_keeps_the_default() {
    let mut host = SweepHost::default();
    host.set("from", "360");
    host.set("to", "1440");
    host.set("step", "5");
    host.set("kinds", r#"["overlap","overflow","overflow","nope"]"#);
    let settings = Settings::read(&mut host);
    assert_eq!((settings.from, settings.to, settings.step), (360, 1440, 5));
    assert_eq!(
        settings.kinds,
        vec!["overflow", "overlap"],
        "known, once, in order"
    );
    let mut host = SweepHost::default();
    host.set("from", "50");
    host.set("to", "9999");
    host.set("step", "0");
    host.set("kinds", "[]");
    assert_eq!(Settings::read(&mut host), Settings::default());
}

#[test]
fn a_calls_arguments_win_over_the_settings_and_are_checked() {
    let settings = Settings::default()
        .with_args(&json!({ "from": 600, "to": 800, "step": 2, "kinds": ["text-clipped"] }))
        .expect("valid");
    assert_eq!((settings.from, settings.to, settings.step), (600, 800, 2));
    assert_eq!(settings.kinds, vec!["text-clipped"]);
    for (args, name) in [
        (json!({ "from": 100 }), "from"),
        (json!({ "to": 4000 }), "to"),
        (json!({ "step": 0 }), "step"),
        (json!({ "step": "10" }), "step"),
        (json!({ "kinds": ["nope"] }), "kinds"),
        (json!({ "kinds": [] }), "kinds"),
    ] {
        let failure = Settings::default().with_args(&args).expect_err("refused");
        assert_eq!(failure.code(), Some(rb_plugin_api::codes::INVALID));
        assert_eq!(failure.param("argument"), Some(name), "{args}");
    }
}

#[test]
fn the_first_width_must_not_pass_the_last() {
    let settings = Settings::default()
        .with_args(&json!({ "from": 900, "to": 800 }))
        .expect("each in range");
    let failure = settings.span().expect_err("from > to");
    assert_eq!(failure.param("argument"), Some("to"));
}

#[test]
fn a_shown_width_is_held_two_minutes_unless_the_setting_says_otherwise() {
    let mut host = SweepHost::default();
    assert_eq!(show_seconds(&mut host), SHOW_SECONDS);
    host.set("showSeconds", "30");
    assert_eq!(show_seconds(&mut host), 30);
    for bad in ["9", "241", "\"30\"", "-1"] {
        host.set("showSeconds", bad);
        assert_eq!(show_seconds(&mut host), SHOW_SECONDS, "{bad}");
    }
}
