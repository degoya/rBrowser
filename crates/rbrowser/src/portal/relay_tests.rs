use rb_sync::{
    ElementSelector, ElementTarget, HoverData, InputData, MouseButton, MouseData, Offset,
};
use serde_json::json;

use super::*;

fn target() -> ElementTarget {
    ElementTarget {
        hosts: Vec::new(),
        element: ElementSelector::Css {
            path: "#go".to_owned(),
        },
        offset: Offset { x: 0.5, y: 0.5 },
    }
}

fn click() -> SyncEvent {
    SyncEvent::Click(MouseData {
        control: None,
        button: MouseButton::Left,
        detail: 1,
        modifiers: 0,
        target: target(),
    })
}

fn on() -> SyncSwitches {
    SyncSwitches::default()
}

#[test]
fn a_click_goes_to_the_device_as_it_is() {
    let message = for_device(&click(), &on()).expect("replayable");
    let json = serde_json::to_value(&message).expect("serialises");
    assert_eq!(json["kind"], "event");
    assert_eq!(json["event"]["type"], "click");
}

#[test]
fn hover_keys_and_pointers_cannot_be_replayed_by_script() {
    let hover = SyncEvent::Hover(HoverData {
        target: target(),
        menu: false,
    });
    assert_eq!(for_device(&hover, &on()), Err(Skip::NotReplayable));
}

#[test]
fn a_password_never_leaves_the_machine() {
    let password = SyncEvent::Input(InputData {
        input_type: "insertText".to_owned(),
        data: Some("x".to_owned()),
        value: Some("secret".to_owned()),
        sensitive: true,
        target: target(),
    });
    assert_eq!(for_device(&password, &on()), Err(Skip::Sensitive));
}

#[test]
fn a_kind_switched_off_is_not_sent() {
    let mut switches = on();
    switches.click = false;
    assert_eq!(for_device(&click(), &switches), Err(Skip::Off));
    let mut switches = on();
    switches.enabled = false;
    assert_eq!(for_device(&click(), &switches), Err(Skip::Off));
}

#[test]
fn navigation_carries_the_path_only() {
    let message = navigate("http://localhost:5173/a?b#c").expect("followable");
    assert_eq!(
        serde_json::to_value(&message).expect("serialises"),
        json!({ "kind": "navigate", "path": "/a?b#c" })
    );
    assert_eq!(navigate("about:blank"), None);
}

#[test]
fn the_device_reports_parse_and_junk_is_refused() {
    assert_eq!(
        parse_report(r#"{"kind":"replayed","status":"done"}"#),
        Some(Report::Replayed {
            status: ReplayStatus::Done
        })
    );
    assert_eq!(
        parse_report(r#"{"kind":"replayed","status":"unsupported"}"#),
        Some(Report::Replayed {
            status: ReplayStatus::Unsupported
        })
    );
    assert_eq!(parse_report(r#"{"kind":"hello"}"#), Some(Report::Hello));
    assert_eq!(parse_report(r#"{"kind":"navigate","url":"x"}"#), None);
    assert_eq!(parse_report(&"x".repeat(MAX_REPORT + 1)), None);
}

#[test]
fn counters_add_up() {
    let mut counts = Counts::default();
    counts.note(&Report::Replayed {
        status: ReplayStatus::Done,
    });
    counts.note(&Report::Replayed {
        status: ReplayStatus::NotFound,
    });
    counts.skipped(Skip::NotReplayable);
    assert_eq!((counts.replayed, counts.failed, counts.skipped), (1, 1, 1));
}

fn missed(origin: &str) -> Option<Report> {
    parse_report(&json!({ "kind": "missed", "origin": origin }).to_string())
}

/// RB-090-72: a device names an origin its page failed to load from - a bare origin as
/// `URL.origin` writes it, nothing else; a device is untrusted.
#[test]
fn a_missed_origin_is_taken_only_as_a_bare_origin() {
    for origin in [
        "http://localhost:5173",
        "http://127.0.0.1:9000",
        "http://[::1]:5173",
        "http://project.test",
        "https://cdn.example.com",
    ] {
        assert_eq!(
            missed(origin),
            Some(Report::Missed {
                origin: origin.to_owned(),
                via: None,
            }),
            "{origin}"
        );
    }
    for junk in [
        "http://localhost:5173/a.png",
        "http://localhost:5173?token=secret",
        "http://localhost:5173#x",
        "http://user:pass@localhost:5173",
        "HTTP://LOCALHOST:5173",
        "http://localhost:80",
        "http://localhost:99999",
        "http://localhost:x",
        "http://a:1:2",
        "http://",
        "ws://localhost:5173",
        "file:///etc/passwd",
        "null",
        "",
        "http://local host:5173",
        "http://localhost:5173\n",
    ] {
        assert_eq!(missed(junk), None, "{junk:?}");
    }
    let long = format!("http://{}.test", "a".repeat(MAX_ORIGIN));
    assert_eq!(missed(&long), None, "too long");
    assert_eq!(parse_report(r#"{"kind":"missed"}"#), None);
    assert_eq!(parse_report(r#"{"kind":"missed","origin":5}"#), None);
    assert_eq!(
        parse_report(&format!(
            r#"{{"kind":"missed","origin":"http://localhost:5173","pad":"{}"}}"#,
            "x".repeat(MAX_REPORT)
        )),
        None,
        "an oversized message is refused whole"
    );
}

#[test]
fn a_miss_counts_no_replay() {
    let mut counts = Counts::default();
    counts.note(&Report::Missed {
        origin: "http://localhost:5173".to_owned(),
        via: Some(Via::Img),
    });
    assert_eq!(counts, Counts::default());
}

/// RB-090-72: a miss may say how the page asked (an element or the resource timing's initiator
/// type); an unknown word is `other`, anything but a word refuses the report.
#[test]
fn a_miss_names_how_the_page_asked() {
    let with = |via: serde_json::Value| {
        parse_report(
            &json!({ "kind": "missed", "origin": "http://localhost:3000", "via": via }).to_string(),
        )
    };
    let via = |report: Option<Report>| match report {
        Some(Report::Missed { via, .. }) => Ok(via),
        other => Err(other),
    };
    assert_eq!(via(with(json!("img"))), Ok(Some(Via::Img)));
    assert_eq!(
        via(with(json!("xmlhttprequest"))),
        Ok(Some(Via::Xmlhttprequest))
    );
    assert_eq!(via(with(json!("css"))), Ok(Some(Via::Css)));
    assert_eq!(via(with(json!("beacon"))), Ok(Some(Via::Other)));
    assert_eq!(via(with(serde_json::Value::Null)), Ok(None));
    assert_eq!(with(json!(5)), None);
    assert_eq!(with(json!({ "x": 1 })), None);
    assert_eq!(Via::Fetch.as_str(), "fetch");
}
