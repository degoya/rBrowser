use rb_plugin_api::{Finding, Severity};

use super::{
    BREAKPOINT_CALLS, DUPLICATE, MAX_FINDINGS, N_PLUS_ONE, NO_CACHE, Page, SLOW, UNCOMPRESSED,
    describe, findings,
};
use crate::calls::{Call, Response};
use crate::endpoint_settings::Limits;
use crate::endpoints::tests::call;
use crate::endpoints::{Options, group};

fn pages() -> Vec<Page> {
    vec![
        Page {
            view: 1,
            label: "Mobile".to_owned(),
            url: "https://a.test/shop#top".to_owned(),
        },
        Page {
            view: 2,
            label: "Desktop".to_owned(),
            url: "https://a.test/shop".to_owned(),
        },
    ]
}

fn run(calls: &[Call], enabled: &[&str], only: Option<u32>) -> Vec<Finding> {
    let endpoints = group(calls, &Options::default());
    let enabled: Vec<String> = enabled.iter().map(|id| (*id).to_owned()).collect();
    findings(&endpoints, &pages(), &Limits::default(), &enabled, only)
}

fn rules(found: &[Finding]) -> Vec<(&str, Option<u32>)> {
    found
        .iter()
        .map(|finding| (finding.rule.as_str(), finding.view))
        .collect()
}

#[test]
fn every_rule_is_described_with_its_title() {
    let described = describe();
    assert_eq!(described.len(), 6);
    assert!(
        described
            .iter()
            .all(|rule| rb_plugin_api::check::valid_rule_id(&rule.id))
    );
    assert_eq!(
        described[0].title,
        "plugin.network-panel.rules.endpoint.duplicate"
    );
}

#[test]
fn the_same_call_twice_in_one_page_load_is_a_duplicate() {
    let calls = [
        call(1, "GET", "https://a.test/api/cart?token=secret", 1.0, 10.0),
        call(1, "GET", "https://a.test/api/cart?token=secret", 2.0, 10.0),
        call(2, "GET", "https://a.test/api/cart?token=secret", 3.0, 10.0),
    ];
    let found = run(&calls, &[DUPLICATE], None);
    assert_eq!(rules(&found), [(DUPLICATE, Some(1))]);
    let finding = &found[0];
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(
        finding.message,
        "plugin.network-panel.findings.endpoint.duplicate"
    );
    // The page without its fragment: the same finding in every view merges.
    assert_eq!(finding.url.as_deref(), Some("https://a.test/shop"));
    assert!(
        finding
            .params
            .contains(&("count".to_owned(), "2".to_owned()))
    );
    assert!(!format!("{finding:?}").contains("secret"));
}

#[test]
fn a_call_before_the_last_new_document_is_no_duplicate() {
    let mut old = call(1, "GET", "https://a.test/api/cart", 1.0, 10.0);
    old.current = false;
    let calls = [old, call(1, "GET", "https://a.test/api/cart", 2.0, 10.0)];
    assert!(run(&calls, &[DUPLICATE], None).is_empty());
}

#[test]
fn many_different_calls_of_one_endpoint_are_n_plus_one() {
    let calls: Vec<Call> = (0..5)
        .map(|at| {
            call(
                1,
                "GET",
                &format!("https://a.test/api/items/{at}"),
                f64::from(at),
                5.0,
            )
        })
        .collect();
    assert_eq!(
        rules(&run(&calls, &[N_PLUS_ONE], None)),
        [(N_PLUS_ONE, Some(1))]
    );
    assert!(run(&calls[..4], &[N_PLUS_ONE], None).is_empty());
}

