use super::*;

fn allow() -> Vec<String> {
    allowlist(
        Some("http://localhost:5173/start"),
        [
            "http://LOCALHOST:5173/",
            "https://staging.example.com",
            "http://10.0.0.5:8080/app",
        ],
    )
}

#[test]
fn the_allowlist_holds_the_environments_and_the_start_url_once_each() {
    assert_eq!(
        allow(),
        vec![
            "http://localhost:5173".to_owned(),
            "https://staging.example.com".to_owned(),
            "http://10.0.0.5:8080".to_owned(),
        ]
    );
}

#[test]
fn an_origin_outside_the_allowlist_has_no_upstream() {
    assert_eq!(
        upstream_for(Some("http://evil.example.com/x"), &allow()),
        Err(Refusal::NotAllowed)
    );
    assert_eq!(upstream_for(None, &allow()), Err(Refusal::NoPage));
}

#[test]
fn an_https_environment_is_refused_by_name() {
    assert_eq!(
        upstream_for(Some("https://staging.example.com/a"), &allow()),
        Err(Refusal::Https)
    );
}

#[test]
fn an_http_environment_is_the_upstream() {
    let upstream = upstream_for(Some("http://localhost:5173/a/b?c#d"), &allow()).expect("allowed");
    assert_eq!(upstream.origin(), "http://localhost:5173");
    assert_eq!(upstream.authority(), "localhost:5173");
    assert_eq!(upstream.host(), "localhost");
    assert_eq!(upstream.port(), 5173);
    let default_port = Upstream::parse("http://example.test").expect("parses");
    assert_eq!(default_port.port(), 80);
    assert_eq!(default_port.authority(), "example.test");
}

#[test]
fn the_device_path_is_path_query_and_hash() {
    assert_eq!(
        device_path("http://localhost:5173/a/b?c=1#top").as_deref(),
        Some("/a/b?c=1#top")
    );
    assert_eq!(device_path("http://localhost:5173").as_deref(), Some("/"));
    assert_eq!(device_path("about:blank"), None);
}

#[test]
fn only_the_portal_itself_is_a_valid_host() {
    assert!(host_matches(Some("192.168.1.20:4000"), "192.168.1.20:4000"));
    assert!(host_matches(
        Some("192.168.1.20:4000 "),
        "192.168.1.20:4000"
    ));
    assert!(!host_matches(Some("evil.example.com"), "192.168.1.20:4000"));
    assert!(!host_matches(Some("192.168.1.20"), "192.168.1.20:4000"));
    assert!(!host_matches(None, "192.168.1.20:4000"));
}

#[test]
fn a_location_on_the_upstream_moves_to_the_portal() {
    let portal = "http://192.168.1.20:4000";
    let upstream = "http://localhost:5173";
    assert_eq!(
        rewrite_url("http://localhost:5173/login?x=1", upstream, portal),
        "http://192.168.1.20:4000/login?x=1"
    );
    assert_eq!(
        rewrite_url("http://LOCALHOST:5173", upstream, portal),
        "http://192.168.1.20:4000/"
    );
    assert_eq!(rewrite_url("/relative", upstream, portal), "/relative");
    assert_eq!(
        rewrite_url("https://other.example/", upstream, portal),
        "https://other.example/"
    );
    assert_eq!(
        rewrite_url("http://localhost:51730/x", upstream, portal),
        "http://localhost:51730/x",
        "a longer port is another origin"
    );
}

#[test]
fn a_default_port_is_no_part_of_the_path() {
    assert_eq!(
        device_path("http://example.test:80/x").as_deref(),
        Some("/x")
    );
    assert_eq!(device_path("http://example.test?q").as_deref(), Some("/?q"));
}

#[test]
fn an_environment_with_credentials_is_left_out_of_the_allowlist() {
    // `origin_of` drops user info; the URL as written decides, so no device reaches an origin
    // whose entry carried credentials (RB-080-28 review).
    let list = allowlist(
        Some("http://user:pw@localhost:3000/"),
        [
            "http://api:secret@localhost:4000/",
            "http://localhost:5000/",
        ],
    );
    assert_eq!(list, vec!["http://localhost:5000".to_owned()]);
}

#[test]
fn ipv6_and_credentials() {
    let v6 = Upstream::parse("http://[::1]:8080/").expect("parses");
    assert_eq!((v6.host(), v6.port()), ("[::1]", 8080));
    assert_eq!(Upstream::parse("http://[::1]/").map(|u| u.port()), Some(80));
    assert_eq!(Upstream::parse("http://user:pw@host/"), None);
    assert_eq!(Upstream::parse("https://host/"), None);
}

fn envs<'a>(list: &'a [(&'a str, Vec<String>)]) -> AssetOrigins {
    asset_origins(list.iter().map(|(url, assets)| (*url, assets.as_slice())))
}

fn strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|item| (*item).to_owned()).collect()
}

/// RB-090-65: per environment origin its asset origins, canonical, once each; nothing that is
/// not a bare `http` origin, nothing that is the environment itself.
#[test]
fn asset_origins_are_canonical_http_origins_per_environment() {
    let list = [
        (
            "http://localhost:8000/start",
            strings(&[
                "http://[::1]:5173",
                "HTTP://LOCALHOST:5174/",
                "http://[::1]:5173/",
                "https://cdn.example.com",
                "http://localhost:5173/build",
                "http://u:p@localhost:5175",
                "http://localhost:8000",
            ]),
        ),
        (
            "http://LOCALHOST:8000/other",
            strings(&["http://127.0.0.1:5176"]),
        ),
        ("https://staging.example.com", strings(&[])),
        (
            "http://u:p@10.0.0.5:8080",
            strings(&["http://10.0.0.5:5173"]),
        ),
    ];
    let assets = envs(&list);
    assert_eq!(
        assets,
        vec![(
            "http://localhost:8000".to_owned(),
            strings(&[
                "http://[::1]:5173",
                "http://localhost:5174",
                "http://127.0.0.1:5176"
            ])
        )]
    );
    assert_eq!(
        assets_of(&assets, "http://localhost:8000"),
        strings(&[
            "http://[::1]:5173",
            "http://localhost:5174",
            "http://127.0.0.1:5176"
        ])
        .as_slice()
    );
    assert!(assets_of(&assets, "http://localhost:9999").is_empty());
}

#[test]
fn every_asset_origin_is_listened_on_once() {
    let list = [
        ("http://localhost:8000", strings(&["http://localhost:5173"])),
        (
            "http://localhost:8001",
            strings(&["http://localhost:5173", "http://localhost:5174"]),
        ),
    ];
    assert_eq!(
        listened(&envs(&list)),
        strings(&["http://localhost:5173", "http://localhost:5174"])
    );
    assert!(listened(&Vec::new()).is_empty());
}
