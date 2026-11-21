use std::path::{Path, PathBuf};

use super::*;

fn root() -> PathBuf {
    PathBuf::from(if cfg!(windows) { "C:\\site" } else { "/site" })
}

fn out() -> PathBuf {
    root().join(".rbrowser").join("out")
}

fn project_bases() -> Bases {
    bases(Some(&root()), None, None).expect("bases")
}

fn refused_text(result: Result<PathBuf, PathError>) -> String {
    match result {
        Err(PathError::Refused(text)) => text,
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn a_default_export_lands_in_the_exports_folder() {
    let bases = project_bases();
    assert_eq!(bases.folder, out().join("exports"));
    assert_eq!(bases.allowed, [out()]);
    assert_eq!(bases.root, Some(root()));
    assert_eq!(
        resolve(&bases, None, "run-1.sarif"),
        Ok(out().join("exports").join("run-1.sarif"))
    );
}

#[test]
fn a_relative_path_lands_below_the_export_folder() {
    let bases = project_bases();
    assert_eq!(
        resolve(&bases, Some("ci/report.sarif"), "x"),
        Ok(out().join("exports").join("ci").join("report.sarif"))
    );
    assert_eq!(
        resolve(&bases, Some("ci\\.\\report.xml"), "x"),
        Ok(out().join("exports").join("ci").join("report.xml"))
    );
}

#[test]
fn a_path_that_goes_up_is_refused() {
    let bases = project_bases();
    for path in [
        "../x.sarif",
        "a/../../x.sarif",
        "a\\..\\..\\..\\config.json",
        "..",
        "ok/..",
    ] {
        let text = refused_text(resolve(&bases, Some(path), "x"));
        assert!(text.contains(".."), "{path}: {text}");
    }
}

#[test]
fn an_absolute_path_outside_the_allowed_folders_is_refused() {
    let bases = project_bases();
    for path in [
        "/etc/passwd",
        "/site/.rbrowser/config.json",
        "/site/.rbrowser/outside/x.sarif",
        "/site/src/main.ts",
        "/site/.rbrowser/out",
    ] {
        refused_text(resolve(&bases, Some(path), "x"));
    }
}

#[cfg(not(windows))]
#[test]
fn an_absolute_path_inside_the_project_output_is_taken() {
    let bases = project_bases();
    assert_eq!(
        resolve(&bases, Some("/site/.rbrowser/out/review/x.html"), "x"),
        Ok(PathBuf::from("/site/.rbrowser/out/review/x.html"))
    );
    refused_text(resolve(
        &bases,
        Some("/site/.rbrowser/out/../config.json"),
        "x",
    ));
    refused_text(resolve(&bases, Some("/site/.rbrowser/out/./x.html"), "x"));
}

#[test]
fn drives_streams_shares_devices_and_control_characters_are_refused() {
    let bases = project_bases();
    for path in [
        "C:\\Windows\\x.sarif",
        "c:x.sarif",
        "report.sarif:stream",
        "\\\\server\\share\\x.sarif",
        "//server/share/x.sarif",
        "\\\\.\\pipe\\x",
        "\\\\?\\C:\\x",
        "a\u{0}b.sarif",
        "a\nb.sarif",
        "",
    ] {
        refused_text(resolve(&bases, Some(path), "x"));
    }
    let long = "a".repeat(MAX_PATH_LEN + 1);
    refused_text(resolve(&bases, Some(&long), "x"));
}

#[test]
fn a_path_without_a_file_name_is_refused() {
    let bases = project_bases();
    for path in ["/", ".", "./", "\\"] {
        refused_text(resolve(&bases, Some(path), "x"));
    }
}

#[cfg(not(windows))]
#[test]
fn the_user_settings_may_name_an_absolute_folder() {
    let bases = bases(Some(&root()), None, Some("/home/me/reports")).expect("bases");
    assert_eq!(bases.folder, PathBuf::from("/home/me/reports"));
    assert_eq!(bases.allowed, [out(), PathBuf::from("/home/me/reports")]);
    assert_eq!(
        resolve(&bases, None, "run-1.xml"),
        Ok(PathBuf::from("/home/me/reports/run-1.xml"))
    );
    assert_eq!(
        resolve(&bases, Some("/home/me/reports/a/b.html"), "x"),
        Ok(PathBuf::from("/home/me/reports/a/b.html"))
    );
    refused_text(resolve(&bases, Some("/home/me/other.html"), "x"));
    // Without a project, the user's folder is still a place to write.
    let alone = bases_without_project();
    assert_eq!(alone.allowed, [PathBuf::from("/home/me/reports")]);
}

#[cfg(not(windows))]
fn bases_without_project() -> Bases {
    bases(None, None, Some("/home/me/reports")).expect("bases")
}

#[test]
fn a_project_may_only_name_a_folder_below_its_output() {
    let bases = bases(Some(&root()), Some("reviews"), None).expect("bases");
    assert_eq!(bases.folder, out().join("reviews"));
    assert_eq!(bases.allowed, [out()]);
    for dir in ["/tmp/x", "../src", "C:\\x", "\\\\host\\share"] {
        assert!(
            matches!(bases_of_project(dir), Err(PathError::Refused(_))),
            "{dir}"
        );
    }
}

fn bases_of_project(dir: &str) -> Result<Bases, PathError> {
    bases(Some(&root()), Some(dir), None)
}

#[test]
fn a_share_as_the_user_folder_is_refused() {
    assert!(matches!(
        bases(Some(&root()), None, Some("\\\\host\\share\\reports")),
        Err(PathError::Refused(_))
    ));
}

#[test]
fn without_a_project_or_a_folder_there_is_nowhere_to_write() {
    assert_eq!(bases(None, None, None), Err(PathError::NoBase));
    assert_eq!(bases(None, Some("reviews"), None), Err(PathError::NoBase));
    assert_eq!(bases(None, None, Some("relative")), Err(PathError::NoBase));
}

#[test]
fn a_write_makes_the_folders_and_writes_the_text() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let bases = bases(Some(dir.path()), None, None).expect("bases");
    let path = resolve(&bases, Some("ci/run.sarif"), "x").expect("a path");
    write_inside(&bases, &path, "{}\n").expect("written");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "{}\n");
}

