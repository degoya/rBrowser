use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::*;
use crate::video::library::VideoEntry;
use crate::video::registry::FileStamp;

const OLD: &str = "2026-11-14_09-00-00";
const MID: &str = "2026-11-15_10-30-00";
const NEW: &str = "2026-11-16_14-03-07";

fn entry(id: &str, age_days: u64, now: SystemTime) -> RecordingEntry {
    RecordingEntry {
        id: id.to_owned(),
        dir: Path::new("unused").join(id),
        files: vec![VideoEntry {
            name: "mobile.mp4".to_owned(),
            bytes: 1,
            modified: now.checked_sub(Duration::from_secs(age_days * 86_400 + 60)),
            poster: None,
        }],
    }
}

#[test]
fn retention_keeps_the_newest_and_the_young_and_never_the_protected() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    let recordings = [entry(NEW, 0, now), entry(MID, 2, now), entry(OLD, 10, now)];
    let none = Vec::<String>::new();
    assert_eq!(expired(&recordings, 0, 0, now, &[]), none, "0 keeps all");
    assert_eq!(expired(&recordings, 1, 0, now, &[]), [MID, OLD]);
    assert_eq!(expired(&recordings, 5, 0, now, &[]), none);
    assert_eq!(expired(&recordings, 0, 3, now, &[]), [OLD]);
    assert_eq!(
        expired(&recordings, 2, 1, now, &[]),
        [MID, OLD],
        "either limit deletes"
    );
    assert_eq!(
        expired(&recordings, 0, 1, now, &[MID]),
        [OLD],
        "the recording just made is kept"
    );
    assert_eq!(
        expired(&recordings, 1, 0, now, &[NEW, MID]),
        [OLD],
        "the one being written is kept as well"
    );
    let undated = RecordingEntry {
        files: Vec::new(),
        ..entry(OLD, 0, now)
    };
    assert_eq!(expired(&[undated], 0, 1, now, &[]), none);
}

#[test]
fn retention_deletes_only_what_rbrowser_wrote_and_forgets_it() {
    let project = tempfile::tempdir().expect("temp dir");
    let data = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let registry = Registry::for_project(data.path(), root);
    for id in [OLD, MID, NEW, "2026-11-01_00-00-00"] {
        let dir = root.join(".rbrowser/out/video").join(id);
        fs::create_dir_all(&dir).expect("folder");
        fs::write(dir.join("mobile.mp4"), b"0123").expect("file");
        if id != "2026-11-01_00-00-00" {
            let stamp = FileStamp::of(&fs::metadata(dir.join("mobile.mp4")).expect("metadata"))
                .expect("stamp");
            registry
                .record(id, &[("mobile.mp4".to_owned(), stamp)])
                .expect("records");
        }
    }
    let now = SystemTime::now();
    assert_eq!(
        apply(root, &registry, 2, 0, now, &[NEW]).expect("applies"),
        [OLD]
    );
    assert!(!registry.load().knows(OLD), "forgotten");
    assert!(
        root.join(".rbrowser/out/video/2026-11-01_00-00-00/mobile.mp4")
            .exists(),
        "a planted recording is never the retention's"
    );
    assert_eq!(
        apply(root, &registry, 0, 0, now, &[]).expect("applies"),
        Vec::<String>::new()
    );
}

/// A folder that cannot be removed yet (on Windows: a player still holds a deleted file; here:
/// something else lies in it) stays pending and goes on a later run (review RB-110-12).
#[test]
fn a_folder_that_stayed_is_removed_on_a_later_run() {
    let project = tempfile::tempdir().expect("temp dir");
    let data = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let registry = Registry::for_project(data.path(), root);
    for id in [OLD, NEW] {
        let dir = root.join(".rbrowser/out/video").join(id);
        fs::create_dir_all(&dir).expect("folder");
        fs::write(dir.join("mobile.mp4"), b"0123").expect("file");
        let stamp =
            FileStamp::of(&fs::metadata(dir.join("mobile.mp4")).expect("metadata")).expect("stamp");
        registry
            .record(id, &[("mobile.mp4".to_owned(), stamp)])
            .expect("records");
    }
    let old = root.join(".rbrowser/out/video").join(OLD);
    fs::write(old.join("held.tmp"), b"x").expect("a file that keeps the folder");
    let now = SystemTime::now();
    assert_eq!(
        apply(root, &registry, 1, 0, now, &[NEW]).expect("applies"),
        [OLD]
    );
    let recorded = registry.load();
    assert!(!recorded.knows(OLD));
    assert!(recorded.pending.contains(OLD), "its folder stayed");
    assert!(old.exists());
    assert_eq!(
        settle_pending(root, &registry),
        Vec::<String>::new(),
        "still held"
    );
    fs::remove_file(old.join("held.tmp")).expect("lets go");
    assert_eq!(
        apply(root, &registry, 1, 0, now, &[NEW]).expect("applies"),
        Vec::<String>::new()
    );
    assert!(!old.exists(), "removed on the next run");
    assert!(registry.load().pending.is_empty());
}
