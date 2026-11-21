use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse};

use super::*;

fn answer(status: u16, headers: &[(&str, &str)]) -> HttpResponse {
    HttpResponse {
        status,
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        body: Vec::new(),
    }
}

fn host() -> MemoryHost {
    MemoryHost::new().grant(Capability::Http)
}

#[test]
fn the_document_is_asked_for_with_head_and_its_headers_read() {
    let mut host = host();
    host.responses.insert(
        "https://example.test/".to_owned(),
        answer(
            200,
            &[
                ("Date", "Fri, 20 Nov 2026 10:00:00 GMT"),
                ("X-Frame-Options", "DENY"),
            ],
        ),
    );
    let fetched = document(&mut host, "https://example.test/").expect("fetched");
    assert_eq!(fetched.status, 200);
    assert_eq!(
        fetched.headers.get("x-frame-options").as_deref(),
        Some("DENY")
    );
    assert_eq!(host.fetched.len(), 1);
    assert_eq!(host.fetched[0].method, "HEAD");
    assert!(
        host.fetched[0]
            .headers
            .iter()
            .all(|(name, _)| name != "cookie")
    );
}

#[test]
fn redirects_are_followed_and_their_cookies_kept() {
    let mut host = host();
    host.responses.insert(
        "http://example.test/a".to_owned(),
        answer(
            301,
            &[
                ("Location", "https://example.test/a"),
                ("Set-Cookie", "first=SECRET; Path=/"),
            ],
        ),
    );
    host.responses.insert(
        "https://example.test/a".to_owned(),
        answer(302, &[("Location", "b?x=1")]),
    );
    host.responses.insert(
        "https://example.test/b?x=1".to_owned(),
        answer(
            200,
            &[("date", "now"), ("set-cookie", "second=SECRET; Secure")],
        ),
    );
    let fetched = document(&mut host, "http://example.test/a").expect("fetched");
    assert_eq!(fetched.final_url, "https://example.test/b?x=1");
    assert_eq!(fetched.url, "http://example.test/a");
    assert_eq!(fetched.redirects, 2);
    let names: Vec<&str> = fetched
        .cookies
        .iter()
        .map(|cookie| cookie.name.as_str())
        .collect();
    assert_eq!(names, ["first", "second"]);
    assert_eq!(fetched.downgrade, None);
    assert!(!fetched.headers.has("set-cookie"));
    assert!(!format!("{fetched:?}").contains("SECRET"));
}

#[test]
fn a_redirect_loop_ends_with_a_failure() {
    let mut host = host();
    host.responses.insert(
        "https://example.test/".to_owned(),
        answer(302, &[("Location", "/")]),
    );
    let failure = document(&mut host, "https://example.test/").expect_err("loop");
    assert_eq!(
        failure.code(),
        Some("plugin.security-headers.errors.redirect")
    );
    assert_eq!(host.fetched.len(), MAX_REDIRECTS + 1);
}

#[test]
fn a_redirect_out_of_the_web_is_refused() {
    for location in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "https://user@evil.test/",
        "",
    ] {
        let mut host = host();
        host.responses.insert(
            "https://example.test/".to_owned(),
            answer(302, &[("Location", location)]),
        );
        let failure = document(&mut host, "https://example.test/").expect_err(location);
        assert_eq!(
            failure.code(),
            Some("plugin.security-headers.errors.redirect"),
            "{location}"
        );
        assert_eq!(host.fetched.len(), 1);
    }
}

#[test]
fn a_refused_head_is_asked_again_with_get() {
    struct HeadRefused(MemoryHost);
    impl PluginHost for HeadRefused {
        fn log(&mut self, level: &str, message: &str) {
            self.0.log(level, message);
        }
        fn now_unix_seconds(&mut self) -> u64 {
            0
        }
        fn setting(&mut self, key: &str) -> Option<String> {
            self.0.setting(key)
        }
        fn views(&mut self) -> Result<Vec<rb_plugin_api::ViewInfo>, Failure> {
            self.0.views()
        }
        fn active_view(&mut self) -> Result<Option<rb_plugin_api::ViewId>, Failure> {
            self.0.active_view()
        }
        fn evaluate(
            &mut self,
            view: rb_plugin_api::ViewId,
            script: &str,
        ) -> Result<String, Failure> {
            self.0.evaluate(view, script)
        }
        fn cdp_send(
            &mut self,
            view: rb_plugin_api::ViewId,
            method: &str,
            params: &str,
        ) -> Result<String, Failure> {
            self.0.cdp_send(view, method, params)
        }
        fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, Failure> {
            self.0.fetch(request)?;
            Ok(if request.method == "HEAD" {
                answer(405, &[("date", "now")])
            } else {
                answer(200, &[("date", "now"), ("referrer-policy", "no-referrer")])
            })
        }
        fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
            self.0.project_read(path)
        }
        fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
            self.0.project_list(dir)
        }
        fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
            self.0.out_write(path, bytes)
        }
        fn report_findings(&mut self, items: &[rb_plugin_api::Finding]) -> Result<(), Failure> {
            self.0.report_findings(items)
        }
        fn sidecar_call(&mut self, method: &str, params: &str) -> Result<String, Failure> {
            self.0.sidecar_call(method, params)
        }
    }
    let mut host = HeadRefused(host());
    let fetched = document(&mut host, "https://example.test/").expect("fetched");
    assert!(fetched.headers.has("referrer-policy"));
    let methods: Vec<&str> = host
        .0
        .fetched
        .iter()
        .map(|request| request.method.as_str())
        .collect();
    assert_eq!(methods, ["HEAD", "GET"]);
}

