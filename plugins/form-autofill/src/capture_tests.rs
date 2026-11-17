use super::*;
use crate::fake::FakeHost;
use serde_json::json;

/// A sign-up form as the page script's `capture` reads it: the values filled by hand, the
/// password and the file without theirs.
fn signup() -> Value {
    json!({
        "url": "https://shop.example.com/register?step=2",
        "title": "Register - Shop",
        "scope": "focused",
        "form": { "selector": "#signup", "name": "Sign up" },
        "fields": [
            { "index": 0, "key": "input:email:email", "tag": "input", "type": "email",
              "name": "email", "label": "E-mail", "value": "linda@example.com" },
            { "index": 1, "key": "input:password:pw", "tag": "input", "type": "password",
              "name": "pw", "excluded": "password" },
            { "index": 2, "key": "input:file:avatar", "tag": "input", "type": "file",
              "name": "avatar", "excluded": "file" },
            { "index": 3, "key": "input:text:3", "tag": "input", "type": "text",
              "id": "nick", "label": "Nickname", "value": "" },
            { "index": 4, "key": "input:checkbox:terms", "tag": "input", "type": "checkbox",
              "name": "terms", "value": "true" },
            { "index": 5, "key": "input:text:4", "tag": "input", "type": "text", "value": "orphan" },
            { "index": 6, "key": "input:text:pin", "tag": "input", "type": "text",
              "name": "pin", "value": "1234" },
            { "index": 7, "key": "input:text:email:1", "tag": "input", "type": "text",
              "name": "email", "value": "again@example.com" },
            { "index": 8, "key": "select:select:tags", "tag": "select", "type": "select",
              "name": "tags", "excluded": "multiple" }
        ]
    })
}

fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "Mobile", "chromium")
        .view(2, "Desktop", "chromium")
        .view(3, "Firefox", "firefox")
        .collect(1, signup())
        .collect(2, signup());
    host
}

fn run(host: &mut FakeHost, args: Value) -> Value {
    capture(host, &CaptureArgs::parse(&args.to_string()).expect("args")).expect("captures")
}

fn by_locator<'a>(answer: &'a Value, locator: &str) -> &'a Value {
    answer["fields"]
        .as_array()
        .and_then(|fields| fields.iter().find(|field| field["locator"] == locator))
        .unwrap_or_else(|| panic!("no field {locator}"))
}

#[test]
fn a_form_becomes_entries_by_the_locators_the_fill_uses() {
    let mut host = host();
    let answer = run(&mut host, json!({ "target": 2 }));
    assert_eq!(answer["view"], 2);
    assert_eq!(answer["name"], "Sign up");
    assert_eq!(answer["form"]["selector"], "#signup");
    assert_eq!(answer["scope"], "focused");
    let email = by_locator(&answer, "email");
    assert_eq!(email["value"], "linda@example.com");
    assert_eq!(email["selected"], true);
    assert_eq!(email["kind"], "email");
    assert_eq!(email["label"], "E-mail");
    // No name: the id is the locator; an empty value is offered, not chosen.
    let nick = by_locator(&answer, "nick");
    assert_eq!(nick["value"], "");
    assert_eq!(nick["selected"], false);
    assert_eq!(by_locator(&answer, "terms")["value"], "true");
    // The request asked for the values, not for every hidden field to be skipped.
    let request = &host.requests(2)[0];
    assert_eq!(request["op"], "capture");
    assert_eq!(request["hiddenInputs"], false);
    assert_eq!(request["skipHidden"], false);
}

#[test]
fn passwords_secrets_and_files_are_never_taken_and_say_why() {
    let mut host = host();
    let answer = run(&mut host, json!({}));
    let fields = answer["fields"].as_array().expect("fields");
    let reasons: Vec<(&str, &str)> = fields
        .iter()
        .filter_map(|field| Some((field["label"].as_str()?, field["excluded"].as_str()?)))
        .collect();
    assert!(reasons.contains(&("pw", "password")), "{reasons:?}");
    assert!(reasons.contains(&("avatar", "file")), "{reasons:?}");
    assert!(reasons.contains(&("pin", "secret")), "{reasons:?}");
    assert!(reasons.contains(&("tags", "multiple")), "{reasons:?}");
    let kept_out = ["password", "secret", "file", "multiple"];
    for field in fields
        .iter()
        .filter(|field| kept_out.iter().any(|reason| field["excluded"] == *reason))
    {
        assert!(field.get("value").is_none(), "{field}");
        assert_eq!(field["selected"], false);
    }
    // The page sent the PIN's value; this side drops it again.
    assert!(!answer.to_string().contains("1234"));
}

#[test]
fn a_field_without_a_locator_or_with_a_taken_one_is_left_out() {
    let mut host = host();
    let answer = run(&mut host, json!({}));
    let fields = answer["fields"].as_array().expect("fields");
    let orphan = fields
        .iter()
        .find(|field| field["excluded"] == "no-locator")
        .expect("the field without name or id");
    assert!(
        orphan.get("value").is_some(),
        "a value without a locator is shown"
    );
    let again = fields
        .iter()
        .find(|field| field["excluded"] == "duplicate")
        .expect("the second field named email");
    assert_eq!(again["locator"], "email");
    assert_eq!(by_locator(&answer, "email")["value"], "linda@example.com");
}

