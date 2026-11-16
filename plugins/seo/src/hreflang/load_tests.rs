use rb_plugin_api::HttpResponse;
use rb_plugin_api::test_support::MemoryHost;

use super::*;
use crate::testing::serve;

fn url(text: &str) -> Url {
    Url::parse(text).expect("url")
}

fn robots(host: &mut MemoryHost, origin: &str, text: &str) {
    host.responses.insert(
        format!("{origin}/robots.txt"),
        HttpResponse {
            status: 200,
            headers: vec![("content-type".to_owned(), "text/plain".to_owned())],
            body: text.as_bytes().to_vec(),
        },
    );
}

const PAGE: &str = r#"<html lang="de"><head><title>Rad</title>
<link rel="alternate" hreflang="en" href="https://a.test/en/">
<link rel="canonical" href="https://a.test/de/"></head>
<body><nav>Menu</nav><main>Ein Text <img src="a.jpg" alt=" Ein Bild. "></main></body></html>"#;

#[test]
fn a_member_keeps_what_the_rules_read() {
    let mut host = serve(
        "https://a.test/de/",
        PAGE,
        &[
            (
                "link",
                r#"<https://a.test/fr/>; rel="alternate"; hreflang="fr""#,
            ),
            ("content-language", "de, de-AT"),
        ],
    );
    robots(&mut host, "https://a.test", "User-agent: *\nAllow: /\n");
    let start = url("https://a.test/en/");
    let mut origins = Origins::new(Site::load(&mut host, &start));
    let sitemap = BTreeMap::from([(
        url("https://a.test/de/"),
        vec![Alternate::new("it", "https://a.test/it/", &start)],
    )]);
    let loaded = member(
        &mut host,
        &mut origins,
        &start,
        &url("https://a.test/de/"),
        &sitemap,
    );
    assert!(loaded.ok());
    let codes: Vec<(Source, &str)> = loaded
        .alternates()
        .map(|(source, a)| (source, a.code.as_str()))
        .collect();
    assert_eq!(
        codes,
        [
            (Source::Html, "en"),
            (Source::Header, "fr"),
            (Source::Sitemap, "it")
        ]
    );
    assert_eq!(loaded.lang.as_deref(), Some("de"));
    assert_eq!(loaded.content_language, ["de", "de-AT"]);
    assert_eq!(loaded.canonicals, ["https://a.test/de/"]);
    assert_eq!(loaded.text, "Ein Text");
    assert_eq!(loaded.alts, ["ein bild"]);
    assert!(loaded.declares(&url("https://a.test/fr/")));
}

#[test]
fn a_redirecting_member_is_not_followed() {
    let mut host = MemoryHost::new().grant(rb_plugin_api::Capability::Http);
    host.responses.insert(
        "https://a.test/de/".to_owned(),
        HttpResponse {
            status: 301,
            headers: vec![("location".to_owned(), "/de/home".to_owned())],
            body: Vec::new(),
        },
    );
    let start = url("https://a.test/en/");
    let mut origins = Origins::new(Site::load(&mut host, &start));
    let loaded = member(
        &mut host,
        &mut origins,
        &start,
        &url("https://a.test/de/"),
        &BTreeMap::new(),
    );
    assert_eq!(loaded.answer, Answer::Redirect(301));
    assert!(host.fetched.iter().all(|r| !r.url.ends_with("/de/home")));
}

#[test]
fn robots_txt_and_local_addresses_keep_a_member_unloaded() {
    let mut host = serve("https://a.test/private/de/", PAGE, &[]);
    robots(
        &mut host,
        "https://a.test",
        "User-agent: *\nDisallow: /private/\n",
    );
    let start = url("https://a.test/en/");
    let mut origins = Origins::new(Site::load(&mut host, &start));
    let blocked = member(
        &mut host,
        &mut origins,
        &start,
        &url("https://a.test/private/de/"),
        &BTreeMap::new(),
    );
    assert_eq!(blocked.answer, Answer::Skipped(Skipped::Robots));
    assert!(blocked.engine_blocked);
    let local = member(
        &mut host,
        &mut origins,
        &start,
        &url("http://192.168.1.2/de/"),
        &BTreeMap::new(),
    );
    assert_eq!(local.answer, Answer::Skipped(Skipped::Local));
    let fetched: Vec<&str> = host.fetched.iter().map(|r| r.url.as_str()).collect();
    assert_eq!(fetched, ["https://a.test/robots.txt"]);
}

#[test]
fn every_origin_reads_its_own_robots_txt_once_up_to_the_limit() {
    let mut host = MemoryHost::new().grant(rb_plugin_api::Capability::Http);
    let start = url("https://a.test/en/");
    let mut origins = Origins::new(Site::load(&mut host, &start));
    for index in 0..MAX_ORIGINS + 2 {
        let page = url(&format!("https://site{index}.test/de/"));
        let _ = member(&mut host, &mut origins, &start, &page, &BTreeMap::new());
        let _ = member(&mut host, &mut origins, &start, &page, &BTreeMap::new());
    }
    let robots = host
        .fetched
        .iter()
        .filter(|r| r.url.ends_with("/robots.txt"))
        .count();
    assert_eq!(robots, MAX_ORIGINS);
    let last = url(&format!("https://site{}.test/de/", MAX_ORIGINS + 1));
    let loaded = member(&mut host, &mut origins, &start, &last, &BTreeMap::new());
    assert_eq!(loaded.answer, Answer::Skipped(Skipped::Limit));
}
