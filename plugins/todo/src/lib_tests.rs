use rb_plugin_api::codes;
use rb_plugin_api::feature::Feature;
use serde_json::{Value, json};

use super::Todo;
use crate::fake_host::FakeHost;
use crate::format::revision;

const WEBSITE: &str = include_str!("../tests/fixtures/website.todo");
/// 2026-11-16 14:05:09 UTC.
const NOW: u64 = 1_794_837_909;

fn host() -> FakeHost {
    let mut host = FakeHost::with(&[
        ("website.todo", WEBSITE),
        ("docs/plan.todo", "Plan:\n  \u{2610} Write it\n"),
    ]);
    host.now = NOW;
    host
}

fn tool(host: &mut FakeHost, name: &str, args: &Value) -> Result<Value, rb_plugin_api::Failure> {
    Todo.call_tool(host, name, &args.to_string())
        .map(|text| serde_json::from_str(&text).expect("JSON"))
}

fn line(host: &FakeHost, path: &str, number: usize) -> String {
    host.text(path)
        .lines()
        .nth(number - 1)
        .expect("line")
        .to_owned()
}

#[test]
fn list_answers_every_todo_file_then_one_with_its_items() {
    let mut host = host();
    host.files
        .insert("broken.todo".to_owned(), vec![0xff, 0x00]);
    let answer = tool(&mut host, "list", &json!({})).expect("listed");
    let files = answer["files"].as_array().expect("files");
    let paths: Vec<&str> = files
        .iter()
        .map(|file| file["path"].as_str().expect("path"))
        .collect();
    assert_eq!(paths, ["broken.todo", "docs/plan.todo", "website.todo"]);
    assert_eq!(files[0]["error"], "plugin.todo.errors.encoding");
    // The heads only: path, rev and counts; the items are the one file's call.
    assert_eq!(
        files[1],
        json!({ "path": "docs/plan.todo", "rev": revision(b"Plan:\n  \xe2\x98\x90 Write it\n"),
                "counts": { "open": 1, "done": 0, "cancelled": 0 } })
    );
    assert_eq!(files[2]["rev"], revision(WEBSITE.as_bytes()));
    assert_eq!(answer["truncated"], false);
    assert!(answer.get("next").is_none());
    let one = tool(&mut host, "list", &json!({ "file": "docs/plan.todo" })).expect("one");
    assert_eq!(one["path"], "docs/plan.todo");
    assert_eq!(one["items"][1]["text"], "Write it");
    assert!(one.get("next").is_none());
    let failure = tool(&mut host, "list", &json!({ "file": "broken.todo" })).expect_err("bad");
    assert_eq!(failure.code(), Some("plugin.todo.errors.encoding"));
    // The panel's look for changes: the files it shows, no search.
    host.listed.clear();
    let named = tool(
        &mut host,
        "list",
        &json!({ "files": ["website.todo", "gone.todo"] }),
    )
    .expect("named");
    assert!(host.listed.is_empty());
    assert_eq!(named["files"][0]["path"], "website.todo");
    assert_eq!(named["files"][1]["error"], codes::NOT_FOUND);
    for args in [
        json!({ "files": "website.todo" }),
        json!({ "files": ["notes.txt"] }),
        json!({ "file": "a.todo", "files": ["b.todo"] }),
    ] {
        let failure = tool(&mut host, "list", &args).expect_err("bad");
        assert_eq!(
            failure.code(),
            Some("plugin.todo.errors.bad_args"),
            "{args}"
        );
    }
}

#[test]
fn toggle_checks_a_task_in_local_time_and_tells_the_panel() {
    let mut host = host();
    let answer = tool(
        &mut host,
        "toggle",
        &json!({ "file": "website.todo", "line": 2, "utcOffset": 60,
                 "rev": revision(WEBSITE.as_bytes()) }),
    )
    .expect("toggled");
    assert_eq!(
        line(&host, "website.todo", 2),
        "  \u{2714} Write the *landing* page @high @est(2h) @done(26-11-16 15:05)"
    );
    assert_eq!(answer["state"], "done");
    assert_eq!(answer["file"]["items"][1]["state"], "done");
    assert_eq!(
        answer["$post"],
        json!([{ "target": "panel", "message": { "changed": "website.todo" } }])
    );
    // Again: open, the time tag gone; an explicit state: cancelled.
    tool(
        &mut host,
        "toggle",
        &json!({ "file": "website.todo", "line": 2 }),
    )
    .expect("again");
    assert_eq!(
        line(&host, "website.todo", 2),
        "  \u{2610} Write the *landing* page @high @est(2h)"
    );
    tool(
        &mut host,
        "toggle",
        &json!({ "file": "website.todo", "line": 2, "state": "cancelled" }),
    )
    .expect("cancelled");
    assert!(line(&host, "website.todo", 2).starts_with("  \u{2718} "));
}

