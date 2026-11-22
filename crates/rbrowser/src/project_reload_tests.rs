use std::path::Path;
use std::sync::Arc;

use rb_ipc::Scope;
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

const CONFIG: &str = r#"{
  "version": 1,
  "startUrl": "http://localhost:3000",
  "environments": [
    { "id": "local", "url": "http://localhost:3000" }
  ]
}
"#;

/// A core on a project whose config the watcher never reports - as over a WSL share
/// (`\\wsl.localhost`, no change notifications): no watch is started here at all.
fn core_in(dir: &Path) -> Arc<Core> {
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
    Core::new(host, ui, Handle::current(), setup)
}

fn environment_ids(core: &Core) -> Vec<String> {
    crate::ipc::snapshot(core.host())
        .environments
        .into_iter()
        .map(|environment| environment.id)
        .collect()
}

fn set_environments(core: &Core, value: serde_json::Value) {
    let answer = crate::settings_window::set_setting(
        core,
        "environments",
        &value,
        Scope::Project,
        crate::settings_window::Caller::Shell,
    );
    assert!(
        matches!(answer, rb_ipc::CoreToShell::Ack),
        "written: {answer:?}"
    );
}

/// RB-094-23: an environment the settings window creates, renames or deletes is in the state the
/// main menu, the palette and the toolbar menus read - and in the config MCP reads - at once,
/// with no watcher event and no restart.
#[tokio::test]
async fn an_environment_written_in_the_settings_is_in_effect_at_once() {
    let dir = tempfile::tempdir().expect("temp");
    let core = core_in(dir.path());
    set_environments(
        &core,
        json!([
            { "id": "local", "url": "http://localhost:3000" },
            { "id": "staging", "url": "https://staging.example.com" }
        ]),
    );
    assert_eq!(environment_ids(&core), ["local", "staging"]);
    assert!(
        core.host()
            .config()
            .environments
            .iter()
            .any(|environment| environment.id == "staging"),
        "the config MCP reads holds it"
    );
    // Renamed.
    set_environments(
        &core,
        json!([
            { "id": "local", "url": "http://localhost:3000" },
            { "id": "stage", "url": "https://staging.example.com" }
        ]),
    );
    assert_eq!(environment_ids(&core), ["local", "stage"]);
    // Deleted.
    set_environments(
        &core,
        json!([{ "id": "local", "url": "http://localhost:3000" }]),
    );
    assert_eq!(environment_ids(&core), ["local"]);
}

/// The settings window applies its write, and the watcher's event for the same write (on a disk
/// that sends one) finds nothing new: the grid is not built twice.
#[tokio::test]
async fn the_same_files_apply_once() {
    let dir = tempfile::tempdir().expect("temp");
    let core = core_in(dir.path());
    assert!(matches!(apply_from_disk(&core), Ok(Reloaded::Applied)));
    assert!(matches!(apply_from_disk(&core), Ok(Reloaded::Unchanged)));
    let file = dir.path().join("site/.rbrowser/config.json");
    let changed = CONFIG.replace("localhost:3000\" }", "localhost:3001\" }");
    std::fs::write(&file, changed).expect("written");
    assert!(matches!(apply_from_disk(&core), Ok(Reloaded::Applied)));
    assert_eq!(
        core.host().config().environments[0].url,
        "http://localhost:3001"
    );
}

/// A reload of its own (`app.reload_config`, a saved preset, a view edit, the engine, the
/// portal, the first-run wizard: `commands::reload_config`) is remembered as applied too, so a
/// later write back to the earlier bytes is not taken for the files applied last.
#[tokio::test]
async fn a_direct_reload_is_remembered_and_a_write_back_applies() {
    let dir = tempfile::tempdir().expect("temp");
    let core = core_in(dir.path());
    assert!(matches!(apply_from_disk(&core), Ok(Reloaded::Applied)));
    let file = dir.path().join("site/.rbrowser/config.json");
    let staging = CONFIG.replace(
        "{ \"id\": \"local\", \"url\": \"http://localhost:3000\" }",
        "{ \"id\": \"local\", \"url\": \"http://localhost:3000\" },\n    { \"id\": \"staging\", \"url\": \"https://staging.example.com\" }",
    );
    assert_ne!(staging, CONFIG);
    std::fs::write(&file, &staging).expect("written");
    crate::commands::reload_config(core.host()).expect("reloaded");
    assert_eq!(environment_ids(&core), ["local", "staging"]);
    assert!(
        matches!(apply_from_disk(&core), Ok(Reloaded::Unchanged)),
        "the files the direct reload applied"
    );

    std::fs::write(&file, CONFIG).expect("written back");
    assert!(matches!(apply_from_disk(&core), Ok(Reloaded::Applied)));
    assert_eq!(environment_ids(&core), ["local"]);
}

