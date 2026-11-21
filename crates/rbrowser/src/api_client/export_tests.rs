use std::fs;

use rb_api_model::{Auth, Entry};
use serde_json::json;

use super::*;
use crate::api_client::ops::Settings;
use crate::api_client::protocol::{Op, Origin, permitted, requested};

fn context(root: &Path, code: CodeSettings) -> Context {
    Context {
        root: Some(root.to_path_buf()),
        trusted: true,
        environment: Some("staging".into()),
        environments: vec![("staging".into(), "https://staging.example.com/".into())],
        allowed_hosts: vec![],
        secrets: json!({ "staging": { "token": "tok-secret-1" } }),
        settings: Settings {
            history_size: 10,
            max_response_bytes: 1024,
            bench_repeats: 3,
            bench_concurrency: 1,
            code,
        },
        now: 1_700_000_000,
    }
}

const POSIX: CodeSettings = CodeSettings {
    shell: Shell::Posix,
    variables: Variables::Environment,
};

/// `users/me.http`: the base URL, a public variable, the private token, a process secret.
fn saved(root: &Path) -> String {
    let api = root.join(".rbrowser/api");
    fs::create_dir_all(&api).expect("folder");
    fs::write(
        api.join("http-client.env.json"),
        r#"{"staging": {"user": "ada"}}"#,
    )
    .expect("env file");
    let mut request = ApiRequest::new("GET", "{{baseUrl}}/users/{{user}}");
    request.name = "Me".into();
    request.auth = Auth::Bearer {
        token: "{{token}}".into(),
    };
    request
        .headers
        .push(Entry::new("X-Api-Key", "{{$secret api-key}}"));
    Store::new(root)
        .save(None, Some("users"), &request)
        .expect("saved")
}

#[test]
fn code_puts_plain_values_in_and_names_every_secret() {
    let dir = tempfile::tempdir().expect("temp dir");
    let id = saved(dir.path());
    let ctx = context(dir.path(), POSIX);
    let store = Store::new(dir.path());
    let answer = code(&ctx, &store, Some(&id), None, Language::Curl, None).expect("code");
    let text = answer["text"].as_str().expect("text");
    assert_eq!(
        text,
        "curl --location https://staging.example.com/users/ada \\
  --header 'X-Api-Key: '\"${RB_SECRET_API_KEY}\" \\
  --header 'Authorization: Bearer '\"${token}\""
    );
    assert!(!text.contains("tok-secret-1"));
    assert_eq!(answer["env"], json!(["RB_SECRET_API_KEY", "token"]));
    assert_eq!(answer["environment"], "staging");
    assert_eq!(answer["shell"], "posix");
}

#[test]
fn code_never_reads_a_process_secret() {
    // The process variable may exist; the snippet still names it only.
    let dir = tempfile::tempdir().expect("temp dir");
    let id = saved(dir.path());
    let store = Store::new(dir.path());
    let answer = code(
        &context(dir.path(), POSIX),
        &store,
        Some(&id),
        None,
        Language::Fetch,
        None,
    )
    .expect("code");
    let text = answer["text"].as_str().expect("text");
    assert!(text.contains("'X-Api-Key': `${process.env.RB_SECRET_API_KEY}`"));
    assert!(text.contains("`Bearer ${process.env.token}`"));
}

#[test]
fn template_settings_keep_every_variable() {
    let dir = tempfile::tempdir().expect("temp dir");
    let id = saved(dir.path());
    let settings = CodeSettings {
        shell: Shell::Posix,
        variables: Variables::Template,
    };
    let answer = code(
        &context(dir.path(), settings),
        &Store::new(dir.path()),
        Some(&id),
        None,
        Language::Curl,
        None,
    )
    .expect("code");
    let text = answer["text"].as_str().expect("text");
    assert!(text.starts_with("curl --globoff --location '{{baseUrl}}/users/{{user}}'"));
    assert_eq!(answer["env"], json!([]));
}

