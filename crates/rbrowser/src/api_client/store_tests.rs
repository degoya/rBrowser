use std::fs;

use rb_api_model::{ApiRequest, Body};

use super::*;

fn project() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::new(dir.path());
    (dir, store)
}

fn named(name: &str) -> ApiRequest {
    let mut request = ApiRequest::new("GET", "{{baseUrl}}/users");
    request.name = name.into();
    request
}

#[test]
fn a_request_is_saved_as_a_file_named_after_its_title_and_listed() {
    let (dir, store) = project();
    let id = store
        .save(None, Some("users"), &named("List users"))
        .expect("saved");
    assert_eq!(id, "users/list-users.http");
    let text = fs::read_to_string(dir.path().join(".rbrowser/api/users/list-users.http"))
        .expect("written");
    assert!(text.starts_with("### List users\nGET {{baseUrl}}/users\n"));
    let again = store
        .save(None, Some("users"), &named("List users"))
        .expect("saved");
    assert_eq!(again, "users/list-users-2.http");
    let listed = store.list().expect("lists");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].name, "List users");
    assert_eq!(listed[0].method, "GET");
    let (read, _) = store.read(&id).expect("reads");
    assert_eq!(read, named("List users"));
}

#[test]
fn saving_under_an_id_replaces_the_file_and_delete_removes_it() {
    let (_dir, store) = project();
    let id = store
        .save(Some("a.http"), None, &named("A"))
        .expect("saved");
    let mut changed = named("A");
    changed.body = Body::Json { text: "{}".into() };
    changed.method = "POST".into();
    store
        .save(Some(id.as_str()), None, &changed)
        .expect("saved");
    assert_eq!(store.read(&id).expect("reads").0, changed);
    store.delete(&id).expect("deleted");
    let missing = store.read(&id).expect_err("gone");
    assert_eq!(missing.code(), Some("plugin.api-client.errors.not_found"));
    assert!(store.delete(&id).is_err());
}

#[test]
fn bad_ids_and_invalid_requests_are_refused() {
    let (_dir, store) = project();
    for id in ["../x.http", "/x.http", "x.json", ".x.http"] {
        let failure = store
            .save(Some(id), None, &named("x"))
            .expect_err("refused");
        assert_eq!(failure.code(), Some("plugin.api-client.errors.bad_id"));
    }
    let failure = store
        .save(None, Some("../up"), &named("x"))
        .expect_err("refused");
    assert_eq!(failure.code(), Some("plugin.api-client.errors.bad_id"));
    let failure = store
        .save(None, None, &ApiRequest::new("get", "x"))
        .expect_err("refused");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.invalid_request")
    );
}

#[test]
fn a_file_that_does_not_read_is_listed_with_its_problem() {
    let (dir, store) = project();
    let api = dir.path().join(".rbrowser/api");
    fs::create_dir_all(&api).expect("dir");
    fs::write(api.join("broken.http"), "# only a comment\n").expect("written");
    fs::write(api.join("notes.txt"), "not a request").expect("written");
    let listed = store.list().expect("lists");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "broken.http");
    assert!(
        listed[0]
            .problem
            .as_deref()
            .is_some_and(|problem| problem.contains("unreadable"))
    );
}

#[test]
fn no_project_folder_lists_nothing_and_the_env_file_is_empty() {
    let (_dir, store) = project();
    assert!(store.list().expect("lists").is_empty());
    assert_eq!(store.env_file().expect("reads"), EnvFile::default());
}

#[test]
fn the_env_file_reads_and_a_broken_one_says_why() {
    let (dir, store) = project();
    let api = dir.path().join(".rbrowser/api");
    fs::create_dir_all(&api).expect("dir");
    fs::write(
        api.join("http-client.env.json"),
        r#"{"local": {"baseUrl": "http://localhost:3000"}}"#,
    )
    .expect("written");
    let file = store.env_file().expect("reads");
    assert_eq!(file.get("local", "baseUrl"), Some("http://localhost:3000"));
    fs::write(api.join("http-client.env.json"), "[1]").expect("written");
    let failure = store.env_file().expect_err("refused");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.env_unreadable")
    );
}

#[test]
fn a_body_file_stays_in_the_project_and_out_of_rbrowsers_own_files() {
    let (dir, store) = project();
    fs::create_dir_all(dir.path().join(".rbrowser/api/upload")).expect("dir");
    fs::create_dir_all(dir.path().join("assets")).expect("dir");
    fs::write(dir.path().join("assets/logo.png"), b"png").expect("written");
    fs::write(dir.path().join(".rbrowser/api/upload/data.json"), b"{}").expect("written");
    fs::write(dir.path().join(".rbrowser/config.local.json"), b"{}").expect("written");
    let id = Some("upload/send.http");
    assert_eq!(
        store
            .body_file(id, "../../../assets/logo.png", 1024)
            .expect("reads"),
        b"png"
    );
    assert_eq!(
        store.body_file(id, "./data.json", 1024).expect("reads"),
        b"{}"
    );
    for refused in [
        "../../config.local.json",
        "../../../../outside.txt",
        "C:/Windows/win.ini",
        "../../../.git/config",
    ] {
        assert!(store.body_file(id, refused, 1024).is_err(), "{refused}");
    }
    assert!(store.body_file(id, "./data.json", 1).is_err());
}

#[test]
fn a_body_file_names_rbrowsers_own_files_in_no_spelling() {
    // Windows reads every one of these as `.git` or `.rbrowser`: case folded, trailing dots and
    // spaces dropped, an 8.3 short name. Each that can be made exists here, so only the guard
    // refuses it.
    let (dir, store) = project();
    fs::create_dir_all(dir.path().join(".rbrowser/api")).expect("dir");
    let spellings = [
        ".RBROWSER/config.local.json",
        ".rbrowser./config.local.json",
        ".rbrowser /config.local.json",
        ".Rbrowser. ./config.local.json",
        "RBROWS~1/config.local.json",
        ".GIT/config",
        ".git./config",
        "GIT~1/config",
        ".../x.txt",
        ".rbrowser/API./x.txt",
    ];
    for spelling in spellings {
        // Where the platform can make the spelling, the file is there; a spelling it cannot
        // make (an 8.3 name with short names off, `...` on Windows) is refused all the same.
        let path = dir.path().join(spelling);
        if fs::create_dir_all(path.parent().expect("a folder")).is_ok() {
            let _made = fs::write(&path, b"secret");
        }
        let refused = store
            .body_file(Some("send.http"), &format!("../../{spelling}"), 1024)
            .expect_err(spelling);
        assert_eq!(
            refused.code(),
            Some("plugin.api-client.errors.outside"),
            "{spelling}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_link_is_never_followed() {
    let (dir, store) = project();
    let outside = tempfile::tempdir().expect("temp dir");
    fs::write(
        outside.path().join("secret.http"),
        "GET https://a.example\n",
    )
    .expect("written");
    fs::create_dir_all(dir.path().join(".rbrowser")).expect("dir");
    std::os::unix::fs::symlink(outside.path(), dir.path().join(".rbrowser/api")).expect("link");
    let failure = store.read("secret.http").expect_err("refused");
    assert_eq!(failure.code(), Some("plugin.api-client.errors.link"));
    assert!(store.list().is_err());
    assert!(store.save(Some("x.http"), None, &named("x")).is_err());
}
