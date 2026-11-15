use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::*;

#[test]
fn inside_an_app_bundle_the_files_are_its_resources() {
    let exe = Path::new("/Applications/rBrowser.app/Contents/MacOS/rbrowser");
    assert_eq!(
        resource_dir(exe),
        Some(PathBuf::from(
            "/Applications/rBrowser.app/Contents/Resources"
        ))
    );
}

#[test]
fn elsewhere_the_files_lie_beside_the_executable() {
    for (exe, dir) in [
        ("/opt/rbrowser/rbrowser", "/opt/rbrowser"),
        // A folder called MacOS that is no bundle's.
        ("/home/a/MacOS/rbrowser", "/home/a/MacOS"),
        ("/home/a/Contents/MacOS/rbrowser", "/home/a/Contents/MacOS"),
    ] {
        assert_eq!(
            resource_dir(Path::new(exe)),
            Some(PathBuf::from(dir)),
            "{exe}"
        );
    }
    assert_eq!(resource_dir(Path::new("rbrowser")), Some(PathBuf::new()));
}

#[test]
fn the_framework_of_the_app_and_of_a_helper() {
    let app = Path::new("/Applications/rBrowser.app/Contents/MacOS/rbrowser");
    assert_eq!(
        framework(app, false),
        Some(PathBuf::from(
            "/Applications/rBrowser.app/Contents/MacOS/../Frameworks/Chromium Embedded Framework.framework/Chromium Embedded Framework"
        ))
    );
    let helper = Path::new(
        "/Applications/rBrowser.app/Contents/Frameworks/rbrowser Helper (GPU).app/Contents/MacOS/rbrowser Helper (GPU)",
    );
    assert_eq!(
        framework(helper, true),
        Some(PathBuf::from(
            "/Applications/rBrowser.app/Contents/Frameworks/rbrowser Helper (GPU).app/Contents/MacOS/../../../Chromium Embedded Framework.framework/Chromium Embedded Framework"
        ))
    );
    assert_eq!(framework(Path::new("/"), false), None);
}

#[test]
fn the_process_serial_number_of_an_older_finder_is_dropped() {
    let args: Vec<OsString> = [
        "rbrowser",
        "-psn_0_1234567",
        "--background",
        "https://a.test",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    let kept: Vec<OsString> = ["rbrowser", "--background", "https://a.test"]
        .into_iter()
        .map(OsString::from)
        .collect();
    assert_eq!(without_process_serial(args), kept);
}

#[test]
fn every_other_argument_stays() {
    let args: Vec<OsString> = ["rbrowser", "open", "-p", "psn_1", "--psn"]
        .into_iter()
        .map(OsString::from)
        .collect();
    assert_eq!(without_process_serial(args.clone()), args);
}
