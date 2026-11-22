use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use super::dialog;

const TITLE: &str = "rBrowser could not start";
const TEXT: &str = "No Chromium sandbox can start: install the deb or rpm package.";

/// A stand-in program `name` in `dir`, executable when `mode` says so.
fn program(dir: &Path, name: &str, mode: u32) {
    let path = dir.join(name);
    std::fs::write(&path, "#!/bin/sh\n").expect("write");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).expect("mode");
}

fn path_of(dirs: &[&Path]) -> OsString {
    std::env::join_paths(dirs).expect("joins")
}

/// RB-097-05 review: with a display the reason shows in the desktop's dialog - zenity first,
/// the text one argument without markup, no shell between.
#[test]
fn zenity_comes_first_with_the_text_as_one_argument() {
    let (first, second) = (
        tempfile::tempdir().expect("dir"),
        tempfile::tempdir().expect("dir"),
    );
    program(first.path(), "kdialog", 0o755);
    program(second.path(), "zenity", 0o755);
    let path = path_of(&[first.path(), second.path()]);
    let (program, args) = dialog(TITLE, TEXT, true, Some(&path)).expect("a dialog");
    assert_eq!(program, second.path().join("zenity"));
    assert_eq!(
        args,
        [
            "--error".to_owned(),
            "--no-markup".to_owned(),
            format!("--title={TITLE}"),
            format!("--text={TEXT}"),
        ]
    );
}

/// Without zenity kdialog shows it; a file that is not executable is no program.
#[test]
fn kdialog_without_zenity_and_no_file_that_cannot_run() {
    let dir = tempfile::tempdir().expect("dir");
    program(dir.path(), "zenity", 0o644);
    program(dir.path(), "kdialog", 0o755);
    let path = path_of(&[dir.path()]);
    let (program, args) = dialog(TITLE, TEXT, true, Some(&path)).expect("a dialog");
    assert_eq!(program, dir.path().join("kdialog"));
    assert_eq!(args, ["--title", TITLE, "--error", TEXT]);
}

/// No display, no `PATH` or neither program: stderr only.
#[test]
fn no_display_or_no_program_shows_no_dialog() {
    let dir = tempfile::tempdir().expect("dir");
    let empty = path_of(&[dir.path()]);
    assert!(dialog(TITLE, TEXT, true, Some(&empty)).is_none());
    assert!(dialog(TITLE, TEXT, true, None).is_none());
    program(dir.path(), "zenity", 0o755);
    assert!(dialog(TITLE, TEXT, false, Some(&empty)).is_none());
}
