use std::path::Path;

use super::*;

#[test]
fn nothing_set_gives_the_defaults() {
    let settings = VideoSettings::resolve(
        &VideoConfig::default(),
        &VideoConfig::default(),
        &ToolsSettings::default(),
    );
    assert_eq!(settings.fps, DEFAULT_FPS);
    assert_eq!(settings.max_duration, Duration::from_secs(300));
    assert_eq!(settings.quality, DEFAULT_QUALITY);
    assert_eq!(settings.ffmpeg_path, None);
    assert_eq!(settings.keep_recordings, 0, "keeps every recording");
    assert_eq!(settings.keep_days, 0);
    assert!(!settings.autoplay);
}

/// Retention of the Videos panel (RB-110-12, security review): a checked-in project never
/// deletes more than the person allows - between the project's value and the user's the one
/// that keeps more wins, 0 (keep all) the most, and the user's default is 0. Autoplay: the
/// project wins as for every other key.
#[test]
fn retention_keeps_the_more_lenient_value_and_is_bounded() {
    let config = |keep: Option<u32>, days: Option<u32>| VideoConfig {
        keep_recordings: keep,
        keep_days: days,
        ..VideoConfig::default()
    };
    let resolve = |project: &VideoConfig, user: &VideoConfig| {
        let settings = VideoSettings::resolve(project, user, &ToolsSettings::default());
        (settings.keep_recordings, settings.keep_days)
    };
    let none = config(None, None);
    assert_eq!(
        resolve(&config(Some(1), Some(1)), &none),
        (0, 0),
        "the project alone deletes nothing"
    );
    assert_eq!(
        resolve(&none, &config(Some(5), Some(7))),
        (5, 7),
        "the user's own"
    );
    assert_eq!(
        resolve(&config(Some(2), Some(3)), &config(Some(5), Some(7))),
        (5, 7)
    );
    assert_eq!(
        resolve(&config(Some(9), Some(30)), &config(Some(5), Some(7))),
        (9, 30)
    );
    assert_eq!(
        resolve(&config(Some(0), Some(0)), &config(Some(5), Some(7))),
        (0, 0),
        "0 keeps all"
    );
    assert_eq!(
        resolve(&config(Some(5), Some(7)), &config(Some(0), Some(0))),
        (0, 0)
    );
    assert_eq!(
        resolve(&none, &config(Some(50_000), Some(99_999))),
        (MAX_KEEP_RECORDINGS, MAX_KEEP_DAYS),
        "bounded"
    );
    let project = VideoConfig {
        autoplay: Some(true),
        ..VideoConfig::default()
    };
    let user = VideoConfig {
        autoplay: Some(false),
        ..VideoConfig::default()
    };
    assert!(VideoSettings::resolve(&project, &user, &ToolsSettings::default()).autoplay);
}

#[test]
fn the_project_wins_and_every_value_is_bounded() {
    let project = VideoConfig {
        fps: Some(500),
        ..VideoConfig::default()
    };
    let user = VideoConfig {
        fps: Some(24),
        max_duration_sec: Some(99_999),
        quality: Some(0),
        ..VideoConfig::default()
    };
    let settings = VideoSettings::resolve(&project, &user, &ToolsSettings::default());
    assert_eq!(settings.fps, 60);
    assert_eq!(
        settings.max_duration,
        Duration::from_secs(u64::from(MAX_SECS))
    );
    assert_eq!(settings.quality, 1);
}

#[test]
fn only_an_absolute_ffmpeg_path_is_taken() {
    // A rooted path without a drive is relative to the current drive on Windows.
    const ROOTED: &str = "/usr/bin/ffmpeg";
    const DRIVE: &str = "C:\\tools\\ffmpeg.exe";
    let tools = |path: &str| ToolsSettings {
        ffmpeg_path: Some(path.to_owned()),
        ..ToolsSettings::default()
    };
    let none = VideoConfig::default();
    let (relatives, absolutes): (&[&str], &[&str]) = if cfg!(windows) {
        (&["ffmpeg.exe", "bin/ffmpeg", "  ", ROOTED], &[DRIVE])
    } else {
        (&["ffmpeg.exe", "bin/ffmpeg", "  "], &[DRIVE, ROOTED])
    };
    for &relative in relatives {
        let settings = VideoSettings::resolve(&none, &none, &tools(relative));
        assert_eq!(settings.ffmpeg_path, None, "{relative:?}");
    }
    for &absolute in absolutes {
        let settings = VideoSettings::resolve(&none, &none, &tools(absolute));
        assert_eq!(
            settings.ffmpeg_path.as_deref(),
            Some(Path::new(absolute)),
            "{absolute}"
        );
    }
}

#[test]
fn the_folder_is_below_the_project_out_folder() {
    let folder = run_folder(Path::new("/p"), &Stamp::from_unix(0));
    assert_eq!(
        folder,
        Path::new("/p/.rbrowser/out/video/1970-01-01_00-00-00")
    );
}

#[test]
fn file_names_are_the_breakpoints_made_unique_and_safe() {
    let views = [
        (ViewId(1), "mobile".to_owned()),
        (ViewId(2), "desktop".to_owned()),
        (ViewId(3), "mobile".to_owned()),
        (ViewId(4), "../con".to_owned()),
    ];
    // Always WebM (RB-090-52): it plays in the Videos panel on every platform.
    let names = file_names(&views);
    let expected: Vec<(ViewId, String)> = [
        (ViewId(1), "mobile-1.webm"),
        (ViewId(2), "desktop.webm"),
        (ViewId(3), "mobile-3.webm"),
        (ViewId(4), "_con.webm"),
    ]
    .into_iter()
    .map(|(view, name)| (view, name.to_owned()))
    .collect();
    assert_eq!(names, expected);
}

#[test]
fn the_run_folder_is_made_of_real_folders() {
    let dir = tempfile::tempdir().expect("tempdir");
    let stamp = Stamp::from_unix(0);
    let made = make_run_folder(dir.path(), &stamp).expect("made");
    assert_eq!(made, run_folder(dir.path(), &stamp));
    assert!(made.is_dir());
    // A second recording in the same second uses the same folder (ffmpeg never overwrites).
    assert_eq!(make_run_folder(dir.path(), &stamp).expect("again"), made);
}

/// Charge 9 review L1: a link checked into the project (`.rbrowser/out` pointing elsewhere) is
/// never followed; nothing is made behind it.
#[cfg(unix)]
#[test]
fn a_link_on_the_way_to_the_run_folder_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    std::fs::create_dir_all(dir.path().join(".rbrowser")).expect("dir");
    std::os::unix::fs::symlink(elsewhere.path(), dir.path().join(".rbrowser/out")).expect("link");
    assert!(make_run_folder(dir.path(), &Stamp::from_unix(0)).is_err());
    assert!(
        std::fs::read_dir(elsewhere.path())
            .expect("list")
            .next()
            .is_none(),
        "nothing made behind the link"
    );
}
