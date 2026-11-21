use serde_json::json;

use super::*;

#[test]
fn text_without_secrets_comes_back_unchanged() {
    for text in [
        "navigated view 3 to https://example.test/shop?page=2",
        "max_tokens=50 key: F5 session: 3",
        "layout preset 2 applied",
    ] {
        assert_eq!(redact_text(text), text);
    }
}

#[test]
fn a_token_assignment_is_masked() {
    assert_eq!(
        redact_text("mcp listening token=abc123def port=7777"),
        "mcp listening token=*** port=7777"
    );
    assert_eq!(
        redact_text(r#"cdp proxy started token="s3cr3t-value""#),
        r#"cdp proxy started token="***""#
    );
    assert_eq!(redact_text("RB_MCP_TOKEN=xyz987"), "RB_MCP_TOKEN=***");
}

#[test]
fn json_style_secrets_are_masked() {
    assert_eq!(
        redact_text(r#"{"password": "hunter22", "user": "bob"}"#),
        r#"{"password": "***", "user": "bob"}"#
    );
}

#[test]
fn credential_header_lines_and_schemes_are_masked() {
    assert_eq!(
        redact_text("Authorization: Bearer abc.def"),
        "Authorization: ***"
    );
    assert_eq!(redact_text("Cookie: sid=1; theme=dark"), "Cookie: ***");
    assert_eq!(
        redact_text(r#"headers {"authorization": "Bearer abc.def"}"#),
        r#"headers {"authorization": "***"}"#
    );
    assert_eq!(
        redact_text(r#"request cookie="sid=1; theme=dark""#),
        r#"request cookie="***""#
    );
}

#[test]
fn a_bearer_value_in_prose_is_masked_but_basic_prose_stays() {
    assert_eq!(
        redact_text("sent Bearer abc.def now"),
        "sent Bearer *** now"
    );
    assert_eq!(redact_text("a basic layout"), "a basic layout");
}

#[test]
fn the_user_info_of_an_address_is_masked() {
    assert_eq!(
        redact_text("proxy http://bob:pa55@proxy.local:3128/ in use"),
        "proxy http://***@proxy.local:3128/ in use"
    );
    assert_eq!(
        redact_text("open https://example.test/cb?access_token=zz9&x=1"),
        "open https://example.test/cb?access_token=***&x=1"
    );
}

#[test]
fn redacting_twice_equals_redacting_once() {
    for text in [
        "token=abc Authorization: Bearer x",
        "proxy http://bob:pa55@proxy.local:3128/",
        r#"{"password": "hunter22"}"#,
        "Cookie: a=b",
    ] {
        let once = redact_text(text);
        assert_eq!(redact_text(&once), once);
    }
}

#[test]
fn known_secrets_are_masked_wherever_they_stand() {
    let secrets = Secrets::default();
    secrets.add("tok-4711-geheim");
    assert_eq!(
        secrets.redact("ws://127.0.0.1:9222/devtools/tok-4711-geheim/browser"),
        "ws://127.0.0.1:9222/devtools/***/browser"
    );
}

#[test]
fn a_removed_secret_is_no_longer_kept() {
    let secrets = Secrets::default();
    secrets.add("tok-run-1-0123456789");
    secrets.add("tok-run-2-0123456789");
    secrets.remove("tok-run-1-0123456789");
    assert_eq!(
        secrets.redact("tok-run-1-0123456789 tok-run-2-0123456789"),
        "tok-run-1-0123456789 ***"
    );
    assert_eq!(secrets.len(), 1);
}

#[test]
fn very_short_values_are_not_registered() {
    let secrets = Secrets::default();
    secrets.add("ab");
    assert_eq!(secrets.redact("a tab"), "a tab");
}

#[test]
fn the_global_set_masks_a_registered_token() {
    register_secret("global-token-0815");
    assert_eq!(redact("got global-token-0815"), "got ***");
}

#[test]
fn the_global_set_forgets_a_token_that_is_given_up() {
    register_secret("run-endpoint-key-4242");
    unregister_secret("run-endpoint-key-4242");
    assert_eq!(
        redact("got run-endpoint-key-4242"),
        "got run-endpoint-key-4242"
    );
}

#[test]
fn json_masks_secret_keys_headers_and_proxy_credentials() {
    let config = json!({
        "startUrl": "https://example.test",
        "breakpoints": [{ "id": "phone", "headers": { "X-Api": "hdr-0815", "Accept": "text/html" } }],
        "network": { "proxy": "http://bob:pw-1234@proxy:3128" },
        "mcp": { "token": "tok-9" },
        "count": 3
    });
    let redacted = redact_json(&config);
    assert_eq!(
        redacted.value,
        json!({
            "startUrl": "https://example.test",
            "breakpoints": [{ "id": "phone", "headers": { "X-Api": "***", "Accept": "***" } }],
            "network": { "proxy": "http://***@proxy:3128" },
            "mcp": { "token": "***" },
            "count": 3
        })
    );
    for removed in ["hdr-0815", "text/html", "bob:pw-1234", "pw-1234", "tok-9"] {
        assert!(
            redacted.removed.iter().any(|value| value == removed),
            "{removed} not recorded in {:?}",
            redacted.removed
        );
    }
}

#[test]
fn json_masks_non_string_values_under_a_secret_key() {
    let redacted = redact_json(&json!({ "secret": { "user": "bob", "pin": 1234 } }));
    assert_eq!(redacted.value, json!({ "secret": "***" }));
}

#[test]
fn json_keeps_ordinary_keys_that_look_like_secret_names() {
    let config = json!({ "breakpoints": [{ "session": "admin" }], "key": "F5" });
    assert_eq!(redact_json(&config).value, config);
}

#[test]
fn the_log_s_names_lie_inside_the_one_list() {
    // RB-094-10: the log rules keep their narrower lists (in prose `code=1` stays), every name of
    // them taken from `rb_util::mask`.
    use rb_util::mask::{sensitive_header, sensitive_param};
    for name in SECRET_NAMES.iter().chain(SECRET_PARTS).chain(SECRET_WORDS) {
        assert!(
            sensitive_param(name) || sensitive_header(name),
            "{name} is missing from rb_util::mask"
        );
    }
    for name in CREDENTIAL_HEADERS {
        assert!(
            sensitive_header(name),
            "{name} is missing from rb_util::mask"
        );
    }
}

#[test]
fn an_ios_device_s_udid_stays_out_of_the_zip_and_the_logs() {
    let udid = "00008110-001A2B3C4D5E801E";
    let config = json!({ "breakpoints": [{ "id": "phone", "engine": "ios", "iosDevice": udid }] });
    let redacted = redact_json(&config);
    assert_eq!(
        redacted.value,
        json!({ "breakpoints": [{ "id": "phone", "engine": "ios", "iosDevice": "***" }] })
    );
    assert_eq!(redacted.removed, [udid]);
    assert_eq!(
        redact_text(&format!(r#"{{"iosDevice": "{udid}"}}"#)),
        r#"{"iosDevice": "***"}"#
    );
}
