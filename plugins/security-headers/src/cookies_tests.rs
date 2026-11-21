use super::*;

const SECRET: &str = "s3cr3t-VALUE-never-shown";

#[test]
fn a_set_cookie_line_is_read_without_its_value() {
    let line = format!("sid={SECRET}; Path=/; HttpOnly; Secure; SameSite=Lax; Domain=example.test");
    let cookie = parse_set_cookie(&line).expect("cookie");
    assert_eq!(cookie.name, "sid");
    assert!(cookie.secure && cookie.http_only && cookie.domain);
    assert_eq!(cookie.same_site, Some(SameSite::Lax));
    assert_eq!(cookie.path.as_deref(), Some("/"));
    assert!(!format!("{cookie:?}").contains(SECRET));
}

#[test]
fn attributes_are_read_case_insensitively_and_bad_same_site_is_none() {
    let cookie = parse_set_cookie("a=1; secure; HTTPONLY; samesite=STRICT").expect("cookie");
    assert!(cookie.secure && cookie.http_only);
    assert_eq!(cookie.same_site, Some(SameSite::Strict));
    let cookie = parse_set_cookie("a=1; SameSite=sometimes").expect("cookie");
    assert_eq!(cookie.same_site, None);
}

#[test]
fn nameless_and_empty_lines_are_no_cookie() {
    assert!(parse_set_cookie("").is_none());
    assert!(parse_set_cookie("=value").is_none());
    assert!(parse_set_cookie("novalue").is_none());
    assert!(parse_set_cookie("   ; Secure").is_none());
}

#[test]
fn a_hostile_line_is_bounded_and_never_panics() {
    let long_name = format!("{}=v", "n".repeat(10_000));
    let many = format!("a=b{}", "; Secure".repeat(10_000));
    let controls = "a\u{0}\u{7}b=v; Path=/\u{1b}[31m";
    for line in [
        long_name.as_str(),
        many.as_str(),
        controls,
        "\u{e9}\u{1F600}=x; SameSite",
    ] {
        if let Some(cookie) = parse_set_cookie(line) {
            assert!(cookie.name.chars().count() <= MAX_NAME_CHARS);
            assert!(!cookie.name.chars().any(char::is_control));
            assert!(
                cookie
                    .path
                    .as_ref()
                    .is_none_or(|path| path.chars().count() <= MAX_PATH_CHARS)
            );
        }
    }
}

#[test]
fn the_cdp_answer_is_read_without_values() {
    let answer = serde_json::json!({ "cookies": [
        { "name": "sid", "value": SECRET, "domain": "example.test", "path": "/", "secure": false, "httpOnly": true },
        { "name": "pref", "value": SECRET, "domain": ".example.test", "path": "/", "secure": true, "httpOnly": false, "sameSite": "None" },
        { "value": "no name" },
        "not an object"
    ]})
    .to_string();
    let cookies = from_cdp(&answer);
    assert_eq!(cookies.len(), 2);
    assert_eq!(cookies[0].name, "sid");
    assert!(!cookies[0].domain && cookies[0].http_only && !cookies[0].secure);
    assert_eq!(cookies[0].same_site, None);
    assert!(cookies[1].domain);
    assert_eq!(cookies[1].same_site, Some(SameSite::None));
    assert!(!format!("{cookies:?}").contains(SECRET));
    assert!(from_cdp("not json").is_empty());
    assert!(from_cdp("{\"cookies\": 3}").is_empty());
}

#[test]
fn the_cdp_answer_is_bounded() {
    let many: Vec<serde_json::Value> = (0..MAX_COOKIES + 30)
        .map(|n| serde_json::json!({ "name": format!("c{n}"), "value": "v" }))
        .collect();
    let answer = serde_json::json!({ "cookies": many }).to_string();
    assert_eq!(from_cdp(&answer).len(), MAX_COOKIES);
}

fn cookie(line: &str) -> Cookie {
    parse_set_cookie(line).expect("cookie")
}

#[test]
fn missing_flags_on_https() {
    let found = audit(&cookie("sid=x; Path=/"), true);
    assert_eq!(found.missing, ["Secure", "HttpOnly", "SameSite"]);
    assert_eq!(found.prefix, None);
    assert!(audit(&cookie("sid=x; Secure; HttpOnly; SameSite=Lax"), true).is_clean());
}

