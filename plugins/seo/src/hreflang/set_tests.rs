use super::*;
use crate::hreflang::load::Answer;
use crate::hreflang::sources::{Alternate, Source};
use crate::url::Url;

fn page(path: &str, alts: &[&str]) -> Loaded {
    let url = Url::parse(&format!("https://a.test/{path}/")).expect("url");
    let mut page = Loaded::new(url, Answer::Status(200));
    page.alts = alts.iter().map(|alt| (*alt).to_owned()).collect();
    page
}

fn findings(run: impl FnOnce(&mut Sink<'_>), config: &Config) -> Vec<(String, String)> {
    let enabled = Vec::new();
    let mut sink = Sink::new(config, &enabled);
    run(&mut sink);
    sink.findings
        .iter()
        .map(|f| (f.rule.clone(), f.url.clone().unwrap_or_default()))
        .collect()
}

#[test]
fn an_alt_kept_in_german_on_the_french_page_is_one_hint() {
    let alt = "ein rotes fahrrad steht am strand";
    let pages = vec![
        (
            page("en", &["a red bicycle stands on the beach"]),
            "en".to_owned(),
        ),
        (page("de", &[alt, "logo"]), "de".to_owned()),
        (page("fr", &[alt]), "fr".to_owned()),
    ];
    let found = findings(|sink| alts(&pages, sink), &Config::default());
    assert_eq!(
        found,
        [(
            "images.alt_untranslated".to_owned(),
            "https://a.test/fr/".to_owned()
        )]
    );
}

#[test]
fn the_same_alt_in_one_language_or_a_short_one_is_fine() {
    let pages = vec![
        (
            page("en", &["a red bicycle on the beach", "acme"]),
            "en".to_owned(),
        ),
        (
            page("uk", &["a red bicycle on the beach", "acme"]),
            "en-GB".to_owned(),
        ),
        (page("de", &["acme"]), "de".to_owned()),
    ];
    assert!(findings(|sink| alts(&pages, sink), &Config::default()).is_empty());
}

#[test]
fn shared_titles_in_the_set_are_duplicates() {
    let mut en = page("en", &[]);
    en.title = Some("Fahrrad".to_owned());
    let mut de = page("de", &[]);
    de.title = Some("Fahrrad".to_owned());
    let pages = vec![(en, "en".to_owned()), (de, "de".to_owned())];
    let found = findings(|sink| duplicates(&pages, sink), &Config::default());
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|(rule, _)| rule == "meta.title_duplicate"));
}

#[test]
fn expected_locales_are_matched_by_code_or_bare_language() {
    let base = Url::parse("https://a.test/en/").expect("url");
    let mut start = Loaded::new(base.clone(), Answer::Status(200));
    start.declared.insert(
        Source::Html,
        ["en", "de-AT", "fr-FR"]
            .iter()
            .map(|code| Alternate::new(code, "https://a.test/x/", &base))
            .collect(),
    );
    let config = Config {
        locales: ["de", "fr-fr", "fr-BE", "it"].map(str::to_owned).to_vec(),
        ..Config::default()
    };
    let enabled = Vec::new();
    let mut sink = Sink::new(&config, &enabled);
    locales(&config, &start, &mut sink);
    let missing: Vec<&str> = sink
        .findings
        .iter()
        .map(|f| f.params[0].1.as_str())
        .collect();
    assert_eq!(missing, ["fr-BE", "it"]);
    let empty = Loaded::new(base, Answer::Status(200));
    let found = findings(
        |sink| locales(&Config::default(), &empty, sink),
        &Config::default(),
    );
    assert_eq!(found[0].0, "hreflang.none");
}
