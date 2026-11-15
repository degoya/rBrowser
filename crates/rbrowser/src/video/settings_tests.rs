use std::path::Path;

use super::*;

#[test]
fn nothing_set_gives_the_defaults() {
    let settings = VideoSettings::resolve(
        &VideoConfig::default(),
        &VideoConfig::default(),
        &ToolsSettings::default(),
    );
    assert_eq!(settings.format, VideoFormat::Mp4);
    assert_eq!(settings.fps, DEFAULT_FPS);
    assert_eq!(settings.max_duration, Duration::from_secs(300));
    assert_eq!(settings.quality, DEFAULT_QUALITY);
    assert_eq!(settings.ffmpeg_path, None);
}

#[test]
fn the_project_wins_and_every_value_is_bounded() {
    let project = VideoConfig {
        fps: Some(500),
        ..VideoConfig::default()
    };
    let user = VideoConfig {
        format: Some(VideoFormat::Webm),
        fps: Some(24),
        max_duration_sec: Some(99_999),
        quality: Some(0),
    };
    let settings = VideoSettings::resolve(&project, &user, &ToolsSettings::default());
    assert_eq!(settings.format, VideoFormat::Webm);
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
    let names = file_names(&views, VideoFormat::Webm);
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