#[test]
fn code_of_the_panels_draft_and_of_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ctx = context(dir.path(), POSIX);
    let store = Store::new(dir.path());
    let draft = ApiRequest::new("DELETE", "{{baseUrl}}/users/1");
    let answer = code(&ctx, &store, None, Some(draft), Language::Httpie, None).expect("code");
    assert_eq!(
        answer["text"],
        "http --follow DELETE https://staging.example.com/users/1"
    );
    let failure = code(&ctx, &store, None, None, Language::Curl, None).expect_err("nothing");
    assert_eq!(failure.code(), Some("plugin.api-client.errors.bad_args"));
    let bad = ApiRequest::new("get me", "https://x/");
    let failure = code(&ctx, &store, None, Some(bad), Language::Curl, None).expect_err("bad");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.invalid_request")
    );
}

#[test]
fn export_writes_a_collection_below_the_projects_output() {
    let dir = tempfile::tempdir().expect("temp dir");
    saved(dir.path());
    let ctx = context(dir.path(), POSIX);
    let store = Store::new(dir.path());
    let answer = export(&ctx, &store, dir.path(), Some("users"), Format::Postman).expect("export");
    assert_eq!(
        answer["path"],
        ".rbrowser/out/api-client/exports/users.postman_collection.json"
    );
    assert_eq!(answer["count"], 1);
    let text = fs::read_to_string(
        dir.path()
            .join(".rbrowser/out/api-client/exports/users.postman_collection.json"),
    )
    .expect("written");
    assert!(text.contains("{{token}}"));
    assert!(text.contains("{{baseUrl}}/users/{{user}}"));
    assert!(!text.contains("tok-secret-1"));
    assert!(!text.contains("staging.example.com"));
}

#[test]
fn export_of_every_request_as_a_script_and_of_an_empty_folder() {
    let dir = tempfile::tempdir().expect("temp dir");
    saved(dir.path());
    let ctx = context(dir.path(), POSIX);
    let store = Store::new(dir.path());
    let answer = export(&ctx, &store, dir.path(), None, Format::Curl).expect("export");
    let path = answer["path"].as_str().expect("path");
    assert!(
        path.starts_with(EXPORT_DIR) && path.ends_with(".sh"),
        "{path}"
    );
    assert_eq!(
        answer["env"],
        json!(["baseUrl", "user", "RB_SECRET_API_KEY", "token"])
    );
    let failure =
        export(&ctx, &store, dir.path(), Some("orders"), Format::Http).expect_err("empty");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.nothing_to_export")
    );
    let failure = export(&ctx, &store, dir.path(), Some("../x"), Format::Http).expect_err("bad");
    assert_eq!(failure.code(), Some("plugin.api-client.errors.bad_id"));
}

#[test]
fn the_settings_read_their_values_and_fall_back() {
    let values = |value: Value| value.as_object().cloned().expect("an object");
    assert_eq!(settings_of(&Map::new()), CodeSettings::default());
    assert_eq!(
        settings_of(&values(json!({
            "api-client.codeShell": "cmd",
            "api-client.codeVariables": "template"
        }))),
        CodeSettings {
            shell: Shell::Cmd,
            variables: Variables::Template
        }
    );
    assert_eq!(
        settings_of(&values(json!({ "api-client.codeShell": "fish" }))),
        CodeSettings::default()
    );
}

#[test]
fn mcp_may_write_a_saved_request_as_code_and_nothing_else() {
    let op = |text: &str| -> Op {
        requested(&format!("{{\"$apiClient\": {text}}}"))
            .expect("an op")
            .expect("reads")
    };
    let saved = op(r#"{"op": "code", "id": "users/me.http", "language": "curl"}"#);
    assert!(permitted(&saved, Origin::Tool).is_ok());
    let draft = op(
        r#"{"op": "code", "request": {"method": "GET", "url": "https://x/"}, "language": "fetch"}"#,
    );
    assert!(permitted(&draft, Origin::Tool).is_err());
    assert!(permitted(&draft, Origin::Panel).is_ok());
    let export = op(r#"{"op": "export", "folder": "users", "format": "postman"}"#);
    assert_eq!(export.id(), "export");
    assert!(permitted(&export, Origin::Tool).is_err());
    assert!(permitted(&export, Origin::Command).is_err());
    assert!(permitted(&export, Origin::Panel).is_ok());
}
