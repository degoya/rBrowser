use std::fs;
use std::path::Path;

use super::*;
use crate::video::library::{self, LibraryError};
use crate::video::panel::register_saved;
use crate::video::registry::{FileStamp, Recorded, Registry};

const RUN: &str = "2026-11-16_14-03-07";

fn folder(root: &Path) -> std::path::PathBuf {
    root.join(".rbrowser/out/video").join(RUN)
}

/// File `name` of `bytes` bytes in the recording, not registered.
fn planted(root: &Path, name: &str, bytes: usize) {
    fs::create_dir_all(folder(root)).expect("folder");
    fs::write(folder(root).join(name), vec![7_u8; bytes]).expect("file");
}

/// File `name` as the recorder leaves it: on disk and registered.
fn written(recorded: &mut Recorded, root: &Path, name: &str, bytes: usize) {
    planted(root, name, bytes);
    let metadata = fs::symlink_metadata(folder(root).join(name)).expect("metadata");
    recorded
        .recordings
        .entry(RUN.to_owned())
        .or_default()
        .insert(name.to_owned(), FileStamp::of(&metadata).expect("a stamp"));
}

#[test]
fn a_poster_is_named_after_its_video() {
    assert_eq!(poster_name("mobile.webm").as_deref(), Some("mobile.jpg"));
    assert_eq!(
        poster_name("500x800@2.mp4").as_deref(),
        Some("500x800@2.jpg")
    );
    assert_eq!(poster_name("notes.txt"), None);
    assert_eq!(poster_name("mobile.jpg"), None, "a poster has no poster");
    for good in ["mobile.jpg", "500x800@2.jpg", "a+b-3.jpg"] {
        assert!(is_poster_name(good), "{good}");
    }
    for bad in [
        "",
        ".jpg",
        "mobile",
        "mobile.JPG",
        "mobile.jpeg",
        "mobile.png",
        "mobile.webm",
        "../mobile.jpg",
        "a/b.jpg",
        "con.jpg",
        "MOBILE~1.jpg",
        "mob ile.jpg",
    ] {
        assert!(!is_poster_name(bad), "{bad:?}");
    }
}

#[test]
fn the_poster_is_one_scaled_frame_and_never_overwrites() {
    let args: Vec<String> = poster_args(Path::new("in.webm"), Path::new("in.jpg"))
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let after = |flag: &str| {
        args.iter()
            .position(|arg| arg == flag)
            .and_then(|index| args.get(index + 1))
            .cloned()
    };
    assert!(args.contains(&"-n".to_owned()), "{args:?}");
    assert_eq!(after("-i").as_deref(), Some("in.webm"));
    assert_eq!(after("-frames:v").as_deref(), Some("1"));
    assert!(
        after("-vf").is_some_and(|filter| filter.starts_with("scale=")),
        "{args:?}"
    );
    // RB-090-52, acceptance 2026-11-17: the video is limited range BT.709 with square pixels; a
    // JPEG is full range BT.601, so the poster is converted, not merely re-tagged.
    assert_eq!(
        after("-vf").as_deref(),
        Some("scale='min(640,iw)':-2:out_range=pc:out_color_matrix=bt601"),
        "{args:?}"
    );
    assert_eq!(after("-pix_fmt").as_deref(), Some("yuvj420p"));
    assert_eq!(args.last().map(String::as_str), Some("in.jpg"));
}

/// The recorder registers a poster only beside a video it wrote in the same go: a `.jpg` the
/// recorder did not make for one of its videos is not taken.
#[test]
fn a_poster_is_registered_only_with_its_video() {
    let project = tempfile::tempdir().expect("temp dir");
    let registry_dir = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    planted(root, "mobile.webm", 4);
    planted(root, "mobile.jpg", 2);
    planted(root, "other.jpg", 2);
    let names = ["mobile.webm", "mobile.jpg", "other.jpg"].map(str::to_owned);
    register_saved(registry_dir.path(), &folder(root), &names).expect("registers");
    let recorded = Registry::for_project(registry_dir.path(), root).load();
    let files: Vec<&str> = recorded.recordings[RUN]
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(files, ["mobile.jpg", "mobile.webm"]);
}

#[test]
fn a_registered_poster_is_listed_and_served_as_a_jpeg() {
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let mut recorded = Recorded::default();
    written(&mut recorded, root, "mobile.webm", 4);
    written(&mut recorded, root, "mobile.jpg", 2);
    written(&mut recorded, root, "desktop.webm", 4);
    let recordings = library::list(root, &recorded).expect("lists");
    let files: Vec<(&str, Option<&str>)> = recordings[0]
        .files
        .iter()
        .map(|file| (file.name.as_str(), file.poster.as_deref()))
        .collect();
    assert_eq!(
        files,
        [("desktop.webm", None), ("mobile.webm", Some("mobile.jpg"))],
        "a poster is no video of its own"
    );
    assert_eq!(library::mime("mobile.jpg"), Some(MIME));
    let (_, metadata) = library::open_video(root, &recorded, RUN, "mobile.jpg").expect("opens");
    assert_eq!(metadata.len(), 2);
    assert!(
        matches!(
            library::locate(root, &recorded, RUN, Some("mobile.jpg")),
            Err(LibraryError::Refused(_))
        ),
        "shown or deleted by name: videos only"
    );
}

