use std::path::{Path, PathBuf};

use super::*;

#[test]
fn the_program_it_starts_is_rbrowser_exe_beside_it() {
    assert_eq!(
        gui_beside(Path::new("/app/rbrowser.com")),
        Some(PathBuf::from("/app/rbrowser.exe"))
    );
    assert_eq!(
        gui_beside(Path::new("/target/debug/rbrowser-console.exe")),
        Some(PathBuf::from("/target/debug/rbrowser.exe"))
    );
}

#[test]
fn it_never_starts_itself() {
    assert_eq!(gui_beside(Path::new("/app/rbrowser.exe")), None);
    assert_eq!(gui_beside(Path::new("rbrowser.exe")), None);
}

#[test]
fn a_program_that_cannot_start_is_an_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let result = run(
        &dir.path().join("rbrowser.exe"),
        Vec::<std::ffi::OsString>::new(),
        &mut out,
        &mut err,
        GRACE,
    );
    assert!(result.is_err());
}

#[test]
fn the_output_is_relayed_until_the_end_even_when_the_sink_fails() {
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("closed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let input = vec![b'x'; 3 * 8192 + 5];
    let mut reader = input.as_slice();
    relay(&mut reader, Broken);
    assert!(
        reader.is_empty(),
        "the pipe is drained, so the program never blocks"
    );

    let mut out = Vec::new();
    relay(&mut b"line one\nline two\n".as_slice(), &mut out);
    assert_eq!(out, b"line one\nline two\n");
}

/// A shell script standing in for rbrowser.exe.
#[cfg(unix)]
fn fake_gui(dir: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let path = dir.join("rbrowser.exe");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

#[cfg(unix)]
#[test]
fn a_command_prints_through_it_and_its_exit_code_comes_back() {
    let dir = tempfile::tempdir().expect("temp dir");
    let gui = fake_gui(
        dir.path(),
        "echo proposed; echo 'nothing was written' >&2; exit 7",
    );
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let outcome = run(&gui, ["init"], &mut out, &mut err, GRACE).expect("runs");
    assert!(matches!(outcome, Outcome::Exited(7)), "{outcome:?}");
    assert_eq!(String::from_utf8_lossy(&out), "proposed\n");
    assert_eq!(String::from_utf8_lossy(&err), "nothing was written\n");
}

#[cfg(unix)]
#[test]
fn the_arguments_reach_the_program_unchanged() {
    let dir = tempfile::tempdir().expect("temp dir");
    let gui = fake_gui(dir.path(), "printf '%s\\n' \"$@\"");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let outcome = run(
        &gui,
        ["init", "--url", "https://degoya.digital/a b"],
        &mut out,
        &mut err,
        GRACE,
    )
    .expect("runs");
    assert!(matches!(outcome, Outcome::Exited(0)), "{outcome:?}");
    assert_eq!(
        String::from_utf8_lossy(&out),
        "init\n--url\nhttps://degoya.digital/a b\n"
    );
}

#[cfg(unix)]
#[test]
fn an_app_that_lets_go_of_its_output_runs_on_and_the_console_is_free() {
    let dir = tempfile::tempdir().expect("temp dir");
    // What rbrowser.exe does before its window opens: it closes the handles it was given.
    let gui = fake_gui(dir.path(), "echo opening; exec >&- 2>&-; sleep 30");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let outcome = run(&gui, ["."], &mut out, &mut err, GRACE).expect("runs");
    let Outcome::Detached(mut child) = outcome else {
        panic!("the app is still running: {outcome:?}");
    };
    // Still running: `run` returned without waiting for the app's 30 s.
    assert!(child.try_wait().expect("state").is_none());
    assert_eq!(String::from_utf8_lossy(&out), "opening\n");
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(unix)]
#[test]
fn the_exit_code_counts_when_the_program_ends_right_after_its_output() {
    let dir = tempfile::tempdir().expect("temp dir");
    let gui = fake_gui(dir.path(), "exec >&- 2>&-; sleep 0.1; exit 3");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let outcome = run(&gui, ["--reload"], &mut out, &mut err, GRACE).expect("runs");
    assert!(matches!(outcome, Outcome::Exited(3)), "{outcome:?}");
}
