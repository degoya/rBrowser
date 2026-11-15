use std::io::Write as _;

use super::*;
use crate::extensions::crx3::{Verified, verify};
use crate::extensions::folder::check;

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/extensions/crx"
);

fn fixtures() -> serde_json::Value {
    let text = std::fs::read_to_string(format!("{FIXTURES}/fixtures.json")).expect("fixtures.json");
    serde_json::from_str(&text).expect("json")
}

fn publisher() -> [u8; 32] {
    let hex = fixtures()["publisherKeyHash"]
        .as_str()
        .expect("hash")
        .to_owned();
    let mut hash = [0_u8; 32];
    for (index, byte) in hash.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).expect("hex");
    }
    hash
}

fn id() -> String {
    fixtures()["id"].as_str().expect("id").to_owned()
}

fn crx(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{name}")).expect("fixture")
}

fn setup() -> (tempfile::TempDir, ExtensionStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ExtensionStore::new(dir.path().join("extensions"));
    (dir, store)
}

fn install(store: &ExtensionStore, name: &str) -> Result<InstalledExtension, InstallError> {
    let bytes = crx(name);
    let verified = verify(&bytes, &id(), &publisher()).expect("verifies");
    store.install(&verified)
}

/// A ZIP of `files` as a verified CRX would hand it over.
fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in files {
        writer.start_file(*name, options).expect("member");
        writer.write_all(bytes).expect("bytes");
    }
    writer.finish().expect("zip").into_inner()
}

fn verified(zip: &[u8]) -> Verified<'_> {
    let bytes = crx("valid.crx");
    let real = verify(&bytes, &id(), &publisher()).expect("verifies");
    Verified {
        id: real.id,
        public_key: real.public_key,
        zip,
    }
}

