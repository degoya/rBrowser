use std::ffi::OsString;
use std::path::Path;

use rb_config::McpSettings;

use super::{arguments, wanted};

#[test]
fn only_with_the_setting_the_server_and_a_project() {
    let root = Path::new(r"\\wsl.localhost\Ubuntu\home\me\app");
    let on = McpSettings {
        enabled: true,
        stdio_auto_start: true,
        ..McpSettings::default()
    };
    assert!(wanted(&on, Some(root)));
    assert!(!wanted(&on, None), "no project: nothing to open");
    assert!(
        !wanted(&McpSettings::default(), Some(root)),
        "off by default"
    );
    let server_off = McpSettings {
        enabled: false,
        ..on.clone()
    };
    assert!(!wanted(&server_off, Some(root)), "it would serve nothing");
}

#[test]
fn the_project_opens_in_the_background() {
    let root = Path::new("/work/site");
    assert_eq!(
        arguments(root),
        vec![
            OsString::from("--project"),
            OsString::from("/work/site"),
            OsString::from("--background"),
        ]
    );
}
