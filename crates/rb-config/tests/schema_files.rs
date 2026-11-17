//! The checked-in schemas (`schema/config.v1.json`, `schema/settings.v1.json`) are generated
//! from the types; this test fails when one is stale. Regenerate with
//! `RB_UPDATE_SCHEMA=1 cargo nextest run -p rb-config --test schema_files`.

use std::fs;
use std::path::Path;

#[test]
fn the_checked_in_schemas_are_current() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let update = std::env::var_os("RB_UPDATE_SCHEMA").is_some_and(|value| value == "1");
    let mut stale = Vec::new();
    for (relative, text) in rb_config::schema_files() {
        let path = root.join(relative);
        if update {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir).expect("create schema/");
            }
            fs::write(&path, &text).expect("write the schema");
        } else if fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
            stale.push(relative);
        }
    }
    assert!(
        stale.is_empty(),
        "stale schema files {stale:?}; regenerate with \
         RB_UPDATE_SCHEMA=1 cargo nextest run -p rb-config --test schema_files"
    );
}
