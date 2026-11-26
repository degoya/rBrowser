use rb_plugin_types::codes;

use super::{allowed_imports, refused_import};
use crate::manifest::parse_manifest;
use crate::test_support::{ManifestBuilder, signing_key};

#[test]
fn a_plugin_imports_host_types_and_one_interface_per_grant() {
    let builder = ManifestBuilder::default()
        .capability("http = true")
        .capability("network-rules = true");
    let manifest = parse_manifest(builder.text(&signing_key(1)).as_bytes()).expect("valid");
    assert_eq!(
        allowed_imports(&manifest),
        [
            "rbrowser:plugin/host@0.1.0",
            "rbrowser:plugin/types@0.1.0",
            "rbrowser:plugin/http@0.1.0",
        ]
    );
}

#[test]
fn a_refused_import_names_its_capability_or_itself() {
    let http = refused_import("rbrowser:plugin/http@0.1.0");
    assert_eq!(http.code(), Some(codes::CAPABILITY_DENIED));
    assert_eq!(http.param("capability"), Some("http"));
    let wasi = refused_import("wasi:filesystem/types@0.2.0");
    assert_eq!(wasi.code(), Some(codes::DENIED));
    let other_version = refused_import("rbrowser:plugin/http@0.2.0");
    assert_eq!(other_version.code(), Some(codes::DENIED));
}

#[test]
fn project_write_is_its_own_import_reading_grants_no_writing() {
    let reader =
        ManifestBuilder::default().capability("project-read = { globs = [\"**/*.todo\"] }");
    let manifest = parse_manifest(reader.text(&signing_key(1)).as_bytes()).expect("valid");
    assert!(
        !allowed_imports(&manifest).contains(&"rbrowser:plugin/project-write@0.1.0".to_owned())
    );
    let writer = ManifestBuilder::default()
        .capability("project-read = { globs = [\"**/*.todo\"] }")
        .capability("project-write = { globs = [\"**/*.todo\"] }");
    let manifest = parse_manifest(writer.text(&signing_key(1)).as_bytes()).expect("valid");
    assert!(allowed_imports(&manifest).contains(&"rbrowser:plugin/project-write@0.1.0".to_owned()));
    let refused = refused_import("rbrowser:plugin/project-write@0.1.0");
    assert_eq!(refused.param("capability"), Some("project-write"));
}

#[test]
fn project_read_and_out_write_allow_their_large_file_imports() {
    // RB-099-24: the additive interfaces of the same rights; without the rights neither.
    let both = ManifestBuilder::default()
        .capability("project-read = true")
        .capability("out-write = true");
    let manifest = parse_manifest(both.text(&signing_key(1)).as_bytes()).expect("valid");
    let allowed = allowed_imports(&manifest);
    for name in [
        "rbrowser:plugin/project@0.1.0",
        "rbrowser:plugin/project-range@0.1.0",
        "rbrowser:plugin/out@0.1.0",
        "rbrowser:plugin/out-stream@0.1.0",
    ] {
        assert!(allowed.contains(&name.to_owned()), "{name}");
    }
    let http = ManifestBuilder::default().capability("http = true");
    let manifest = parse_manifest(http.text(&signing_key(1)).as_bytes()).expect("valid");
    let allowed = allowed_imports(&manifest);
    assert!(!allowed.contains(&"rbrowser:plugin/project-range@0.1.0".to_owned()));
    assert!(!allowed.contains(&"rbrowser:plugin/out-stream@0.1.0".to_owned()));
    let range = refused_import("rbrowser:plugin/project-range@0.1.0");
    assert_eq!(range.code(), Some(codes::CAPABILITY_DENIED));
    assert_eq!(range.param("capability"), Some("project-read"));
    let stream = refused_import("rbrowser:plugin/out-stream@0.1.0");
    assert_eq!(stream.param("capability"), Some("out-write"));
}
