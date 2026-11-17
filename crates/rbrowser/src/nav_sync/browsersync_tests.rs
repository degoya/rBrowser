use super::*;

const SITE: &str = "http://localhost:3000";

#[test]
fn a_detection_pauses_sync_once() {
    let mut guard = BrowsersyncGuard::default();
    assert_eq!(guard.detected(SITE, true, true), GuardAction::Pause);
    assert_eq!(guard.paused(), Some(SITE));
    // The next page of the same site, sync still off: nothing more to do.
    assert_eq!(guard.detected(SITE, true, false), GuardAction::Nothing);
}

#[test]
fn sync_turned_on_again_means_sync_anyway_for_that_site() {
    let mut guard = BrowsersyncGuard::default();
    guard.detected(SITE, true, true);
    // The person switched sync on ("sync anyway", the toolbar, Leader S S, MCP).
    assert_eq!(guard.detected(SITE, true, true), GuardAction::Allow);
    assert_eq!(guard.paused(), None);
    assert_eq!(guard.detected(SITE, true, true), GuardAction::Nothing);
    // Another site with Browsersync pauses again.
    assert_eq!(
        guard.detected("http://localhost:3001", true, true),
        GuardAction::Pause
    );
}

#[test]
fn nothing_happens_with_detection_off_or_sync_already_off() {
    let mut guard = BrowsersyncGuard::default();
    assert_eq!(guard.detected(SITE, false, true), GuardAction::Nothing);
    assert_eq!(guard.detected(SITE, true, false), GuardAction::Nothing);
    assert_eq!(guard.paused(), None, "sync was off by the person's choice");
}

#[test]
fn turning_detection_off_lifts_the_notice() {
    let mut guard = BrowsersyncGuard::default();
    guard.detected(SITE, true, true);
    guard.reset();
    assert_eq!(guard.paused(), None);
    assert_eq!(guard.detected(SITE, true, true), GuardAction::Pause);
}