#[test]
fn a_change_against_an_older_version_is_refused_and_nothing_written() {
    let mut host = host();
    let failure = tool(
        &mut host,
        "toggle",
        &json!({ "file": "website.todo", "line": 2, "rev": "0000000000000000" }),
    )
    .expect_err("stale");
    assert_eq!(failure.code(), Some("plugin.todo.errors.changed"));
    assert_eq!(failure.param("path"), Some("website.todo"));
    assert_eq!(host.text("website.todo"), WEBSITE);
    assert!(host.written.is_empty());
}

#[test]
fn an_untrusted_workspace_lists_but_never_writes() {
    let mut host = host();
    host.trusted = false;
    tool(&mut host, "list", &json!({})).expect("listed");
    let failure = tool(
        &mut host,
        "toggle",
        &json!({ "file": "website.todo", "line": 2 }),
    )
    .expect_err("untrusted");
    assert_eq!(failure.code(), Some("plugin.todo.errors.untrusted"));
    assert_eq!(host.text("website.todo"), WEBSITE);
}

#[test]
fn toggle_refuses_what_is_no_task() {
    let mut host = host();
    for (line, code) in [
        (1, "plugin.todo.errors.not_a_task"),
        (99, "plugin.todo.errors.no_line"),
    ] {
        let failure = tool(
            &mut host,
            "toggle",
            &json!({ "file": "website.todo", "line": line }),
        )
        .expect_err("refused");
        assert_eq!(failure.code(), Some(code), "{line}");
    }
    for args in [
        json!({ "line": 2 }),
        json!({ "file": "website.todo" }),
        json!({ "file": "website.todo", "line": 0 }),
        json!({ "file": "website.todo", "line": 2, "state": "half" }),
        json!({ "file": "website.todo", "line": 2, "utcOffset": 5000 }),
        json!({ "file": "notes.txt", "line": 2 }),
    ] {
        let failure = tool(&mut host, "toggle", &args).expect_err("bad");
        assert_eq!(
            failure.code(),
            Some("plugin.todo.errors.bad_args"),
            "{args}"
        );
    }
}

#[test]
fn add_finds_its_file_and_project() {
    let mut host = FakeHost::with(&[]);
    let answer = tool(&mut host, "add", &json!({ "text": "First" })).expect("made");
    assert_eq!(host.text("TODO.todo"), "\u{2610} First\n");
    assert_eq!(answer["line"], 1);
    tool(
        &mut host,
        "add",
        &json!({ "text": "Checkout", "project": "Shop" }),
    )
    .expect("project made");
    assert_eq!(
        host.text("TODO.todo"),
        "\u{2610} First\n\nShop:\n  \u{2610} Checkout\n"
    );
    tool(&mut host, "add", &json!({ "text": "Cart", "under": 3 })).expect("under");
    assert_eq!(line(&host, "TODO.todo", 5), "  \u{2610} Cart");
    let failure = tool(
        &mut host,
        "add",
        &json!({ "text": "x", "project": "Shop.Missing" }),
    )
    .expect_err("no such project");
    assert_eq!(failure.code(), Some("plugin.todo.errors.no_project"));

    let mut several = host_with_two();
    let failure = tool(&mut several, "add", &json!({ "text": "x" })).expect_err("which file");
    assert_eq!(failure.code(), Some("plugin.todo.errors.several_files"));
    tool(
        &mut several,
        "add",
        &json!({ "file": "docs/plan.todo", "text": "Read it", "project": "Plan" }),
    )
    .expect("added");
    assert_eq!(
        several.text("docs/plan.todo"),
        "Plan:\n  \u{2610} Write it\n  \u{2610} Read it\n"
    );
}

fn host_with_two() -> FakeHost {
    host()
}

