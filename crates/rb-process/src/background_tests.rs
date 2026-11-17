use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::*;

#[test]
fn with_nice_the_program_runs_through_it_at_a_lower_priority() {
    let (program, leading) = low_priority_argv(
        Path::new("/opt/ffmpeg/ffmpeg"),
        Some(Path::new("/usr/bin/nice")),
    );
    assert_eq!(program, PathBuf::from("/usr/bin/nice"));
    assert_eq!(
        leading,
        [
            OsString::from("-n"),
            OsString::from(NICE_INCREMENT.to_string()),
            OsString::from("/opt/ffmpeg/ffmpeg"),
        ]
    );
}

#[test]
fn without_nice_the_program_runs_itself() {
    let (program, leading) = low_priority_argv(Path::new("C:\\tools\\ffmpeg.exe"), None);
    assert_eq!(program, PathBuf::from("C:\\tools\\ffmpeg.exe"));
    assert!(leading.is_empty());
}

#[test]
fn nice_is_the_first_candidate_that_is_a_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("missing-nice");
    let present = dir.path().join("nice");
    std::fs::write(&present, b"program").expect("file");
    assert_eq!(
        first_file(&[missing.as_path(), present.as_path()]),
        Some(present.clone())
    );
    assert_eq!(first_file(&[missing.as_path()]), None);
    // A folder is no program.
    assert_eq!(first_file(&[dir.path()]), None);
}

/// Below normal is the priority class a background encoder gets on Windows; the value is the
/// Win32 constant.
#[test]
fn the_windows_priority_class_is_below_normal() {
    assert_eq!(BELOW_NORMAL_PRIORITY_CLASS, 0x0000_4000);
}

#[cfg(unix)]
#[tokio::test]
async fn a_low_priority_command_still_runs_with_its_arguments() {
    let output = crate::low_priority_command(Path::new("/bin/sh"))
        .args(["-c", "echo ran"])
        .output()
        .await
        .expect("runs");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "ran");
}
