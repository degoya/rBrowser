use std::time::Duration;

use rb_config::{PanelDock, PanelDockState, UserSettings};
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

pub(crate) struct Setup {
    pub(crate) ui: Arc<RecordingUi>,
    pub(crate) core: Arc<Core>,
    pub(crate) _dir: tempfile::TempDir,
}

pub(crate) fn setup_with(user: UserSettings) -> Setup {
    let dir = tempfile::tempdir().expect("temp dir");
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(
        host,
        ui.clone(),
        Handle::current(),
        CoreSetup {
            user,
            user_path: Some(dir.path().join("settings.json")),
            ..CoreSetup::default()
        },
    );
    Setup {
        ui,
        core,
        _dir: dir,
    }
}

/// The panels as before RB-080-93 - in their own window, open when shown - which most tests
/// here start from; `rail_tests.rs` starts from today's defaults (the rail on the right).
pub(crate) fn setup() -> Setup {
    let mut user = UserSettings::default();
    user.panels.dock = PanelDock::Floating;
    user.panels.dock_state = PanelDockState::Open;
    setup_with(user)
}

/// The panels in their own window, as before RB-080-93 made the docked rail the default: for
/// tests elsewhere that open the panels window.
pub(crate) fn float(core: &Core) {
    let mut user = core.user_settings();
    user.panels.dock = PanelDock::Floating;
    core.replace_user_settings(user);
}

/// Every dock the window was asked for, in order.
pub(crate) fn docks(ui: &RecordingUi) -> Vec<Option<DockedPanels>> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::PanelDock(dock) => Some(dock),
            _ => None,
        })
        .collect()
}

fn last_dock(ui: &RecordingUi) -> Option<DockedPanels> {
    docks(ui).last().cloned().flatten()
}

pub(crate) fn windows(ui: &RecordingUi) -> Vec<String> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::PageWindow(PageWindow::Panels, url) => Some(url),
            _ => None,
        })
        .collect()
}

/// The settings file once `done` holds for it.
pub(crate) async fn file_holds(core: &Core, done: impl Fn(&Value) -> bool) {
    let path = core
        .user_settings_path()
        .expect("a settings file")
        .to_owned();
    for _ in 0..400 {
        let file = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok());
        if file.as_ref().is_some_and(&done) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("the settings file never held it");
}

const MAIN: &str = "rbrowser://ui/settings.html#/panels/hello-panel.main";

#[test]
fn a_side_takes_its_own_size_within_the_bounds() {
    let sizes = rb_config::PanelDockSize {
        left: 300,
        right: 50,
        bottom: 9_000,
    };
    assert_eq!(PanelSide::Left.size_in(&sizes), 300);
    assert_eq!(PanelSide::Right.size_in(&sizes), MIN_DOCK);
    assert_eq!(PanelSide::Bottom.size_in(&sizes), MAX_DOCK);
    assert_eq!(PanelSide::of(PanelDock::Floating), None);
    for side in PanelSide::ALL {
        assert_eq!(PanelSide::of(side.setting()), Some(side));
    }
    assert!(PanelSide::Left.horizontal() && PanelSide::Right.horizontal());
    assert!(!PanelSide::Bottom.horizontal());
}

#[test]
fn dragging_towards_the_views_grows_the_dock_on_every_side() {
    // Left: the bar is right of the panels, a move to the right grows them.
    assert_eq!(dragged(PanelSide::Left, 400, 60), 460);
    assert_eq!(dragged(PanelSide::Left, 400, -60), 340);
    // Right and bottom: a move left or up grows them.
    assert_eq!(dragged(PanelSide::Right, 400, -60), 460);
    assert_eq!(dragged(PanelSide::Bottom, 300, 40), 260);
    assert_eq!(dragged(PanelSide::Bottom, 200, 5_000), MIN_DOCK);
    assert_eq!(dragged(PanelSide::Left, 3_900, 5_000), MAX_DOCK);
}

#[test]
fn a_size_argument_is_checked() {
    assert_eq!(size_arg(&Value::Null), Ok(None));
    assert_eq!(size_arg(&json!({})), Ok(None));
    assert_eq!(size_arg(&json!({ "size": 400 })), Ok(Some(400)));
    for bad in [
        json!({ "size": 119 }),
        json!({ "size": 4001 }),
        json!({ "size": -1 }),
        json!({ "size": "400" }),
        json!({ "size": 400.5 }),
    ] {
        assert!(
            matches!(size_arg(&bad), Err(CommandError::InvalidArgs(_))),
            "{bad}"
        );
    }
}

