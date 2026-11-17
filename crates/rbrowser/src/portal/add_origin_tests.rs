use std::path::{Path, PathBuf};
use std::sync::Arc;

use rb_config::UserSettings;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::ui::recording::RecordingUi;

/// `config.json` in the form the config is written in, so an untouched part stays the same
/// byte for byte.
const CONFIG: &str = r#"{
  "version": 1,
  "startUrl": "http://localhost:8000",
  "breakpoints": [
    {
      "id": "mobile",
      "width": 375,
      "height": 812
    }
  ],
  "environments": [
    {
      "id": "local",
      "url": "http://localhost:8000",
      "assetOrigins": [
        "http://127.0.0.1:5174"
      ]
    },
    {
      "id": "staging",
      "url": "https://staging.example.com"
    }
  ]
}
"#;

/// `config.local.json`: it sets the local environment's list (which replaces the one of
/// `config.json`) and adds an environment of its own.
const LOCAL: &str = r#"{
  "environments": [
    {
      "id": "local",
      "assetOrigins": [
        "http://127.0.0.1:5176"
      ]
    },
    {
      "id": "mine",
      "url": "http://localhost:9000"
    }
  ]
}
"#;

struct Project {
    core: Arc<Core>,
    config: PathBuf,
    local: PathBuf,
}

/// A core on a project whose `config.json` is [`CONFIG`] (and `config.local.json` `local`),
/// in effect as read from the files; the offer as `offer` says.
fn project(dir: &Path, local: Option<&str>, offer: bool) -> Project {
    let root = dir.join("site");
    std::fs::create_dir_all(root.join(".rbrowser")).expect("folder");
    let config = root.join(".rbrowser/config.json");
    let local_path = root.join(".rbrowser/config.local.json");
    std::fs::write(&config, CONFIG).expect("config");
    if let Some(text) = local {
        std::fs::write(&local_path, text).expect("local config");
    }
    let loaded = crate::project::load(&root).expect("a valid config");
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(loaded, Some(root)).expect("opens");
    let mut user = UserSettings::default();
    user.portal.offer_asset_origins = offer;
    let setup = CoreSetup {
        user,
        user_path: Some(dir.join("user/settings.json")),
        ..CoreSetup::default()
    };
    reset();
    Project {
        core: Core::new(host, ui, Handle::current(), setup),
        config,
        local: local_path,
    }
}

fn text(path: &Path) -> String {
    std::fs::read_to_string(path).expect("the file")
}

/// `before` with `origin` appended to the list that ends with `last` (the form the config is
/// written in).
fn appended(before: &str, last: &str, origin: &str) -> String {
    let line = format!("        \"{last}\"\n");
    assert!(before.contains(&line), "{line}");
    before.replace(
        &line,
        &format!("        \"{last}\",\n        \"{origin}\"\n"),
    )
}

fn effective(core: &Core, id: &str) -> Vec<String> {
    core.host()
        .config()
        .environments
        .into_iter()
        .find(|env| env.id == id)
        .map(|env| env.asset_origins)
        .unwrap_or_default()
}

/// The status bar offers the origin with a button that runs the command for the environment
/// the page belongs to, and links to the environments' setting; once per shown environment.
#[tokio::test(flavor = "multi_thread")]
async fn an_unmapped_origin_is_offered_with_a_button() {
    let dir = tempfile::tempdir().expect("temp");
    let Project { core, .. } = project(dir.path(), None, true);
    let before = core.status_log().items().len();
    offered(&core, "http://localhost:8000", "http://localhost:5173");
    let message = core.status_log().items().pop().expect("a message");
    assert_eq!(message.message_key, OFFER_KEY);
    assert_eq!(message.settings_key.as_deref(), Some(ENVIRONMENTS_KEY));
    assert_eq!(message.args["origin"], "http://localhost:5173");
    assert_eq!(message.args["environment"], "local");
    let action = message.action.expect("a button");
    assert_eq!(action.command, ADD_ASSET_ORIGIN);
    assert_eq!(action.label_key, ADD_LABEL_KEY);
    assert_eq!(action.label_args["origin"], "http://localhost:5173");
    assert_eq!(
        action.args,
        json!({ "environment": "local", "origin": "http://localhost:5173" })
    );
    // Once while that environment is shown; a page of no environment's origin has nothing to
    // add the origin to.
    offered(&core, "http://localhost:8000", "http://localhost:5173");
    offered(&core, "http://localhost:9999", "http://localhost:5173");
    assert_eq!(core.status_log().items().len(), before + 1);
    // After another environment was shown, and after a start of the portal, it is offered again.
    offered(&core, "http://localhost:8000", "http://localhost:5173");
    assert_eq!(core.status_log().items().len(), before + 2);
    reset();
    offered(&core, "http://localhost:8000", "http://localhost:5173");
    assert_eq!(core.status_log().items().len(), before + 3);
}

/// `portal.offerAssetOrigins` off: the log line alone, no offer.
#[tokio::test(flavor = "multi_thread")]
async fn switched_off_nothing_is_offered() {
    let dir = tempfile::tempdir().expect("temp");
    let Project { core, .. } = project(dir.path(), None, false);
    let before = core.status_log().items().len();
    offered(&core, "http://localhost:8000", "http://localhost:5173");
    assert_eq!(core.status_log().items().len(), before);
    assert!(run(&core, &Value::Null).is_err(), "no offer waits");
}

