use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::*;

/// `path` made absolute on this platform (a drive on Windows).
fn abs(path: &str) -> PathBuf {
    PathBuf::from(format!("{}{path}", if cfg!(windows) { "C:" } else { "" }))
}

fn env() -> Env {
    Env {
        path: Some(std::env::join_paths([abs("/usr/bin"), abs("/opt/tools")]).expect("a PATH")),
        android_home: Some(abs("/sdk/home")),
        sdk_root: Some(abs("/sdk/root")),
        local_app_data: None,
        home: Some(abs("/home/me")),
    }
}

fn files(paths: &[&str]) -> impl Fn(&Path) -> bool {
    let set: HashSet<PathBuf> = paths.iter().map(|path| abs(path)).collect();
    move |path: &Path| set.contains(path)
}

/// The owner's order (2026-11-21): the setting, PATH, ANDROID_HOME, ANDROID_SDK_ROOT, the usual
/// SDK folder, then the Platform Tools rBrowser downloaded.
#[test]
fn the_candidates_come_in_the_owners_order() {
    let managed = abs("/tools/adb/37.0.1/adb");
    let list = candidates(Some("/own/adb"), &env(), Some(&managed), "adb");
    let sources: Vec<Source> = list.iter().map(|(_, source)| *source).collect();
    assert_eq!(
        sources,
        [
            Source::Setting,
            Source::Path,
            Source::Path,
            Source::AndroidHome,
            Source::SdkRoot,
            Source::UsualSdk,
            Source::UsualSdk,
            Source::Managed,
        ]
    );
    assert_eq!(list[1].0, abs("/usr/bin/adb"));
    assert_eq!(list[3].0, abs("/sdk/home/platform-tools/adb"));
    assert_eq!(list[4].0, abs("/sdk/root/platform-tools/adb"));
    assert_eq!(list[7].0, managed);
}

#[test]
fn the_first_file_wins() {
    let found = locate(
        None,
        &env(),
        None,
        "adb",
        files(&["/sdk/root/platform-tools/adb", "/opt/tools/adb"]),
    )
    .expect("found");
    assert_eq!(
        found,
        Found {
            path: abs("/opt/tools/adb"),
            source: Source::Path,
        }
    );
}

#[test]
fn the_usual_sdk_folders_are_asked() {
    let bare = Env {
        path: None,
        android_home: None,
        sdk_root: None,
        local_app_data: Some(abs("/local")),
        home: Some(abs("/home/me")),
    };
    let found = locate(
        None,
        &bare,
        None,
        "adb.exe",
        files(&["/local/Android/Sdk/platform-tools/adb.exe"]),
    )
    .expect("found");
    assert_eq!(found.source, Source::UsualSdk);
    let mac = locate(
        None,
        &bare,
        None,
        "adb",
        files(&["/home/me/Library/Android/sdk/platform-tools/adb"]),
    )
    .expect("found");
    assert_eq!(mac.source, Source::UsualSdk);
}

/// A path the person set that names no file is said, never silently replaced by another adb.
#[test]
fn a_set_path_that_is_no_file_is_refused() {
    let error = locate(
        Some("/own/adb"),
        &env(),
        None,
        "adb",
        files(&["/usr/bin/adb"]),
    )
    .expect_err("refused");
    assert_eq!(error, Missing::SettingNotAFile(PathBuf::from("/own/adb")));
}

#[test]
fn nothing_found_says_so() {
    let error = locate(None, &env(), None, "adb", files(&[])).expect_err("none");
    assert_eq!(error, Missing::NotFound);
}

#[test]
fn the_managed_tools_come_last() {
    let managed = abs("/tools/adb/37.0.1/adb");
    let found = locate(
        None,
        &env(),
        Some(&managed),
        "adb",
        files(&["/tools/adb/37.0.1/adb"]),
    )
    .expect("found");
    assert_eq!(found.source, Source::Managed);
}

#[test]
fn the_program_name_follows_the_platform() {
    assert_eq!(program_name(true), "adb.exe");
    assert_eq!(program_name(false), "adb");
}

/// A relative or empty `PATH` entry names a folder of whatever the current directory is (a
/// project a page or a download put there): never asked.
#[test]
fn a_relative_path_entry_is_skipped() {
    let path = std::env::join_paths([
        PathBuf::from("tools"),
        PathBuf::from("."),
        PathBuf::new(),
        abs("/usr/bin"),
    ])
    .expect("a PATH");
    let relative = Env {
        path: Some(path),
        ..Env::default()
    };
    let list = candidates(None, &relative, None, "adb");
    assert_eq!(list, [(abs("/usr/bin/adb"), Source::Path)]);
    let in_cwd = |path: &Path| path.is_relative() || path == abs("/usr/bin/adb");
    let found = locate(None, &relative, None, "adb", in_cwd).expect("found");
    assert_eq!(found.path, abs("/usr/bin/adb"));
}

// RB-096-10: the debug log names the adb that runs and where it was found.
#[test]
fn the_adb_in_use_is_in_the_debug_log() {
    let log = crate::engines::android::adb::tests::Log::default();
    let _guard = tracing::subscriber::set_default(log.subscriber());
    let found = locate(
        None,
        &env(),
        None,
        "adb",
        files(&["/sdk/home/platform-tools/adb"]),
    )
    .expect("found");
    assert_eq!(found.source, Source::AndroidHome);
    let text = log.text();
    assert!(text.contains("adb in use"), "{text}");
    assert!(text.contains("AndroidHome"), "{text}");
    assert!(text.contains("platform-tools"), "{text}");
    let missing = locate(None, &Env::default(), None, "adb", files(&[]));
    assert_eq!(missing, Err(Missing::NotFound));
    assert!(log.text().contains("no adb"), "{}", log.text());
}
