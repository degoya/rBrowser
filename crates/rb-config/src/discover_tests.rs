use super::*;
use std::fs;

#[test]
fn finds_the_project_from_a_nested_directory_and_a_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().join("site");
    fs::create_dir_all(root.join(".rbrowser")).expect("mkdir");
    fs::write(root.join(".rbrowser/config.json"), "{}").expect("write");
    let nested = root.join("src/components");
    fs::create_dir_all(&nested).expect("mkdir");
    fs::write(nested.join("App.vue"), "").expect("write");

    assert_eq!(discover(&nested), Some(root.clone()));
    assert_eq!(discover(&nested.join("App.vue")), Some(root.clone()));
    assert_eq!(discover(&root), Some(root));
}

#[test]
fn a_config_directory_without_config_json_does_not_count() {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(dir.path().join("a/.rbrowser")).expect("mkdir");
    assert_eq!(discover(&dir.path().join("a")), None);
}
