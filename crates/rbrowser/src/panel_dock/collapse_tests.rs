use rb_commands::CommandError;
use rb_config::{PanelDock, PanelDockState, UserSettings};
use serde_json::{Value, json};

use super::*;
use crate::panel_dock::tests::{docks, file_holds, setup, setup_with, windows};
use crate::panel_dock::{
    DockedPanels, PanelSide, Room, dock, on_room, on_settings_changed, on_title, show,
};

const MAIN: &str = "rbrowser://ui/settings.html#/panels/hello-panel.main";
const PAGE: &str = "rbrowser://ui/settings.html#/panels";

fn last_dock(setup: &crate::panel_dock::tests::Setup) -> DockedPanels {
    docks(&setup.ui).last().cloned().flatten().expect("docked")
}

#[tokio::test]
async fn toggle_folds_the_open_dock_and_opens_it_again_at_its_size() {
    let setup = setup();
    dock(&setup.core, PanelSide::Left, &json!({ "size": 600 })).expect("docks");
    assert!(!last_dock(&setup).collapsed);
    let answer = toggle(&setup.core, &Value::Null).expect("folds");
    assert_eq!(
        answer,
        json!({ "position": "left", "size": 600, "collapsed": true })
    );
    let folded = last_dock(&setup);
    assert_eq!(
        (folded.side, folded.size, folded.collapsed),
        (PanelSide::Left, 600, true)
    );
    assert_eq!(
        setup.core.user_settings().panels.dock_state,
        PanelDockState::Rail
    );
    file_holds(&setup.core, |file| file["panels"]["dockState"] == "rail").await;
    let answer = toggle(&setup.core, &Value::Null).expect("opens");
    assert_eq!(answer["collapsed"], false);
    let open = last_dock(&setup);
    assert_eq!((open.size, open.collapsed), (600, false));
    assert_eq!(
        setup.core.user_settings().panels.dock_state,
        PanelDockState::Open
    );
    file_holds(&setup.core, |file| file["panels"]["dockState"] == "open").await;
}

#[tokio::test]
async fn collapsed_sets_the_state_instead_of_flipping_it() {
    let setup = setup();
    dock(&setup.core, PanelSide::Bottom, &Value::Null).expect("docks");
    for _ in 0..2 {
        toggle(&setup.core, &json!({ "collapsed": true })).expect("folds");
        assert!(last_dock(&setup).collapsed);
    }
    toggle(&setup.core, &json!({ "collapsed": false })).expect("opens");
    assert!(!last_dock(&setup).collapsed);
    for bad in [json!({ "collapsed": "yes" }), json!({ "collapsed": 1 })] {
        assert!(
            matches!(toggle(&setup.core, &bad), Err(CommandError::InvalidArgs(_))),
            "{bad}"
        );
    }
}

/// RB-080-93: panels in their own window open it, and fold by closing it - the rail comes
/// back docked (right, with no side before).
#[tokio::test]
async fn floating_panels_open_their_window_and_fold_back_to_the_rail() {
    let mut user = UserSettings::default();
    user.panels.dock = PanelDock::Floating;
    user.panels.dock_state = PanelDockState::Hidden;
    let setup = setup_with(user);
    let answer = toggle(&setup.core, &Value::Null).expect("opens");
    assert_eq!(
        answer,
        json!({ "position": "floating", "collapsed": false })
    );
    assert_eq!(windows(&setup.ui), [PAGE.to_owned()]);
    assert!(docks(&setup.ui).is_empty());
    let answer = toggle(&setup.core, &Value::Null).expect("folds");
    assert_eq!(answer["collapsed"], true);
    let rail = last_dock(&setup);
    assert_eq!((rail.side, rail.collapsed), (PanelSide::Right, true));
    let panels = setup.core.user_settings().panels;
    assert_eq!(
        (panels.dock, panels.dock_state),
        (PanelDock::Right, PanelDockState::Rail)
    );
}

#[tokio::test]
async fn toggle_on_a_closed_dock_opens_it() {
    let mut user = UserSettings::default();
    user.panels.dock = PanelDock::Right;
    user.panels.dock_state = PanelDockState::Rail;
    let setup = setup_with(user);
    let answer = toggle(&setup.core, &Value::Null).expect("opens");
    assert_eq!(answer["collapsed"], false);
    let open = last_dock(&setup);
    assert_eq!((open.side, open.collapsed), (PanelSide::Right, false));
}

/// Review point 2: a check run opened the findings by `show`, which unfolded the dock and wrote
/// `panels.dockCollapsed = false` without the person. An open that is no person's choice keeps
/// the fold and writes nothing; the page marks the entry in the strip.
#[tokio::test]
async fn an_open_by_a_command_keeps_the_fold_and_writes_nothing() {
    let mut user = UserSettings::default();
    user.panels.dock = PanelDock::Bottom;
    user.panels.dock_state = PanelDockState::Rail;
    let setup = setup_with(user);
    show(&setup.core, PAGE.to_owned());
    assert!(last_dock(&setup).collapsed);
    show(&setup.core, MAIN.to_owned());
    assert!(last_dock(&setup).collapsed);
    crate::panel_dock::core_panels::open(&setup.core, crate::panel_dock::core_panels::FINDINGS);
    assert!(last_dock(&setup).collapsed);
    assert_eq!(
        setup.core.user_settings().panels.dock_state,
        PanelDockState::Rail
    );
}

