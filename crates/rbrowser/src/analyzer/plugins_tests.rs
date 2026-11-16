use rb_config::UserSettings;
use rb_plugin_host::{LocaleSink, SettingsSink};
use serde_json::json;

use super::{I18nLocales, NoRuntime, SchemaSink, block_on, load};

#[test]
fn locales_reach_the_catalogue_and_leave_it_again() {
    let file = br#"{ "plugin": { "sink-test": { "name": "Sink test" } } }"#;
    I18nLocales
        .register("sink-test", "en", file)
        .expect("registered");
    assert_eq!(
        rb_i18n::plugin_texts("sink-test")
            .get("plugin.sink-test.name")
            .map(String::as_str),
        Some("Sink test")
    );
    I18nLocales.unregister("sink-test");
    assert!(rb_i18n::plugin_texts("sink-test").is_empty());
    assert!(I18nLocales.register("sink-test", "english", file).is_err());
}

#[test]
fn a_schema_without_an_existing_category_is_refused() {
    let sink = SchemaSink::new();
    let refused = json!({
        "type": "object",
        "properties": { "depth": {
            "type": "integer", "default": 1,
            "x-category": "nowhere", "x-section": "x", "x-order": 1, "x-scope": ["user"]
        } }
    });
    assert!(sink.register_plugin_schema("sink-test", &refused).is_err());
    assert!(sink.into_schemas().is_empty());
}

#[test]
fn an_empty_or_foreign_folder_loads_nothing() {
    // The load runs on the core's runtime from a blocking thread; the test enters one of its own.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let _runtime = runtime.enter();
    let dir = tempfile::tempdir().expect("temp dir");
    let user = UserSettings::default();
    let none = load(dir.path(), &user);
    assert!(none.report.loaded.is_empty() && none.report.refused.is_empty());

    std::fs::write(dir.path().join("breakpoint-analyzer.rbplugin"), b"no zip").expect("written");
    let refused = load(dir.path(), &user);
    assert!(refused.report.loaded.is_empty());
    let error = refused
        .call_tool("breakpoint-analyzer", "detect", &json!({}))
        .expect_err("not loaded");
    assert_eq!(error.code, "plugins.error.invalid");
}

#[test]
fn only_the_first_caller_starts_the_load() {
    let flag = std::sync::atomic::AtomicBool::new(false);
    assert!(super::claim(&flag));
    assert!(
        !super::claim(&flag),
        "a second request starts no second thread"
    );
}

#[test]
fn the_sink_counts_the_keys_a_plugin_registered() {
    let sink = SchemaSink::new();
    let schema = json!({
        "type": "object",
        "properties": { "group": { "type": "object", "properties": {
            "a": { "type": "integer", "default": 1, "x-category": "checks", "x-section": "seo",
                   "x-order": 1, "x-scope": ["user"] },
            "b": { "type": "integer", "default": 1, "x-category": "checks", "x-section": "seo",
                   "x-order": 2, "x-scope": ["user"] }
        } } }
    });
    let count = sink
        .register_plugin_schema("sink-count", &schema)
        .expect("registered");
    assert_eq!(count, 2, "the keys, not the top-level properties");
    assert_eq!(sink.counts().get("sink-count"), Some(&2));
}

#[test]
fn without_a_runtime_block_on_is_an_error_not_a_wait() {
    // A plain test thread has no runtime: the UI thread's case.
    assert_eq!(block_on(async { 1 }), Err(NoRuntime));
    let error = super::PluginError::from(NoRuntime);
    assert_eq!(error.code, rb_plugin_types::codes::HOST_FAILED);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_blocking_thread_of_the_runtime_and_a_thread_that_entered_it_run_the_future() {
    let on_blocking = tokio::task::spawn_blocking(|| block_on(async { 2 }))
        .await
        .expect("joined");
    assert_eq!(on_blocking, Ok(2));
    let handle = tokio::runtime::Handle::current();
    let on_thread = std::thread::spawn(move || {
        let _runtime = handle.enter();
        block_on(async { 3 })
    })
    .join()
    .expect("joined");
    assert_eq!(on_thread, Ok(3));
}
