use super::{Action, LocatorPreference, ParseError, Selector, parse_request};

const BARE: &str = r##"{
  "version": 1,
  "name": "login",
  "startUrl": "http://127.0.0.1:8000/recorder.html",
  "breakpoints": [{ "id": "mobile", "width": 390, "height": 844, "deviceScaleFactor": 3 }],
  "startedAtMs": 1,
  "appVersion": "0.5.0",
  "steps": [
    { "id": 1, "action": { "type": "navigate", "url": "http://127.0.0.1:8000/recorder.html" },
      "view": "mobile", "atMs": 0 },
    { "id": 2, "action": { "type": "click", "target": { "selectors": [
        { "kind": "testid", "testid": "go" }, { "kind": "css", "path": "#go" } ] } },
      "view": "mobile", "atMs": 5, "onlyIn": ["mobile"] }
  ]
}"##;

#[test]
fn a_bare_recording_gets_the_default_options() {
    let request = parse_request(BARE).expect("a recording");
    assert_eq!(request.recording.name, "login");
    assert_eq!(request.recording.steps.len(), 2);
    assert_eq!(request.options.locator_preference, LocatorPreference::Auto);
    assert!(request.options.projects.is_empty());
    let Action::Click { target, .. } = &request.recording.steps[1].action else {
        panic!("a click");
    };
    assert_eq!(
        target.selectors[0],
        Selector::Testid {
            testid: "go".to_owned(),
            nth: 0
        }
    );
    assert_eq!(request.recording.steps[1].only_in, ["mobile"]);
}

#[test]
fn the_hosts_request_carries_options_and_projects() {
    let input = format!(
        r#"{{ "recording": {BARE}, "options": {{ "locatorPreference": "css",
            "baseUrl": "http://127.0.0.1:8000",
            "projects": [{{ "name": "mobile", "width": 390, "height": 844, "isMobile": true }}] }} }}"#
    );
    let request = parse_request(&input).expect("a request");
    assert_eq!(request.options.locator_preference, LocatorPreference::Css);
    assert_eq!(
        request.options.base_url.as_deref(),
        Some("http://127.0.0.1:8000")
    );
    assert_eq!(request.options.projects[0].is_mobile, Some(true));
}

#[test]
fn another_format_version_is_refused_by_number() {
    let input = BARE.replace("\"version\": 1", "\"version\": 2");
    assert!(matches!(parse_request(&input), Err(ParseError::Version(2))));
}

#[test]
fn what_is_no_recording_is_refused() {
    assert!(matches!(parse_request("{}"), Err(ParseError::Invalid(_))));
    assert!(matches!(parse_request("nope"), Err(ParseError::Invalid(_))));
    assert!(matches!(
        parse_request(
            r#"{ "version": 1, "name": "x", "steps": [{ "id": 1, "action": { "type": "teleport" } }] }"#
        ),
        Err(ParseError::Invalid(_))
    ));
}

/// RB-092-01: a step switched off in the editor is neither exported nor played: the request
/// leaves it out, whatever the format.
#[test]
fn a_step_switched_off_is_left_out_of_the_request() {
    let text = BARE.replace(
        r#""atMs": 5, "onlyIn""#,
        r#""atMs": 5, "disabled": true, "onlyIn""#,
    );
    let request = parse_request(&text).expect("a recording");
    let ids: Vec<u32> = request.recording.steps.iter().map(|step| step.id).collect();
    assert_eq!(ids, [1]);
    let wrapped = format!(r#"{{ "recording": {text} }}"#);
    assert_eq!(
        parse_request(&wrapped)
            .expect("a request")
            .recording
            .steps
            .len(),
        1
    );
}
