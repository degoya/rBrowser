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