#[test]
fn an_answer_the_host_filtered_is_not_judged() {
    let mut host = host();
    host.responses.insert(
        "https://example.test/".to_owned(),
        answer(
            200,
            &[("content-type", "text/html"), ("content-length", "10")],
        ),
    );
    let failure = document(&mut host, "https://example.test/").expect_err("filtered");
    assert_eq!(
        failure.code(),
        Some("plugin.security-headers.errors.headers_unavailable")
    );
}

#[test]
fn without_the_capability_nothing_is_fetched() {
    let failure = document(&mut MemoryHost::new(), "https://example.test/").expect_err("denied");
    assert_eq!(
        failure.code(),
        Some(rb_plugin_api::codes::CAPABILITY_DENIED)
    );
}

#[test]
fn locations_resolve_against_the_current_url() {
    let base = "https://example.test/shop/item?x=1#top";
    assert_eq!(
        resolve(base, "/a").as_deref(),
        Some("https://example.test/a")
    );
    assert_eq!(
        resolve(base, "b").as_deref(),
        Some("https://example.test/shop/b")
    );
    assert_eq!(
        resolve(base, "//cdn.example.test/c").as_deref(),
        Some("https://cdn.example.test/c")
    );
    assert_eq!(
        resolve(base, "http://other.test/").as_deref(),
        Some("http://other.test/")
    );
    assert_eq!(
        resolve("https://example.test", "d").as_deref(),
        Some("https://example.test/d")
    );
    assert_eq!(resolve(base, &"x".repeat(5000)), None);
    assert_eq!(resolve(base, "data:text/html,hi"), None);
    assert_eq!(resolve(base, "javascript:alert(1)"), None);
    assert_eq!(
        resolve(base, "./c:d").as_deref(),
        Some("https://example.test/shop/./c:d")
    );
}

#[test]
fn a_cookie_a_later_hop_deletes_is_gone() {
    let mut host = host();
    host.responses.insert(
        "https://example.test/login".to_owned(),
        answer(
            302,
            &[
                ("Location", "/home"),
                ("Set-Cookie", "temp=; Path=/"),
                ("Set-Cookie", "sid=; Path=/"),
            ],
        ),
    );
    host.responses.insert(
        "https://example.test/home".to_owned(),
        answer(
            200,
            &[
                ("date", "Fri, 20 Nov 2026 10:00:00 GMT"),
                ("set-cookie", "temp=; Max-Age=0"),
                ("set-cookie", "old=; Expires=Fri, 20 Nov 2026 09:00:00 GMT"),
            ],
        ),
    );
    let fetched = document(&mut host, "https://example.test/login").expect("fetched");
    let names: Vec<&str> = fetched
        .cookies
        .iter()
        .map(|cookie| cookie.name.as_str())
        .collect();
    assert_eq!(names, ["sid"]);
}

#[test]
fn an_https_page_redirecting_to_http_is_a_downgrade() {
    let mut host = host();
    host.responses.insert(
        "https://example.test/".to_owned(),
        answer(301, &[("Location", "http://example.test/plain")]),
    );
    host.responses.insert(
        "http://example.test/plain".to_owned(),
        answer(301, &[("Location", "https://example.test/again")]),
    );
    host.responses.insert(
        "https://example.test/again".to_owned(),
        answer(200, &[("date", "now")]),
    );
    let fetched = document(&mut host, "https://example.test/").expect("fetched");
    assert_eq!(
        fetched.downgrade.as_deref(),
        Some("http://example.test/plain")
    );
    assert_eq!(fetched.final_url, "https://example.test/again");
}

#[test]
fn a_query_or_fragment_alone_keeps_the_path() {
    let base = "https://example.test/shop/item?x=1#top";
    assert_eq!(
        resolve(base, "?y=2").as_deref(),
        Some("https://example.test/shop/item?y=2")
    );
    assert_eq!(
        resolve(base, "#end").as_deref(),
        Some("https://example.test/shop/item?x=1#end")
    );
    assert_eq!(
        resolve("https://example.test", "?a=1").as_deref(),
        Some("https://example.test/?a=1")
    );
}
