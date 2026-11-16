use serde_json::json;

use super::*;
use crate::event::Offset;

fn target(element: ElementSelector) -> ElementTarget {
    ElementTarget {
        hosts: Vec::new(),
        element,
        offset: Offset { x: 0.5, y: 0.5 },
    }
}

#[test]
fn the_placeholder_names_the_secret() {
    assert_eq!(secret_placeholder("password"), "{{secret:password}}");
}

#[test]
fn candidates_rank_the_agent_selector_before_its_css_path() {
    let testid = ElementSelector::Testid {
        testid: "submit".into(),
        nth: 0,
        path: "#order > button".into(),
    };
    assert_eq!(
        candidates(&testid),
        vec![
            Selector::Testid {
                testid: "submit".into(),
                nth: 0
            },
            Selector::Css {
                path: "#order > button".into()
            },
        ]
    );
    let css = ElementSelector::Css {
        path: "#password".into(),
    };
    assert_eq!(
        candidates(&css),
        vec![Selector::Css {
            path: "#password".into()
        }]
    );
}

#[test]
fn a_locator_keeps_hosts_and_frames_and_finds_its_css_path() {
    let mut element = target(ElementSelector::Role {
        role: "button".into(),
        name: "Menu".into(),
        nth: 1,
        path: "#burger".into(),
    });
    element.hosts = vec![ElementSelector::Css {
        path: "#host".into(),
    }];
    let frame = vec!["iframe#pay".to_owned()];
    let locator = Locator::from_target(&element, Some(&frame));
    assert_eq!(locator.css_path(), "#burger");
    assert_eq!(
        locator.hosts,
        vec![Selector::Css {
            path: "#host".into()
        }]
    );
    assert_eq!(locator.frame, Some(frame));
    assert_eq!(Locator::from_target(&element, Some(&[])).frame, None);
}

#[test]
fn a_step_serialises_in_the_documented_shape() {
    let step = Step {
        id: 3,
        name: None,
        action: Action::Click {
            target: Locator {
                selectors: vec![
                    Selector::Role {
                        role: "button".into(),
                        name: "Menu".into(),
                        nth: 0,
                    },
                    Selector::Css {
                        path: "#burger".into(),
                    },
                ],
                hosts: Vec::new(),
                frame: None,
            },
            button: None,
            modifiers: 0,
        },
        view: "mobile".into(),
        at_ms: 1200,
        only_in: vec!["mobile".into()],
    };
    assert_eq!(
        serde_json::to_value(&step).expect("serialise"),
        json!({
            "id": 3,
            "action": {
                "type": "click",
                "target": { "selectors": [
                    { "kind": "role", "role": "button", "name": "Menu" },
                    { "kind": "css", "path": "#burger" }
                ] }
            },
            "view": "mobile",
            "atMs": 1200,
            "onlyIn": ["mobile"]
        })
    );
}

#[test]
fn wait_for_url_and_assertions_are_camel_case() {
    let wait = serde_json::to_value(Action::WaitForUrl {
        url: "https://x.test/done".into(),
    })
    .expect("serialise");
    assert_eq!(wait["type"], "waitForUrl");
    let assert = serde_json::to_value(Action::Assert {
        assertion: Assertion::Url {
            url: "https://x.test/".into(),
        },
    })
    .expect("serialise");
    assert_eq!(
        assert,
        json!({ "type": "assert", "assertion": { "kind": "url", "url": "https://x.test/" } })
    );
}

#[test]
fn unknown_fields_are_refused() {
    let text = r#"{"version":1,"name":"a","startedAtMs":0,"appVersion":"0.1.0","extra":1}"#;
    assert!(serde_json::from_str::<Recording>(text).is_err());
}
