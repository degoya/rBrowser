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
