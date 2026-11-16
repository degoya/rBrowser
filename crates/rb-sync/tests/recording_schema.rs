//! `schema/recording.v1.json` is generated from `rb_sync::recorder::Recording` (RB-050-01); this
//! test fails when it is stale. Regenerate with
//! `RB_UPDATE_SCHEMA=1 cargo nextest run -p rb-sync --test recording_schema`.

use std::fs;
use std::path::Path;

const FILE: &str = rb_sync::recorder::RECORDING_SCHEMA_FILE;

#[test]
fn the_checked_in_recording_schema_is_current() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(FILE);
    let text = rb_sync::recorder::recording_schema();
    if std::env::var_os("RB_UPDATE_SCHEMA").is_some_and(|value| value == "1") {
        fs::write(&path, &text).expect("write the schema");
    }
    assert_eq!(
        fs::read_to_string(&path).ok().as_deref(),
        Some(text.as_str()),
        "stale {FILE}; regenerate with \
         RB_UPDATE_SCHEMA=1 cargo nextest run -p rb-sync --test recording_schema"
    );
}

#[test]
fn the_schema_names_every_step_type_and_the_version() {
    let text = rb_sync::recorder::recording_schema();
    for needle in [
        "\"navigate\"",
        "\"click\"",
        "\"dblclick\"",
        "\"fill\"",
        "\"press\"",
        "\"select\"",
        "\"check\"",
        "\"uncheck\"",
        "\"hover\"",
        "\"scroll\"",
        "\"waitForUrl\"",
        "\"assert\"",
        "\"onlyIn\"",
        "\"version\"",
    ] {
        assert!(text.contains(needle), "{needle} is not in the schema");
    }
}