/// Review point 6: another side keeps the fold; `collapsed` sets it.
#[tokio::test]
async fn docking_keeps_the_fold_unless_collapsed_says_otherwise() {
    let setup = setup();
    dock(&setup.core, PanelSide::Left, &Value::Null).expect("docks");
    toggle(&setup.core, &Value::Null).expect("folds");
    let answer = dock(&setup.core, PanelSide::Right, &Value::Null).expect("moves");
    assert_eq!(answer["collapsed"], true);
    let folded = last_dock(&setup);
    assert_eq!((folded.side, folded.collapsed), (PanelSide::Right, true));
    let answer = dock(
        &setup.core,
        PanelSide::Bottom,
        &json!({ "collapsed": false }),
    )
    .expect("opens");
    assert_eq!(answer["collapsed"], false);
    assert!(!last_dock(&setup).collapsed);
    assert_eq!(
        setup.core.user_settings().panels.dock_state,
        PanelDockState::Open
    );
    assert!(matches!(
        dock(
            &setup.core,
            PanelSide::Bottom,
            &json!({ "collapsed": "no" })
        ),
        Err(CommandError::InvalidArgs(_))
    ));
}

/// Review point 3: undocking a folded dock left it folded; docked again from the setting the
/// window became a bare icon column.
#[tokio::test]
async fn undocking_ends_the_fold() {
    let setup = setup();
    dock(&setup.core, PanelSide::Left, &Value::Null).expect("docks");
    toggle(&setup.core, &Value::Null).expect("folds");
    crate::panel_dock::undock(&setup.core, &Value::Null).expect("undocks");
    assert_eq!(
        setup.core.user_settings().panels.dock_state,
        PanelDockState::Open
    );
    file_holds(&setup.core, |file| file["panels"]["dockState"] == "open").await;
    let mut user = setup.core.user_settings();
    user.panels.dock = PanelDock::Right;
    setup.core.replace_user_settings(user);
    on_settings_changed(&setup.core);
    let docked = last_dock(&setup);
    assert_eq!((docked.side, docked.collapsed), (PanelSide::Right, false));
}

/// Review point 10: the setting is the one source - the toggle flips what the page shows.
#[tokio::test]
async fn toggle_flips_the_setting_the_page_shows() {
    let setup = setup();
    dock(&setup.core, PanelSide::Left, &Value::Null).expect("docks");
    // The setting says folded before the shown dock heard it.
    let mut user = setup.core.user_settings();
    user.panels.dock_state = PanelDockState::Rail;
    setup.core.replace_user_settings(user);
    let answer = toggle(&setup.core, &Value::Null).expect("opens");
    assert_eq!(answer["collapsed"], false);
    assert!(!last_dock(&setup).collapsed);
    assert_eq!(
        setup.core.user_settings().panels.dock_state,
        PanelDockState::Open
    );
}

#[tokio::test]
async fn a_folded_dock_neither_drags_nor_takes_the_strip_as_its_size() {
    let setup = setup();
    dock(&setup.core, PanelSide::Right, &Value::Null).expect("docks");
    on_room(
        &setup.core,
        Room {
            side: PanelSide::Right,
            shown: 500,
            ceiling: 900,
        },
    );
    toggle(&setup.core, &Value::Null).expect("folds");
    let before = docks(&setup.ui).len();
    on_title(&setup.core, "rb-splitter:move:1:-40");
    on_title(&setup.core, "rb-splitter:end:2:-40");
    assert_eq!(docks(&setup.ui).len(), before);
    assert_eq!(setup.core.user_settings().panels.dock_size.right, 520);
    // A layout of the strip is no size of the dock: a step after opening starts from 500.
    on_room(
        &setup.core,
        Room {
            side: PanelSide::Right,
            shown: strip(PanelSide::Right),
            ceiling: 900,
        },
    );
    let answer = dock(&setup.core, PanelSide::Right, &json!({ "step": 1 })).expect("larger");
    assert_eq!(answer["size"], 540);
}

#[tokio::test]
async fn the_open_dock_follows_a_changed_collapsed_setting() {
    let setup = setup();
    dock(&setup.core, PanelSide::Left, &Value::Null).expect("docks");
    let mut user = setup.core.user_settings();
    user.panels.dock_state = PanelDockState::Rail;
    setup.core.replace_user_settings(user);
    on_settings_changed(&setup.core);
    assert!(last_dock(&setup).collapsed);
    let before = docks(&setup.ui).len();
    on_settings_changed(&setup.core);
    assert_eq!(docks(&setup.ui).len(), before);
}

#[test]
fn the_strip_is_narrower_than_any_open_dock() {
    for side in PanelSide::ALL {
        assert!(strip(side) > 0 && strip(side) < crate::devtools::layout::MIN_DOCK);
    }
    assert_eq!(strip(PanelSide::Left), STRIP_WIDTH);
    assert_eq!(strip(PanelSide::Right), STRIP_WIDTH);
    assert_eq!(strip(PanelSide::Bottom), STRIP_HEIGHT);
}
