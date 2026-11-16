//! The generated schemas against the rules of the settings lint (`docs/settings.md`, "Which
//! property is a key"; `scripts/check-settings.sh` of RB-010-02 reads the checked-in files).

use std::collections::{BTreeMap, HashSet};

use rb_settings::{
    Category, X_CATEGORY, X_INPUT, X_LOCAL_ONLY, X_OPTIONS, X_ORDER, X_SCOPE, X_SECTION,
};
use serde_json::Map;

use super::*;
use crate::LOCAL_ONLY_KEYS;
use crate::annotate::STRUCTURAL_KEYS;

/// The settings keys of a schema: every root property but `$schema`/`version` that carries
/// annotations, and the properties of the unannotated groups, recursively. A property that is
/// neither annotated nor a group fails.
fn keys(schema: &Value) -> Vec<(String, &Map<String, Value>)> {
    fn walk<'a>(
        props: &'a Map<String, Value>,
        prefix: &str,
        out: &mut Vec<(String, &'a Map<String, Value>)>,
    ) {
        for (name, prop) in props {
            if prefix.is_empty() && STRUCTURAL_KEYS.contains(&name.as_str()) {
                continue;
            }
            let key = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}.{name}")
            };
            let map = prop
                .as_object()
                .unwrap_or_else(|| panic!("`{key}` is not an object schema"));
            if map.contains_key(X_CATEGORY) || map.contains_key(X_SECTION) {
                out.push((key, map));
            } else if let Some(Value::Object(children)) = map.get("properties") {
                walk(children, &key, out);
            } else {
                panic!("`{key}` is neither a settings key nor a group: place it in annotate.rs");
            }
        }
    }
    let mut out = Vec::new();
    if let Some(Value::Object(props)) = schema.get("properties") {
        walk(props, "", &mut out);
    }
    out
}

#[test]
fn every_property_is_a_key_or_a_group_of_keys() {
    let (project, user) = schemas();
    for schema in [project.as_value(), user.as_value()] {
        let found = keys(schema);
        assert!(found.len() > 20);
        for (key, map) in found {
            let category = map
                .get(X_CATEGORY)
                .and_then(Value::as_str)
                .and_then(Category::from_id)
                .unwrap_or_else(|| panic!("`{key}` has no valid {X_CATEGORY}"));
            let section = map[X_SECTION].as_str().unwrap_or_default();
            assert!(
                rb_settings::section(category, section).is_some(),
                "`{key}`: unknown section `{section}`"
            );
            assert!(map.get(X_ORDER).and_then(Value::as_u64).is_some(), "{key}");
            assert!(
                map.get(X_SCOPE).and_then(Value::as_array).is_some(),
                "{key}"
            );
        }
    }
    for structural in STRUCTURAL_KEYS {
        assert!(
            project.as_value()["properties"][structural]
                .get(X_CATEGORY)
                .is_none()
        );
    }
}

#[test]
fn x_order_is_unique_per_section_across_both_schemas() {
    let (project, user) = schemas();
    let mut seen: BTreeMap<(String, String), HashSet<u64>> = BTreeMap::new();
    for schema in [project.as_value(), user.as_value()] {
        for (key, map) in keys(schema) {
            let section = (
                map[X_CATEGORY].as_str().unwrap_or_default().to_owned(),
                map[X_SECTION].as_str().unwrap_or_default().to_owned(),
            );
            let order = map[X_ORDER].as_u64().unwrap_or_default();
            assert!(
                seen.entry(section).or_default().insert(order),
                "`{key}`: x-order {order} repeats in its section"
            );
        }
    }
}

#[test]
fn local_only_keys_are_marked_and_scoped_to_local() {
    let (project, user) = schemas();
    let marked: Vec<String> = keys(project.as_value())
        .into_iter()
        .filter(|(_, map)| map.get(X_LOCAL_ONLY) == Some(&Value::Bool(true)))
        .map(|(key, map)| {
            assert_eq!(map[X_SCOPE], serde_json::json!(["local"]), "{key}");
            key
        })
        .collect();
    let mut expected: Vec<&str> = LOCAL_ONLY_KEYS.to_vec();
    expected.sort_unstable();
    let mut marked: Vec<&str> = marked.iter().map(String::as_str).collect();
    marked.sort_unstable();
    assert_eq!(marked, expected);
    assert!(
        keys(user.as_value())
            .iter()
            .all(|(_, map)| map.get(X_LOCAL_ONLY).is_none())
    );
}

#[test]
fn the_schemas_are_inline_and_describe_the_keys() {
    let (project, user) = schemas();
    let project = project.as_value();
    assert!(project.get("$defs").is_none(), "subschemas are inlined");
    assert_eq!(
        project["properties"]["sync"]["properties"]["scroll"]["type"],
        "boolean"
    );
    assert!(
        project["properties"]["breakpoints"]["items"]["properties"]["dpr"].is_object(),
        "flattened emulation fields sit in the breakpoint"
    );
    let user = user.as_value();
    assert!(user["properties"]["shortcuts"]["properties"]["bindings"].is_object());
    let level = &user["properties"]["logs"]["properties"]["level"];
    assert_eq!(level["default"], "info");
    assert_eq!(level["enum"][4], "trace");
    assert_eq!(
        user["properties"]["logs"]["properties"]["maxFiles"]["maximum"],
        50
    );
}

