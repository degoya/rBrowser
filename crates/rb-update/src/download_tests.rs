//! The hash check and the size cap of the download, against a fake HTTP source.

use super::*;
use crate::{MemoryFetcher, sign::hex_sha256};

const URL: &str =
    "https://github.com/degoya/rBrowser/releases/download/v1.1.0/rbrowser-1.1.0-windows-x64.zip";

fn artifact_for(body: &[u8]) -> Artifact {
    Artifact {
        platform: "windows".to_owned(),
        arch: "x86_64".to_owned(),
        kind: "zip".to_owned(),
        url: URL.to_owned(),
        sha256: hex_sha256(body),
        size: body.len() as u64,
    }
}

fn body() -> Vec<u8> {
    (0..200u8).collect()
}

fn files(directory: &Path) -> usize {
    fs::read_dir(directory).expect("list").count()
}

fn fetch(
    fetcher: &MemoryFetcher,
    artifact: &Artifact,
    directory: &Path,
) -> Result<PathBuf, UpdateError> {
    download_verified(fetcher, artifact, directory, &Progress::default())
}

#[test]
fn a_matching_download_is_kept_under_its_name() {
    let directory = tempfile::tempdir().expect("tempdir");
    let fetcher = MemoryFetcher::new();
    fetcher.serve(URL, body());
    let progress = Progress::default();
    let path = download_verified(
        &fetcher,
        &artifact_for(&body()),
        directory.path(),
        &progress,
    )
    .expect("download");
    assert_eq!(
        path,
        directory.path().join("rbrowser-1.1.0-windows-x64.zip")
    );
    assert_eq!(fs::read(&path).expect("read"), body());
    assert_eq!(progress.received(), 200);
    assert_eq!(files(directory.path()), 1, "no .partial stays");
}

#[test]
fn a_wrong_sha256_leaves_no_file() {
    let directory = tempfile::tempdir().expect("tempdir");
    let fetcher = MemoryFetcher::new();
    let mut tampered = body();
    tampered[10] ^= 0xff;
    fetcher.serve(URL, tampered);
    let error = fetch(&fetcher, &artifact_for(&body()), directory.path()).expect_err("digest");
    assert_eq!(error.code(), "update.digest_mismatch");
    assert_eq!(files(directory.path()), 0);
}

/// The size cap: a server that sends more than the manifest says is cut off, one that sends less
/// is refused, and neither leaves a file.
#[test]
fn a_body_longer_or_shorter_than_the_manifest_is_refused() {
    let directory = tempfile::tempdir().expect("tempdir");
    let fetcher = MemoryFetcher::new();
    let mut longer = body();
    longer.extend_from_slice(&[0; 50]);
    fetcher.serve(URL, longer);
    let error = fetch(&fetcher, &artifact_for(&body()), directory.path()).expect_err("longer");
    assert_eq!(error.code(), "update.size_mismatch");
    fetcher.serve(URL, body()[..150].to_vec());
    let error = fetch(&fetcher, &artifact_for(&body()), directory.path()).expect_err("shorter");
    assert_eq!(error.code(), "update.size_mismatch");
    assert_eq!(files(directory.path()), 0);
}

/// A source that declares no length and streams on: cut off at the manifest's size.
#[test]
fn an_endless_body_without_a_length_is_cut_off_at_the_size() {
    struct Endless;
    impl std::io::Read for Endless {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            buffer.fill(1);
            Ok(buffer.len())
        }
    }
    struct Source;
    impl Fetcher for Source {
        fn fetch(&self, _: &url::Url, _: u64) -> anyhow::Result<Option<Vec<u8>>> {
            Ok(None)
        }
        fn open(&self, _: &url::Url) -> anyhow::Result<crate::fetch::Download> {
            Ok(crate::fetch::Download {
                length: None,
                body: Box::new(Endless),
            })
        }
    }
    let directory = tempfile::tempdir().expect("tempdir");
    let error = download_verified(
        &Source,
        &artifact_for(&body()),
        directory.path(),
        &Progress::default(),
    )
    .expect_err("endless");
    assert_eq!(error.code(), "update.size_mismatch");
    assert_eq!(files(directory.path()), 0);
}

#[test]
fn an_artifact_that_is_not_https_is_never_fetched() {
    let directory = tempfile::tempdir().expect("tempdir");
    let fetcher = MemoryFetcher::new();
    let mut artifact = artifact_for(&body());
    artifact.url = URL.replace("https://", "http://");
    let error = fetch(&fetcher, &artifact, directory.path()).expect_err("http");
    assert_eq!(error.code(), "update.invalid");
    assert!(fetcher.requests().is_empty());
}

#[test]
fn a_missing_file_and_a_cancel_are_refused_without_a_file() {
    let directory = tempfile::tempdir().expect("tempdir");
    let fetcher = MemoryFetcher::new();
    let error = fetch(&fetcher, &artifact_for(&body()), directory.path()).expect_err("404");
    assert_eq!(error.code(), "update.download_failed");
    fetcher.serve(URL, body());
    let progress = Progress::default();
    progress.cancel();
    assert!(
        download_verified(
            &fetcher,
            &artifact_for(&body()),
            directory.path(),
            &progress
        )
        .is_err()
    );
    assert_eq!(files(directory.path()), 0);
}

#[test]
fn space_for_the_archive_and_its_unpacked_copy_is_required() {
    let artifact = artifact_for(&body());
    assert!(check_space(&artifact, None).is_ok());
    assert!(check_space(&artifact, Some(400)).is_ok());
    let error = check_space(&artifact, Some(399)).expect_err("space");
    assert_eq!(error.code(), "update.no_space");
}

#[test]
fn a_file_name_keeps_only_safe_characters() {
    let url = url::Url::parse("https://example.test/a/..%2F..%2Fevil$.zip").expect("url");
    assert_eq!(file_name(&url), "2F..2Fevil.zip");
    let url = url::Url::parse("https://example.test/").expect("url");
    assert_eq!(file_name(&url), "rbrowser-update.bin");
}
