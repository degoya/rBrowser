use super::{
    add, add_project, archive, block_end, clean_text, delete, find_project, indent_step, move_item,
    set_state, set_text, toggled,
};
use crate::format::{Document, State};

const WEBSITE: &str = include_str!("../tests/fixtures/website.todo");
const SHOP: &[u8] = include_bytes!("../tests/fixtures/shop-crlf-bom.todo");
const STAMP: &str = "26-11-16 14:05";

fn edited(text: &str, change: impl FnOnce(&mut Document)) -> String {
    let mut document = Document::parse(text);
    change(&mut document);
    document.render()
}

/// `text` with line `index` replaced by `line`.
fn with_line(text: &str, index: usize, line: &str) -> String {
    let mut lines: Vec<&str> = text.split('\n').collect();
    lines[index] = line;
    lines.join("\n")
}

#[test]
fn checking_a_task_writes_done_with_the_time_as_todo_plus_does() {
    let text = edited(WEBSITE, |document| {
        set_state(document, 1, State::Done, STAMP).expect("task");
    });
    assert_eq!(
        text,
        with_line(
            WEBSITE,
            1,
            "  \u{2714} Write the *landing* page @high @est(2h) @done(26-11-16 14:05)"
        )
    );
}

#[test]
fn unchecking_drops_the_time_and_cancelling_writes_cancelled() {
    let reopened = edited(WEBSITE, |document| {
        set_state(document, 2, State::Open, STAMP).expect("task");
    });
    assert_eq!(
        reopened,
        with_line(WEBSITE, 2, "  \u{2610} Pick the colours")
    );
    let cancelled = edited(WEBSITE, |document| {
        set_state(document, 2, State::Cancelled, STAMP).expect("task");
    });
    assert_eq!(
        cancelled,
        with_line(
            WEBSITE,
            2,
            "  \u{2718} Pick the colours @cancelled(26-11-16 14:05)"
        )
    );
    // Every other tag stays where it is.
    let started = edited(WEBSITE, |document| {
        set_state(document, 13, State::Done, STAMP).expect("task");
    });
    assert_eq!(
        started,
        with_line(
            WEBSITE,
            13,
            "    \u{2714} Tagged @started(26-11-16 08:00) @owner(alex) @done(26-11-16 14:05)"
        )
    );
}

#[test]
fn a_bracket_task_keeps_its_brackets_and_other_symbols_become_todo_plus_ones() {
    let text = edited(WEBSITE, |document| {
        set_state(document, 8, State::Done, STAMP).expect("[ ]");
        set_state(document, 9, State::Open, STAMP).expect("[x]");
        set_state(document, 7, State::Done, STAMP).expect("-");
        set_state(document, 11, State::Open, STAMP).expect("+");
    });
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[8], "    [x] Markdown box @done(26-11-16 14:05)");
    assert_eq!(lines[9], "    [ ] Markdown done");
    assert_eq!(lines[7], "    \u{2714} Old-style box @done(26-11-16 14:05)");
    assert_eq!(lines[11], "    \u{2610} Plus done");
}

#[test]
fn a_toggle_opens_what_is_finished_and_finishes_what_is_open() {
    assert_eq!(toggled(State::Open), State::Done);
    assert_eq!(toggled(State::Done), State::Open);
    assert_eq!(toggled(State::Cancelled), State::Open);
    let mut document = Document::parse(WEBSITE);
    assert_eq!(
        set_state(&mut document, 0, State::Done, STAMP),
        Err("errors.not_a_task")
    );
    assert_eq!(
        set_state(&mut document, 99, State::Done, STAMP),
        Err("errors.no_line")
    );
    // The same state changes nothing.
    let unchanged = edited(WEBSITE, |document| {
        set_state(document, 2, State::Done, STAMP).expect("task");
    });
    assert_eq!(unchanged, WEBSITE);
}

#[test]
fn a_text_change_keeps_the_symbol_the_indent_and_a_projects_tags() {
    let text = edited(WEBSITE, |document| {
        set_text(document, 5, "Icons for the footer @today").expect("task");
        set_text(document, 4, "Look").expect("project");
        set_text(document, 15, "Another comment.").expect("note");
    });
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[5], "    \u{2610} Icons for the footer @today");
    assert_eq!(lines[4], "  Look: @critical");
    assert_eq!(lines[15], "Another comment.");
    let mut document = Document::parse(WEBSITE);
    // A text that would turn the line into another kind, a blank one or a line break: refused.
    assert_eq!(
        set_text(&mut document, 15, "- now a task"),
        Err("errors.text")
    );
    assert_eq!(
        set_text(&mut document, 15, "Now a project:"),
        Err("errors.text")
    );
    assert_eq!(set_text(&mut document, 5, "two\nlines"), Err("errors.text"));
    assert_eq!(set_text(&mut document, 14, "x"), Err("errors.blank_line"));
    assert_eq!(set_text(&mut document, 5, "   "), Err("errors.text"));
}

