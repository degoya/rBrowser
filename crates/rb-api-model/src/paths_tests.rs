use super::*;

#[test]
fn ids_stay_below_the_collections() {
    assert!(is_request_id("users/create.http"));
    assert!(is_request_id("a/b/c/d/e.http"));
    assert!(!is_request_id("a/b/c/d/e/f.http"));
    assert!(!is_request_id("../escape.http"));
    assert!(!is_request_id("/abs.http"));
    assert!(!is_request_id("a//b.http"));
    assert!(!is_request_id(".hidden.http"));
    assert!(!is_request_id("users/create.json"));
    assert!(!is_request_id(".http"));
    assert!(!is_request_id("c:\\x.http"));
    assert!(!is_request_id(&format!(
        "{}.http",
        "a".repeat(MAX_ID_BYTES)
    )));
    assert!(is_folder(""));
    assert!(is_folder("users/admin"));
    assert!(!is_folder("users/.."));
}

#[test]
fn a_title_becomes_a_file_name() {
    assert_eq!(file_name("Create user (v2)"), "create-user-v2.http");
    assert_eq!(file_name("  GET /users/:id "), "get-users-id.http");
    assert_eq!(file_name("***"), "request.http");
    assert_eq!(
        file_name(&"x".repeat(80)),
        format!("{}.http", "x".repeat(60))
    );
}

#[test]
fn a_taken_id_gets_a_number() {
    let taken = ["a.http", "a-2.http"];
    assert_eq!(free_id("a.http", |id| taken.contains(&id)), "a-3.http");
    assert_eq!(free_id("b.http", |id| taken.contains(&id)), "b.http");
    assert_eq!(join("users/", "a.http"), "users/a.http");
    assert_eq!(join("", "a.http"), "a.http");
}
