use std::fs;
use std::path::Path;

use super::*;

const HELLO: &str = r#"{ "manifest_version": 3, "name": "Hello", "version": "1.0" }"#;

fn extension(dir: &Path, name: &str, manifest: &str) -> String {
    let folder = dir.join(name);
    fs::create_dir_all(&folder).expect("folder");
    fs::write(folder.join(MANIFEST_FILE), manifest).expect("manifest");
    folder.to_string_lossy().into_owned()
}

#[test]
fn a_folder_with_a_valid_manifest_is_an_extension() {
    let dir = tempfile::tempdir().expect("dir");
    let path = extension(dir.path(), "hello", HELLO);
    let found = check(&path).expect("valid");
    assert_eq!(found.folder, Path::new(&path));
    assert_eq!(found.manifest.name, "Hello");
}

#[test]
fn the_manifest_itself_names_its_folder() {
    let dir = tempfile::tempdir().expect("dir");
    let path = extension(dir.path(), "hello", HELLO);
    let manifest = Path::new(&path).join(MANIFEST_FILE);
    let found = check(&manifest.to_string_lossy()).expect("valid");
    assert_eq!(found.folder, Path::new(&path));
}

#[test]
fn a_byte_order_mark_is_no_reason_to_refuse() {
    let dir = tempfile::tempdir().expect("dir");
    let path = extension(dir.path(), "bom", &format!("\u{feff}{HELLO}"));
    assert!(check(&path).is_ok());
}

#[test]
fn what_is_no_extension_folder_is_refused_with_its_reason() {
    let dir = tempfile::tempdir().expect("dir");
    let empty = dir.path().join("empty");
    fs::create_dir_all(&empty).expect("folder");
    let v2 = extension(
        dir.path(),
        "v2",
        r#"{ "manifest_version": 2, "name": "a", "version": "1" }"#,
    );
    let gone = dir.path().join("gone");
    assert!(matches!(check(""), Err(FolderError::Missing)));
    assert!(matches!(
        check(&gone.to_string_lossy()),
        Err(FolderError::Missing)
    ));
    assert!(matches!(
        check(&empty.to_string_lossy()),
        Err(FolderError::NoManifest)
    ));
    assert!(matches!(
        check(&v2),
        Err(FolderError::Manifest(ManifestError::NotV3(_)))
    ));
    assert!(matches!(
        check("relative/folder"),
        Err(FolderError::NotAbsolute)
    ));
}

/// Chromium splits `--load-extension` at commas: such a path cannot be named.
#[test]
fn a_path_with_a_comma_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let path = extension(dir.path(), "a,b", HELLO);
    assert!(matches!(check(&path), Err(FolderError::Comma)));
}

#[test]
fn an_oversized_manifest_is_not_read() {
    let dir = tempfile::tempdir().expect("dir");
    let padding = " ".repeat(usize::try_from(MAX_MANIFEST_BYTES).expect("fits"));
    let path = extension(dir.path(), "big", &format!("{HELLO}{padding}"));
    assert!(matches!(check(&path), Err(FolderError::TooLarge)));
}

#[test]
fn a_project_folder_resolves_inside_the_project() {
    let dir = tempfile::tempdir().expect("dir");
    extension(&dir.path().join("tools"), "my-extension", HELLO);
    for written in [
        "tools/my-extension",
        "tools\\my-extension",
        "./tools/my-extension/",
    ] {
        let found = project_folder(dir.path(), written).expect(written);
        assert_eq!(
            found,
            dir.path().join("tools").join("my-extension"),
            "{written}"
        );
    }
}

#[test]
fn a_project_folder_outside_the_project_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join("project");
    fs::create_dir_all(&root).expect("root");
    extension(dir.path(), "x", HELLO);
    for written in [
        "../x",
        "tools/../../x",
        "..\\x",
        "/etc",
        "\\\\server\\share\\x",
        "C:\\x",
        "C:x",
        "",
    ] {
        assert!(
            matches!(
                project_folder(&root, written),
                Err(FolderError::OutsideProject)
            ),
            "{written}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_link_out_of_the_project_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join("project");
    fs::create_dir_all(&root).expect("root");
    let outside = extension(dir.path(), "outside", HELLO);
    std::os::unix::fs::symlink(&outside, root.join("linked")).expect("link");
    assert!(matches!(
        project_folder(&root, "linked"),
        Err(FolderError::OutsideProject)
    ));
}

#[test]
fn every_folder_error_has_a_stable_code() {
    let errors = [
        FolderError::Missing,
        FolderError::NotAbsolute,
        FolderError::Comma,
        FolderError::NoManifest,
        FolderError::TooLarge,
        FolderError::Unreadable("x".to_owned()),
        FolderError::OutsideProject,
    ];
    let mut codes: Vec<&str> = errors.iter().map(FolderError::code).collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), errors.len());
    assert_eq!(
        FolderError::Manifest(ManifestError::NoName).code(),
        "extensions.manifest_no_name"
    );
}

/// The fixture of the Windows acceptance (`tests/fixtures/extensions/hello`) is an extension
/// rBrowser loads.
#[test]
fn the_hello_fixture_is_a_valid_extension() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/extensions/hello")
        .canonicalize()
        .expect("the fixture exists");
    let found = check(&fixture.to_string_lossy()).expect("valid");
    assert_eq!(found.manifest.name, "rBrowser fixture: hello");
    assert_eq!(found.manifest.version, "1.0.0");
}
