use std::fs;

use super::*;

const POSTMAN: &str = r#"{
  "info": { "name": "Shop API", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json" },
  "variable": [{ "key": "token", "value": "tok-live-1" }, { "key": "region", "value": "eu" }],
  "item": [
    { "name": "Users", "item": [
      { "name": "List users", "request": { "method": "GET", "url": "{{baseUrl}}/users",
        "auth": { "type": "basic", "basic": [{ "key": "username", "value": "ada" }, { "key": "password", "value": "pw-1" }] } },
        "event": [{ "listen": "test", "script": { "exec": ["pm.test('x')"] } }] }
    ] },
    { "name": "Ping", "request": { "method": "GET", "url": "{{baseUrl}}/ping" } }
  ]
}"#;

fn wanted(files: Vec<ImportFile>) -> Wanted {
    Wanted {
        files,
        format: None,
        folder: None,
        environments: true,
        preview: false,
    }
}

fn postman() -> Vec<ImportFile> {
    vec![ImportFile {
        name: "shop.postman_collection.json".into(),
        text: POSTMAN.into(),
    }]
}

#[test]
fn an_import_writes_one_file_per_request_below_the_collections_folder() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::new(dir.path());
    let answer = run(&store, wanted(postman())).expect("imports");
    assert_eq!(answer["folder"], "shop-api");
    assert_eq!(
        answer["requests"][0]["id"],
        "shop-api/users/list-users.http"
    );
    assert_eq!(answer["requests"][1]["id"], "shop-api/ping.http");
    let (request, text) = store
        .read("shop-api/users/list-users.http")
        .expect("reads back");
    assert_eq!(request.name, "List users");
    assert!(text.contains("{{$secret password}}"), "{text}");
    assert!(!text.contains("pw-1"), "{text}");
    assert_eq!(answer["report"]["imported"], 2);
    assert_eq!(answer["report"]["notes"][0]["code"], "tests");
    assert_eq!(
        answer["report"]["secrets"][0]["variable"],
        "RB_SECRET_TOKEN"
    );
}

#[test]
fn environments_are_added_and_never_overwrite_a_value() {
    let dir = tempfile::tempdir().expect("temp dir");
    let api = dir.path().join(".rbrowser/api");
    fs::create_dir_all(&api).expect("folder");
    fs::write(
        api.join("http-client.env.json"),
        r#"{ "$shared": { "region": "us" }, "dev": { "baseUrl": "http://localhost" } }"#,
    )
    .expect("env file");
    let store = Store::new(dir.path());
    let answer = run(&store, wanted(postman())).expect("imports");
    assert_eq!(answer["kept"], json!(["$shared.region"]));
    let file: Value =
        serde_json::from_str(&fs::read_to_string(api.join("http-client.env.json")).expect("read"))
            .expect("json");
    assert_eq!(file["$shared"]["region"], "us");
    assert_eq!(file["$shared"]["token"], "{{$secret token}}");
    assert_eq!(file["dev"]["baseUrl"], "http://localhost");
    let text = serde_json::to_string(&answer).expect("serializes");
    assert!(!text.contains("tok-live-1") && !text.contains("pw-1"));
    assert_eq!(
        answer["environments"][0]["variables"],
        json!(["token", "region"])
    );
}

#[test]
fn a_preview_writes_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::new(dir.path());
    let mut asked = wanted(postman());
    asked.preview = true;
    let answer = run(&store, asked).expect("reads");
    assert_eq!(answer["preview"], true);
    assert_eq!(answer["requests"][0]["folder"], "shop-api/users");
    assert!(answer["requests"][0].get("id").is_none());
    assert!(!dir.path().join(".rbrowser").exists());
}

#[test]
fn a_second_import_takes_free_names_and_environments_can_stay_out() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::new(dir.path());
    run(&store, wanted(postman())).expect("first");
    let mut again = wanted(postman());
    again.environments = false;
    again.folder = Some("legacy".into());
    let answer = run(&store, again).expect("second");
    assert_eq!(answer["requests"][1]["id"], "legacy/ping.http");
    let mut third = wanted(postman());
    third.environments = false;
    let answer = run(&store, third).expect("third");
    assert_eq!(answer["requests"][1]["id"], "shop-api/ping-2.http");
    assert_eq!(answer["environmentsWritten"], false);
}

#[test]
fn what_does_not_read_fails_with_its_code() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::new(dir.path());
    let code = |asked: Wanted| {
        run(&store, asked)
            .expect_err("refused")
            .code()
            .map(str::to_owned)
    };
    assert_eq!(
        code(wanted(Vec::new())).as_deref(),
        Some("plugin.api-client.errors.import_empty")
    );
    assert_eq!(
        code(wanted(vec![ImportFile {
            name: "notes.txt".into(),
            text: "some words".into()
        }]))
        .as_deref(),
        Some("plugin.api-client.errors.import_unknown_format")
    );
    let mut bad_format = wanted(postman());
    bad_format.format = Some("word".into());
    assert_eq!(
        code(bad_format).as_deref(),
        Some("plugin.api-client.errors.bad_format")
    );
    let mut bad_folder = wanted(postman());
    bad_folder.folder = Some("../out".into());
    assert_eq!(
        code(bad_folder).as_deref(),
        Some("plugin.api-client.errors.bad_id")
    );
}
