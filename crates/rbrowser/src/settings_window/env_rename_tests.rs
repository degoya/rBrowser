use serde_json::json;

use super::*;

fn env(id: &str, url: &str) -> Value {
    json!({ "id": id, "url": url })
}

#[test]
fn a_changed_id_at_the_same_place_and_url_is_a_rename() {
    let before = json!([
        env("local", "http://localhost:3000"),
        env("staging", "https://s.example")
    ]);
    let after = json!([
        env("local", "http://localhost:3000"),
        env("stage", "https://s.example")
    ]);
    assert_eq!(
        renames(&before, &after),
        [("staging".to_owned(), "stage".to_owned())]
    );
}

#[test]
fn an_added_removed_or_moved_environment_is_no_rename() {
    let one = json!([env("local", "http://localhost:3000")]);
    let two = json!([
        env("local", "http://localhost:3000"),
        env("staging", "https://s.example")
    ]);
    assert!(renames(&one, &two).is_empty(), "added");
    assert!(renames(&two, &one).is_empty(), "removed");
    let swapped = json!([
        env("staging", "https://s.example"),
        env("local", "http://localhost:3000")
    ]);
    assert!(
        renames(&two, &swapped).is_empty(),
        "moved: both ids are still there"
    );
    assert!(renames(&json!(null), &two).is_empty(), "nothing before");
}

/// A new id at the place of an old one with another URL is another environment: it takes no
/// Basic auth along to a host the person never stored a credential for (RB-094-08's rule that
/// nothing but the person turns the answer on for another environment).
#[test]
fn a_new_url_with_the_new_id_is_no_rename() {
    let before = json!([env("staging", "https://s.example")]);
    let after = json!([env("other", "https://elsewhere.example")]);
    assert!(renames(&before, &after).is_empty());
}

#[test]
fn every_reference_follows_the_new_id_and_nothing_else_changes() {
    let mut document = json!({
        "version": 1,
        "environments": [env("local", "http://localhost:3000"), env("stage", "https://s.example")],
        "basicAuth": { "environments": ["staging"] },
        "envCompare": { "environments": ["local", "staging"], "breakpoints": ["staging"] },
        "startUrl": "staging"
    });
    let renamed = [("staging".to_owned(), "stage".to_owned())];
    assert!(rename_in(&mut document, &renamed));
    assert_eq!(document["basicAuth"]["environments"], json!(["stage"]));
    assert_eq!(
        document["envCompare"]["environments"],
        json!(["local", "stage"])
    );
    assert_eq!(
        document["envCompare"]["breakpoints"],
        json!(["staging"]),
        "not a reference"
    );
    assert_eq!(document["startUrl"], json!("staging"));
    assert!(
        !rename_in(&mut document, &renamed),
        "nothing left to change"
    );
}

#[test]
fn a_document_without_references_is_left_as_it_is() {
    let mut document = json!({ "version": 1, "basicAuth": { "environments": "not a list" } });
    let before = document.clone();
    assert!(!rename_in(
        &mut document,
        &[("a".to_owned(), "b".to_owned())]
    ));
    assert_eq!(document, before);
}

/// The API client names its environments like the project's (`rb_api_model::env`): the private
/// values in `config.local.json` (`api-client.secrets`) and the checked-in
/// `.rbrowser/api/http-client.env.json` follow the rename, in their order.
#[test]
fn the_api_client_s_environments_follow_the_rename() {
    let renamed = [("staging".to_owned(), "stage".to_owned())];
    let mut local =
        json!({ "api-client": { "secrets": { "local": { "t": "1" }, "staging": { "t": "2" } } } });
    assert!(rename_in(&mut local, &renamed));
    let secrets = &local["api-client"]["secrets"];
    assert_eq!(secrets["stage"], json!({ "t": "2" }));
    assert!(secrets.get("staging").is_none());
    let order: Vec<&String> = secrets.as_object().expect("object").keys().collect();
    assert_eq!(order, ["local", "stage"]);

    let dir = tempfile::tempdir().expect("temp");
    let api = dir.path().join(".rbrowser/api");
    std::fs::create_dir_all(&api).expect("folder");
    let file = api.join(rb_api_model::env::ENV_FILE);
    std::fs::write(
        &file,
        r#"{ "$shared": { "a": "1" }, "staging": { "b": "2" }, "local": { "c": "3" } }"#,
    )
    .expect("written");
    rename_api_environments(dir.path(), &renamed);
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("read")).expect("json");
    let keys: Vec<&String> = written.as_object().expect("object").keys().collect();
    assert_eq!(keys, ["$shared", "stage", "local"]);
    assert_eq!(written["stage"], json!({ "b": "2" }));
}

#[test]
fn a_key_already_taken_is_not_written_over() {
    let mut map = json!({ "staging": { "a": "1" }, "stage": { "b": "2" } });
    assert!(!rename_keys(
        &mut map,
        &[("staging".to_owned(), "stage".to_owned())]
    ));
    assert_eq!(map["stage"], json!({ "b": "2" }));
}
