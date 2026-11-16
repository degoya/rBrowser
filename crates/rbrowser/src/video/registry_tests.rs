use std::fs;
use std::path::Path;

use super::*;

const RUN: &str = "2026-11-16_14-03-07";

fn stamp(bytes: u64) -> FileStamp {
    FileStamp {
        bytes,
        modified_ms: 1_800_000_000_000,
    }
}

#[test]
fn nothing_recorded_is_an_empty_registry() {
    let data = tempfile::tempdir().expect("temp dir");
    let registry = Registry::for_project(data.path(), Path::new("/work/shop"));
    let recorded = registry.load();
    assert!(recorded.recordings.is_empty());
    assert!(!recorded.knows(RUN));
}

#[test]
fn a_saved_recording_is_known_per_project_and_forgotten_again() {
    let data = tempfile::tempdir().expect("temp dir");
    let shop = Registry::for_project(data.path(), Path::new("/work/shop"));
    let other = Registry::for_project(data.path(), Path::new("/work/other"));
    shop.record(
        RUN,
        &[
            ("mobile.mp4".to_owned(), stamp(3)),
            ("desktop.mp4".to_owned(), stamp(5)),
        ],
    )
    .expect("records");
    let recorded = shop.load();
    assert!(recorded.knows(RUN));
    assert_eq!(recorded.stamp(RUN, "mobile.mp4"), Some(stamp(3)));
    assert_eq!(recorded.stamp(RUN, "tablet.mp4"), None);
    assert!(!other.load().knows(RUN), "another project's registry");

    shop.forget(RUN, Some("mobile.mp4"))
        .expect("forgets a file");
    assert_eq!(shop.load().stamp(RUN, "mobile.mp4"), None);
    assert!(shop.load().knows(RUN));
    shop.forget(RUN, Some("desktop.mp4"))
        .expect("forgets the last file");
    assert!(!shop.load().knows(RUN), "a recording without files is gone");
    shop.record(RUN, &[("mobile.mp4".to_owned(), stamp(3))])
        .expect("records");
    shop.forget(RUN, None).expect("forgets the recording");
    assert!(!shop.load().knows(RUN));
}

/// The registry lives outside the project: a checked-in file cannot add to it, and a damaged
/// or foreign file reads as empty.
#[test]
fn a_damaged_or_foreign_registry_file_knows_nothing() {
    let data = tempfile::tempdir().expect("temp dir");
    let registry = Registry::for_project(data.path(), Path::new("/work/shop"));
    registry
        .record(RUN, &[("mobile.mp4".to_owned(), stamp(3))])
        .expect("records");
    let file = fs::read_dir(data.path())
        .expect("lists")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "json"))
        .expect("one file");
    let text = fs::read_to_string(&file).expect("reads");
    fs::write(&file, text.replace("/work/shop", "/work/other")).expect("writes");
    assert!(!registry.load().knows(RUN), "a file of another root");
    fs::write(&file, b"{ not json").expect("writes");
    assert!(!registry.load().knows(RUN));
}

#[test]
fn a_stamp_comes_from_the_files_size_and_time() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("mobile.mp4");
    fs::write(&path, b"0123").expect("writes");
    let metadata = fs::symlink_metadata(&path).expect("metadata");
    let taken = FileStamp::of(&metadata).expect("a stamp");
    assert_eq!(taken.bytes, 4);
    assert!(taken.modified_ms > 0);
    let recorded = Recorded {
        root: String::new(),
        recordings: [(
            RUN.to_owned(),
            [("mobile.mp4".to_owned(), taken)].into_iter().collect(),
        )]
        .into_iter()
        .collect(),
        ..Recorded::default()
    };
    assert!(recorded.matches(RUN, "mobile.mp4", &metadata));
    fs::write(&path, b"012345").expect("changes");
    let changed = fs::symlink_metadata(&path).expect("metadata");
    assert!(
        !recorded.matches(RUN, "mobile.mp4", &changed),
        "another size"
    );
    assert!(!recorded.matches(RUN, "tablet.mp4", &metadata));
}

/// A recording deleted while its folder stays (Windows: a player still holds a file) is kept as
/// pending until the folder is gone (review RB-110-12).
#[test]
fn a_deletion_whose_folder_stayed_is_pending_until_cleared() {
    let data = tempfile::tempdir().expect("temp dir");
    let registry = Registry::for_project(data.path(), Path::new("/work/shop"));
    registry
        .record(
            RUN,
            &[
                ("mobile.mp4".to_owned(), stamp(3)),
                ("desktop.mp4".to_owned(), stamp(5)),
            ],
        )
        .expect("records");
    registry
        .forget_deleted(RUN, Some("mobile.mp4"), false)
        .expect("forgets");
    assert!(
        registry.load().pending.is_empty(),
        "a file of a recording that has more"
    );
    registry
        .forget_deleted(RUN, Some("desktop.mp4"), false)
        .expect("forgets");
    let recorded = registry.load();
    assert!(!recorded.knows(RUN));
    assert!(recorded.pending.contains(RUN));
    registry.clear_pending(&[RUN.to_owned()]).expect("clears");
    assert!(registry.load().pending.is_empty());
    registry
        .record(RUN, &[("mobile.mp4".to_owned(), stamp(3))])
        .expect("records");
    registry.forget_deleted(RUN, None, true).expect("forgets");
    assert!(registry.load().pending.is_empty(), "the folder went");
}

/// The route reads the registry per request: parsed once and again only when the file changed
/// (review RB-110-12); a change through the registry itself is seen at once.
#[test]
fn the_cached_registry_follows_every_change() {
    let data = tempfile::tempdir().expect("temp dir");
    let registry = Registry::for_project(data.path(), Path::new("/work/shop"));
    assert!(!registry.load_cached().knows(RUN));
    registry
        .record(RUN, &[("mobile.mp4".to_owned(), stamp(3))])
        .expect("records");
    assert!(registry.load_cached().knows(RUN));
    assert_eq!(registry.load_cached(), registry.load());
    registry.forget(RUN, None).expect("forgets");
    assert!(!registry.load_cached().knows(RUN));
}
