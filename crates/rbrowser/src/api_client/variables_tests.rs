use std::fs;

use serde_json::json;

use super::*;
use crate::api_client::fakes::{context, no_env};

fn project() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::new(dir.path());
    (dir, store)
}

fn code(failure: &Failure) -> &str {
    failure.code().unwrap_or("")
}

#[test]
fn the_variables_come_with_the_private_names_never_their_values() {
    let (dir, store) = project();
    store
        .write_env_json(&json!({ "$shared": { "user": "ada" }, "staging": { "path": "/v1" } }))
        .expect("written");
    let ctx = context(dir.path());
    let answer = read(&ctx, &store).expect("reads");
    assert_eq!(answer["file"]["staging"]["path"], "/v1");
    assert_eq!(answer["private"], json!({ "staging": ["token"] }));
    assert_eq!(answer["baseUrls"]["production"], "https://api.example.com");
    assert!(!answer.to_string().contains("tok-secret-1"));
}

#[test]
fn a_save_writes_the_file_and_moves_secret_rows_to_the_private_values() {
    let (dir, store) = project();
    let ctx = context(dir.path());
    let wanted = Wanted {
        deleted: Vec::new(),
        file: json!({ "$shared": { "user": "ada" }, "qa": { "path": "/qa" } }),
        base: Value::Null,
        private: serde_json::from_value(json!({
            "qa": { "password": "pw-qa" },
            "staging": { "token": null }
        }))
        .expect("patch"),
        renamed: BTreeMap::new(),
    };
    let (saved, file) = save(&ctx, &store, wanted).expect("saved");
    let path = dir.path().join(".rbrowser/api/http-client.env.json");
    assert!(!path.exists(), "the file waits for the private values");
    assert_eq!(saved, json!({ "qa": { "password": "pw-qa" } }));
    commit(
        Some(dir.path()),
        &Value::Null,
        Some(&saved),
        Some(&file),
        |_| Ok(()),
    )
    .expect("kept");
    let text = fs::read_to_string(path).expect("written");
    assert!(text.contains("\"path\": \"/qa\""));
    assert!(!text.contains("pw-qa"));
}

#[test]
fn the_file_is_written_only_after_the_private_values_are_kept() {
    let (dir, store) = project();
    store
        .write_env_json(&json!({ "dev": { "token": "public-before" } }))
        .expect("written");
    let file = json!({ "dev": {} });
    let private = json!({ "dev": { "token": "now-private" } });
    let failure = commit(
        Some(dir.path()),
        &Value::Null,
        Some(&private),
        Some(&file),
        |_| Err(error(FailureKind::Io, "secrets_not_saved")),
    )
    .expect_err("not kept");
    assert_eq!(code(&failure), "plugin.api-client.errors.secrets_not_saved");
    assert_eq!(
        store.env_json().expect("reads"),
        json!({ "dev": { "token": "public-before" } })
    );
    let mut order = Vec::new();
    commit(
        Some(dir.path()),
        &Value::Null,
        Some(&private),
        Some(&file),
        |_| {
            order.push(store.env_json().expect("reads"));
            Ok(())
        },
    )
    .expect("kept");
    assert_eq!(order, [json!({ "dev": { "token": "public-before" } })]);
    assert_eq!(store.env_json().expect("reads"), file);
}

/// Code review w33, item 2: the file not written after the private values were kept puts the
/// private values back as they were - a row moved from the private values to the file is not
/// lost, and one moved the other way is not kept twice.
#[test]
fn a_file_not_written_puts_the_private_values_back() {
    let dir = tempfile::tempdir().expect("temp dir");
    // A file where the project folder should be: the environment file cannot be written.
    let root = dir.path().join("blocked");
    fs::write(&root, b"").expect("written");
    let before = json!({ "dev": { "token": "was-private" } });
    let directions = [
        (json!({}), json!({ "dev": { "token": "was-private" } })),
        (
            json!({ "dev": { "token": "was-private", "key": "now-private" } }),
            json!({ "dev": {} }),
        ),
    ];
    for (secrets, file) in directions {
        let mut kept = Vec::new();
        let failure = commit(
            Some(&root),
            &before,
            Some(&secrets),
            Some(&file),
            |values| {
                kept.push(values.clone());
                Ok(())
            },
        )
        .expect_err("not written");
        assert_ne!(code(&failure), "plugin.api-client.errors.secrets_not_saved");
        assert_eq!(kept, [secrets.clone(), before.clone()]);
    }
}

