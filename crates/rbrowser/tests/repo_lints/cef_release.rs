//! Guards the end of the UI thread (RB-080-91): every `thread_local!` of `cef_app` is emptied
//! by `cef_app/teardown.rs` when the main window is destroyed, or says here why it holds no CEF
//! object at the end. A thread-local still holding one is dropped by the CRT after
//! `CefShutdown` - the owner's crash dumps were the layout modes' overlays (`placed.rs`) released
//! into a CEF that was gone.

use crate::{relative, workspace_root};

/// The thread-locals that hold no CEF object when the window is gone, and why.
const KEPT: &[(&str, &str, &str)] = &[
    ("active_label", "ACTIVE", "a view id"),
    ("board_pan", "PAN", "the middle button's pan: numbers"),
    ("board_pan", "DRAG", "a number"),
    ("board_pan", "FROM", "a camera: numbers"),
    ("board_pan", "LATEST", "numbers"),
    ("board_pan", "PRESS", "a window handle and a point"),
    ("board_pan", "REPLAY", "a count"),
    ("board_pan", "WHEEL", "a wheel turn: numbers"),
    ("active_label", "ENABLED", "a switch"),
    ("active_label", "NEUTRAL", "colours by view"),
    ("page_windows", "GENERATIONS", "counters"),
    (
        "embed",
        "STATE",
        "Win32 handles of the external windows; the engines end them",
    ),
    (
        "strip_clicks",
        "HOOK",
        "a Win32 hook, uninstalled on destroy",
    ),
    ("panel_dock", "GENERATIONS", "counters"),
    (
        "settings_window",
        "PAGES",
        "emptied by its own window's destroy; the main window closes it first",
    ),
    (
        "page_windows",
        "SLOTS",
        "emptied by each window's destroy; the main window closes them first",
    ),
    (
        "pop_out",
        "WINDOWS",
        "drained by `close_all` when the main window is asked to close",
    ),
];

/// The names of the statics declared in the `thread_local!` blocks of `source`.
fn thread_locals(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find("thread_local! {") {
        let tail = &rest[start..];
        let end = tail.find("\n}").map_or(tail.len(), |end| end + 2);
        for line in tail[..end].lines() {
            let line = line.trim_start();
            let declared = line
                .strip_prefix("pub static ")
                .or_else(|| line.strip_prefix("static "));
            if let Some(name) = declared.and_then(|rest| rest.split(':').next()) {
                names.push(name.trim().to_owned());
            }
        }
        rest = &tail[end..];
    }
    names
}

#[test]
fn every_ui_thread_holder_is_released_before_cef_shuts_down() {
    let root = workspace_root();
    let dir = root.join("crates/rbrowser/src/cef_app");
    let teardown = std::fs::read_to_string(dir.join("teardown.rs")).expect("teardown.rs");
    let entries = std::fs::read_dir(&dir).expect("cef_app");
    let mut missing = Vec::new();
    let mut seen = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(module) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if path.extension().is_none_or(|ext| ext != "rs") || module.ends_with("_tests") {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        for name in thread_locals(&source) {
            seen += 1;
            let kept = KEPT
                .iter()
                .any(|(file, kept, _)| *file == module && *kept == name);
            let released = teardown.contains(&format!("super::{module}::"))
                || (module == "views" && teardown.contains("UI.with"));
            if !kept && !released {
                missing.push(format!("{}: {name}", relative(&root, &path)));
            }
        }
    }
    assert!(seen > 0, "no thread-local found - the scan is broken");
    assert!(
        missing.is_empty(),
        "thread-locals of the UI thread that `cef_app/teardown.rs` does not release - dropped \
         after CefShutdown they crash the end of the app; release them there or name the reason \
         in KEPT:\n{}",
        missing.join("\n")
    );
}

#[test]
fn the_scan_finds_the_statics_of_a_block() {
    let source = "thread_local! {\n    /// Doc.\n    static A: RefCell<u8> = RefCell::new(0);\n    pub static B: Cell<u8> = const { Cell::new(0) };\n}\n";
    assert_eq!(thread_locals(source), ["A", "B"]);
}
