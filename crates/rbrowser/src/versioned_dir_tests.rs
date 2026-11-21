use std::fs;
use std::path::Path;

use super::{POINTER, activate, prune, read_pointer};

/// An unpacked version under `staging`, holding `marker` with `text`.
fn unpacked(staging: &Path, name: &str, text: &str) -> std::path::PathBuf {
    let folder = staging.join(name);
    fs::create_dir_all(&folder).expect("folder");
    fs::write(folder.join("marker"), text).expect("marker");
    folder
}

fn marker(folder: &Path) -> String {
    fs::read_to_string(folder.join("marker")).expect("marker")
}

#[test]
fn an_activated_version_is_in_place_and_named_by_the_pointer() {
    let root = tempfile::tempdir().expect("temp");
    let dir = root.path().join("tool");
    fs::create_dir_all(&dir).expect("dir");
    assert_eq!(read_pointer(&dir), None);
    let from = unpacked(root.path(), "staged", "new");
    let target = activate(&dir, "1.2.0", &from, "1").expect("activated");
    assert_eq!(target, dir.join("1.2.0"));
    assert_eq!(marker(&target), "new");
    assert!(!from.exists());
    assert_eq!(read_pointer(&dir).as_deref(), Some("1.2.0"));
    let names: Vec<_> = fs::read_dir(&dir)
        .expect("listed")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(
        names.len(),
        2,
        "the version and the pointer, no temporary file"
    );
}

#[test]
fn a_folder_of_the_same_version_is_replaced_and_its_old_copy_pruned() {
    let root = tempfile::tempdir().expect("temp");
    let dir = root.path().join("tool");
    unpacked(&dir, "1.0.0", "old");
    let from = unpacked(root.path(), "staged", "new");
    let target = activate(&dir, "1.0.0", &from, "7").expect("activated");
    assert_eq!(marker(&target), "new");
    prune(&dir, "1.0.0");
    assert!(
        !dir.join(".retired-1.0.0-7").exists(),
        "pruned with the rest"
    );
}

#[test]
fn a_pointer_that_cannot_be_written_restores_the_folder_set_aside() {
    let root = tempfile::tempdir().expect("temp");
    let dir = root.path().join("tool");
    unpacked(&dir, "1.0.0", "old");
    // A folder with content where the pointer goes: the rename over it fails.
    fs::create_dir_all(dir.join(POINTER).join("blocked")).expect("blocked");
    let from = unpacked(root.path(), "staged", "new");
    assert!(activate(&dir, "1.0.0", &from, "2").is_err());
    assert_eq!(marker(&dir.join("1.0.0")), "old", "the old version is back");
    assert!(!dir.join(".retired-1.0.0-2").exists());
}

#[test]
fn a_failed_move_restores_the_folder_set_aside() {
    let root = tempfile::tempdir().expect("temp");
    let dir = root.path().join("tool");
    unpacked(&dir, "1.0.0", "old");
    let missing = root.path().join("not-there");
    assert!(activate(&dir, "1.0.0", &missing, "3").is_err());
    assert_eq!(marker(&dir.join("1.0.0")), "old");
    assert_eq!(read_pointer(&dir), None, "no pointer was written");
}

#[test]
fn prune_keeps_the_active_version_and_the_files() {
    let root = tempfile::tempdir().expect("temp");
    let dir = root.path();
    for version in ["1.0.0", "2.0.0", ".retired-2.0.0-1"] {
        fs::create_dir_all(dir.join(version)).expect("version");
    }
    fs::write(dir.join(POINTER), r#"{"version":"2.0.0"}"#).expect("pointer");
    prune(dir, "2.0.0");
    assert!(dir.join("2.0.0").is_dir());
    assert!(!dir.join("1.0.0").exists());
    assert!(!dir.join(".retired-2.0.0-1").exists());
    assert_eq!(read_pointer(dir).as_deref(), Some("2.0.0"));
}

#[test]
fn a_pointer_that_does_not_read_names_nothing() {
    let root = tempfile::tempdir().expect("temp");
    fs::write(root.path().join(POINTER), "not json").expect("pointer");
    assert_eq!(read_pointer(root.path()), None);
}
