use super::*;
use rb_settings::{X_INPUT, X_OPTIONS};
use serde_json::json;

fn section_exists(category: Category, section: &str) -> bool {
    rb_settings::section(category, section).is_some()
}

#[test]
fn every_key_names_an_existing_section_once() {
    let mut seen = std::collections::HashSet::new();
    for def in PROJECT_KEYS.iter().chain(USER_KEYS) {
        assert!(
            section_exists(def.category, def.section),
            "{}: section `{}` is not in category `{}`",
            def.key,
            def.section,
            def.category.id()
        );
        assert!(seen.insert(def.key), "key `{}` listed twice", def.key);
    }
}

#[test]
fn local_only_keys_are_project_keys() {
    for key in LOCAL_ONLY_KEYS {
        assert!(PROJECT_KEYS.iter().any(|def| def.key == *key), "{key}");
    }
}

/// What workspace trust guards, what holds request headers, where screenshots are written and
/// the recent projects `project_open` opens are a person's to change, never an agent's (batch-3
/// security review, finding 1): `settings_set` refuses them like `config_set` does.
#[test]
fn trust_relevant_keys_are_protected_from_mcp() {
    for key in [
        "network.proxy",
        "network.ignoreCertificateErrors",
        "network.hostResolverRules",
        "emulation.headers",
        "breakpoints",
        "extensions",
        "capture.dir",
        "recentProjects",
        // Where recordings land and which fields they mask (charge-4 review, recorder 4).
        "automation.recorder.dir",
        "automation.recorderExport.dir",
        "automation.recorder.secretFields",
        // The folders of Chrome extensions: code in every view (RB-080-06).
        "engines.chromium.extensions.unpacked",
        // Switching extensions or the Web Store back on loads code into every view (security
        // review RB-090-06, finding 2; RB-090-21).
        "engines.chromium.extensions.enabled",
        "engines.chromium.extensions.allowWebStore",
        // Plugins' Lighthouse runs beyond the views' sites (RB-090-21).
        "plugins.lighthouseAnyUrl",
        // The consent probe beyond public addresses and the project's own sites (RB-094-09).
        "plugins.consentProbeAnyUrl",
    ] {
        let def = PROJECT_KEYS
            .iter()
            .chain(USER_KEYS)
            .find(|def| def.key == key)
            .unwrap_or_else(|| panic!("{key} is a key"));
        assert!(def.protected, "{key} is open to MCP");
    }
}

/// An agent that raised the log level would read every MCP payload in the log (batch-3
/// security review, finding 2): `logs.level` is a person's.
#[test]
fn the_log_level_is_protected_from_mcp() {
    let def = USER_KEYS
        .iter()
        .find(|def| def.key == "logs.level")
        .expect("a key");
    assert!(def.protected);
}

#[test]
fn a_key_is_annotated_and_its_group_is_not() {
    let mut schema = json!({ "properties": {
        "network": { "type": "object", "properties": {
            "hostResolverRules": { "type": "string" },
            "proxy": { "type": "string" },
            "ignoreCertificateErrors": { "type": "boolean" }
        } },
        "startUrl": { "type": "string" }
    } });
    let mut counters = OrderCounters::new();
    let missing = annotate(&mut schema, SchemaKind::Project, &mut counters)
        .expect_err("the other keys of the table are not in this schema");
    assert!(missing.contains(&"breakpoints"));

    let network = &schema["properties"]["network"];
    assert!(network.get(X_CATEGORY).is_none(), "a group stays bare");
    let rules = &network["properties"]["hostResolverRules"];
    assert_eq!(rules[X_CATEGORY], "network");
    assert_eq!(rules[X_SECTION], "host-resolver");
    assert_eq!(rules[X_ORDER], 1);
    assert_eq!(rules[X_SCOPE], json!(["project", "local"]));
    assert!(rules.get(X_LOCAL_ONLY).is_none());
    let certs = &network["properties"]["ignoreCertificateErrors"];
    assert_eq!(certs[X_SECTION], "certificates");
    assert_eq!(certs[X_SCOPE], json!(["local"]));
    assert_eq!(certs[X_LOCAL_ONLY], true);
    assert_eq!(network["properties"]["proxy"][X_LOCAL_ONLY], true);
    assert_eq!(schema["properties"]["startUrl"][X_SECTION], "start-url");
}

#[test]
fn user_keys_are_user_scoped_and_mcp_keys_protected() {
    let mut schema = json!({ "properties": {
        "mcp": { "properties": { "enabled": {}, "allowEvaluate": {} } },
        "ui": { "properties": { "language": {}, "theme": {} } },
        "updates": { "properties": {
            "enabled": {}, "channel": {}, "check": {}, "autoInstall": {}, "cefAgeWarning": {}
        } }
    } });
    let _ = annotate(&mut schema, SchemaKind::User, &mut OrderCounters::new());
    let props = &schema["properties"];
    let evaluate = &props["mcp"]["properties"]["allowEvaluate"];
    assert_eq!(evaluate[X_PROTECTED], true);
    assert_eq!(evaluate[X_CATEGORY], "security");
    assert_eq!(evaluate[X_SCOPE], json!(["user"]));
    let theme = &props["ui"]["properties"]["theme"];
    assert!(theme.get(X_PROTECTED).is_none());
    assert_eq!(theme[X_SECTION], "appearance");
    let updates = &props["updates"]["properties"];
    // Self-update (RB-090-02): the switch first, every update key protected from MCP.
    for (order, key) in ["enabled", "channel", "check", "autoInstall"]
        .iter()
        .enumerate()
    {
        assert_eq!(updates[key][X_ORDER], order + 1, "{key}");
        assert_eq!(updates[key][X_PROTECTED], true, "{key}");
        assert_eq!(updates[key][X_SECTION], "updates", "{key}");
    }
    assert_eq!(updates["cefAgeWarning"][X_ORDER], 10, "a fixed order");
    assert!(updates["cefAgeWarning"].get(X_PROTECTED).is_none());
}

