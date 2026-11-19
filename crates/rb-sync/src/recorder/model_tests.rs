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

/// RB-090-81: the agent's index among the visible matches reaches the recording, so the
/// export can take the match the person saw; without it the recording reads as before.
#[test]
fn the_visible_index_of_a_role_or_text_match_is_recorded() {
    let agent: ElementSelector = serde_json::from_value(json!({
        "kind": "role", "role": "link", "name": "Read more", "nth": 4, "visibleNth": 3,
        "path": "main > article:nth-of-type(5) > a"
    }))
    .expect("the agent's selector");
    let recorded = candidates(&agent);
    assert_eq!(
        serde_json::to_value(&recorded[0]).expect("serialise"),
        json!({ "kind": "role", "role": "link", "name": "Read more", "nth": 4, "visibleNth": 3 })
    );
    let text: ElementSelector = serde_json::from_value(json!({
        "kind": "text", "tag": "span", "text": "Details", "nth": 2, "visibleNth": 0, "path": "p > span"
    }))
    .expect("a text selector");
    assert_eq!(
        serde_json::to_value(&candidates(&text)[0]).expect("serialise"),
        json!({ "kind": "text", "tag": "span", "text": "Details", "nth": 2, "visibleNth": 0 })
    );
    let old: Selector =
        serde_json::from_value(json!({ "kind": "role", "role": "link", "name": "More", "nth": 1 }))
            .expect("a recording without the field");
    assert_eq!(
        old,
        Selector::Role {
            role: "link".into(),
            name: "More".into(),
            nth: 1,
            visible_nth: None,
            within: None,
        }
    );
}

#[test]
fn a_selector_keeps_the_ancestor_that_makes_it_unique() {
    // RB-090-84: the agent's `within` goes into the recording; one without reads as before.
    let agent: ElementSelector = serde_json::from_value(json!({
        "kind": "role", "role": "link", "name": "Brands & Produkte", "within": "#main-nav",
        "path": "#main-nav > ul > li:nth-of-type(6) > div > a"
    }))
    .expect("a role selector");
    let recorded = candidates(&agent);
    assert_eq!(
        serde_json::to_value(&recorded[0]).expect("serialise"),
        json!({ "kind": "role", "role": "link", "name": "Brands & Produkte", "within": "#main-nav" })
    );
    let text: ElementSelector = serde_json::from_value(json!({
        "kind": "text", "tag": "span", "text": "Offers", "within": "main", "path": "main > p > span"
    }))
    .expect("a text selector");
    assert_eq!(
        serde_json::to_value(&candidates(&text)[0]).expect("serialise"),
        json!({ "kind": "text", "tag": "span", "text": "Offers", "within": "main" })
    );
}

#[test]
fn a_locator_keeps_hosts_and_frames_and_finds_its_css_path() {
    let mut element = target(ElementSelector::Role {
        role: "button".into(),
        name: "Menu".into(),
        nth: 1,
        visible_nth: None,
        within: None,
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
                        visible_nth: None,
                        within: None,
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

#[test]
fn a_wait_for_element_step_reads_and_writes_as_wait_for() {
    // The page menu's "Wait for element" (RB-090-60).
    let text = json!({
        "type": "waitFor",
        "target": { "selectors": [{ "kind": "css", "path": "#done" }] }
    });
    let action: Action = serde_json::from_value(text.clone()).expect("a waitFor step");
    assert_eq!(
        action,
        Action::WaitFor {
            target: Locator {
                selectors: vec![Selector::Css {
                    path: "#done".into()
                }],
                hosts: Vec::new(),
                frame: None,
            }
        }
    );
    assert_eq!(serde_json::to_value(&action).expect("serialises"), text);
}
