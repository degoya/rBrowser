use super::*;
use crate::tools::ToolError;
use crate::tools::fetch::MemoryFetcher;
use crate::tools::manifest::{Archive, ToolEntry, WINDOWS_X64};

const URL: &str = "https://example.test/ffmpeg.zip";

/// A ZIP laid out as the BtbN builds are: a top folder, `bin/` with the programs, the licence.
fn archive(executable: &[u8]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in [
        ("ffmpeg-n9/bin/ffmpeg.exe", executable),
        ("ffmpeg-n9/bin/ffplay.exe", b"not taken".as_slice()),
        ("ffmpeg-n9/LICENSE.txt", b"LGPL".as_slice()),
    ] {
        writer.start_file(name, options).expect("member");
        writer.write_all(bytes).expect("bytes");
    }
    writer.finish().expect("zip").into_inner()
}

fn entry_for(version: &str, bytes: &[u8]) -> ToolEntry {
    ToolEntry {
        name: "ffmpeg".to_owned(),
        platform: WINDOWS_X64.to_owned(),
        version: version.to_owned(),
        url: URL.to_owned(),
        sha256: sha256_hex(bytes),
        size: u64::try_from(bytes.len()).expect("size"),
        archive: Archive::Zip,
        executable: "ffmpeg.exe".to_owned(),
        members: vec!["ffmpeg.exe".to_owned(), "LICENSE.txt".to_owned()],
        license: "LGPL-3.0-or-later".to_owned(),
        source_url: "https://example.test/source.tar.gz".to_owned(),
        upstream_url: None,
        upstream_source_url: None,
    }
}

fn setup() -> (tempfile::TempDir, ToolStore, MemoryFetcher) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ToolStore::new(dir.path().join("tools"));
    (dir, store, MemoryFetcher::default())
}

/// No staging folder is left (the lock files stay).
fn staging_is_empty(store: &ToolStore) -> bool {
    std::fs::read_dir(store.root().join(STAGING))
        .map(|entries| entries.flatten().all(|entry| entry.path().is_file()))
        .unwrap_or(true)
}

#[test]
fn an_install_unpacks_the_members_and_points_at_the_version() {
    let (_dir, store, fetcher) = setup();
    let bytes = archive(b"MZ ffmpeg");
    fetcher.serve(URL, bytes.clone());
    let entry = entry_for("n9.0.2", &bytes);
    let installed = store.install(&fetcher, &entry).expect("installed");
    let version_dir = store.root().join("ffmpeg").join("n9.0.2");
    assert_eq!(installed.version, "n9.0.2");
    assert_eq!(installed.executable, version_dir.join("ffmpeg.exe"));
    assert_eq!(
        std::fs::read(version_dir.join("ffmpeg.exe")).expect("exe"),
        b"MZ ffmpeg"
    );
    assert!(version_dir.join("LICENSE.txt").is_file());
    assert!(
        !version_dir.join("ffplay.exe").exists(),
        "only the members are taken"
    );
    assert_eq!(store.installed(&entry), Some(installed));
    assert!(staging_is_empty(&store));
}

/// Acceptance: an archive with a wrong SHA-256 activates nothing, leaves the staging empty, and
/// the error names the expected and the received hash.
#[test]
fn a_wrong_hash_activates_nothing_and_names_both_hashes() {
    let (_dir, store, fetcher) = setup();
    let served = archive(b"tampered");
    fetcher.serve(URL, served.clone());
    let mut entry = entry_for("n9.0.2", &archive(b"MZ ffmpeg"));
    entry.size = u64::try_from(served.len()).expect("size");
    let error = store.install(&fetcher, &entry).expect_err("refused");
    let ToolError::HashMismatch {
        expected, actual, ..
    } = &error
    else {
        panic!("not a hash mismatch: {error}");
    };
    assert_eq!(expected, &entry.sha256);
    assert_eq!(actual, &sha256_hex(&served));
    let text = error.to_string();
    assert!(
        text.contains(expected.as_str()) && text.contains(actual.as_str()),
        "{text}"
    );
    assert!(!store.root().join("ffmpeg").join("n9.0.2").exists());
    assert_eq!(store.installed(&entry), None);
    assert!(staging_is_empty(&store));
}

