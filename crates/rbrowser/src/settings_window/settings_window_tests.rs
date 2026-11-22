use std::path::Path;
use std::sync::Arc;

use rb_ipc::{PathKind, Scope};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

const CONFIG: &str = r#"{
  "version": 1,
  "startUrl": "http://localhost:3000",
  "sync": {
    "scroll": true
  }
}
"#;

fn core_in(dir: &Path) -> (Arc<RecordingUi>, Arc<Core>) {
    let root = dir.join("site");
    std::fs::create_dir_all(root.join(".rbrowser")).expect("folder");
    std::fs::write(root.join(".rbrowser/config.json"), CONFIG).expect("config");
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), Some(root))
        .expect("opens");
    let setup = CoreSetup {
        user_path: Some(dir.join("user/settings.json")),
        ..CoreSetup::default()
    };
    let core = Core::new(host, ui.clone(), Handle::current(), setup);
    (ui, core)
}

fn sent(ui: &RecordingUi) -> Vec<Value> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) => {
                let start = script.find("detail:")? + "detail:".len();
                let end = script.rfind("}));")?;
                serde_json::from_str(&script[start..end]).ok()
            }
            _ => None,
        })
        .collect()
}

fn windows(ui: &RecordingUi) -> Vec<String> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::SettingsWindow(url) => Some(url),
            _ => None,
        })
        .collect()
}

fn answer(core: &Core, message: ShellToCore) -> CoreToShell {
    ipc(core, &message)
        .expect("this module's message")
        .expect("answered")
}

fn set(core: &Core, key: &str, value: Value, scope: Scope) -> CoreToShell {
    answer(
        core,
        ShellToCore::SetSetting {
            key: key.to_owned(),
            value,
            scope,
        },
    )
}

#[test]
fn a_deep_link_routes_to_a_known_key_or_area_only() {
    let (registry, _) = crate::control::settings::settings_registry();
    assert_eq!(
        route(&registry, Some("sync.scroll")),
        "settings/sync.scroll"
    );
    assert_eq!(route(&registry, Some("shortcuts")), "settings/shortcuts");
    assert_eq!(
        route(&registry, Some("sitemap")),
        "settings/capture?section=sitemap"
    );
    // RB-094-26: a link that opens nothing names its key, never silently another page.
    assert_eq!(
        route(&registry, Some("no.such")),
        "settings?missing=no.such"
    );
    assert_eq!(route(&registry, Some("\"><script>")), "settings?missing=");
    assert_eq!(route(&registry, None), "settings");
}

/// The API client's settings schema as the plugin load hands it over
/// (`crate::analyzer::plugins` -> `crate::plugins::services`).
fn api_client_schema() -> (String, Value) {
    let schema = serde_json::from_str(include_str!(
        "../../../../plugins/api-client/settings.schema.json"
    ))
    .expect("JSON");
    ("api-client".to_owned(), schema)
}

/// RB-094-26, the owner's finding: "Open the setting" of the API client's benchmark hint opened
/// General > Start behaviour. The plugin load handed its schema to the services only, and the
/// window's registry held the core's keys: the link resolved to nothing (`#/settings`), the
/// window's schema had no plugin key and a write of one was `unknown_key`. Red before the fix.
#[tokio::test]
async fn a_plugin_key_opens_its_field_shows_in_the_schema_and_takes_a_write() {
    let dir = tempfile::tempdir().expect("temp");
    let (ui, core) = core_in(dir.path());
    crate::plugins::services::shared().set_schemas(vec![api_client_schema()]);
    open(&core, Some("api-client.benchRepeats"));
    assert_eq!(
        windows(&ui).last().map(String::as_str),
        Some("rbrowser://ui/settings.html#/settings/api-client.benchRepeats")
    );
    let CoreToShell::Settings { schema, values } = answer(
        &core,
        ShellToCore::GetSettings {
            include_schema: true,
        },
    ) else {
        panic!("settings");
    };
    let schema = schema.expect("asked for");
    let def = schema["defs"]
        .as_array()
        .and_then(|defs| {
            defs.iter()
                .find(|def| def["key"] == "api-client.benchRepeats")
        })
        .expect("the plugin's key is in the window's schema");
    assert_eq!(def["plugin"], "api-client");
    assert_eq!(def["category"], "network");
    assert_eq!(def["section"], "api-client");
    assert!(
        values
            .iter()
            .any(|value| value.key == "api-client.benchRepeats")
    );
    // The window writes the plugin's keys: a user key, and the local-only allow list.
    let wrote = set(&core, "api-client.benchRepeats", json!(20), Scope::User);
    assert!(matches!(wrote, CoreToShell::Ack), "{wrote:?}");
    let wrote = set(
        &core,
        "api-client.allowedHosts",
        json!(["api.example.test"]),
        Scope::Local,
    );
    assert!(matches!(wrote, CoreToShell::Ack), "{wrote:?}");
}

