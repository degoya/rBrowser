use super::*;

fn folder_with(files: &[(&str, &str)]) -> tempfile::TempDir {
    let folder = tempfile::tempdir().expect("tempdir");
    for (name, text) in files {
        fs::write(folder.path().join(name), text).expect("write");
    }
    folder
}

#[test]
fn the_packaged_zip_is_portable_and_updates_itself() {
    let folder = folder_with(&[(VERSION_FILE, "rBrowser 1.1.0\ncommit abc\n")]);
    let kind = detect(folder.path());
    assert_eq!(kind, InstallKind::Portable);
    assert!(kind.installs_itself());
}

#[test]
fn a_marker_names_the_kind_and_only_portable_installs_itself() {
    let folder = folder_with(&[
        (VERSION_FILE, "rBrowser 1.1.0\n"),
        (INSTALL_KIND_FILE, "# written by the installer\n\nmsi\n"),
    ]);
    assert_eq!(detect(folder.path()), InstallKind::Msi);
    assert!(!InstallKind::Msi.installs_itself());
    assert!(!InstallKind::Scoop.installs_itself());
    let marked = folder_with(&[
        (VERSION_FILE, "rBrowser 1.1.0\n"),
        (INSTALL_KIND_FILE, "portable"),
    ]);
    assert_eq!(detect(marked.path()), InstallKind::Portable);
    let strange = folder_with(&[
        (VERSION_FILE, "rBrowser 1.1.0\n"),
        (INSTALL_KIND_FILE, "apt"),
    ]);
    assert_eq!(detect(strange.path()), InstallKind::Unknown);
}

#[test]
fn a_build_from_source_is_unknown() {
    let folder = folder_with(&[("rbrowser.exe", "program")]);
    assert_eq!(detect(folder.path()), InstallKind::Unknown);
    let other = folder_with(&[(VERSION_FILE, "rCommander 1.0.0\n")]);
    assert_eq!(detect(other.path()), InstallKind::Unknown);
}

#[test]
fn every_kind_parses_back() {
    for kind in [
        InstallKind::Portable,
        InstallKind::Msi,
        InstallKind::Scoop,
        InstallKind::Winget,
    ] {
        assert_eq!(InstallKind::parse(kind.as_str()), Some(kind));
    }
    assert_eq!(InstallKind::parse("unknown"), None);
}
