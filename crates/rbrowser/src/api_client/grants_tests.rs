use serde_json::json;

use super::*;

#[test]
fn a_grant_names_the_host_without_scheme_and_port() {
    assert_eq!(
        host_of("https://API.Example.com:8443/v1?x=1"),
        Some("api.example.com".into())
    );
    assert_eq!(host_of("http://localhost:3000/"), Some("localhost".into()));
    assert_eq!(host_of("ftp://example.com/"), None);
    assert_eq!(host_of("{{baseUrl}}/users"), None);
    assert_eq!(normalized("Example.COM."), Some("example.com".into()));
    assert_eq!(normalized("[::1]"), Some("[::1]".into()));
    assert_eq!(normalized("evil.com/x"), None);
    assert_eq!(normalized("a..b"), None);
    assert_eq!(normalized(""), None);
}

#[test]
fn the_list_is_read_kept_sorted_and_bounded() {
    let list = list_of(Some(&json!(["b.example", "A.example", 3, "bad host"])));
    assert_eq!(list, ["b.example", "a.example"]);
    assert!(allowed(&list, "A.EXAMPLE"));
    assert!(!allowed(&list, "c.example"));
    assert_eq!(
        with_host(&list, "c.example"),
        Some(vec![
            "a.example".to_owned(),
            "b.example".to_owned(),
            "c.example".to_owned()
        ])
    );
    assert_eq!(with_host(&list, "not a host"), None);
    let full: Vec<String> = (0..MAX_HOSTS).map(|n| format!("h{n}.example")).collect();
    assert_eq!(with_host(&full, "new.example"), None);
    assert!(with_host(&full, "h1.example").is_some());
    assert!(list_of(None).is_empty());
}
