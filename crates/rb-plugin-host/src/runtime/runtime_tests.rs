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
