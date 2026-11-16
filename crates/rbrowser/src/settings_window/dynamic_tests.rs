use std::path::PathBuf;
use std::sync::Arc;

use rb_bidi::launcher::{Installed, Kind};
use rb_ipc::CoreToShell;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::cdp::lock;
use crate::control::{Core, CoreSetup};
use crate::engines::firefox::installed::TEST_INSTALLED;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

fn payload() -> Value {
    json!({
        "version": 1,
        "defs": [
            { "key": "engines.firefox.path", "schema": { "type": ["string", "null"] } },
            { "key": "engines.firefox.profileDir", "schema": { "type": ["string", "null"] } }
        ]
    })
}

#[test]
fn the_data_lands_at_its_key_and_nowhere_else() {
    let mut payload = payload();
    add(
        &mut payload,
        "engines.firefox.path",
        json!({ "options": [{ "value": "C:\\ff\\firefox.exe", "label": "Firefox 134.0" }] }),
    );
    assert_eq!(
        payload["defs"][0]["dynamic"]["options"][0]["value"],
        json!("C:\\ff\\firefox.exe")
    );
    assert!(payload["defs"][1].get("dynamic").is_none());
    assert_eq!(payload["defs"][0]["schema"]["type"][0], json!("string"));
}

#[test]
fn an_unknown_key_changes_nothing() {
    let mut payload = payload();
    let before = payload.clone();
    add(&mut payload, "engines.nope", json!({ "options": [] }));
    assert_eq!(payload, before);
}

#[tokio::test]
async fn the_schema_answer_carries_the_installed_firefox_editions() {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(host, ui, Handle::current(), CoreSetup::default());
    *lock(&TEST_INSTALLED) = vec![Installed {
        path: PathBuf::from("/opt/firefox/firefox"),
        kind: Kind::Release,
        version: Some("134.0".to_owned()),
    }];
    let CoreToShell::Settings { schema, .. } = super::super::apply::settings_answer(&core, true)
    else {
        panic!("settings");
    };
    *lock(&TEST_INSTALLED) = Vec::new();
    let schema = schema.expect("the schema was asked for");
    let def = schema["defs"]
        .as_array()
        .expect("defs")
        .iter()
        .find(|def| def["key"] == "engines.firefox.path")
        .expect("the Firefox path is listed");
    assert_eq!(
        def["dynamic"]["options"][0]["value"],
        json!("/opt/firefox/firefox")
    );
    assert!(def["dynamic"].get("missing").is_none());
    assert_eq!(def["schema"]["x-options"], json!("dynamic"));
}

fn facts() -> Vec<PluginFacts> {
    vec![
        PluginFacts {
            id: "seo".to_owned(),
            name: "SEO".to_owned(),
            panels: true,
            page_scripts: false,
        },
        PluginFacts {
            id: "console".to_owned(),
            name: "Console".to_owned(),
            panels: true,
            page_scripts: true,
        },
        PluginFacts {
            id: "overlay-grid".to_owned(),
            name: "Grid overlay".to_owned(),
            panels: false,
            page_scripts: true,
        },
    ]
}

fn values(data: &Value) -> Vec<&str> {
    data["options"]
        .as_array()
        .expect("options")
        .iter()
        .map(|option| option["value"].as_str().expect("value"))
        .collect()
}

/// RB-080-72: each plugin source offers the plugins it is about, by name.
#[test]
fn the_plugin_sources_offer_their_plugins_by_name() {
    let facts = facts();
    let plugins = plugin_options(&facts, "plugins").expect("a plugin source");
    assert_eq!(values(&plugins), ["console", "overlay-grid", "seo"]);
    assert_eq!(plugins["options"][1]["label"], json!("Grid overlay"));
    assert!(plugins.get("open").is_none(), "only the plugins there are");
    assert_eq!(
        values(&plugin_options(&facts, "plugin-panels").expect("panels")),
        ["console", "seo"]
    );
    assert_eq!(
        values(&plugin_options(&facts, "plugin-page-scripts").expect("page scripts")),
        ["console", "overlay-grid"]
    );
    assert!(plugin_options(&facts, "check-rules").is_none());
}

