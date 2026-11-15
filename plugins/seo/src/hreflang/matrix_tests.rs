use super::*;
use crate::config::Config;
use crate::hreflang::sources::{Alternate, Source};
use crate::url::Url;

fn page(path: &str, links: &[&str], answer: Answer) -> Loaded {
    let url = Url::parse(&format!("https://a.test/{path}/")).expect("url");
    let mut page = Loaded::new(url.clone(), answer);
    page.declared.insert(
        Source::Html,
        links
            .iter()
            .map(|link| Alternate::new(link, &format!("/{link}/"), &url))
            .collect(),
    );
    page
}

#[test]
fn the_matrix_names_exactly_the_missing_pairs() {
    let ok = Answer::Status(200);
    let pages = vec![
        (
            page("en", &["en", "de", "fr", "es"], ok.clone()),
            "en".to_owned(),
        ),
        (page("de", &["en", "de", "fr"], ok.clone()), "de".to_owned()),
        (page("fr", &["fr", "de"], ok.clone()), "fr".to_owned()),
        (page("es", &[], Answer::Status(404)), "es".to_owned()),
    ];
    let config = Config::default();
    let enabled = Vec::new();
    let mut sink = Sink::new(&config, &enabled);
    let report = build(&pages, &mut sink);
    use Cell::{Missing, Own, Unknown, Yes};
    assert_eq!(
        report.matrix,
        [
            [Own, Yes, Yes, Unknown],
            [Yes, Own, Yes, Unknown],
            [Missing, Yes, Own, Unknown],
            [Unknown, Unknown, Unknown, Own],
        ]
    );
    assert_eq!(
        report.missing,
        [Pair {
            from: "https://a.test/fr/".to_owned(),
            to: "https://a.test/en/".to_owned()
        }]
    );
    let found: Vec<(&str, Option<&str>)> = sink
        .findings
        .iter()
        .map(|f| (f.rule.as_str(), f.url.as_deref()))
        .collect();
    assert_eq!(
        found,
        [("hreflang.return_missing", Some("https://a.test/fr/"))]
    );
    assert_eq!(report.pages[3].answer, "404");
}

#[test]
fn a_pair_neither_page_links_is_an_incomplete_set() {
    let ok = Answer::Status(200);
    let pages = vec![
        (page("en", &["en", "de"], ok.clone()), "en".to_owned()),
        (page("de", &["de", "en"], ok.clone()), "de".to_owned()),
        (page("fr", &["fr"], ok), "fr".to_owned()),
    ];
    let config = Config::default();
    let enabled = Vec::new();
    let mut sink = Sink::new(&config, &enabled);
    let report = build(&pages, &mut sink);
    assert_eq!(report.missing.len(), 4);
    assert!(
        sink.findings
            .iter()
            .all(|f| f.rule == "hreflang.set_incomplete")
    );
}
