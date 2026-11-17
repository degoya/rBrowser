use std::fs;
use std::path::Path;

use super::*;
use crate::video::registry::FileStamp;

const OLD: &str = "2026-11-14_09-00-00";
const NEW: &str = "2026-11-16_14-03-07";

/// A video file `name` of `bytes` bytes in recording `id` below `root`, not registered.
fn planted(root: &Path, id: &str, name: &str, bytes: usize) {
    let dir = root.join(".rbrowser/out/video").join(id);
    fs::create_dir_all(&dir).expect("folder");
    fs::write(dir.join(name), vec![7_u8; bytes]).expect("file");
}

/// Registers file `name` of recording `id` as it is on disk now: what the recorder does.
fn register(recorded: &mut Recorded, root: &Path, id: &str, name: &str) {
    let path = root.join(".rbrowser/out/video").join(id).join(name);
    let metadata = fs::symlink_metadata(path).expect("metadata");
    recorded
        .recordings
        .entry(id.to_owned())
        .or_default()
        .insert(name.to_owned(), FileStamp::of(&metadata).expect("a stamp"));
}

/// A video file the recorder wrote: on disk and registered.
fn video(recorded: &mut Recorded, root: &Path, id: &str, name: &str, bytes: usize) {
    planted(root, id, name, bytes);
    register(recorded, root, id, name);
}

fn ids(recordings: &[RecordingEntry]) -> Vec<&str> {
    recordings
        .iter()
        .map(|recording| recording.id.as_str())
        .collect()
}

#[test]
fn names_are_plain_video_files_and_recordings_the_recorders_stamps() {
    for good in [
        "mobile.mp4",
        "desktop.webm",
        "500x800@2.mp4",
        "tablet-3.MP4",
        "a+b.webm",
    ] {
        assert!(is_video_name(good), "{good}");
    }
    for bad in [
        "",
        "mobile",
        ".mp4",
        "mobile.mov",
        "mobile.mp4.txt",
        "../mobile.mp4",
        "a/b.mp4",
        "a\\b.mp4",
        "C:mobile.mp4",
        "con.mp4",
        "MOBILE~1.mp4",
        "mobile.mp4.",
        "mob ile.mp4",
        "mobile\u{0}.mp4",
        "m\u{f6}bile.mp4",
    ] {
        assert!(!is_video_name(bad), "{bad:?}");
    }
    assert!(is_recording_id(NEW));
    for bad in [
        "",
        ".",
        "..",
        "../x",
        "a/b",
        "nul",
        "zzzz",
        "empty",
        "2026-11-16",
        "2026-11-16_14-03-07x",
        "2026-11-16_14-03-0",
        "2026-11-16 14-03-07",
        "2026-11-16_14:03:07",
        "2026-1a-16_14-03-07",
        "\u{663}026-11-16_14-03-07",
    ] {
        assert!(!is_recording_id(bad), "{bad:?}");
    }
    assert_eq!(mime("mobile.mp4"), Some("video/mp4"));
    assert_eq!(mime("mobile.WEBM"), Some("video/webm"));
    assert_eq!(mime("mobile.png"), None);
    assert_eq!(breakpoint_of("500x800@2.mp4"), "500x800@2");
}

#[test]
fn the_list_is_newest_first_with_sizes_and_only_registered_video_files() {
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let mut recorded = Recorded::default();
    assert_eq!(list(root, &recorded).expect("no folder"), Vec::new());
    video(&mut recorded, root, OLD, "mobile.mp4", 10);
    video(&mut recorded, root, NEW, "mobile.mp4", 3);
    video(&mut recorded, root, NEW, "desktop.mp4", 5);
    planted(root, NEW, "notes.txt", 1);
    planted(root, NEW, "planted.webm", 1);
    planted(root, "2026-11-01_00-00-00", "mobile.webm", 1);
    fs::write(root.join(".rbrowser/out/video/stray.mp4"), b"x").expect("stray file");
    let recordings = list(root, &recorded).expect("lists");
    assert_eq!(
        ids(&recordings),
        [NEW, OLD],
        "a recording rBrowser did not write is left out"
    );
    let newest = &recordings[0];
    let files: Vec<(&str, u64)> = newest
        .files
        .iter()
        .map(|file| (file.name.as_str(), file.bytes))
        .collect();
    assert_eq!(files, [("desktop.mp4", 5), ("mobile.mp4", 3)]);
    assert!(newest.modified().is_some());
}

/// A checked-in recording (security review): a folder or file the registry does not know is
/// neither listed nor located nor opened, even with a name of the recorder's form.
#[test]
fn a_planted_recording_is_never_listed_or_served() {
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    planted(root, NEW, "a.webm", 3);
    let recorded = Recorded::default();
    assert_eq!(list(root, &recorded).expect("lists"), Vec::new());
    assert_eq!(
        locate(root, &recorded, NEW, None),
        Err(LibraryError::NotFound)
    );
    assert_eq!(
        locate(root, &recorded, NEW, Some("a.webm")),
        Err(LibraryError::NotFound)
    );
    assert!(open_video(root, &recorded, NEW, "a.webm").is_err());
    assert_eq!(
        delete(root, &recorded, NEW, None, &[]),
        Err(LibraryError::NotFound)
    );
    assert!(
        root.join(".rbrowser/out/video")
            .join(NEW)
            .join("a.webm")
            .exists()
    );
}