#[tokio::test]
async fn a_config_that_does_not_load_is_not_applied_and_tried_again() {
    let dir = tempfile::tempdir().expect("temp");
    let core = core_in(dir.path());
    let file = dir.path().join("site/.rbrowser/config.json");
    std::fs::write(&file, "{ not json").expect("written");
    assert!(apply_from_disk(&core).is_err());
    assert!(apply_from_disk(&core).is_err(), "not remembered as applied");
}

#[test]
fn the_fingerprint_tells_the_two_files_and_the_root_apart() {
    let dir = tempfile::tempdir().expect("temp");
    let root = dir.path();
    std::fs::create_dir_all(root.join(".rbrowser")).expect("folder");
    let none = fingerprint(root);
    std::fs::write(root.join(".rbrowser/config.json"), "{}").expect("written");
    let shared = fingerprint(root);
    assert_ne!(none, shared);
    std::fs::write(root.join(".rbrowser/config.local.json"), "{}").expect("written");
    let both = fingerprint(root);
    assert_ne!(shared, both);
    assert_eq!(both, fingerprint(root));
    assert_ne!(both, fingerprint(&root.join("elsewhere")));
}

/// RB-094-24 (owner, 2026-11-22): Basic auth added to an environment, then the environment
/// renamed in the settings - the challenge of its host is still answered, and the view stays in
/// the renamed environment.
#[tokio::test]
async fn a_renamed_environment_keeps_its_basic_auth() {
    use crate::basic_auth::store::MemoryStore;
    use crate::basic_auth::{BasicAuth, Challenge, Credential};

    let dir = tempfile::tempdir().expect("temp");
    let core = core_in(dir.path());
    let file = dir.path().join("site/.rbrowser/config.json");
    std::fs::write(
        &file,
        r#"{
  "version": 1,
  "environments": [
    { "id": "local", "url": "http://localhost:3000" },
    { "id": "staging", "url": "https://staging.example.test/" }
  ],
  "basicAuth": { "environments": ["staging"] },
  "envCompare": { "environments": ["local", "staging"] }
}
"#,
    )
    .expect("written");
    apply_from_disk(&core).expect("applied");
    core.host()
        .update_shell_state(|shell| shell.environment = Some("staging".to_owned()));
    let auth = BasicAuth::new(Box::new(MemoryStore::default()));
    let credential = Credential::new("ada", "rename-test-password-1").expect("valid");
    auth.set("project-a", &core.host().config(), "staging", &credential)
        .expect("stored");

    set_environments(
        &core,
        json!([
            { "id": "local", "url": "http://localhost:3000" },
            { "id": "stage", "url": "https://staging.example.test/" }
        ]),
    );

    let config = core.host().config();
    assert!(config.basic_auth.uses("stage"), "{:?}", config.basic_auth);
    assert!(!config.basic_auth.uses("staging"));
    assert_eq!(config.env_compare.environments, ["local", "stage"]);
    let challenge = Challenge {
        origin_url: "https://staging.example.test/",
        is_proxy: false,
        host: "staging.example.test",
        port: 443,
        realm: "Staging",
        scheme: "basic",
    };
    assert_eq!(
        auth.answer(&challenge, 1, Some("project-a"), &config),
        Ok(credential)
    );
    assert_eq!(
        core.host().shell_state().environment.as_deref(),
        Some("stage")
    );
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("read")).expect("json");
    assert_eq!(written["basicAuth"]["environments"], json!(["stage"]));
}