#[cfg(unix)]
#[test]
fn a_folder_link_out_of_the_output_is_refused_on_writing() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let outside = tempfile::tempdir().expect("another temp dir");
    let bases = bases(Some(dir.path()), None, None).expect("bases");
    std::fs::create_dir_all(&bases.folder).expect("the folder");
    std::os::unix::fs::symlink(outside.path(), bases.folder.join("escape")).expect("a link");
    let path = resolve(&bases, Some("escape/x.sarif"), "x").expect("lexically fine");
    let error = write_inside(&bases, &path, "x").expect_err("refused");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(!outside.path().join("x.sarif").exists());
}

#[cfg(unix)]
#[test]
fn a_file_link_is_never_written_through() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let outside = tempfile::tempdir().expect("another temp dir");
    let target = outside.path().join("victim.txt");
    std::fs::write(&target, "keep").expect("the victim");
    let bases = bases(Some(dir.path()), None, None).expect("bases");
    std::fs::create_dir_all(&bases.folder).expect("the folder");
    let link = bases.folder.join("run.sarif");
    std::os::unix::fs::symlink(&target, &link).expect("a link");
    assert!(write_inside(&bases, &link, "x").is_err());
    assert!(write_file(&link, "x").is_err());
    assert_eq!(std::fs::read_to_string(&target).expect("read"), "keep");
}

#[test]
fn the_person_s_own_file_is_written_where_named() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let path: &Path = &dir.path().join("deep").join("report.sarif");
    write_file(path, "x").expect("written");
    assert_eq!(std::fs::read_to_string(path).expect("read"), "x");
}

#[cfg(unix)]
#[test]
fn a_checked_in_link_as_the_output_folder_is_refused_on_writing() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let outside = tempfile::tempdir().expect("another temp dir");
    std::fs::create_dir_all(dir.path().join(".rbrowser")).expect("the config folder");
    std::os::unix::fs::symlink(outside.path(), dir.path().join(".rbrowser").join("out"))
        .expect("a link");
    let bases = bases(Some(dir.path()), None, None).expect("bases");
    let path = resolve(&bases, Some("authorized_keys"), "x").expect("lexically fine");
    let error = write_inside(&bases, &path, "x").expect_err("refused");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(
        !outside
            .path()
            .join("exports")
            .join("authorized_keys")
            .exists()
    );
}