#[test]
fn a_large_text_answer_without_compression_is_reported_a_compressed_one_not() {
    let mut plain = call(1, "GET", "https://a.test/api/big", 1.0, 5.0);
    plain.mime_type = Some("application/json".to_owned());
    plain.decoded_size = Some(50_000.0);
    plain.response = Some(Response::default());
    let mut packed = plain.clone();
    packed.url = "https://a.test/api/packed".to_owned();
    packed.response = Some(Response {
        encoding: Some("gzip".to_owned()),
        ..Response::default()
    });
    let mut image = plain.clone();
    image.url = "https://a.test/hero.png".to_owned();
    image.mime_type = Some("image/png".to_owned());
    let mut small = plain.clone();
    small.url = "https://a.test/api/small".to_owned();
    small.decoded_size = Some(500.0);
    let found = run(&[plain, packed, image, small], &[UNCOMPRESSED], None);
    assert_eq!(rules(&found), [(UNCOMPRESSED, Some(1))]);
    assert_eq!(found[0].path.as_deref(), Some("GET https://a.test/api/big"));
}

#[test]
fn a_get_answer_nothing_may_cache_is_reported() {
    let mut bare = call(1, "GET", "https://a.test/api/menu", 1.0, 5.0);
    bare.response = Some(Response::default());
    let mut tagged = bare.clone();
    tagged.url = "https://a.test/api/tagged".to_owned();
    tagged.response = Some(Response {
        etag: true,
        ..Response::default()
    });
    let mut post = bare.clone();
    post.method = "POST".to_owned();
    let mut unknown = bare.clone();
    unknown.url = "https://a.test/api/firefox".to_owned();
    unknown.response = None;
    let found = run(&[bare, tagged, post, unknown], &[NO_CACHE], None);
    assert_eq!(rules(&found), [(NO_CACHE, Some(1))]);
    assert_eq!(found[0].severity, Severity::Info);
}

#[test]
fn an_endpoint_whose_p95_reaches_the_limit_is_slow() {
    let calls = [
        call(1, "GET", "https://a.test/api/slow", 1.0, 200.0),
        call(1, "GET", "https://a.test/api/slow", 2.0, 1500.0),
        call(2, "GET", "https://a.test/api/slow", 3.0, 100.0),
    ];
    let found = run(&calls, &[SLOW], None);
    assert_eq!(rules(&found), [(SLOW, Some(1))]);
    assert!(
        found[0]
            .params
            .contains(&("p95".to_owned(), "1500".to_owned()))
    );
}

#[test]
fn a_breakpoint_calling_an_endpoint_more_often_is_reported_once_at_it() {
    let mut calls: Vec<Call> = (0..3)
        .map(|at| call(1, "GET", "https://a.test/api/teaser", f64::from(at), 5.0))
        .collect();
    calls.push(call(2, "GET", "https://a.test/api/teaser", 9.0, 5.0));
    let found = run(&calls, &[BREAKPOINT_CALLS], None);
    assert_eq!(rules(&found), [(BREAKPOINT_CALLS, Some(1))]);
    let breakpoints = found[0]
        .params
        .iter()
        .find(|(name, _)| name == "breakpoints")
        .map(|(_, value)| value.as_str());
    assert_eq!(breakpoints, Some("Mobile 3, Desktop 1"));
    // A view the run does not name gets none.
    assert!(run(&calls, &[BREAKPOINT_CALLS], Some(2)).is_empty());
}

#[test]
fn a_view_that_never_called_the_endpoint_counts_as_none() {
    let calls = [
        call(1, "GET", "https://a.test/api/a", 1.0, 5.0),
        call(1, "GET", "https://a.test/api/a", 2.0, 5.0),
        call(2, "GET", "https://a.test/api/b", 3.0, 5.0),
    ];
    let found = run(&calls, &[BREAKPOINT_CALLS], None);
    assert_eq!(rules(&found), [(BREAKPOINT_CALLS, Some(1))]);
}

#[test]
fn an_empty_list_of_rules_runs_every_rule_and_the_findings_are_capped() {
    let calls: Vec<Call> = (0..(MAX_FINDINGS as u32 + 10))
        .flat_map(|at| {
            let url = format!("https://a.test/api/{at}x");
            [
                call(1, "GET", &url, 1.0, 5.0),
                call(1, "GET", &url, 2.0, 5.0),
            ]
        })
        .collect();
    assert_eq!(run(&calls, &[], None).len(), MAX_FINDINGS);
}
