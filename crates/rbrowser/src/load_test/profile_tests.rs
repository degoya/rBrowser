use std::time::Duration;

use super::*;

fn ramp() -> Profile {
    Profile {
        users: 10,
        ramp_up_s: 10,
        hold_s: 20,
        ramp_down_s: 10,
        rps: None,
    }
}

fn at(seconds: f64) -> Duration {
    Duration::from_secs_f64(seconds)
}

#[test]
fn a_constant_profile_has_every_user_from_the_start_to_the_end() {
    let constant = Profile {
        users: 50,
        ramp_up_s: 0,
        hold_s: 300,
        ramp_down_s: 0,
        rps: None,
    };
    assert_eq!(users_at(&constant, at(0.0)), 50);
    assert_eq!(users_at(&constant, at(299.9)), 50);
    assert_eq!(users_at(&constant, at(300.0)), 0);
}

#[test]
fn a_ramp_climbs_holds_and_falls() {
    let profile = ramp();
    assert_eq!(users_at(&profile, at(0.0)), 1);
    assert_eq!(users_at(&profile, at(4.5)), 5);
    assert_eq!(users_at(&profile, at(9.99)), 10);
    assert_eq!(users_at(&profile, at(10.0)), 10);
    assert_eq!(users_at(&profile, at(29.9)), 10);
    assert_eq!(users_at(&profile, at(35.0)), 5);
    assert_eq!(users_at(&profile, at(39.9)), 1);
    assert_eq!(users_at(&profile, at(40.0)), 0);
}

#[test]
fn the_limits_come_from_the_settings_and_a_scenario_over_them_is_refused() {
    let limits = Limits::default();
    assert_eq!(limits.max_users, DEFAULT_MAX_USERS);
    assert_eq!(limits.max_duration_s, DEFAULT_MAX_DURATION_S);
    let mut profile = ramp();
    assert_eq!(check_limits(&profile, &limits), Ok(()));
    profile.users = 51;
    let over = check_limits(&profile, &limits).expect_err("over");
    assert_eq!(over.what, "users");
    assert_eq!(over.setting, MAX_USERS_KEY);
    assert_eq!((over.wanted, over.limit), (51.0, 50.0));
    profile.users = 50;
    profile.hold_s = 300;
    assert_eq!(
        check_limits(&profile, &limits).expect_err("long").what,
        "duration"
    );
    profile.hold_s = 20;
    profile.rps = Some(limits.max_rps + 1.0);
    assert_eq!(
        check_limits(&profile, &limits).expect_err("fast").what,
        "rps"
    );
}

#[test]
fn there_is_no_ceiling_but_the_settings() {
    let generous = Limits {
        max_users: 100_000,
        max_rps: 1_000_000.0,
        max_duration_s: 86_400,
    };
    let profile = Profile {
        users: 100_000,
        ramp_up_s: 0,
        hold_s: 86_400,
        ramp_down_s: 0,
        rps: Some(1_000_000.0),
    };
    assert_eq!(check_limits(&profile, &generous), Ok(()));
}

#[test]
fn the_rate_in_effect_is_the_scenarios_or_the_settings_ceiling() {
    let limits = Limits::default();
    let mut profile = ramp();
    assert_eq!(rate_of(&profile, &limits), limits.max_rps);
    profile.rps = Some(12.5);
    assert_eq!(rate_of(&profile, &limits), 12.5);
}

#[test]
fn the_limits_read_from_the_plugins_values_and_fall_back_on_bad_ones() {
    let mut values = serde_json::Map::new();
    values.insert(MAX_USERS_KEY.into(), serde_json::json!(500));
    values.insert(MAX_RPS_KEY.into(), serde_json::json!(0));
    values.insert(MAX_DURATION_KEY.into(), serde_json::json!("long"));
    let limits = Limits::of(&values);
    assert_eq!(limits.max_users, 500);
    assert_eq!(limits.max_rps, DEFAULT_MAX_RPS);
    assert_eq!(limits.max_duration_s, DEFAULT_MAX_DURATION_S);
}
