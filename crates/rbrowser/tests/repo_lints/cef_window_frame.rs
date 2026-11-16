//! Guards the window frame of every CEF window: each `wrap_window_delegate!` answers
//! `can_resize`, `can_maximize` and `can_minimize` itself.
//!
//! The `cef` crate's trait defaults return 0 where CEF's own C++ defaults are true, so a window
//! delegate that leaves them out makes a window that cannot be resized, maximised or minimised
//! (the owner's main window, build ed2fd51). A popup that must not resize answers 0 on purpose.

use crate::{files_with, relative, workspace_root};

/// The answers every window delegate gives itself.
const REQUIRED: &[&str] = &["fn can_resize(", "fn can_maximize(", "fn can_minimize("];

/// The text of each `wrap_window_delegate! { ... }` block in `source` (to the closing brace at the
/// start of a line, as rustfmt writes it).
fn blocks(source: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find("wrap_window_delegate! {") {
        let tail = &rest[start..];
        let end = tail.find("\n}").map_or(tail.len(), |end| end + 2);
        out.push(&tail[..end]);
        rest = &tail[end..];
    }
    out
}

#[test]
fn every_window_delegate_answers_its_frame_buttons() {
    let root = workspace_root();
    let mut files = Vec::new();
    files_with(&root.join("crates"), &[".rs"], &mut files);
    let mut missing = Vec::new();
    let mut seen = 0;
    for file in files {
        // This lint's own examples are not window delegates.
        if file.ends_with("tests/repo_lints/cef_window_frame.rs") {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&file) else {
            continue;
        };
        for block in blocks(&source) {
            seen += 1;
            for required in REQUIRED {
                if !block.contains(required) {
                    missing.push(format!("{}: {required}", relative(&root, &file)));
                }
            }
        }
    }
    assert!(seen > 0, "no window delegate found - the scan is broken");
    assert!(
        missing.is_empty(),
        "window delegates leave the cef crate's 0 defaults (no resize/maximise/minimise):\n{}",
        missing.join("\n")
    );
}

#[test]
fn a_block_ends_at_its_closing_brace() {
    let source = "wrap_window_delegate! {\n    struct A {}\n}\nfn x() {}\nwrap_window_delegate! {\n    struct B {}\n}\n";
    let found = blocks(source);
    assert_eq!(found.len(), 2);
    assert!(found[0].contains("struct A") && !found[0].contains("struct B"));
}
