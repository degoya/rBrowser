//! Guards the close of every browser hosted inside the main window: each
//! `wrap_life_span_handler!` answers `do_close` itself, unless its browser has a top-level
//! window of its own (the allow-list below, with the reason).
//!
//! CEF's default `DoClose()` returns false, which "will send the standard close notification to
//! the browser's top-level parent window (... CefWindowDelegate::CanClose() callback from
//! Views)" (`cef_life_span_handler.h`). For a grid view, the shell, the DevTools dock or its
//! splitter that parent is the main window, which answers `can_close` with 1 and quits the app:
//! focus mode closing the other views ended rBrowser (owner, build 22481e2, RB-010-12).

use crate::{files_with, relative, workspace_root};

/// The life span handlers whose browser closes with a window of its own, so CEF's default close
/// notification goes to that window and not to the main one.
const OWN_WINDOW: &[(&str, &str)] = &[
    (
        "struct PagesLifeSpan",
        "the pages window (`settings_window.rs`) is a top-level window of its own",
    ),
    (
        "struct WindowLifeSpan",
        "the native DevTools window (`devtools.rs`) is a top-level window of its own",
    ),
];

/// The text of each `wrap_life_span_handler! { ... }` block in `source` (to the closing brace
/// at the start of a line, as rustfmt writes it).
fn blocks(source: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find("wrap_life_span_handler! {") {
        let tail = &rest[start..];
        let end = tail.find("\n}").map_or(tail.len(), |end| end + 2);
        out.push(&tail[..end]);
        rest = &tail[end..];
    }
    out
}

/// Whether `block` may leave `do_close` to CEF's default.
fn allowed(block: &str) -> bool {
    OWN_WINDOW.iter().any(|(name, _)| {
        block
            .find(name)
            .is_some_and(|at| !is_ident_char(block[at + name.len()..].chars().next()))
    })
}

fn is_ident_char(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

#[test]
fn every_life_span_handler_in_the_main_window_answers_do_close() {
    let root = workspace_root();
    let mut files = Vec::new();
    files_with(&root.join("crates"), &[".rs"], &mut files);
    let mut missing = Vec::new();
    let mut seen = 0;
    for file in files {
        // This lint's own examples are not life span handlers.
        if file.ends_with("tests/repo_lints/cef_do_close.rs") {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&file) else {
            continue;
        };
        for block in blocks(&source) {
            seen += 1;
            if !allowed(block) && !block.contains("fn do_close(") {
                let name = block.lines().nth(1).unwrap_or_default().trim();
                missing.push(format!("{}: {name}", relative(&root, &file)));
            }
        }
    }
    assert!(seen > 0, "no life span handler found - the scan is broken");
    assert!(
        missing.is_empty(),
        "life span handlers leave CEF's DoClose() default, so closing their browser closes the \
         main window and quits the app; answer `do_close` or name the window of their own:\n{}",
        missing.join("\n")
    );
}

#[test]
fn the_allow_list_matches_whole_names_only() {
    assert!(allowed(
        "wrap_life_span_handler! {\n    struct PagesLifeSpan {}\n}"
    ));
    assert!(!allowed(
        "wrap_life_span_handler! {\n    struct PagesLifeSpanToo {}\n}"
    ));
    assert!(!allowed(
        "wrap_life_span_handler! {\n    struct ViewLifeSpan {}\n}"
    ));
}

#[test]
fn a_block_ends_at_its_closing_brace() {
    let source = "wrap_life_span_handler! {\n    struct A {}\n}\nfn x() {}\nwrap_life_span_handler! {\n    struct B {}\n}\n";
    let found = blocks(source);
    assert_eq!(found.len(), 2);
    assert!(found[0].contains("struct A") && !found[0].contains("struct B"));
}
