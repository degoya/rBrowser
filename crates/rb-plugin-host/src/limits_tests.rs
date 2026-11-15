use super::{DEFAULT_TIMEOUT_MILLISECONDS, MAX_FUEL, PluginLimits};

#[test]
fn the_defaults_pass_their_own_check() {
    let limits = PluginLimits::default();
    assert!(limits.check().is_ok());
    assert_eq!(limits.timeout(), DEFAULT_TIMEOUT_MILLISECONDS);
}

#[test]
fn a_manifest_cannot_ask_for_more_than_the_ceiling_or_for_nothing() {
    let greedy = PluginLimits {
        fuel: MAX_FUEL + 1,
        ..PluginLimits::default()
    };
    assert!(greedy.check().expect_err("over").contains("limits.fuel"));
    let none = PluginLimits {
        timeout_milliseconds: Some(0),
        ..PluginLimits::default()
    };
    assert!(
        none.check()
            .expect_err("zero")
            .contains("limits.timeout_milliseconds")
    );
}