#[tokio::test]
async fn floating_panels_open_the_window_as_before() {
    let setup = setup();
    show(&setup.core, MAIN.to_owned());
    assert_eq!(windows(&setup.ui), [MAIN.to_owned()]);
    assert!(docks(&setup.ui).is_empty());
}

#[tokio::test]
async fn with_a_dock_setting_the_panels_open_docked_at_its_size() {
    let mut user = UserSettings::default();
    user.panels.dock = PanelDock::Right;
    user.panels.dock_state = PanelDockState::Open;
    user.panels.dock_size.right = 610;
    let setup = setup_with(user);
    show(&setup.core, MAIN.to_owned());
    assert!(windows(&setup.ui).is_empty());
    assert_eq!(
        last_dock(&setup.ui),
        Some(DockedPanels {
            side: PanelSide::Right,
            size: 610,
            url: MAIN.to_owned(),
            collapsed: false,
        })
    );
}

#[tokio::test]
async fn docking_hides_the_window_docks_and_keeps_the_side() {
    let setup = setup();
    let args = json!({ "plugin": "hello-panel", "panel": "main" });
    let answer = dock(&setup.core, PanelSide::Left, &args).expect("docks");
    assert_eq!(
        answer,
        json!({ "position": "left", "size": 520, "collapsed": false })
    );
    assert!(
        setup
            .ui
            .calls()
            .contains(&UiCall::HidePageWindow(PageWindow::Panels))
    );
    let docked = last_dock(&setup.ui).expect("docked");
    assert_eq!(
        (docked.side, docked.size, docked.url.as_str()),
        (PanelSide::Left, 520, MAIN)
    );
    assert_eq!(setup.core.user_settings().panels.dock, PanelDock::Left);
    file_holds(&setup.core, |file| file["panels"]["dock"] == "left").await;
    // Another side moves the open dock; a size given is kept for that side.
    dock(&setup.core, PanelSide::Bottom, &json!({ "size": 250 })).expect("moves");
    let docked = last_dock(&setup.ui).expect("docked");
    assert_eq!((docked.side, docked.size), (PanelSide::Bottom, 250));
    let panels = setup.core.user_settings().panels;
    assert_eq!(
        (panels.dock, panels.dock_size.bottom),
        (PanelDock::Bottom, 250)
    );
    assert_eq!(panels.dock_size.left, 520);
    file_holds(&setup.core, |file| {
        file["panels"]["dock"] == "bottom" && file["panels"]["dockSize"]["bottom"] == 250
    })
    .await;
}

#[tokio::test]
async fn undocking_restores_the_window_at_the_docked_panel() {
    let setup = setup();
    dock(
        &setup.core,
        PanelSide::Right,
        &json!({ "plugin": "hello-panel", "panel": "main" }),
    )
    .expect("docks");
    let answer = undock(&setup.core, &Value::Null).expect("undocks");
    assert_eq!(answer, json!({ "position": "floating" }));
    assert_eq!(docks(&setup.ui).last(), Some(&None));
    assert_eq!(windows(&setup.ui), [MAIN.to_owned()]);
    assert_eq!(setup.core.user_settings().panels.dock, PanelDock::Floating);
    // The size of the side stays for the next dock.
    dock(&setup.core, PanelSide::Right, &Value::Null).expect("docks again");
    assert_eq!(last_dock(&setup.ui).expect("docked").size, 520);
}

#[tokio::test]
async fn close_hidden_removes_the_dock_and_hides_the_window() {
    let setup = setup();
    dock(&setup.core, PanelSide::Bottom, &Value::Null).expect("docks");
    close(&setup.core, &json!({ "to": "hidden" })).expect("hides");
    assert_eq!(docks(&setup.ui).last(), Some(&None));
    assert!(
        setup
            .ui
            .calls()
            .contains(&UiCall::HidePageWindow(PageWindow::Panels))
    );
    // The side stays the setting: the next open docks there again, as the rail.
    show(&setup.core, MAIN.to_owned());
    let docked = last_dock(&setup.ui).expect("docked");
    assert_eq!((docked.side, docked.collapsed), (PanelSide::Bottom, true));
}

#[tokio::test]
async fn a_bad_size_changes_nothing() {
    let setup = setup();
    let refused = dock(&setup.core, PanelSide::Left, &json!({ "size": 10 }));
    assert!(matches!(refused, Err(CommandError::InvalidArgs(_))));
    assert!(docks(&setup.ui).is_empty());
    assert_eq!(setup.core.user_settings().panels.dock, PanelDock::Floating);
}

