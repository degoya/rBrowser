use std::path::Path;

use serde_json::json;

use super::*;

#[test]
fn nothing_set_gives_the_defaults() {
    let (settings, problems) = CaptureSettings::resolve(None, None);
    assert!(problems.is_empty());
    assert_eq!(settings, CaptureSettings::default());
    assert_eq!(settings.path_pattern, crate::naming::DEFAULT_PATTERN);
    assert_eq!(settings.element_padding, DEFAULT_PADDING);
    assert_eq!(settings.default_delay, Delay::None);
    assert!(settings.overview_labels);
    let root = Path::new("/work/site");
    assert_eq!(
        settings.output_dir(root).expect("default folder"),
        root.join(".rbrowser").join("out").join("screenshots")
    );
}

#[test]
fn the_project_wins_over_the_user() {
    let project = json!({ "pathPattern": "{date}/{env}/{breakpoint}", "elementPadding": 8 });
    let user = json!({ "pathPattern": "{breakpoint}", "defaultDelay": 5, "overviewLabels": false });
    let (settings, problems) = CaptureSettings::resolve(Some(&project), Some(&user));
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(settings.path_pattern, "{date}/{env}/{breakpoint}");
    assert_eq!(settings.element_padding, 8);
    assert_eq!(settings.default_delay, Delay::Five);
    assert!(!settings.overview_labels);
}

#[test]
fn a_bad_value_falls_back_and_is_named() {
    let project = json!({
        "pathPattern": "{nope}",
        "elementPadding": 100_000,
        "defaultDelay": 7,
        "overviewLabels": "yes",
        "dir": 5
    });
    let (settings, problems) = CaptureSettings::resolve(Some(&project), None);
    assert_eq!(settings, CaptureSettings::default());
    assert_eq!(problems.len(), 5, "{problems:?}");
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("capture.pathPattern"))
    );
}

#[test]
fn the_clean_keys_are_read_and_the_project_wins() {
    let project = json!({ "hideSelectors": ["#promo"], "freezeAnimations": false });
    let user = json!({
        "hideBanners": false,
        "hideSelectors": ["#user-only"],
        "useBuiltinList": false,
        "freezeAnimations": true,
        "settleMs": 250
    });
    let (settings, problems) = CaptureSettings::resolve(Some(&project), Some(&user));
    assert!(problems.is_empty(), "{problems:?}");
    assert!(!settings.clean.hide_banners);
    assert_eq!(settings.clean.hide_selectors, ["#promo"]);
    assert!(!settings.clean.use_builtin_list);
    assert!(!settings.clean.freeze_animations);
    assert_eq!(settings.clean.settle_ms, 250);
}

#[test]
fn bad_clean_values_fall_back_and_are_named() {
    let project = json!({
        "hideBanners": "yes",
        "hideSelectors": "#one",
        "useBuiltinList": 1,
        "freezeAnimations": null,
        "settleMs": 99_999
    });
    let (settings, problems) = CaptureSettings::resolve(Some(&project), None);
    assert_eq!(settings, CaptureSettings::default());
    assert_eq!(problems.len(), 4, "{problems:?}");
}

#[test]
fn a_project_folder_stays_inside_the_project() {
    let root = Path::new("/work/site");
    let (inside, _) = CaptureSettings::resolve(Some(&json!({ "dir": "shots/out" })), None);
    assert_eq!(
        inside.output_dir(root).expect("relative folder"),
        root.join("shots").join("out")
    );
    for outside in [
        "../elsewhere",
        "/etc",
        "C:\\Windows",
        "\\\\server\\share",
        "a/../../b",
    ] {
        let (settings, _) = CaptureSettings::resolve(Some(&json!({ "dir": outside })), None);
        assert!(
            matches!(settings.output_dir(root), Err(VisualError::Folder(_))),
            "{outside} was taken"
        );
    }
}

#[test]
fn the_user_may_name_an_absolute_folder() {
    let root = Path::new("/work/site");
    let absolute = if cfg!(windows) {
        "C:\\shots"
    } else {
        "/home/me/shots"
    };
    let (settings, _) = CaptureSettings::resolve(None, Some(&json!({ "dir": absolute })));
    assert_eq!(
        settings.output_dir(root).expect("absolute user folder"),
        Path::new(absolute)
    );
    let (settings, _) = CaptureSettings::resolve(None, Some(&json!({ "dir": "../up" })));
    assert!(settings.output_dir(root).is_err());
}

#[test]
fn an_empty_folder_is_the_default() {
    let (settings, problems) = CaptureSettings::resolve(Some(&json!({ "dir": "  " })), None);
    assert!(problems.is_empty());
    assert_eq!(settings.dir, None);
}

#[test]
fn a_project_folder_is_never_in_the_config_or_git_folder() {
    let root = Path::new("/work/site");
    for refused in [
        ".git/x",
        ".GIT/hooks",
        ".git./x",
        ".rbrowser",
        ".rbrowser/trust-ish",
        ".rbrowser/plugins",
        ".Rbrowser/config",
    ] {
        let (settings, _) = CaptureSettings::resolve(Some(&json!({ "dir": refused })), None);
        assert!(
            matches!(settings.output_dir(root), Err(VisualError::Folder(_))),
            "{refused} was taken"
        );
    }
    for taken in [
        ".rbrowser/out",
        ".rbrowser/out/shots",
        "docs/.rbrowser-shots",
    ] {
        let (settings, _) = CaptureSettings::resolve(Some(&json!({ "dir": taken })), None);
        assert!(settings.output_dir(root).is_ok(), "{taken} was refused");
    }
}

#[test]
fn the_user_folder_is_no_network_share_or_device() {
    let root = Path::new("/work/site");
    for refused in [
        "\\\\server\\share\\shots",
        "//server/share/shots",
        "\\\\?\\UNC\\server\\share",
        "\\\\.\\pipe\\shots",
        "\\\\?\\C:\\shots",
        "\\\\.\\C:\\shots",
    ] {
        let (settings, _) = CaptureSettings::resolve(None, Some(&json!({ "dir": refused })));
        assert!(
            matches!(settings.output_dir(root), Err(VisualError::Folder(_))),
            "{refused} was taken"
        );
    }
}

/// A folder link checked into the project leads out of it: refused, for a `capture.dir` of the
/// project and for the default folder alike.
#[cfg(unix)]
#[test]
fn a_link_out_of_the_project_is_refused() {
    let project = tempfile::tempdir().expect("a project");
    let elsewhere = tempfile::tempdir().expect("another folder");
    let root = project.path();
    std::os::unix::fs::symlink(elsewhere.path(), root.join("shots")).expect("a link");
    let (settings, _) = CaptureSettings::resolve(Some(&json!({ "dir": "shots/out" })), None);
    assert!(matches!(
        settings.output_dir(root),
        Err(VisualError::Folder(_))
    ));
    std::fs::create_dir(root.join(".rbrowser")).expect("a folder");
    std::os::unix::fs::symlink(elsewhere.path(), root.join(".rbrowser/out")).expect("a link");
    assert!(matches!(
        CaptureSettings::default().output_dir(root),
        Err(VisualError::Folder(_))
    ));
    // A real folder is fine.
    std::fs::create_dir(root.join("real")).expect("a folder");
    let (settings, _) = CaptureSettings::resolve(Some(&json!({ "dir": "real/out" })), None);
    assert_eq!(
        settings.output_dir(root).expect("inside"),
        root.join("real").join("out")
    );
}
