use std::sync::atomic::AtomicU8;

use rb_commands::{Binding, Group, Key, McpCoverage};
use serde_json::json;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

fn core() -> Arc<Core> {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    Core::new(host, ui, Handle::current(), CoreSetup::default())
}

/// The settings the registered sidecar holds and its request timeout.
fn in_effect() -> Option<(SidecarSettings, Duration)> {
    let guard = lock(&SIDECAR);
    let running = guard.as_ref()?;
    Some((
        running.sidecar.launcher().settings(),
        running.sidecar.rpc_timeout(),
    ))
}

#[test]
fn the_settings_come_from_the_engines_section() {
    let mut user = UserSettings::default();
    user.engines.node_path = Some("C:\\node\\node.exe".to_owned());
    user.engines.node_rpc_timeout_ms = 5_000;
    user.engines.playwright_browsers_path = Some("D:\\pw".to_owned());
    let settings = settings_of(&user);
    assert_eq!(settings.node_path.as_deref(), Some("C:\\node\\node.exe"));
    assert_eq!(settings.rpc_timeout_ms, 5_000);
    assert_eq!(settings.browsers_path.as_deref(), Some("D:\\pw"));
}

#[test]
fn the_default_timeout_is_the_sidecars() {
    let settings = settings_of(&UserSettings::default());
    assert_eq!(
        settings.rpc_timeout_ms,
        rb_sidecar::settings::DEFAULT_RPC_TIMEOUT_MS
    );
    assert_eq!(settings.node_path, None);
    assert_eq!(settings.browsers_path, None);
}

#[test]
fn the_restart_is_leader_r_s_and_the_update_leader_v_u_both_without_mcp() {
    let defs = definitions().expect("valid");
    let ids: Vec<&str> = defs.iter().map(|def| def.id.as_str()).collect();
    assert_eq!(ids, ["sidecar.restart", "engines.update"]);
    let chords: Vec<Vec<Key>> = defs
        .iter()
        .map(|def| match def.default_bindings.as_slice() {
            [Binding::Leader(chord)] => chord.keys.clone(),
            other => panic!("one leader chord expected, got {other:?}"),
        })
        .collect();
    assert_eq!(
        chords,
        [
            vec![Key::Char('R'), Key::Char('S')],
            vec![Key::Char('V'), Key::Char('U')]
        ]
    );
    assert_eq!(defs[0].group, Group::Reload);
    assert!(
        defs.iter()
            .all(|def| matches!(def.mcp, McpCoverage::Omitted(_)))
    );
}

#[test]
fn the_versions_answer_fills_node_and_playwright() {
    let mut versions = rb_diag::Versions::default();
    apply_versions(
        &mut versions,
        &json!({ "node": "26.13.0", "playwright": "1.58.0", "browsers": [] }),
    );
    assert_eq!(versions.node.as_deref(), Some("26.13.0"));
    assert_eq!(versions.playwright.as_deref(), Some("1.58.0"));
}

#[test]
fn a_partial_versions_answer_keeps_what_it_does_not_name() {
    let mut versions = rb_diag::Versions {
        playwright: Some("old".to_owned()),
        ..rb_diag::Versions::default()
    };
    apply_versions(
        &mut versions,
        &json!({ "node": "26.13.0", "playwright": 7 }),
    );
    assert_eq!(versions.node.as_deref(), Some("26.13.0"));
    assert_eq!(versions.playwright.as_deref(), Some("old"));
}

#[test]
fn progress_shows_in_steps_of_a_quarter() {
    let last = AtomicU8::new(u8::MAX);
    let shown: Vec<u8> = [0, 3, 10, 25, 26, 49, 51, 60, 80, 99, 100, 100]
        .into_iter()
        .filter(|percent| next_step(&last, *percent))
        .collect();
    assert_eq!(shown, [0, 25, 51, 80, 100]);
}

#[tokio::test]
async fn changed_user_settings_reach_the_sidecar_for_its_next_start() {
    let core = core();
    let (settings, timeout) = in_effect().expect("registered without a browsers folder");
    assert_eq!(settings, settings_of(&UserSettings::default()));
    assert_eq!(timeout, settings.rpc_timeout());
    let mut user = core.user_settings();
    user.engines.node_path = Some("C:\\node\\node.exe".to_owned());
    user.engines.node_rpc_timeout_ms = 5_000;
    user.engines.playwright_browsers_path = Some("D:\\pw".to_owned());
    core.replace_user_settings(user.clone());
    let (settings, timeout) = in_effect().expect("registered");
    assert_eq!(settings, settings_of(&user));
    assert_eq!(timeout, Duration::from_secs(5));
}

#[test]
fn every_update_run_shows_its_progress_from_the_start() {
    let last = Arc::new(AtomicU8::new(u8::MAX));
    let ok: Handler = Arc::new(|_| Ok(Value::Null));
    let handler = fresh_progress(Arc::clone(&last), ok);
    handler(Value::Null).expect("first run");
    assert!(next_step(&last, 0));
    assert!(next_step(&last, 100));
    handler(Value::Null).expect("second run");
    assert!(next_step(&last, 0), "the second run shows its first step");
}
