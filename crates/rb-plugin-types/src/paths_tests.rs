use super::{PathRefusal, in_config_dir, is_local_config, relative_segments};
use crate::codes;

#[test]
fn a_plain_relative_path_is_split() {
    assert_eq!(
        relative_segments("report/2026/index.html"),
        Ok(vec!["report", "2026", "index.html"])
    );
    assert_eq!(relative_segments("a\\b.txt"), Ok(vec!["a", "b.txt"]));
}

#[test]
fn climbing_out_is_refused() {
    for path in ["../escape.txt", "a/../../b", "a\\..\\b", ".."] {
        assert_eq!(relative_segments(path), Err(PathRefusal::Parent), "{path}");
    }
}

#[test]
fn absolute_paths_drives_and_streams_are_refused() {
    for path in [
        "/etc/passwd",
        "\\\\server\\share",
        "C:\\Windows",
        "c:x",
        "file.txt:stream",
    ] {
        assert_eq!(
            relative_segments(path),
            Err(PathRefusal::Absolute),
            "{path}"
        );
    }
}

#[test]
fn segments_windows_would_change_are_refused() {
    for path in [
        "a//b",
        "./a",
        "a/.",
        "name.",
        "name ",
        "CON",
        "nul.txt",
        "x/LPT1.log",
    ] {
        assert_eq!(relative_segments(path), Err(PathRefusal::Segment), "{path}");
    }
    assert_eq!(relative_segments(""), Err(PathRefusal::Empty));
    assert_eq!(relative_segments("a\nb"), Err(PathRefusal::Control));
    assert_eq!(
        relative_segments(&"a/".repeat(40)),
        Err(PathRefusal::TooLong)
    );
}

#[test]
fn a_refusal_names_the_path_under_its_code() {
    let failure = PathRefusal::Parent.failure("../escape.txt");
    assert_eq!(failure.code(), Some(codes::PATH_OUTSIDE));
    assert_eq!(failure.param("path"), Some("../escape.txt"));
}

#[test]
fn the_local_config_is_recognised_in_any_case() {
    assert!(is_local_config(&[".rbrowser", "config.local.json"]));
    assert!(is_local_config(&[".rbrowser", "CONFIG.LOCAL.JSON"]));
    assert!(!is_local_config(&[".rbrowser", "config.json"]));
    assert!(in_config_dir(&[".rbrowser", "config.json"]));
    assert!(!in_config_dir(&[".rbrowser"]));
    assert!(!in_config_dir(&["src", "main.css"]));
}

#[test]
fn the_config_folder_and_the_local_config_are_found_whatever_their_case() {
    // Windows opens `.RBROWSER\CONFIG.LOCAL.JSON` as the real file, and NTFS folds `ſ` to `S` and
    // `ı` to `I` when it compares names.
    for segments in [
        &[".RBROWSER", "config.local.json"][..],
        &[".Rbrowser", "Config.Local.Json"],
        &[".rbrowſer", "config.local.json"],
        &[".rbrowser", "confıg.local.json"],
    ] {
        assert!(is_local_config(segments), "{segments:?}");
        assert!(in_config_dir(segments), "{segments:?}");
    }
    assert!(!is_local_config(&["rbrowser", "config.local.json"]));
    assert!(!is_local_config(&[".rbrowser", "sub", "config.local.json"]));
}

#[test]
fn short_names_windows_makes_up_are_refused() {
    // `CONFIG~1.JSO` is the 8.3 name Windows may give `config.local.json`.
    for path in [
        "CONFIG~1.JSO",
        ".rbrowser/CONFIG~2.JSO",
        "RBROWS~1/config.local.json",
        "a/b~12",
    ] {
        assert_eq!(relative_segments(path), Err(PathRefusal::Segment), "{path}");
    }
    for path in ["notes~", "a~b.txt", "~/x", "x/~draft.md"] {
        assert!(relative_segments(path).is_ok(), "{path}");
    }
}
