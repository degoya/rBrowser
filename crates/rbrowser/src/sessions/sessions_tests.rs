use std::sync::Arc;

use rb_config::ProjectConfig;
use rb_engine::ViewId;
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::nav::marker_url;
use crate::host::ui::recording::{RecordingUi, UiCall};

fn config() -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "startUrl": "http://localhost:3000/",
        "sessions": ["admin"],
        "breakpoints": [
            { "id": "desktop", "width": 1440, "height": 900 },
            { "id": "mobile", "width": 393, "height": 852, "session": "admin" }
        ]
    }))
    .expect("the test config parses")
}

struct Setup {
    ui: Arc<RecordingUi>,
    core: Arc<Core>,
}

fn setup() -> Setup {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(config(), None).expect("opens");
    for id in 1..=20 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    let core = Core::new(host, ui.clone(), Handle::current(), CoreSetup::default());
    core.shell_ready();
    Setup { ui, core }
}

fn shell_scripts(ui: &RecordingUi) -> Vec<String> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) => Some(script),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn without_a_session_the_shell_asks() {
    let setup = setup();
    for id in [ASSIGN, CLEAR] {
        assert_eq!(
            run(&setup.core, id, &Value::Null).expect("runs"),
            Value::Null
        );
        assert!(
            shell_scripts(&setup.ui)
                .iter()
                .any(|script| script.contains("uiCommand") && script.contains(id)),
            "{id} reached the shell"
        );
    }
    // The session bar sits in the shell strip: the shell takes the keyboard.
    assert_eq!(
        setup
            .ui
            .calls()
            .iter()
            .filter(|call| **call == UiCall::FocusShell)
            .count(),
        2
    );
    assert!(
        !setup
            .ui
            .calls()
            .iter()
            .any(|call| matches!(call, UiCall::ClearCookies(_)))
    );
}

#[tokio::test]
async fn assign_moves_the_active_view() {
    let setup = setup();
    setup
        .core
        .host()
        .set_active_view(ViewId(1))
        .expect("active");
    let answer = run(&setup.core, ASSIGN, &json!({ "session": "admin" })).expect("moves");
    assert_eq!(
        answer,
        json!({ "session": "admin", "moved": [{ "from": 1, "view": 3 }] })
    );
    let answer = run(
        &setup.core,
        ASSIGN,
        &json!({ "target": "all", "session": "default" }),
    )
    .expect("moves");
    assert_eq!(answer["moved"].as_array().map(Vec::len), Some(2));
}

#[tokio::test]
async fn clear_answers_with_the_session_views() {
    let setup = setup();
    let answer = run(&setup.core, CLEAR, &json!({ "session": "admin" })).expect("clears");
    assert_eq!(
        answer,
        json!({ "session": "admin", "views": [2], "origins": [] })
    );
    assert!(
        setup
            .ui
            .calls()
            .contains(&UiCall::ClearCookies("admin".to_owned()))
    );
}

#[tokio::test]
async fn bad_arguments_are_refused() {
    let setup = setup();
    for (id, args) in [
        (ASSIGN, json!({ "session": "ghost" })),
        (ASSIGN, json!({ "session": "../admin" })),
        (ASSIGN, json!({ "session": "admin", "extra": 1 })),
        (CLEAR, json!({ "session": "Admin" })),
        (CLEAR, json!({ "session": 7 })),
    ] {
        assert!(
            matches!(
                run(&setup.core, id, &args),
                Err(CommandError::InvalidArgs(_))
            ),
            "{id} {args}"
        );
    }
}

#[test]
fn the_profile_is_set_once() {
    set_profile(PathBuf::from("/first"));
    set_profile(PathBuf::from("/second"));
    assert_eq!(profile(), Some(Path::new("/first")));
}

#[test]
fn session_cookies_persist_only_when_the_user_said_so() {
    assert!(!persist_session_cookies(), "secure default");
    set_persist_session_cookies(true);
    assert!(persist_session_cookies());
    set_persist_session_cookies(false);
    assert!(!persist_session_cookies());
}

#[test]
fn the_shell_lists_default_first_and_colours_the_named_ones() {
    let infos = session_infos(&config());
    assert_eq!(
        serde_json::to_value(infos).expect("serializes"),
        json!([
            { "id": "default" },
            { "id": "admin", "color": css_color(auto_color("admin")) }
        ])
    );
}

#[test]
fn the_sessions_fixture_is_a_valid_project() {
    let text = include_str!("../../../../tests/fixtures/sessions-project/.rbrowser/config.json");
    let config: ProjectConfig = serde_json::from_str(text).expect("the fixture parses");
    assert!(rb_config::check(&config).is_empty());
    assert_eq!(known_sessions(&config), ["default", "admin"]);
    assert_eq!(badge(&config, "admin").map(|b| b.rgb), Some(0xe5_48_4d));
}
