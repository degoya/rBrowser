use super::*;

const PAGE: &str = "http://localhost:8000";
const VITE: &str = "http://[::1]:5173";
const PORTAL: &str = "http://192.168.1.20:4000";
const ASSETS: &str = "http://192.168.1.20:4100";

fn map() -> OriginMap {
    OriginMap::new(PAGE, PORTAL).with(VITE, ASSETS)
}

fn rewritten(body: &str) -> String {
    String::from_utf8(map().rewrite_body(body.as_bytes())).expect("utf-8")
}

/// A Laravel page names the Vite dev server for its styles and scripts (`@vite`); both point at
/// the portal, each at its own listener.
#[test]
fn a_page_names_the_portal_for_itself_and_its_dev_server() {
    let html = "<script type=\"module\" src=\"http://[::1]:5173/@vite/client\"></script>\
                <link rel=\"stylesheet\" href=\"http://[::1]:5173/resources/css/app.css\">\
                <a href=\"http://localhost:8000/login\">in</a>";
    assert_eq!(
        rewritten(html),
        "<script type=\"module\" src=\"http://192.168.1.20:4100/@vite/client\"></script>\
         <link rel=\"stylesheet\" href=\"http://192.168.1.20:4100/resources/css/app.css\">\
         <a href=\"http://192.168.1.20:4000/login\">in</a>"
    );
}

/// Vite writes `server.origin` into CSS and JavaScript (`url()`, imports): absolute and relative
/// URLs alike reach the device.
#[test]
fn css_and_javascript_point_at_the_portal_and_relative_urls_stay() {
    assert_eq!(
        rewritten(
            "body{background:url(http://[::1]:5173/img/a.png)} .b{background:url(/img/b.png)}"
        ),
        "body{background:url(http://192.168.1.20:4100/img/a.png)} .b{background:url(/img/b.png)}"
    );
    assert_eq!(
        rewritten(
            "import \"http://[::1]:5173/@vite/env\";import x from \"/node_modules/.vite/x.js\";"
        ),
        "import \"http://192.168.1.20:4100/@vite/env\";import x from \"/node_modules/.vite/x.js\";"
    );
}

#[test]
fn only_a_whole_origin_matches_in_any_case() {
    assert_eq!(
        rewritten(
            "http://LOCALHOST:8000/a http://localhost:80001/ http://localhost:8000.example/ \
                   https://localhost:8000/ http://[::1]:51730/ http://localhost:8000"
        ),
        "http://192.168.1.20:4000/a http://localhost:80001/ http://localhost:8000.example/ \
         https://localhost:8000/ http://[::1]:51730/ http://192.168.1.20:4000"
    );
    assert_eq!(map().rewrite_body(b"<p>none</p>"), b"<p>none</p>");
}

/// One pass: a portal origin written in never matches again, and a shorter origin never shadows
/// a longer one.
#[test]
fn the_body_is_rewritten_in_one_pass() {
    let map = OriginMap::new("http://a", "http://a:8000").with("http://a:8000", "http://b:1");
    assert_eq!(
        String::from_utf8(map.rewrite_body(b"http://a/x http://a:8000/y")).expect("utf-8"),
        "http://a:8000/x http://b:1/y"
    );
}

#[test]
fn a_pair_mapped_already_is_not_added_again() {
    let map = map()
        .with(VITE, "http://192.168.1.20:9")
        .with("http://x:1", ASSETS);
    assert_eq!(map.upstreams().collect::<Vec<_>>(), vec![PAGE, VITE]);
    assert_eq!(map.main_upstream(), PAGE);
}

#[test]
fn urls_and_origins_go_both_ways() {
    let map = map();
    assert_eq!(
        map.to_upstream_url("http://192.168.1.20:4100/resources/js/app.js?v=1"),
        "http://[::1]:5173/resources/js/app.js?v=1"
    );
    assert_eq!(
        map.to_upstream_url("http://192.168.1.20:4000/start"),
        "http://localhost:8000/start"
    );
    assert_eq!(
        map.to_portal_url("http://localhost:8000/login"),
        "http://192.168.1.20:4000/login"
    );
    assert_eq!(
        map.to_portal_url("http://elsewhere/x"),
        "http://elsewhere/x"
    );
}

