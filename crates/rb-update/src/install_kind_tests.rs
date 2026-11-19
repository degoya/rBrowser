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
        (INSTALL_KIND_FILE, "chocolatey"),
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
        InstallKind::Homebrew,
        InstallKind::Deb,
        InstallKind::Rpm,
    ] {
        assert_eq!(InstallKind::parse(kind.as_str()), Some(kind));
    }
    assert_eq!(InstallKind::parse("unknown"), None);
}

/// Scoop writes its marker (`packaging/scoop/rbrowser.json.in`); winget and Homebrew write none,
/// so their folders tell.
#[test]
fn a_package_managers_folder_names_its_kind_without_a_marker() {
    for (path, kind) in [
        (r"C:\Users\a\scoop\apps\rbrowser\0.9.0", InstallKind::Scoop),
        (
            r"C:\Users\a\AppData\Local\Microsoft\WinGet\Packages\degoya.rBrowser_Microsoft.Winget.Source_8wekyb3d8bbwe\rbrowser-0.9.0-windows-x64",
            InstallKind::Winget,
        ),
        (
            "/opt/homebrew/Caskroom/rbrowser/0.9.0/rBrowser.app/Contents/MacOS",
            InstallKind::Homebrew,
        ),
        (
            "/home/linuxbrew/.linuxbrew/Cellar/rbrowser/0.9.0/libexec",
            InstallKind::Homebrew,
        ),
    ] {
        assert_eq!(by_path(Path::new(path)), Some(kind), "{path}");
    }
    assert_eq!(by_path(Path::new(r"C:\Tools\rbrowser-0.9.0")), None);
}

/// A portable ZIP unpacked into a folder whose path merely contains `cellar`, `caskroom`,
/// `scoop` or `winget` stays portable: the rules name the package manager's own layout with
/// rBrowser's package in it, not a word anywhere in the path.
#[test]
fn a_portable_folder_with_a_package_managers_word_in_its_path_stays_portable() {
    for path in [
        r"D:\Cellar\tools\rbrowser-0.9.0",
        "/home/a/Caskroom/rbrowser-0.9.0",
        r"C:\scoop\apps\other\rbrowser-0.9.0",
        r"C:\Users\a\WinGet\Packages\rbrowser",
        "/opt/homebrew/Cellar/other/1.0/rbrowser-0.9.0",
    ] {
        assert_eq!(by_path(Path::new(path)), None, "{path}");
    }
    let root = tempfile::tempdir().expect("tempdir");
    let folder = root.path().join("Cellar").join("tools");
    fs::create_dir_all(&folder).expect("folder");
    fs::write(folder.join(VERSION_FILE), "rBrowser 0.9.0\n").expect("write");
    assert_eq!(detect(&folder), InstallKind::Portable);
}

/// The job's rule: portable updates itself, a package manager shows its command, anything else
/// only the download.
#[test]
fn each_kind_says_how_it_is_updated() {
    assert_eq!(InstallKind::Portable.update_way(), UpdateWay::SelfUpdate);
    assert_eq!(
        InstallKind::Scoop.update_way(),
        UpdateWay::Command("scoop update rbrowser")
    );
    assert_eq!(
        InstallKind::Winget.update_way(),
        UpdateWay::Command("winget upgrade degoya.rBrowser")
    );
    assert_eq!(
        InstallKind::Homebrew.update_way(),
        UpdateWay::Command("brew upgrade rbrowser")
    );
    assert!(matches!(
        InstallKind::Deb.update_way(),
        UpdateWay::Command(_)
    ));
    assert!(matches!(
        InstallKind::Rpm.update_way(),
        UpdateWay::Command(_)
    ));
    assert_eq!(InstallKind::Msi.update_way(), UpdateWay::DownloadOnly);
    assert_eq!(InstallKind::Unknown.update_way(), UpdateWay::DownloadOnly);
}
