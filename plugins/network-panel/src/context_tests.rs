use serde_json::json;

use super::*;
use crate::fake_host::FakeHost;

#[test]
fn the_hosts_state_reads() {
    let args = json!({ "$network": {
        "session": "s1", "trusted": true, "hits": { "api.json#user": 3 },
        "rejected": [{ "id": "api.json#bad", "reason": "bad regex" }], "applied": 2,
        "cacheDisabled": true,
        "views": [{ "view": 1, "supported": true, "intercepting": true },
                  { "view": 2, "supported": false, "intercepting": false }],
    } });
    let context = Context::read(&args);
    assert!(context.present && context.trusted && context.cache_disabled);
    assert_eq!(context.session, "s1");
    assert_eq!(context.hits["api.json#user"], 3);
    assert_eq!(
        context.rejected,
        [("api.json#bad".to_owned(), "bad regex".to_owned())]
    );
    assert_eq!(context.applied, 2);
    assert_eq!(
        context.views[1],
        ViewState {
            view: 2,
            supported: false,
            intercepting: false
        }
    );
}

#[test]
fn without_a_state_nothing_is_trusted() {
    let context = Context::read(&json!({ "$network": "forged" }));
    assert!(!context.present && !context.trusted);
    assert!(!Context::read(&json!({})).trusted);
}

#[test]
fn settings_read_with_their_defaults_and_bounds() {
    let mut host = FakeHost::default();
    assert_eq!(Settings::read(&mut host), Settings::default());
    host.set("mocksEnabled", "false");
    host.set("mocksDir", "\".rbrowser/api-mocks/\"");
    host.set("block", r#"["*/tracking.js", "", 3]"#);
    host.set("cacheDisabled", "true");
    host.set("harBodies", "false");
    let settings = Settings::read(&mut host);
    assert!(!settings.mocks_enabled && settings.cache_disabled && !settings.har_bodies);
    assert_eq!(settings.mocks_dir, ".rbrowser/api-mocks");
    assert_eq!(settings.block, ["*/tracking.js"]);
}

#[test]
fn a_mocks_dir_outside_the_rbrowser_folder_falls_back_to_the_default() {
    for dir in [
        "src/mocks",
        ".rbrowser/../secrets",
        "/etc",
        ".rbrowser/",
        ".rbrowser//x",
    ] {
        let mut host = FakeHost::default();
        host.set("mocksDir", &json!(dir).to_string());
        assert_eq!(
            Settings::read(&mut host).mocks_dir,
            DEFAULT_MOCKS_DIR,
            "{dir}"
        );
    }
}

#[test]
fn the_debugger_parts_of_the_state_and_the_settings_read() {
    let args = json!({ "$network": {
        "session": "s1", "trusted": true,
        "held": [{ "id": "h1", "url": "https://a.test/?sid=***" }],
        "answers": { "a1": { "done": true } },
        "composed": [{ "status": 200 }],
        "breakpoints": { "enabled": false, "active": 2 },
        "skipped": { "api.json#big": 3 },
    } });
    let context = Context::read(&args);
    assert_eq!(context.held.len(), 1);
    assert_eq!(context.answers["a1"]["done"], true);
    assert_eq!(context.composed.len(), 1);
    assert!(!context.breakpoints_enabled);
    assert_eq!(context.breakpoints_active, 2);
    assert_eq!(context.skipped["api.json#big"], 3);
    let mut host = FakeHost::default();
    host.set("breakpoints.timeoutMs", "5000");
    host.set("allowComposeFromMcp", "true");
    host.set("mapLocalRoots", r#"["C:\\sites", ""]"#);
    host.set("rewrite.maxBodyBytes", "1024");
    let settings = Settings::read(&mut host);
    assert_eq!(settings.breakpoint_timeout_ms, 5000);
    assert!(settings.allow_compose_from_mcp);
    assert_eq!(settings.map_local_roots, vec!["C:\\sites".to_owned()]);
    assert_eq!(settings.rewrite_max_body_bytes, 1024);
    let defaults = Settings::read(&mut FakeHost::default());
    assert_eq!(
        defaults.breakpoint_timeout_ms,
        DEFAULT_BREAKPOINT_TIMEOUT_MS
    );
    assert_eq!(defaults.rewrite_max_body_bytes, DEFAULT_MAX_BODY_BYTES);
}
