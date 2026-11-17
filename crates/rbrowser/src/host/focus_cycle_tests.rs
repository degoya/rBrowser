use super::*;

fn all(_: FocusStop) -> bool {
    true
}

#[test]
fn tab_runs_toolbar_view_status_bar_and_round() {
    assert_eq!(
        next_stop(FocusStop::Toolbar, true, all),
        Some(FocusStop::ActiveView)
    );
    assert_eq!(
        next_stop(FocusStop::ActiveView, true, all),
        Some(FocusStop::StatusBar)
    );
    assert_eq!(
        next_stop(FocusStop::StatusBar, true, all),
        Some(FocusStop::Toolbar)
    );
}

#[test]
fn shift_tab_runs_the_other_way() {
    assert_eq!(
        next_stop(FocusStop::Toolbar, false, all),
        Some(FocusStop::StatusBar)
    );
    assert_eq!(
        next_stop(FocusStop::StatusBar, false, all),
        Some(FocusStop::ActiveView)
    );
    assert_eq!(
        next_stop(FocusStop::ActiveView, false, all),
        Some(FocusStop::Toolbar)
    );
}

#[test]
fn skips_a_hidden_status_bar_and_a_missing_view() {
    let no_bar = |stop: FocusStop| stop != FocusStop::StatusBar;
    assert_eq!(
        next_stop(FocusStop::ActiveView, true, no_bar),
        Some(FocusStop::Toolbar)
    );
    let no_view = |stop: FocusStop| stop != FocusStop::ActiveView;
    assert_eq!(
        next_stop(FocusStop::Toolbar, true, no_view),
        Some(FocusStop::StatusBar)
    );
}

#[test]
fn stays_when_no_other_stop_is_there() {
    let only_toolbar = |stop: FocusStop| stop == FocusStop::Toolbar;
    assert_eq!(next_stop(FocusStop::Toolbar, true, only_toolbar), None);
}