/// RB-094-26: an unknown key opens the settings with the key named, never their start
/// silently - `missing` for a key no plugin names, even while no plugin load has finished (as in
/// this test process): `pending` is for an installed plugin's key only.
#[tokio::test]
async fn an_unknown_key_opens_the_settings_with_the_key_named() {
    let dir = tempfile::tempdir().expect("temp");
    let (ui, core) = core_in(dir.path());
    open(&core, Some("no.such.key"));
    assert_eq!(
        windows(&ui).last().map(String::as_str),
        Some("rbrowser://ui/settings.html#/settings?missing=no.such.key")
    );
}

#[tokio::test]
async fn settings_open_and_open_settings_open_the_pages_window_at_the_key() {
    let dir = tempfile::tempdir().expect("temp");
    let (ui, core) = core_in(dir.path());
    let ran = run_command(&core, OPEN_COMMAND, &json!({ "key": "sync.scroll" }));
    assert!(matches!(ran, Some(Ok(Value::Null))));
    assert!(matches!(
        answer(&core, ShellToCore::OpenSettings { key: None }),
        CoreToShell::Ack
    ));
    assert!(run_command(&core, SHORTCUTS_EDIT_COMMAND, &Value::Null).is_some());
    assert!(run_command(&core, OPEN_PLUGINS_COMMAND, &Value::Null).is_some());
    assert!(run_command(&core, "view.back", &Value::Null).is_none());
    assert_eq!(
        windows(&ui),
        [
            "rbrowser://ui/settings.html#/settings/sync.scroll",
            "rbrowser://ui/settings.html#/settings",
            "rbrowser://ui/settings.html#/settings/shortcuts.bindings",
            "rbrowser://ui/settings.html#/settings/plugins",
        ]
    );
}

#[tokio::test]
async fn get_settings_brings_the_values_and_once_asked_the_schema() {
    let dir = tempfile::tempdir().expect("temp");
    let (_ui, core) = core_in(dir.path());
    let CoreToShell::Settings { values, schema } = answer(
        &core,
        ShellToCore::GetSettings {
            include_schema: true,
        },
    ) else {
        panic!("settings");
    };
    let schema = schema.expect("asked for");
    assert_eq!(schema["defs"].as_array().map(Vec::len), Some(values.len()));
    let scroll = values
        .iter()
        .find(|value| value.key == "sync.scroll")
        .expect("listed");
    assert_eq!(scroll.origin, rb_settings::Origin::Project);
    let CoreToShell::Settings { schema, .. } = answer(
        &core,
        ShellToCore::GetSettings {
            include_schema: false,
        },
    ) else {
        panic!("settings");
    };
    assert!(schema.is_none());
}

#[tokio::test]
async fn the_effective_project_values_follow_the_shells_trust() {
    let dir = tempfile::tempdir().expect("temp");
    let (_ui, core) = core_in(dir.path());
    std::fs::write(
        dir.path().join("site/.rbrowser/config.local.json"),
        r#"{ "network": { "proxy": "http://127.0.0.1:8888" } }"#,
    )
    .expect("local");
    let proxy = |core: &Core| {
        apply::documents(core)
            .project
            .expect("a project")
            .effective
            .pointer("/network/proxy")
            .cloned()
    };
    assert_eq!(
        proxy(&core),
        None,
        "untrusted: the gated key does not apply"
    );
    // The shell's trust is the one in effect; the settings window does not check it again.
    core.host().update_shell_state(|shell| shell.trusted = true);
    assert_eq!(proxy(&core), Some(json!("http://127.0.0.1:8888")));
}

