use std::io::Write as _;
use std::sync::Mutex;

use rb_config::{ToolsSettings, VideoConfig};

use super::*;
use crate::tools::fetch::MemoryFetcher;
use crate::tools::manifest::{Archive, WINDOWS_X64};
use crate::tools::store::sha256_hex;

const URL: &str = "https://packages.example.test/ffmpeg.zip";

fn archive() -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for name in ["build/bin/ffmpeg.exe", "build/LICENSE.txt"] {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .expect("member");
        writer.write_all(b"bytes").expect("bytes");
    }
    writer.finish().expect("zip").into_inner()
}

fn entry(bytes: &[u8]) -> ToolEntry {
    ToolEntry {
        name: "ffmpeg".to_owned(),
        platform: WINDOWS_X64.to_owned(),
        version: "n9.0.2".to_owned(),
        url: URL.to_owned(),
        sha256: sha256_hex(bytes),
        size: u64::try_from(bytes.len()).expect("size"),
        archive: Archive::Zip,
        executable: "ffmpeg.exe".to_owned(),
        members: vec!["ffmpeg.exe".to_owned(), "LICENSE.txt".to_owned()],
        license: "LGPL-3.0-or-later".to_owned(),
        source_url: "https://packages.example.test/ffmpeg-source.tar.gz".to_owned(),
        upstream_url: None,
        upstream_source_url: None,
    }
}

fn keys(sent: &Mutex<Vec<CoreToShell>>) -> Vec<String> {
    lock(sent)
        .iter()
        .filter_map(|message| match message {
            CoreToShell::Notification(notification) => Some(notification.message_key.clone()),
            _ => None,
        })
        .collect()
}

fn settings(path: Option<&Path>) -> VideoSettings {
    VideoSettings::resolve(
        &VideoConfig::default(),
        &VideoConfig::default(),
        &ToolsSettings {
            ffmpeg_path: path.map(|path| path.display().to_string()),
        },
    )
}

/// The consent notice names size, source host, licence and the source code, and asking for
/// it reaches no network.
#[test]
fn the_consent_notice_says_what_would_be_downloaded() {
    let fetcher = MemoryFetcher::default();
    let CoreToShell::Notification(notification) = needed_notice(&entry(&vec![0; 2_500_000])) else {
        panic!("not a notification");
    };
    assert_eq!(notification.message_key, NEEDED_KEY);
    assert_eq!(notification.args["size"], "2.5");
    assert_eq!(notification.args["license"], "LGPL-3.0-or-later");
    assert_eq!(
        notification.args["source"],
        "https://packages.example.test/ffmpeg-source.tar.gz"
    );
    assert_eq!(notification.args["from"], "packages.example.test");
    assert_eq!(notification.settings_key.as_deref(), Some(SETTINGS_KEY));
    assert!(fetcher.requests().is_empty());
}

#[test]
fn an_install_says_what_it_downloads_and_where_it_went() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ToolStore::new(dir.path().join("tools"));
    let fetcher = MemoryFetcher::default();
    let bytes = archive();
    fetcher.serve(URL, bytes.clone());
    let sent = Mutex::new(Vec::new());
    let installed = install(&store, &fetcher, &entry(&bytes), &|message| {
        lock(&sent).push(message);
    })
    .expect("installed");
    assert_eq!(keys(&sent), [DOWNLOADING_KEY, INSTALLED_KEY]);
    assert_eq!(fetcher.requests(), [URL]);
    let found = managed(&store, &entry(&bytes)).expect("managed ffmpeg");
    assert_eq!(found.path, installed.executable);
    assert_eq!(found.source, Source::Managed);
    assert_eq!(found.version.as_deref(), Some("n9.0.2"));
}

#[test]
fn a_failed_install_is_told_and_a_later_one_may_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ToolStore::new(dir.path().join("tools"));
    let fetcher = MemoryFetcher::default();
    let bytes = archive();
    let mut wrong = entry(&bytes);
    wrong.sha256 = sha256_hex(b"other");
    fetcher.serve(URL, bytes.clone());
    let sent = Mutex::new(Vec::new());
    let notify = |message| lock(&sent).push(message);
    assert!(install(&store, &fetcher, &wrong, &notify).is_err());
    assert_eq!(keys(&sent), [DOWNLOADING_KEY, INSTALL_FAILED_KEY]);
    assert!(managed(&store, &wrong).is_none());
    install(&store, &fetcher, &entry(&bytes), &notify).expect("the next one runs");
}

#[test]
fn the_users_own_ffmpeg_comes_first_when_it_exists() {
    let dir = tempfile::tempdir().expect("tempdir");
    let own = dir.path().join("my-ffmpeg");
    std::fs::write(&own, b"program").expect("file");
    let found = locate(&settings(Some(&own)), None).expect("found");
    assert_eq!(found.source, Source::Custom);
    assert_eq!(found.path, own);
    let missing = dir.path().join("missing");
    assert_eq!(
        locate(&settings(Some(&missing)), None),
        None,
        "neither the own nor a managed one"
    );
}
