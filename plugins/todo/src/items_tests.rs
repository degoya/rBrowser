use serde_json::{Value, json};

/// A small file's one page: all its items.
fn entry(path: &str, text: &str) -> Value {
    crate::page::file_page(path, text, 0)
}

const WEBSITE: &str = include_str!("../tests/fixtures/website.todo");

#[test]
fn every_line_but_blank_ones_is_an_item_with_its_place() {
    let file = entry("website.todo", WEBSITE);
    let items = file["items"].as_array().expect("items");
    assert_eq!(items.len(), 18);
    assert_eq!(file["path"], "website.todo");
    assert_eq!(file["rev"].as_str().map(str::len), Some(16));
    assert_eq!(
        items[0],
        json!({ "line": 1, "depth": 0, "kind": "project", "text": "Website", "raw": "Website:",
                "tags": [], "project": "", "archived": false })
    );
    assert_eq!(
        items[1],
        json!({ "line": 2, "depth": 1, "kind": "task", "state": "open",
                "symbol": "\u{2610}", "text": "Write the *landing* page",
                "raw": "Write the *landing* page @high @est(2h)",
                "tags": [{ "name": "high", "value": null }, { "name": "est", "value": "2h" }],
                "project": "Website", "archived": false })
    );
    // A note below a task below a project below a project.
    assert_eq!(items[6]["kind"], "note");
    assert_eq!(items[6]["depth"], 3);
    assert_eq!(items[6]["project"], "Website.Design");
    // The archived task.
    let last = items.last().expect("archived");
    assert_eq!(last["archived"], true);
    assert_eq!(last["project"], "Archive");
    assert_eq!(last["line"], 20);
}

#[test]
fn the_counts_leave_the_archive_out() {
    let file = entry("website.todo", WEBSITE);
    assert_eq!(
        file["counts"],
        json!({ "open": 6, "done": 3, "cancelled": 3 })
    );
    assert_eq!(
        entry("empty.todo", "")["counts"],
        json!({ "open": 0, "done": 0, "cancelled": 0 })
    );
}

const MARKDOWN: &str = include_str!("../tests/fixtures/markdown.todo");

#[test]
fn markdown_headings_are_items_of_their_own_with_their_level() {
    let file = entry("notes.todo", MARKDOWN);
    let items = file["items"].as_array().expect("items");
    assert_eq!(items.len(), 13);
    assert_eq!(
        items[0],
        json!({ "line": 1, "depth": 0, "kind": "heading", "level": 1, "text": "Relaunch",
                "raw": "# Relaunch", "tags": [], "project": "", "archived": false })
    );
    // The closing hashes are no part of the text; the raw line keeps them for an edit.
    assert_eq!(items[4]["text"], "Header / Navigation");
    assert_eq!(items[4]["raw"], "## Header / Navigation ##");
    assert_eq!(items[4]["level"], 2);
    // A heading's tags are badges, as a task's.
    assert_eq!(items[6]["kind"], "heading");
    assert_eq!(items[6]["level"], 3);
    assert_eq!(items[6]["text"], "Mobile");
    assert_eq!(
        items[6]["tags"],
        json!([{ "name": "today", "value": null }])
    );
    // The lines below a heading are where they are, at their indent's depth.
    assert_eq!(items[2]["kind"], "task");
    assert_eq!(items[2]["depth"], 0);
    // `#tag` and seven hashes are notes, as written.
    assert_eq!(items[8]["kind"], "note");
    assert_eq!(items[8]["text"], "#hashtag is a note");
    assert_eq!(items[9]["kind"], "note");
    // A heading inside a project lies in it.
    assert_eq!(items[11]["kind"], "heading");
    assert_eq!(items[11]["depth"], 1);
    assert_eq!(items[11]["project"], "Website");
    assert_eq!(items[12]["project"], "Website");
    // Projects and the other kinds carry no level.
    assert!(items[10].get("level").is_none());
    assert_eq!(
        file["counts"],
        json!({ "open": 3, "done": 1, "cancelled": 0 })
    );
}