fn manifest(version: &str) -> Vec<u8> {
    format!(r#"{{"manifest_version":3,"name":"made","version":"{version}","content_scripts":[]}}"#)
        .into_bytes()
}

#[test]
fn an_install_unpacks_the_version_folder_with_the_key_in_the_manifest() {
    let (_dir, store) = setup();
    let installed = install(&store, "valid.crx").expect("installs");
    let folder = store.root().join(id()).join("1.0.0");
    assert_eq!(installed.folder, folder);
    assert_eq!(installed.version, "1.0.0");
    assert_eq!(installed.name, "rBrowser fixture: hello");
    assert!(folder.join("content.js").is_file());
    // Loaded unpacked, Chromium keeps the Web Store id only with the key in the manifest.
    let checked = check(&folder.to_string_lossy()).expect("a loadable folder");
    assert_eq!(checked.manifest.id.as_deref(), Some(id().as_str()));
    assert_eq!(store.installed(), vec![installed.clone()]);
    assert_eq!(store.installed_one(&id()), Some(installed));
    assert!(staging_is_empty(&store));
}

#[test]
fn a_zip_slip_member_is_refused_and_nothing_is_activated() {
    let (dir, store) = setup();
    let error = install(&store, "zip-slip.crx").expect_err("refused");
    assert!(matches!(error, InstallError::UnsafePath(_)), "{error:?}");
    assert!(store.installed().is_empty());
    assert!(
        !store.root().join(id()).exists(),
        "no folder of the extension"
    );
    assert!(!dir.path().join("evil.js").exists());
    assert!(!store.root().join("evil.js").exists());
    assert!(staging_is_empty(&store));
}

#[test]
fn a_manifest_v2_extension_is_refused() {
    let (_dir, store) = setup();
    let error = install(&store, "mv2.crx").expect_err("refused");
    assert!(matches!(error, InstallError::Manifest(_)), "{error:?}");
    assert!(store.installed().is_empty());
    assert!(!store.root().join(id()).exists());
}

#[test]
fn a_newer_version_replaces_the_old_folder_after_it_is_in_place() {
    let (_dir, store) = setup();
    install(&store, "valid.crx").expect("installs");
    let newer = install(&store, "valid-2.crx").expect("installs");
    assert_eq!(newer.version, "2.0.0");
    assert_eq!(
        store.installed_one(&id()).map(|i| i.version),
        Some("2.0.0".to_owned())
    );
    assert!(
        !store.root().join(id()).join("1.0.0").exists(),
        "the old version is gone"
    );
    assert!(
        store
            .root()
            .join(id())
            .join("2.0.0")
            .join("manifest.json")
            .is_file()
    );
}

#[test]
fn the_same_version_again_is_installed_once() {
    let (_dir, store) = setup();
    let first = install(&store, "valid.crx").expect("installs");
    let again = install(&store, "valid.crx").expect("installs");
    assert_eq!(first, again);
    assert_eq!(store.installed().len(), 1);
}

#[test]
fn too_many_files_or_too_many_bytes_are_refused() {
    let (_dir, store) = setup();
    let text = manifest("1.0.0");
    let files = [
        ("manifest.json", text.as_slice()),
        ("a.js", b"aaaa".as_slice()),
        ("b.js", b"bbbb".as_slice()),
    ];
    let zip = zip_of(&files);
    let error = store
        .install_limited(
            &verified(&zip),
            Limits {
                bytes: 1024,
                files: 2,
            },
        )
        .expect_err("refused");
    assert_eq!(error, InstallError::TooManyFiles(2));
    let error = store
        .install_limited(
            &verified(&zip),
            Limits {
                bytes: 70,
                files: 10,
            },
        )
        .expect_err("refused");
    assert_eq!(error, InstallError::TooLarge(70));
    assert!(store.installed().is_empty());
    assert!(!store.root().join(id()).exists());
    assert!(staging_is_empty(&store));
}

/// Two members that are one name in Windows' letter-case folding: the prompt would name one
/// manifest and the install keep the other, so the CRX is refused at both.
#[test]
fn members_whose_names_differ_in_letter_case_only_are_refused() {
    let (_dir, store) = setup();
    let zip = zip_of(&[
        ("manifest.json", manifest("1.0.0").as_slice()),
        ("Manifest.json", manifest("9.9.9").as_slice()),
    ]);
    let error = store.install(&verified(&zip)).expect_err("refused");
    assert_eq!(
        error,
        InstallError::NameCollision("Manifest.json".to_owned())
    );
    assert_eq!(error.code(), "extensions.install_name_collision");
    assert!(store.installed().is_empty());
    assert!(!store.root().join(id()).exists());
    assert!(staging_is_empty(&store));
    assert_eq!(peek_manifest(&zip).expect_err("refused"), error);
    let folders = zip_of(&[
        ("manifest.json", manifest("1.0.0").as_slice()),
        ("lib/", b"".as_slice()),
        ("lib/A.js", b"a".as_slice()),
        ("lib/a.js", b"a".as_slice()),
    ]);
    assert_eq!(
        store.install(&verified(&folders)).expect_err("refused"),
        InstallError::NameCollision("lib/a.js".to_owned())
    );
}

#[test]
fn a_manifest_whose_key_is_another_id_is_refused() {
    let (_dir, store) = setup();
    // The manifest names a key: it must be the developer's, else the ids would not match.
    let other = r#"{"manifest_version":3,"name":"x","version":"1.0.0","key":"AAAA"}"#;
    let zip = zip_of(&[("manifest.json", other.as_bytes())]);
    let error = store.install(&verified(&zip)).expect_err("refused");
    assert_eq!(error, InstallError::KeyMismatch);
}

#[test]
fn a_zip_without_a_manifest_is_refused() {
    let (_dir, store) = setup();
    let zip = zip_of(&[("content.js", b"x".as_slice())]);
    let error = store.install(&verified(&zip)).expect_err("refused");
    assert!(matches!(error, InstallError::Manifest(_)), "{error:?}");
    assert!(!store.root().join(id()).exists());
}

#[test]
fn nested_folders_are_unpacked_and_a_bad_id_is_refused() {
    let (_dir, store) = setup();
    let zip = zip_of(&[
        ("manifest.json", manifest("3.1.0").as_slice()),
        ("lib/deep/x.js", b"x".as_slice()),
    ]);
    let installed = store.install(&verified(&zip)).expect("installs");
    assert!(
        installed
            .folder
            .join("lib")
            .join("deep")
            .join("x.js")
            .is_file()
    );
    let mut bad = verified(&zip);
    bad.id = "../x".to_owned();
    assert_eq!(
        store.install(&bad).expect_err("refused"),
        InstallError::BadId
    );
}

#[test]
fn remove_deletes_the_extension_and_its_pointer() {
    let (_dir, store) = setup();
    install(&store, "valid.crx").expect("installs");
    assert!(store.remove(&id()).expect("removes"));
    assert!(store.installed().is_empty());
    assert!(!store.remove(&id()).expect("nothing left"));
    assert_eq!(
        store.remove("../x").expect_err("refused"),
        InstallError::BadId
    );
}

/// RB-080-30: a remove takes the staging leftovers of the id (a crashed install's) and the
/// retired versions with it, and nothing of another id.
#[test]
fn remove_takes_the_staging_leftovers_and_the_retired_versions_too() {
    let (_dir, store) = setup();
    // The pid 123 is made up: no process of the machine may decide the test (it is a live
    // daemon in some WSL distributions).
    let nobody = |_: u32| false;
    install(&store, "valid.crx").expect("installs");
    let retired = store.root().join(id()).join(".retired-1.0.0-x");
    std::fs::create_dir_all(&retired).expect("retired");
    let staging = store.root().join(STAGING);
    let mine = staging.join(format!("{}-123-0", id()));
    let other = staging.join(format!("{}-123-1", "b".repeat(32)));
    std::fs::create_dir_all(&mine).expect("staging");
    std::fs::create_dir_all(&other).expect("staging");
    assert!(store.remove_with(&id(), &nobody).expect("removes"));
    assert!(!store.root().join(id()).exists());
    assert!(!mine.exists(), "the id's staging leftover is gone");
    assert!(other.exists(), "another id's staging stays");
    std::fs::create_dir_all(&mine).expect("staging again");
    assert!(
        !store.remove_with(&id(), &nobody).expect("sweeps"),
        "a staging leftover alone is no installed extension"
    );
    assert!(!mine.exists());
}

/// RB-080-30: the store is shared by every rBrowser instance. A staging folder of the id whose
/// pid is alive is another window's running install: the remove is refused and nothing is
/// deleted, not even the dead leftovers. A dead pid's folder is a leftover and goes; another
/// id's live install does not block.
#[test]
fn remove_is_refused_while_another_window_installs_the_id() {
    let (_dir, store) = setup();
    install(&store, "valid.crx").expect("installs");
    let staging = store.root().join(STAGING);
    let live = staging.join(format!("{}-4242-0", id()));
    let dead = staging.join(format!("{}-99-3", id()));
    let other_live = staging.join(format!("{}-4242-1", "b".repeat(32)));
    for folder in [&live, &dead, &other_live] {
        std::fs::create_dir_all(folder).expect("staging");
    }
    let alive = |pid: u32| pid == 4242;
    assert_eq!(
        store.remove_with(&id(), &alive).expect_err("refused"),
        InstallError::InProgressElsewhere
    );
    assert_eq!(store.installed().len(), 1, "nothing was deleted");
    assert!(live.exists() && dead.exists(), "nothing was swept either");
    std::fs::remove_dir_all(&live).expect("the other window finished");
    assert!(store.remove_with(&id(), &alive).expect("removes"));
    assert!(!dead.exists(), "the dead pid's leftover went with it");
    assert!(
        other_live.exists(),
        "another id's live install is not touched"
    );
}

/// RB-080-30: `remove` judges the pid with the real process table: this process's own pid
/// counts as dead (no install of ours runs once the folder is taken), as does a pid no process
/// has; a name without a pid is nobody's and stays.
#[test]
fn remove_sweeps_this_process_and_dead_pids_and_leaves_odd_names() {
    let (_dir, store) = setup();
    install(&store, "valid.crx").expect("installs");
    let staging = store.root().join(STAGING);
    let own = staging.join(format!("{}-{}-7", id(), std::process::id()));
    let gone = staging.join(format!("{}-{}-0", id(), u32::MAX));
    let odd = staging.join(format!("{}-x", id()));
    for folder in [&own, &gone, &odd] {
        std::fs::create_dir_all(folder).expect("staging");
    }
    assert!(store.remove(&id()).expect("removes"));
    assert!(!own.exists() && !gone.exists());
    assert!(odd.exists());
}

/// RB-080-30: while an install of the id runs, the remove is refused and nothing is deleted.
#[test]
fn remove_is_refused_while_an_install_of_the_id_runs() {
    let (_dir, store) = setup();
    install(&store, "valid.crx").expect("installs");
    let held = InProgress::take(store.root().join(id())).expect("free");
    assert_eq!(
        store.remove(&id()).expect_err("refused"),
        InstallError::InProgress
    );
    assert_eq!(store.installed().len(), 1, "nothing was deleted");
    drop(held);
    assert!(
        store
            .remove(&id())
            .expect("removes once the install is done")
    );
}

#[test]
fn a_stale_pointer_or_a_strange_folder_is_not_listed() {
    let (_dir, store) = setup();
    let folder = store.root().join(id());
    std::fs::create_dir_all(folder.join("9.9.9")).expect("dir");
    std::fs::write(folder.join(POINTER), r#"{"version":"1.0.0"}"#).expect("pointer");
    std::fs::create_dir_all(store.root().join("not-an-id")).expect("dir");
    assert!(store.installed().is_empty());
}

/// A second install of an id while one runs is refused, so the two never share a staging
/// folder; another store's install of the same id is not held up.
#[test]
fn a_second_install_of_the_same_id_is_refused_while_one_runs() {
    let (_dir, store) = setup();
    let (_other_dir, other) = setup();
    let held = InProgress::take(store.root().join(id())).expect("free");
    assert!(InProgress::take(store.root().join(id())).is_none());
    let error = install(&store, "valid.crx").expect_err("refused");
    assert_eq!(error, InstallError::InProgress);
    assert_eq!(error.code(), "extensions.install_in_progress");
    assert!(store.installed().is_empty());
    assert!(staging_is_empty(&store));
    install(&other, "valid.crx").expect("another store installs");
    drop(held);
    install(&store, "valid.crx").expect("installs once the first is done");
    assert!(
        InProgress::take(store.root().join(id())).is_some(),
        "an install leaves the id free"
    );
}

#[test]
fn every_attempt_stages_under_a_name_of_its_own() {
    let first = attempt_name(&id());
    let second = attempt_name(&id());
    assert_ne!(first, second);
    let prefix = format!("{}-{}-", id(), std::process::id());
    assert!(first.starts_with(&prefix), "{first}");
    assert!(second.starts_with(&prefix), "{second}");
}

#[test]
fn every_install_error_has_a_code() {
    for error in [
        InstallError::BadId,
        InstallError::InProgress,
        InstallError::Io(String::new()),
        InstallError::Unpack(String::new()),
        InstallError::UnsafePath(String::new()),
        InstallError::NameCollision(String::new()),
        InstallError::TooManyFiles(1),
        InstallError::TooLarge(1),
        InstallError::KeyMismatch,
        InstallError::Manifest(crate::extensions::folder::FolderError::NoManifest),
    ] {
        assert!(error.code().starts_with("extensions."), "{error:?}");
    }
}

#[test]
fn the_manifest_is_read_out_of_the_zip_without_unpacking() {
    let zip = zip_of(&[("manifest.json", manifest("4.2.0").as_slice())]);
    let found = peek_manifest(&zip).expect("reads");
    assert_eq!(
        (found.name.as_str(), found.version.as_str()),
        ("made", "4.2.0")
    );
    let error = peek_manifest(&zip_of(&[("x.js", b"x".as_slice())])).expect_err("refused");
    assert_eq!(error, InstallError::Manifest(FolderError::NoManifest));
    assert!(matches!(
        peek_manifest(b"not a zip"),
        Err(InstallError::Unpack(_))
    ));
}

fn staging_is_empty(store: &ExtensionStore) -> bool {
    std::fs::read_dir(store.root().join(STAGING))
        .map(|entries| entries.flatten().all(|entry| entry.path().is_file()))
        .unwrap_or(true)
}
