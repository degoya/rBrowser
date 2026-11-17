use std::fs;
use std::path::Path;
#[cfg(unix)]
use std::sync::{Arc, Mutex};

use rb_ipc::VideoTarget;

use super::*;
#[cfg(unix)]
use crate::cdp::lock;
#[cfg(unix)]
use crate::video::encoder::WebmCodec;
#[cfg(unix)]
use crate::video::registry::Registry;
use crate::video::registry::{FileStamp, Recorded};

const RUN: &str = "2026-11-16_14-03-07";

fn folder(root: &Path) -> PathBuf {
    root.join(".rbrowser/out/video").join(RUN)
}

/// A video the recorder wrote: on disk and registered.
fn recorded_video(recorded: &mut Recorded, root: &Path, name: &str) {
    fs::create_dir_all(folder(root)).expect("folder");
    fs::write(folder(root).join(name), b"video").expect("file");
    let metadata = fs::symlink_metadata(folder(root).join(name)).expect("metadata");
    recorded
        .recordings
        .entry(RUN.to_owned())
        .or_default()
        .insert(name.to_owned(), FileStamp::of(&metadata).expect("a stamp"));
}

/// RB-090-52: only a video rBrowser recorded is made into another format, beside it, and never
/// over a file already there; the recording being written is left alone.
#[test]
fn a_job_is_planned_only_for_a_registered_video() {
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let mut recorded = Recorded::default();
    recorded_video(&mut recorded, root, "mobile.webm");
    fs::write(folder(root).join("planted.webm"), b"x").expect("planted");

    let job = plan(root, &recorded, RUN, "mobile.webm", VideoTarget::Mp4, &[]).expect("planned");
    assert_eq!(job.target, "mobile.mp4");
    assert_eq!(job.folder, folder(root));
    assert_eq!(job.source, folder(root).join("mobile.webm"));

    assert_eq!(
        plan(root, &recorded, RUN, "planted.webm", VideoTarget::Mp4, &[]),
        Err(ConvertError::Library(LibraryError::NotFound)),
        "a file rBrowser did not record"
    );
    assert_eq!(
        plan(
            root,
            &recorded,
            RUN,
            "mobile.webm",
            VideoTarget::Mp4,
            &[RUN]
        ),
        Err(ConvertError::Library(LibraryError::Running))
    );
    assert!(matches!(
        plan(root, &recorded, RUN, "mobile.webm", VideoTarget::Webm, &[]),
        Err(ConvertError::Library(LibraryError::Refused(_)))
    ));
    fs::write(folder(root).join("mobile.mp4"), b"someone's").expect("there");
    assert_eq!(
        plan(root, &recorded, RUN, "mobile.webm", VideoTarget::Mp4, &[]),
        Err(ConvertError::Exists("mobile.mp4".to_owned()))
    );
}

#[cfg(unix)]
fn fake_ffmpeg(dir: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let program = dir.join("ffmpeg");
    fs::write(&program, format!("#!/bin/sh\n{body}\n")).expect("script");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("mode");
    program
}

/// Stands in for ffmpeg: says the length, reports half and the end, writes its last argument
/// (the `.part` file) - or, for a poster, the JPEG.
#[cfg(unix)]
const CONVERTS: &str = r#"for last; do :; done
echo "  Duration: 00:00:04.00, start: 0.000000, bitrate: 1 kb/s" >&2
sleep 0.2
echo "out_time_us=2000000"
echo "progress=continue"
echo "out_time_us=4000000"
echo "progress=end"
printf converted > "$last""#;

