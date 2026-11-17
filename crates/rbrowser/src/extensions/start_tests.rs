use std::fs;
use std::path::Path;

use rb_config::{ChromiumExtensionSettings, UnpackedExtension};

use super::*;

const HELLO: &str = r#"{ "manifest_version": 3, "name": "Hello", "version": "1.0" }"#;

fn extension(dir: &Path, name: &str) -> String {
    let folder = dir.join(name);
    fs::create_dir_all(&folder).expect("folder");
    fs::write(folder.join("manifest.json"), HELLO).expect("manifest");
    folder.to_string_lossy().into_owned()
}

fn entry(path: &str, enabled: bool) -> UnpackedExtension {
    UnpackedExtension {
        path: path.to_owned(),
        enabled,
    }
}

fn settings(entries: Vec<UnpackedExtension>) -> ChromiumExtensionSettings {
    ChromiumExtensionSettings {
        unpacked: entries,
        ..ChromiumExtensionSettings::default()
    }
}

#[test]
fn the_valid_switched_on_folders_load_and_the_rest_is_named() {
    let dir = tempfile::tempdir().expect("dir");
    let a = extension(dir.path(), "a");
    let b = extension(dir.path(), "b");
    let off = extension(dir.path(), "off");
    let gone = dir.path().join("gone").to_string_lossy().into_owned();
    let plan = plan(&settings(vec![
        entry(&a, true),
        entry(&off, false),
        entry(&gone, true),
        entry(&b, true),
    ]));
    assert_eq!(plan.loaded_paths(), [a.as_str(), b.as_str()]);
    assert_eq!(plan.skipped.len(), 1);
    assert_eq!(plan.skipped[0].path, gone);
    assert_eq!(plan.skipped[0].error, FolderError::Missing);
}

#[test]
fn a_folder_named_twice_loads_once() {
    let dir = tempfile::tempdir().expect("dir");
    let a = extension(dir.path(), "a");
    let plan = plan(&settings(vec![
        entry(&a, true),
        entry(&a.to_uppercase(), true),
        entry(&a, true),
    ]));
    assert_eq!(plan.loaded.len(), 1, "{plan:?}");
}

#[test]
fn the_switch_names_every_loaded_folder_and_nothing_loads_none() {
    let dir = tempfile::tempdir().expect("dir");
    let a = extension(dir.path(), "a");
    let b = extension(dir.path(), "b");
    let both = plan(&settings(vec![entry(&a, true), entry(&b, true)]));
    assert_eq!(
        switches(&both),
        [(LOAD_SWITCH.to_owned(), Some(format!("{a},{b}")))]
    );
    assert!(switches(&plan(&settings(Vec::new()))).is_empty());
}

#[test]
fn switched_off_extensions_load_nothing_and_check_no_folder() {
    let mut off = settings(vec![entry("/nowhere", true)]);
    off.enabled = false;
    let plan = plan(&off);
    assert!(plan.loaded.is_empty());
    assert!(plan.skipped.is_empty(), "nothing is checked while off");
    assert!(switches(&plan).is_empty());
}

#[test]
fn a_restart_is_needed_when_the_folders_to_load_differ_from_the_started_ones() {
    let dir = tempfile::tempdir().expect("dir");
    let a = extension(dir.path(), "a");
    let b = extension(dir.path(), "b");
    let started = plan(&settings(vec![entry(&a, true)]));
    assert!(!needs_restart(
        &started,
        &plan(&settings(vec![entry(&a, true)]))
    ));
    assert!(needs_restart(
        &started,
        &plan(&settings(vec![entry(&a, true), entry(&b, true)]))
    ));
    assert!(needs_restart(&started, &plan(&settings(vec![]))));
}

/// RB-080-23: an installed Web Store extension loads after the person's folders; none while
/// the extensions are off.
#[test]
fn the_installed_web_store_extensions_load_too() {
    let dir = tempfile::tempdir().expect("dir");
    let a = extension(dir.path(), "a");
    let store = super::super::store::ExtensionStore::new(dir.path().join("store"));
    let id = "b".repeat(32);
    let folder = store.root().join(&id).join("1.0");
    fs::create_dir_all(&folder).expect("folder");
    fs::write(folder.join("manifest.json"), HELLO).expect("manifest");
    fs::write(
        store.root().join(&id).join(super::super::store::POINTER),
        r#"{"version":"1.0"}"#,
    )
    .expect("pointer");
    let plan = plan_with_store(&settings(vec![entry(&a, true)]), Some(&store));
    assert_eq!(plan.loaded.len(), 2);
    assert_eq!(plan.loaded[1].folder, folder);
    assert_eq!(plan.web_store.len(), 1);
    assert_eq!(plan.web_store[0].id, id);
    assert!(
        switches(&plan)[0]
            .1
            .as_deref()
            .expect("paths")
            .contains("store")
    );
    let off = ChromiumExtensionSettings {
        enabled: false,
        ..settings(vec![entry(&a, true)])
    };
    let plan = plan_with_store(&off, Some(&store));
    assert!(plan.loaded.is_empty());
    assert!(plan.web_store.is_empty());
    assert!(needs_restart(
        &plan,
        &plan_with_store(&settings(vec![]), Some(&store))
    ));
}
