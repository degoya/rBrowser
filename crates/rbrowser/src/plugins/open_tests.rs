use rb_plugin_host::PanelPlacement;

use super::*;

fn panel(plugin: &str, panel: &str) -> PanelInfo {
    PanelInfo {
        plugin: plugin.to_owned(),
        panel: panel.to_owned(),
        title: panel.to_owned(),
        placement: PanelPlacement::Sidebar,
        icon: None,
        url: format!("rbrowser://plugin/{plugin}/panel/index.html"),
    }
}

fn offered() -> Vec<PanelInfo> {
    vec![panel("console", "console"), panel("outline", "outline")]
}

#[test]
fn an_answer_opens_a_panel_of_its_own_plugin() {
    let (answer, wanted) = split_open(
        "console",
        json!({ "$open": { "panel": "console" }, "lines": 3 }),
        &offered(),
    );
    assert_eq!(answer, json!({ "lines": 3 }));
    assert_eq!(wanted.as_deref(), Some("console"));
    let (answer, wanted) = split_open(
        "console",
        json!({ "$open": { "panel": "console" } }),
        &offered(),
    );
    assert_eq!(answer, json!({}));
    assert_eq!(wanted.as_deref(), Some("console"));
}

#[test]
fn another_plugins_or_an_unknown_panel_is_ignored_and_stripped() {
    for asked in ["outline", "nope", "Console", "console/../x"] {
        let (answer, wanted) = split_open(
            "console",
            json!({ "$open": { "panel": asked }, "n": 1 }),
            &offered(),
        );
        assert_eq!(answer, json!({ "n": 1 }), "{asked}");
        assert_eq!(wanted, None, "{asked}");
    }
    // A plugin whose panel is not offered (hidden, or no feature plugin served) opens nothing.
    let (_, wanted) = split_open("hello-panel", json!({ "$open": { "panel": "main" } }), &[]);
    assert_eq!(wanted, None);
}

#[test]
fn a_wrong_shape_is_ignored_and_stripped() {
    for open in [
        json!("console"),
        json!(null),
        json!([]),
        json!({}),
        json!({ "panel": 5 }),
        json!({ "panel": ["console"] }),
    ] {
        let (answer, wanted) = split_open("console", json!({ "$open": open, "n": 1 }), &offered());
        assert_eq!(answer, json!({ "n": 1 }), "{open}");
        assert_eq!(wanted, None, "{open}");
    }
}

#[test]
fn an_answer_without_open_stays_as_it_is() {
    for answer in [
        json!({ "n": 1 }),
        json!("text"),
        json!(["$open"]),
        json!(null),
        json!({ "nested": { "$open": { "panel": "console" } } }),
    ] {
        let (kept, wanted) = split_open("console", answer.clone(), &offered());
        assert_eq!(kept, answer);
        assert_eq!(wanted, None);
    }
}

#[test]
fn the_select_event_names_plugin_and_panel() {
    assert_eq!(
        select_message("console", "console"),
        json!({ "select": { "plugin": "console", "panel": "console" } })
    );
}

/// RB-094-27: a tab asked for waits for the panel's next read, once, per plugin; a later ask
/// replaces it.
#[test]
fn an_asked_tab_is_taken_once_by_its_plugin() {
    ask_tab("tab-test-a", "variables");
    ask_tab("tab-test-b", "import");
    ask_tab("tab-test-a", "export");
    assert_eq!(take_tab("tab-test-a").as_deref(), Some("export"));
    assert_eq!(take_tab("tab-test-a"), None);
    assert_eq!(take_tab("tab-test-b").as_deref(), Some("import"));
    assert_eq!(take_tab("tab-test-c"), None);
}

/// The event an open panel reads its asked tab on: no tab in it, the panel asks for its state.
#[test]
fn the_tab_event_names_no_tab() {
    assert_eq!(
        tab_event("api-client"),
        json!({ "event": { "plugin": "api-client", "message": { "tab": true } } })
    );
}