#[tokio::test]
async fn a_drag_resizes_live_and_keeps_its_end_for_the_side() {
    let setup = setup();
    dock(&setup.core, PanelSide::Left, &Value::Null).expect("docks");
    on_title(&setup.core, "rb-splitter:move:1:30");
    on_title(&setup.core, "rb-splitter:move:2:80");
    let live: Vec<u32> = docks(&setup.ui)
        .into_iter()
        .flatten()
        .map(|dock| dock.size)
        .collect();
    assert_eq!(live[live.len() - 2..], [550, 600]);
    assert_eq!(setup.core.user_settings().panels.dock_size.left, 520);
    on_title(&setup.core, "rb-splitter:end:3:100");
    assert_eq!(setup.core.user_settings().panels.dock_size.left, 620);
    assert_eq!(last_dock(&setup.ui).expect("docked").size, 620);
    file_holds(&setup.core, |file| {
        file["panels"]["dockSize"]["left"] == 620
    })
    .await;
    // The next drag starts from there.
    on_title(&setup.core, "rb-splitter:move:4:-20");
    assert_eq!(last_dock(&setup.ui).expect("docked").size, 600);
}

#[tokio::test]
async fn without_a_dock_the_bar_does_nothing() {
    let setup = setup();
    on_title(&setup.core, "rb-splitter:end:1:100");
    on_title(&setup.core, "not a report");
    assert!(docks(&setup.ui).is_empty());
}

#[tokio::test]
async fn the_open_dock_follows_a_changed_setting() {
    let setup = setup();
    dock(
        &setup.core,
        PanelSide::Left,
        &json!({ "plugin": "hello-panel", "panel": "main" }),
    )
    .expect("docks");
    let mut user = setup.core.user_settings();
    user.panels.dock = PanelDock::Right;
    user.panels.dock_size.right = 700;
    setup.core.replace_user_settings(user);
    on_settings_changed(&setup.core);
    let docked = last_dock(&setup.ui).expect("docked");
    assert_eq!((docked.side, docked.size), (PanelSide::Right, 700));
    // Unchanged settings ask the window for nothing.
    let before = docks(&setup.ui).len();
    on_settings_changed(&setup.core);
    assert_eq!(docks(&setup.ui).len(), before);
    // Floating: the dock goes and the window opens at its panel.
    let mut user = setup.core.user_settings();
    user.panels.dock = PanelDock::Floating;
    setup.core.replace_user_settings(user);
    on_settings_changed(&setup.core);
    assert_eq!(docks(&setup.ui).last(), Some(&None));
    assert_eq!(windows(&setup.ui), [MAIN.to_owned()]);
}

#[tokio::test]
async fn hidden_panels_ignore_a_changed_side() {
    let setup = setup();
    let mut user = setup.core.user_settings();
    user.panels.dock_state = PanelDockState::Hidden;
    user.panels.dock = PanelDock::Bottom;
    setup.core.replace_user_settings(user);
    on_settings_changed(&setup.core);
    assert!(docks(&setup.ui).iter().all(Option::is_none));
    assert!(windows(&setup.ui).is_empty());
}

/// RB-080-93: a side named while nothing is shown brings the panels there - open or the rail,
/// as `panels.dockState` says; they no longer wait for the next open.
#[tokio::test]
async fn a_side_named_while_nothing_shows_docks_the_panels() {
    let setup = setup();
    let mut user = setup.core.user_settings();
    user.panels.dock = PanelDock::Bottom;
    setup.core.replace_user_settings(user);
    on_settings_changed(&setup.core);
    let docked = last_dock(&setup.ui).expect("docked");
    assert_eq!((docked.side, docked.collapsed), (PanelSide::Bottom, false));
    assert!(windows(&setup.ui).is_empty());
}

const LOG: &str = "rbrowser://ui/settings.html#/panels/hello-panel.log";

fn room(side: PanelSide, shown: u32, ceiling: u32) -> Room {
    Room {
        side,
        shown,
        ceiling,
    }
}

#[tokio::test]
async fn a_drag_starts_from_the_shown_size_and_keeps_what_can_be_shown() {
    // Review point 2: the stored 520 is more than the window lets the dock have.
    let setup = setup();
    dock(&setup.core, PanelSide::Left, &Value::Null).expect("docks");
    on_room(&setup.core, room(PanelSide::Left, 400, 450));
    on_title(&setup.core, "rb-splitter:move:1:-20");
    assert_eq!(last_dock(&setup.ui).expect("docked").size, 380);
    on_title(&setup.core, "rb-splitter:move:2:100");
    assert_eq!(last_dock(&setup.ui).expect("docked").size, 450);
    on_title(&setup.core, "rb-splitter:end:3:30");
    assert_eq!(setup.core.user_settings().panels.dock_size.left, 430);
    // A room of another side is not this dock's.
    on_room(&setup.core, room(PanelSide::Bottom, 200, 300));
    on_title(&setup.core, "rb-splitter:move:4:0");
    assert_eq!(last_dock(&setup.ui).expect("docked").size, 430);
}

