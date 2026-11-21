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
