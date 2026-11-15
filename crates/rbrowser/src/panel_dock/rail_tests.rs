use rb_commands::CommandError;
use rb_config::{PanelCloseTo, PanelDock, PanelDockAtStart, PanelDockState, UserSettings};
use serde_json::{Value, json};

use super::*;
use crate::host::ui::recording::UiCall;
use crate::panel_dock::collapse::{STRIP_WIDTH, strip};
use crate::panel_dock::tests::{docks, file_holds, setup_with, windows};
use crate::panel_dock::{dock, show, undock};

const PAGE: &str = "rbrowser://ui/settings.html#/panels";
const MAIN: &str = "rbrowser://ui/settings.html#/panels/hello-panel.main";

fn settings(dock: PanelDock, state: PanelDockState, start: PanelDockAtStart) -> PanelSettings {
    PanelSettings {
        dock,
        dock_state: state,
        dock_at_start: start,
        ..PanelSettings::default()
    }
}

fn user_with(panels: PanelSettings) -> UserSettings {
    UserSettings {
        panels,
        ..UserSettings::default()
    }
}

fn last(setup: &crate::panel_dock::tests::Setup) -> Option<DockedPanels> {
    docks(&setup.ui).last().cloned().flatten()
}

#[test]
fn the_start_rule_gives_the_rail_hidden_or_the_last_state() {
    use PanelDockAtStart::{Hidden, Last, Rail};
    use PanelDockState as S;
    let cases = [
        (PanelDock::Right, S::Open, Rail, PanelDock::Right, S::Rail),
        (PanelDock::Left, S::Hidden, Rail, PanelDock::Left, S::Rail),
        (
            PanelDock::Bottom,
            S::Open,
            Hidden,
            PanelDock::Bottom,
            S::Hidden,
        ),
        (PanelDock::Left, S::Open, Last, PanelDock::Left, S::Open),
        (PanelDock::Left, S::Hidden, Last, PanelDock::Left, S::Hidden),
        // Panels left in their own window start at the rail on the right; hidden stay hidden.
        (
            PanelDock::Floating,
            S::Open,
            Last,
            PanelDock::Right,
            S::Rail,
        ),
        (
            PanelDock::Floating,
            S::Open,
            Rail,
            PanelDock::Right,
            S::Rail,
        ),
        (
            PanelDock::Floating,
            S::Open,
            Hidden,
            PanelDock::Floating,
            S::Hidden,
        ),
    ];
    for (dock, state, start, want_dock, want_state) in cases {
        let started = at_start(&settings(dock, state, start));
        assert_eq!(
            (started.dock, started.dock_state),
            (want_dock, want_state),
            "{dock:?} {state:?} {start:?}"
        );
    }
}

#[test]
fn the_first_dock_is_the_rail_on_the_right_with_the_default_settings() {
    let first = first_dock(&PanelSettings::default()).expect("a rail");
    assert_eq!(
        first,
        DockedPanels {
            side: PanelSide::Right,
            size: 520,
            url: PAGE.to_owned(),
            collapsed: true,
        }
    );
    // The window lays it out at its strip from the first layout.
    assert_eq!(strip(first.side), STRIP_WIDTH);
    let hidden = settings(
        PanelDock::Right,
        PanelDockState::Rail,
        PanelDockAtStart::Hidden,
    );
    assert_eq!(first_dock(&hidden), None);
    let open = settings(
        PanelDock::Left,
        PanelDockState::Open,
        PanelDockAtStart::Last,
    );
    assert_eq!(
        first_dock(&open).map(|dock| (dock.side, dock.collapsed)),
        Some((PanelSide::Left, false))
    );
}

#[tokio::test]
async fn the_core_starts_with_the_rail_the_window_shows() {
    let setup = setup_with(UserSettings::default());
    on_start(&setup.core);
    assert_eq!(last(&setup), first_dock(&PanelSettings::default()));
    assert_eq!(
        setup.core.user_settings().panels.dock_state,
        PanelDockState::Rail
    );
}

#[tokio::test]
async fn a_start_from_the_own_window_writes_the_side_of_the_rail() {
    let setup = setup_with(user_with(settings(
        PanelDock::Floating,
        PanelDockState::Open,
        PanelDockAtStart::Rail,
    )));
    on_start(&setup.core);
    let rail = last(&setup).expect("a rail");
    assert_eq!((rail.side, rail.collapsed), (PanelSide::Right, true));
    file_holds(&setup.core, |file| {
        file["panels"]["dock"] == "right" && file["panels"]["dockState"] == "rail"
    })
    .await;
}

