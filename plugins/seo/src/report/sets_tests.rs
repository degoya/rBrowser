use super::*;
use crate::hreflang::matrix::{Pair, SetPage};
use crate::parity::ParityView;

fn page(url: &str, code: &str) -> SetPage {
    SetPage {
        url: url.to_owned(),
        code: code.to_owned(),
        answer: "200".to_owned(),
    }
}

#[test]
fn the_matrix_is_a_table_with_headers_and_lists_exactly_the_missing_pairs() {
    use Cell::{Missing, Own, Yes};
    let set = SetReport {
        pages: vec![
            page("https://a.test/en/", "en"),
            page("https://a.test/<de>/", ""),
        ],
        matrix: vec![vec![Own, Yes], vec![Missing, Own]],
        missing: vec![Pair {
            from: "https://a.test/<de>/".to_owned(),
            to: "https://a.test/en/".to_owned(),
        }],
    };
    let mut out = String::new();
    hreflang(&mut out, &set);
    assert!(out.contains("<caption>"));
    assert_eq!(out.matches("<th scope=\"col\">").count(), 5);
    assert_eq!(out.matches("<th scope=\"row\">").count(), 2);
    assert_eq!(out.matches("class=\"missing\"").count(), 1);
    assert_eq!(out.matches("<li>").count(), 1);
    assert!(out.contains("&lt;de&gt;"), "escaped");
    assert!(!out.contains("<de>"));
    assert!(
        out.contains(">2</abbr>"),
        "a page without a code is named by its number"
    );
    assert!(out.contains("Missing"), "spelled out: {out}");
}

#[test]
fn a_complete_set_says_nothing_is_missing() {
    let set = SetReport {
        pages: vec![page("https://a.test/en/", "en")],
        matrix: vec![vec![Cell::Own]],
        missing: Vec::new(),
    };
    let mut out = String::new();
    hreflang(&mut out, &set);
    assert!(!out.contains("<li>"));
    assert!(out.contains(&label("matrix.none_missing")));
}

#[test]
fn the_parity_table_marks_the_reference() {
    let view = |id, breakpoint: &str| ParityView {
        view: id,
        breakpoint: breakpoint.to_owned(),
        width: 375,
        title: "T".to_owned(),
        h1: "-".to_owned(),
        canonical: "-".to_owned(),
        hreflang: "-".to_owned(),
        structured_data: "<b>".to_owned(),
        text_similarity: None,
        text: None,
    };
    let report = ParityReport {
        reference: 2,
        views: vec![view(1, "desktop"), view(2, "mobile")],
    };
    let mut out = String::new();
    parity(&mut out, &report);
    assert_eq!(out.matches("<th scope=\"row\">").count(), 2);
    assert!(out.contains(&format!("mobile ({})", label("parity.reference"))));
    assert!(out.contains("&lt;b&gt;"));
}
