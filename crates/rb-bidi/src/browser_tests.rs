use serde_json::json;

use super::*;

#[test]
fn the_session_asks_for_nothing_insecure() {
    let command = session_new();
    assert_eq!(command.method, "session.new");
    let always = &command.params["capabilities"]["alwaysMatch"];
    assert_eq!(always["acceptInsecureCerts"], false);
    assert_eq!(always["unhandledPromptBehavior"]["default"], "ignore");
}

#[test]
fn the_subscription_names_the_events() {
    assert_eq!(
        subscribe(&["script.message"]).params,
        json!({ "events": ["script.message"] })
    );
}

#[test]
fn the_window_goes_where_it_is_told() {
    let rect = WindowRect {
        x: -10,
        y: 20,
        width: 400,
        height: 900,
    };
    assert_eq!(
        set_client_window_state("w1", rect).params,
        json!({ "clientWindow": "w1", "state": "normal", "x": -10, "y": 20, "width": 400, "height": 900 })
    );
}

#[test]
fn the_version_is_read_and_checked() {
    let answer = json!({ "sessionId": "s", "capabilities": { "browserName": "firefox", "browserVersion": "159.0.1" } });
    assert_eq!(browser_version(&answer), Some("159.0.1".to_owned()));
    assert_eq!(check_version("159.0.1", 140), Ok(()));
    assert_eq!(check_version("140.0esr", 140), Ok(()));
    assert_eq!(
        check_version("128.5.0", 140),
        Err(BidiError::TooOld {
            found: "128.5.0".to_owned(),
            min: 140,
        })
    );
    assert!(
        check_version("nightly", 140).is_ok(),
        "an unreadable version is given the benefit of the doubt"
    );
}