#[test]
fn deleting_an_item_takes_its_children_along() {
    let text = edited(WEBSITE, |document| {
        delete(document, 5).expect("task with a note");
    });
    let mut expected: Vec<&str> = WEBSITE.split('\n').collect();
    expected.drain(5..7);
    assert_eq!(text, expected.join("\n"));
    let project = edited(WEBSITE, |document| {
        delete(document, 4).expect("project");
    });
    let mut expected: Vec<&str> = WEBSITE.split('\n').collect();
    expected.drain(4..14);
    assert_eq!(project, expected.join("\n"));
}

#[test]
fn a_file_without_a_final_line_break_keeps_that_style() {
    let text = "A:\n  \u{2610} one\n  \u{2610} two";
    assert_eq!(
        edited(text, |document| {
            delete(document, 2).expect("last");
        }),
        "A:\n  \u{2610} one"
    );
    assert_eq!(
        edited(text, |document| {
            add(document, "three", Some(0)).expect("added");
        }),
        "A:\n  \u{2610} one\n  \u{2610} two\n  \u{2610} three"
    );
    assert_eq!(
        edited(text, |document| {
            move_item(document, 2, true).expect("moved");
        }),
        "A:\n  \u{2610} two\n  \u{2610} one"
    );
}

#[test]
fn moving_swaps_an_item_with_its_sibling_and_never_leaves_its_parent() {
    let text = "P:\n  \u{2610} a\n    note of a\n  \u{2610} b\nQ:\n  \u{2610} c\n";
    let mut document = Document::parse(text);
    assert_eq!(move_item(&mut document, 3, true), Ok(1));
    assert_eq!(
        document.render(),
        "P:\n  \u{2610} b\n  \u{2610} a\n    note of a\nQ:\n  \u{2610} c\n"
    );
    assert_eq!(move_item(&mut document, 2, false), Err("errors.no_sibling"));
    assert_eq!(move_item(&mut document, 1, true), Err("errors.no_sibling"));
    assert_eq!(move_item(&mut document, 1, false), Ok(3));
    assert_eq!(document.render(), text);
    // Projects move with their blocks; the blank line between them stays.
    let projects = "A:\n  \u{2610} a\n\nB:\n  \u{2610} b\n";
    assert_eq!(
        edited(projects, |document| {
            assert_eq!(move_item(document, 3, true), Ok(0));
        }),
        "B:\n  \u{2610} b\n\nA:\n  \u{2610} a\n"
    );
}

#[test]
fn a_new_task_goes_to_the_end_of_its_project_in_the_files_indent() {
    let text = edited(WEBSITE, |document| {
        assert_eq!(add(document, "New in design", Some(4)), Ok(14));
        assert_eq!(add(document, "New on top", None), Ok(18));
    });
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[14], "    \u{2610} New in design");
    assert_eq!(lines[15], "");
    assert_eq!(lines[18], "\u{2610} New on top");
    assert_eq!(lines[19], "");
    assert_eq!(lines[20], "Archive:");
    let mut document = Document::parse(WEBSITE);
    assert_eq!(add(&mut document, "x", Some(6)), Err("errors.not_a_parent"));
    assert_eq!(add(&mut document, "", None), Err("errors.text"));
}

#[test]
fn a_tab_indented_crlf_file_stays_one() {
    let shop = std::str::from_utf8(SHOP).expect("UTF-8");
    let text = edited(shop, |document| {
        add(document, "Payment", Some(0)).expect("added");
        set_state(document, 1, State::Done, STAMP).expect("task");
    });
    assert_eq!(
        text,
        "\u{feff}Shop:\r\n\t\u{2714} Checkout @done(26-11-16 14:05)\r\n\t\u{2714} Cart @done(26-11-01 10:00)\r\n\tLegal:\r\n\t\t\u{2610} Imprint\r\n\t\u{2610} Payment"
    );
    assert_eq!(indent_step(&Document::parse(shop)), "\t");
}