/// Review 2: the sources travel apart from the schema and are asked again while the window is
/// open, so a breakpoint added since shows up.
#[tokio::test]
async fn the_sources_answer_follows_the_project() {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(host, ui, Handle::current(), CoreSetup::default());
    let before = sources(&core);
    let ids = |data: &serde_json::Map<String, Value>| values(&data["breakpoints"]).len();
    let count = ids(&before);
    let mut config = default_config();
    let mut added = config.breakpoints[0].clone();
    added.id = "added-later".to_owned();
    config.breakpoints.push(added);
    core.host().open_config(config, None).expect("opens again");
    let after = sources(&core);
    assert_eq!(ids(&after), count + 1);
    assert!(values(&after["breakpoints"]).contains(&"added-later"));
    let CoreToShell::Settings { schema, .. } = super::super::apply::settings_answer(&core, true)
    else {
        panic!("settings");
    };
    let schema = schema.expect("the schema");
    assert!(
        schema["defs"]
            .as_array()
            .expect("defs")
            .iter()
            .all(|def| def["key"] == "engines.firefox.path" || def.get("dynamic").is_none()),
        "the schema the palette asks for carries no source"
    );
}

/// Every source a schema names is one the core fills, and the settings lint knows the same.
#[test]
fn the_schemas_lint_and_core_name_the_same_sources() {
    fn walk<'a>(node: &'a Value, found: &mut Vec<&'a str>) {
        match node {
            Value::Object(map) => {
                if let Some(Value::String(source)) = map.get("x-options") {
                    found.push(source);
                }
                map.values().for_each(|child| walk(child, found));
            }
            Value::Array(items) => items.iter().for_each(|child| walk(child, found)),
            _ => {}
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut text = Vec::new();
    for file in ["schema/config.v1.json", "schema/settings.v1.json"] {
        text.push(std::fs::read_to_string(root.join(file)).expect("the schema is checked in"));
    }
    let schemas: Vec<Value> = text
        .iter()
        .map(|text| serde_json::from_str(text).expect("JSON"))
        .collect();
    let mut found = Vec::new();
    schemas.iter().for_each(|schema| walk(schema, &mut found));
    for source in found {
        assert!(
            source == "dynamic" || SOURCES.contains(&source),
            "the schema names the source `{source}` the core does not fill"
        );
    }
    let lint = std::fs::read_to_string(root.join("scripts/lib/settings_schema.py"))
        .expect("the lint is checked in");
    let listed: String = lint
        .split_once("OPTION_SOURCES = (")
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(list, _)| list.split_whitespace().collect())
        .expect("the lint has OPTION_SOURCES");
    let quoted: Vec<String> = SOURCES
        .iter()
        .map(|source| format!("\"{source}\""))
        .collect();
    assert_eq!(
        listed,
        quoted.join(","),
        "scripts/lib/settings_schema.py lists the sources of `SOURCES` in the same order"
    );
}

#[test]
fn the_project_sources_offer_its_environments_and_breakpoints() {
    let mut config = default_config();
    config.environments = vec![
        serde_json::from_value(json!({ "id": "local", "url": "http://localhost:5173" }))
            .expect("an environment"),
    ];
    let environments = config_options(&config, "environments").expect("environments");
    assert_eq!(values(&environments), ["local"]);
    assert_eq!(
        environments["options"][0]["label"],
        json!("local - http://localhost:5173")
    );
    let breakpoints = config_options(&config, "breakpoints").expect("breakpoints");
    let ids: Vec<&str> = config.breakpoints.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(values(&breakpoints), ids);
    assert!(config_options(&config, "plugins").is_none());
}
