use std::path::{Path, PathBuf};

use super::*;

fn install() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\rbrowser")
    } else {
        PathBuf::from("/opt/rbrowser")
    }
}

fn absolute(name: &str) -> String {
    install().join(name).display().to_string()
}

#[test]
fn the_expected_major_comes_from_nvmrc() {
    let (major, _, _) = parse_version(NVMRC).expect("the .nvmrc holds a version");
    assert_eq!(expected_major(), major);
    assert!(major >= 26);
}

#[test]
fn versions_parse_with_and_without_v() {
    assert_eq!(parse_version("v26.13.0\n"), Some((26, 13, 0)));
    assert_eq!(parse_version("24.21.0"), Some((24, 21, 0)));
    assert_eq!(parse_version("v26"), None);
    assert_eq!(parse_version("node"), None);
}

#[test]
fn the_bundled_runtime_and_script_sit_beside_the_executable() {
    let node = bundled_node(&install());
    assert!(node.starts_with(install().join("node")));
    assert_eq!(
        sidecar_script(&install()),
        install().join("sidecar").join("dist").join("main.js")
    );
}

#[test]
fn the_setting_wins_over_the_bundled_runtime() {
    let own = absolute("own-node");
    let found = resolve_node(Some(&own), &install(), |_| true).expect("found");
    assert_eq!(found, PathBuf::from(own));
}

#[test]
fn without_a_setting_the_bundled_runtime_is_taken() {
    let found = resolve_node(Some("  "), &install(), |path| {
        path == bundled_node(&install()).as_path()
    })
    .expect("found");
    assert_eq!(found, bundled_node(&install()));
}

#[test]
fn a_missing_bundled_runtime_is_an_error_with_its_path() {
    let error = resolve_node(None, &install(), |_| false).expect_err("missing");
    assert!(matches!(error, SidecarError::NodeNotFound(path) if path == bundled_node(&install())));
}

#[test]
fn a_relative_or_missing_configured_runtime_is_refused() {
    let relative = resolve_node(Some("node.exe"), &install(), |_| true).expect_err("relative");
    assert!(matches!(relative, SidecarError::NodePathInvalid(_)));
    let missing =
        resolve_node(Some(&absolute("gone")), &install(), |_| false).expect_err("missing");
    assert!(matches!(missing, SidecarError::NodePathInvalid(_)));
}

#[test]
fn a_matching_major_passes() {
    let found = check_version(Path::new("node"), "v26.1.0\n", 26).expect("matches");
    assert_eq!(found, "26.1.0");
}

#[test]
fn a_wrong_major_says_what_was_expected_and_found() {
    let error = check_version(Path::new("node"), "v24.21.0\n", 26).expect_err("mismatch");
    assert_eq!(error.to_string(), "Node 26 expected, 24.21.0 found (node)");
}

#[test]
fn output_without_a_version_is_unknown() {
    let error = check_version(Path::new("node"), "hello", 26).expect_err("unknown");
    assert!(matches!(error, SidecarError::NodeVersionUnknown { .. }));
}