#[test]
fn the_active_view_is_captured_without_a_target_and_a_firefox_view_is_refused() {
    let mut host = host();
    let answer = run(&mut host, json!({ "target": "all" }));
    assert_eq!(answer["view"], 1, "the active view");
    let failure = capture(
        &mut host,
        &CaptureArgs::parse(r#"{"target":3}"#).expect("args"),
    )
    .expect_err("firefox");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.form-autofill.errors.capture_unsupported")
    );
    let failure = capture(
        &mut host,
        &CaptureArgs::parse(r#"{"target":9}"#).expect("args"),
    )
    .expect_err("no view");
    assert_eq!(failure.code.as_deref(), Some(codes::NOT_FOUND));
}

#[test]
fn hidden_inputs_are_asked_for_only_with_the_setting() {
    let mut host = host();
    host.set("captureHiddenInputs", "true");
    run(&mut host, json!({ "target": 1 }));
    assert_eq!(host.requests(1)[0]["hiddenInputs"], true);
}

#[test]
fn the_answer_names_the_stored_presets_and_the_settings_profiles() {
    let mut host = host();
    host.set(
        "profiles",
        r#"[{"name":"testuser","values":{"email":"a@b.c"}}]"#,
    );
    crate::store::save(
        &mut host,
        &crate::store::SaveArgs::parse(r#"{"name":"Sign up","values":{"email":"x@y.z"}}"#)
            .expect("args"),
    )
    .expect("saved");
    let answer = run(&mut host, json!({}));
    assert_eq!(answer["presets"], json!(["Sign up"]));
    assert_eq!(answer["profiles"], json!(["testuser"]));
    assert_eq!(answer["available"], true);
}

#[test]
fn a_selector_that_finds_nothing_fails_in_words() {
    let mut host = FakeHost::default();
    host.view(1, "Mobile", "chromium")
        .collect(1, json!({ "error": "not-found", "fields": [] }));
    let failure = capture(
        &mut host,
        &CaptureArgs::parse(r##"{"selector":"#x"}"##).expect("args"),
    )
    .expect_err("nothing");
    assert_eq!(failure.code.as_deref(), Some(codes::NOT_FOUND));
    assert_eq!(failure.param("selector"), Some("#x"));
    assert_eq!(host.requests(1)[0]["selector"], "#x");
}

#[test]
fn a_pending_request_captures_its_view_once() {
    let mut host = host();
    host.now = 500;
    crate::store::request_capture(&mut host, 2).expect("noted");
    let answer = run(&mut host, json!({ "pending": true }));
    assert_eq!(answer["view"], 2);
    assert_eq!(answer["pending"], true);
    let answer = run(&mut host, json!({ "pending": true }));
    assert_eq!(answer, json!({ "pending": false }));
    assert!(
        host.requests(1).is_empty(),
        "nothing evaluated without a request"
    );
}

#[test]
fn a_named_view_clears_a_pending_request_too() {
    let mut host = host();
    crate::store::request_capture(&mut host, 2).expect("noted");
    let answer = run(&mut host, json!({ "pending": true, "target": 1 }));
    assert_eq!(answer["view"], 1, "the post's view wins");
    assert_eq!(
        run(&mut host, json!({ "pending": true })),
        json!({ "pending": false })
    );
}

#[test]
fn the_command_notes_the_view_and_opens_the_panel() {
    let mut host = host();
    let answer = command(
        &mut host,
        &CaptureArgs::parse(r#"{"target":2}"#).expect("args"),
    )
    .expect("asks");
    assert_eq!(answer["$open"]["panel"], "autofill");
    assert_eq!(answer["$post"][0]["target"], "panel");
    assert_eq!(
        answer["$post"][0]["message"],
        json!({ "type": "capture", "view": 2 })
    );
    assert_eq!(
        host.out_json(crate::store::REQUEST_FILE).expect("noted")["view"],
        2
    );
    assert!(
        host.evaluated.is_empty(),
        "the panel captures, not the command"
    );
    // Without a project the panel still hears the post.
    host.no_project = true;
    let answer = command(&mut host, &CaptureArgs::parse("{}").expect("args")).expect("asks");
    assert_eq!(answer["$post"][0]["message"]["view"], 1);
}

#[test]
fn bad_arguments_are_named() {
    for (args, reason) in [
        (r#"{"pending":"yes"}"#, "pending"),
        (r#"{"selector":7}"#, "selector"),
        (r#"{"target":"some"}"#, "target"),
        ("[]", "args"),
    ] {
        let failure = CaptureArgs::parse(args).expect_err("refused");
        assert_eq!(failure.param("reason"), Some(reason), "{args}");
    }
    assert_eq!(
        CaptureArgs::parse("").expect("empty"),
        CaptureArgs::default()
    );
}

#[test]
fn the_suggested_name_falls_back_to_the_title_and_the_host() {
    let mut page = signup();
    page["form"] = Value::Null;
    assert_eq!(suggested_name(&page), "Register - Shop");
    page["title"] = json!("  ");
    assert_eq!(suggested_name(&page), "shop.example.com");
    page["url"] = json!("about:blank");
    assert_eq!(suggested_name(&page), "about:blank");
    page["form"] = json!({ "name": "x".repeat(120) });
    assert_eq!(suggested_name(&page).chars().count(), 80);
}

#[test]
fn a_store_that_does_not_read_is_named_not_taken_for_no_project() {
    // Review finding 3: the dialog says why it cannot save.
    let mut host = host();
    host.files
        .insert(crate::store::READ_PATH.to_owned(), b"not json".to_vec());
    let answer = run(&mut host, json!({}));
    assert_eq!(answer["available"], false);
    assert_eq!(
        answer["storeError"]["code"],
        "plugin.form-autofill.errors.presets_unreadable"
    );
    host.no_project = true;
    let answer = run(&mut host, json!({}));
    assert_eq!(answer["storeError"]["code"], codes::NOT_AVAILABLE);
    host.no_project = false;
    host.files.clear();
    let answer = run(&mut host, json!({}));
    assert_eq!(answer["available"], true);
    assert!(answer["storeError"].is_null());
}
