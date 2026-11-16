use rb_plugin_api::feature::Feature;
use serde_json::{Value, json};

use super::{CUT_BYTES, MAX_ITEM_BYTES, PAGE_BYTES, prefix};
use crate::Todo;
use crate::fake_host::FakeHost;
use crate::format::{Document, revision};
use crate::items::items;

/// What a panel message may carry: the host's and the panels window's `MAX_MESSAGE_BYTES`.
const MESSAGE_BYTES: usize = 64 * 1024;

/// The tool's answer, held to the message limit as the host measures it.
fn call(host: &mut FakeHost, name: &str, args: &Value) -> Result<Value, rb_plugin_api::Failure> {
    Todo.call_tool(host, name, &args.to_string()).map(|text| {
        assert!(
            text.len() < MESSAGE_BYTES,
            "{name} answered {} bytes",
            text.len()
        );
        serde_json::from_str(&text).expect("JSON")
    })
}

/// A todo file of `projects` projects with `tasks` tasks each, a note now and then.
fn long_file(projects: usize, tasks: usize) -> String {
    let mut text = String::new();
    for project in 0..projects {
        text.push_str(&format!("Project {project}:\n"));
        for task in 0..tasks {
            text.push_str(&format!(
                "  \u{2610} Task {task} of project {project}, written out a little @high @est(2h)\n"
            ));
            if task % 10 == 0 {
                text.push_str("    A note below it, with \"quotes\" and a \\ backslash.\n");
            }
        }
        text.push('\n');
    }
    text
}

/// Every page of `path` from item 0, `next` after `next` with the first page's rev.
fn every_page(host: &mut FakeHost, path: &str) -> (Vec<Value>, usize) {
    let first = call(host, "list", &json!({ "file": path })).expect("first page");
    let rev = first["rev"].as_str().expect("rev").to_owned();
    let mut items: Vec<Value> = first["items"].as_array().expect("items").clone();
    let mut pages = 1;
    let mut next = first["next"].as_u64();
    while let Some(from) = next {
        let page = call(
            host,
            "list",
            &json!({ "file": path, "from": from, "rev": rev }),
        )
        .expect("next page");
        assert_eq!(page["rev"], rev.as_str());
        assert_eq!(page["counts"], first["counts"]);
        let more = page["items"].as_array().expect("items");
        assert!(!more.is_empty(), "a page moves on");
        items.extend(more.iter().cloned());
        let after = page["next"].as_u64();
        assert!(after.is_none_or(|after| after > from), "next moves on");
        next = after;
        pages += 1;
    }
    (items, pages)
}

#[test]
fn a_file_of_thousands_of_items_lists_whole_through_pages_that_fit() {
    let text = long_file(30, 100);
    let whole = items(&Document::parse(&text));
    assert!(whole.len() > 3000);
    let mut host = FakeHost::with(&[("big.todo", &text)]);
    let (listed, pages) = every_page(&mut host, "big.todo");
    assert!(pages > 2, "{pages} pages");
    assert_eq!(listed, whole);
    // Each page within its budget, the envelope's room left.
    let first = call(&mut host, "list", &json!({ "file": "big.todo" })).expect("page");
    assert!(first.to_string().len() <= PAGE_BYTES);
    assert_eq!(first["path"], "big.todo");
    assert_eq!(first["rev"], revision(text.as_bytes()));
}

#[test]
fn a_giant_line_is_cut_and_the_pages_still_move_on() {
    let giant_task = "word ".repeat(40_000);
    let giant_project = "P".repeat(30_000);
    let tags = (0..3_000)
        .map(|n| format!("@t{n}"))
        .collect::<Vec<_>>()
        .join(" ");
    let text = format!(
        "{giant_project}:\n  \u{2610} {giant_task}\n  \u{2610} small\n  \u{2610} {tags}\n\u{2610} after\n"
    );
    let mut host = FakeHost::with(&[("giant.todo", &text)]);
    let (listed, _) = every_page(&mut host, "giant.todo");
    assert_eq!(listed.len(), 5);
    for item in &listed {
        assert!(
            item.to_string().len() <= MAX_ITEM_BYTES + 512,
            "{}",
            item["line"]
        );
    }
    assert_eq!(listed[0]["cut"], true);
    assert_eq!(listed[1]["cut"], true);
    assert!(
        listed[1]["text"]
            .as_str()
            .expect("text")
            .starts_with("word word")
    );
    assert!(listed[1]["raw"].as_str().expect("raw").len() <= CUT_BYTES);
    // A child of the giant project: its project path cut, the item itself small.
    assert_eq!(listed[2]["text"], "small");
    assert_eq!(listed[2]["cut"], true);
    assert_eq!(listed[3]["cut"], true);
    assert!(!listed[3]["tags"].as_array().expect("tags").is_empty());
    assert_eq!(listed[4]["text"], "after");
    assert!(listed[4].get("cut").is_none());
}

