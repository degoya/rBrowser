use std::sync::Arc;

use rb_engine::ViewHost as _;
use rb_ipc::{CoreToShell, SettingIssue};
use rb_mcp::{Backend as _, ConfigScope, LogStore, SettingScope};
use serde_json::{Value, json};

use super::{CoreBackend, masked, rejection};
use crate::mcp::tests::{core, core_with_project};

#[test]
fn a_protected_rejection_says_so_and_points_at_the_key() {
    let answer = CoreToShell::SettingRejected {
        key: "mcp.allowEvaluate".to_owned(),
        issues: vec![SettingIssue {
            path: String::new(),
            message_key: "settings.rejected.protected".to_owned(),
        }],
    };
    let failure = rejection("mcp.allowEvaluate", &answer).expect("a failure");
    assert_eq!(failure.code, "settings.rejected.protected");
    assert_eq!(failure.settings_key.as_deref(), Some("mcp.allowEvaluate"));
    assert!(failure.message.contains("protected"));
    assert!(rejection("x", &CoreToShell::Ack).is_none());
}

#[test]
fn masked_hides_header_values_and_credentials() {
    let config = json!({
        "emulation": { "headers": { "Authorization": "Bearer abcdefgh12345678" } },
        "startUrl": "https://user:secretpass@example.test/",
    });
    let text = masked(&config).to_string();
    assert!(!text.contains("abcdefgh12345678"), "{text}");
    assert!(!text.contains("secretpass"), "{text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_cannot_write_a_protected_setting() {
    let core = core();
    let backend = CoreBackend::new(&core, Arc::new(LogStore::default()));
    for key in [
        "mcp.enabled",
        "mcp.allowEvaluate",
        "engines.chromium.sandbox",
    ] {
        let failure = backend
            .set_setting(key, &Value::Bool(true), SettingScope::User)
            .expect_err("protected");
        assert_eq!(failure.code, "settings.rejected.protected", "{key}");
    }
    assert!(!core.user_settings().mcp.enabled);
    assert!(!core.user_settings().mcp.allow_evaluate);
}

/// The extension switches load code into every view: an agent may not switch them back on
/// after the person switched them off - nor off, the whole key is the person's (security
/// review RB-090-06, finding 2; RB-090-21).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_cannot_switch_chrome_extensions_or_the_web_store() {
    let core = core();
    let backend = CoreBackend::new(&core, Arc::new(LogStore::default()));
    for key in [
        "engines.chromium.extensions.enabled",
        "engines.chromium.extensions.allowWebStore",
    ] {
        for value in [true, false] {
            let failure = backend
                .set_setting(key, &Value::Bool(value), SettingScope::User)
                .expect_err("protected");
            assert_eq!(failure.code, "settings.rejected.protected", "{key}");
        }
    }
    let settings = core.user_settings();
    assert!(settings.engines.chromium.extensions.enabled);
    assert!(settings.engines.chromium.extensions.allow_web_store);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_writes_an_ordinary_setting_and_reads_it_back() {
    let core = core();
    let backend = CoreBackend::new(&core, Arc::new(LogStore::default()));
    backend
        .set_setting("grid.fitToWindow", &Value::Bool(false), SettingScope::User)
        .expect("written");
    assert!(!core.user_settings().grid.fit_to_window);
    let settings = backend.settings();
    let entry = rb_mcp::settings_view::entry(&settings, "grid.fitToWindow").expect("listed");
    assert_eq!(entry["value"], false);
    assert_eq!(entry["origin"], "user");
    assert_eq!(entry["category"], "layout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_views_and_the_project_come_from_the_host() {
    let core = core();
    let backend = CoreBackend::new(&core, Arc::new(LogStore::default()));
    assert!(!backend.views().is_empty());
    let info = backend.project_info();
    assert_eq!(info["root"], Value::Null);
    assert_eq!(info["trusted"], false);
    assert_eq!(backend.options().navigation_timeout.as_millis(), 15_000);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_config_write_is_checked_before_it_lands() {
    let dir = tempfile::tempdir().expect("temp dir");
    let core = core_with_project(dir.path());
    let backend = CoreBackend::new(&core, Arc::new(LogStore::default()));
    let mut document = backend
        .config_document(ConfigScope::Project)
        .expect("the project file");
    document["layout"] = json!({ "gap": 6 });
    backend
        .write_config_document(ConfigScope::Project, &document)
        .expect("a valid config is written");
    let written =
        rb_config::read_json(&rb_config::ConfigFile::Project.path(dir.path())).expect("on disk");
    assert_eq!(written["layout"]["gap"], 6);
    let mut twice = written.clone();
    let first = twice["breakpoints"][0].clone();
    twice["breakpoints"]
        .as_array_mut()
        .expect("a list")
        .push(first);
    let refused = backend
        .write_config_document(ConfigScope::Project, &twice)
        .expect_err("a duplicate id");
    assert!(
        refused.message.contains("/breakpoints"),
        "{}",
        refused.message
    );
    let still =
        rb_config::read_json(&rb_config::ConfigFile::Project.path(dir.path())).expect("on disk");
    assert_eq!(still, written);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_a_project_there_is_no_config_to_write() {
    let core = core();
    let backend = CoreBackend::new(&core, Arc::new(LogStore::default()));
    let failure = backend
        .config_document(ConfigScope::Project)
        .expect_err("no project");
    assert_eq!(failure.code, "settings.rejected.no_project");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cdp_to_a_view_without_a_session_fails_cleanly() {
    let core = core();
    let backend = CoreBackend::new(&core, Arc::new(LogStore::default()));
    let view = core.host().views()[0].id;
    let failure = backend
        .cdp(view, "Runtime.evaluate", json!({ "expression": "1" }))
        .await
        .expect_err("not attached");
    assert_eq!(failure.code, "cdp.failed");
    assert_eq!(backend.options().max_answer_bytes, 262_144);
    let masked = backend.mask(&json!({ "url": "https://u:topsecretpw@a.test/" }));
    assert!(!masked.to_string().contains("topsecretpw"));
}

/// RB-090-71: adding a portal asset origin writes the project config - a person's click in the
/// status bar; an MCP client is refused before the command runs, and nothing is written.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_cannot_add_a_portal_asset_origin() {
    let dir = tempfile::tempdir().expect("temp dir");
    let core = core_with_project(dir.path());
    let backend = CoreBackend::new(&core, Arc::new(LogStore::default()));
    let path = rb_config::ConfigFile::Project.path(dir.path());
    let before = std::fs::read_to_string(&path).expect("the project file");
    let refused = backend
        .run_command(
            crate::portal::offer::ADD_ASSET_ORIGIN,
            json!({ "environment": "local", "origin": "http://localhost:5173" }),
        )
        .expect_err("a person's decision");
    assert!(refused.to_string().contains("MCP cannot"), "{refused}");
    assert_eq!(std::fs::read_to_string(&path).expect("on disk"), before);
}

/// RB-099-25: the backend runs a tool's command marked as MCP's. A person's check run opens the
/// dock folded to its rail (the default at start) at the findings; `checks_run` - the same
/// command from a tool - answers with them and keeps the fold.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_mcp_check_run_keeps_the_fold_and_a_persons_opens_it() {
    use rb_config::PanelDockState;
    let core = core();
    assert_eq!(core.user_settings().panels.dock_state, PanelDockState::Rail);
    let backend = CoreBackend::new(&core, Arc::new(LogStore::default()));
    backend
        .run_command(crate::checks::RUN_ALL_COMMAND, Value::Null)
        .expect("runs without any check plugin");
    assert_eq!(core.user_settings().panels.dock_state, PanelDockState::Rail);
    assert!(
        !crate::mcp::origin::is_mcp(),
        "the mark ended with the call"
    );
    core.run_command(crate::checks::RUN_ALL_COMMAND, Value::Null)
        .expect("runs without any check plugin");
    assert_eq!(core.user_settings().panels.dock_state, PanelDockState::Open);
}
