use rb_api_model::ApiRequest;

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

fn code(failure: &Failure) -> &str {
    failure.code().unwrap_or("")
}

#[test]
fn a_new_collection_is_an_empty_folder_and_listed() {
    let (dir, store) = project();
    assert_eq!(store.create_folder("users").expect("made"), "users");
    assert!(dir.path().join(".rbrowser/api/users").is_dir());
    assert_eq!(
        store.create_folder("users/admin/").expect("made"),
        "users/admin"
    );
    assert_eq!(store.folders().expect("lists"), ["users", "users/admin"]);
    let again = store.create_folder("users").expect_err("taken");
    assert_eq!(code(&again), "plugin.api-client.errors.exists");
}

#[test]
fn a_collection_name_stays_below_the_folder() {
    let (_dir, store) = project();
    for bad in [
        "",
        "..",
        "../out",
        ".git",
        "a/../../b",
        "a\\b",
        "x.http",
        "a/b/c/d/e",
    ] {
        let failure = store.create_folder(bad).expect_err(bad);
        assert_eq!(
            code(&failure),
            "plugin.api-client.errors.bad_folder",
            "{bad}"
        );
    }
}

#[test]
fn a_collection_is_renamed_never_over_another() {
    let (dir, store) = project();
    store
        .save(None, Some("users"), &named("List users"))
        .expect("saved");
    store.create_folder("people").expect("made");
    let taken = store.rename_folder("users", "people").expect_err("taken");
    assert_eq!(code(&taken), "plugin.api-client.errors.exists");
    let inside = store
        .rename_folder("users", "users/sub")
        .expect_err("into itself");
    assert_eq!(code(&inside), "plugin.api-client.errors.bad_folder");
    assert_eq!(
        store.rename_folder("users", "accounts").expect("renamed"),
        "accounts"
    );
    assert!(
        dir.path()
            .join(".rbrowser/api/accounts/list-users.http")
            .is_file()
    );
    let gone = store.rename_folder("users", "x").expect_err("gone");
    assert_eq!(code(&gone), "plugin.api-client.errors.folder_not_found");
}

#[test]
fn a_collection_is_never_moved_where_its_subfolders_would_be_too_deep() {
    let (dir, store) = project();
    store.create_folder("a/b/c").expect("made");
    let deep = store.rename_folder("a", "x/y/z").expect_err("too deep");
    assert_eq!(code(&deep), "plugin.api-client.errors.too_deep");
    assert!(dir.path().join(".rbrowser/api/a/b/c").is_dir());
    assert_eq!(store.rename_folder("a", "x/y").expect("moved"), "x/y");
    assert!(
        store
            .folders()
            .expect("lists")
            .contains(&"x/y/b/c".to_owned())
    );
}

#[test]
fn a_collection_is_duplicated_under_a_free_name() {
    let (_dir, store) = project();
    store
        .save(None, Some("users"), &named("List users"))
        .expect("saved");
    store
        .save(None, Some("users/admin"), &named("Ban"))
        .expect("saved");
    let (copy, count) = store.duplicate_folder("users").expect("copied");
    assert_eq!((copy.as_str(), count), ("users-copy", 2));
    let (second, _) = store.duplicate_folder("users").expect("copied");
    assert_eq!(second, "users-copy-2");
    let ids: Vec<String> = store
        .list()
        .expect("lists")
        .into_iter()
        .map(|listed| listed.id)
        .collect();
    assert!(ids.contains(&"users-copy/admin/ban.http".to_owned()));
    assert!(ids.contains(&"users-copy/list-users.http".to_owned()));
}

#[test]
fn a_collection_is_deleted_with_its_requests_counted() {
    let (dir, store) = project();
    store.save(None, Some("users"), &named("A")).expect("saved");
    store
        .save(None, Some("users/deep"), &named("B"))
        .expect("saved");
    store.save(None, None, &named("Top")).expect("saved");
    assert_eq!(store.delete_folder("users").expect("deleted"), 2);
    assert!(!dir.path().join(".rbrowser/api/users").exists());
    assert_eq!(store.list().expect("lists").len(), 1);
    let top = store.delete_folder("").expect_err("never the top");
    assert_eq!(code(&top), "plugin.api-client.errors.bad_folder");
}

#[test]
fn a_request_moves_into_a_collection_never_over_another() {
    let (dir, store) = project();
    let id = store.save(None, None, &named("Me")).expect("saved");
    store
        .save(None, Some("users"), &named("Me"))
        .expect("saved");
    let taken = store.move_request(&id, "users").expect_err("taken");
    assert_eq!(code(&taken), "plugin.api-client.errors.exists");
    assert_eq!(
        store.move_request(&id, "people").expect("moved"),
        "people/me.http"
    );
    assert!(dir.path().join(".rbrowser/api/people/me.http").is_file());
    assert!(!dir.path().join(".rbrowser/api/me.http").exists());
    assert_eq!(
        store.move_request("people/me.http", "").expect("moved"),
        "me.http"
    );
}

#[test]
fn a_request_is_duplicated_beside_itself() {
    let (_dir, store) = project();
    let id = store
        .save(None, Some("users"), &named("Me"))
        .expect("saved");
    let copy = store.duplicate(&id, Some("Me copy")).expect("copied");
    assert_eq!(copy, "users/me-copy.http");
    let (request, _) = store.read(&copy).expect("reads");
    assert_eq!(request.name, "Me copy");
    let same = store.duplicate(&id, None).expect("copied");
    assert_eq!(same, "users/me-2.http");
}

#[cfg(unix)]
#[test]
fn a_link_inside_is_neither_followed_nor_copied() {
    let (dir, store) = project();
    store.save(None, Some("users"), &named("A")).expect("saved");
    let outside = tempfile::tempdir().expect("outside");
    std::fs::write(outside.path().join("x.http"), "GET https://a.test/\n").expect("written");
    std::os::unix::fs::symlink(
        outside.path(),
        dir.path().join(".rbrowser/api/users/linked"),
    )
    .expect("link");
    let (copy, count) = store.duplicate_folder("users").expect("copied");
    assert_eq!(count, 1);
    assert!(
        !dir.path()
            .join(".rbrowser/api")
            .join(copy)
            .join("linked")
            .exists()
    );
    assert_eq!(store.delete_folder("users").expect("deleted"), 1);
    assert!(outside.path().join("x.http").is_file());
    std::os::unix::fs::symlink(outside.path(), dir.path().join(".rbrowser/api/out")).expect("link");
    let failure = store.rename_folder("out", "in").expect_err("a link");
    assert_eq!(code(&failure), "plugin.api-client.errors.link");
}
