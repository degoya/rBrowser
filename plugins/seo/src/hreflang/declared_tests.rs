use super::*;
use crate::hreflang::load::Answer;
use crate::hreflang::sources::Alternate;
use crate::url::Url;

fn url(text: &str) -> Url {
    Url::parse(text).expect("url")
}

fn page(sources: &[(Source, &[(&str, &str)])]) -> Loaded {
    let base = url("https://a.test/en/");
    let mut page = Loaded::new(base.clone(), Answer::Status(200));
    for (source, list) in sources {
        page.declared.insert(
            *source,
            list.iter()
                .map(|(code, href)| Alternate::new(code, href, &base))
                .collect(),
        );
    }
    page
}

fn run(config: &Config, page: &Loaded) -> Vec<String> {
    let enabled = Vec::new();
    let mut sink = Sink::new(config, &enabled);
    sink.page(&page.url.to_string(), None);
    check(config, page, &mut sink);
    sink.findings
        .iter()
        .map(|finding| {
            let mut params: Vec<String> = finding
                .params
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            params.sort();
            format!(
                "{} {}",
                finding
                    .message
                    .strip_prefix("plugin.seo.findings.")
                    .unwrap_or_default(),
                params.join(" ")
            )
        })
        .collect()
}

const EN: (&str, &str) = ("en", "https://a.test/en/");
const DE: (&str, &str) = ("de", "https://a.test/de/");
const XD: (&str, &str) = ("x-default", "https://a.test/en/");

#[test]
fn a_complete_set_in_one_source_has_no_finding() {
    let page = page(&[(Source::Html, &[EN, DE, XD])]);
    assert!(run(&Config::default(), &page).is_empty());
    assert!(
        run(
            &Config::default(),
            &Loaded::new(url("https://a.test/"), Answer::Status(200))
        )
        .is_empty()
    );
}

#[test]
fn bad_codes_relative_hrefs_and_duplicates_are_named() {
    let page = page(&[(
        Source::Html,
        &[
            EN,
            XD,
            ("en-UK", "https://a.test/uk/"),
            ("de_DE", "/de/"),
            ("de-de", "https://a.test/de2/"),
            ("it", "https://a.test/uk/"),
        ],
    )]);
    assert_eq!(
        run(&Config::default(), &page),
        [
            "hreflang.code_suggestion code=en-UK suggestion=en-GB",
            "hreflang.code_suggestion code=de_DE suggestion=de-DE",
            "hreflang.target_relative code=de_DE href=/de/",
            "hreflang.duplicate_url codes=en-UK, it source=html url=https://a.test/uk/",
        ]
    );
}

#[test]
fn the_same_code_twice_is_a_duplicate() {
    let page = page(&[(Source::Html, &[EN, XD, DE, ("DE", "https://a.test/de/")])]);
    assert_eq!(
        run(&Config::default(), &page),
        [
            "hreflang.duplicate_code code=de count=2 source=html",
            "hreflang.duplicate_url codes=de, DE source=html url=https://a.test/de/",
        ]
    );
}

#[test]
fn self_and_x_default_are_required_the_latter_by_setting() {
    let page = page(&[(Source::Html, &[DE])]);
    assert_eq!(
        run(&Config::default(), &page),
        ["hreflang.self_missing ", "hreflang.x_default_missing "]
    );
    let config = Config {
        require_x_default: false,
        ..Config::default()
    };
    assert_eq!(run(&config, &page), ["hreflang.self_missing "]);
}

#[test]
fn the_sources_must_agree() {
    let page = page(&[
        (Source::Html, &[EN, XD, DE]),
        (Source::Header, &[EN, XD, ("de", "https://a.test/fr/")]),
        (Source::Sitemap, &[EN, XD, DE, ("es", "https://a.test/es/")]),
    ]);
    assert_eq!(
        run(&Config::default(), &page),
        [
            "hreflang.source_conflict code=de values=html: https://a.test/de/; header: https://a.test/fr/; sitemap: https://a.test/de/",
            "hreflang.source_missing code=es in=sitemap missing=html, header",
        ]
    );
}

#[test]
fn keys_compare_codes_in_any_case() {
    assert_eq!(key("DE-de"), "de-de");
    assert_eq!(key("X-Default"), "x-default");
    assert_eq!(key(" en-UK "), "en-uk");
}