/// A registered file somebody changed since (another size) is not served, and a planted file
/// beside registered ones is not listed.
#[test]
fn a_changed_file_is_no_longer_served() {
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let mut recorded = Recorded::default();
    video(&mut recorded, root, NEW, "mobile.mp4", 3);
    assert!(open_video(root, &recorded, NEW, "mobile.mp4").is_ok());
    planted(root, NEW, "mobile.mp4", 30);
    assert!(matches!(
        locate(root, &recorded, NEW, Some("mobile.mp4")),
        Err(LibraryError::Refused(_))
    ));
    assert!(open_video(root, &recorded, NEW, "mobile.mp4").is_err());
    assert_eq!(list(root, &recorded).expect("lists"), Vec::new());
}

#[test]
fn locate_finds_only_files_of_the_recordings_folder() {
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let mut recorded = Recorded::default();
    video(&mut recorded, root, NEW, "mobile.mp4", 3);
    planted(root, NEW, "notes.txt", 1);
    let found = locate(root, &recorded, NEW, Some("mobile.mp4")).expect("found");
    assert_eq!(
        found,
        root.join(".rbrowser/out/video")
            .join(NEW)
            .join("mobile.mp4")
    );
    assert_eq!(
        locate(root, &recorded, NEW, None).expect("the folder"),
        root.join(".rbrowser/out/video").join(NEW)
    );
    assert_eq!(
        locate(root, &recorded, NEW, Some("tablet.mp4")),
        Err(LibraryError::NotFound)
    );
    assert_eq!(
        locate(root, &recorded, OLD, Some("mobile.mp4")),
        Err(LibraryError::NotFound)
    );
    for (id, file) in [
        (NEW, "notes.txt"),
        (NEW, "../mobile.mp4"),
        ("..", "mobile.mp4"),
        ("../video", "mobile.mp4"),
        (NEW, "/etc/passwd.mp4"),
    ] {
        assert!(
            matches!(
                locate(root, &recorded, id, Some(file)),
                Err(LibraryError::Refused(_))
            ),
            "{id}/{file}"
        );
    }
    let (_, opened) = open_video(root, &recorded, NEW, "mobile.mp4").expect("opens");
    assert_eq!(opened.len(), 3);
}

/// A recording of another project is never reached: the route names no root, only the open
/// project's recording and file.
#[test]
fn another_projects_recording_is_not_found() {
    let ours = tempfile::tempdir().expect("temp dir");
    let theirs = tempfile::tempdir().expect("temp dir");
    let mut recorded = Recorded::default();
    video(&mut recorded, theirs.path(), NEW, "mobile.mp4", 3);
    assert_eq!(
        locate(ours.path(), &recorded, NEW, Some("mobile.mp4")),
        Err(LibraryError::NotFound)
    );
}

#[cfg(unix)]
#[test]
fn no_link_is_followed_anywhere_on_the_way() {
    use std::os::unix::fs::symlink;
    let outside = tempfile::tempdir().expect("temp dir");
    let mut recorded = Recorded::default();
    video(&mut recorded, outside.path(), NEW, "mobile.mp4", 3);
    fs::write(outside.path().join("secret.mp4"), b"secret").expect("secret");

    // `.rbrowser/out` a link to another project's output.
    let linked_out = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(linked_out.path().join(".rbrowser")).expect("config folder");
    symlink(
        outside.path().join(".rbrowser/out"),
        linked_out.path().join(".rbrowser/out"),
    )
    .expect("link");
    assert!(matches!(
        locate(linked_out.path(), &recorded, NEW, Some("mobile.mp4")),
        Err(LibraryError::Refused(_))
    ));
    assert!(matches!(
        list(linked_out.path(), &recorded),
        Err(LibraryError::Refused(_))
    ));

    // A recording folder that is a link: neither listed nor reached.
    let linked_run = tempfile::tempdir().expect("temp dir");
    video(&mut recorded, linked_run.path(), OLD, "mobile.mp4", 1);
    symlink(
        outside.path().join(".rbrowser/out/video").join(NEW),
        linked_run.path().join(".rbrowser/out/video").join(NEW),
    )
    .expect("link");
    assert_eq!(
        ids(&list(linked_run.path(), &recorded).expect("lists")),
        [OLD]
    );
    assert!(matches!(
        locate(linked_run.path(), &recorded, NEW, Some("mobile.mp4")),
        Err(LibraryError::Refused(_))
    ));

    // A video file that is a link - even one registered under its name - is neither listed
    // nor reached nor opened nor deleted through.
    let linked_file = tempfile::tempdir().expect("temp dir");
    video(&mut recorded, linked_file.path(), NEW, "mobile.mp4", 1);
    let link = linked_file
        .path()
        .join(".rbrowser/out/video")
        .join(NEW)
        .join("desktop.mp4");
    symlink(outside.path().join("secret.mp4"), &link).expect("link");
    let target = fs::metadata(&link).expect("the target");
    recorded
        .recordings
        .entry(NEW.to_owned())
        .or_default()
        .insert(
            "desktop.mp4".to_owned(),
            FileStamp::of(&target).expect("stamp"),
        );
    let listed = list(linked_file.path(), &recorded).expect("lists");
    assert_eq!(listed[0].files.len(), 1, "the link is not listed");
    assert!(matches!(
        locate(linked_file.path(), &recorded, NEW, Some("desktop.mp4")),
        Err(LibraryError::Refused(_))
    ));
    assert!(open_video(linked_file.path(), &recorded, NEW, "desktop.mp4").is_err());
    assert!(matches!(
        delete(linked_file.path(), &recorded, NEW, Some("desktop.mp4"), &[]),
        Err(LibraryError::Refused(_))
    ));
    assert!(outside.path().join("secret.mp4").exists());
}

