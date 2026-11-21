use std::sync::Arc;
use std::time::Duration;

use rb_commands::{CommandId, CommandRegistry, Group, McpCoverage};
use rb_engine::ViewId;
use rb_ipc::{CoreToShell, NotificationLevel};
use rb_util::perf::{Registry, Summary, names};
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::{Core, CoreSetup};
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

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn the_report_gives_every_counter_in_milliseconds() {
    let registry = Registry::new();
    for n in 1..=20 {
        registry.record(names::SYNC_REPLAY_CHROMIUM, ms(n));
    }
    registry.record(names::IPC_QUERY, Duration::from_micros(1_250));
    let report = report_of(&registry, ms(9_000));
    assert_eq!(report["enabled"], json!(true));
    assert_eq!(report["uptimeMs"], json!(9000.0));
    assert_eq!(report["window"], json!(rb_util::perf::WINDOW));
    let counters = report["counters"].as_array().expect("counters");
    assert_eq!(counters.len(), 2);
    assert_eq!(counters[0]["name"], json!("ipc.query"));
    assert_eq!(counters[0]["p95Ms"], json!(1.25));
    let sync = &counters[1];
    assert_eq!(sync["name"], json!("sync.replay.chromium"));
    assert_eq!(sync["count"], json!(20));
    assert_eq!(sync["window"], json!(20));
    assert_eq!(sync["p50Ms"], json!(10.0));
    assert_eq!(sync["p95Ms"], json!(19.0));
    assert_eq!(sync["maxMs"], json!(20.0));
    assert_eq!(sync["meanMs"], json!(10.5));
}

#[test]
fn a_switched_off_registry_says_so() {
    let registry = Registry::new();
    registry.set_enabled(false);
    let report = report_of(&registry, ms(1));
    assert_eq!(report["enabled"], json!(false));
    assert_eq!(report["counters"], json!([]));
}

fn summary(name: &'static str, p95: u64) -> Summary {
    Summary {
        name,
        count: 1,
        window: 1,
        p50: ms(p95),
        p95: ms(p95),
        max: ms(p95),
        mean: ms(p95),
    }
}

#[test]
fn the_notice_names_the_p95_of_sync_ipc_plugins_and_the_first_view() {
    let CoreToShell::Notification(notice) = notification(&[
        summary(names::SYNC_REPLAY_CHROMIUM, 42),
        summary(names::IPC_QUERY, 3),
        summary(names::STARTUP_FIRST_VIEW, 1_800),
    ]) else {
        panic!("a notification");
    };
    assert_eq!(notice.level, NotificationLevel::Info);
    assert_eq!(notice.message_key, SUMMARY_KEY);
    assert_eq!(notice.args["sync"], "42.0");
    assert_eq!(notice.args["ipc"], "3.0");
    assert_eq!(notice.args["plugin"], "-", "no plugin call yet");
    assert_eq!(notice.args["firstView"], "1800.0");
    assert_eq!(notice.settings_key.as_deref(), Some(COUNTERS_SETTING));
}

#[test]
fn the_command_is_leader_d_b_covered_by_app_perf() {
    let defs = definitions().expect("definitions");
    assert_eq!(defs.len(), 1);
    let def = &defs[0];
    assert_eq!(def.id.as_str(), "debug.perf_counters");
    assert_eq!(def.title_key, "commands.debug.perf_counters");
    assert_eq!(def.group, Group::Devtools);
    let chords: Vec<String> = def
        .default_bindings
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(chords, vec!["Ctrl+Shift+Space D B".to_owned()]);
    assert_eq!(def.mcp, McpCoverage::Covered("app_perf".to_owned()));
}

#[test]
fn the_chord_passes_the_lint_with_every_core_command() {
    use rb_commands::lint::lint_defaults;
    use rb_commands::{Platform, builtin, builtin_reserved};
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.push(rb_diag::command_def().expect("diagnostics.create"));
    defs.extend(rb_plugin_host::core_command_defs().expect("plugins.*"));
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[tokio::test]
async fn the_command_answers_the_report_and_resets_when_asked() {
    let core = core();
    let mut registry = CommandRegistry::new();
    assert!(register(&mut registry, &core).is_empty());
    rb_util::perf::record(names::IPC_QUERY, ms(2));
    let id = CommandId::new(PERF_COUNTERS).expect("id");
    let answer = registry.dispatch(&id, json!({})).expect("answers");
    let counters = answer["counters"].as_array().expect("counters");
    assert!(
        counters
            .iter()
            .any(|counter| counter["name"] == "ipc.query")
    );
    let answer = registry
        .dispatch(&id, json!({ "reset": true }))
        .expect("answers");
    assert_eq!(answer["reset"], json!(true));
    let refused = registry.dispatch(&id, json!({ "reset": "yes" }));
    assert!(refused.is_err(), "reset is true or false");
}

#[tokio::test]
async fn the_setting_switches_the_recording() {
    let core = core();
    let mut user = core.user_settings();
    user.performance.counters = false;
    core.replace_user_settings(user.clone());
    apply_settings(&core.user_settings().performance);
    assert!(!rb_util::perf::global().enabled());
    user.performance.counters = true;
    core.replace_user_settings(user);
    apply_settings(&core.user_settings().performance);
    assert!(rb_util::perf::global().enabled());
}

#[tokio::test]
async fn the_first_load_end_of_a_view_is_the_start_up_time() {
    let core = core();
    core.host().on_loading_state(ViewId(1), true);
    core.host().on_load_end(ViewId(1), "http://localhost/");
    assert!(
        rb_util::perf::global()
            .snapshot()
            .iter()
            .any(|series| series.name == names::STARTUP_FIRST_VIEW && series.count == 1)
    );
}

/// Review w30: with the counters off at the first load, switching them on later must not make
/// the next load the start-up time.
#[test]
fn a_load_after_the_counters_were_switched_on_is_no_start_up_time() {
    let registry = rb_util::perf::global();
    registry.set_enabled(false);
    first_view_loaded();
    registry.set_enabled(true);
    first_view_loaded();
    assert!(
        registry
            .snapshot()
            .iter()
            .all(|summary| summary.name != names::STARTUP_FIRST_VIEW),
        "no start-up time from a later load"
    );
}
