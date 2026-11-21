use std::io::Write as _;
use std::sync::Mutex;

use rb_ipc::{CoreToShell, NotificationLevel};

use super::*;
use crate::tools::fetch::MemoryFetcher;
use crate::tools::manifest::{ANDROID_SDK_LICENSE, Archive, WINDOWS_X64};
use crate::tools::store::sha256_hex;

const URL: &str = "https://dl.example.test/platform-tools_r37.0.1-win.zip";

fn archive() -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in [
        ("platform-tools/adb.exe", b"MZ adb".as_slice()),
        ("platform-tools/AdbWinApi.dll", b"MZ dll".as_slice()),
        ("platform-tools/fastboot.exe", b"not taken".as_slice()),
        ("platform-tools/NOTICE.txt", b"notice".as_slice()),
    ] {
        writer.start_file(name, options).expect("member");
        writer.write_all(bytes).expect("bytes");
    }
    writer.finish().expect("zip").into_inner()
}

fn entry(bytes: &[u8]) -> ToolEntry {
    ToolEntry {
        name: crate::tools::ADB.to_owned(),
        platform: WINDOWS_X64.to_owned(),
        version: "37.0.1".to_owned(),
        url: URL.to_owned(),
        sha256: sha256_hex(bytes),
        size: u64::try_from(bytes.len()).expect("size"),
        archive: Archive::Zip,
        executable: "adb.exe".to_owned(),
        members: vec![
            "adb.exe".to_owned(),
            "AdbWinApi.dll".to_owned(),
            "NOTICE.txt".to_owned(),
        ],
        license: ANDROID_SDK_LICENSE.to_owned(),
        source_url: "https://source.example.test/adb".to_owned(),
        upstream_url: None,
        upstream_source_url: None,
    }
}

fn keys(sent: &Mutex<Vec<CoreToShell>>) -> Vec<(String, NotificationLevel)> {
    crate::cdp::lock(sent)
        .iter()
        .filter_map(|message| match message {
            CoreToShell::Notification(notification) => {
                Some((notification.message_key.clone(), notification.level))
            }
            _ => None,
        })
        .collect()
}

/// The consent notice names what would be downloaded, from where and under which licence, and
/// links the adb path setting; nothing touches the network for it.
#[test]
fn the_consent_notice_names_size_source_and_licence() {
    let bytes = archive();
    let CoreToShell::Notification(notice) = needed_notice(&entry(&bytes), "no adb was found")
    else {
        panic!("a notification");
    };
    assert_eq!(notice.message_key, NEEDED_KEY);
    assert_eq!(notice.settings_key.as_deref(), Some(PATH_SETTING));
    assert_eq!(notice.args["from"], "dl.example.test");
    assert_eq!(notice.args["license"], ANDROID_SDK_LICENSE);
    assert_eq!(notice.args["version"], "37.0.1");
    assert_eq!(notice.args["reason"], "no adb was found");
}

#[test]
fn the_download_installs_adb_with_its_libraries_and_says_so() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ToolStore::new(dir.path().join("tools"));
    let fetcher = MemoryFetcher::default();
    let bytes = archive();
    fetcher.serve(URL, bytes.clone());
    let sent = Mutex::new(Vec::new());
    let installed = install(&store, &fetcher, &entry(&bytes), &|message| {
        crate::cdp::lock(&sent).push(message);
    })
    .expect("installed");
    assert!(installed.executable.ends_with("adb.exe"));
    assert!(installed.dir.join("AdbWinApi.dll").is_file());
    assert!(
        !installed.dir.join("fastboot.exe").exists(),
        "only the members"
    );
    assert_eq!(
        managed(Some(&store), &entry(&bytes)),
        Some(installed.executable.clone())
    );
    assert_eq!(
        keys(&sent),
        [
            (DOWNLOADING_KEY.to_owned(), NotificationLevel::Info),
            (INSTALLED_KEY.to_owned(), NotificationLevel::Info),
        ]
    );
}

#[test]
fn a_wrong_download_installs_nothing_and_says_why() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ToolStore::new(dir.path().join("tools"));
    let fetcher = MemoryFetcher::default();
    let bytes = archive();
    fetcher.serve(URL, b"something else".to_vec());
    let sent = Mutex::new(Vec::new());
    assert!(
        install(&store, &fetcher, &entry(&bytes), &|message| {
            crate::cdp::lock(&sent).push(message);
        })
        .is_err()
    );
    assert_eq!(managed(Some(&store), &entry(&bytes)), None);
    assert_eq!(
        keys(&sent).last(),
        Some(&(FAILED_KEY.to_owned(), NotificationLevel::Error))
    );
}
