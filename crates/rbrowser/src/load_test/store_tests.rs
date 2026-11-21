use super::*;
use crate::load_test::scenario::Scenario;

const FILE: &str =
    r#"{"name":"Smoke","steps":[{"request":"a.http"}],"profile":{"users":2,"holdS":5}}"#;

fn project() -> (tempfile::TempDir, Scenarios) {
    let dir = tempfile::tempdir().expect("temp");
    let scenarios = Scenarios::new(dir.path());
    (dir, scenarios)
}

#[test]
fn a_scenario_is_saved_named_after_its_title_listed_read_and_deleted() {
    let (dir, scenarios) = project();
    assert_eq!(scenarios.list().expect("empty"), Vec::new());
    let scenario = Scenario::from_json(FILE).expect("reads");
    let id = scenarios.save(None, &scenario).expect("saved");
    assert_eq!(id, "Smoke.json");
    assert!(dir.path().join(".rbrowser/api/load/Smoke.json").is_file());
    let listed = scenarios.list().expect("listed");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "Smoke.json");
    assert_eq!(listed[0].name, "Smoke");
    assert_eq!(listed[0].problem, None);
    assert_eq!(scenarios.read("Smoke.json").expect("read"), scenario);
    scenarios.delete("Smoke.json").expect("deleted");
    assert!(scenarios.list().expect("listed").is_empty());
}

#[test]
fn a_file_that_does_not_read_is_listed_with_its_problem() {
    let (dir, scenarios) = project();
    let folder = dir.path().join(".rbrowser/api/load");
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("broken.json"), "{").expect("write");
    std::fs::write(folder.join("notes.txt"), "x").expect("write");
    let listed = scenarios.list().expect("listed");
    assert_eq!(listed.len(), 1);
    assert!(listed[0].problem.is_some());
    let failure = scenarios.read("broken.json").expect_err("broken");
    assert_eq!(failure.code(), Some("plugin.load-test.errors.unreadable"));
}

#[test]
fn an_id_that_leaves_the_folder_is_refused() {
    let (_dir, scenarios) = project();
    for id in ["../x.json", "a/b.json", "x.http", "", ".json", "C:x.json"] {
        let failure = scenarios.read(id).expect_err(id);
        assert_eq!(
            failure.code(),
            Some("plugin.load-test.errors.bad_id"),
            "{id}"
        );
    }
}

#[cfg(unix)]
#[test]
fn no_link_is_followed() {
    let (dir, scenarios) = project();
    let outside = tempfile::tempdir().expect("outside");
    std::fs::create_dir_all(dir.path().join(".rbrowser/api")).expect("api");
    std::os::unix::fs::symlink(outside.path(), dir.path().join(".rbrowser/api/load"))
        .expect("link");
    let failure = scenarios.list().expect_err("a link");
    assert_eq!(failure.code(), Some("plugin.load-test.errors.link"));
}