#[test]
fn delete_removes_registered_files_and_nothing_else() {
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let mut recorded = Recorded::default();
    video(&mut recorded, root, NEW, "mobile.mp4", 3);
    video(&mut recorded, root, NEW, "desktop.mp4", 3);
    video(&mut recorded, root, OLD, "mobile.mp4", 3);
    planted(root, OLD, "notes.txt", 1);
    planted(root, OLD, "planted.mp4", 1);
    assert_eq!(
        delete(root, &recorded, NEW, Some("mobile.mp4"), &[]).map(|deleted| deleted.names),
        Ok(vec!["mobile.mp4".to_owned()])
    );
    assert_eq!(
        delete(root, &recorded, NEW, Some("desktop.mp4"), &[]).map(|deleted| deleted.names),
        Ok(vec!["desktop.mp4".to_owned()])
    );
    assert!(
        !root.join(".rbrowser/out/video").join(NEW).exists(),
        "the folder goes with its last video"
    );
    assert_eq!(
        delete(root, &recorded, OLD, None, &[]).map(|deleted| deleted.names),
        Ok(vec!["mobile.mp4".to_owned()])
    );
    let old = root.join(".rbrowser/out/video").join(OLD);
    assert!(
        old.join("notes.txt").exists() && old.join("planted.mp4").exists(),
        "what rBrowser did not write stays, and its folder with it"
    );
    assert_eq!(
        delete(root, &recorded, OLD, Some("mobile.mp4"), &[]),
        Err(LibraryError::NotFound)
    );
}

#[test]
fn a_running_recording_is_not_deleted() {
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let mut recorded = Recorded::default();
    video(&mut recorded, root, NEW, "mobile.mp4", 3);
    assert_eq!(
        delete(root, &recorded, NEW, None, &[OLD, NEW]),
        Err(LibraryError::Running)
    );
    assert_eq!(
        delete(root, &recorded, NEW, Some("mobile.mp4"), &[NEW]),
        Err(LibraryError::Running)
    );
    assert!(locate(root, &recorded, NEW, Some("mobile.mp4")).is_ok());
}

/// One recording that cannot be read is left out; the list, the panel and the retention go on
/// with the others (review RB-110-12).
#[cfg(unix)]
#[test]
fn an_unreadable_recording_is_left_out_and_the_rest_listed() {
    use std::os::unix::fs::PermissionsExt;
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let mut recorded = Recorded::default();
    video(&mut recorded, root, NEW, "mobile.mp4", 3);
    video(&mut recorded, root, OLD, "mobile.mp4", 3);
    let locked = root.join(".rbrowser/out/video").join(NEW);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("locks");
    let listed = list(root, &recorded);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("unlocks");
    let listed = listed.expect("the list does not fail");
    assert!(ids(&listed).contains(&OLD), "{:?}", ids(&listed));
}

#[test]
fn a_deletion_says_whether_the_folder_went_and_an_empty_one_goes_later() {
    let project = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    let mut recorded = Recorded::default();
    video(&mut recorded, root, NEW, "mobile.mp4", 3);
    planted(root, NEW, "notes.txt", 1);
    let deleted = delete(root, &recorded, NEW, None, &[]).expect("deletes");
    assert_eq!(deleted.names, ["mobile.mp4"]);
    assert!(!deleted.folder_removed, "something else is in it");
    assert!(!remove_empty_folder(root, NEW));
    fs::remove_file(root.join(".rbrowser/out/video").join(NEW).join("notes.txt")).expect("frees");
    assert!(remove_empty_folder(root, NEW));
    assert!(!root.join(".rbrowser/out/video").join(NEW).exists());
    assert!(
        remove_empty_folder(root, NEW),
        "a folder that is gone is gone"
    );
}
