use super::*;
use crate::jsonio::write_config_file;

#[test]
fn only_the_config_files_count() {
    assert!(is_config_file(Path::new("/p/.rbrowser/config.json")));
    assert!(is_config_file(Path::new("/p/.rbrowser/config.local.json")));
    assert!(!is_config_file(Path::new("/p/config.json")));
    assert!(!is_config_file(Path::new("/p/.rbrowser/flows/a.ts")));
    let [project, local] = watched_files(Path::new("/p"));
    assert!(is_config_file(&project) && is_config_file(&local));
}

#[test]
fn a_config_change_reloads_and_reports_the_result() {
    let dir = tempfile::tempdir().expect("temp dir");
    let value = serde_json::json!({ "version": 1, "startUrl": "http://a" });
    write_config_file(dir.path(), ConfigFile::Project, &value).expect("write");

    let mut seen = None;
    let changed = [dir.path().join("src/app.css")];
    assert!(!reload_on_change(dir.path(), &changed, |r| seen = Some(r)));
    assert!(seen.is_none());

    let changed = [ConfigFile::Project.path(dir.path())];
    assert!(reload_on_change(dir.path(), &changed, |r| seen = Some(r)));
    let loaded = seen.expect("called").expect("loads");
    assert_eq!(loaded.config.start_url.as_deref(), Some("http://a"));
}
