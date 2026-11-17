use std::path::PathBuf;

use rb_config::{ConfigFile, UserSettings};
use serde_json::json;

use super::*;
use crate::host::state::default_config;
use crate::settings_window::deepcheck::reason as deep;
use crate::settings_window::values::ProjectDocuments;

fn registry() -> SettingsRegistry {
    let (registry, issues) = crate::control::settings::settings_registry();
    assert!(issues.is_empty(), "{issues:?}");
    registry
}

fn documents(project_file: Value, local_file: Option<Value>) -> Documents {
    Documents {
        user: serde_json::to_value(UserSettings::default()).expect("serializes"),
        user_file: Some(json!({ "ui": { "theme": "dark" } })),
        project: Some(ProjectDocuments {
            root: PathBuf::from("/work/site"),
            effective: serde_json::to_value(default_config()).expect("serializes"),
            project_file,
            local_file,
        }),
    }
}

fn reasons(issues: &[SettingIssue]) -> Vec<(&str, &str)> {
    issues
        .iter()
        .map(|issue| (issue.path.as_str(), issue.message_key.as_str()))
        .collect()
}

fn project_file() -> Value {
    serde_json::from_str(
        r#"{
  "version": 1,
  "startUrl": "http://localhost:3000",
  "sync": { "scroll": true, "hover": false },
  "layout": { "columns": 2, "gap": 8 }
}"#,
    )
    .expect("json")
}

