use super::{
    Document, Kind, State, classify, heading_text, indent_width, project_name, remove_tags,
    revision, strip_tags, tags,
};

const WEBSITE: &str = include_str!("../tests/fixtures/website.todo");
const SHOP: &[u8] = include_bytes!("../tests/fixtures/shop-crlf-bom.todo");
const MARKDOWN: &str = include_str!("../tests/fixtures/markdown.todo");

fn shop() -> &'static str {
    std::str::from_utf8(SHOP).expect("UTF-8")
}

#[test]
fn a_file_read_and_written_unchanged_is_byte_identical() {
    for text in [
        WEBSITE,
        shop(),
        MARKDOWN,
        "",
        "a",
        "a\n",
        "\n\n",
        "a\r\nb",
        "\u{feff}",
        "\u{feff}x\n",
        "mixed\r\nendings\nhere\r\n",
        "  \t \n\t\n",
        "\u{2610} only a task without a newline",
    ] {
        assert_eq!(Document::parse(text).render(), text, "{text:?}");
    }
}

#[test]
fn line_endings_and_the_byte_order_mark_are_kept_per_line() {
    let document = Document::parse(shop());
    assert!(document.bom);
    assert_eq!(document.lines.len(), 5);
    assert_eq!(document.lines[0].text, "Shop:");
    assert_eq!(document.lines[0].ending, "\r\n");
    assert_eq!(document.lines[4].ending, "");
    assert_eq!(document.ending(), "\r\n");
    assert_eq!(Document::parse("a\nb\n").ending(), "\n");
    assert_eq!(Document::parse("").ending(), "\n");
}

#[test]
fn projects_tasks_notes_and_blank_lines_are_told_apart() {
    let document = Document::parse(WEBSITE);
    let kinds: Vec<Kind> = document
        .lines
        .iter()
        .map(|line| classify(&line.text).kind)
        .collect();
    use Kind::{Blank, Note, Project, Task};
    use State::{Cancelled, Done, Open};
    assert_eq!(
        kinds,
        [
            Project,
            Task(Open),
            Task(Done),
            Task(Cancelled),
            Project,
            Task(Open),
            Note,
            Task(Open),
            Task(Open),
            Task(Done),
            Task(Cancelled),
            Task(Done),
            Task(Cancelled),
            Task(Open),
            Blank,
            Note,
            Task(Open),
            Blank,
            Project,
            Task(Done),
        ]
    );
    assert_eq!(document.width(4), Some(2));
    assert_eq!(document.width(6), Some(6));
    assert_eq!(document.width(14), None);
}

#[test]
fn every_symbol_todo_plus_reads_is_a_task() {
    for (symbol, state) in [
        ("\u{2610}", State::Open),
        ("\u{274d}", State::Open),
        ("\u{2751}", State::Open),
        ("\u{25a0}", State::Open),
        ("\u{2b1c}", State::Open),
        ("\u{25a1}", State::Open),
        ("\u{25aa}", State::Open),
        ("\u{25ab}", State::Open),
        ("\u{2013}", State::Open),
        ("\u{2014}", State::Open),
        ("\u{2261}", State::Open),
        ("\u{2192}", State::Open),
        ("\u{203a}", State::Open),
        ("-", State::Open),
        ("[]", State::Open),
        ("[ ]", State::Open),
        ("\u{2714}", State::Done),
        ("\u{2713}", State::Done),
        ("\u{2611}", State::Done),
        ("+", State::Done),
        ("[x]", State::Done),
        ("[X]", State::Done),
        ("[+]", State::Done),
        ("\u{2718}", State::Cancelled),
        ("x", State::Cancelled),
        ("X", State::Cancelled),
        ("[-]", State::Cancelled),
    ] {
        let line = format!("  {symbol}\tthe task @high");
        let parsed = classify(&line);
        assert_eq!(parsed.kind, Kind::Task(state), "{symbol}");
        assert_eq!(parsed.symbol, symbol);
        assert_eq!(parsed.indent, "  ");
        assert_eq!(parsed.gap, "\t");
        assert_eq!(parsed.rest, "the task @high");
    }
    // A symbol needs white space after it; a word that starts like one is a note.
    for note in ["xylophone", "-dash", "[x]done", "+1", "\u{2610}"] {
        assert_eq!(classify(note).kind, Kind::Note, "{note}");
    }
}

