use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, Finding, HttpResponse};

use super::*;
use crate::testing::short as lines;

/// 2026-11-12T00:00:00Z.
const NOW: u64 = 1_794_441_600;
const ORIGIN: &str = "https://shop.test";

fn file(kind: &str, text: &str) -> HttpResponse {
    HttpResponse {
        status: 200,
        headers: vec![("content-type".to_owned(), kind.to_owned())],
        body: text.as_bytes().to_vec(),
    }
}

fn host(files: &[(&str, HttpResponse)]) -> MemoryHost {
    let mut host = MemoryHost::new().grant(Capability::Http);
    host.now = NOW;
    for (path, response) in files {
        host.responses
            .insert(format!("{ORIGIN}{path}"), response.clone());
    }
    host
}

fn run(host: &mut MemoryHost) -> Vec<Finding> {
    let config = Config::default();
    let enabled = Vec::new();
    let mut sink = Sink::new(&config, &enabled);
    let start = Url::parse(&format!("{ORIGIN}/page")).expect("url");
    check(host, &config, &start, &mut sink);
    sink.findings
}

const SECURITY: &str = "Contact: mailto:security@shop.test\nExpires: 2027-06-30T00:00:00Z\n";
const LLMS: &str = "# Shop\n\n> Lamps.\n\n## Docs\n\n- [Guide](/guide.md)\n";

#[test]
fn both_files_in_order_give_no_finding_and_check_their_links_by_head() {
    let mut host = host(&[
        ("/llms.txt", file("text/plain", LLMS)),
        ("/guide.md", file("text/markdown", "# Guide")),
        (
            "/.well-known/security.txt",
            file("text/plain; charset=utf-8", SECURITY),
        ),
    ]);
    assert_eq!(lines(&run(&mut host), "wellknown"), Vec::<String>::new());
    assert!(
        host.fetched
            .iter()
            .any(|r| r.method == "HEAD" && r.url == "https://shop.test/guide.md")
    );
}

#[test]
fn missing_files_and_a_single_page_app_s_fallback_are_hints() {
    let page = file("text/html", "<!doctype html><title>App</title>");
    let mut host = host(&[("/llms.txt", page.clone()), ("/security.txt", page)]);
    assert_eq!(
        lines(&run(&mut host), "wellknown"),
        [
            "wellknown.llms_missing wellknown.llms_missing",
            "wellknown.security_missing wellknown.security_missing"
        ]
    );
}

#[test]
fn a_file_only_at_the_old_place_is_a_hint_and_still_checked() {
    let mut host = host(&[
        ("/llms.txt", file("text/plain", LLMS)),
        ("/guide.md", file("text/plain", "x")),
        ("/security.txt", file("text/html", SECURITY)),
    ]);
    let findings = run(&mut host);
    assert_eq!(
        lines(&findings, "wellknown"),
        [
            "wellknown.security_location wellknown.security_location",
            "wellknown.security_transport wellknown.security_type",
        ]
    );
    assert_eq!(
        findings[0].url.as_deref(),
        Some("https://shop.test/security.txt")
    );
}

#[test]
fn expired_far_doubled_and_unreadable_expiry_and_no_contact_are_named() {
    for (text, expected) in [
        (
            "Expires: 2026-01-01T00:00:00Z\n",
            vec!["security_contact", "security_expired"],
        ),
        (
            "Contact: a\nExpires: 2028-01-01T00:00:00Z\n",
            vec!["security_expires_far"],
        ),
        (
            "Contact: a\nExpires: 2027-01-01T00:00:00Z\nExpires: 2027-02-01T00:00:00Z\n",
            vec!["security_expires_multiple"],
        ),
        (
            "Contact: a\nExpires: soon\n",
            vec!["security_expires_invalid"],
        ),
        ("Contact: a\n", vec!["security_expires_missing"]),
    ] {
        let mut host = host(&[
            ("/llms.txt", file("text/plain", LLMS)),
            ("/guide.md", file("text/plain", "x")),
            ("/.well-known/security.txt", file("text/plain", text)),
        ]);
        let messages: Vec<String> = run(&mut host)
            .iter()
            .map(|f| {
                f.message
                    .trim_start_matches("plugin.seo.findings.wellknown.")
                    .to_owned()
            })
            .collect();
        assert_eq!(messages, expected, "{text}");
    }
}

#[test]
fn broken_links_of_llms_txt_are_named_and_a_head_refusal_falls_back_to_get() {
    let llms = "# Shop\n\n> Lamps.\n\n## Docs\n\n- [Gone](/gone.md)\n- [Get only](/get.md)\n";
    let mut host = host(&[
        ("/llms.txt", file("text/plain", llms)),
        ("/.well-known/security.txt", file("text/plain", SECURITY)),
    ]);
    // The memory host answers HEAD and GET alike; 405 for both shows the fallback's request.
    host.responses.insert(
        "https://shop.test/get.md".to_owned(),
        HttpResponse {
            status: 405,
            ..HttpResponse::default()
        },
    );
    let findings = run(&mut host);
    assert_eq!(
        lines(&findings, "wellknown"),
        [
            "wellknown.llms_link wellknown.llms_link",
            "wellknown.llms_link wellknown.llms_link",
        ]
    );
    let get = host
        .fetched
        .iter()
        .filter(|r| r.url == "https://shop.test/get.md")
        .map(|r| r.method.as_str())
        .collect::<Vec<_>>();
    assert_eq!(get, ["HEAD", "GET"]);
}

#[test]
fn the_group_switched_off_fetches_nothing() {
    let mut host = host(&[]);
    let config = Config {
        groups_off: vec!["wellknown".to_owned()],
        ..Config::default()
    };
    let enabled = Vec::new();
    let mut sink = Sink::new(&config, &enabled);
    let start = Url::parse(ORIGIN).expect("url");
    assert!(!wanted(&sink));
    check(&mut host, &config, &start, &mut sink);
    assert!(host.fetched.is_empty() && sink.findings.is_empty());
}