#[test]
fn add_without_a_file_never_guesses_one() {
    // The settings leave the root's TODO.todo out: making or writing it would put the todo where
    // the panel does not show it (code review RB-110-13).
    let mut narrowed = FakeHost::with(&[("TODO.todo", "\u{2610} hidden\n")]);
    narrowed
        .settings
        .insert("files".to_owned(), r#"["docs/*.todo"]"#.to_owned());
    let failure = tool(&mut narrowed, "add", &json!({ "text": "x" })).expect_err("no file");
    assert_eq!(failure.code(), Some("plugin.todo.errors.no_file"));
    assert!(narrowed.written.is_empty());

    // A search a bound stopped may have missed the file meant: no TODO.todo is made then.
    let many: Vec<(String, String)> = (0..=crate::files::MAX_FILES)
        .map(|n| (format!("d{n:03}/x.todo"), String::new()))
        .collect();
    let refs: Vec<(&str, &str)> = many
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let mut crowded = FakeHost::with(&refs);
    let failure = tool(&mut crowded, "add", &json!({ "text": "x" })).expect_err("truncated");
    assert_eq!(failure.code(), Some("plugin.todo.errors.search_incomplete"));
    assert!(crowded.written.is_empty());
}

#[test]
fn edit_changes_deletes_and_moves_items() {
    let mut host = host();
    let path = "docs/plan.todo";
    tool(
        &mut host,
        "add",
        &json!({ "file": path, "text": "Second", "under": 1 }),
    )
    .expect("added");
    tool(
        &mut host,
        "edit",
        &json!({ "file": path, "line": 2, "action": "text", "text": "Write it all" }),
    )
    .expect("text");
    let moved = tool(
        &mut host,
        "edit",
        &json!({ "file": path, "line": 3, "action": "up" }),
    )
    .expect("moved");
    assert_eq!(moved["line"], 2);
    assert_eq!(
        host.text(path),
        "Plan:\n  \u{2610} Second\n  \u{2610} Write it all\n"
    );
    tool(
        &mut host,
        "edit",
        &json!({ "file": path, "line": 2, "action": "delete" }),
    )
    .expect("deleted");
    assert_eq!(host.text(path), "Plan:\n  \u{2610} Write it all\n");
    let failure = tool(
        &mut host,
        "edit",
        &json!({ "file": path, "line": 2, "action": "down" }),
    )
    .expect_err("last");
    assert_eq!(failure.code(), Some("plugin.todo.errors.no_sibling"));
    for args in [
        json!({ "file": path, "line": 2, "action": "text" }),
        json!({ "file": path, "line": 2, "action": "delete", "text": "x" }),
        json!({ "file": path, "line": 2, "action": "rename" }),
    ] {
        let failure = tool(&mut host, "edit", &args).expect_err("bad");
        assert_eq!(
            failure.code(),
            Some("plugin.todo.errors.bad_args"),
            "{args}"
        );
    }
}

#[test]
fn archive_moves_the_finished_tasks_of_every_file() {
    let mut host = host();
    let answer = tool(&mut host, "archive", &json!({})).expect("archived");
    assert_eq!(answer["archived"], 6);
    assert_eq!(answer["files"].as_array().map(Vec::len), Some(1));
    assert_eq!(host.written, ["website.todo"]);
    assert!(
        host.text("website.todo")
            .ends_with("  x Lower-case cancelled @project(Website.Design)\n")
    );
    let failure = tool(&mut host, "archive", &json!({ "rev": "x" })).expect_err("rev alone");
    assert_eq!(failure.code(), Some("plugin.todo.errors.bad_args"));
    // The command does the same.
    let again = Todo
        .run_command(&mut host, "archive", "{}")
        .expect("command");
    let again: Value = serde_json::from_str(&again).expect("JSON");
    assert_eq!(again["archived"], 0);
}

#[test]
fn the_commands_open_the_panel() {
    let mut host = host();
    let panel: Value =
        serde_json::from_str(&Todo.run_command(&mut host, "panel", "").expect("panel"))
            .expect("JSON");
    assert_eq!(panel, json!({ "$open": { "panel": "todo" } }));
    let new: Value = serde_json::from_str(&Todo.run_command(&mut host, "new", "{}").expect("new"))
        .expect("JSON");
    assert_eq!(
        new,
        json!({ "$open": { "panel": "todo" },
                "$post": [{ "target": "panel", "message": { "new": true } }] })
    );
    let unknown = Todo
        .run_command(&mut host, "other", "{}")
        .expect_err("unknown");
    assert_eq!(unknown.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    let bad = Todo
        .call_tool(&mut host, "list", "[1]")
        .expect_err("no object");
    assert_eq!(bad.code(), Some("plugin.todo.errors.bad_args"));
}

#[test]
fn without_a_project_list_says_so() {
    let mut host = host();
    host.no_project = true;
    let failure = tool(&mut host, "list", &json!({})).expect_err("no project");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
}

#[test]
fn add_plans_a_heading_and_the_tasks_below_it() {
    // An agent plans over MCP: a heading, then tasks `under` its line.
    let mut host = FakeHost::with(&[]);
    let answer = tool(&mut host, "add", &json!({ "text": "## Sprint" })).expect("heading");
    assert_eq!(answer["line"], 1);
    assert_eq!(host.text("TODO.todo"), "## Sprint\n");
    let heading = &answer["file"]["items"][0];
    assert_eq!(heading["kind"], "heading");
    assert_eq!(heading["level"], 2);
    tool(&mut host, "add", &json!({ "text": "Plan it", "under": 1 })).expect("task");
    tool(&mut host, "add", &json!({ "text": "Ship it", "under": 1 })).expect("task");
    assert_eq!(
        host.text("TODO.todo"),
        "## Sprint\n\u{2610} Plan it\n\u{2610} Ship it\n"
    );
    let answer = tool(&mut host, "add", &json!({ "text": "# Next" })).expect("heading");
    assert_eq!(answer["line"], 5);
    assert_eq!(line(&host, "TODO.todo", 5), "# Next");
}
