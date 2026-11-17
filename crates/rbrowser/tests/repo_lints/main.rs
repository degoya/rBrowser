//! The repository's own rules as tests, one binary for all of them so a run links it once:
//!
//! * `file_length` - at most 500 lines per Rust source and per script.
//! * `no_german` - code and documentation are English-only; the job files are German.
//! * `no_console_window` - every spawn asks for no console window on Windows.
//! * `cef_window_frame` - every CEF window answers its frame buttons.
//! * `cef_do_close` - every browser inside the main window answers `do_close`.
//! * `cef_release` - every UI-thread holder of a CEF object is released before CEF shuts down.
//! * `no_wall_clock_upper_bound` - no test asserts an upper bound on the wall clock.
//!
//! `scan` is what they read of a source: the code without comments and texts, calls and the
//! statements they stand in.

use std::path::{Path, PathBuf};

mod cef_do_close;
mod cef_release;
mod cef_window_frame;
mod file_length;
mod no_console_window;
mod no_german;
mod no_wall_clock_upper_bound;
mod scan;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

/// Every file below `directory` whose name ends in one of `extensions`, skipping the workspace's
/// build directory (`<root>/target`), the throw-away spikes (`<root>/spikes`, outside the
/// workspace), `node_modules` and hidden directories. A folder named `target` anywhere else is
/// source like any other and is read.
fn files_with(directory: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
    let root = workspace_root();
    walk(
        directory,
        extensions,
        &[root.join("target"), root.join("spikes")],
        out,
    );
}

fn walk(directory: &Path, extensions: &[&str], skipped: &[PathBuf], out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if kind.is_dir() {
            // A crate's own `target/` (a plugin builds in its folder) is build output.
            let build_output = name == "target" && directory.join("Cargo.toml").is_file();
            if !name.starts_with('.')
                && name != "node_modules"
                && !build_output
                && !skipped.contains(&path)
            {
                walk(&path, extensions, skipped, out);
            }
        } else if kind.is_file() && extensions.iter().any(|ext| name.ends_with(ext)) {
            out.push(path);
        }
    }
}

/// Only the directories the walk is given are skipped by path: a folder named `target` below
/// `src/` is source and is read.
#[test]
fn only_the_given_directories_are_skipped() {
    let root = tempfile::tempdir().expect("tempdir");
    let skipped = [root.path().join("target"), root.path().join("spikes")];
    for file in [
        "target/debug/built.rs",
        "spikes/cef/src/main.rs",
        "src/target/kept.rs",
        "src/lib.rs",
        "ui/node_modules/x/index.rs",
        ".hidden/x.rs",
    ] {
        let path = root.path().join(file);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("dirs");
        std::fs::write(&path, "").expect("writes");
    }
    let mut found = Vec::new();
    walk(root.path(), &[".rs"], &skipped, &mut found);
    let mut found: Vec<String> = found
        .iter()
        .map(|path| relative(root.path(), path))
        .collect();
    found.sort();
    assert_eq!(found, ["src/lib.rs", "src/target/kept.rs"]);
}

/// `path` relative to the workspace root, with `/` on every platform.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
