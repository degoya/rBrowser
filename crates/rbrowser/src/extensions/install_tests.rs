use super::*;
use crate::extensions::update_check::update_check_url;
use crate::extensions::webstore::{MemoryHops, update_url};

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/extensions/crx"
);
const BLOB: &str = "https://clients2.googleusercontent.com/crx/blobs/x/extension.crx";
const CHROMIUM: &str = "156.0.0.0";

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

fn serve(hops: &MemoryHops, name: &str) {
    let bytes = std::fs::read(format!("{FIXTURES}/{name}")).expect("fixture");
    let length = u64::try_from(bytes.len()).expect("fits");
    hops.redirect(&update_url(&id(), CHROMIUM), BLOB)
        .body(BLOB, &bytes, Some(length));
}

fn setup() -> (tempfile::TempDir, ExtensionStore, MemoryHops) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ExtensionStore::new(dir.path().join("extensions"));
    (dir, store, MemoryHops::default())
}

fn installer<'a>(store: &'a ExtensionStore, hops: &'a MemoryHops) -> Installer<'a> {
    Installer {
        fetcher: hops,
        store,
        chromium_version: CHROMIUM.to_owned(),
        publisher_key_hash: publisher(),
    }
}

#[test]
fn fetch_downloads_verifies_and_names_the_manifest() {
    let (_dir, store, hops) = setup();
    serve(&hops, "valid.crx");
    let candidate = installer(&store, &hops).fetch(&id()).expect("fetches");
    assert_eq!(candidate.id, id());
    assert_eq!(candidate.name, "rBrowser fixture: hello");
    assert_eq!(candidate.version, "1.0.0");
    assert!(store.installed().is_empty(), "fetching installs nothing");
    assert_eq!(hops.asked().len(), 2);
}

#[test]
fn a_bad_signature_is_refused_before_anything_is_written() {
    let (_dir, store, hops) = setup();
    serve(&hops, "bad-signature.crx");
    let error = installer(&store, &hops).fetch(&id()).expect_err("refused");
    assert_eq!(error, InstallFailure::Crx(crx3::Crx3Error::BadSignature));
    assert_eq!(error.code(), "extensions.crx_bad_signature");
    assert!(!store.root().exists() || store.installed().is_empty());
}

#[test]
fn a_wrong_reference_or_a_failed_download_is_named() {
    let (_dir, store, hops) = setup();
    let error = installer(&store, &hops).fetch("nope").expect_err("refused");
    assert_eq!(error, InstallFailure::Download(DownloadError::BadId));
    let error = installer(&store, &hops).fetch(&id()).expect_err("refused");
    assert!(
        matches!(error, InstallFailure::Download(DownloadError::Network(_))),
        "{error:?}"
    );
}

#[test]
fn install_by_id_puts_the_extension_in_the_store() {
    let (_dir, store, hops) = setup();
    serve(&hops, "valid.crx");
    let installed = installer(&store, &hops)
        .install_by_id(&id())
        .expect("installs");
    assert_eq!(installed.version, "1.0.0");
    assert_eq!(store.installed_one(&id()), Some(installed));
}

#[test]
fn a_zip_slip_crx_is_refused_at_the_install() {
    let (_dir, store, hops) = setup();
    serve(&hops, "zip-slip.crx");
    let error = installer(&store, &hops)
        .install_by_id(&id())
        .expect_err("refused");
    assert!(
        matches!(error, InstallFailure::Install(InstallError::UnsafePath(_))),
        "{error:?}"
    );
    assert!(store.installed().is_empty());
}

#[test]
fn newer_finds_the_update_or_nothing() {
    let (_dir, store, hops) = setup();
    serve(&hops, "valid.crx");
    let installed = installer(&store, &hops)
        .install_by_id(&id())
        .expect("installs");
    assert!(
        installer(&store, &hops)
            .newer(&installed)
            .expect("checks")
            .is_none()
    );
    let (_dir, store, hops) = setup();
    serve(&hops, "valid.crx");
    let installed = installer(&store, &hops)
        .install_by_id(&id())
        .expect("installs");
    let hops = MemoryHops::default();
    serve(&hops, "valid-2.crx");
    let newer = installer(&store, &hops)
        .newer(&installed)
        .expect("checks")
        .expect("newer");
    assert_eq!(newer.version, "2.0.0");
    let updated = installer(&store, &hops).install(&newer).expect("installs");
    assert_eq!(updated.version, "2.0.0");
}

/// The update check names the store's version without a download; the CRX is downloaded only
/// when that version is newer, and when the check answers nothing usable.
#[test]
fn newer_asks_the_update_check_first_and_downloads_only_a_newer_version() {
    let (_dir, store, hops) = setup();
    serve(&hops, "valid.crx");
    let installed = installer(&store, &hops)
        .install_by_id(&id())
        .expect("installs");
    let check = update_check_url(&id(), CHROMIUM);
    let answer = |version: &str| {
        format!(
            r#"<gupdate><app appid="{}" status="ok"><updatecheck status="ok" version="{version}"/></app></gupdate>"#,
            id()
        )
    };
    let hops = MemoryHops::default();
    hops.body(&check, answer("1.0.0").as_bytes(), None);
    assert!(
        installer(&store, &hops)
            .newer(&installed)
            .expect("checks")
            .is_none()
    );
    assert_eq!(
        hops.asked(),
        std::slice::from_ref(&check),
        "no download of a current version"
    );
    let hops = MemoryHops::default();
    hops.body(&check, answer("2.0.0").as_bytes(), None);
    serve(&hops, "valid-2.crx");
    let newer = installer(&store, &hops)
        .newer(&installed)
        .expect("checks")
        .expect("newer");
    assert_eq!(newer.version, "2.0.0");
    assert_eq!(
        hops.asked().len(),
        3,
        "the check, then the download's two hops"
    );
    let hops = MemoryHops::default();
    hops.body(&check, b"<gupdate/>", None);
    serve(&hops, "valid-2.crx");
    let newer = installer(&store, &hops)
        .newer(&installed)
        .expect("checks")
        .expect("newer by the download");
    assert_eq!(newer.version, "2.0.0");
}

#[test]
fn versions_compare_number_by_number() {
    assert!(is_newer("2.0.0", "1.9.9"));
    assert!(is_newer("1.10", "1.9"));
    assert!(is_newer("1.0.0.1", "1.0.0"));
    assert!(!is_newer("1.0.0", "1.0.0"));
    assert!(!is_newer("1.0", "1.0.0"));
    assert!(!is_newer("0.9", "1.0"));
    assert!(!is_newer("x", "1.0"));
}
