use std::path::{Path, PathBuf};

use super::*;
use crate::error::SidecarError;
use crate::node::{bundled_node, resolve_node, sidecar_script};

fn bundle_exe() -> PathBuf {
    PathBuf::from("/Applications/rBrowser.app/Contents/MacOS/rbrowser")
}

/// Built with `join` as `package_dir` builds it, so the separators match on Windows too.
fn resources() -> PathBuf {
    PathBuf::from("/Applications/rBrowser.app/Contents").join("Resources")
}

#[test]
fn beside_the_executable_the_package_is_its_folder() {
    let exe = Path::new("/opt/rbrowser/rbrowser");
    assert_eq!(
        package_dir(exe, Layout::Beside),
        Some(PathBuf::from("/opt/rbrowser"))
    );
    // The Windows ZIP keeps that layout whatever the executable's folder is called.
    assert_eq!(
        package_dir(&bundle_exe(), Layout::Beside),
        Some(PathBuf::from("/Applications/rBrowser.app/Contents/MacOS"))
    );
}

#[test]
fn in_the_app_bundle_the_package_is_contents_resources() {
    assert_eq!(
        package_dir(&bundle_exe(), Layout::AppBundle),
        Some(resources())
    );
}

#[test]
fn the_bundle_holds_the_runtime_and_the_script_in_resources() {
    let dir = package_dir(&bundle_exe(), Layout::AppBundle).expect("a bundle");
    assert_eq!(
        sidecar_script(&dir),
        resources().join("sidecar").join("dist").join("main.js")
    );
    assert!(bundled_node(&dir).starts_with(resources().join("node")));
    if !cfg!(windows) {
        assert_eq!(
            bundled_node(&dir),
            resources().join("node").join("bin").join("node")
        );
    }
}

#[test]
fn an_executable_outside_a_bundle_finds_the_package_beside_it() {
    // A development build on a Mac (target/<triple>/<profile>/rbrowser), or a MacOS folder that
    // is not a bundle's Contents/MacOS.
    for exe in [
        "/Users/a/rbrowser/target/aarch64-apple-darwin/release/rbrowser",
        "/Users/a/MacOS/rbrowser",
    ] {
        let exe = Path::new(exe);
        assert_eq!(
            package_dir(exe, Layout::AppBundle),
            exe.parent().map(Path::to_path_buf),
            "{}",
            exe.display()
        );
    }
}

#[test]
fn an_executable_without_a_folder_has_no_package() {
    for layout in [Layout::Beside, Layout::AppBundle] {
        assert_eq!(package_dir(Path::new("rbrowser"), layout), None);
        assert_eq!(package_dir(Path::new("/"), layout), None);
    }
}

#[test]
fn a_bundle_without_node_names_the_missing_runtime() {
    let dir = package_dir(&bundle_exe(), Layout::AppBundle).expect("a bundle");
    let error = resolve_node(None, &dir, |_| false).expect_err("missing");
    assert!(
        matches!(&error, SidecarError::NodeNotFound(path) if path.starts_with(resources().join("node")))
    );
    assert!(
        error
            .to_string()
            .contains(&resources().join("node").display().to_string()),
        "{error}"
    );
    assert_eq!(error.i18n_key(), "sidecar.error.nodeNotFound");
}

#[test]
fn the_layout_follows_the_platform() {
    let expected = if cfg!(target_os = "macos") {
        Layout::AppBundle
    } else {
        Layout::Beside
    };
    assert_eq!(Layout::current(), expected);
}
