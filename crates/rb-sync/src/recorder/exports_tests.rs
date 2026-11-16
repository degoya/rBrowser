use std::fs;

use super::*;
use crate::recorder::model::{RECORDING_VERSION, RecordedBreakpoint, Recording};

const PROTECTED: &[&str] = &["flows", "snippets", "plugins", "mocks", "scripts"];

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(dir.path().join(".rbrowser")).expect("config dir");
    dir
}

fn files() -> Vec<(String, String)> {
    vec![
        ("login.spec.ts".to_owned(), "test".to_owned()),
        ("playwright.config.ts".to_owned(), "config".to_owned()),
    ]
}

#[test]
fn the_folder_must_lie_in_rbrowser_outside_the_protected_folders() {
    let root = project();
    assert!(ExportsDir::open(root.path(), DEFAULT_EXPORTS_DIR, PROTECTED).is_ok());
    for bad in [
        ".rbrowser/flows",
        "out/exports",
        "../x",
        "/tmp/x",
        ".rbrowser",
    ] {
        assert!(
            matches!(
                ExportsDir::open(root.path(), bad, PROTECTED),
                Err(ExportsError::BadDir(_))
            ),
            "{bad}"
        );
    }
}

#[test]
fn an_export_is_a_new_folder_of_its_own() {
    let root = project();
    let dir = ExportsDir::open(root.path(), DEFAULT_EXPORTS_DIR, PROTECTED).expect("dir");
    let written = dir.write_new("login", &files()).expect("written");
    assert_eq!(
        written.folder,
        root.path().join(".rbrowser/out/exports/login")
    );
    assert_eq!(written.files.len(), 2);
    assert_eq!(
        fs::read_to_string(written.folder.join("login.spec.ts")).expect("spec"),
        "test"
    );
}

#[test]
fn an_existing_export_is_never_overwritten() {
    let root = project();
    let dir = ExportsDir::open(root.path(), DEFAULT_EXPORTS_DIR, PROTECTED).expect("dir");
    let first = dir.write_new("login", &files()).expect("first");
    fs::write(first.folder.join("login.spec.ts"), "edited").expect("edited");
    let second = dir.write_new("login", &files()).expect("second");
    assert_eq!(
        second.folder,
        root.path().join(".rbrowser/out/exports/login-2")
    );
    assert_eq!(
        fs::read_to_string(first.folder.join("login.spec.ts")).expect("kept"),
        "edited"
    );
    let third = dir.write_new("login", &files()).expect("third");
    assert!(third.folder.ends_with("login-3"));
}

#[test]
fn a_file_name_with_a_folder_or_a_bad_stem_is_refused() {
    let root = project();
    let dir = ExportsDir::open(root.path(), DEFAULT_EXPORTS_DIR, PROTECTED).expect("dir");
    for name in [
        "../x.ts",
        "a/b.ts",
        "",
        ".",
        "..",
        "x\\y.ts",
        "c:x.ts",
        "nul.spec.ts",
        "CON",
        "com1.ts",
    ] {
        assert!(
            matches!(
                dir.write_new("login", &[(name.to_owned(), String::new())]),
                Err(ExportsError::BadFileName(_))
            ),
            "{name:?}"
        );
    }
    for stem in ["../evil", "aux"] {
        assert!(matches!(
            dir.write_new(stem, &files()),
            Err(ExportsError::BadName(_))
        ));
    }
    assert!(
        !root.path().join(".rbrowser/out/exports").exists()
            || fs::read_dir(root.path().join(".rbrowser/out/exports"))
                .expect("listed")
                .next()
                .is_none(),
        "nothing written for a refused export"
    );
}

#[cfg(unix)]
#[test]
fn a_link_out_of_rbrowser_is_refused() {
    let root = project();
    let outside = tempfile::tempdir().expect("outside");
    std::os::unix::fs::symlink(outside.path(), root.path().join(".rbrowser/out")).expect("link");
    let dir =
        ExportsDir::open(root.path(), DEFAULT_EXPORTS_DIR, PROTECTED).expect("lexically fine");
    assert!(matches!(
        dir.write_new("login", &files()),
        Err(ExportsError::BadDir(_))
    ));
    assert!(
        fs::read_dir(outside.path())
            .expect("listed")
            .next()
            .is_none()
    );
}

#[test]
fn the_request_is_the_recording_and_the_options_in_camel_case() {
    let recording = Recording {
        schema: None,
        version: RECORDING_VERSION,
        name: "login".into(),
        start_url: None,
        environment: None,
        breakpoints: vec![RecordedBreakpoint {
            id: "mobile".into(),
            width: 390,
            height: 844,
            device_scale_factor: 3.0,
        }],
        started_at_ms: 1,
        app_version: "0.5.0".into(),
        steps: Vec::new(),
    };
    let request = ExportRequest {
        recording: &recording,
        options: ExportOptions {
            locator_preference: "testid".into(),
            base_url: Some("http://127.0.0.1:8000".into()),
            projects: vec![ExportProject {
                name: "mobile".into(),
                engine: "chromium".into(),
                width: 390,
                height: 844,
                device_scale_factor: Some(3.0),
                is_mobile: Some(true),
                has_touch: Some(true),
                user_agent: None,
                locale: Some("de-DE".into()),
                color_scheme: Some("dark".into()),
                reduced_motion: None,
            }],
        },
    };
    let value = serde_json::to_value(&request).expect("serialised");
    assert_eq!(value["recording"]["name"], "login");
    assert_eq!(value["options"]["locatorPreference"], "testid");
    assert_eq!(value["options"]["baseUrl"], "http://127.0.0.1:8000");
    let project = &value["options"]["projects"][0];
    assert_eq!(project["deviceScaleFactor"], 3.0);
    assert_eq!(project["isMobile"], true);
    assert_eq!(project["colorScheme"], "dark");
    assert!(project.get("userAgent").is_none());
}
