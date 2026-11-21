use rb_commands::lint::lint_defaults;
use rb_commands::{Platform, builtin, builtin_reserved};
use rb_config::Environment;
use serde_json::json;

use super::*;
use crate::host::ui::recording::UiCall;
use crate::sync_hub::tests::{Setup, attach_all, setup};

/// Views 1 desktop, 2 tablet and 3 mobile, attached; the tablet's navigation decoupled
/// (`syncNavigation: false`), the environment `local` in force.
async fn decoupled_tablet() -> Setup {
    let setup = setup();
    let host = setup.core.host();
    let mut config = host.config();
    config.sync.navigation_settle_ms = 20;
    for breakpoint in &mut config.breakpoints {
        if breakpoint.id == "tablet" {
            breakpoint.sync_navigation = false;
        }
    }
    config.environments = vec![Environment {
        id: "local".into(),
        url: "http://localhost:5173".into(),
        color: None,
        reload_interval_sec: None,
        asset_origins: Vec::new(),
    }];
    host.open_config(config, None).expect("reopens");
    host.update_shell_state(|shell| shell.environment = Some("local".to_owned()));
    crate::sync_hub::on_start(&setup.core);
    crate::nav_sync::on_start(&setup.core);
    attach_all(&setup).await;
    setup
}

/// Every view a navigation went to, with its URL.
/// The loads after the views' first: every view opens `about:blank` when it attaches.
fn navigations(setup: &Setup) -> Vec<(ViewId, String)> {
    setup
        .ui
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Navigate(view, url) if url != "about:blank" => Some((view, url)),
            _ => None,
        })
        .collect()
}

fn urls(setup: &Setup) -> Vec<String> {
    setup
        .core
        .host()
        .views()
        .into_iter()
        .map(|view| view.url)
        .collect()
}

/// The status bar's events about the field's refusals.
fn refusals(setup: &Setup) -> Vec<String> {
    setup
        .ui
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::StatusEvent(script) if script.contains("cells.address.refused.") => {
                Some(script)
            }
            _ => None,
        })
        .collect()
}

// RB-092-03 acceptance: the address field of a decoupled view loads what was typed in that view
// only; the other views keep their pages.
#[tokio::test(flavor = "multi_thread")]
async fn the_field_loads_what_was_typed_in_that_view_only() {
    let setup = decoupled_tablet().await;
    let core = &setup.core;
    let before = urls(&setup);
    assert_eq!(
        run(core, &json!({ "view": 2, "url": "/pricing?plan=pro" })),
        Ok(json!({ "view": 2, "url": "http://localhost:5173/pricing?plan=pro" }))
    );
    assert_eq!(
        run(core, &json!({ "view": 2, "url": "shop.test/cart" })),
        Ok(json!({ "view": 2, "url": "http://shop.test/cart" }))
    );
    assert_eq!(
        navigations(&setup),
        vec![
            (
                ViewId(2),
                "http://localhost:5173/pricing?plan=pro".to_owned()
            ),
            (ViewId(2), "http://shop.test/cart".to_owned()),
        ]
    );
    let after = urls(&setup);
    assert_eq!((&after[0], &after[2]), (&before[0], &before[2]));
    assert!(
        crate::nav_sync::status(core.host())
            .decoupled
            .contains(&ViewId(2)),
        "the view keeps its own navigation"
    );
}