#[test]
fn secure_is_not_asked_over_http_unless_same_site_is_none() {
    assert_eq!(
        audit(&cookie("sid=x; HttpOnly; SameSite=Lax"), false).missing,
        Vec::<&str>::new()
    );
    assert_eq!(
        audit(&cookie("sid=x; HttpOnly; SameSite=None"), false).missing,
        ["Secure"]
    );
}

#[test]
fn prefixes_are_held_to_their_rules() {
    assert_eq!(
        audit(&cookie("__Secure-a=x; HttpOnly; SameSite=Lax"), false).prefix,
        Some("__Secure-")
    );
    assert_eq!(
        audit(
            &cookie("__Secure-a=x; Secure; HttpOnly; SameSite=Lax"),
            true
        )
        .prefix,
        None
    );
    assert_eq!(
        audit(
            &cookie("__Host-a=x; Secure; HttpOnly; SameSite=Lax; Path=/; Domain=example.test"),
            true
        )
        .prefix,
        Some("__Host-")
    );
    assert_eq!(
        audit(
            &cookie("__host-a=x; Secure; HttpOnly; SameSite=Lax; Path=/app"),
            true
        )
        .prefix,
        Some("__Host-")
    );
    assert_eq!(
        audit(
            &cookie("__Host-a=x; Secure; HttpOnly; SameSite=Lax; Path=/"),
            true
        )
        .prefix,
        None
    );
}

#[test]
fn the_summary_names_flags_but_never_the_value() {
    let summary = summary(&cookie(&format!(
        "sid={SECRET}; Secure; HttpOnly; SameSite=Strict; Max-Age=60"
    )));
    assert_eq!(summary, "Secure; HttpOnly; SameSite=Strict");
    assert_eq!(super::summary(&cookie("a=b")), "");
}

#[test]
fn merged_keeps_the_first_of_a_name() {
    let merged = merge(
        vec![cookie("a=1; Secure"), cookie("b=1")],
        vec![cookie("a=2"), cookie("c=1")],
    );
    let names: Vec<&str> = merged.iter().map(|cookie| cookie.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "c"]);
    assert!(merged[0].secure);
}

#[test]
fn a_deleting_line_is_marked_deleted() {
    for line in [
        "old=; Max-Age=0",
        "old=; max-age=-1; Secure",
        "old=; Expires=Thu, 01 Jan 1970 00:00:00 GMT",
        "old=; expires=Thu, 01-Jan-1970 00:00:01 GMT; Path=/",
    ] {
        assert!(cookie(line).deleted, "{line}");
    }
    for line in [
        "live=; Max-Age=3600",
        "live=; Expires=Fri, 20 Nov 2099 10:00:00 GMT",
        "live=; Max-Age=60; Expires=Thu, 01 Jan 1970 00:00:00 GMT",
        "live=; Max-Age=soon",
        "live=",
    ] {
        assert!(!cookie(line).deleted, "{line}");
    }
    // The last Max-Age counts.
    assert!(!cookie("x=; Max-Age=0; Max-Age=60").deleted);
}

#[test]
fn expires_is_held_to_the_answers_date() {
    let now = Some(1_795_168_800);
    let past = "a=; Expires=Fri, 20 Nov 2026 09:00:00 GMT";
    let future = "a=; Expires=Fri, 20 Nov 2026 11:00:00 GMT";
    assert!(parse_set_cookie_at(past, now).expect("cookie").deleted);
    assert!(!parse_set_cookie_at(future, now).expect("cookie").deleted);
    // Without a Date only an Expires before 2000 deletes.
    assert!(!parse_set_cookie(past).expect("cookie").deleted);
}

#[test]
fn a_later_answer_replaces_and_deletes_earlier_cookies() {
    let applied = apply(
        vec![cookie("a=1"), cookie("b=1"), cookie("c=1")],
        vec![
            cookie("a=; Max-Age=0"),
            cookie("b=2; Secure"),
            cookie("d=1"),
        ],
    );
    let names: Vec<&str> = applied.iter().map(|cookie| cookie.name.as_str()).collect();
    assert_eq!(names, ["c", "b", "d"]);
    assert!(applied[1].secure);
    assert!(apply(Vec::new(), vec![cookie("z=; Max-Age=0")]).is_empty());
}
