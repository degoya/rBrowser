use serde_json::json;

use super::*;

fn values(value: serde_json::Value) -> serde_json::Map<String, Value> {
    value.as_object().cloned().expect("an object")
}

#[test]
fn the_settings_hold_to_their_ranges() {
    assert_eq!(
        settings_of(&serde_json::Map::new()),
        Settings {
            history_size: DEFAULT_HISTORY_SIZE,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            bench_repeats: super::super::bench::DEFAULT_REPEATS,
            bench_concurrency: 1,
            code: super::super::export::CodeSettings::default(),
        }
    );
    let set = settings_of(&values(json!({
        "api-client.historySize": 100_000,
        "api-client.maxResponseBytes": 1u64 << 40
    })));
    assert_eq!(set.history_size, super::super::history::MAX_ENTRIES);
    assert_eq!(set.max_response_bytes, MAX_RESPONSE_BYTES);
    let small = settings_of(&values(json!({
        "api-client.historySize": 0,
        "api-client.maxResponseBytes": 1
    })));
    assert_eq!(small.history_size, 0);
    assert_eq!(small.max_response_bytes, 1024);
}

#[test]
fn the_benchmark_settings_stay_small() {
    let set = settings_of(&values(json!({
        "api-client.benchRepeats": 1_000_000,
        "api-client.benchConcurrency": 64
    })));
    assert_eq!(set.bench_repeats, super::super::bench::MAX_REPEATS);
    assert_eq!(set.bench_concurrency, super::super::bench::MAX_CONCURRENCY);
    let low = settings_of(&values(json!({
        "api-client.benchRepeats": 0,
        "api-client.benchConcurrency": 0
    })));
    assert_eq!((low.bench_repeats, low.bench_concurrency), (1, 1));
}

/// RB-096-13: `secrets_not_saved` and `grant_not_saved` carry the core's reason - the text of the
/// refusal and its key - where 0.9.6 sent the code alone. The owner's refusal was
/// `settings.rejected.unknown_key`: the registry the write planned over held no plugin key.
#[test]
fn a_refused_write_names_the_core_s_reason() {
    let refused = rb_ipc::CoreToShell::SettingRejected {
        key: SECRETS_KEY.to_owned(),
        issues: vec![rb_ipc::SettingIssue {
            path: String::new(),
            message_key: "settings.rejected.unknown_key".to_owned(),
        }],
    };
    let failure = refusal("secrets_not_saved", &refused).expect("a refusal");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.secrets_not_saved")
    );
    assert_eq!(
        failure.params.get("issue").map(String::as_str),
        Some("settings.rejected.unknown_key")
    );
    let reason = failure.params.get("reason").expect("a reason");
    assert!(
        !reason.is_empty() && !reason.starts_with("settings."),
        "{reason}"
    );
    assert!(refusal("grant_not_saved", &rb_ipc::CoreToShell::Ack).is_none());
}