#[tokio::test]
async fn close_folds_to_the_rail_and_hides_only_when_asked() {
    let setup = setup_with(user_with(settings(
        PanelDock::Bottom,
        PanelDockState::Open,
        PanelDockAtStart::Rail,
    )));
    dock(&setup.core, PanelSide::Bottom, &Value::Null).expect("docks");
    assert!(!last(&setup).expect("docked").collapsed);
    let answer = close(&setup.core, &Value::Null).expect("closes");
    assert_eq!(
        answer,
        json!({ "position": "bottom", "size": 320, "collapsed": true })
    );
    assert!(last(&setup).expect("the rail").collapsed);
    assert!(
        setup
            .ui
            .calls()
            .contains(&UiCall::HidePageWindow(PageWindow::Panels))
    );
    file_holds(&setup.core, |file| file["panels"]["dockState"] == "rail").await;
    let answer = close(&setup.core, &json!({ "to": "hidden" })).expect("hides");
    assert_eq!(answer, json!({ "position": "closed", "state": "hidden" }));
    assert_eq!(docks(&setup.ui).last(), Some(&None));
    assert_eq!(
        setup.core.user_settings().panels.dock_state,
        PanelDockState::Hidden
    );
    for bad in [json!({ "to": "window" }), json!({ "to": true })] {
        assert!(
            matches!(close(&setup.core, &bad), Err(CommandError::InvalidArgs(_))),
            "{bad}"
        );
    }
}

#[tokio::test]
async fn close_hides_with_close_to_hidden() {
    let mut panels = settings(
        PanelDock::Left,
        PanelDockState::Open,
        PanelDockAtStart::Rail,
    );
    panels.close_to = PanelCloseTo::Hidden;
    let setup = setup_with(user_with(panels));
    dock(&setup.core, PanelSide::Left, &Value::Null).expect("docks");
    close(&setup.core, &Value::Null).expect("closes");
    assert_eq!(docks(&setup.ui).last(), Some(&None));
    // `to` still wins.
    close(&setup.core, &json!({ "to": "rail" })).expect("folds");
    assert!(last(&setup).expect("the rail").collapsed);
}

#[tokio::test]
async fn the_own_window_closed_by_hand_brings_the_rail_back_where_it_was() {
    let setup = setup_with(user_with(settings(
        PanelDock::Left,
        PanelDockState::Open,
        PanelDockAtStart::Rail,
    )));
    dock(&setup.core, PanelSide::Left, &Value::Null).expect("docks");
    undock(&setup.core, &Value::Null).expect("undocks");
    assert_eq!(docks(&setup.ui).last(), Some(&None));
    on_window_closed(&setup.core);
    let rail = last(&setup).expect("the rail");
    assert_eq!((rail.side, rail.collapsed), (PanelSide::Left, true));
    let panels = setup.core.user_settings().panels;
    assert_eq!(
        (panels.dock, panels.dock_state),
        (PanelDock::Left, PanelDockState::Rail)
    );
    // A window closed already changes nothing.
    let before = docks(&setup.ui).len();
    on_window_closed(&setup.core);
    assert_eq!(docks(&setup.ui).len(), before);
}

#[tokio::test]
async fn the_own_window_without_a_side_before_comes_back_on_the_right() {
    let setup = setup_with(user_with(settings(
        PanelDock::Floating,
        PanelDockState::Open,
        PanelDockAtStart::Rail,
    )));
    show(&setup.core, MAIN.to_owned());
    // The window's page names the panel it shows; the rail keeps it.
    crate::panel_dock::selected(&setup.core, "hello-panel", "main").expect("a panel id");
    on_window_closed(&setup.core);
    let rail = last(&setup).expect("the rail");
    assert_eq!(
        (rail.side, rail.collapsed, rail.url.as_str()),
        (PanelSide::Right, true, MAIN)
    );
}