#[test]
fn schema_files_are_canonical_json() {
    for (path, text) in schema_files() {
        assert!(path.starts_with("schema/") && path.ends_with(".v1.json"));
        let value: Value = serde_json::from_str(&text).expect("parses");
        assert_eq!(to_json_string(&value), text);
    }
}

/// The inputs of RB-080-20: paths name their dialog, known values are a choice or suggested.
#[test]
fn the_schemas_name_dialogs_choices_and_suggestions() {
    let (project, user) = schemas();
    let project = project.as_value();
    let user = user.as_value();
    let engines = &user["properties"]["engines"]["properties"];
    assert_eq!(engines["firefox"]["properties"]["path"][X_INPUT], "file");
    assert_eq!(engines["nodePath"][X_INPUT], "file");
    assert_eq!(engines["playwrightBrowsersPath"][X_INPUT], "folder");
    assert_eq!(
        user["properties"]["tools"]["properties"]["ffmpegPath"][X_INPUT],
        "file"
    );
    assert_eq!(
        project["properties"]["capture"]["properties"]["dir"][X_INPUT],
        "folder"
    );
    assert_eq!(
        project["properties"]["extensions"]["items"]["properties"]["path"][X_INPUT],
        "folder"
    );
    // Every `x-input` sits on a string.
    for schema in [project, user] {
        check_inputs(schema, "");
    }
    let lighthouse = &project["properties"]["checks"]["properties"]["lighthouse"];
    assert_eq!(
        lighthouse["properties"]["categories"]["items"]["enum"],
        serde_json::json!(["performance", "accessibility", "best-practices", "seo"])
    );
    let emulation = &project["properties"]["emulation"]["properties"];
    assert_eq!(emulation["platform"][X_OPTIONS][0], "Win32");
    assert_eq!(emulation["timezone"][X_OPTIONS][1], "Europe/Berlin");
    assert_eq!(emulation["locale"][X_OPTIONS][0], "en-US");
    assert_eq!(
        project["properties"]["breakpoints"]["items"]["properties"]["platform"][X_OPTIONS][1],
        "MacIntel"
    );
    assert_eq!(
        user["properties"]["emulation"]["properties"]["defaultTimezone"][X_OPTIONS][0],
        "UTC"
    );
}

/// RB-080-72: lists and maps of values the core knows name the source it fills them from, so
/// the settings window offers a choice with names instead of free text.
#[test]
fn known_values_name_their_source() {
    let (project, user) = schemas();
    let project = project.as_value();
    let user = user.as_value();
    let plugins = &user["properties"]["plugins"]["properties"];
    assert_eq!(plugins["disabled"][X_OPTIONS], "plugins");
    assert_eq!(
        plugins["pageScriptsDisabled"][X_OPTIONS],
        "plugin-page-scripts"
    );
    assert_eq!(plugins["panelsHidden"][X_OPTIONS], "plugin-panels");
    assert_eq!(
        project["properties"]["checks"]["properties"]["rules"][X_OPTIONS],
        "check-rules"
    );
    let compare = &project["properties"]["envCompare"]["properties"];
    assert_eq!(compare["environments"][X_OPTIONS], "environments");
    assert_eq!(compare["breakpoints"][X_OPTIONS], "breakpoints");
    // A string of a few fixed values is a choice, not free text.
    let cookies = &user["properties"]["engines"]["properties"]["cookies"]["properties"];
    assert_eq!(
        cookies["copy"]["enum"],
        serde_json::json!(["manual", "onStart", "onNavigate"])
    );
    assert_eq!(
        cookies["domainFilter"]["enum"],
        serde_json::json!(["environment", "all"])
    );
}

/// RB-090-44: the plugin panels' message limit - its range, default and place - as the core
/// holds it, and a hand-edited value outside the range is held to it.
#[test]
fn the_panel_message_limit_has_its_range_default_and_place() {
    use crate::PluginSettings;
    let (_, user) = schemas();
    let key = &user.as_value()["properties"]["plugins"]["properties"]["panelMessageMaxBytes"];
    assert_eq!(key["minimum"], PluginSettings::PANEL_MESSAGE_MIN_BYTES);
    assert_eq!(key["maximum"], PluginSettings::PANEL_MESSAGE_MAX_BYTES);
    assert_eq!(key["default"], PluginSettings::PANEL_MESSAGE_DEFAULT_BYTES);
    assert_eq!(key[X_CATEGORY], "plugins");
    assert_eq!(key[X_SECTION], "installed");
    assert!(key[X_ORDER].is_u64(), "{key}");
    let mut settings = PluginSettings::default();
    assert_eq!(settings.panel_message_limit(), 4 * 1024 * 1024);
    settings.panel_message_max_bytes = 1;
    assert_eq!(settings.panel_message_limit(), 64 * 1024);
    settings.panel_message_max_bytes = u32::MAX;
    assert_eq!(settings.panel_message_limit(), 16 * 1024 * 1024);
}

fn check_inputs(node: &Value, path: &str) {
    let Some(map) = node.as_object() else {
        return;
    };
    if let Some(input) = map.get(X_INPUT) {
        assert!(
            matches!(input.as_str(), Some("file" | "folder")),
            "{path}: x-input {input}"
        );
        let is_string = match map.get("type") {
            Some(Value::String(kind)) => kind == "string",
            Some(Value::Array(kinds)) => kinds.iter().any(|kind| kind == "string"),
            _ => false,
        };
        assert!(is_string, "{path}: x-input on a non-string");
    }
    for (name, child) in map {
        check_inputs(child, &format!("{path}/{name}"));
    }
}