/// Putting them back may fail too: the file's failure is the answer.
#[test]
fn a_failed_put_back_answers_the_file_failure() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().join("blocked");
    fs::write(&root, b"").expect("written");
    let mut calls = 0;
    let failure = commit(
        Some(&root),
        &json!({ "dev": { "token": "t" } }),
        Some(&json!({})),
        Some(&json!({ "dev": { "token": "t" } })),
        |_| {
            calls += 1;
            if calls == 1 {
                Ok(())
            } else {
                Err(error(FailureKind::Io, "secrets_not_saved"))
            }
        },
    )
    .expect_err("not written");
    assert_eq!(calls, 2);
    assert_ne!(code(&failure), "plugin.api-client.errors.secrets_not_saved");
}

#[test]
fn a_rename_onto_an_environment_kept_is_refused_and_a_swap_keeps_both() {
    let (dir, store) = project();
    let mut ctx = context(dir.path());
    ctx.secrets = json!({ "dev": { "a": "dev-1" }, "stage": { "b": "stage-2" } });
    let onto = Wanted {
        deleted: Vec::new(),
        file: json!({ "dev": {} }),
        base: Value::Null,
        private: BTreeMap::new(),
        renamed: BTreeMap::from([("stage".to_owned(), "dev".to_owned())]),
    };
    let failure = save(&ctx, &store, onto).expect_err("taken");
    assert_eq!(code(&failure), "plugin.api-client.errors.exists");
    let swap = Wanted {
        deleted: Vec::new(),
        file: json!({ "dev": {}, "stage": {} }),
        base: Value::Null,
        private: BTreeMap::new(),
        renamed: BTreeMap::from([
            ("dev".to_owned(), "stage".to_owned()),
            ("stage".to_owned(), "dev".to_owned()),
        ]),
    };
    let (private, _) = save(&ctx, &store, swap).expect("swapped");
    assert_eq!(
        private,
        json!({ "stage": { "a": "dev-1" }, "dev": { "b": "stage-2" } })
    );
}

#[test]
fn a_save_over_a_file_changed_meanwhile_is_refused() {
    let (dir, store) = project();
    store
        .write_env_json(&json!({ "dev": { "a": "1" } }))
        .expect("written");
    let ctx = context(dir.path());
    let wanted = Wanted {
        deleted: Vec::new(),
        file: json!({ "dev": { "a": "2" } }),
        base: json!({ "dev": { "a": "0" } }),
        private: BTreeMap::new(),
        renamed: BTreeMap::new(),
    };
    let failure = save(&ctx, &store, wanted).expect_err("changed");
    assert_eq!(code(&failure), "plugin.api-client.errors.env_changed");
}

#[test]
fn a_renamed_environment_takes_its_private_values_along() {
    let (dir, store) = project();
    let ctx = context(dir.path());
    let wanted = Wanted {
        deleted: Vec::new(),
        file: json!({ "stage": {} }),
        base: Value::Null,
        private: BTreeMap::new(),
        renamed: BTreeMap::from([("staging".to_owned(), "stage".to_owned())]),
    };
    let (private, file) = save(&ctx, &store, wanted).expect("saved");
    assert_eq!(private, json!({ "stage": { "token": "tok-secret-1" } }));
    commit(
        Some(dir.path()),
        &Value::Null,
        Some(&private),
        Some(&file),
        |_| Ok(()),
    )
    .expect("kept");
    let dropped = Wanted {
        deleted: Vec::new(),
        file: json!({}),
        base: json!({ "stage": {} }),
        private: serde_json::from_value(json!({ "staging": null })).expect("patch"),
        renamed: BTreeMap::new(),
    };
    assert_eq!(save(&ctx, &store, dropped).expect("saved").0, json!({}));
}