#[test]
fn projects_are_found_by_their_path_and_made_when_missing() {
    let document = Document::parse(WEBSITE);
    assert_eq!(find_project(&document, "Website"), Some(0));
    assert_eq!(find_project(&document, "Website.Design"), Some(4));
    assert_eq!(find_project(&document, "Design"), None);
    assert_eq!(find_project(&document, "Archive"), Some(18));
    let text = edited("\u{2610} loose\n", |document| {
        assert_eq!(add_project(document, "Shop"), Ok(2));
        assert_eq!(add(document, "Cart", Some(2)), Ok(3));
    });
    assert_eq!(text, "\u{2610} loose\n\nShop:\n  \u{2610} Cart\n");
    let mut document = Document::parse("");
    assert_eq!(add_project(&mut document, "A.B"), Err("errors.text"));
    assert_eq!(add_project(&mut document, "Solo"), Ok(0));
    assert_eq!(document.render(), "Solo:\n");
}

#[test]
fn archiving_moves_finished_tasks_with_their_project_into_archive() {
    let text = edited(WEBSITE, |document| {
        assert_eq!(archive(document), 6);
    });
    let expected = "Website:
  \u{2610} Write the *landing* page @high @est(2h)
  Design: @critical
    \u{2610} Icons for the `nav` bar @today
      A note on the icons, _without_ a symbol.
    - Old-style box
    [ ] Markdown box
    \u{2610} Tagged @started(26-11-16 08:00) @owner(alex)

Notes for later, a comment line.
\u{2610} A task without a project

Archive:
  \u{2714} Ship the beta @done(26-10-01 12:00) @project(Website)
  \u{2714} Pick the colours @done(26-11-14 09:30) @project(Website)
  \u{2718} Use a carousel @cancelled(26-11-12 18:00) @project(Website)
  [x] Markdown done @done(26-11-15 10:00) @project(Website.Design)
  [-] Markdown cancelled @project(Website.Design)
  + Plus done @project(Website.Design)
  x Lower-case cancelled @project(Website.Design)
";
    assert_eq!(text, expected);
    // Once more, nothing is left to move.
    assert_eq!(
        edited(&text, |document| {
            assert_eq!(archive(document), 0);
        }),
        text
    );
}

#[test]
fn archiving_makes_the_archive_project_when_there_is_none() {
    let text = "A:\n  \u{2714} done @done(x)\n  \u{2610} open\n\u{2714} top";
    assert_eq!(
        edited(text, |document| {
            assert_eq!(archive(document), 2);
        }),
        "A:\n  \u{2610} open\n\nArchive:\n  \u{2714} done @done(x) @project(A)\n  \u{2714} top"
    );
}

#[test]
fn a_block_ends_before_the_next_line_as_shallow() {
    let document = Document::parse(WEBSITE);
    assert_eq!(block_end(&document, 0), 14);
    assert_eq!(block_end(&document, 5), 7);
    assert_eq!(block_end(&document, 18), 20);
    assert_eq!(clean_text("  ok  "), Ok("ok".to_owned()));
    assert_eq!(clean_text(&"x".repeat(1001)), Err("errors.text"));
}

const MARKDOWN: &str = include_str!("../tests/fixtures/markdown.todo");

#[test]
fn a_heading_is_edited_as_its_whole_line_and_stays_a_heading() {
    let text = edited(MARKDOWN, |document| {
        set_text(document, 2, "### Ratios and images ###").expect("heading");
    });
    assert_eq!(text, with_line(MARKDOWN, 2, "### Ratios and images ###"));
    let inside = edited(MARKDOWN, |document| {
        set_text(document, 14, "# Inside").expect("indented heading");
    });
    assert_eq!(inside, with_line(MARKDOWN, 14, "  # Inside"));
    let mut document = Document::parse(MARKDOWN);
    // Without its hashes it would be a note, with seven or none after them too: refused.
    for refused in ["Ratios", "####### Ratios", "##Ratios", "##"] {
        assert_eq!(
            set_text(&mut document, 2, refused),
            Err("errors.text"),
            "{refused}"
        );
    }
    assert_eq!(document.render(), MARKDOWN);
}

#[test]
fn checking_a_task_below_a_heading_changes_only_its_line() {
    let text = edited(MARKDOWN, |document| {
        set_state(document, 3, State::Done, STAMP).expect("task");
    });
    assert_eq!(
        text,
        with_line(
            MARKDOWN,
            3,
            "\u{2714} Hero image 16:9 @high @done(26-11-16 14:05)"
        )
    );
}

#[test]
fn a_heading_is_deleted_and_moved_as_its_own_line() {
    let deleted = edited(MARKDOWN, |document| {
        delete(document, 2).expect("heading");
    });
    let mut expected: Vec<&str> = MARKDOWN.split('\n').collect();
    expected.remove(2);
    assert_eq!(deleted, expected.join("\n"));
    let moved = edited("## A\n\u{2610} a\n## B\n", |document| {
        assert_eq!(move_item(document, 2, true), Ok(1));
    });
    assert_eq!(moved, "## A\n## B\n\u{2610} a\n");
}