#[test]
fn saving_one_field_changes_only_that_key_of_config_json() {
    let registry = registry();
    let docs = documents(project_file(), None);
    let before = rb_config::to_json_string(&project_file());
    let Ok(Plan::Project { file, document, .. }) = plan(
        &registry,
        &docs,
        "sync.hover",
        &json!(true),
        Scope::Project,
        Caller::Shell,
    ) else {
        panic!("planned");
    };
    assert_eq!(file, ConfigFile::Project);
    let after = rb_config::to_json_string(&document);
    let changed: Vec<(&str, &str)> = before
        .lines()
        .zip(after.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(before.lines().count(), after.lines().count());
    assert_eq!(changed, [("    \"hover\": false", "    \"hover\": true")]);
}

#[test]
fn a_layout_gap_change_writes_the_gap_alone() {
    let registry = registry();
    let docs = documents(project_file(), None);
    let mut layout = docs.effective(registry.get("layout").expect("known"));
    layout["gap"] = json!(16);
    let Ok(Plan::Project { document, .. }) = plan(
        &registry,
        &docs,
        "layout",
        &layout,
        Scope::Project,
        Caller::Shell,
    ) else {
        panic!("planned");
    };
    assert_eq!(document["layout"], json!({ "columns": 2, "gap": 16 }));
}

#[test]
fn a_local_only_key_is_refused_for_the_project_with_its_reason() {
    let registry = registry();
    let docs = documents(project_file(), None);
    let issues = plan(
        &registry,
        &docs,
        "network.ignoreCertificateErrors",
        &json!(true),
        Scope::Project,
        Caller::Shell,
    )
    .expect_err("refused");
    assert_eq!(reasons(&issues), [("", reason::LOCAL_ONLY)]);
    let Ok(Plan::Project { file, document, .. }) = plan(
        &registry,
        &docs,
        "network.ignoreCertificateErrors",
        &json!(true),
        Scope::Local,
        Caller::Shell,
    ) else {
        panic!("planned for the local file");
    };
    assert_eq!(file, ConfigFile::Local);
    assert_eq!(
        document,
        json!({ "network": { "ignoreCertificateErrors": true } })
    );
}

#[test]
fn reset_removes_the_value_from_config_local_json() {
    let registry = registry();
    let docs = documents(
        project_file(),
        Some(json!({ "sync": { "scroll": false }, "startUrl": "http://localhost:4000" })),
    );
    let Ok(Plan::Project { file, document, .. }) = plan(
        &registry,
        &docs,
        "sync.scroll",
        &Value::Null,
        Scope::Local,
        Caller::Shell,
    ) else {
        panic!("planned");
    };
    assert_eq!(file, ConfigFile::Local);
    assert_eq!(document, json!({ "startUrl": "http://localhost:4000" }));
}

#[test]
fn a_negative_width_is_refused_at_its_field() {
    let registry = registry();
    let docs = documents(project_file(), None);
    let mut breakpoints = docs.effective(registry.get("breakpoints").expect("known"));
    breakpoints[0]["width"] = json!(-5);
    let issues = plan(
        &registry,
        &docs,
        "breakpoints",
        &breakpoints,
        Scope::Project,
        Caller::Shell,
    )
    .expect_err("refused");
    assert_eq!(reasons(&issues), [("/0/width", deep::MINIMUM)]);
}

#[test]
fn a_rule_of_the_config_is_named_below_the_key() {
    let registry = registry();
    let docs = documents(project_file(), None);
    let mut breakpoints = docs.effective(registry.get("breakpoints").expect("known"));
    breakpoints[0]["width"] = json!(0);
    let issues = plan(
        &registry,
        &docs,
        "breakpoints",
        &breakpoints,
        Scope::Project,
        Caller::Shell,
    )
    .expect_err("refused");
    assert_eq!(reasons(&issues), [("/0/width", "config.error.size_zero")]);
}

#[test]
fn a_breakpoint_needs_a_size_or_a_known_device_preset() {
    let registry = registry();
    let docs = documents(project_file(), None);
    let with = |device: Option<&str>| {
        let mut breakpoints = docs.effective(registry.get("breakpoints").expect("known"));
        let first = breakpoints[0].as_object_mut().expect("an object");
        first.remove("width");
        first.remove("height");
        if let Some(device) = device {
            first.insert("device".to_owned(), json!(device));
        }
        plan(
            &registry,
            &docs,
            "breakpoints",
            &breakpoints,
            Scope::Project,
            Caller::Shell,
        )
    };
    let issues = with(None).expect_err("refused");
    assert_eq!(
        reasons(&issues),
        [
            ("/0/width", "config.error.size_required"),
            ("/0/height", "config.error.size_required")
        ]
    );
    assert!(
        with(Some("iphone-15")).is_ok(),
        "the preset supplies the size"
    );
    let issues = with(Some("no-such-phone")).expect_err("refused");
    assert_eq!(
        reasons(&issues),
        [("/0/device", "config.error.unknown_device")]
    );
}

#[test]
fn unknown_keys_wrong_types_scopes_and_the_bindings_are_refused() {
    let registry = registry();
    let docs = documents(project_file(), None);
    let refuse = |key: &str, value: Value, scope: Scope| {
        plan(&registry, &docs, key, &value, scope, Caller::Shell).expect_err(key)
    };
    assert_eq!(
        reasons(&refuse("no.such", json!(1), Scope::User)),
        [("", reason::UNKNOWN_KEY)]
    );
    assert_eq!(
        reasons(&refuse("sync.scroll", json!("yes"), Scope::Project)),
        [("", reason::INVALID)]
    );
    assert_eq!(
        reasons(&refuse("ui.theme", json!("dark"), Scope::Project)),
        [("", reason::SCOPE)]
    );
    assert_eq!(
        reasons(&refuse("shortcuts.bindings", json!({}), Scope::User)),
        [("", reason::SHORTCUT_EDITOR)]
    );
    let no_project = Documents {
        project: None,
        ..documents(project_file(), None)
    };
    assert_eq!(
        reasons(
            &plan(
                &registry,
                &no_project,
                "sync.scroll",
                &json!(false),
                Scope::Project,
                Caller::Shell
            )
            .expect_err("no project")
        ),
        [("", reason::NO_PROJECT)]
    );
}

#[test]
fn a_protected_key_is_refused_for_mcp_but_not_for_the_window() {
    let registry = registry();
    let docs = documents(project_file(), None);
    let issues = plan(
        &registry,
        &docs,
        "mcp.allowEvaluate",
        &json!(true),
        Scope::User,
        Caller::Mcp,
    )
    .expect_err("protected");
    assert_eq!(reasons(&issues), [("", reason::PROTECTED)]);
    let Ok(Plan::User { document, settings }) = plan(
        &registry,
        &docs,
        "mcp.allowEvaluate",
        &json!(true),
        Scope::User,
        Caller::Shell,
    ) else {
        panic!("the person may");
    };
    assert!(settings.mcp.allow_evaluate);
    assert_eq!(
        document,
        json!({ "ui": { "theme": "dark" }, "mcp": { "allowEvaluate": true } })
    );
}

/// `settings_set` refuses what `config_set` refuses (batch-3 security review, finding 1): the
/// keys workspace trust guards, request headers, the extensions, the screenshot folder and the
/// recent projects. The settings window still writes every one of them.
#[test]
fn trust_relevant_keys_are_refused_for_mcp_and_written_by_the_window() {
    let registry = registry();
    let docs = documents(project_file(), None);
    let breakpoints = docs.effective(registry.get("breakpoints").expect("known"));
    let cases = [
        ("network.proxy", json!("127.0.0.1:8080"), Scope::Local),
        ("network.ignoreCertificateErrors", json!(true), Scope::Local),
        (
            "network.hostResolverRules",
            json!("MAP site.test 127.0.0.1"),
            Scope::Project,
        ),
        (
            "emulation.headers",
            json!({ "X-Test": "1" }),
            Scope::Project,
        ),
        ("breakpoints", breakpoints, Scope::Project),
        ("extensions", json!([]), Scope::Project),
        ("capture.dir", json!("shots"), Scope::Project),
        ("recentProjects", json!(["/work/site"]), Scope::User),
    ];
    for (key, value, scope) in cases {
        let issues =
            plan(&registry, &docs, key, &value, scope, Caller::Mcp).expect_err("refused for MCP");
        assert_eq!(reasons(&issues), [("", reason::PROTECTED)], "{key}");
        if let Err(issues) = plan(&registry, &docs, key, &value, scope, Caller::Shell) {
            panic!("the window may write {key}: {issues:?}");
        }
    }
}

/// The refusal list of `config_set` holds for `settings_set` too, `x-protected` or not, so the
/// two cannot drift: a key below a refused root, or a value that sets request headers.
#[test]
fn the_config_set_refusal_list_holds_for_mcp_settings_set() {
    let registry = registry();
    let docs = documents(project_file(), None);
    let issues = plan(
        &registry,
        &docs,
        "plugins.crashLimit",
        &json!(5),
        Scope::User,
        Caller::Mcp,
    )
    .expect_err("below the refused root `plugins`");
    assert_eq!(reasons(&issues), [("", reason::PROTECTED)]);
    assert!(
        plan(
            &registry,
            &docs,
            "plugins.crashLimit",
            &json!(5),
            Scope::User,
            Caller::Shell
        )
        .is_ok()
    );
    // Every protected project key is refused by `config_set` as well; the breakpoints alone
    // are edited by the breakpoint tools, which refuse their headers.
    for def in registry.iter().filter(|def| {
        def.protected_from_mcp
            && def.scope_allowed.contains(&Scope::Project)
            && def.key != "breakpoints"
    }) {
        assert!(
            rb_mcp::config_edit::refusal(&def.key).is_some(),
            "{} is protected from settings_set but open to config_set",
            def.key
        );
    }
}

#[test]
fn a_user_reset_restores_the_default_and_leaves_the_file() {
    let registry = registry();
    let mut docs = documents(project_file(), None);
    docs.user["ui"]["theme"] = json!("dark");
    let Ok(Plan::User { document, settings }) = plan(
        &registry,
        &docs,
        "ui.theme",
        &Value::Null,
        Scope::User,
        Caller::Shell,
    ) else {
        panic!("planned");
    };
    assert_eq!(document, json!({}));
    assert_eq!(settings.ui.theme, rb_config::Theme::default());
}

/// RB-090-02: without the feature `self-update`, `setSetting` (and later MCP's `settings_set`) on
/// an update key is refused; the CEF notice stays writable.
#[test]
fn an_update_key_is_refused_while_self_update_is_not_built() {
    let registry = registry();
    let docs = documents(project_file(), None);
    for key in crate::updates::LOCKED_KEYS {
        let value = if *key == "updates.enabled" {
            json!(true)
        } else {
            registry.get(key).expect("a key").default_value()
        };
        let result = plan(&registry, &docs, key, &value, Scope::User, Caller::Shell);
        if crate::updates::SELF_UPDATE_BUILT {
            assert!(result.is_ok(), "{key}");
        } else {
            let issues = result.expect_err("locked");
            assert_eq!(reasons(&issues), [("", reason::UPDATES_INACTIVE)], "{key}");
        }
    }
    assert!(
        plan(
            &registry,
            &docs,
            "updates.cefAgeWarning",
            &json!(false),
            Scope::User,
            Caller::Shell
        )
        .is_ok()
    );
}