#[tokio::test]
async fn a_step_starts_from_the_shown_size_and_none_is_lost() {
    // Review point 3: Smaller and Larger step from what is shown, two quick ones both count.
    let setup = setup();
    dock(&setup.core, PanelSide::Right, &Value::Null).expect("docks");
    on_room(&setup.core, room(PanelSide::Right, 300, 600));
    let answer = dock(&setup.core, PanelSide::Right, &json!({ "step": 1 })).expect("larger");
    assert_eq!(
        answer,
        json!({ "position": "right", "size": 340, "collapsed": false })
    );
    let answer = dock(&setup.core, PanelSide::Right, &json!({ "step": 1 })).expect("larger");
    assert_eq!(answer["size"], 380);
    assert_eq!(setup.core.user_settings().panels.dock_size.right, 380);
    // Never beyond what the window can give, never under the minimum.
    on_room(&setup.core, room(PanelSide::Right, 380, 400));
    let answer = dock(&setup.core, PanelSide::Right, &json!({ "step": 1 })).expect("larger");
    assert_eq!(answer["size"], 400);
    on_room(&setup.core, room(PanelSide::Right, MIN_DOCK, 400));
    let answer = dock(&setup.core, PanelSide::Right, &json!({ "step": -1 })).expect("smaller");
    assert_eq!(answer["size"], MIN_DOCK);
    for bad in [
        json!({ "step": 2 }),
        json!({ "step": 0 }),
        json!({ "step": "1" }),
        json!({ "step": 1, "size": 300 }),
    ] {
        assert!(
            matches!(
                dock(&setup.core, PanelSide::Right, &bad),
                Err(CommandError::InvalidArgs(_))
            ),
            "{bad}"
        );
    }
}

#[tokio::test]
async fn a_step_on_a_closed_side_starts_from_its_kept_size() {
    let setup = setup();
    let answer = dock(&setup.core, PanelSide::Bottom, &json!({ "step": -1 })).expect("docks");
    assert_eq!(answer["size"], 280);
}

#[tokio::test]
async fn the_panel_chosen_in_the_page_goes_along_when_the_panels_move() {
    // Review point 4: undocking opens the panel the docked page shows now.
    let setup = setup();
    dock(
        &setup.core,
        PanelSide::Left,
        &json!({ "plugin": "hello-panel", "panel": "main" }),
    )
    .expect("docks");
    selected(&setup.core, "hello-panel", "log").expect("a panel id");
    undock(&setup.core, &Value::Null).expect("undocks");
    assert_eq!(windows(&setup.ui), [LOG.to_owned()]);
    for (plugin, panel) in [("Hello", "log"), ("hello-panel", "../x"), ("", "log")] {
        assert!(
            selected(&setup.core, plugin, panel).is_err(),
            "{plugin} {panel}"
        );
    }
}

#[tokio::test]
async fn an_open_window_docks_when_the_setting_names_a_side() {
    // Review point 9.
    let setup = setup();
    show(&setup.core, MAIN.to_owned());
    selected(&setup.core, "hello-panel", "log").expect("a panel id");
    let mut user = setup.core.user_settings();
    user.panels.dock = PanelDock::Bottom;
    setup.core.replace_user_settings(user);
    on_settings_changed(&setup.core);
    let docked = last_dock(&setup.ui).expect("docked");
    assert_eq!((docked.side, docked.url.as_str()), (PanelSide::Bottom, LOG));
}

/// RB-080-93: the window closed by hand brings the rail back (right, with no side before); a
/// changed side moves the rail.
#[tokio::test]
async fn a_window_closed_by_hand_comes_back_as_the_rail() {
    let setup = setup();
    show(&setup.core, MAIN.to_owned());
    on_window_closed(&setup.core);
    let rail = last_dock(&setup.ui).expect("the rail");
    assert_eq!((rail.side, rail.collapsed), (PanelSide::Right, true));
    let mut user = setup.core.user_settings();
    user.panels.dock = PanelDock::Left;
    setup.core.replace_user_settings(user);
    on_settings_changed(&setup.core);
    let rail = last_dock(&setup.ui).expect("the rail");
    assert_eq!((rail.side, rail.collapsed), (PanelSide::Left, true));
}
