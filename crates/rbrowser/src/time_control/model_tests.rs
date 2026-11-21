use rb_config::{ClockUserSettings, EmulationUserSettings};
use rb_engine::{Target, ViewId};
use serde_json::{Value, json};

use super::*;

/// 2026-11-20 12:00:00 UTC, the real time of the tests.
const NOW: i64 = 1_795_176_000_000;

fn local(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> ClockStart {
    ClockStart::Local(LocalTime {
        year,
        month,
        day,
        hour,
        minute,
        second: 0,
        millis: 0,
    })
}

fn defaults(default: &str, speed: f64) -> ClockUserSettings {
    ClockUserSettings {
        default: default.to_owned(),
        speed,
    }
}

#[test]
fn a_time_without_a_zone_is_each_pages_local_time() {
    assert_eq!(
        parse_time("2026-12-24T10:00", NOW),
        Ok(local(2026, 12, 24, 10, 0))
    );
    assert_eq!(
        parse_time(" 2026-12-24 10:00 ", NOW),
        Ok(local(2026, 12, 24, 10, 0))
    );
    assert_eq!(parse_time("2026-12-24", NOW), Ok(local(2026, 12, 24, 0, 0)));
    assert_eq!(
        parse_time("2026-12-24T10:00:30.250", NOW),
        Ok(ClockStart::Local(LocalTime {
            year: 2026,
            month: 12,
            day: 24,
            hour: 10,
            minute: 0,
            second: 30,
            millis: 250,
        }))
    );
}

#[test]
fn a_time_with_a_zone_or_now_is_one_moment_for_every_view() {
    let utc = parse_time("2026-12-24T09:00:00Z", NOW).expect("utc");
    let berlin = parse_time("2026-12-24T10:00:00+01:00", NOW).expect("offset");
    assert_eq!(utc, ClockStart::Instant(1_798_102_800_000));
    assert_eq!(utc, berlin);
    assert_eq!(parse_time("", NOW), Ok(ClockStart::Instant(NOW)));
    assert_eq!(parse_time("now", NOW), Ok(ClockStart::Instant(NOW)));
    assert_eq!(parse_time("NOW", NOW), Ok(ClockStart::Instant(NOW)));
}

#[test]
fn a_time_that_is_none_is_refused_with_the_forms_taken() {
    for bad in [
        "tomorrow",
        "24.12.2026",
        "2026-13-01",
        "2026-02-30T10:00",
        "2026-12-24T25:00",
    ] {
        let error = parse_time(bad, NOW).expect_err(bad);
        assert!(error.contains("2026-12-24T10:00"), "{error}");
    }
}

#[test]
fn every_form_reads_as_it_did_with_chrono() {
    // RB-090-77: the fractions, the offsets and the floor of an instant before 1970.
    assert_eq!(
        parse_time("2026-12-24 10:00:30.5", NOW),
        Ok(ClockStart::Local(LocalTime {
            year: 2026,
            month: 12,
            day: 24,
            hour: 10,
            minute: 0,
            second: 30,
            millis: 500,
        }))
    );
    assert_eq!(
        parse_time("2026-12-24T09:00:00.250Z", NOW),
        Ok(ClockStart::Instant(1_798_102_800_250))
    );
    assert_eq!(
        parse_time("2026-12-24T09:00:00.123456789-01:30", NOW),
        Ok(ClockStart::Instant(1_798_108_200_123))
    );
    assert_eq!(
        parse_time("1969-12-31T23:59:59.9995Z", NOW),
        Ok(ClockStart::Instant(-1))
    );
    // Review w30: chrono also read a space or a `t` between date and time.
    for spaced in ["2026-12-24 09:00:00.250Z", "2026-12-24t09:00:00.250Z"] {
        assert_eq!(
            parse_time(spaced, NOW),
            Ok(ClockStart::Instant(1_798_102_800_250)),
            "{spaced}"
        );
    }
    for bad in [
        "0000-01-01",
        "0000-01-01T00:00:00Z",
        "10000-01-01T00:00:00Z",
        "2026-12-24T10",
        "2026-12-24T10:00Z",
        "2026-12-24T10:00:00+24:00",
    ] {
        assert!(parse_time(bad, NOW).is_err(), "{bad}");
    }
}

#[test]
fn the_start_reads_back_as_the_activity_shows_it() {
    assert_eq!(local(2026, 12, 24, 10, 0).text(), "2026-12-24T10:00:00");
    assert_eq!(
        ClockStart::Instant(1_798_102_800_000).text(),
        "2026-12-24T09:00:00.000Z"
    );
    assert_eq!(
        ClockStart::Instant(1_798_102_800_250).text(),
        "2026-12-24T09:00:00.250Z"
    );
    assert_eq!(ClockStart::Instant(-1).text(), "1969-12-31T23:59:59.999Z");
}

#[test]
fn the_page_gets_its_clock_as_data() {
    let fixed = Clock::starting(local(2026, 12, 24, 10, 0), 0.0, NOW);
    assert_eq!(
        fixed.config(),
        json!({ "local": [2026, 12, 24, 10, 0, 0, 0], "offset": 0.0, "anchor": NOW, "speed": 0.0 })
    );
    let running = Clock::starting(ClockStart::Instant(5_000), 2.0, NOW);
    assert_eq!(
        running.config(),
        json!({ "origin": 5_000, "offset": 0.0, "anchor": NOW, "speed": 2.0 })
    );
}

#[test]
fn a_new_speed_keeps_what_ran_and_starts_from_the_real_time() {
    let running = Clock::starting(ClockStart::Instant(5_000), 2.0, NOW);
    let slower = running.with_speed(1.0, NOW + 1_500);
    assert_eq!(slower.offset_ms, 3_000.0);
    assert_eq!(slower.anchor_ms, NOW + 1_500);
    assert_eq!(slower.speed, 1.0);
    assert_eq!(slower.start, ClockStart::Instant(5_000));
    // The fixed clock runs nothing before its change.
    let fixed = Clock::starting(local(2026, 12, 24, 10, 0), 0.0, NOW);
    assert_eq!(fixed.with_speed(1.0, NOW + 60_000).offset_ms, 0.0);
}

#[test]
fn the_clock_command_reads_its_arguments() {
    assert_eq!(parse_clock(&Value::Null, NOW), Ok(ClockRequest::Toggle));
    assert_eq!(parse_clock(&json!({}), NOW), Ok(ClockRequest::Toggle));
    assert_eq!(
        parse_clock(&json!({ "reset": true }), NOW),
        Ok(ClockRequest::Reset)
    );
    assert_eq!(
        parse_clock(&json!({ "time": "2026-12-24T10:00" }), NOW),
        Ok(ClockRequest::Set {
            start: Some(local(2026, 12, 24, 10, 0)),
            speed: None,
        })
    );
    assert_eq!(
        parse_clock(&json!({ "time": 1_798_102_800_000_i64, "speed": 60 }), NOW),
        Ok(ClockRequest::Set {
            start: Some(ClockStart::Instant(1_798_102_800_000)),
            speed: Some(60.0),
        })
    );
    assert_eq!(
        parse_clock(&json!({ "speed": 1 }), NOW),
        Ok(ClockRequest::Set {
            start: None,
            speed: Some(1.0),
        })
    );
}

#[test]
fn the_clock_command_refuses_what_it_cannot_do() {
    for bad in [
        json!({ "reset": true, "time": "2026-12-24" }),
        json!({ "reset": true, "speed": 1 }),
        json!({ "reset": false }),
        json!({ "speed": -1 }),
        json!({ "speed": 1000 }),
        json!({ "time": true }),
        json!({ "time": "soon" }),
        json!({ "target": "all" }),
        json!({ "zone": "UTC" }),
    ] {
        assert!(parse_clock(&bad, NOW).is_err(), "{bad}");
    }
}

#[test]
fn the_shortcut_sets_the_settings_clock_or_takes_a_running_one_back() {
    let settings = defaults("2026-12-24T10:00", 0.0);
    let set = next_clock(&ClockRequest::Toggle, None, &settings, NOW)
        .expect("set")
        .expect("a clock");
    assert_eq!(set.start, local(2026, 12, 24, 10, 0));
    assert_eq!(set.speed, 0.0);
    assert_eq!(
        next_clock(&ClockRequest::Toggle, Some(&set), &settings, NOW),
        Ok(None)
    );
    // The default setting freezes the moment the shortcut is pressed.
    let now = next_clock(&ClockRequest::Toggle, None, &defaults("", 0.0), NOW)
        .expect("set")
        .expect("a clock");
    assert_eq!(now.start, ClockStart::Instant(NOW));
    let broken = next_clock(&ClockRequest::Toggle, None, &defaults("soon", 0.0), NOW)
        .expect_err("a broken setting");
    assert!(broken.contains("emulation.clock.default"), "{broken}");
}

#[test]
fn a_time_alone_takes_the_running_speed_else_the_settings() {
    let settings = defaults("", 2.0);
    let request = ClockRequest::Set {
        start: Some(local(2026, 12, 24, 10, 0)),
        speed: None,
    };
    let first = next_clock(&request, None, &settings, NOW)
        .expect("set")
        .expect("a clock");
    assert_eq!(first.speed, 2.0);
    let running = Clock::starting(ClockStart::Instant(0), 5.0, NOW);
    let second = next_clock(&request, Some(&running), &settings, NOW)
        .expect("set")
        .expect("a clock");
    assert_eq!(second.speed, 5.0);
    assert_eq!(second.anchor_ms, NOW);
}

#[test]
fn a_speed_alone_goes_on_from_the_running_clock_or_the_real_time() {
    let settings = defaults("", 0.0);
    let request = ClockRequest::Set {
        start: None,
        speed: Some(10.0),
    };
    let fresh = next_clock(&request, None, &settings, NOW)
        .expect("set")
        .expect("a clock");
    assert_eq!(fresh.start, ClockStart::Instant(NOW));
    assert_eq!(fresh.speed, 10.0);
    let running = Clock::starting(ClockStart::Instant(0), 1.0, NOW);
    let faster = next_clock(&request, Some(&running), &settings, NOW + 2_000)
        .expect("set")
        .expect("a clock");
    assert_eq!(faster.start, ClockStart::Instant(0));
    assert_eq!(faster.offset_ms, 2_000.0);
    assert_eq!(
        next_clock(&ClockRequest::Reset, Some(&running), &settings, NOW),
        Ok(None)
    );
}

#[test]
fn the_animations_command_reads_its_arguments() {
    assert_eq!(
        parse_animations(&Value::Null),
        Ok(AnimationArgs {
            target: None,
            request: AnimationRequest::Cycle,
        })
    );
    assert_eq!(
        parse_animations(&json!({ "rate": 0.25, "target": 2 })),
        Ok(AnimationArgs {
            target: Some(Target::View(ViewId(2))),
            request: AnimationRequest::Rate(0.25),
        })
    );
    assert_eq!(
        parse_animations(&json!({ "step": true, "target": "all" })),
        Ok(AnimationArgs {
            target: Some(Target::All),
            request: AnimationRequest::Step(None),
        })
    );
    assert_eq!(
        parse_animations(&json!({ "step": 40 })),
        Ok(AnimationArgs {
            target: None,
            request: AnimationRequest::Step(Some(40)),
        })
    );
    for bad in [
        json!({ "rate": -0.1 }),
        json!({ "rate": 11 }),
        json!({ "rate": 0.5, "step": true }),
        json!({ "step": false }),
        json!({ "step": 0 }),
        json!({ "step": 60_001 }),
        json!({ "step": "1s" }),
        json!({ "speed": 1 }),
    ] {
        assert!(parse_animations(&bad).is_err(), "{bad}");
    }
}

#[test]
fn the_shortcut_steps_through_the_settings_rates() {
    let rates = rates_of(&[0.0, 0.1, 0.25, 1.0]);
    assert_eq!(next_rate(1.0, &rates), 0.0);
    assert_eq!(next_rate(0.0, &rates), 0.1);
    assert_eq!(next_rate(0.1, &rates), 0.25);
    assert_eq!(next_rate(0.25, &rates), 1.0);
    // A rate set elsewhere goes to the first of the list.
    assert_eq!(next_rate(0.7, &rates), 0.0);
}

#[test]
fn the_settings_rates_are_cleaned_and_keep_a_way_back_to_100_percent() {
    assert_eq!(rates_of(&[]), [0.0, 0.1, 0.25, 1.0]);
    assert_eq!(rates_of(&[0.5, f64::NAN, -1.0, 0.5, 20.0]), [0.5, 1.0]);
    assert_eq!(rates_of(&[1.0, 0.0]), [1.0, 0.0]);
}

#[test]
fn the_step_is_the_given_one_or_the_settings() {
    let settings = EmulationUserSettings {
        animation_step_ms: 40,
        ..EmulationUserSettings::default()
    };
    assert_eq!(step_ms(None, &settings), 40);
    assert_eq!(step_ms(Some(250), &settings), 250);
    let broken = EmulationUserSettings {
        animation_step_ms: 0,
        ..EmulationUserSettings::default()
    };
    assert_eq!(step_ms(None, &broken), 1);
}

#[test]
fn the_activity_names_what_is_changed_and_nothing_else() {
    assert_eq!(activity(None, &[(ViewId(1), 1.0)]), None);
    let clock = Clock::starting(local(2026, 12, 24, 10, 0), 0.0, NOW);
    let shown = activity(
        Some(&clock),
        &[(ViewId(3), 0.25), (ViewId(1), 0.0), (ViewId(2), 1.0)],
    )
    .expect("changed");
    assert_eq!(
        shown,
        TimeControlActivity {
            clock: Some(ClockActivity {
                start: "2026-12-24T10:00:00".into(),
                local: true,
                speed: 0.0,
            }),
            animations: vec![
                ViewRate {
                    view: ViewId(1),
                    rate: 0.0,
                },
                ViewRate {
                    view: ViewId(3),
                    rate: 0.25,
                },
            ],
        }
    );
}