#[tokio::test]
async fn hidden_panels_come_back_as_the_rail_when_a_command_opens_one() {
    let setup = setup_with(user_with(settings(
        PanelDock::Right,
        PanelDockState::Hidden,
        PanelDockAtStart::Rail,
    )));
    show(&setup.core, MAIN.to_owned());
    let rail = last(&setup).expect("the rail");
    assert_eq!((rail.collapsed, rail.url.as_str()), (true, MAIN));
    assert_eq!(
        setup.core.user_settings().panels.dock_state,
        PanelDockState::Rail
    );
}

#[tokio::test]
async fn the_panels_follow_a_changed_dock_state() {
    let setup = setup_with(user_with(settings(
        PanelDock::Right,
        PanelDockState::Hidden,
        PanelDockAtStart::Rail,
    )));
    let set = |state: PanelDockState| {
        let mut user = setup.core.user_settings();
        user.panels.dock_state = state;
        setup.core.replace_user_settings(user);
        on_settings_changed(&setup.core);
    };
    set(PanelDockState::Rail);
    assert!(last(&setup).expect("the rail").collapsed);
    set(PanelDockState::Open);
    assert!(!last(&setup).expect("open").collapsed);
    set(PanelDockState::Hidden);
    assert_eq!(docks(&setup.ui).last(), Some(&None));
    // Hidden ignores another side.
    let before = docks(&setup.ui).len();
    let mut user = setup.core.user_settings();
    user.panels.dock = PanelDock::Bottom;
    setup.core.replace_user_settings(user);
    on_settings_changed(&setup.core);
    assert_eq!(docks(&setup.ui).len(), before);
}

/// Review (wave c20): `floating` chosen in the settings while the panels stood at the rail was
/// taken for a closed window and wrote the side back at once - the own window could never be
/// chosen there. It opens the window now and keeps the choice; closing the window by hand still
/// brings the rail back where it was.
#[tokio::test]
async fn floating_chosen_at_the_rail_opens_the_own_window_and_keeps_the_choice() {
    let setup = setup_with(user_with(settings(
        PanelDock::Left,
        PanelDockState::Rail,
        PanelDockAtStart::Rail,
    )));
    on_start(&setup.core);
    let mut user = setup.core.user_settings();
    user.panels.dock = PanelDock::Floating;
    setup.core.replace_user_settings(user);
    on_settings_changed(&setup.core);
    assert_eq!(docks(&setup.ui).last(), Some(&None));
    assert_eq!(windows(&setup.ui), [PAGE.to_owned()]);
    let panels = setup.core.user_settings().panels;
    assert_eq!(
        (panels.dock, panels.dock_state),
        (PanelDock::Floating, PanelDockState::Open)
    );
    file_holds(&setup.core, |file| file["panels"]["dockState"] == "open").await;
    // A settings change after it keeps the window.
    on_settings_changed(&setup.core);
    assert_eq!(windows(&setup.ui).len(), 1);
    assert_eq!(setup.core.user_settings().panels.dock, PanelDock::Floating);
    // Closed by hand: the rail is back at the side it came from.
    on_window_closed(&setup.core);
    let rail = last(&setup).expect("the rail");
    assert_eq!((rail.side, rail.collapsed), (PanelSide::Left, true));
    assert_eq!(setup.core.user_settings().panels.dock, PanelDock::Left);
}

/// The own window open, `rail` chosen in the settings: the window closes, the rail comes back.
#[tokio::test]
async fn the_rail_chosen_while_the_own_window_is_open_closes_it() {
    let setup = setup_with(user_with(settings(
        PanelDock::Left,
        PanelDockState::Open,
        PanelDockAtStart::Last,
    )));
    dock(&setup.core, PanelSide::Left, &Value::Null).expect("docks");
    undock(&setup.core, &Value::Null).expect("undocks");
    let mut user = setup.core.user_settings();
    user.panels.dock_state = PanelDockState::Rail;
    setup.core.replace_user_settings(user);
    on_settings_changed(&setup.core);
    let rail = last(&setup).expect("the rail");
    assert_eq!((rail.side, rail.collapsed), (PanelSide::Left, true));
    assert!(
        setup
            .ui
            .calls()
            .contains(&UiCall::HidePageWindow(PageWindow::Panels))
    );
}

#[test]
fn folded_is_the_rail_and_open_is_open() {
    assert_eq!(state_of(true), PanelDockState::Rail);
    assert_eq!(state_of(false), PanelDockState::Open);
}
