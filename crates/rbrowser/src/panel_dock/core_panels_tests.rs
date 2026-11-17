use rb_config::{PanelDock, UserSettings};
use serde_json::{Value, json};

use super::*;
use crate::host::ui::recording::UiCall;
use crate::page_window::PageWindow;
use crate::panel_dock::tests::{docks, setup, setup_with};
use crate::panel_dock::{PanelSide, undock};

const FINDINGS_URL: &str = "rbrowser://ui/settings.html#/panels/findings";

#[test]
fn a_core_panel_is_a_key_without_a_dot_and_only_a_known_one() {
    assert_eq!(url(FINDINGS).as_deref(), Some(FINDINGS_URL));
    assert_eq!(
        url(VIDEOS).as_deref(),
        Some("rbrowser://ui/settings.html#/panels/videos")
    );
    // RB-090-60.
    assert_eq!(
        url(RECORDER).as_deref(),
        Some("rbrowser://ui/settings.html#/panels/recorder")
    );
    assert_eq!(
        url(FLOWS).as_deref(),
        Some("rbrowser://ui/settings.html#/panels/flows")
    );
    for other in ["", "sitemap", "hello-panel.main", "../findings", "Findings"] {
        assert_eq!(url(other), None, "{other}");
    }
    assert_eq!(
        select_message(FINDINGS),
        json!({ "select": { "core": "findings" } })
    );
}

#[tokio::test]
async fn findings_open_in_the_window_or_the_dock_and_are_selected() {
    let setup = setup();
    open(&setup.core, FINDINGS);
    assert!(setup.ui.calls().contains(&UiCall::PageWindow(
        PageWindow::Panels,
        FINDINGS_URL.to_owned()
    )));
    let mut user = UserSettings::default();
    user.panels.dock = PanelDock::Bottom;
    user.panels.dock_state = rb_config::PanelDockState::Rail;
    let docked = setup_with(user);
    open(&docked.core, FINDINGS);
    let dock = docks(&docked.ui).last().cloned().flatten().expect("docked");
    // A check's findings keep the fold (review point 2); the page marks the entry.
    assert_eq!(
        (dock.side, dock.url.as_str(), dock.collapsed),
        (PanelSide::Bottom, FINDINGS_URL, true)
    );
}

#[tokio::test]
async fn the_panels_move_with_findings_when_the_page_shows_them() {
    let setup = setup();
    crate::panel_dock::dock(&setup.core, PanelSide::Left, &json!({})).expect("docks");
    selected(&setup.core, FINDINGS).expect("a core panel");
    assert!(selected(&setup.core, "sitemap").is_err());
    undock(&setup.core, &Value::Null).expect("undocks");
    assert!(setup.ui.calls().contains(&UiCall::PageWindow(
        PageWindow::Panels,
        FINDINGS_URL.to_owned()
    )));
}

/// RB-090-71 item 2: the Videos command is a person's (its menu rows, `Leader C T`, the palette;
/// no MCP tool runs it): the dock folded to its rail - the default - or hidden opens at the
/// Videos panel, where a check's open keeps the fold.
#[tokio::test]
async fn a_person_s_open_unfolds_the_dock_at_the_panel() {
    const VIDEOS_URL: &str = "rbrowser://ui/settings.html#/panels/videos";
    for state in [
        rb_config::PanelDockState::Rail,
        rb_config::PanelDockState::Hidden,
        rb_config::PanelDockState::Open,
    ] {
        let mut user = UserSettings::default();
        user.panels.dock = PanelDock::Right;
        user.panels.dock_state = state;
        let docked = setup_with(user);
        open_unfolded(&docked.core, VIDEOS);
        let dock = docks(&docked.ui).last().cloned().flatten().expect("docked");
        assert_eq!(
            (dock.side, dock.url.as_str(), dock.collapsed),
            (PanelSide::Right, VIDEOS_URL, false),
            "{state:?}"
        );
        assert_eq!(
            docked.core.user_settings().panels.dock_state,
            rb_config::PanelDockState::Open,
            "{state:?}"
        );
        assert!(
            docked.ui.calls().iter().any(|call| matches!(
                call,
                UiCall::Shell(script) if script.contains(r#""select":{"core":"videos"}"#)
            )),
            "an open page selects the panel ({state:?})"
        );
    }
    // In their own window the panels open as before.
    let floating = setup();
    open_unfolded(&floating.core, VIDEOS);
    assert!(floating.ui.calls().contains(&UiCall::PageWindow(
        PageWindow::Panels,
        VIDEOS_URL.to_owned()
    )));
    // A key that is no core panel opens nothing.
    let none = setup();
    let before = none.ui.calls().len();
    open_unfolded(&none.core, "sitemap");
    assert_eq!(none.ui.calls().len(), before);
}

/// RB-090-71 item 2, every opener of a core panel: the commands a person runs from a menu, the
/// palette or a shortcut open a dock folded to its rail at their panel; only a check's open (the
/// test above) keeps the fold. No MCP tool runs these commands.
#[tokio::test]
async fn every_core_panel_command_opens_the_folded_dock_at_its_panel() {
    for (command, key) in [
        ("panels.findings", FINDINGS),
        (crate::recorder::PANEL, RECORDER),
        (crate::flows::PANEL, FLOWS),
        (crate::video::defs::PANEL, VIDEOS),
    ] {
        let mut user = UserSettings::default();
        user.panels.dock = PanelDock::Left;
        user.panels.dock_state = rb_config::PanelDockState::Rail;
        let docked = setup_with(user);
        docked
            .core
            .run_command(command, Value::Null)
            .expect(command);
        let dock = docks(&docked.ui).last().cloned().flatten().expect(command);
        assert_eq!(
            (dock.side, dock.url, dock.collapsed),
            (PanelSide::Left, url(key).expect("a core panel"), false),
            "{command}"
        );
        assert_eq!(
            docked.core.user_settings().panels.dock_state,
            rb_config::PanelDockState::Open,
            "{command}"
        );
    }
}
