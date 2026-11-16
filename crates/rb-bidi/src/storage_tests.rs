use serde_json::json;

use super::*;

fn cookie(expiry: Option<u64>) -> Cookie {
    Cookie {
        name: "sid".to_owned(),
        value: "hunter2".to_owned(),
        domain: "localhost".to_owned(),
        path: "/".to_owned(),
        http_only: true,
        secure: false,
        same_site: "lax".to_owned(),
        expiry,
    }
}

#[test]
fn a_cookie_is_set_with_its_flags() {
    assert_eq!(
        set_cookie(&cookie(Some(42))).params,
        json!({ "cookie": {
            "name": "sid",
            "value": { "type": "string", "value": "hunter2" },
            "domain": "localhost",
            "path": "/",
            "httpOnly": true,
            "secure": false,
            "sameSite": "lax",
            "expiry": 42,
        }})
    );
    assert!(
        set_cookie(&cookie(None)).params["cookie"]
            .get("expiry")
            .is_none()
    );
}

#[test]
fn the_value_is_never_shown() {
    let shown = format!("{:?} {:?}", cookie(None), set_cookie(&cookie(None)));
    assert!(!shown.contains("hunter2"), "{shown}");
}

#[test]
fn a_cookie_goes_into_the_user_context_of_its_session() {
    assert_eq!(
        set_cookie_in(&cookie(None), Some("uc-1")).params["partition"],
        json!({ "type": "storageKey", "userContext": "uc-1" })
    );
    let default = set_cookie_in(&cookie(None), None).params;
    assert!(default.get("partition").is_none());
    assert_eq!(default, set_cookie(&cookie(None)).params);
}

#[test]
fn a_user_context_is_created_and_read_back() {
    assert_eq!(create_user_context().method, "browser.createUserContext");
    assert_eq!(
        user_context_of(&json!({ "userContext": "uc-1" })),
        Ok("uc-1".to_owned())
    );
    assert!(user_context_of(&json!({})).is_err());
}

#[test]
fn the_default_session_is_the_default_user_context() {
    assert!(is_default_session(DEFAULT_SESSION));
    assert!(is_default_session(""));
    assert!(!is_default_session("admin"));
}
