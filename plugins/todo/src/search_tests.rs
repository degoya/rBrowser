use rb_plugin_api::feature::Feature;
use serde_json::{Value, json};

use super::{MAX_QUERY_CHARS, Query, filter};
use crate::Todo;
use crate::fake_host::FakeHost;
use crate::format::Document;
use crate::items::items;

const WEBSITE: &str = include_str!("../tests/fixtures/website.todo");
/// `Menu:` with umlaut, a task `Creme brulee` with its accents, tagged `@high`.
const CAFE: &str = "Men\u{fc}:\n  \u{2610} Cr\u{e8}me br\u{fb}l\u{e9}e @high\n";

fn host() -> FakeHost {
    FakeHost::with(&[
        ("website.todo", WEBSITE),
        ("cafe.todo", CAFE),
        ("docs/plan.todo", "Plan:\n  \u{2610} Write it\n"),
    ])
}

fn list(host: &mut FakeHost, args: &Value) -> Result<Value, rb_plugin_api::Failure> {
    Todo.call_tool(host, "list", &args.to_string())
        .map(|text| serde_json::from_str(&text).expect("JSON"))
}

/// The lines of a page's items and whether each matched.
fn lines(page: &Value) -> Vec<(u64, bool)> {
    page["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| {
            (
                item["line"].as_u64().expect("line"),
                item["match"].as_bool().expect("match"),
            )
        })
        .collect()
}

fn query(text: &str) -> Query {
    Query::parse(text).expect("terms")
}

#[test]
fn a_query_reads_words_and_tag_terms_and_none_without_a_term() {
    assert_eq!(
        Query::parse("  Caf\u{e9}  @HIGH @ @est(2h) "),
        Some(Query {
            words: vec!["cafe".to_owned()],
            tags: vec!["high".to_owned(), "est".to_owned()],
        })
    );
    assert_eq!(Query::parse("   "), None);
    assert_eq!(Query::parse("@ @("), None);
}

#[test]
fn an_item_matches_its_text_or_tags_every_term_at_once() {
    let all = items(&Document::parse(WEBSITE));
    let item = |line: u64| {
        all.iter()
            .find(|item| item["line"] == line)
            .expect("item")
            .clone()
    };
    // Line 2: "Write the *landing* page @high @est(2h)"; line 14: "Tagged ... @owner(alex)".
    assert!(query("LANDING page").matches(&item(2)));
    assert!(!query("landing colours").matches(&item(2)));
    assert!(query("alex").matches(&item(14)));
    assert!(query("@hi").matches(&item(2)));
    assert!(query("@est").matches(&item(2)));
    assert!(!query("@igh").matches(&item(2)));
    assert!(!query("@high").matches(&item(14)));
}

#[test]
fn the_matches_keep_their_headings_projects_and_parents_as_context() {
    let all = items(&Document::parse(
        "# Relaunch\n## Design\n\u{2610} Hero cafe\n## Copy\n\u{2610} Menu\n# Later\n\u{2610} Cafe again\n",
    ));
    let (shown, matches) = filter(&all, &query("cafe"));
    assert_eq!(matches, 2);
    assert_eq!(
        lines(&json!({ "items": shown })),
        [(1, false), (2, false), (3, true), (6, false), (7, true)]
    );
    let (shown, _) = filter(&all, &query("copy"));
    assert_eq!(lines(&json!({ "items": shown })), [(1, false), (4, true)]);
    assert_eq!(filter(&all, &query("nothing")), (Vec::new(), 0));
}

#[test]
fn list_with_a_file_and_a_query_answers_the_matches_with_their_context() {
    let mut host = host();
    let page = list(
        &mut host,
        &json!({ "file": "website.todo", "query": "icons" }),
    )
    .expect("page");
    // Website: > Design: > the task and the note below it, both about icons.
    assert_eq!(lines(&page), [(1, false), (5, false), (6, true), (7, true)]);
    assert_eq!(page["matches"], 2);
    // The counts stay the file's.
    assert_eq!(
        page["counts"]["open"],
        list(&mut host, &json!({ "file": "website.todo" })).expect("all")["counts"]["open"]
    );
    let done = list(
        &mut host,
        &json!({ "file": "website.todo", "query": "@done" }),
    )
    .expect("done");
    // The archive's own project holds its match; no heading or project above it does.
    assert_eq!(
        lines(&done),
        [
            (1, false),
            (3, true),
            (5, false),
            (10, true),
            (19, false),
            (20, true)
        ]
    );
    let tagged = list(
        &mut host,
        &json!({ "file": "website.todo", "query": "ALEX" }),
    )
    .expect("tag");
    assert_eq!(lines(&tagged), [(1, false), (5, false), (14, true)]);
    let none = list(
        &mut host,
        &json!({ "file": "website.todo", "query": "zebra" }),
    )
    .expect("none");
    assert_eq!(none["items"], json!([]));
    assert_eq!(none["matches"], 0);
}

#[test]
fn list_with_a_query_finds_without_case_and_accents() {
    let mut host = host();
    let page = list(
        &mut host,
        &json!({ "file": "cafe.todo", "query": "CREME brulee" }),
    )
    .expect("page");
    assert_eq!(lines(&page), [(1, false), (2, true)]);
    let project = list(&mut host, &json!({ "file": "cafe.todo", "query": "menu" })).expect("menu");
    assert_eq!(lines(&project), [(1, true)]);
}

#[test]
fn list_without_a_file_names_the_files_with_matches_and_how_many() {
    let mut host = host();
    let answer = list(&mut host, &json!({ "query": "@high" })).expect("heads");
    let files = answer["files"].as_array().expect("files");
    let found: Vec<(&str, u64)> = files
        .iter()
        .map(|file| {
            (
                file["path"].as_str().expect("path"),
                file["matches"].as_u64().expect("matches"),
            )
        })
        .collect();
    assert_eq!(found, [("cafe.todo", 1), ("website.todo", 1)]);
    assert!(files.iter().all(|file| file.get("items").is_none()));
    // A query of white space only is no search: every file.
    let every = list(&mut host, &json!({ "query": "  " })).expect("every");
    assert_eq!(every["files"].as_array().expect("files").len(), 3);
}

#[test]
fn a_query_that_does_not_read_is_refused() {
    let mut host = host();
    for args in [
        json!({ "query": 7 }),
        json!({ "query": "x".repeat(MAX_QUERY_CHARS + 1) }),
        json!({ "file": "website.todo", "query": ["icons"] }),
    ] {
        let failure = list(&mut host, &args).expect_err("bad");
        assert_eq!(
            failure.code(),
            Some("plugin.todo.errors.bad_args"),
            "{args}"
        );
    }
    assert!(list(&mut host, &json!({ "query": "x".repeat(MAX_QUERY_CHARS) })).is_ok());
}