/// Without `config.local.json` the origin joins the list in `config.json`; nothing else of the
/// file changes. Exactly that origin: another loopback name of a listed port is added as such.
#[tokio::test(flavor = "multi_thread")]
async fn the_click_adds_the_origin_to_config_json_only() {
    let dir = tempfile::tempdir().expect("temp");
    let Project { core, config, .. } = project(dir.path(), None, true);
    let args = json!({ "environment": "local", "origin": "http://localhost:5174" });
    let answer = run(&core, &args).expect("added");
    assert_eq!(answer["added"], true);
    let once = appended(CONFIG, "http://127.0.0.1:5174", "http://localhost:5174");
    assert_eq!(text(&config), once);
    assert_eq!(
        effective(&core, "local"),
        vec!["http://127.0.0.1:5174", "http://localhost:5174"],
        "the config in effect follows at once"
    );
    assert_eq!(run(&core, &args).expect("listed")["added"], false);
    assert_eq!(text(&config), once);
}

/// When `config.local.json` sets the environment's list, the origin goes there and the click
/// takes effect; `config.json` and every other part of `config.local.json` stay as they were.
#[tokio::test(flavor = "multi_thread")]
async fn a_list_set_in_the_local_file_gets_the_origin_there() {
    let dir = tempfile::tempdir().expect("temp");
    let Project {
        core,
        config,
        local,
    } = project(dir.path(), Some(LOCAL), true);
    let args = json!({ "environment": "local", "origin": "http://localhost:5173" });
    assert_eq!(run(&core, &args).expect("added")["added"], true);
    assert_eq!(text(&config), CONFIG);
    assert_eq!(
        text(&local),
        appended(LOCAL, "http://127.0.0.1:5176", "http://localhost:5173")
    );
    assert_eq!(
        effective(&core, "local"),
        vec!["http://127.0.0.1:5176", "http://localhost:5173"],
        "the click takes effect"
    );
}

/// The local file's own environment gets its list there; a project environment the local file
/// only colours keeps its list in `config.json`.
#[tokio::test(flavor = "multi_thread")]
async fn each_environment_gets_the_origin_in_its_own_file() {
    let dir = tempfile::tempdir().expect("temp");
    let Project {
        core,
        config,
        local,
    } = project(dir.path(), Some(LOCAL), true);
    let mine = json!({ "environment": "mine", "origin": "http://localhost:5180" });
    assert_eq!(run(&core, &mine).expect("added")["added"], true);
    assert_eq!(text(&config), CONFIG);
    let expected = LOCAL.replace(
        "      \"url\": \"http://localhost:9000\"\n",
        "      \"url\": \"http://localhost:9000\",\n      \"assetOrigins\": [\n        \"http://localhost:5180\"\n      ]\n",
    );
    assert_eq!(text(&local), expected);
    assert_eq!(effective(&core, "mine"), vec!["http://localhost:5180"]);
}

/// Without arguments the command takes the oldest offer that waits and says which; the button
/// names its own.
#[tokio::test(flavor = "multi_thread")]
async fn without_arguments_the_oldest_offer_is_added() {
    let dir = tempfile::tempdir().expect("temp");
    let Project { core, config, .. } = project(dir.path(), None, true);
    assert!(run(&core, &Value::Null).is_err());
    offered(&core, "http://localhost:8000", "http://127.0.0.1:5175");
    offered(&core, "http://localhost:8000", "http://127.0.0.1:5177");
    let first = run(&core, &Value::Null).expect("added");
    assert_eq!(first["origin"], "http://127.0.0.1:5175");
    let notice = core.status_log().items().pop().expect("a notice");
    assert_eq!(notice.message_key, ADDED_KEY);
    assert_eq!(notice.args["origin"], "http://127.0.0.1:5175");
    assert_eq!(notice.args["environment"], "local");
    let second = run(&core, &Value::Null).expect("added");
    assert_eq!(second["origin"], "http://127.0.0.1:5177");
    assert!(run(&core, &Value::Null).is_err(), "every offer was taken");
    let once = appended(CONFIG, "http://127.0.0.1:5174", "http://127.0.0.1:5175");
    assert_eq!(
        text(&config),
        appended(&once, "http://127.0.0.1:5175", "http://127.0.0.1:5177")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_foreign_origin_or_an_unknown_environment_writes_nothing() {
    let dir = tempfile::tempdir().expect("temp");
    let Project { core, config, .. } = project(dir.path(), None, true);
    for args in [
        json!({ "environment": "local", "origin": "http://cdn.example.com" }),
        json!({ "environment": "local", "origin": "http://localhost:5173/x" }),
        json!({ "environment": "gone", "origin": "http://localhost:5173" }),
        json!({ "origin": "http://localhost:5173" }),
    ] {
        assert!(run(&core, &args).is_err(), "{args}");
    }
    assert_eq!(text(&config), CONFIG);
}
