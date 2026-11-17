use super::insertion;
use crate::edit::add;
use crate::format::Document;

const MARKDOWN: &str = include_str!("../tests/fixtures/markdown.todo");

fn added(text: &str, under: usize) -> String {
    let mut document = Document::parse(text);
    add(&mut document, "New", Some(under)).expect("added");
    document.render()
}

#[test]
fn a_new_task_below_a_heading_ends_its_own_lines_before_the_next_heading() {
    let document = Document::parse(MARKDOWN);
    // `## Ratios / Images`: after its note, before the blank line and `## Header`.
    assert_eq!(insertion(&document, 2), (5, String::new()));
    // `# Relaunch` has no line of its own before `## Ratios`: right below it.
    assert_eq!(insertion(&document, 0), (1, String::new()));
    // `## Header` ends at its sub-heading `### Mobile`.
    assert_eq!(insertion(&document, 6), (8, String::new()));
    let text = added(MARKDOWN, 2);
    let lines: Vec<&str> = text.split('\n').collect();
    assert_eq!(lines[4], "Notes on the images.");
    assert_eq!(lines[5], "\u{2610} New");
    assert_eq!(lines[6], "");
    assert_eq!(lines[7], "## Header / Navigation ##");
}

#[test]
fn a_heading_inside_a_project_takes_its_indent_and_ends_with_the_project() {
    let document = Document::parse(MARKDOWN);
    assert_eq!(insertion(&document, 14), (16, "  ".to_owned()));
    assert!(added(MARKDOWN, 14).ends_with("  \u{2610} Footer\n  \u{2610} New\n"));
    // A deeper heading of a project in the section does not end it; the file's end does.
    assert_eq!(insertion(&document, 8), (16, String::new()));
    // A line less indented than the heading ends it.
    let nested = "Website:\n  ## Design\n  \u{2610} Icons\nShop:\n";
    assert_eq!(
        added(nested, 1),
        "Website:\n  ## Design\n  \u{2610} Icons\n  \u{2610} New\nShop:\n"
    );
}

#[test]
fn a_heading_ends_before_the_archive_and_takes_its_first_lines_indent() {
    assert_eq!(
        added("## Done\n\u{2610} x\nArchive:\n  \u{2714} y\n", 0),
        "## Done\n\u{2610} x\n\u{2610} New\nArchive:\n  \u{2714} y\n"
    );
    assert_eq!(
        added("## Indented below\n  \u{2610} x\n", 0),
        "## Indented below\n  \u{2610} x\n  \u{2610} New\n"
    );
    // A file without a final line break keeps that style.
    assert_eq!(
        added("## A\n\u{2610} x", 0),
        "## A\n\u{2610} x\n\u{2610} New"
    );
}

fn headed(text: &str, heading: &str, under: Option<usize>) -> Result<String, &'static str> {
    let mut document = Document::parse(text);
    add(&mut document, heading, under)?;
    Ok(document.render())
}

#[test]
fn a_new_heading_at_the_top_level_goes_before_the_archive_after_a_blank_line() {
    assert_eq!(headed("", "# Sprint", None), Ok("# Sprint\n".to_owned()));
    assert_eq!(
        headed("\u{2610} a\n\nArchive:\n  \u{2714} b\n", "## Later", None),
        // The blank line that stood before `Archive:` stays there.
        Ok("\u{2610} a\n\n## Later\n\nArchive:\n  \u{2714} b\n".to_owned())
    );
    // No box: a heading line as written, closing `#`s and all; `#tag` stays a task.
    assert_eq!(
        headed("\u{2610} a", "### Done ###", None),
        Ok("\u{2610} a\n\n### Done ###".to_owned())
    );
    assert_eq!(headed("", "#tag", None), Ok("\u{2610} #tag\n".to_owned()));
}

#[test]
fn a_new_heading_below_a_heading_ends_its_whole_section() {
    // Below `## Ratios / Images`: before `## Header`, the next heading of its level.
    let text = headed(MARKDOWN, "### Crops", Some(2)).expect("added");
    let lines: Vec<&str> = text.split('\n').collect();
    assert_eq!(lines[4], "Notes on the images.");
    assert_eq!(lines[5], "");
    assert_eq!(lines[6], "### Crops");
    assert_eq!(lines[7], "");
    assert_eq!(lines[8], "## Header / Navigation ##");
    // Below `## Header`: past its `### Mobile` and the project after it, to the file's end.
    let text = headed(MARKDOWN, "### Desktop", Some(6)).expect("added");
    assert!(
        text.ends_with("  \u{2610} Footer\n\n### Desktop\n"),
        "{text}"
    );
    // The rest of the file is byte for byte what it was.
    assert_eq!(&text[..MARKDOWN.len()], MARKDOWN);
}

#[test]
fn a_new_heading_in_a_project_takes_its_childrens_indent_while_it_stays_a_heading() {
    assert_eq!(
        headed("Website:\n  \u{2610} a\nShop:\n", "## Design", Some(0)),
        Ok("Website:\n  \u{2610} a\n  ## Design\nShop:\n".to_owned())
    );
    // Four columns make no heading: refused, nothing changed.
    assert_eq!(
        headed("A:\n  B:\n    \u{2610} c\n", "## Deep", Some(1)),
        Err("errors.heading_depth")
    );
    assert_eq!(
        headed("A note\n", "## X", Some(0)),
        Err("errors.not_a_parent")
    );
}
