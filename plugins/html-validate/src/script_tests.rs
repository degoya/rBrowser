use serde_json::{Map, Value, json};

use super::{MAX_MESSAGES, NU_TIMEOUT_MS, RUNNER_SOURCE, Request, script};

fn options_of(script: &str) -> Value {
    let start = script.find("rbHtmlValidate.start(").expect("the call") + 21;
    let end = script.rfind(");\n})()").expect("the end");
    serde_json::from_str(&script[start..end]).expect("the options are JSON")
}

#[test]
fn the_script_wraps_the_runner_and_one_call() {
    let rules: Map<String, Value> = [("no-dup-id".to_owned(), json!("error"))]
        .into_iter()
        .collect();
    let text = script(1, &Request::builtin("<p>", &rules)).expect("script");
    assert!(text.starts_with("(() => {\n"));
    assert!(text.contains(RUNNER_SOURCE));
    assert!(text.ends_with("})()"));
    assert_eq!(
        options_of(&text),
        json!({
            "engine": "builtin",
            "source": "<p>",
            "rules": { "no-dup-id": "error" },
            "maxMessages": MAX_MESSAGES,
            "maxText": 300,
        })
    );
}

#[test]
fn the_nu_request_names_the_endpoint_and_the_timeout() {
    let text = script(1, &Request::nu("http://localhost:8888/?out=json", "<p>")).expect("script");
    assert_eq!(
        options_of(&text),
        json!({
            "engine": "nu",
            "endpoint": "http://localhost:8888/?out=json",
            "source": "<p>",
            "maxMessages": MAX_MESSAGES,
            "maxText": 300,
            "timeoutMs": NU_TIMEOUT_MS,
        })
    );
}

#[test]
fn the_source_cannot_leave_its_string() {
    let rules = Map::new();
    let source = "</script>\"); alert(1); (\"\u{2028}";
    let text = script(1, &Request::builtin(source, &rules)).expect("script");
    assert_eq!(options_of(&text)["source"], json!(source));
}

#[test]
fn a_script_over_the_host_limit_is_too_large() {
    let rules = Map::new();
    // Every quote doubles in JSON: 2.5 MiB of them is over 4 MiB.
    let source = "\"".repeat(2_500_000);
    let failure = script(3, &Request::builtin(&source, &rules)).expect_err("too large");
    assert_eq!(
        failure.code(),
        Some("plugin.html-validate.errors.too_large")
    );
    assert_eq!(failure.param("view"), Some("3"));
    assert_eq!(failure.param("limit"), Some("4096"));
}
