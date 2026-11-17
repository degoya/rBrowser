use serde_json::json;

use super::*;
use crate::cookies::model::cookies_of;

fn cookie(name: &str, domain: &str) -> ChromeCookie {
    cookies_of(&json!({ "cookies": [
        { "name": name, "value": "v", "domain": domain, "path": "/" }
    ]}))
    .pop()
    .expect("a cookie")
}

#[test]
fn a_site_takes_its_own_and_its_parent_domains_cookies() {
    let filter = Filter::new(Some(vec!["App.Example.com".to_owned()]), &[]);
    assert_eq!(filter.check(&cookie("a", "app.example.com")), Ok(()));
    assert_eq!(filter.check(&cookie("b", ".example.com")), Ok(()));
    assert_eq!(
        filter.check(&cookie("c", "api.example.com")),
        Err(SkipReason::OtherDomain)
    );
    assert_eq!(
        filter.check(&cookie("d", "badexample.com")),
        Err(SkipReason::OtherDomain)
    );
    assert_eq!(
        Filter::new(Some(Vec::new()), &[]).check(&cookie("e", "example.com")),
        Err(SkipReason::OtherDomain),
        "no site shown: nothing to copy"
    );
    assert_eq!(
        Filter::new(None, &[]).check(&cookie("f", "elsewhere.org")),
        Ok(())
    );
}

#[test]
fn the_exclusion_list_takes_names_with_wildcards_and_domains() {
    let exclude = [
        "_ga*".to_owned(),
        " tracker.example ".to_owned(),
        String::new(),
    ];
    let filter = Filter::new(None, &exclude);
    for (name, domain) in [
        ("_ga", "example.com"),
        ("_ga_XYZ", "example.com"),
        ("sid", "tracker.example"),
        ("sid", ".cdn.tracker.example"),
    ] {
        assert_eq!(
            filter.check(&cookie(name, domain)),
            Err(SkipReason::Excluded),
            "{name} {domain}"
        );
    }
    assert_eq!(filter.check(&cookie("sid", "example.com")), Ok(()));
    assert_eq!(filter.check(&cookie("x_ga", "example.com")), Ok(()));
}

#[test]
fn glob_matches_star_anywhere() {
    assert!(glob("*", "anything"));
    assert!(glob("a*b*c", "a-b-c"));
    assert!(glob("ab", "ab"));
    assert!(!glob("ab", "abc"));
    assert!(!glob("a*a", "a"));
    assert!(glob("*_session", "app_session"));
}

#[test]
fn the_cookie_host_of_a_url_leaves_scheme_port_user_and_brackets_out() {
    assert_eq!(
        cookie_host("http://127.0.0.1:18766/account"),
        Some("127.0.0.1".to_owned())
    );
    assert_eq!(
        cookie_host("https://user:pw@Shop.Example.com/x?y#z"),
        Some("shop.example.com".to_owned())
    );
    assert_eq!(cookie_host("http://[::1]:8080/"), Some("::1".to_owned()));
    assert_eq!(cookie_host("about:blank"), None);
    assert_eq!(cookie_host("file:///C:/site/index.html"), None);
    assert!(domain_matches("a.b.example.com", "example.com"));
    assert!(!domain_matches("example.com", ""));
}
