use std::sync::Arc;

use rb_mcp::{Backend as _, LogStore, SettingScope};
use serde_json::json;

use super::{Change, REFUSED, changes, is_shortcut_key};
use crate::mcp::backend::CoreBackend;
use crate::mcp::tests::core;

#[test]
fn the_bindings_key_and_one_command_below_it_are_shortcut_keys() {
    assert!(is_shortcut_key("shortcuts.bindings"));
    assert!(is_shortcut_key("shortcuts.bindings.view.reload"));
    assert!(!is_shortcut_key("shortcuts.bindings."));
    assert!(!is_shortcut_key("shortcuts.bindingsX"));
    assert!(!is_shortcut_key("shortcuts.leaderTimeoutMs"));
}

#[test]
fn a_list_sets_null_resets_and_the_object_names_several() {
    assert_eq!(
        changes(
            "shortcuts.bindings.view.reload",
            &json!(["Ctrl+Shift+Space R R"])
        ),
        Ok(vec![Change::Set(
            "view.reload".to_owned(),
            vec!["Ctrl+Shift+Space R R".to_owned()]
        )])
    );
    assert_eq!(
        changes("shortcuts.bindings.view.reload", &json!(null)),
        Ok(vec![Change::Reset(Some("view.reload".to_owned()))])
    );
    assert_eq!(
        changes("shortcuts.bindings", &json!(null)),
        Ok(vec![Change::Reset(None)])
    );
    assert_eq!(
        changes("shortcuts.bindings", &json!({ "a.b": [], "c.d": null })),
        Ok(vec![
            Change::Set("a.b".to_owned(), Vec::new()),
            Change::Reset(Some("c.d".to_owned())),
        ])
    );
    for wrong in [json!("Ctrl+K"), json!([1])] {
        let failure = changes("shortcuts.bindings.a.b", &wrong).expect_err("not a list of texts");
        assert_eq!(failure.code, "settings.rejected.invalid");
    }
    assert!(changes("shortcuts.bindings", &json!([])).is_err());
}

/// `Ctrl+T` is the browser's new tab: refused as reserved, with the reason; nothing changes.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reserved_shortcut_is_refused_with_its_reason() {
    let core = core();
    let backend = CoreBackend::new(&core, Arc::new(LogStore::default()));
    let before = core.user_settings().shortcuts.bindings;
    let failure = backend
        .set_setting(
            "shortcuts.bindings.mcp.copy_config",
            &json!(["Ctrl+T"]),
            SettingScope::User,
        )
        .expect_err("reserved");
    assert_eq!(failure.code, REFUSED);
    assert!(failure.message.contains("Ctrl+T"), "{}", failure.message);
    assert!(
        failure.message.to_lowercase().contains("new tab"),
        "{}",
        failure.message
    );
    assert_eq!(core.user_settings().shortcuts.bindings, before);
}

/// A free binding goes through the editor's path: the registry and the settings follow; null
/// restores the default. Another scope than user is refused.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_free_shortcut_is_set_and_reset() {
    let core = core();
    let backend = CoreBackend::new(&core, Arc::new(LogStore::default()));
    let key = "shortcuts.bindings.mcp.copy_config";
    backend
        .set_setting(key, &json!(["Ctrl+Shift+Space , Q"]), SettingScope::User)
        .expect("a free chord");
    assert_eq!(
        core.user_settings()
            .shortcuts
            .bindings
            .get("mcp.copy_config")
            .cloned(),
        Some(vec!["Ctrl+Shift+Space , Q".to_owned()])
    );
    backend
        .set_setting(key, &json!(null), SettingScope::User)
        .expect("reset");
    assert!(
        !core
            .user_settings()
            .shortcuts
            .bindings
            .contains_key("mcp.copy_config")
    );
    let failure = backend
        .set_setting(key, &json!([]), SettingScope::Project)
        .expect_err("not a project's");
    assert_eq!(failure.code, "settings.rejected.scope");
}

/// A command MCP may not run - granting or revoking trust, switching MCP - is not
/// MCP's to rebind either: a free key for `trust.grant` would be one keystroke from trust.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_protected_command_cannot_be_rebound_over_mcp() {
    let core = core();
    let backend = CoreBackend::new(&core, Arc::new(LogStore::default()));
    let rb_ipc::CoreToShell::Commands { commands, .. } = core.commands() else {
        panic!("the commands answer");
    };
    for id in ["trust.grant", "trust.revoke", "mcp.toggle"] {
        assert!(
            commands.iter().any(|info| info.id == id),
            "{id} is registered"
        );
        let failure = backend
            .set_setting(
                &format!("shortcuts.bindings.{id}"),
                &json!(["Ctrl+Shift+Space , Q"]),
                SettingScope::User,
            )
            .expect_err("protected");
        assert_eq!(failure.code, "settings.rejected.protected", "{id}");
    }
    assert!(core.user_settings().shortcuts.bindings.is_empty());
}

/// F3 (RB-030-03): a batch is applied whole or not at all - two commands asking for the same
/// free chord both pass the check against the registry before the batch, the second is refused
/// when it is applied, and the first must not stay bound or be written.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_batch_that_fails_halfway_changes_nothing() {
    let core = core();
    let backend = CoreBackend::new(&core, Arc::new(LogStore::default()));
    let before = core.commands();
    let failure = backend
        .set_setting(
            "shortcuts.bindings",
            &json!({
                "mcp.copy_config": ["Ctrl+Shift+Space , Q"],
                "sync.resync": ["Ctrl+Shift+Space , Q"],
            }),
            SettingScope::User,
        )
        .expect_err("the second takes the first's chord");
    assert_eq!(failure.code, "settings.rejected.invalid", "{failure:?}");
    assert!(core.user_settings().shortcuts.bindings.is_empty());
    assert_eq!(core.commands(), before, "the registry is as it was");
}