#[test]
fn a_body_of_another_size_is_refused() {
    let (_dir, store, fetcher) = setup();
    let bytes = archive(b"MZ ffmpeg");
    let mut longer = bytes.clone();
    longer.extend_from_slice(b"appended");
    fetcher.serve(URL, longer);
    let entry = entry_for("n9.0.2", &bytes);
    let error = store.install(&fetcher, &entry).expect_err("refused");
    assert!(matches!(error, ToolError::Download { .. }), "{error}");
    assert_eq!(store.installed(&entry), None);
    assert!(staging_is_empty(&store));
}

#[test]
fn an_archive_without_the_executable_activates_nothing() {
    let (_dir, store, fetcher) = setup();
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    writer
        .start_file("LICENSE.txt", zip::write::SimpleFileOptions::default())
        .expect("member");
    writer.write_all(b"LGPL").expect("bytes");
    let bytes = writer.finish().expect("zip").into_inner();
    fetcher.serve(URL, bytes.clone());
    let entry = entry_for("n9.0.2", &bytes);
    assert!(store.install(&fetcher, &entry).is_err());
    assert_eq!(store.installed(&entry), None);
    assert!(staging_is_empty(&store));
}

/// A failed update keeps the version installed before (the roll-back); a good one replaces it
/// and removes the old folder.
#[test]
fn a_failed_update_keeps_the_old_version_and_a_good_one_prunes_it() {
    let (_dir, store, fetcher) = setup();
    let old = archive(b"old");
    fetcher.serve(URL, old.clone());
    let old_entry = entry_for("n8", &old);
    store.install(&fetcher, &old_entry).expect("old");

    let new = archive(b"new");
    let mut broken = entry_for("n9", &new);
    broken.sha256 = sha256_hex(b"something else");
    fetcher.serve(URL, new.clone());
    assert!(store.install(&fetcher, &broken).is_err());
    let still = store
        .installed(&old_entry)
        .expect("the old one stays active");
    assert_eq!(still.version, "n8");

    let new_entry = entry_for("n9", &new);
    store.install(&fetcher, &new_entry).expect("new");
    assert_eq!(
        store
            .installed(&new_entry)
            .map(|installed| installed.version),
        Some("n9".to_owned())
    );
    assert!(!store.root().join("ffmpeg").join("n8").exists(), "pruned");
}

#[test]
fn a_pointer_that_leaves_the_tool_folder_reads_as_nothing_installed() {
    let (_dir, store, _fetcher) = setup();
    let entry = entry_for("n9", b"x");
    let tool = store.root().join("ffmpeg");
    std::fs::create_dir_all(&tool).expect("dir");
    for version in ["../..", "/etc", ".hidden", ""] {
        let pointer = serde_json::json!({ "version": version }).to_string();
        std::fs::write(tool.join(POINTER), pointer).expect("pointer");
        assert_eq!(store.installed(&entry), None, "{version:?}");
    }
}

#[test]
fn remove_deletes_the_tool_and_says_whether_there_was_one() {
    let (_dir, store, fetcher) = setup();
    let bytes = archive(b"MZ");
    fetcher.serve(URL, bytes.clone());
    let entry = entry_for("n9", &bytes);
    store.install(&fetcher, &entry).expect("installed");
    assert!(store.remove("ffmpeg").expect("removed"));
    assert_eq!(store.installed(&entry), None);
    assert!(!store.remove("ffmpeg").expect("nothing left"));
}

/// Charge 9 review L2: installing the version that is active downloads nothing and leaves the
/// folder in use alone.
#[test]
fn installing_the_installed_version_is_a_no_op() {
    let (_dir, store, fetcher) = setup();
    let bytes = archive(b"MZ ffmpeg");
    fetcher.serve(URL, bytes.clone());
    let entry = entry_for("n9", &bytes);
    let first = store.install(&fetcher, &entry).expect("installed");
    let marker = first.dir.join("in-use");
    std::fs::write(&marker, b"x").expect("marker");
    let again = store.install(&fetcher, &entry).expect("installed again");
    assert_eq!(again, first);
    assert_eq!(fetcher.requests().len(), 1, "no second download");
    assert!(marker.is_file(), "the active folder is not replaced");
}