#[test]
fn a_project_ends_in_a_colon_with_only_tags_after_it() {
    assert_eq!(project_name("Website:"), Some("Website"));
    assert_eq!(
        project_name("Design: @critical @owner(alex)"),
        Some("Design")
    );
    assert_eq!(project_name("Archive:  "), Some("Archive"));
    assert_eq!(project_name("Note: see below"), None);
    assert_eq!(project_name(":"), None);
    assert_eq!(project_name("Website: and more @x"), None);
    assert_eq!(classify("\u{2610} Task:").kind, Kind::Task(State::Open));
}

#[test]
fn tags_are_read_with_their_values_and_places() {
    let text = "Write *it* @high @est(2h) mail a@b.example @done(26-11-14 09:30)";
    let found = tags(text);
    let names: Vec<(&str, Option<&str>)> = found
        .iter()
        .map(|tag| (tag.name.as_str(), tag.value.as_deref()))
        .collect();
    assert_eq!(
        names,
        [
            ("high", None),
            ("est", Some("2h")),
            ("done", Some("26-11-14 09:30"))
        ]
    );
    assert_eq!(&text[found[1].start..found[1].end], "@est(2h)");
    assert_eq!(tags("@today first").first().map(|tag| tag.start), Some(0));
    assert!(tags("@ alone").is_empty());
    assert_eq!(
        tags("@open(no close").first().map(|tag| tag.name.as_str()),
        Some("open")
    );
}

#[test]
fn texts_lose_their_tags_for_the_list_and_on_a_change() {
    assert_eq!(
        strip_tags("Write *it*  @high @est(2h) now"),
        "Write *it* now"
    );
    assert_eq!(
        remove_tags("Pick @high the colours @done(26-11-14 09:30)", &["done"]),
        "Pick @high the colours"
    );
    assert_eq!(
        remove_tags("A @done(x) b @cancelled(y)", &["done", "cancelled"]),
        "A b"
    );
    assert_eq!(
        remove_tags("Untouched  text ", &["done"]),
        "Untouched  text "
    );
}

#[test]
fn the_revision_follows_the_bytes() {
    assert_eq!(revision(b"a"), revision(b"a"));
    assert_ne!(revision(b"a"), revision(b"a\n"));
    assert_eq!(revision(b"").len(), 16);
}

#[test]
fn a_tab_is_four_columns() {
    assert_eq!(indent_width("\t"), 4);
    assert_eq!(indent_width("  \t"), 6);
    assert_eq!(indent_width(""), 0);
}

#[test]
fn markdown_headings_are_headings_of_their_level_without_their_hashes() {
    for (line, level, text) in [
        ("# Relaunch", 1, "Relaunch"),
        ("## Ratios / Images", 2, "Ratios / Images"),
        ("###### Six", 6, "Six"),
        ("   ### Three spaces in", 3, "Three spaces in"),
        ("  ## Two spaces in", 2, "Two spaces in"),
        ("## Closed ##", 2, "Closed"),
        ("## Closed #####   ", 2, "Closed"),
        ("#\tAfter a tab", 1, "After a tab"),
        ("## Not# closed#", 2, "Not# closed#"),
        ("## C# and F#", 2, "C# and F#"),
        ("# Website:", 1, "Website:"),
        ("### Mobile @today", 3, "Mobile @today"),
    ] {
        let parsed = classify(line);
        assert_eq!(parsed.kind, Kind::Heading(level), "{line:?}");
        assert_eq!(heading_text(parsed.rest), text, "{line:?}");
    }
    let parsed = classify("  ## Closed ##");
    assert_eq!(
        (parsed.indent, parsed.symbol, parsed.gap, parsed.rest),
        ("  ", "##", " ", "Closed ##")
    );
}

#[test]
fn a_hash_without_white_space_or_text_or_too_deep_is_no_heading() {
    for note in [
        "#tag",
        "#hashtag and more",
        "#####",
        "## ",
        "## ##",
        "#   #  ",
        "####### Seven",
        "    # Four spaces in",
        "\t# A tab in",
    ] {
        assert_eq!(classify(note).kind, Kind::Note, "{note:?}");
    }
    // A task whose text starts with a hash stays a task.
    assert_eq!(classify("- # not a heading").kind, Kind::Task(State::Open));
    assert_eq!(heading_text("##"), "");
    assert_eq!(heading_text("Text \\#"), "Text \\#");
}
