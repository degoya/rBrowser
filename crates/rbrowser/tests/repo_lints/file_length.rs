//! At most 500 lines per file (AGENTS.md, Conventions), for every Rust source of the repository,
//! every script under `scripts/` and `.claude/hooks/`, and the TypeScript and Vue sources of the
//! front ends (`ui/`, `agent/`, `sidecar/`, `plugins/`) - not their `dist/` build output.
//!
//! A file over the limit is split - a module of its own, re-exported where the old path was
//! public - unless it is one of `EXCEPTIONS`, each with the reason it stays whole. `BASELINE` is
//! a ratchet for files that are over the limit and may only shrink; it starts empty and should
//! stay that way. An entry whose file is back at the limit, or gone, leaves the list.
//!
//! Lines are counted as `wc -l` counts them.

use crate::{files_with, relative, workspace_root};

const LIMIT: usize = 500;

/// What the rule covers: Rust sources anywhere, scripts below [`SCRIPT_DIRS`], web sources below
/// [`WEB_DIRS`].
const RUST: &[&str] = &[".rs"];
const SCRIPTS: &[&str] = &[".sh", ".py"];
const SCRIPT_DIRS: &[&str] = &["scripts", ".claude/hooks"];
const WEB: &[&str] = &[".ts", ".mts", ".js", ".mjs", ".vue"];
const WEB_DIRS: &[&str] = &["ui", "agent", "sidecar", "plugins"];

/// Files above the limit on purpose, each with its reason. No count.
const EXCEPTIONS: &[(&str, &str)] = &[];

/// Files above the limit and their line counts when they were listed; may only go down.
const BASELINE: &[(&str, usize)] = &[];

/// The lines of `path` as `wc -l` counts them; a file that cannot be read is an error, never 0
/// lines.
fn line_count(path: &std::path::Path) -> std::io::Result<usize> {
    std::fs::read(path).map(|bytes| bytes.iter().filter(|byte| **byte == b'\n').count())
}

/// Every covered file over the limit, relative to the root with `/`, and its count; and every
/// covered file that cannot be read, with why.
fn over_limit() -> (Vec<(String, usize)>, Vec<String>) {
    let root = workspace_root();
    let mut files = Vec::new();
    files_with(&root, RUST, &mut files);
    assert!(!files.is_empty(), "no Rust sources were found at all");
    for directory in SCRIPT_DIRS {
        files_with(&root.join(directory), SCRIPTS, &mut files);
    }
    for directory in WEB_DIRS {
        let mut web = Vec::new();
        files_with(&root.join(directory), WEB, &mut web);
        // A front end's `dist/` is its build output (the sidecar's esbuild bundle), generated
        // and exempt like every generated file.
        web.retain(|path| !relative(&root, path).split('/').any(|part| part == "dist"));
        files.extend(web);
    }
    let mut over = Vec::new();
    let mut unreadable = Vec::new();
    for path in &files {
        match line_count(path) {
            Ok(lines) if lines > LIMIT => over.push((relative(&root, path), lines)),
            Ok(_) => {}
            Err(error) => unreadable.push(format!("{}: {error}", relative(&root, path))),
        }
    }
    over.sort();
    (over, unreadable)
}

#[test]
fn no_file_outgrows_the_limit() {
    let (over, unreadable) = over_limit();
    assert!(
        unreadable.is_empty(),
        "these files cannot be read, so their length is unknown:\n  {}",
        unreadable.join("\n  ")
    );
    let mut offenders = Vec::new();
    for (path, lines) in over {
        if EXCEPTIONS.iter().any(|(listed, _)| *listed == path) {
            continue;
        }
        match BASELINE.iter().find(|(listed, _)| *listed == path) {
            Some((_, baseline)) if lines <= *baseline => {}
            Some((_, baseline)) => {
                offenders.push(format!("{path}: {lines} lines, grown past its {baseline}"));
            }
            None => offenders.push(format!("{path}: {lines} lines")),
        }
    }
    assert!(
        offenders.is_empty(),
        "these files are over {LIMIT} lines (AGENTS.md, Conventions): split them into modules \
         of their own, re-exported where the old path was public\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_lists_name_only_files_still_over_the_limit() {
    let (over, _) = over_limit();
    let stale: Vec<String> = EXCEPTIONS
        .iter()
        .map(|(path, _)| *path)
        .chain(BASELINE.iter().map(|(path, _)| *path))
        .filter(|path| !over.iter().any(|(file, _)| file == path))
        .map(|path| format!("{path}: at or under {LIMIT} lines, or gone"))
        .collect();
    assert!(
        stale.is_empty(),
        "remove these entries from file_length.rs, the list only shrinks:\n  {}",
        stale.join("\n  ")
    );
}

/// A file that cannot be read is an error, not a file of 0 lines.
#[test]
fn an_unreadable_file_is_an_error_not_zero_lines() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(line_count(&dir.path().join("gone.rs")).is_err());
    let file = dir.path().join("three.rs");
    std::fs::write(&file, "a\nb\nc\n").expect("writes");
    assert_eq!(line_count(&file).expect("reads"), 3);
}