/// A module script on the asset listener carries the page's portal origin as `Origin`; the dev
/// server must see the page's own origin, and its answer must name the portal again.
#[test]
fn the_origin_header_names_the_page_upstream_and_cors_the_portal() {
    let map = map();
    assert_eq!(map.origin_to_upstream(PORTAL), PAGE);
    assert_eq!(map.origin_to_upstream(ASSETS), VITE);
    assert_eq!(map.origin_to_upstream("null"), "null");
    assert_eq!(
        map.origin_to_upstream("http://evil.example"),
        "http://evil.example"
    );
    assert_eq!(map.origin_to_portal(PAGE), PORTAL);
    assert_eq!(map.origin_to_portal(VITE), ASSETS);
    assert_eq!(map.origin_to_portal("*"), "*");
}

/// The evidence for the next case: a loopback origin the page names but nobody maps, origin
/// only - no path, no query.
#[test]
fn an_unmapped_loopback_origin_is_found_once_without_its_path() {
    let html = b"<script src=\"http://localhost:5173/@vite/client?token=secret\"></script>\
                 <script src=\"http://localhost:5173/app.js\"></script>\
                 <img src=\"http://127.0.0.1:9000/a.png\"><img src=\"http://[::1]:5173/mapped.png\">\
                 <a href=\"http://localhost:8000/\">page</a><a href=\"http://example.com:5173/\">far</a>\
                 <a href=\"http://localhost.example.com/\">not loopback</a>";
    assert_eq!(
        unmapped_loopback(html, &map()),
        vec![
            "http://localhost:5173".to_owned(),
            "http://127.0.0.1:9000".to_owned()
        ]
    );
    assert!(unmapped_loopback(b"<p>http://</p>", &map()).is_empty());
}

/// A site that writes its own origin into links and asset URLs (`APP_URL`) would send the
/// device to a host it cannot reach.
#[test]
fn absolute_upstream_urls_point_at_the_portal() {
    let html = b"<a href=\"http://localhost:3000/a?x=1\">a</a><script src=\"http://localhost:3000/app.js\"></script>\
                 <a href=\"http://localhost:30001/\">other port</a><a href=\"https://localhost:3000/\">tls</a>\
                 <a href=\"HTTP://LOCALHOST:3000/up\">case</a> http://localhost:3000";
    let out = OriginMap::new("http://localhost:3000", "http://192.168.1.5:4312").rewrite_body(html);
    let out = String::from_utf8(out).expect("utf-8");
    assert_eq!(
        out,
        "<a href=\"http://192.168.1.5:4312/a?x=1\">a</a><script src=\"http://192.168.1.5:4312/app.js\"></script>\
         <a href=\"http://localhost:30001/\">other port</a><a href=\"https://localhost:3000/\">tls</a>\
         <a href=\"http://192.168.1.5:4312/up\">case</a> http://192.168.1.5:4312"
    );
    assert_eq!(
        OriginMap::new("http://a:1", "http://b:2").rewrite_body(b"<p>none</p>"),
        b"<p>none</p>"
    );
}

/// Only a map with asset origins makes a style sheet or script worth reading whole.
#[test]
fn a_map_knows_whether_it_has_asset_origins() {
    assert!(!OriginMap::new(PAGE, PORTAL).has_assets());
    assert!(map().has_assets());
    assert!(!OriginMap::new(PAGE, PORTAL).with(PAGE, ASSETS).has_assets());
}