/// Where screenshots are written is the person's choice, never an agent's (batch-2 security
/// review, H2): `capture.dir` is protected in every scope.
#[test]
fn the_screenshot_folder_is_protected() {
    let mut schema = json!({ "properties": {
        "capture": { "properties": { "dir": {}, "pathPattern": {} } }
    } });
    let _ = annotate(&mut schema, SchemaKind::Project, &mut OrderCounters::new());
    let capture = &schema["properties"]["capture"]["properties"];
    assert_eq!(capture["dir"][X_PROTECTED], true);
    assert!(capture["pathPattern"].get(X_PROTECTED).is_none());
}

/// "Save layout as preset" (RB-080-04): where it writes by default is a Layout > Further modes
/// setting of the person's, not protected (MCP's `layout_save_preset` names its file itself).
#[test]
fn the_preset_file_setting_sits_with_the_layout_modes() {
    let mut schema = json!({ "properties": {
        "layoutModes": { "properties": { "spacePan": {}, "savePresetTo": {} } }
    } });
    let _ = annotate(&mut schema, SchemaKind::User, &mut OrderCounters::new());
    let modes = &schema["properties"]["layoutModes"]["properties"];
    let save = &modes["savePresetTo"];
    assert_eq!(save[X_CATEGORY], "layout");
    assert_eq!(save[X_SECTION], "modes");
    assert_eq!(save[X_SCOPE], json!(["user"]));
    assert!(save.get(X_PROTECTED).is_none());
    assert!(save[X_ORDER].as_u64() > modes["spacePan"][X_ORDER].as_u64());
}

/// Every path a native dialog fills carries `x-input` (RB-080-20): the keys, and the fields of
/// a list's items.
#[test]
fn path_keys_and_fields_carry_their_dialog() {
    let mut schema = json!({ "properties": {
        "engines": { "properties": {
            "firefox": { "properties": { "path": { "type": ["string", "null"] }, "profileDir": {} } },
            "chromium": { "properties": { "extensions": { "properties": {
                "unpacked": { "type": "array", "items": { "properties": { "path": { "type": "string" } } } }
            } } } }
        } }
    } });
    let _ = annotate(&mut schema, SchemaKind::User, &mut OrderCounters::new());
    let engines = &schema["properties"]["engines"]["properties"];
    assert_eq!(engines["firefox"]["properties"]["path"][X_INPUT], "file");
    // RB-080-32: the installed Firefox editions come from the core at runtime.
    assert_eq!(
        engines["firefox"]["properties"]["path"][X_OPTIONS],
        "dynamic"
    );
    assert_eq!(
        engines["firefox"]["properties"]["profileDir"][X_INPUT],
        "folder"
    );
    assert!(
        engines["firefox"]["properties"]["profileDir"]
            .get(X_OPTIONS)
            .is_none()
    );
    let unpacked = &engines["chromium"]["properties"]["extensions"]["properties"]["unpacked"];
    assert_eq!(unpacked["items"]["properties"]["path"][X_INPUT], "folder");
}

#[test]
fn a_missing_input_path_is_reported() {
    let mut schema = json!({ "properties": { "capture": { "properties": { "dir": {} } } } });
    let missing = annotate_inputs(&mut schema, PROJECT_INPUTS).expect_err("four are missing");
    assert_eq!(
        missing,
        [
            "checks.export.dir",
            "automation.recorder.dir",
            "automation.recorderExport.dir",
            "extensions[].path"
        ]
    );
    assert_eq!(
        schema["properties"]["capture"]["properties"]["dir"][X_INPUT],
        "folder"
    );
}

/// Every key named like a path or a folder is in the input table, so the lint's rule
/// (`scripts/lib/settings_schema.py`) and the table cannot drift.
#[test]
fn every_path_named_key_has_an_input() {
    for (keys, inputs) in [(PROJECT_KEYS, PROJECT_INPUTS), (USER_KEYS, USER_INPUTS)] {
        for def in keys {
            let leaf = def
                .key
                .rsplit('.')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            // `emulation.dir` is the text direction (`ltr`/`rtl`, RB-092-02), an enum the lint
            // passes over too (`scripts/lib/settings_schema.py` asks for a plain string).
            let not_a_path = def.key == "emulation.dir";
            if (leaf.ends_with("path") || leaf.ends_with("dir")) && !not_a_path {
                assert!(
                    inputs.iter().any(|input| input.path == def.key),
                    "{}: a path key without an x-input entry",
                    def.key
                );
            }
        }
    }
}