/// The same rule as for the videos (security review): a poster rBrowser did not write, or one
/// changed since, is neither listed nor served.
#[test]
fn a_planted_or_changed_poster_is_neither_listed_nor_served() {
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let mut recorded = Recorded::default();
    written(&mut recorded, root, "mobile.webm", 4);
    planted(root, "mobile.jpg", 2);
    let listed = library::list(root, &recorded).expect("lists");
    assert_eq!(listed[0].files[0].poster, None);
    assert_eq!(
        library::open_video(root, &recorded, RUN, "mobile.jpg").map(|_| ()),
        Err(LibraryError::NotFound)
    );
    written(&mut recorded, root, "mobile.jpg", 2);
    fs::write(folder(root).join("mobile.jpg"), b"changed since").expect("changes");
    let listed = library::list(root, &recorded).expect("lists");
    assert_eq!(listed[0].files[0].poster, None);
    assert!(library::open_video(root, &recorded, RUN, "mobile.jpg").is_err());
}

#[test]
fn deleting_a_video_takes_its_poster_and_a_recording_all_of_them() {
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let mut recorded = Recorded::default();
    written(&mut recorded, root, "mobile.webm", 4);
    written(&mut recorded, root, "mobile.jpg", 2);
    written(&mut recorded, root, "desktop.webm", 4);
    written(&mut recorded, root, "desktop.jpg", 2);
    let one = library::delete(root, &recorded, RUN, Some("mobile.webm"), &[]).expect("deletes");
    assert_eq!(one.names, ["mobile.webm"]);
    assert!(!one.folder_removed);
    assert!(!folder(root).join("mobile.jpg").exists(), "its poster went");
    assert!(folder(root).join("desktop.jpg").exists());
    let all = library::delete(root, &recorded, RUN, None, &[]).expect("deletes");
    assert_eq!(all.names, ["desktop.webm"]);
    assert!(all.folder_removed, "no poster keeps the folder");
}

#[test]
fn the_registry_forgets_a_poster_with_its_video() {
    let registry_dir = tempfile::tempdir().expect("temp dir");
    let registry = Registry::for_project(registry_dir.path(), Path::new("/work/shop"));
    let stamp = FileStamp {
        bytes: 1,
        modified_ms: 1,
    };
    let files = ["mobile.webm", "mobile.jpg", "desktop.webm", "desktop.jpg"]
        .map(|name| (name.to_owned(), stamp));
    registry.record(RUN, &files).expect("records");
    registry
        .forget_deleted(RUN, Some("mobile.webm"), false)
        .expect("forgets");
    let left: Vec<String> = registry.load().recordings[RUN].keys().cloned().collect();
    assert_eq!(left, ["desktop.jpg", "desktop.webm"]);
    registry
        .forget_deleted(RUN, Some("desktop.webm"), true)
        .expect("forgets");
    assert!(
        !registry.load().knows(RUN),
        "a recording left with posters only goes"
    );
}

/// RB-090-52: a WebM and its MP4 export share the breakpoint's poster; it stays while one of
/// them does, in the folder and in the registry.
#[test]
fn a_poster_shared_with_an_export_stays_with_the_other_video() {
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let mut recorded = Recorded::default();
    written(&mut recorded, root, "mobile.webm", 4);
    written(&mut recorded, root, "mobile.mp4", 4);
    written(&mut recorded, root, "mobile.jpg", 2);
    assert!(recorded.poster_shared(RUN, "mobile.mp4"));
    assert!(!recorded.poster_shared(RUN, "desktop.webm"));
    let one = library::delete(root, &recorded, RUN, Some("mobile.mp4"), &[]).expect("deletes");
    assert_eq!(one.names, ["mobile.mp4"]);
    assert!(
        folder(root).join("mobile.jpg").exists(),
        "the WebM keeps it"
    );

    let registry_dir = tempfile::tempdir().expect("temp dir");
    let registry = Registry::for_project(registry_dir.path(), Path::new("/work/shop"));
    let stamp = FileStamp {
        bytes: 1,
        modified_ms: 1,
    };
    let files = ["mobile.webm", "mobile.mp4", "mobile.jpg"].map(|name| (name.to_owned(), stamp));
    registry.record(RUN, &files).expect("records");
    registry
        .forget_deleted(RUN, Some("mobile.mp4"), false)
        .expect("forgets");
    let left: Vec<String> = registry.load().recordings[RUN].keys().cloned().collect();
    assert_eq!(left, ["mobile.jpg", "mobile.webm"]);
    registry
        .forget_deleted(RUN, Some("mobile.webm"), false)
        .expect("forgets");
    assert!(!registry.load().knows(RUN), "the last video takes it");
}
