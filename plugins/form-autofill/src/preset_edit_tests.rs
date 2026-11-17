use super::*;
use crate::fake::FakeHost;
use crate::store::{FILE, SaveArgs, read, save};
use rb_plugin_api::codes;
use serde_json::json;

fn stored(host: &mut FakeHost, names: &[&str]) {
    for name in names {
        let args = SaveArgs::parse(
            &json!({ "name": name, "values": { "email": format!("{name}@example.com") } })
                .to_string(),
        )
        .expect("args");
        save(host, &args).expect("saves");
    }
}

fn names(host: &mut FakeHost) -> Vec<String> {
    read(host)
        .expect("reads")
        .into_iter()
        .map(|preset| preset.name)
        .collect()
}

#[test]
fn a_stored_preset_is_deleted_and_the_panel_told() {
    let mut host = FakeHost::default();
    stored(&mut host, &["one", "two"]);
    let answer = delete(
        &mut host,
        &NameArgs::parse(r#"{"name":" one "}"#).expect("args"),
    )
    .expect("deletes");
    assert_eq!(answer["deleted"], "one");
    assert_eq!(answer["$post"][0]["target"], "panel");
    assert_eq!(answer["$post"][0]["message"]["type"], "presets");
    assert_eq!(answer["$post"][0]["message"]["presets"][0]["name"], "two");
    assert_eq!(names(&mut host), ["two"]);
    // Only the plugin's own file is written.
    assert!(host.out_json(FILE).is_some());
    assert!(
        host.files
            .keys()
            .all(|path| path.starts_with(crate::fake::OUT))
    );
}

#[test]
fn a_name_that_is_no_stored_preset_is_refused() {
    let mut host = FakeHost::default();
    host.set(
        "profiles",
        r#"[{"name":"testuser","values":{"email":"a@b.c"}}]"#,
    );
    stored(&mut host, &["one"]);
    for name in ["ghost", "testuser"] {
        let args = NameArgs::parse(&json!({ "name": name }).to_string()).expect("args");
        let failure = delete(&mut host, &args).expect_err("unknown");
        assert_eq!(
            failure.code.as_deref(),
            Some("plugin.form-autofill.errors.preset_unknown"),
            "{name}"
        );
        assert_eq!(failure.param("name"), Some(name));
        let args =
            NameArgs::parse(&json!({ "name": name, "to": "new" }).to_string()).expect("args");
        let failure = rename(&mut host, &args).expect_err("unknown");
        assert_eq!(
            failure.code.as_deref(),
            Some("plugin.form-autofill.errors.preset_unknown")
        );
    }
    assert_eq!(names(&mut host), ["one"]);
}

#[test]
fn a_preset_is_renamed_in_place_never_onto_another_or_a_settings_profile() {
    let mut host = FakeHost::default();
    host.set(
        "profiles",
        r#"[{"name":"testuser","values":{"email":"a@b.c"}}]"#,
    );
    stored(&mut host, &["one", "two"]);
    let rename_json = |host: &mut FakeHost, value: Value| {
        rename(host, &NameArgs::parse(&value.to_string()).expect("args"))
    };
    let answer =
        rename_json(&mut host, json!({ "name": "one", "to": " Checkout " })).expect("renames");
    assert_eq!(answer["renamed"], "one");
    assert_eq!(answer["to"], "Checkout");
    assert_eq!(answer["$post"][0]["message"]["type"], "presets");
    assert_eq!(names(&mut host), ["Checkout", "two"]);
    let kept = crate::store::find(&mut host, "Checkout").expect("found");
    assert_eq!(
        kept.values.get("email").map(String::as_str),
        Some("one@example.com")
    );
    let failure =
        rename_json(&mut host, json!({ "name": "two", "to": "Checkout" })).expect_err("taken");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.form-autofill.errors.preset_exists")
    );
    let failure = rename_json(&mut host, json!({ "name": "two", "to": "testuser" }))
        .expect_err("a settings profile");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.form-autofill.errors.preset_in_settings")
    );
    // The same name again changes nothing and is no failure.
    rename_json(&mut host, json!({ "name": "two", "to": "two" })).expect("the same name");
    assert_eq!(names(&mut host), ["Checkout", "two"]);
}

#[test]
fn bad_names_are_named_and_an_unreadable_file_is_never_rewritten() {
    for (args, reason) in [
        ("{}", "name"),
        (r#"{"name":""}"#, "name"),
        (r#"{"name":7}"#, "name"),
        ("[]", "args"),
        ("x", "args"),
    ] {
        let failure = NameArgs::parse(args).expect_err(args);
        assert_eq!(failure.code.as_deref(), Some(codes::INVALID), "{args}");
        assert_eq!(failure.param("reason"), Some(reason), "{args}");
    }
    let long = "x".repeat(81);
    let failure =
        NameArgs::parse(&json!({ "name": "a", "to": long }).to_string()).expect_err("too long");
    assert_eq!(failure.param("reason"), Some("to"));
    let mut host = FakeHost::default();
    let args = NameArgs::parse(r#"{"name":"one","to":"two"}"#).expect("args");
    let failure = rename(
        &mut host,
        &NameArgs {
            to: None,
            ..args.clone()
        },
    )
    .expect_err("no to");
    assert_eq!(failure.param("reason"), Some("to"));
    host.files
        .insert(crate::store::READ_PATH.to_owned(), b"{ not json".to_vec());
    for failure in [
        delete(&mut host, &args).expect_err("unreadable"),
        rename(&mut host, &args).expect_err("unreadable"),
    ] {
        assert_eq!(
            failure.code.as_deref(),
            Some("plugin.form-autofill.errors.presets_unreadable")
        );
    }
    assert_eq!(
        host.files.get(crate::store::READ_PATH).map(Vec::as_slice),
        Some(b"{ not json".as_slice())
    );
    host.no_project = true;
    assert!(delete(&mut host, &args).is_err(), "no project, no store");
}
