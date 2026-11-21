use super::{PROBE_SESSION, prober, should_start};

#[test]
fn the_probe_session_is_no_config_session() {
    // A project config can never name it, so no view is ever moved into the probe's context.
    assert!(!rb_config::is_valid_session_id(PROBE_SESSION));
    assert_ne!(PROBE_SESSION, rb_config::DEFAULT_SESSION);
}

#[test]
fn there_is_no_prober_before_a_window_starts_one() {
    assert!(prober().is_none());
}

#[test]
fn a_consent_check_loaded_live_starts_the_prober_once() {
    // RB-092-06: a plugin granted `consent-probe` installed while the app runs starts the prober
    // as the window's start would; a running one is kept, none is started without such a plugin.
    assert!(should_start(true, false));
    assert!(!should_start(true, true));
    assert!(!should_start(false, false));
    assert!(!should_start(false, true));
}
