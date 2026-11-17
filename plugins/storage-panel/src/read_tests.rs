use serde_json::json;

use super::*;
use crate::browser::{Browser, view};
use crate::columns;

const PAGE: &str = "http://127.0.0.1:18767/storage/index.html";

fn browser() -> Browser {
    let mut browser = Browser::default();
    browser.add_view(view(1, "mobile", "chromium", PAGE), "default");
    browser.add_view(view(2, "desktop", "chromium", "about:blank"), "default");
    browser.add_view(view(3, "fx", "firefox", PAGE), "firefox");
    browser.set_cookie(
        1,
        json!({ "name": "sid", "value": "s3cr3t", "domain": "127.0.0.1", "path": "/",
                "httpOnly": true, "secure": false, "session": true }),
    );
    browser.set_cookie(
        1,
        json!({ "name": "consent", "value": "yes", "domain": "127.0.0.1", "path": "/",
                "httpOnly": false, "secure": false, "session": true }),
    );
    browser.set_local(1, "featureFlag", "off");
    browser
}

#[test]
fn a_chromium_view_answers_cookies_and_both_storage_areas() {
    let mut browser = browser();
    let columns = columns::columns(&mut browser);
    let data = read_view(&mut browser, &columns[0], &Kind::ALL, true);
    assert_eq!(data.status, "ok");
    let names: Vec<&str> = data.cookies.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["sid", "consent"], "HttpOnly cookies are read too");
    assert_eq!(data.origin.as_deref(), Some("http://127.0.0.1:18767"));
    assert_eq!(data.local, [("featureFlag".to_owned(), "off".to_owned())]);
    assert!(data.session.is_empty());
    assert_eq!(browser.cdp_sent[0].1, "Network.getCookies");
    assert_eq!(
        browser.cdp_sent[0].2,
        json!({ "urls": [PAGE] }),
        "only the view's own URL"
    );
}

#[test]
fn httponly_cookies_stay_out_when_the_setting_hides_them() {
    let mut browser = browser();
    let columns = columns::columns(&mut browser);
    let data = read_view(&mut browser, &columns[0], &[Kind::Cookies], false);
    assert_eq!(data.cookies.len(), 1);
    assert_eq!(data.cookies[0].name, "consent");
    assert!(browser.evaluated.is_empty(), "no storage asked, none read");
}

#[test]
fn other_engines_and_pages_without_storage_are_marked_not_failed() {
    let mut browser = browser();
    let columns = columns::columns(&mut browser);
    let all = read_all(&mut browser, &columns, Target::All, &Kind::ALL, true);
    assert_eq!(all.len(), 3);
    assert_eq!(all[1].status, "ok");
    assert_eq!(all[1].storage_error.as_deref(), Some("unavailable"));
    assert_eq!(all[1].origin, None);
    assert_eq!(all[2].status, "unsupported");
    assert!(browser.evaluated.iter().all(|(view, _)| *view != 3));
    assert!(
        browser.cdp_sent.iter().all(|(view, _, _)| *view == 1),
        "about:blank has no cookies to ask for"
    );
}

#[test]
fn a_view_that_fails_names_the_hosts_code() {
    let mut browser = browser();
    browser.broken.insert(1);
    let columns = columns::columns(&mut browser);
    let only = read_all(&mut browser, &columns, Target::View(1), &Kind::ALL, true);
    assert_eq!(only.len(), 1);
    assert_eq!(only[0].status, "error");
    assert_eq!(only[0].error.as_deref(), Some("plugins.error.host_failed"));
}

#[test]
fn kinds_parse_from_both_spellings() {
    assert_eq!(Kind::parse("cookie"), Some(Kind::Cookies));
    assert_eq!(Kind::parse("localStorage"), Some(Kind::Local));
    assert_eq!(Kind::parse("session"), Some(Kind::Session));
    assert_eq!(Kind::parse("indexedDB"), None);
    assert_eq!(Kind::Session.area(), "session");
    assert_eq!(Kind::Local.name(), "localStorage");
}

#[test]
fn the_script_is_the_ops_function_applied_to_the_op() {
    let script = script(&json!({ "op": "read" }));
    assert!(script.starts_with("((op) =>") || script.starts_with("(//"));
    assert!(script.ends_with(r#")({"op":"read"})"#));
}
