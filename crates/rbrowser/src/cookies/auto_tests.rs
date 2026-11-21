use super::*;

#[test]
fn the_mode_reads_the_setting_and_falls_back_to_manual() {
    assert_eq!(Mode::of("manual"), Mode::Manual);
    assert_eq!(Mode::of("onStart"), Mode::OnStart);
    assert_eq!(Mode::of("onNavigate"), Mode::OnNavigate);
    assert_eq!(Mode::of("onstart"), Mode::Manual);
    assert_eq!(Mode::of(""), Mode::Manual);
}

#[test]
fn a_view_counts_as_started_when_it_turns_running() {
    let before = HashMap::from([
        (ViewId(1), ExternalStatus::Running),
        (ViewId(2), ExternalStatus::Starting),
        (ViewId(3), ExternalStatus::Restarting),
    ]);
    let now = [
        (ViewId(1), ExternalStatus::Running),
        (ViewId(2), ExternalStatus::Running),
        (ViewId(3), ExternalStatus::Failed),
        (ViewId(4), ExternalStatus::Running),
    ];
    assert_eq!(started(&before, &now), vec![ViewId(2), ViewId(4)]);
    assert!(started(&HashMap::new(), &[]).is_empty());
}

/// RB-090-38: the status poll wakes only while there are external views to watch - once at the
/// start, before anything was looked at, and never in a window of Chromium views alone.
#[test]
fn the_status_poll_runs_only_while_there_are_external_views() {
    assert!(needs_poll(None, false), "the first look");
    assert!(
        !needs_poll(Some(&HashMap::new()), false),
        "no external view: no wakeup"
    );
    let one = HashMap::from([(ViewId(3), ExternalStatus::Running)]);
    assert!(needs_poll(Some(&one), false));
    // A view switched to Firefox after the first look, its window not listed yet.
    assert!(needs_poll(Some(&HashMap::new()), true));
}
