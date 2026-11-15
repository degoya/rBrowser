use regex::Regex;

use super::*;

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

#[test]
fn header_operations_set_case_blind_add_and_remove() {
    let original = pairs(&[("Accept", "*/*"), ("X-Old", "a"), ("x-old", "b")]);
    let ops = vec![
        HeaderOp::Set("x-old".to_owned(), "c".to_owned()),
        HeaderOp::Add("X-More".to_owned(), "1".to_owned()),
        HeaderOp::Add("x-more".to_owned(), "2".to_owned()),
        HeaderOp::Remove("accept".to_owned()),
        HeaderOp::Set("X-New".to_owned(), "n".to_owned()),
    ];
    assert_eq!(
        apply_headers(&original, &ops),
        pairs(&[
            ("x-old", "c"),
            ("X-More", "1"),
            ("x-more", "2"),
            ("X-New", "n")
        ])
    );
}

#[test]
fn query_operations_set_and_remove_parameters() {
    let ops = vec![
        QueryOp::Set("debug".to_owned(), "1".to_owned()),
        QueryOp::Remove("utm_source".to_owned()),
        QueryOp::Set("q".to_owned(), "a b&c".to_owned()),
    ];
    assert_eq!(
        apply_query(
            "https://a.test/p?utm_source=x&q=old&utm_source=y#frag",
            &ops
        ),
        "https://a.test/p?q=a%20b%26c&debug=1#frag"
    );
    assert_eq!(
        apply_query("https://a.test/p", &[QueryOp::Remove("x".to_owned())]),
        "https://a.test/p"
    );
    assert_eq!(
        apply_query("https://a.test/p?a=1", &[QueryOp::Remove("a".to_owned())]),
        "https://a.test/p"
    );
}

fn endpoint(scheme: Option<&str>, host: &str, port: Option<u16>, path: Option<&str>) -> Endpoint {
    Endpoint {
        scheme: scheme.map(str::to_owned),
        host: host.to_owned(),
        port,
        path: path.map(str::to_owned),
    }
}

#[test]
fn map_remote_swaps_host_port_and_path_prefix_and_keeps_the_rest() {
    let from = endpoint(Some("https"), "api.example.com", None, Some("/v1"));
    let to = endpoint(Some("http"), "localhost", Some(3001), Some("/api"));
    assert_eq!(
        map_remote("https://api.example.com/v1/users?x=1#f", &from, &to).as_deref(),
        Some("http://localhost:3001/api/users?x=1#f")
    );
    assert_eq!(
        map_remote("https://API.example.com:443/v1", &from, &to).as_deref(),
        Some("http://localhost:3001/api"),
        "the host case-blind, the port when from names none"
    );
    assert_eq!(
        map_remote("https://api.example.com/v2/x", &from, &to),
        None,
        "another prefix"
    );
    assert_eq!(
        map_remote("http://api.example.com/v1/x", &from, &to),
        None,
        "another scheme"
    );
    assert_eq!(
        map_remote("https://other.test/v1/x", &from, &to),
        None,
        "another host"
    );
    let host_only = endpoint(None, "api.example.com", None, None);
    let target = endpoint(None, "localhost", Some(3001), None);
    assert_eq!(
        map_remote("https://api.example.com:8443/v1/x", &host_only, &target).as_deref(),
        Some("https://localhost:3001/v1/x"),
        "scheme and path stay"
    );
    assert_eq!(
        map_remote(
            "https://api.example.com/",
            &host_only,
            &endpoint(None, "b.test", None, None)
        )
        .as_deref(),
        Some("https://b.test/")
    );
    assert_eq!(host_header(&to), "localhost:3001");
    assert_eq!(
        origin_header("http://localhost:3001/api/users"),
        Some("http://localhost:3001".to_owned())
    );
}

#[test]
fn a_body_replacement_rewrites_text_and_leaves_binary_alone() {
    let replace = BodyReplace {
        regex: Regex::new("Hello (\\w+)").expect("a regex"),
        replacement: "Bye $1".to_owned(),
    };
    assert_eq!(
        replace_body(b"Hello World, Hello Moon", &replace).as_deref(),
        Some(b"Bye World, Bye Moon".as_slice())
    );
    assert_eq!(
        replace_body(b"nothing here", &replace),
        None,
        "no match, no change"
    );
    assert_eq!(replace_body(&[0xff, 0xfe], &replace), None, "no text");
}