#[tokio::test]
async fn a_project_write_changes_one_line_and_tells_every_page() {
    let dir = tempfile::tempdir().expect("temp");
    let (ui, core) = core_in(dir.path());
    let file = dir.path().join("site/.rbrowser/config.json");
    assert!(matches!(
        set(&core, "sync.scroll", json!(false), Scope::Project),
        CoreToShell::Ack
    ));
    let after = std::fs::read_to_string(&file).expect("written");
    let changed: Vec<(&str, &str)> = CONFIG
        .lines()
        .zip(after.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(changed, [("    \"scroll\": true", "    \"scroll\": false")]);
    assert_eq!(CONFIG.lines().count(), after.lines().count());
    assert!(
        sent(&ui)
            .iter()
            .any(|m| m["type"] == "settingsChanged" && m["keys"] == json!(["sync.scroll"]))
    );
}

/// RB-090-36: a user setting's `settingsChanged` carries its new value, so no page asks
/// `getSettings` again; a project write (it may take other keys along, RB-090-61) and a reload
/// of the project's config carry none - the pages read them again, off the UI thread.
#[tokio::test]
async fn settings_changed_carries_the_new_value_of_a_user_setting() {
    let dir = tempfile::tempdir().expect("temp");
    let (ui, core) = core_in(dir.path());
    let last_change = || {
        sent(&ui)
            .into_iter()
            .rfind(|m| m["type"] == "settingsChanged")
            .expect("sent")
    };
    set(&core, "ui.theme", json!("dark"), Scope::User);
    assert_eq!(
        last_change()["values"],
        json!([{ "key": "ui.theme", "value": "dark", "origin": "user" }])
    );
    set(&core, "sync.scroll", json!(false), Scope::Project);
    assert_eq!(last_change()["keys"], json!(["sync.scroll"]));
    assert!(last_change().get("values").is_none());
    on_config_changed(&core);
    assert!(last_change().get("values").is_none());
}

#[tokio::test]
async fn a_local_value_is_written_to_config_local_json_and_reset_there() {
    let dir = tempfile::tempdir().expect("temp");
    let (_ui, core) = core_in(dir.path());
    let local = dir.path().join("site/.rbrowser/config.local.json");
    set(&core, "sync.click", json!(false), Scope::Local);
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(&local).expect("local")).expect("json");
    assert_eq!(written, json!({ "sync": { "click": false } }));
    let CoreToShell::Settings { values, .. } = apply::settings_answer(&core, false) else {
        panic!("settings");
    };
    let click = values
        .iter()
        .find(|v| v.key == "sync.click")
        .expect("listed");
    assert_eq!(click.origin, rb_settings::Origin::Local);
    assert_eq!(click.value, json!(false));
    set(&core, "sync.click", Value::Null, Scope::Local);
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(&local).expect("local")).expect("json");
    assert_eq!(written, json!({}));
}

#[tokio::test]
async fn a_refused_write_names_its_reason_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("temp");
    let (_ui, core) = core_in(dir.path());
    let CoreToShell::SettingRejected { key, issues } = set(
        &core,
        "network.ignoreCertificateErrors",
        json!(true),
        Scope::Project,
    ) else {
        panic!("refused");
    };
    assert_eq!(key, "network.ignoreCertificateErrors");
    assert_eq!(issues[0].message_key, write::reason::LOCAL_ONLY);
    let file = dir.path().join("site/.rbrowser/config.json");
    assert_eq!(std::fs::read_to_string(file).expect("kept"), CONFIG);
}

#[tokio::test]
async fn a_user_write_applies_at_once_and_reaches_the_file() {
    let dir = tempfile::tempdir().expect("temp");
    let (_ui, core) = core_in(dir.path());
    set(&core, "shortcuts.leaderTimeoutMs", json!(2000), Scope::User);
    assert_eq!(core.user_settings().shortcuts.leader_timeout_ms, 2000);
    set(&core, "logs.level", json!("debug"), Scope::User);
    let file: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("user/settings.json")).expect("written"),
    )
    .expect("json");
    assert_eq!(
        file,
        json!({ "shortcuts": { "leaderTimeoutMs": 2000 }, "logs": { "level": "debug" } })
    );
}

