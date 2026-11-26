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

/// RB-099-13: `tools.adb.install` without the person's acceptance asks the shell to show
/// Google's terms - version, size, source host, licence and the terms page - and downloads
/// nothing; the download follows only `{ "accepted": true }`.
#[test]
fn without_acceptance_the_shell_is_asked_for_the_terms_and_nothing_is_downloaded() {
    let bytes = archive();
    for args in [serde_json::Value::Null, serde_json::json!({})] {
        let Ok(Step::AskTerms(request)) = step(&args, Ok(entry(&bytes))) else {
            panic!("the terms are asked for");
        };
        let CoreToShell::UiCommand { id, args } = *request else {
            panic!("the shell's terms bar");
        };
        assert_eq!(id, crate::engines::android::commands::INSTALL);
        let args = args.expect("what the terms bar shows");
        assert_eq!(args["terms"], TERMS_URL);
        assert_eq!(args["version"], "37.0.1");
        assert_eq!(args["from"], "dl.example.test");
        assert_eq!(args["license"], ANDROID_SDK_LICENSE);
        assert!(args["size"].is_string());
    }
    assert!(matches!(
        step(&serde_json::json!({ "accepted": false }), Ok(entry(&bytes))),
        Ok(Step::AskTerms(_))
    ));
}

#[test]
fn the_acceptance_starts_the_download_of_the_pinned_entry() {
    let bytes = archive();
    let Ok(Step::Download(chosen)) =
        step(&serde_json::json!({ "accepted": true }), Ok(entry(&bytes)))
    else {
        panic!("the download follows the acceptance");
    };
    assert_eq!(*chosen, entry(&bytes));
}

#[test]
fn an_acceptance_that_is_no_boolean_or_an_unknown_argument_is_refused() {
    let bytes = archive();
    for args in [
        serde_json::json!({ "accepted": "yes" }),
        serde_json::json!({ "accepted": 1 }),
        serde_json::json!({ "accept": true }),
    ] {
        assert!(step(&args, Ok(entry(&bytes))).is_err(), "{args}");
    }
}

/// No build pinned for this platform: neither terms nor a download, the failure notice.
#[test]
fn without_a_pinned_build_the_failure_is_told_and_nothing_is_asked() {
    let no_build = || ToolError::NoBuild {
        tool: crate::tools::ADB.to_owned(),
        platform: "linux".to_owned(),
    };
    for args in [
        serde_json::Value::Null,
        serde_json::json!({ "accepted": true }),
    ] {
        let Ok(Step::Refused(notice)) = step(&args, Err(no_build())) else {
            panic!("no terms, no download");
        };
        let CoreToShell::Notification(notice) = *notice else {
            panic!("the failure notice");
        };
        assert_eq!(notice.message_key, FAILED_KEY);
        assert_eq!(notice.level, NotificationLevel::Error);
    }
}

/// The terms page is Google's, over https.
#[test]
fn the_terms_page_is_googles_platform_tools_page() {
    assert!(TERMS_URL.starts_with("https://developer.android.com/"));
}
