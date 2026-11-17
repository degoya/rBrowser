use super::*;

fn absolute(name: &str) -> String {
    if cfg!(windows) {
        format!("C:\\Tools\\{name}")
    } else {
        format!("/opt/tools/{name}")
    }
}

/// RB-080-58 review 2: an explicit Lighthouse browser that cannot work is refused when it is
/// saved, with a sentence, not only noticed at the sidecar's next start.
#[test]
fn the_lighthouse_browser_must_be_an_existing_absolute_file() {
    let chrome = absolute("chrome.exe");
    let is_file = |path: &Path| path == Path::new(&chrome);
    let key = LIGHTHOUSE_BROWSER_KEY;
    assert_eq!(refusal(key, &Value::String(chrome.clone()), is_file), None);
    assert_eq!(
        refusal(key, &Value::String(format!("  {chrome} ")), is_file),
        None
    );
    assert_eq!(
        refusal(key, &Value::from("chrome.exe"), is_file),
        Some(reason::NOT_ABSOLUTE)
    );
    assert_eq!(
        refusal(key, &Value::String(absolute("gone.exe")), is_file),
        Some(reason::NOT_A_FILE)
    );
    // Empty and reset are allowed: the sidecar finds a browser itself then.
    assert_eq!(refusal(key, &Value::from(""), is_file), None);
    assert_eq!(refusal(key, &Value::Null, is_file), None);
    // Other keys are not this check's.
    assert_eq!(
        refusal("engines.nodeRpcTimeoutMs", &Value::from("x"), is_file),
        None
    );
}

#[test]
fn the_refusals_have_texts() {
    for key in [reason::NOT_ABSOLUTE, reason::NOT_A_FILE] {
        let part: serde_json::Value =
            serde_json::from_str(include_str!("../../../../locales/en.json"))
                .expect("the catalogue parses");
        let mut node = &part;
        for segment in key.split('.') {
            node = &node[segment];
        }
        assert!(node.is_string(), "{key}");
    }
}
