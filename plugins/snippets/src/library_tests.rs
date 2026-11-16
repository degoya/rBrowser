use serde_json::{Value, json};

use super::save;
use crate::catalog::LIBRARY_PATH;
use crate::fake_host::FakeHost;

fn saved(host: &FakeHost) -> Value {
    serde_json::from_slice(host.files.get(LIBRARY_PATH).expect("written")).expect("JSON")
}

#[test]
fn a_snippet_is_saved_replaced_and_deleted() {
    let mut host = FakeHost::with_views(Vec::new());
    let answer = save(&mut host, &json!({ "name": "Grid", "code": "return 1" })).expect("saved");
    assert_eq!(answer["saved"], "Grid");
    assert_eq!(answer["$post"][0]["target"], "panel");
    save(&mut host, &json!({ "name": "Other", "code": "return 2" })).expect("saved");
    save(&mut host, &json!({ "name": "Grid", "code": "return 3" })).expect("replaced");
    let library = saved(&host);
    assert_eq!(library["version"], 1);
    assert_eq!(
        library["snippets"],
        json!([{ "name": "Grid", "code": "return 3" }, { "name": "Other", "code": "return 2" }])
    );
    let answer = save(&mut host, &json!({ "name": "Grid", "delete": true })).expect("deleted");
    assert_eq!(answer["deleted"], "Grid");
    assert_eq!(
        saved(&host)["snippets"],
        json!([{ "name": "Other", "code": "return 2" }])
    );
    let failure = save(&mut host, &json!({ "name": "Grid", "delete": true })).expect_err("gone");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.snippets.errors.not_found")
    );
}

#[test]
fn nothing_is_saved_in_an_untrusted_workspace_or_without_a_project() {
    let mut host = FakeHost::with_views(Vec::new());
    host.trusted = false;
    let failure = save(&mut host, &json!({ "name": "Grid", "code": "1" })).expect_err("untrusted");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.snippets.errors.save_untrusted")
    );
    host.no_project = true;
    let failure = save(&mut host, &json!({ "name": "Grid", "code": "1" })).expect_err("no project");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.snippets.errors.no_project")
    );
    assert!(host.files.is_empty());
}

#[test]
fn what_does_not_read_is_refused() {
    let mut host = FakeHost::with_views(Vec::new());
    for (args, code) in [
        (json!({ "code": "1" }), "plugin.snippets.errors.bad_args"),
        (
            json!({ "name": "  ", "code": "1" }),
            "plugin.snippets.errors.bad_args",
        ),
        (json!({ "name": "a" }), "plugin.snippets.errors.bad_args"),
        (
            json!({ "name": "a", "code": " \n" }),
            "plugin.snippets.errors.empty",
        ),
        (
            json!({ "name": "a", "code": "// @world page\n1" }),
            "plugin.snippets.errors.header",
        ),
        (
            json!({ "name": "a", "code": "x".repeat(crate::catalog::MAX_CODE_CHARS + 1) }),
            "plugin.snippets.errors.code_too_long",
        ),
    ] {
        let failure = save(&mut host, &args).expect_err("refused");
        assert_eq!(failure.code.as_deref(), Some(code), "{args}");
    }
    assert!(host.files.is_empty());
}

#[test]
fn a_full_library_takes_no_new_name() {
    let mut host = FakeHost::with_views(Vec::new());
    let entries: Vec<Value> = (0..crate::catalog::MAX_PER_SOURCE)
        .map(|n| json!({ "name": format!("s{n}"), "code": "1" }))
        .collect();
    host.file(LIBRARY_PATH, &json!({ "snippets": entries }).to_string());
    let failure = save(&mut host, &json!({ "name": "new", "code": "1" })).expect_err("full");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.snippets.errors.library_full")
    );
    save(&mut host, &json!({ "name": "s0", "code": "2" })).expect("a known name is replaced");
}