/// A version folder that is there but not active (no pointer) is moved aside before the new one
/// takes its place, then removed - never deleted first.
#[test]
fn a_leftover_version_folder_is_replaced_by_rename() {
    let (_dir, store, fetcher) = setup();
    let leftover = store.root().join("ffmpeg").join("n9");
    std::fs::create_dir_all(&leftover).expect("dir");
    std::fs::write(leftover.join("ffmpeg.exe"), b"stale").expect("stale");
    let bytes = archive(b"MZ fresh");
    fetcher.serve(URL, bytes.clone());
    let entry = entry_for("n9", &bytes);
    let installed = store.install(&fetcher, &entry).expect("installed");
    assert_eq!(
        std::fs::read(&installed.executable).expect("exe"),
        b"MZ fresh"
    );
    let names: Vec<String> = std::fs::read_dir(store.root().join("ffmpeg"))
        .expect("list")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names.len(), 2, "the version and the pointer: {names:?}");
}

/// Another process installing the same tool holds its lock: this one downloads nothing.
#[test]
fn a_second_install_at_the_same_time_is_refused() {
    let (_dir, store, fetcher) = setup();
    let bytes = archive(b"MZ");
    fetcher.serve(URL, bytes.clone());
    let entry = entry_for("n9", &bytes);
    let staging = store.root().join(STAGING);
    std::fs::create_dir_all(&staging).expect("dir");
    let held = std::fs::File::create(staging.join("ffmpeg.lock")).expect("lock file");
    held.try_lock().expect("locked");
    let error = store.install(&fetcher, &entry).expect_err("busy");
    assert!(matches!(error, ToolError::Busy { .. }), "{error}");
    assert!(fetcher.requests().is_empty());
    drop(held);
    store
        .install(&fetcher, &entry)
        .expect("installed once the lock is free");
}

/// What a crashed install left in the staging folder is removed by the next one.
#[test]
fn stale_staging_is_cleaned() {
    let (_dir, store, fetcher) = setup();
    let staging = store.root().join(STAGING);
    for stale in ["ffmpeg", "ffmpeg-4242"] {
        std::fs::create_dir_all(staging.join(stale).join("n9")).expect("stale");
    }
    std::fs::create_dir_all(staging.join("other-1")).expect("another tool's");
    let bytes = archive(b"MZ");
    fetcher.serve(URL, bytes.clone());
    store
        .install(&fetcher, &entry_for("n9", &bytes))
        .expect("installed");
    assert!(!staging.join("ffmpeg").exists());
    assert!(!staging.join("ffmpeg-4242").exists());
    assert!(
        staging.join("other-1").exists(),
        "another tool's staging stays"
    );
}

#[test]
fn the_digest_is_lowercase_hex() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

/// RB-096-05: a bare program (cloudflared's release asset) is checked like an archive and
/// becomes the executable of its version folder.
#[test]
fn a_bare_program_is_installed_as_its_executable() {
    let (_dir, store, fetcher) = setup();
    let bytes = b"MZ cloudflared".to_vec();
    let url = "https://example.test/cloudflared-windows-amd64.exe";
    fetcher.serve(url, bytes.clone());
    let entry = ToolEntry {
        name: "cloudflared".to_owned(),
        url: url.to_owned(),
        archive: Archive::Exe,
        executable: "cloudflared.exe".to_owned(),
        members: vec!["cloudflared.exe".to_owned()],
        license: "Apache-2.0".to_owned(),
        ..entry_for("2026.11.1", &bytes)
    };
    let installed = store.install(&fetcher, &entry).expect("installed");
    let version_dir = store.root().join("cloudflared").join("2026.11.1");
    assert_eq!(installed.executable, version_dir.join("cloudflared.exe"));
    assert_eq!(
        std::fs::read(&installed.executable).expect("program"),
        bytes
    );
    assert_eq!(store.installed(&entry), Some(installed));
    assert!(staging_is_empty(&store));
}

/// A bare program with another hash activates nothing, as an archive does.
#[test]
fn a_bare_program_with_another_hash_is_refused() {
    let (_dir, store, fetcher) = setup();
    let url = "https://example.test/cloudflared-windows-amd64.exe";
    fetcher.serve(url, b"MZ tampered!!".to_vec());
    let entry = ToolEntry {
        name: "cloudflared".to_owned(),
        url: url.to_owned(),
        archive: Archive::Exe,
        executable: "cloudflared.exe".to_owned(),
        members: vec!["cloudflared.exe".to_owned()],
        license: "Apache-2.0".to_owned(),
        ..entry_for("2026.11.1", b"MZ cloudflare")
    };
    assert!(matches!(
        store.install(&fetcher, &entry),
        Err(ToolError::HashMismatch { .. })
    ));
    assert_eq!(store.installed(&entry), None);
    assert!(staging_is_empty(&store));
}