#[test]
fn a_file_that_changes_between_pages_says_so() {
    let text = long_file(10, 100);
    let mut host = FakeHost::with(&[("big.todo", &text)]);
    let first = call(&mut host, "list", &json!({ "file": "big.todo" })).expect("page");
    let next = first["next"].as_u64().expect("more");
    host.files.insert(
        "big.todo".to_owned(),
        format!("\u{2610} new\n{text}").into_bytes(),
    );
    let failure = call(
        &mut host,
        "list",
        &json!({ "file": "big.todo", "from": next, "rev": first["rev"] }),
    )
    .expect_err("changed");
    assert_eq!(failure.code(), Some("plugin.todo.errors.changed"));
    assert_eq!(failure.param("path"), Some("big.todo"));
    // From the start, without the old rev, it reads again.
    call(&mut host, "list", &json!({ "file": "big.todo" })).expect("again");
}

#[test]
fn the_file_list_names_heads_only_and_pages_too() {
    // 200 files with long paths: their heads alone are over one page.
    let folder = "a-folder-with-a-rather-long-name-".repeat(20);
    let many: Vec<(String, String)> = (0..crate::files::MAX_FILES)
        .map(|n| (format!("{folder}/f{n:03}.todo"), long_file(1, 50)))
        .collect();
    let refs: Vec<(&str, &str)> = many
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let mut host = FakeHost::with(&refs);
    let mut seen = Vec::new();
    let mut from = None;
    loop {
        let args = from.map_or_else(|| json!({}), |from| json!({ "from": from }));
        let page = call(&mut host, "list", &args).expect("heads");
        assert_eq!(page["truncated"], false);
        for head in page["files"].as_array().expect("files") {
            assert!(head.get("items").is_none(), "a head carries no items");
            assert_eq!(head["counts"]["open"], 50);
            assert_eq!(head["rev"].as_str().map(str::len), Some(16));
            seen.push(head["path"].as_str().expect("path").to_owned());
        }
        match page["next"].as_u64() {
            Some(next) => from = Some(next),
            None => break,
        }
    }
    let wanted: Vec<String> = many.iter().map(|(path, _)| path.clone()).collect();
    assert_eq!(seen, wanted);
    assert!(from.is_some(), "more than one page");
    let failure = call(&mut host, "list", &json!({ "rev": "x" })).expect_err("rev needs file");
    assert_eq!(failure.code(), Some("plugin.todo.errors.bad_args"));
    let failure = call(&mut host, "list", &json!({ "from": -1 })).expect_err("bad from");
    assert_eq!(failure.code(), Some("plugin.todo.errors.bad_args"));
}

#[test]
fn a_change_answers_the_first_page_and_archive_heads_only() {
    let text = long_file(30, 100);
    let mut host = FakeHost::with(&[("big.todo", &text)]);
    let answer = call(
        &mut host,
        "toggle",
        &json!({ "file": "big.todo", "line": 2, "rev": revision(text.as_bytes()) }),
    )
    .expect("toggled");
    assert_eq!(answer["file"]["items"][1]["state"], "done");
    assert!(answer["file"]["next"].as_u64().is_some());
    let edited = call(
        &mut host,
        "edit",
        &json!({ "file": "big.todo", "line": 3, "action": "delete" }),
    )
    .expect("deleted");
    assert!(edited["file"]["next"].as_u64().is_some());
    let archived = call(&mut host, "archive", &json!({})).expect("archived");
    assert_eq!(archived["archived"], 1);
    let files = archived["files"].as_array().expect("files");
    assert_eq!(files.len(), 1);
    assert!(files[0].get("items").is_none());
    assert_eq!(files[0]["path"], "big.todo");
    assert_eq!(archived["omitted"], 0);
}

#[test]
fn a_prefix_ends_on_a_character_within_its_json_budget() {
    assert_eq!(prefix("abc", 10), "abc");
    assert_eq!(prefix("abcdef", 3), "abc");
    // Two bytes each in UTF-8: never half a character.
    assert_eq!(prefix("\u{e4}\u{e4}\u{e4}", 5), "\u{e4}\u{e4}");
    // An escaped quote takes two bytes, a control character six.
    assert_eq!(prefix("\"\"\"", 5), "\"\"");
    assert_eq!(prefix("a\u{1}b", 6), "a");
}

#[test]
fn markdown_sections_page_like_projects_and_a_giant_heading_is_cut_whole() {
    let mut text = String::new();
    for section in 0..40 {
        text.push_str(&format!("## Section {section} ##\n"));
        for task in 0..80 {
            text.push_str(&format!(
                "\u{2610} Task {task} of section {section}, written out a little @high\n"
            ));
        }
        text.push('\n');
    }
    let giant = "H".repeat(30_000);
    text.push_str(&format!("# {giant}\n\u{2610} after\n"));
    let whole = items(&Document::parse(&text));
    let mut host = FakeHost::with(&[("notes.todo", &text)]);
    let (listed, pages) = every_page(&mut host, "notes.todo");
    assert!(pages > 2, "{pages} pages");
    assert_eq!(listed.len(), whole.len());
    assert_eq!(listed[..listed.len() - 2], whole[..whole.len() - 2]);
    assert_eq!(listed[0]["kind"], "heading");
    assert_eq!(listed[0]["text"], "Section 0");
    let cut = &listed[listed.len() - 2];
    assert_eq!(cut["kind"], "heading");
    assert_eq!(cut["level"], 1);
    assert_eq!(cut["cut"], true);
    assert!(cut["text"].as_str().expect("text").len() <= CUT_BYTES);
    assert_eq!(listed[listed.len() - 1]["text"], "after");
}
