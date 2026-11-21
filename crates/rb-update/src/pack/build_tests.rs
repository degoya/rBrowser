use super::*;
use crate::keygen::{generate, key_files};
use crate::verify::check_assets;

const BASE: &str = "https://github.com/degoya/rBrowser/releases/download/v1.2.0/";

#[test]
fn the_package_zip_is_classified_and_everything_else_left_out() {
    assert_eq!(
        classify("rbrowser-1.2.0-windows-x64.zip", "1.2.0"),
        Some(("windows", "x86_64"))
    );
    for other in [
        "rbrowser-1.1.0-windows-x64.zip",
        "rbrowser-1.2.0-windows-x64.tar.gz",
        "rbrowser-1.2.0-linux-x64.zip",
        "SHA256SUMS",
        "rbrowser-update-stable.json",
    ] {
        assert_eq!(classify(other, "1.2.0"), None, "{other}");
    }
}

struct Release {
    folder: tempfile::TempDir,
    public: String,
}

impl Release {
    fn new() -> Self {
        let folder = tempfile::tempdir().expect("tempdir");
        let assets = folder.path().join("assets");
        std::fs::create_dir_all(&assets).expect("assets");
        std::fs::write(assets.join("rbrowser-1.2.0-windows-x64.zip"), b"zip bytes").expect("zip");
        std::fs::write(assets.join("SHA256SUMS"), b"ignored").expect("sums");
        std::fs::write(folder.path().join("notes.md"), "\n- Self-update.\n\n").expect("notes");
        let public = generate(&folder.path().join("keys")).expect("keygen");
        Self { folder, public }
    }

    fn args(&self, channel: &str, version: &str) -> BuildArgs {
        let (private, _) = key_files(&self.folder.path().join("keys"));
        BuildArgs {
            channel: channel.to_owned(),
            version: version.to_owned(),
            assets: self.folder.path().join("assets"),
            base_url: BASE.to_owned(),
            key: private,
            notes: Some(self.folder.path().join("notes.md")),
            min_version: None,
            valid_days: 30,
            sequence: Some(7),
            out: self.out(),
        }
    }

    fn out(&self) -> PathBuf {
        self.folder
            .path()
            .join("out")
            .join("rbrowser-update-stable.json")
    }

    fn roots(&self) -> [EmbeddedKey; 1] {
        let root: &'static str = Box::leak(self.public.clone().into_boxed_str());
        [EmbeddedKey {
            key_id: UPDATE_KEY_ID,
            public_key: root,
            not_after: None,
        }]
    }
}

/// keygen, build and verify round trip, as `scripts/sign-update.sh` runs them.
#[test]
fn a_built_manifest_verifies_under_its_key_and_lists_the_zip() {
    let release = Release::new();
    build_under(&release.args("stable", "1.2.0"), &release.roots()).expect("build");
    let bytes = std::fs::read(release.out()).expect("manifest");
    let trust = TrustStore::new();
    trust
        .trust(
            UPDATE_KEY_ID.to_owned(),
            sign::decode_public_key(&release.public).expect("public"),
        )
        .expect("trust");
    let read = manifest::verify_with(
        &bytes,
        &trust,
        Channel::Stable,
        None,
        OffsetDateTime::now_utc(),
    )
    .expect("verifies");
    assert_eq!(read.version, "1.2.0");
    assert_eq!(read.sequence, 7);
    assert_eq!(read.notes, "- Self-update.");
    assert_eq!(read.artifacts.len(), 1);
    assert_eq!(
        read.artifacts[0].url,
        format!("{BASE}rbrowser-1.2.0-windows-x64.zip")
    );
    assert_eq!(read.artifacts[0].sha256, sign::hex_sha256(b"zip bytes"));
    check_assets(&read, &release.folder.path().join("assets")).expect("assets match");
    std::fs::write(
        release
            .folder
            .path()
            .join("assets")
            .join("rbrowser-1.2.0-windows-x64.zip"),
        b"other bytes",
    )
    .expect("swap");
    assert!(check_assets(&read, &release.folder.path().join("assets")).is_err());
}

#[test]
fn a_key_that_is_not_the_embedded_root_is_refused() {
    let release = Release::new();
    let other = Release::new();
    assert!(build_under(&release.args("stable", "1.2.0"), &other.roots()).is_err());
    assert!(!release.out().exists());
}

#[test]
fn the_channel_must_match_the_version_and_the_base_must_be_https() {
    let release = Release::new();
    assert!(build_under(&release.args("beta", "1.2.0"), &release.roots()).is_err());
    let mut args = release.args("stable", "1.2.0");
    args.base_url = "http://example.test/".to_owned();
    assert!(build_under(&args, &release.roots()).is_err());
    let args = release.args("stable", "1.3.0");
    assert!(
        build_under(&args, &release.roots()).is_err(),
        "no archive of 1.3.0"
    );
    assert!(!release.out().exists());
}

#[test]
fn long_notes_are_cut_to_the_manifests_limit() {
    let long = "x".repeat(MAX_NOTES_CHARS + 100);
    let cut = notes(&long);
    assert_eq!(cut.chars().count(), MAX_NOTES_CHARS);
    assert!(cut.ends_with("..."));
    assert_eq!(notes("  short \n"), "short");
}
