use serde_json::json;

use super::*;

#[test]
fn the_commands_carry_the_parameters_firefox_takes() {
    assert_eq!(
        create_window(),
        Command::call("browsingContext.create", json!({ "type": "window" }))
    );
    assert_eq!(
        navigate("c1", "http://x/", Wait::None).params,
        json!({ "context": "c1", "url": "http://x/", "wait": "none" })
    );
    assert_eq!(
        navigate("c1", "http://x/", Wait::Interactive).params["wait"],
        "interactive"
    );
    assert_eq!(
        reload("c1", true).params,
        json!({ "context": "c1", "ignoreCache": true, "wait": "none" })
    );
    assert_eq!(
        traverse_history("c1", -1).params,
        json!({ "context": "c1", "delta": -1 })
    );
    assert_eq!(
        set_viewport("c1", 393, 852, 3.0).params,
        json!({ "context": "c1", "viewport": { "width": 393, "height": 852 }, "devicePixelRatio": 3.0 })
    );
    // A DPR of 0 leaves the ratio to the display (RB-080-38): Firefox 160 refuses 0.
    assert_eq!(
        set_viewport("c1", 393, 852, 0.0).params,
        json!({ "context": "c1", "viewport": { "width": 393, "height": 852 } })
    );
    assert_eq!(
        capture_screenshot("c1", false).params,
        json!({ "context": "c1", "origin": "viewport" })
    );
    assert_eq!(capture_screenshot("c1", true).params["origin"], "document");
    assert_eq!(
        get_tree(Some("c1")).params,
        json!({ "root": "c1", "maxDepth": 0 })
    );
    assert_eq!(get_tree(None).params, json!({ "maxDepth": 0 }));
    assert_eq!(activate("c1").method, "browsingContext.activate");
    assert_eq!(
        close("c1").params,
        json!({ "context": "c1", "promptUnload": false })
    );
}

#[test]
fn answers_are_read_or_refused_by_name() {
    assert_eq!(context_of(&json!({ "context": "c9" })), Ok("c9".to_owned()));
    assert!(matches!(
        context_of(&json!({})),
        Err(BidiError::Protocol { .. })
    ));
    let tree = json!({ "contexts": [
        { "context": "a", "clientWindow": "w1", "url": "about:blank", "children": [] },
        { "context": "b", "clientWindow": "w2", "url": "about:blank", "children": [] }
    ]});
    assert_eq!(top_contexts(&tree), vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(client_window(&tree), Some("w1".to_owned()));
    assert_eq!(client_window(&json!({ "contexts": [] })), None);
    assert_eq!(
        screenshot_data(&json!({ "data": "iVBOR" })),
        Ok("iVBOR".to_owned())
    );
    assert!(screenshot_data(&json!({ "data": "" })).is_err());
}

#[test]
fn navigation_events_name_context_and_url() {
    for method in NAVIGATION_EVENTS {
        let params =
            json!({ "context": "c1", "url": "http://x/#a", "navigation": null, "timestamp": 1 });
        assert_eq!(
            navigated(method, &params),
            Some(("c1".to_owned(), "http://x/#a".to_owned()))
        );
    }
    assert_eq!(
        navigated(
            "browsingContext.load",
            &json!({ "context": "c1", "url": "x" })
        ),
        None
    );
    assert_eq!(
        destroyed(
            CONTEXT_DESTROYED,
            &json!({ "context": "c1", "parent": null })
        ),
        Some("c1".to_owned())
    );
    assert_eq!(
        destroyed(
            CONTEXT_DESTROYED,
            &json!({ "context": "f1", "parent": "c1" })
        ),
        None,
        "an iframe going is not the view's end"
    );
}

#[test]
fn a_window_opens_in_the_user_context_of_its_session() {
    assert_eq!(
        create_window_in(Some("uc-1")).params,
        json!({ "type": "window", "userContext": "uc-1" })
    );
    assert_eq!(create_window_in(None).params, create_window().params);
}
