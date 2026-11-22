use std::sync::{Arc, Mutex, PoisonError};

use rb_ipc::SettingValue;
use rb_mcp::{Backend as _, LogStore, SettingScope};
use rb_settings::{Category, Origin, Scope};
use serde_json::{Value, json};

use super::{CHANGED_KEY, audit_text, document, masked_setting, read, record};
use crate::control::settings::settings_registry;
use crate::mcp::backend::CoreBackend;
use crate::mcp::tests::{core, core_and_ui, core_with_project};

/// The messages the shell got, as JSON.
fn sent(ui: &crate::host::ui::recording::RecordingUi) -> Vec<Value> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            crate::host::ui::recording::UiCall::Shell(script) => {
                let start = script.find("detail:")? + "detail:".len();
                let end = script.rfind("}));")?;
                serde_json::from_str(&script[start..end]).ok()
            }
            _ => None,
        })
        .collect()
}

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// `settings_get` without arguments: every category of the window in its order, each with its
/// sections, and every key in the window's order.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_document_has_every_category_section_and_key_in_the_window_s_order() {
    let core = core();
    let document = read(&core);
    let categories: Vec<&str> = document["categories"]
        .as_array()
        .expect("categories")
        .iter()
        .filter_map(|category| category["id"].as_str())
        .collect();
    let expected: Vec<&str> = Category::ALL.iter().map(|category| category.id()).collect();
    assert_eq!(categories, expected);
    assert!(
        document["categories"][0]["sections"]
            .as_array()
            .is_some_and(|sections| !sections.is_empty())
    );
    assert_ne!(
        document["categories"][0]["title"],
        "settings.category.general"
    );
    let keys = document["keys"].as_array().expect("keys");
    assert_eq!(keys.len(), core.settings_registry().iter().count());
    let place = |id: &str| expected.iter().position(|known| *known == id);
    let order: Vec<Option<usize>> = keys
        .iter()
        .map(|entry| place(entry["category"].as_str().unwrap_or_default()))
        .collect();
    assert!(order.windows(2).all(|pair| pair[0] <= pair[1]), "{order:?}");
    let scroll = keys
        .iter()
        .find(|entry| entry["key"] == "sync.scroll")
        .expect("sync.scroll");
    assert_eq!(scroll["type"], "boolean");
    assert_eq!(scroll["protected"], false);
    assert!(
        scroll["label"]
            .as_str()
            .is_some_and(|label| !label.contains('.'))
    );
    let evaluate = keys
        .iter()
        .find(|entry| entry["key"] == "mcp.allowEvaluate")
        .expect("listed");
    assert_eq!(evaluate["protected"], true);
}

/// A secret in a value is masked in what `settings_get` reads.
#[test]
fn a_secret_value_is_masked() {
    let (registry, _) = settings_registry();
    let values = [SettingValue {
        key: "network.proxy".to_owned(),
        value: json!("http://me:hunter2secret@proxy.test:8080"),
        origin: Origin::Local,
    }];
    let text = document(&registry, &values, &|key| key.to_owned()).to_string();
    assert!(!text.contains("hunter2secret"), "{text}");
    assert!(text.contains("network.proxy"), "{text}");
}

/// Every `x-protected` key - and every key `config_set` refuses - is refused for MCP with its
/// reason, in every scope it has, and none is written.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_protected_key_is_refused_for_mcp() {
    let dir = tempfile::tempdir().expect("temp dir");
    let core = core_with_project(dir.path());
    let backend = CoreBackend::new(&core, Arc::new(LogStore::default()));
    let before = read(&core);
    let protected: Vec<(String, Value, Vec<Scope>)> = core
        .settings_registry()
        .iter()
        .filter(|def| {
            def.protected_from_mcp
                || rb_mcp::config_edit::write_refusal(&def.key, &Value::Null).is_some()
        })
        .map(|def| {
            (
                def.key.clone(),
                def.default_value(),
                def.scope_allowed.clone(),
            )
        })
        .collect();
    assert!(protected.len() > 20, "{}", protected.len());
    for (key, value, scopes) in protected {
        for scope in scopes {
            let scope = match scope {
                Scope::User => SettingScope::User,
                Scope::Project => SettingScope::Project,
                Scope::Local => SettingScope::Local,
            };
            let failure = backend
                .set_setting(&key, &value, scope)
                .expect_err("protected");
            assert_eq!(
                failure.code, "settings.rejected.protected",
                "{key} {scope:?}"
            );
            assert_eq!(failure.settings_key.as_deref(), Some(key.as_str()));
        }
    }
    assert_eq!(read(&core), before, "nothing changed");
}

