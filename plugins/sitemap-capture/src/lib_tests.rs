use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse};

use super::*;

const SITE: &str = "https://www.example.test";

fn fixture_host() -> MemoryHost {
    let mut host = MemoryHost::new().grant(Capability::Http);
    let files: [(&str, &[u8]); 5] = [
        (
            "robots.txt",
            include_bytes!("../../../tests/fixtures/sitemap/robots.txt"),
        ),
        (
            "sitemap_index.xml",
            include_bytes!("../../../tests/fixtures/sitemap/sitemap_index.xml"),
        ),
        (
            "sitemap-pages.xml",
            include_bytes!("../../../tests/fixtures/sitemap/sitemap-pages.xml"),
        ),
        (
            "sitemap-nested-index.xml",
            include_bytes!("../../../tests/fixtures/sitemap/sitemap-nested-index.xml"),
        ),
        (
            "sitemap-blog.xml.gz",
            include_bytes!("../../../tests/fixtures/sitemap/sitemap-blog.xml.gz"),
        ),
    ];
    for (name, bytes) in files {
        host.responses.insert(
            format!("{SITE}/{name}"),
            HttpResponse {
                status: 200,
                headers: Vec::new(),
                body: bytes.to_vec(),
            },
        );
    }
    host
}

fn call(host: &mut MemoryHost, tool: &str, args: &Value) -> Result<Value, Failure> {
    SitemapCapture
        .call_tool(host, tool, &args.to_string())
        .map(|text| serde_json::from_str(&text).expect("JSON"))
}

#[test]
fn discover_finds_selects_and_rewrites_the_fixture_site() {
    let mut host = fixture_host();
    let args = json!({
        "source": { "site": SITE },
        "selection": { "exclude": ["/blog/**"], "respectRobots": true },
        "rewrite": { "to": "http://127.0.0.1:8080" }
    });
    let answer = call(&mut host, DISCOVER, &args).expect("discovers");
    assert_eq!(answer["site"], SITE);
    assert_eq!(answer["counts"]["found"], 19);
    assert_eq!(answer["counts"]["pattern"], 7);
    assert_eq!(answer["counts"]["selected"], 12);
    assert_eq!(answer["pages"][1]["url"], "http://127.0.0.1:8080/about");
    assert_eq!(answer["pages"][1]["sitemapUrl"], format!("{SITE}/about"));
    assert_eq!(answer["sitemaps"].as_array().map(Vec::len), Some(4));
    assert_eq!(answer["truncated"], false);
    assert_eq!(answer["foreign"], 0);
    assert_eq!(answer["counts"]["origin"], 0);
    assert!(host.logs.iter().any(|(_, line)| line.contains("19 pages")));
}

#[test]
fn discover_takes_a_file_and_a_url_list() {
    let mut host = fixture_host();
    let file = base64::engine::general_purpose::STANDARD.encode(include_bytes!(
        "../../../tests/fixtures/sitemap/sitemap-blog.xml.gz"
    ));
    let answer = call(&mut host, DISCOVER, &json!({ "source": { "file": file } })).expect("file");
    assert_eq!(answer["counts"]["selected"], 7);
    assert_eq!(answer["site"], Value::Null);
    let urls = json!({ "source": { "urls": [format!("{SITE}/a"), "nope"] } });
    let answer = call(&mut host, DISCOVER, &urls).expect("list");
    assert_eq!(answer["counts"]["selected"], 1);
    assert_eq!(answer["skipped"], 1);
}

#[test]
fn discover_refuses_bad_arguments_with_the_plugins_codes() {
    let mut host = fixture_host();
    let code = |result: Result<Value, Failure>| result.expect_err("refused").code;
    let two = json!({ "source": { "site": SITE, "sitemap": SITE } });
    assert_eq!(
        code(call(&mut host, DISCOVER, &two)),
        Some(error_key("bad_args"))
    );
    let none = json!({});
    assert_eq!(
        code(call(&mut host, DISCOVER, &none)),
        Some(error_key("bad_args"))
    );
    let ftp = json!({ "source": { "site": "ftp://x.test/" } });
    assert_eq!(
        code(call(&mut host, DISCOVER, &ftp)),
        Some(error_key("bad_url"))
    );
    let pattern = json!({ "source": { "urls": [] }, "selection": { "include": ["re:("] } });
    assert_eq!(
        code(call(&mut host, DISCOVER, &pattern)),
        Some(error_key("bad_pattern"))
    );
    let file = json!({ "source": { "file": "not base64!" } });
    assert_eq!(
        code(call(&mut host, DISCOVER, &file)),
        Some(error_key("bad_args"))
    );
    let unknown = json!({ "source": { "site": SITE, "extra": 1 } });
    assert_eq!(
        code(call(&mut host, DISCOVER, &unknown)),
        Some(error_key("bad_args"))
    );
}

#[test]
fn gallery_renders_a_run() {
    let mut host = MemoryHost::new();
    let args = json!({
        "run": {
            "runId": "r1",
            "breakpoints": [{ "name": "mobile", "width": 375, "height": 812 }],
            "pages": [{ "url": "https://a.test/", "slug": "index", "status": 404,
                        "shots": { "mobile": { "state": "done" } } }]
        }
    });
    let answer = call(&mut host, GALLERY, &args).expect("renders");
    let html = answer["html"].as_str().expect("html");
    assert!(html.contains("index/mobile.png"));
    assert!(html.contains("HTTP 404"));
}

#[test]
fn unknown_tools_and_commands_are_refused() {
    let mut host = MemoryHost::new();
    let failure = SitemapCapture
        .call_tool(&mut host, "nope", "{}")
        .expect_err("unknown");
    assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    let failure = SitemapCapture
        .run_command(&mut host, "nope", "{}")
        .expect_err("unknown");
    assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
}

#[test]
fn every_error_code_has_its_text() {
    let catalogue: Value =
        serde_json::from_str(include_str!("../locales/en.json")).expect("the catalogue");
    let errors = &catalogue["plugin"]["sitemap-capture"]["errors"];
    for name in [
        "bad_args",
        "bad_url",
        "bad_pattern",
        "too_many_patterns",
        "bad_date",
        "bad_rewrite",
        "fetch",
        "http_status",
        "redirect_scheme",
        "redirect_origin",
        "private_address",
        "redirect_loop",
        "redirect_invalid",
        "too_large",
        "too_deep",
        "gzip",
        "encoding",
        "xml",
        "not_a_sitemap",
    ] {
        assert!(errors[name].is_string(), "errors.{name}");
    }
}