#[test]
fn a_save_that_is_no_environment_file_writes_nothing() {
    let (dir, store) = project();
    let ctx = context(dir.path());
    let wanted = Wanted {
        deleted: Vec::new(),
        file: json!({ "dev": { "1bad": "x" } }),
        base: Value::Null,
        private: BTreeMap::new(),
        renamed: BTreeMap::new(),
    };
    let failure = save(&ctx, &store, wanted).expect_err("refused");
    assert_eq!(code(&failure), "plugin.api-client.errors.env_unreadable");
    assert!(
        !dir.path()
            .join(".rbrowser/api/http-client.env.json")
            .exists()
    );
    let bad_private = Wanted {
        deleted: Vec::new(),
        file: json!({}),
        base: Value::Null,
        private: serde_json::from_value(json!({ "dev": { "a b": "x" } })).expect("patch"),
        renamed: BTreeMap::new(),
    };
    let failure = save(&ctx, &store, bad_private).expect_err("refused");
    assert_eq!(code(&failure), "plugin.api-client.errors.bad_variable");
}

#[test]
fn a_secret_value_is_set_for_one_environment_or_all() {
    let (dir, _store) = project();
    let ctx = context(dir.path());
    let one = set_secret(&ctx, "password", "pw-1", Some("staging")).expect("set");
    assert_eq!(
        one,
        json!({ "staging": { "token": "tok-secret-1", "password": "pw-1" } })
    );
    let all = set_secret(&ctx, "password", "pw-2", None).expect("set");
    assert_eq!(all["$shared"]["password"], "pw-2");
    let bad = set_secret(&ctx, "{{x}}", "v", None).expect_err("refused");
    assert_eq!(code(&bad), "plugin.api-client.errors.bad_variable");
}

#[test]
fn the_secrets_without_a_value_are_named() {
    let (dir, store) = project();
    let ctx = context(dir.path());
    let env = |name: &str| (name == "RB_SECRET_API_KEY").then(|| "k".to_owned());
    let names = vec![
        "token".to_owned(),
        "api-key".to_owned(),
        "password".to_owned(),
    ];
    let missing =
        missing_secrets(&ctx, &store, &names, Some("staging".into()), &env).expect("checked");
    assert_eq!(missing, ["password"]);
    let none =
        missing_secrets(&ctx, &store, &names, Some("production".into()), &no_env).expect("checked");
    assert_eq!(none, ["token", "api-key", "password"]);
}

/// Code review w33, item 3: an environment deleted and another renamed onto its name in one
/// save - the deletion goes first, so the rename is no clash.
#[test]
fn an_environment_deleted_and_another_renamed_onto_its_name_in_one_save() {
    let (dir, store) = project();
    let mut ctx = context(dir.path());
    ctx.secrets = json!({ "dev": { "a": "dev-1" }, "stage": { "b": "stage-2" } });
    let wanted = Wanted {
        file: json!({ "dev": {} }),
        base: Value::Null,
        private: BTreeMap::new(),
        renamed: BTreeMap::from([("stage".to_owned(), "dev".to_owned())]),
        deleted: vec!["dev".to_owned()],
    };
    let (private, _) = save(&ctx, &store, wanted).expect("saved");
    assert_eq!(private, json!({ "dev": { "b": "stage-2" } }));
    let deleted = Wanted {
        file: json!({}),
        base: Value::Null,
        private: BTreeMap::new(),
        renamed: BTreeMap::new(),
        deleted: vec!["dev".to_owned(), "stage".to_owned()],
    };
    assert_eq!(save(&ctx, &store, deleted).expect("saved").0, json!({}));
}
