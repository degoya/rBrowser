use super::*;
use crate::fake::FakeHost;
use serde_json::json;

fn save_args(value: &Value) -> Result<SaveArgs, Failure> {
    SaveArgs::parse(&value.to_string())
}

fn save_json(host: &mut FakeHost, value: &Value) -> Result<Value, Failure> {
    save(host, &save_args(value)?)
}

#[test]
fn nothing_stored_reads_as_no_presets() {
    let mut host = FakeHost::default();
    assert!(read(&mut host).expect("no file is no presets").is_empty());
    host.no_project = true;
    assert!(read(&mut host).is_err(), "no project is no store");
    assert!(find(&mut host, "any").is_none());
}

#[test]
fn a_preset_is_saved_into_the_plugins_own_file_and_read_back() {
    let mut host = FakeHost::default();
    let answer = save_json(
        &mut host,
        &json!({ "name": " Sign-up ", "values": { "email": "qa@example.com", "plan": "pro" } }),
    )
    .expect("saves");
    assert_eq!(answer["saved"], "Sign-up");
    assert_eq!(answer["replaced"], false);
    assert_eq!(answer["fields"], 2);
    assert_eq!(answer["$post"][0]["target"], "panel");
    assert_eq!(answer["$post"][0]["message"]["type"], "presets");
    let file = host.out_json(FILE).expect("written");
    assert_eq!(file["version"], 1);
    assert_eq!(file["presets"][0]["name"], "Sign-up");
    // Only the plugin's own folder is written.
    assert!(
        host.files
            .keys()
            .all(|path| path.starts_with(crate::fake::OUT))
    );
    let found = find(&mut host, "Sign-up").expect("found");
    assert_eq!(found.values.get("plan").map(String::as_str), Some("pro"));
}

#[test]
fn a_name_taken_needs_replace_and_a_settings_profile_is_never_shadowed() {
    let mut host = FakeHost::default();
    host.set(
        "profiles",
        r#"[{"name":"testuser","values":{"email":"a@b.c"}}]"#,
    );
    let first = json!({ "name": "one", "values": { "email": "x@example.com" } });
    save_json(&mut host, &first).expect("saves");
    let failure = save_json(&mut host, &first).expect_err("taken");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.form-autofill.errors.preset_exists")
    );
    assert_eq!(failure.param("name"), Some("one"));
    let again = json!({ "name": "one", "values": { "email": "y@example.com" }, "replace": true });
    let answer = save_json(&mut host, &again).expect("replaces");
    assert_eq!(answer["replaced"], true);
    assert_eq!(read(&mut host).expect("reads").len(), 1);
    let failure = save_json(
        &mut host,
        &json!({ "name": "testuser", "values": { "email": "z@example.com" }, "replace": true }),
    )
    .expect_err("a settings profile");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.form-autofill.errors.preset_in_settings")
    );
}

#[test]
fn a_locator_that_names_a_secret_is_dropped_and_nothing_left_is_refused() {
    let mut host = FakeHost::default();
    let answer = save_json(
        &mut host,
        &json!({ "name": "login", "values": { "user": "linda", "password": "hunter2", "csrf_token": "abc" } }),
    )
    .expect("saves the rest");
    assert_eq!(answer["fields"], 1);
    assert_eq!(answer["dropped"], json!(["csrf_token", "password"]));
    let stored = host.out_json(FILE).expect("written");
    assert_eq!(stored["presets"][0]["values"], json!({ "user": "linda" }));
    let failure = save_json(
        &mut host,
        &json!({ "name": "secret only", "values": { "pin": "1234" } }),
    )
    .expect_err("nothing left");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.form-autofill.errors.preset_empty")
    );
}

#[test]
fn the_store_holds_at_most_fifty() {
    let mut host = FakeHost::default();
    for at in 0..MAX_PRESETS {
        save_json(
            &mut host,
            &json!({ "name": format!("p{at}"), "values": { "a": "b" } }),
        )
        .expect("saves");
    }
    let failure = save_json(
        &mut host,
        &json!({ "name": "one more", "values": { "a": "b" } }),
    )
    .expect_err("full");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.form-autofill.errors.presets_full")
    );
    save_json(
        &mut host,
        &json!({ "name": "p3", "values": { "a": "c" }, "replace": true }),
    )
    .expect("replacing one is no new one");
}