/// RB-080-27: the shell's query is answered off the thread it came in on, after the write.
#[tokio::test(flavor = "multi_thread")]
async fn the_shell_s_set_setting_writes_off_the_calling_thread() {
    let dir = tempfile::tempdir().expect("temp");
    let (_ui, core) = core_in(dir.path());
    let caller = std::thread::current().id();
    let (tx, rx) = std::sync::mpsc::channel();
    crate::ipc::dispatch_to(
        &core,
        r#"{"type":"setSetting","key":"ui.theme","value":"dark","scope":"user"}"#,
        move |answer| {
            let _ = tx.send((answer, std::thread::current().id()));
        },
    );
    let (answer, thread) = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("answered");
    assert!(answer.expect("ack").contains("\"type\":\"ack\""));
    assert_ne!(thread, caller);
    let file: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("user/settings.json")).expect("written"),
    )
    .expect("json");
    assert_eq!(file, json!({ "ui": { "theme": "dark" } }));
    assert_eq!(core.user_settings().ui.theme, rb_config::Theme::Dark);
}

/// RB-080-27 (review): `set_setting` is one change at a time, whoever calls it on whatever
/// thread - two writers each read, plan and write the same file and would drop each other's
/// key without it.
#[tokio::test(flavor = "multi_thread")]
async fn two_writers_on_two_threads_keep_each_others_key() {
    const ROUNDS: u32 = 40;
    let dir = tempfile::tempdir().expect("temp");
    let (_ui, core) = core_in(dir.path());
    let writer = |key: &'static str, value: fn(u32) -> Value| {
        let core = core.clone();
        tokio::task::spawn_blocking(move || {
            for round in 0..ROUNDS {
                let answer = set_setting(&core, key, &value(round), Scope::User, Caller::Shell);
                assert!(matches!(answer, CoreToShell::Ack), "{answer:?}");
            }
        })
    };
    let theme = writer("ui.theme", |round| {
        json!(if round % 2 == 0 { "dark" } else { "light" })
    });
    let count = writer("palette.recentCount", |round| json!(round + 1));
    theme.await.expect("the theme writer");
    count.await.expect("the count writer");
    let file: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("user/settings.json")).expect("written"),
    )
    .expect("json");
    assert_eq!(file["ui"]["theme"], json!("light"), "{file}");
    assert_eq!(file["palette"]["recentCount"], json!(ROUNDS), "{file}");
    assert_eq!(core.user_settings().ui.theme, rb_config::Theme::Light);
    assert_eq!(core.user_settings().palette.recent_count, ROUNDS);
}

#[tokio::test]
async fn an_unreadable_user_file_is_not_written_over() {
    let dir = tempfile::tempdir().expect("temp");
    let (ui, core) = core_in(dir.path());
    let path = dir.path().join("user/settings.json");
    std::fs::create_dir_all(dir.path().join("user")).expect("folder");
    std::fs::write(&path, "{ broken").expect("file");
    core.shell_ready();
    assert!(matches!(
        set(&core, "ui.theme", json!("dark"), Scope::User),
        CoreToShell::Ack
    ));
    assert_eq!(core.user_settings().ui.theme, rb_config::Theme::Dark);
    assert_eq!(std::fs::read_to_string(&path).expect("kept"), "{ broken");
    assert!(
        sent(&ui)
            .iter()
            .any(|m| m["messageKey"] == apply::NOT_SAVED_KEY)
    );
}

#[tokio::test]
async fn pick_path_answers_at_once_and_the_choice_follows_as_an_event() {
    let dir = tempfile::tempdir().expect("temp");
    let (ui, core) = core_in(dir.path());
    let ack = answer(
        &core,
        ShellToCore::PickPath {
            kind: PathKind::File,
            title_key: Some("settings.key.engines.nodePath.label".to_owned()),
        },
    );
    assert!(matches!(ack, CoreToShell::Ack));
    assert!(
        ui.calls()
            .iter()
            .any(|call| matches!(call, UiCall::PickPath(pick) if pick.kind == PathKind::File))
    );
    assert!(
        sent(&ui)
            .iter()
            .any(|m| m == &json!({ "type": "pickedPath" }))
    );
}

#[tokio::test]
async fn a_config_reload_tells_the_pages_to_read_the_project_keys_again() {
    let dir = tempfile::tempdir().expect("temp");
    let (ui, core) = core_in(dir.path());
    on_config_changed(&core);
    let message = sent(&ui)
        .into_iter()
        .find(|m| m["type"] == "settingsChanged")
        .expect("sent");
    let keys = message["keys"].as_array().expect("keys");
    assert!(keys.iter().any(|key| key == "layout"));
    assert!(!keys.iter().any(|key| key == "ui.theme"));
}