/// `settings_set("sync.scroll", false, "project")` writes `config.json`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_project_setting_lands_in_config_json() {
    let dir = tempfile::tempdir().expect("temp dir");
    let core = core_with_project(dir.path());
    let backend = CoreBackend::new(&core, Arc::new(LogStore::default()));
    backend
        .set_setting("sync.scroll", &json!(false), SettingScope::Project)
        .expect("written");
    let written =
        rb_config::read_json(&rb_config::ConfigFile::Project.path(dir.path())).expect("on disk");
    assert_eq!(written["sync"]["scroll"], false);
}

/// Every change over MCP: a log line with key, scope and the masked values, and a notice in the
/// shell that links to the key - unless `mcp.notifyOnSettingChange` is off.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_change_is_logged_and_shown_unless_the_notice_is_off() {
    let (core, ui) = core_and_ui();
    core.shell_ready();
    let log = Captured::default();
    let writer = log.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        record(
            &core,
            "grid.fitToWindow",
            SettingScope::User,
            &json!(true),
            &json!("https://me:pw12345secret@a.test/"),
        );
    });
    let text =
        String::from_utf8_lossy(&log.0.lock().unwrap_or_else(PoisonError::into_inner)).into_owned();
    assert!(text.contains("MCP changed a setting"), "{text}");
    assert!(text.contains("grid.fitToWindow"), "{text}");
    assert!(text.contains("User"), "{text}");
    assert!(!text.contains("pw12345secret"), "{text}");
    let notices: Vec<Value> = sent(&ui)
        .into_iter()
        .filter(|message| message.to_string().contains(CHANGED_KEY))
        .collect();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(notices[0].to_string().contains("grid.fitToWindow"));
    let mut quiet = core.user_settings();
    quiet.mcp.notify_on_setting_change = false;
    core.replace_user_settings(quiet);
    record(
        &core,
        "grid.fitToWindow",
        SettingScope::User,
        &json!(1),
        &json!(2),
    );
    let after = sent(&ui)
        .into_iter()
        .filter(|message| message.to_string().contains(CHANGED_KEY))
        .count();
    assert_eq!(after, 1, "no second notice");
}

#[test]
fn the_audit_text_is_masked_and_short() {
    assert_eq!(audit_text("sync.scroll", &json!(true)), "true");
    let long = audit_text("network.proxy", &json!("x".repeat(500)));
    assert!(long.ends_with("..."));
    assert!(long.chars().count() <= 203);
    let secret = audit_text(
        "network.proxy",
        &json!({ "Authorization": "Bearer abcdefgh12345678" }),
    );
    assert!(!secret.contains("abcdefgh12345678"), "{secret}");
}

/// F1 (RB-030-03): a header map's values are masked in `settings_get` - the map sits under
/// `value`, not under a `headers` key - and so are a secret-named key's value and default.
#[test]
fn a_header_value_is_masked_in_the_document() {
    let (registry, _) = settings_registry();
    let values = [SettingValue {
        key: "emulation.headers".to_owned(),
        value: json!({ "X-Custom": "headervalue42secret" }),
        origin: Origin::Local,
    }];
    let text = document(&registry, &values, &|key| key.to_owned()).to_string();
    assert!(!text.contains("headervalue42secret"), "{text}");
    assert!(text.contains("X-Custom"), "{text}");
    assert!(text.contains("emulation.headers"), "{text}");
}

/// F1: the value of a setting is masked as it sits under its key's last segment.
#[test]
fn a_setting_value_is_masked_by_its_key() {
    let headers = masked_setting("emulation.headers", &json!({ "X-Custom": "abc123secret" }));
    assert_eq!(headers, json!({ "X-Custom": "***" }));
    let token = masked_setting("some.clientSecret", &json!("tok123value"));
    assert_eq!(token, json!("***"));
    assert_eq!(masked_setting("sync.scroll", &json!(true)), json!(true));
    let audit = audit_text("emulation.headers", &json!({ "X-Custom": "audit42secret" }));
    assert!(!audit.contains("audit42secret"), "{audit}");
}

/// RB-094-26: a key annotated `x-secret` is masked in `settings_get` by the annotation, whatever
/// its name - the names inside stay, the values go.
#[test]
fn an_x_secret_value_is_masked_in_the_document_by_its_annotation() {
    let (mut registry, _) = settings_registry();
    let schema = json!({
        "type": "object",
        "properties": {
            "vault": {
                "type": "object",
                "default": {},
                "x-category": "network",
                "x-section": "api-client",
                "x-order": 90,
                "x-scope": ["local"],
                "x-local-only": true,
                "x-protected": true,
                "x-secret": true
            }
        }
    });
    registry
        .register_plugin_schema("vault-plugin", &schema)
        .expect("registers");
    let values = [SettingValue {
        key: "vault-plugin.vault".to_owned(),
        value: json!({ "staging": { "PLAIN": "opensesame99" } }),
        origin: Origin::Local,
    }];
    let text = document(&registry, &values, &|key| key.to_owned()).to_string();
    assert!(!text.contains("opensesame99"), "{text}");
    assert!(text.contains("PLAIN"), "{text}");
}
