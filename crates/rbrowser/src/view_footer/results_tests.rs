use rb_ipc::BoundaryKind;

use super::*;

fn queries(signature: u64) -> PageQueries {
    PageQueries {
        signature,
        ..PageQueries::default()
    }
}

fn min(width: u32, label: Option<&str>) -> DetectedBreakpoint {
    DetectedBreakpoint {
        width,
        kind: BoundaryKind::Min,
        label: label.map(str::to_owned),
        count: 1,
        source: label.map(|label| format!("tailwind.config.js: {label}")),
        heuristic: false,
    }
}

fn own(widths: &[u32], skipped: usize) -> PageQueries {
    PageQueries {
        detected: widths.iter().map(|width| min(*width, None)).collect(),
        skipped,
        ..PageQueries::default()
    }
}

fn none() -> BTreeSet<&'static str> {
    BTreeSet::new()
}

#[test]
fn a_document_keeps_its_result_until_a_new_one() {
    let mut results = Results::default();
    assert!(results.get("https://a.test/").is_none());
    assert!(results.put("https://a.test/", queries(1), &none()));
    assert!(
        !results.put("https://a.test/", queries(1), &none()),
        "the same again"
    );
    assert!(results.put("https://a.test/", queries(2), &none()));
    assert_eq!(results.get("https://a.test/").map(|q| q.signature), Some(2));
}

#[test]
fn the_documents_used_longest_ago_go_first() {
    let mut results = Results::default();
    for index in 0..MAX_DOCUMENTS {
        results.put(&format!("https://a.test/{index}"), queries(1), &none());
    }
    // A footer names the first one: it is the newest now, though nothing was put.
    results.touch("https://a.test/0");
    results.touch("https://a.test/unknown");
    results.put("https://a.test/new", queries(1), &none());
    assert!(results.get("https://a.test/0").is_some());
    assert!(results.get("https://a.test/1").is_none());
    assert!(results.get("https://a.test/2").is_some());
    assert!(results.get("https://a.test/new").is_some());
    assert!(results.get("https://a.test/unknown").is_none());
}

#[test]
fn a_document_a_view_shows_is_never_put_out() {
    let mut results = Results::default();
    results.put("https://a.test/shown", queries(1), &none());
    let shown = BTreeSet::from(["https://a.test/shown"]);
    for index in 0..MAX_DOCUMENTS + 5 {
        results.put(&format!("https://a.test/{index}"), queries(1), &shown);
    }
    assert!(results.get("https://a.test/shown").is_some());
    assert!(results.get("https://a.test/0").is_none());
    // Every document shown: more than the bound are kept rather than one of them dropped.
    let mut all = Results::default();
    let names: Vec<String> = (0..MAX_DOCUMENTS + 2)
        .map(|index| format!("https://b.test/{index}"))
        .collect();
    let shown: BTreeSet<&str> = names.iter().map(String::as_str).collect();
    for name in &names {
        all.put(name, queries(1), &shown);
    }
    assert!(names.iter().all(|name| all.get(name).is_some()));
}

#[test]
fn clearing_forgets_every_document() {
    let mut results = Results::default();
    assert!(!results.clear());
    results.put("https://a.test/", queries(1), &none());
    assert!(results.clear());
    assert!(results.get("https://a.test/").is_none());
}

#[test]
fn an_incomplete_or_empty_read_never_replaces_the_manual_result() {
    // The CSS on a CDN without CORS: the document reads nothing of it.
    let manual = [min(640, Some("sm")), min(768, Some("md"))];
    for own in [
        own(&[], 0),
        own(&[], 1),
        own(&[500, 600, 700], 1),
        PageQueries {
            capped: true,
            ..own(&[500, 600, 700], 0)
        },
    ] {
        assert_eq!(choose(Some(&own), Some(&manual)), manual, "{own:?}");
        assert!(!own_counts(Some(&own), Some(&manual)));
    }
}

#[test]
fn of_two_results_the_one_that_saw_more_is_named() {
    let manual = [min(640, Some("sm")), min(768, Some("md"))];
    // Fewer boundaries than the analysis that fetched the sheets: the analysis.
    assert_eq!(choose(Some(&own(&[768], 0)), Some(&manual)), manual);
    // As many or more: the page as it is loaded, with the names the analysis gave.
    let chosen = choose(Some(&own(&[640, 768, 1024], 0)), Some(&manual));
    assert_eq!(
        chosen
            .iter()
            .map(|found| (found.width, found.label.as_deref()))
            .collect::<Vec<_>>(),
        [(640, Some("sm")), (768, Some("md")), (1024, None)]
    );
    assert_eq!(chosen[0].source.as_deref(), Some("tailwind.config.js: sm"));
    assert!(own_counts(Some(&own(&[640, 768, 1024], 0)), Some(&manual)));
}

#[test]
fn one_result_alone_is_named_as_it_is() {
    let manual = [min(640, Some("sm"))];
    assert_eq!(choose(None, Some(&manual)), manual);
    assert_eq!(choose(Some(&own(&[500], 2)), None), own(&[500], 2).detected);
    assert_eq!(
        choose(Some(&own(&[500], 2)), Some(&[])),
        own(&[500], 2).detected
    );
    assert!(own_counts(Some(&own(&[500], 2)), None));
    assert!(choose(None, None).is_empty());
    assert!(!own_counts(None, Some(&manual)));
}
