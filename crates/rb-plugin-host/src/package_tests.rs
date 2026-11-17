use super::{check_members, package_digest, read_archive};
use crate::manifest::parse_manifest;
use crate::test_support::{ManifestBuilder, signing_key, zip_package};

#[test]
fn an_unexpected_member_refuses_the_package() {
    for name in [
        "notes.txt",
        "page/../x.js",
        "../escape",
        "locales/en.txt",
        "bin/run.exe",
    ] {
        let package = zip_package(&[
            ("manifest.toml", b"x"),
            ("component.wasm", b"x"),
            (name, b"x"),
        ]);
        let refused = read_archive(std::io::Cursor::new(package));
        assert!(refused.is_err(), "{name}");
    }
}

#[test]
fn page_and_panel_files_and_the_locales_are_members() {
    let package = zip_package(&[
        ("manifest.toml", b"m"),
        ("component.wasm", b"c"),
        ("locales/en.json", b"{}"),
        ("page/overlay.js", b"1"),
        ("panel/index.html", b"<p>"),
        ("panel/assets/app.css", b"a{}"),
    ]);
    let parts = read_archive(std::io::Cursor::new(package)).expect("valid");
    let names: Vec<&str> = parts.files.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        [
            "locales/en.json",
            "page/overlay.js",
            "panel/assets/app.css",
            "panel/index.html"
        ]
    );
    assert!(parts.signature.is_none());
}

#[test]
fn every_file_is_covered_by_the_digest() {
    let files = vec![("page/overlay.js".to_owned(), b"1".to_vec())];
    let changed = vec![("page/overlay.js".to_owned(), b"2".to_vec())];
    assert_ne!(
        package_digest(b"m", b"c", &files),
        package_digest(b"m", b"c", &changed)
    );
    assert_ne!(
        package_digest(b"m", b"c", &files),
        package_digest(b"m", b"c", &[])
    );
}

#[test]
fn the_members_a_manifest_names_must_be_there() {
    let builder =
        ManifestBuilder::default().tables("\n[[page_scripts]]\npath = \"page/overlay.js\"\n");
    let manifest = parse_manifest(builder.text(&signing_key(1)).as_bytes()).expect("valid");
    let locale = builder.locale();
    assert!(check_members(&manifest, std::slice::from_ref(&locale)).is_err());
    let script = ("page/overlay.js".to_owned(), b"1".to_vec());
    assert!(check_members(&manifest, &[locale.clone(), script.clone()]).is_ok());
    let stray = ("locales/de.json".to_owned(), b"{}".to_vec());
    assert!(check_members(&manifest, &[locale, script, stray]).is_err());
}
