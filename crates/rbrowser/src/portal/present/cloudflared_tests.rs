use super::*;
use crate::tools::fetch::MemoryFetcher;
use crate::tools::manifest::{Archive, WINDOWS_X64};
use crate::tools::store::sha256_hex;

const URL: &str = "https://github.com/cloudflare/cloudflared/releases/download/x/cf.exe";

fn entry(bytes: &[u8]) -> ToolEntry {
    ToolEntry {
        name: CLOUDFLARED.to_owned(),
        platform: WINDOWS_X64.to_owned(),
        version: "2026.11.1".to_owned(),
        url: URL.to_owned(),
        sha256: sha256_hex(bytes),
        size: u64::try_from(bytes.len()).expect("size"),
        archive: Archive::Exe,
        executable: "cloudflared.exe".to_owned(),
        members: vec!["cloudflared.exe".to_owned()],
        license: "Apache-2.0".to_owned(),
        source_url: "https://github.com/cloudflare/cloudflared/archive/x.tar.gz".to_owned(),
        upstream_url: None,
        upstream_source_url: None,
    }
}

#[test]
fn the_download_names_size_licence_and_host() {
    let download = Download::of(&ToolEntry {
        size: 55_483_112,
        ..entry(b"x")
    });
    assert_eq!(download.version, "2026.11.1");
    assert_eq!(download.size_mb, "55.5");
    assert_eq!(download.license, "Apache-2.0");
    assert_eq!(download.from, "github.com");
}

#[test]
fn nothing_is_found_before_a_download() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ToolStore::new(dir.path().join("tools"));
    assert_eq!(locate(None, Some(&store), Some(&entry(b"MZ"))), None);
    assert_eq!(locate(None, None, None), None);
}

#[test]
fn the_managed_build_is_found_after_its_install() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ToolStore::new(dir.path().join("tools"));
    let fetcher = MemoryFetcher::default();
    let bytes = b"MZ cloudflared".to_vec();
    fetcher.serve(URL, bytes.clone());
    let entry = entry(&bytes);
    let installed = install(&store, &fetcher, &entry).expect("installed");
    assert_eq!(
        locate(None, Some(&store), Some(&entry)),
        Some(installed.executable)
    );
    assert_eq!(
        fetcher.requests(),
        [URL],
        "one download, only after the call"
    );
}

/// The person's own program wins when it is an absolute path to a file; anything else falls
/// back to the managed one.
#[test]
fn the_persons_own_program_is_taken_when_it_is_an_absolute_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let own = dir.path().join("my-cloudflared");
    std::fs::write(&own, b"x").expect("file");
    let own_text = own.display().to_string();
    assert_eq!(locate(Some(&own_text), None, None), Some(own.clone()));
    assert_eq!(locate(Some("cloudflared"), None, None), None, "relative");
    let missing = dir.path().join("missing").display().to_string();
    assert_eq!(locate(Some(&missing), None, None), None);
    assert_eq!(locate(Some("  "), None, None), None);
}
