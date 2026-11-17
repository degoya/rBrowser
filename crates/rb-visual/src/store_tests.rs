use std::path::Path;

use super::*;

#[test]
fn a_file_is_written_below_the_folder_with_png_appended() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = save_png(dir.path(), Path::new("run/mobile"), b"png").expect("saved");
    assert_eq!(path, dir.path().join("run").join("mobile.png"));
    assert_eq!(std::fs::read(&path).expect("read back"), b"png");
}

#[test]
fn an_existing_file_is_never_overwritten() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = save_png(dir.path(), Path::new("mobile"), b"1").expect("first");
    let second = save_png(dir.path(), Path::new("mobile"), b"2").expect("second");
    let third = save_png(dir.path(), Path::new("mobile"), b"3").expect("third");
    assert_eq!(first, dir.path().join("mobile.png"));
    assert_eq!(second, dir.path().join("mobile-2.png"));
    assert_eq!(third, dir.path().join("mobile-3.png"));
    assert_eq!(std::fs::read(&first).expect("first kept"), b"1");
}

#[test]
fn a_path_leaving_the_folder_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    for bad in ["../x", "/abs/x", ""] {
        assert!(
            save_png(dir.path(), Path::new(bad), b"x").is_err(),
            "{bad} was taken"
        );
    }
}

/// A folder link below the screenshot folder (checked into the project) is not followed.
#[cfg(unix)]
#[test]
fn a_folder_link_below_the_folder_is_not_followed() {
    let dir = tempfile::tempdir().expect("a folder");
    let elsewhere = tempfile::tempdir().expect("another folder");
    std::os::unix::fs::symlink(elsewhere.path(), dir.path().join("2026")).expect("a link");
    let result = save_png(dir.path(), Path::new("2026/desktop"), b"png");
    assert!(matches!(result, Err(VisualError::Folder(_))), "{result:?}");
    assert_eq!(
        std::fs::read_dir(elsewhere.path())
            .expect("readable")
            .count(),
        0
    );
}
