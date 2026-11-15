use super::*;

fn url(text: &str) -> Url {
    Url::parse(text).expect("url")
}

fn codes() -> Vec<(Url, String)> {
    vec![
        (url("https://a.test/en/"), "en".to_owned()),
        (url("https://a.test/de/"), "de-AT".to_owned()),
        (url("https://a.test/uk/"), "en-UK".to_owned()),
    ]
}

fn run(config: &Config, page: &Loaded) -> Vec<String> {
    let enabled = Vec::new();
    let mut sink = Sink::new(config, &enabled);
    sink.page(&page.url.to_string(), None);
    let by_url = codes();
    check(config, page, &Codes { by_url: &by_url }, &mut sink);
    sink.findings
        .iter()
        .map(|finding| {
            let params: Vec<String> = finding
                .params
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            format!(
                "{} {}",
                finding
                    .message
                    .strip_prefix("plugin.seo.findings.")
                    .unwrap_or_default(),
                params.join(" ")
            )
            .trim_end()
            .to_owned()
        })
        .collect()
}

fn de() -> Loaded {
    let mut page = Loaded::new(url("https://a.test/de/"), Answer::Status(200));
    page.lang = Some("de-AT".to_owned());
    page.canonicals = vec!["https://a.test/de/".to_owned()];
    page
}

#[test]
fn a_good_target_has_no_finding() {
    assert!(run(&Config::default(), &de()).is_empty());
}

#[test]
fn status_redirect_failure_and_skips_are_named() {
    let config = Config::default();
    let mut page = de();
    page.answer = Answer::Status(404);
    assert_eq!(run(&config, &page), ["hreflang.target_status detail=404"]);
    page.answer = Answer::Redirect(301);
    assert_eq!(run(&config, &page), ["hreflang.target_redirect detail=301"]);
    page.answer = Answer::Failed("plugins.error.http_failed".to_owned());
    assert_eq!(
        run(&config, &page),
        ["hreflang.target_failed detail=plugins.error.http_failed"]
    );
    page.answer = Answer::Skipped(Skipped::Robots);
    page.engine_blocked = true;
    assert_eq!(
        run(&config, &page),
        ["hreflang.target_robots", "hreflang.unchecked_robots"]
    );
}

#[test]
fn noindex_and_a_canonical_to_another_language_are_errors() {
    let mut page = de();
    page.noindex = true;
    page.canonicals = vec!["/en/".to_owned()];
    assert_eq!(
        run(&Config::default(), &page),
        [
            "hreflang.target_noindex",
            "hreflang.target_canonical_language canonical=https://a.test/en/ code=en"
        ]
    );
    let mut page = de();
    page.canonicals = vec!["https://a.test/de/home".to_owned()];
    assert_eq!(
        run(&Config::default(), &page),
        ["hreflang.target_canonical canonical=https://a.test/de/home"]
    );
}

#[test]
fn lang_and_content_language_must_name_the_code() {
    let mut page = de();
    page.lang = Some("de-CH".to_owned());
    page.content_language = vec!["en".to_owned(), "fr".to_owned()];
    assert_eq!(
        run(&Config::default(), &page),
        [
            "hreflang.lang_mismatch lang=de-CH code=de-AT",
            "hreflang.content_language_mismatch value=en, fr code=de-AT"
        ]
    );
    page.lang = Some("de".to_owned());
    page.content_language = vec!["de-AT".to_owned()];
    assert!(run(&Config::default(), &page).is_empty());
}

#[test]
fn the_text_is_held_to_the_language_only_when_asked() {
    let mut page = de();
    page.text = "The quick brown fox jumps over the lazy dog and runs far away into the \
                 forest, where nobody will ever find it again, not even the hunters."
        .to_owned();
    assert!(run(&Config::default(), &page).is_empty());
    let config = Config {
        check_content_language: true,
        ..Config::default()
    };
    assert_eq!(
        run(&config, &page),
        ["hreflang.content_detected language=English code=de-AT"]
    );
}

#[test]
fn an_invalid_code_is_not_held_to_a_language() {
    let mut page = Loaded::new(url("https://a.test/uk/"), Answer::Status(200));
    page.lang = Some("fr".to_owned());
    assert!(run(&Config::default(), &page).is_empty());
}