// RB-092-03 acceptance: script, data and browser pages are refused with the reason in the status
// bar, and the view loads nothing.
#[tokio::test(flavor = "multi_thread")]
async fn script_data_and_browser_addresses_are_refused_with_the_reason() {
    let setup = decoupled_tablet().await;
    let core = &setup.core;
    for (typed, scheme) in [
        ("javascript:alert(1)", "javascript"),
        ("data:text/html,<b>hi</b>", "data"),
        ("chrome://settings", "chrome"),
    ] {
        let Err(CommandError::InvalidArgs(message)) =
            run(core, &json!({ "view": 2, "url": typed }))
        else {
            panic!("{typed} is refused");
        };
        assert!(message.contains(&format!("{scheme}:")), "{message}");
    }
    assert_eq!(navigations(&setup), Vec::new());
    let events = refusals(&setup);
    assert_eq!(events.len(), 3, "{events:?}");
    assert!(
        events
            .iter()
            .all(|event| event.contains("cells.address.refused.scheme")),
        "{events:?}"
    );
    assert!(events[0].contains("javascript"), "{}", events[0]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_coupled_view_gets_no_field_and_loads_nothing() {
    let setup = decoupled_tablet().await;
    let core = &setup.core;
    assert!(matches!(
        run(core, &json!({ "view": 1 })),
        Err(CommandError::Failed(_))
    ));
    assert!(matches!(
        run(core, &json!({ "view": 1, "url": "localhost:5173/a" })),
        Err(CommandError::InvalidArgs(_))
    ));
    assert_eq!(navigations(&setup), Vec::new());
    assert!(
        !setup
            .ui
            .calls()
            .iter()
            .any(|call| matches!(call, UiCall::EditAddress(..)))
    );
    for bad in [
        json!({ "view": 9 }),
        json!({ "view": "two" }),
        json!({ "view": 2, "url": 7 }),
    ] {
        assert!(
            matches!(run(core, &bad), Err(CommandError::InvalidArgs(_))),
            "{bad}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_command_opens_the_field_of_the_active_view_in_its_strip() {
    let setup = decoupled_tablet().await;
    let core = &setup.core;
    core.host().set_active_view(ViewId(2)).expect("a view");
    assert_eq!(
        run(core, &Value::Null),
        Ok(json!({ "view": 2, "open": true }))
    );
    let opened: Vec<(ViewId, String)> = setup
        .ui
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::EditAddress(view, url, name) => {
                assert!(!name.is_empty(), "the field's accessible name");
                Some((view, url))
            }
            _ => None,
        })
        .collect();
    // Nothing loaded yet: the field starts empty, never with the marker URL.
    assert_eq!(opened, vec![(ViewId(2), String::new())]);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_setting_switches_the_field_off() {
    let setup = decoupled_tablet().await;
    let core = &setup.core;
    let mut user = core.user_settings();
    user.ui.cell_label.address_editable = false;
    core.replace_user_settings(user);
    assert!(matches!(
        run(core, &json!({ "view": 2 })),
        Err(CommandError::Failed(_))
    ));
    assert!(matches!(
        run(core, &json!({ "view": 2, "url": "localhost:5173/a" })),
        Err(CommandError::InvalidArgs(_))
    ));
    assert_eq!(navigations(&setup), Vec::new());
}

// The canvas board draws its labels in a page of its own: the field opens there (`uiCommand`).
#[tokio::test(flavor = "multi_thread")]
async fn on_the_canvas_the_board_page_opens_the_field() {
    use crate::layout::test_support::{config, setup, view_ids};
    let setup = setup(config());
    let core = &setup.core;
    crate::presets::run_command(core, "layout.select_preset", &json!({ "id": "board" }))
        .expect("a presets command")
        .expect("selects");
    crate::nav_sync::on_start(core);
    let view = view_ids(&setup)[0];
    crate::nav_sync::couple(core.host(), view, false).expect("decouples");
    assert_eq!(
        run(core, &json!({ "view": view.0 })),
        Ok(json!({ "view": view.0, "open": true }))
    );
    let calls = setup.ui.calls();
    let sent = calls.iter().any(|call| {
        matches!(call, UiCall::Shell(script)
            if script.contains("uiCommand") && script.contains(EDIT_ADDRESS))
    });
    assert!(sent, "{calls:?}");
    assert!(
        !calls
            .iter()
            .any(|call| matches!(call, UiCall::EditAddress(..)))
    );
}

#[test]
fn the_command_has_its_chord_group_title_and_tool() {
    let defs = definitions().expect("definitions");
    let [def] = defs.as_slice() else {
        panic!("one command: {defs:?}");
    };
    assert_eq!(def.id.as_str(), "view.edit_address");
    assert_eq!(def.title_key, "commands.view.edit_address");
    assert_eq!(def.group, Group::Sync);
    let bindings: Vec<String> = def
        .default_bindings
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(bindings, ["Ctrl+Shift+Space S L"]);
    // MCP loads a view's page with `navigate { target: viewId }`.
    assert_eq!(def.mcp, McpCoverage::Covered("navigate".to_owned()));
}

#[test]
fn the_chord_passes_the_lint_against_the_reserved_list_and_the_other_commands() {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.extend(crate::sessions::defs::definitions().expect("sessions"));
    defs.extend(crate::bookmarks::definitions().expect("bookmarks"));
    defs.extend(crate::sidecar::definitions().expect("sidecar"));
    defs.extend(crate::breakpoint_keys::definitions().expect("breakpoint keys"));
    defs.extend(crate::restore_last::defs::definitions().expect("last pages"));
    defs.push(rb_diag::command_def().expect("diagnostics.create"));
    defs.extend(rb_plugin_host::core_command_defs().expect("plugins.*"));
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}