/// RB-090-52: the result is written as `.part`, renamed once complete, registered with the
/// breakpoint's poster when there was none, and the progress is reported.
#[cfg(unix)]
#[tokio::test]
async fn a_conversion_writes_renames_registers_and_reports() {
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let mut recorded = Recorded::default();
    recorded_video(&mut recorded, root, "mobile.mp4");
    let tools = tempfile::tempdir().expect("temp dir");
    let ffmpeg = fake_ffmpeg(tools.path(), CONVERTS);
    let registry_dir = tempfile::tempdir().expect("temp dir");
    // A stale part of a run the app's end cut short.
    fs::write(folder(root).join("mobile.webm.part"), b"stale").expect("stale");

    let job = plan(root, &recorded, RUN, "mobile.mp4", VideoTarget::Webm, &[]).expect("planned");
    let (_cancel, mut cancelled) = tokio::sync::watch::channel(false);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    let mut report = move |percent: u8| lock(&log).push(percent);
    let names = make(
        &job,
        &ffmpeg,
        EncodeWith {
            encoder: Encoder::Webm(WebmCodec::Vp9),
            quality: 80,
            threads: 2,
        },
        Some(registry_dir.path()),
        &mut cancelled,
        &mut report,
    )
    .await
    .expect("made");
    assert_eq!(names, ["mobile.webm", "mobile.jpg"]);
    assert_eq!(
        fs::read(folder(root).join("mobile.webm")).expect("target"),
        b"converted"
    );
    assert!(!folder(root).join("mobile.webm.part").exists());
    // The share follows ffmpeg's lines (the length may come a little after the first) and ends
    // at 100.
    let seen = lock(&seen).clone();
    assert_eq!(seen.last(), Some(&100), "{seen:?}");
    assert!(seen.windows(2).all(|pair| pair[0] < pair[1]), "{seen:?}");
    let registry = Registry::for_project(registry_dir.path(), root).load();
    let metadata = fs::symlink_metadata(folder(root).join("mobile.webm")).expect("target");
    assert!(
        registry.matches(RUN, "mobile.webm", &metadata),
        "registered"
    );
    assert!(
        registry.stamp(RUN, "mobile.jpg").is_some(),
        "its new poster too"
    );
    assert_eq!(
        fs::read(folder(root).join("mobile.mp4")).expect("source"),
        b"video",
        "the source stays"
    );
}

/// A failed run leaves neither the result nor its `.part`, and says ffmpeg's words.
#[cfg(unix)]
#[tokio::test]
async fn a_failed_run_leaves_nothing_behind() {
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let mut recorded = Recorded::default();
    recorded_video(&mut recorded, root, "mobile.webm");
    let tools = tempfile::tempdir().expect("temp dir");
    let ffmpeg = fake_ffmpeg(
        tools.path(),
        r#"for last; do :; done
printf half > "$last"
echo "Unknown encoder 'h264_mf'" >&2
exit 1"#,
    );
    let job = plan(root, &recorded, RUN, "mobile.webm", VideoTarget::Mp4, &[]).expect("planned");
    let (_cancel, mut cancelled) = tokio::sync::watch::channel(false);
    let error = make(
        &job,
        &ffmpeg,
        EncodeWith {
            encoder: Encoder::Mp4("h264_mf"),
            quality: 80,
            threads: 1,
        },
        None,
        &mut cancelled,
        &mut |_| {},
    )
    .await
    .expect_err("failed");
    assert!(error.to_string().contains("Unknown encoder"), "{error}");
    assert!(!folder(root).join("mobile.mp4").exists());
    assert!(!folder(root).join("mobile.mp4.part").exists());
}

/// The app's end cancels a running conversion: ffmpeg is stopped and its `.part` removed.
#[cfg(unix)]
#[tokio::test]
async fn a_cancelled_run_stops_ffmpeg_and_cleans_up() {
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let mut recorded = Recorded::default();
    recorded_video(&mut recorded, root, "mobile.webm");
    let tools = tempfile::tempdir().expect("temp dir");
    let ffmpeg = fake_ffmpeg(
        tools.path(),
        r#"for last; do :; done
printf half > "$last"
exec sleep 30"#,
    );
    let job = plan(root, &recorded, RUN, "mobile.webm", VideoTarget::Mp4, &[]).expect("planned");
    let (cancel, mut cancelled) = tokio::sync::watch::channel(false);
    let mut progress = |_: u8| {};
    let running = make(
        &job,
        &ffmpeg,
        EncodeWith {
            encoder: Encoder::Mp4("h264_mf"),
            quality: 80,
            threads: 1,
        },
        None,
        &mut cancelled,
        &mut progress,
    );
    let stopper = async {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let _ = cancel.send(true);
    };
    // The give-up deadline: a run that waited for the script's 30 s would not return.
    let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(running, stopper)
    })
    .await
    .expect("the cancel returns");
    assert_eq!(
        result.expect_err("cancelled"),
        ConvertError::Ffmpeg(CANCELLED.to_owned())
    );
    assert!(!folder(root).join("mobile.mp4.part").exists());
    assert!(!folder(root).join("mobile.mp4").exists());
}