/// RB-090-69: a JSON answer or a script names its URLs with escaped slashes
/// (`http:\/\/host:port\/path`, PHP's `json_encode`); they point at the portal too, still
/// escaped, whole origins only, the `https` twin left alone.
#[test]
fn escaped_urls_point_at_the_portal_still_escaped() {
    assert_eq!(
        rewritten(
            r#"{"img":"http:\/\/[::1]:5173\/storage\/a.png","page":"HTTP:\/\/LOCALHOST:8000\/x","#
        ),
        r#"{"img":"http:\/\/192.168.1.20:4100\/storage\/a.png","page":"http:\/\/192.168.1.20:4000\/x","#
    );
    assert_eq!(
        rewritten(
            r#"["http:\/\/localhost:80001\/","https:\/\/localhost:8000\/","http:\/\/localhost:8000"]"#
        ),
        r#"["http:\/\/localhost:80001\/","https:\/\/localhost:8000\/","http:\/\/192.168.1.20:4000"]"#
    );
    assert_eq!(
        rewritten(r#"a("http://localhost:8000/p","http:\/\/localhost:8000\/q")"#),
        r#"a("http://192.168.1.20:4000/p","http:\/\/192.168.1.20:4000\/q")"#
    );
    assert_eq!(rewritten(r#"{"half":"http:\/"}"#), r#"{"half":"http:\/"}"#);
}

/// RB-090-69: the log's evidence comes from escaped URLs too, as the plain origin, once.
#[test]
fn an_escaped_unmapped_loopback_origin_is_found_as_its_plain_origin() {
    let json = br#"{"a":"http:\/\/127.0.0.1:9000\/a.png?token=secret","b":"http://127.0.0.1:9000/b.png","c":"http:\/\/[::1]:5173\/mapped.png","d":"http:\/\/localhost:5174"}"#;
    assert_eq!(
        unmapped_loopback(json, &map()),
        vec![
            "http://127.0.0.1:9000".to_owned(),
            "http://localhost:5174".to_owned()
        ]
    );
    assert!(unmapped_loopback(br#"{"x":"http:\/\/"}"#, &map()).is_empty());
}

/// RB-090-75: an attribute escaper writes `src` and `srcset` with their `:` and `/` as character
/// references (hex upper, hex lower or decimal); each points at the portal in the same form,
/// beside the plain URLs of the same page, the dots of an IP address literal.
#[test]
fn entity_encoded_origins_point_at_the_portal_in_the_same_form() {
    let map = OriginMap::new("http://localhost:3000", PORTAL);
    let page = |body: &str| String::from_utf8(map.rewrite_body(body.as_bytes())).expect("utf-8");
    assert_eq!(
        page(
            "<img src=\"http&#x3A;&#x2F;&#x2F;localhost&#x3A;3000&#x2F;media&#x2F;pages&#x2F;demo&#x2F;a.webp\" \
             srcset=\"http&#x3A;&#x2F;&#x2F;localhost&#x3A;3000&#x2F;media&#x2F;a-1152.webp&#x20;1152w,&#x20;http&#x3A;&#x2F;&#x2F;LOCALHOST&#x3A;3000&#x2F;media&#x2F;a-576.webp&#x20;576w\">\
             <a href=\"http://localhost:3000/demo\">plain</a>"
        ),
        "<img src=\"http&#x3A;&#x2F;&#x2F;192.168.1.20&#x3A;4000&#x2F;media&#x2F;pages&#x2F;demo&#x2F;a.webp\" \
         srcset=\"http&#x3A;&#x2F;&#x2F;192.168.1.20&#x3A;4000&#x2F;media&#x2F;a-1152.webp&#x20;1152w,&#x20;http&#x3A;&#x2F;&#x2F;192.168.1.20&#x3A;4000&#x2F;media&#x2F;a-576.webp&#x20;576w\">\
         <a href=\"http://192.168.1.20:4000/demo\">plain</a>"
    );
    assert_eq!(
        page("<img src=\"http&#x3a;&#x2f;&#x2f;localhost&#x3a;3000&#x2f;a.webp\">"),
        "<img src=\"http&#x3a;&#x2f;&#x2f;192.168.1.20&#x3a;4000&#x2f;a.webp\">"
    );
    assert_eq!(
        page("<img srcset=\"http&#58;&#47;&#47;localhost&#58;3000&#47;a.webp&#32;2x\">"),
        "<img srcset=\"http&#58;&#47;&#47;192.168.1.20&#58;4000&#47;a.webp&#32;2x\">"
    );
}

/// RB-090-75: an encoded origin matches only whole - a longer port, a longer host or the `https`
/// twin stay - and an asset origin (IPv6, its brackets encoded too) points at its own listener.
#[test]
fn an_entity_encoded_origin_matches_only_whole() {
    assert_eq!(
        rewritten(
            "http&#x3A;&#x2F;&#x2F;localhost&#x3A;80001&#x2F; \
             http&#x3A;&#x2F;&#x2F;localhost&#x3A;8000&#x2E;example \
             https&#x3A;&#x2F;&#x2F;localhost&#x3A;8000&#x2F; \
             http&#x3A;&#x2F;&#x2F;&#x5B;&#x3A;&#x3A;1&#x5D;&#x3A;5173&#x2F;app.js \
             http&#58;&#47;&#47;localhost&#58;8000"
        ),
        "http&#x3A;&#x2F;&#x2F;localhost&#x3A;80001&#x2F; \
         http&#x3A;&#x2F;&#x2F;localhost&#x3A;8000&#x2E;example \
         https&#x3A;&#x2F;&#x2F;localhost&#x3A;8000&#x2F; \
         http&#x3A;&#x2F;&#x2F;192.168.1.20&#x3A;4100&#x2F;app.js \
         http&#58;&#47;&#47;192.168.1.20&#58;4000"
    );
    // An origin without a port never shadows the same host with an encoded port.
    let map = OriginMap::new("http://localhost", PORTAL);
    let body =
        b"http&#x3A;&#x2F;&#x2F;localhost&#x3A;3000&#x2F;a http&#x3A;&#x2F;&#x2F;localhost&#x2F;b";
    assert_eq!(
        String::from_utf8(map.rewrite_body(body)).expect("utf-8"),
        "http&#x3A;&#x2F;&#x2F;localhost&#x3A;3000&#x2F;a http&#x3A;&#x2F;&#x2F;192.168.1.20&#x3A;4000&#x2F;b"
    );
}

/// RB-090-75: the log's evidence comes from encoded URLs too, as the plain origin, once.
#[test]
fn an_entity_encoded_unmapped_loopback_origin_is_found_as_its_plain_origin() {
    let html = b"<img src=\"http&#x3A;&#x2F;&#x2F;127.0.0.1&#x3A;9000&#x2F;a.png&#x3F;token&#x3D;secret\">\
                 <img src=\"http://127.0.0.1:9000/b.png\">\
                 <img srcset=\"http&#58;&#47;&#47;localhost&#58;5174&#47;c.png&#32;2x\">\
                 <img src=\"http&#x3a;&#x2f;&#x2f;&#x5b;&#x3a;&#x3a;1&#x5d;&#x3a;5173&#x2f;mapped.png\">\
                 <img src=\"http&#x3A;&#x2F;&#x2F;localhost&#x3A;8000&#x2F;page.png\">";
    assert_eq!(
        unmapped_loopback(html, &map()),
        vec![
            "http://127.0.0.1:9000".to_owned(),
            "http://localhost:5174".to_owned()
        ]
    );
    assert!(unmapped_loopback(b"http&#x3A;&#x2F;&#x2F;", &map()).is_empty());
}

/// RB-090-75 review: a mapped origin the scan can read but no needle matches - a padded
/// reference (`&#x003A;`), a reference in a plain URL, a default port written out - reaches the
/// device unrewritten; the scan names it as such, never as mapped, and never offers it.
#[test]
fn a_mapped_origin_in_a_form_the_rewrite_skips_is_a_gap() {
    let map = OriginMap::new("http://localhost:8123", PORTAL);
    for body in [
        &b"<img src=\"http&#x3a;&#x2f;&#x2f;localhost&#x003a;8123&#x2f;a.webp\">"[..],
        b"<img src=\"http://localhost&#58;8123/b.webp\">",
        b"<img src=\"http&#58;&#47;&#47;localhost&#0058;8123&#47;c.webp\">",
    ] {
        assert_eq!(map.rewrite_body(body), body, "not rewritten");
        let origins = body_origins(body, &map, body.len(), true);
        assert_eq!(
            origins,
            BodyOrigins {
                unmapped: Vec::new(),
                unrewritten: vec!["http://localhost:8123".to_owned()],
            },
            "{}",
            String::from_utf8_lossy(body)
        );
    }
    let page = OriginMap::new("http://localhost", PORTAL);
    assert_eq!(
        body_origins(b"http://localhost:80/x", &page, 21, true).unrewritten,
        vec!["http://localhost".to_owned()]
    );
    // Every form a needle matches is no gap.
    let written = b"http://localhost:8123/a http:\\/\\/localhost:8123\\/b \
                    http&#x3A;&#x2F;&#x2F;localhost&#x3A;8123&#x2F;c http&#58;&#47;&#47;localhost&#58;8123";
    assert_eq!(
        body_origins(written, &map, written.len(), true),
        BodyOrigins::default()
    );
}

/// RB-090-75 review: the gap is logged once, the origin only, as unrewritten - not as an origin
/// to list in `assetOrigins`.
#[test]
fn a_rewrite_gap_is_logged_as_unrewritten() {
    let log = super::super::log_tests::Log::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    let _default = tracing::subscriber::set_default(subscriber);
    let map = OriginMap::new("http://localhost:8124", PORTAL);
    let body =
        b"<img src=\"http&#x3a;&#x2f;&#x2f;localhost&#x003a;8124&#x2f;a.webp?token=secret\">";
    log_unmapped_loopback(body, &map);
    log_unmapped_loopback(body, &map);
    let text = log.text();
    assert_eq!(
        text.matches("origin=http://localhost:8124").count(),
        1,
        "{text}"
    );
    assert!(text.contains("unrewritten"), "{text}");
    assert!(!text.contains("assetOrigins"), "{text}");
    assert!(
        !text.contains("secret") && !text.contains("a.webp"),
        "{text}"
    );
}
