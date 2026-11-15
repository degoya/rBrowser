use super::*;

#[test]
fn numbers_and_ids_leave_the_text() {
    assert_eq!(
        text("Uncaught Error: layout failed at 375 px"),
        "Uncaught Error: layout failed at # px"
    );
    assert_eq!(
        text("request 3f2a9c1b-0d4e-4c55-9a77-2b1c0e5d8f10 failed"),
        "request <id> failed"
    );
    assert_ne!(text("Failed at 375"), text("Failed in 375"));
    assert_eq!(text("  a \n  b  "), "a b");
}

#[test]
fn a_cache_buster_and_the_fragment_leave_the_url() {
    assert_eq!(
        url_key("https://a.test/app.js?v=123#x"),
        "https://a.test/app.js"
    );
    assert_eq!(
        url_key("https://a.test/img.png?_=1731300000123&w=375"),
        "https://a.test/img.png?w=375"
    );
    assert_eq!(
        url_key("https://a.test/api?page=2&t=1731300000"),
        "https://a.test/api?page=2"
    );
    // A long number or hex id is a cache buster whatever its name.
    assert_eq!(
        url_key("https://a.test/x.css?build=9f86d081884c"),
        "https://a.test/x.css"
    );
    assert_eq!(
        url_key("https://a.test/?q=shoes"),
        "https://a.test/?q=shoes"
    );
    assert_eq!(url_key("https://a.test/a?"), "https://a.test/a");
}

#[test]
fn a_stack_leaves_the_message_and_loses_its_positions_in_the_key() {
    let (message, stack) = split_stack(
        "Uncaught Error: boom\n    at f (https://a.test/app.js:3:9)\n    at https://a.test/app.js:7:1",
    );
    assert_eq!(message, "Uncaught Error: boom");
    assert_eq!(
        stack.as_deref(),
        Some("at f (https://a.test/app.js:3:9)\nat https://a.test/app.js:7:1")
    );
    assert_eq!(split_stack("plain").1, None);
    assert_eq!(
        stack_key("at f (https://a.test/app.js?v=2:3:9)\nat g (https://a.test/b.js:1:1)"),
        "f@https://a.test/app.js|g@https://a.test/b.js"
    );
}

#[test]
fn globs_match_case_blind() {
    assert!(glob("*/analytics/*", "https://a.test/analytics/x.js"));
    assert!(glob("*Deprecation*", "XHR deprecation"));
    assert!(glob("a?c", "abc"));
    assert!(!glob("a?c", "ac"));
    assert!(!glob("*.png", "https://a.test/x.jpg"));
    assert!(glob("*", ""));
}

#[test]
fn hosts_and_first_parties() {
    assert_eq!(
        host_of("https://User@WWW.a.test:8080/x"),
        Some("www.a.test".to_owned())
    );
    assert_eq!(host_of("data:image/png;base64,AA"), None);
    assert!(same_site("https://cdn.a.test/x", "https://a.test/"));
    assert!(same_site("https://a.test/x", "https://www.a.test/"));
    assert!(!same_site("https://tracker.test/x", "https://a.test/"));
    assert!(!same_site("https://nota.test/x", "https://a.test/"));
}

#[test]
fn ids_are_stable_and_texts_cut() {
    assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(id_of("js|x"), id_of("js|x"));
    assert_eq!(id_of("js|x").len(), 16);
    assert_eq!(cut("abcdef", 5), "ab...");
    assert_eq!(cut("abc", 5), "abc");
}