#[test]
fn bad_arguments_are_named() {
    for (value, reason) in [
        (json!({ "values": { "a": "b" } }), "name"),
        (json!({ "name": "  ", "values": { "a": "b" } }), "name"),
        (
            json!({ "name": "x".repeat(81), "values": { "a": "b" } }),
            "name",
        ),
        (json!({ "name": "a\u{7}b", "values": { "a": "b" } }), "name"),
        (json!({ "name": "n" }), "values"),
        (json!({ "name": "n", "values": { "a": 1 } }), "values"),
        (json!({ "name": "n", "values": { "": "b" } }), "values"),
        (
            json!({ "name": "n", "values": { "a": "b".repeat(1001) } }),
            "values",
        ),
        (
            json!({ "name": "n", "values": { "a": "b" }, "replace": "yes" }),
            "replace",
        ),
        (json!([1]), "args"),
    ] {
        let failure = save_args(&value).expect_err("refused");
        assert_eq!(failure.code.as_deref(), Some(codes::INVALID), "{value}");
        assert_eq!(failure.param("reason"), Some(reason), "{value}");
    }
    let many: serde_json::Map<String, Value> = (0..=MAX_VALUES)
        .map(|at| (format!("k{at}"), json!("v")))
        .collect();
    let failure = save_args(&json!({ "name": "n", "values": many })).expect_err("too many");
    assert_eq!(failure.param("reason"), Some("values"));
}

#[test]
fn the_list_names_every_preset_with_its_values() {
    let mut host = FakeHost::default();
    save_json(
        &mut host,
        &json!({ "name": "b", "values": { "email": "b@example.com" } }),
    )
    .expect("saves");
    save_json(
        &mut host,
        &json!({ "name": "a", "values": { "x": "1", "y": "2" } }),
    )
    .expect("saves");
    let answer = list(&mut host).expect("lists");
    assert_eq!(answer["presets"][0]["name"], "b");
    assert_eq!(answer["presets"][1]["fields"], 2);
    assert_eq!(answer["presets"][1]["values"]["y"], "2");
    assert_eq!(answer["file"], READ_PATH);
    host.no_project = true;
    let answer = list(&mut host).expect("no project is an empty list");
    assert_eq!(answer["presets"], json!([]));
    assert_eq!(answer["available"], false);
}

#[test]
fn a_file_that_does_not_read_whole_is_never_overwritten() {
    // Review finding 1: what could not be read is refused, not rewritten from what was left.
    let good = json!({ "name": "ok", "values": { "a": "b" } });
    let many: Vec<Value> = (0..=MAX_PRESETS)
        .map(|at| json!({ "name": format!("p{at}"), "values": { "a": "b" } }))
        .collect();
    for file in [
        b"not json".to_vec(),
        json!({ "version": 1, "presets": "x" })
            .to_string()
            .into_bytes(),
        json!({ "version": 1, "presets": [good.clone(), { "values": {} }] })
            .to_string()
            .into_bytes(),
        json!({ "version": 1, "presets": [{ "name": "n", "values": { "a": 1 } }] })
            .to_string()
            .into_bytes(),
        json!({ "version": 1, "presets": [{ "name": " padded ", "values": { "a": "b" } }] })
            .to_string()
            .into_bytes(),
        json!({ "version": 1, "presets": many })
            .to_string()
            .into_bytes(),
    ] {
        let mut host = FakeHost::default();
        host.files.insert(READ_PATH.to_owned(), file.clone());
        let failure = read(&mut host).expect_err("refused");
        assert_eq!(
            failure.code.as_deref(),
            Some("plugin.form-autofill.errors.presets_unreadable")
        );
        let failure = save_json(&mut host, &json!({ "name": "new", "values": { "a": "b" } }))
            .expect_err("no save over it");
        assert_eq!(
            failure.code.as_deref(),
            Some("plugin.form-autofill.errors.presets_unreadable")
        );
        assert_eq!(host.files.get(READ_PATH), Some(&file), "left as it was");
        let listed = list(&mut host).expect("lists");
        assert_eq!(listed["available"], false);
        assert_eq!(
            listed["error"],
            "plugin.form-autofill.errors.presets_unreadable"
        );
    }
    // A fill still finds what reads.
    let mut host = FakeHost::default();
    host.files.insert(
        READ_PATH.to_owned(),
        json!({ "version": 1, "presets": [good, 3] })
            .to_string()
            .into_bytes(),
    );
    assert!(find(&mut host, "ok").is_some());
}

#[test]
fn no_project_is_named_apart_from_a_store_that_does_not_read() {
    let mut host = FakeHost::default();
    host.no_project = true;
    let listed = list(&mut host).expect("lists");
    assert_eq!(listed["available"], false);
    assert_eq!(listed["error"], codes::NOT_AVAILABLE);
}

#[test]
fn a_capture_request_is_taken_once_and_only_while_fresh() {
    let mut host = FakeHost::default();
    host.now = 1000;
    request_capture(&mut host, 3).expect("written");
    host.now = 1000 + REQUEST_SECONDS;
    assert_eq!(take_request(&mut host), Some(3));
    assert_eq!(take_request(&mut host), None, "taken once");
    request_capture(&mut host, 4).expect("written");
    host.now += REQUEST_SECONDS + 1;
    assert_eq!(take_request(&mut host), None, "stale");
    host.no_project = true;
    assert_eq!(take_request(&mut host), None);
    assert!(request_capture(&mut host, 5).is_err());
}
