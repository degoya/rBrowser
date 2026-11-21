use std::fs;
use std::path::Path;
use std::sync::Arc;

use rb_config::UserSettings;
use rb_ipc::IpcErrorCode;
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

const NEW: &str = "2026-11-16_14-03-07";
const OLD: &str = "2026-11-15_10-30-00";

/// Writes video `name` of recording `id` below `root`, as a repository could.
fn planted(root: &Path, id: &str, name: &str) {
    let dir = root.join(".rbrowser/out/video").join(id);
    fs::create_dir_all(&dir).expect("folder");
    fs::write(dir.join(name), b"0123").expect("file");
}

/// Writes video `name` of recording `id` and registers it in `data`, as the recorder does.
fn recorded(data: &Path, root: &Path, id: &str, name: &str) {
    planted(root, id, name);
    let folder = root.join(".rbrowser/out/video").join(id);
    register_saved(data, &folder, &[name.to_owned()]).expect("registers");
}

fn core(root: Option<&Path>) -> Arc<Core> {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), root.map(Path::to_path_buf))
        .expect("opens");
    let setup = CoreSetup {
        user: UserSettings::default(),
        ..CoreSetup::default()
    };
    Core::new(host, ui, Handle::current(), setup)
}

#[test]
fn the_list_carries_urls_paths_and_breakpoints_newest_first() {
    let project = tempfile::tempdir().expect("temp dir");
    let data = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    recorded(data.path(), root, OLD, "mobile.mp4");
    recorded(data.path(), root, NEW, "desktop.webm");
    recorded(data.path(), root, NEW, "500x800@2.mp4");
    planted(root, NEW, "planted.webm");
    let known = Registry::for_project(data.path(), root).load();
    let list = video_list(Some(root), &known, true);
    assert!(list.project);
    assert!(list.autoplay);
    assert_eq!(list.error, None);
    let folder = root.join(".rbrowser").join("out").join("video");
    assert_eq!(list.folder, Some(folder.display().to_string()));
    let ids: Vec<&str> = list
        .recordings
        .iter()
        .map(|recording| recording.id.as_str())
        .collect();
    assert_eq!(ids, [NEW, OLD]);
    let newest = &list.recordings[0];
    assert_eq!(newest.path, folder.join(NEW).display().to_string());
    let names: Vec<&str> = newest.files.iter().map(|file| file.name.as_str()).collect();
    assert_eq!(
        names,
        ["500x800@2.mp4", "desktop.webm"],
        "the planted file is left out"
    );
    let first = &newest.files[0];
    assert_eq!(
        (first.name.as_str(), first.breakpoint.as_str(), first.bytes),
        ("500x800@2.mp4", "500x800@2", 4)
    );
    assert_eq!(first.url, format!("rbrowser://video/{NEW}/500x800@2.mp4"));
    assert_eq!(
        first.path,
        folder.join(NEW).join("500x800@2.mp4").display().to_string()
    );
}

#[test]
fn without_a_project_the_list_says_so() {
    let list = video_list(None, &Recorded::default(), false);
    assert_eq!(
        list,
        VideoList {
            project: false,
            ..VideoList::default()
        }
    );
}

#[test]
fn only_the_files_the_recorder_names_are_registered() {
    let project = tempfile::tempdir().expect("temp dir");
    let data = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    planted(root, NEW, "mobile.mp4");
    planted(root, NEW, "planted.webm");
    let folder = root.join(".rbrowser/out/video").join(NEW);
    register_saved(
        data.path(),
        &folder,
        &[
            "mobile.mp4".to_owned(),
            "notes.txt".to_owned(),
            "gone.mp4".to_owned(),
        ],
    )
    .expect("registers");
    let known = Registry::for_project(data.path(), root).load();
    assert!(known.stamp(NEW, "mobile.mp4").is_some());
    assert_eq!(known.stamp(NEW, "planted.webm"), None);
    assert_eq!(known.stamp(NEW, "gone.mp4"), None);
    assert!(register_saved(data.path(), Path::new("/x/y"), &[]).is_err());
}

#[cfg(unix)]
#[test]
fn a_linked_output_folder_is_an_error_not_a_list() {
    let outside = tempfile::tempdir().expect("temp dir");
    planted(outside.path(), NEW, "mobile.mp4");
    let project = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(project.path().join(".rbrowser")).expect("folder");
    std::os::unix::fs::symlink(
        outside.path().join(".rbrowser/out"),
        project.path().join(".rbrowser/out"),
    )
    .expect("link");
    let list = video_list(Some(project.path()), &Recorded::default(), false);
    assert!(list.recordings.is_empty());
    assert!(list.error.is_some());
}

#[tokio::test]
async fn the_panel_lists_and_deletes_through_the_ipc() {
    let project = tempfile::tempdir().expect("temp dir");
    let data = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    recorded(data.path(), root, NEW, "mobile.mp4");
    recorded(data.path(), root, NEW, "desktop.mp4");
    recorded(data.path(), root, OLD, "mobile.mp4");
    planted(root, "2026-11-01_00-00-00", "evil.webm");
    let core = core(Some(root));
    let dir = Some(data.path());
    let Some(Ok(CoreToShell::Videos(list))) = ipc_in(&core, &ShellToCore::GetVideos, dir) else {
        panic!("getVideos is this module's");
    };
    assert_eq!(
        list.recordings.len(),
        2,
        "the planted recording is not listed"
    );
    let delete = |recording: &str, file: Option<&str>| {
        ipc_in(
            &core,
            &ShellToCore::DeleteVideo(rb_ipc::VideoFileRef {
                recording: recording.to_owned(),
                file: file.map(str::to_owned),
            }),
            dir,
        )
        .expect("deleteVideo is this module's")
    };
    let Ok(CoreToShell::Videos(after)) = delete(NEW, Some("mobile.mp4")) else {
        panic!("answered with the list");
    };
    assert_eq!(after.recordings[0].files.len(), 1);
    let Ok(CoreToShell::Videos(after)) = delete(OLD, None) else {
        panic!("answered with the list");
    };
    let ids: Vec<&str> = after
        .recordings
        .iter()
        .map(|recording| recording.id.as_str())
        .collect();
    assert_eq!(ids, [NEW]);
    assert!(
        !Registry::for_project(data.path(), root).load().knows(OLD),
        "a deleted recording is forgotten"
    );
    assert_eq!(
        delete("..", None).map_err(|(code, _)| code),
        Err(IpcErrorCode::Refused)
    );
    assert_eq!(
        delete(OLD, Some("mobile.mp4")).map_err(|(code, _)| code),
        Err(IpcErrorCode::NotFound)
    );
    assert_eq!(
        delete("2026-11-01_00-00-00", None).map_err(|(code, _)| code),
        Err(IpcErrorCode::NotFound),
        "a planted recording is not deleted either"
    );
    assert!(ipc_in(&core, &ShellToCore::GetState, dir).is_none());
}

#[tokio::test]
async fn reveal_refuses_what_is_no_recording_and_needs_a_project() {
    let project = tempfile::tempdir().expect("temp dir");
    let data = tempfile::tempdir().expect("temp dir");
    let with = core(Some(project.path()));
    let dir = Some(data.path());
    let reveal = |core: &Arc<Core>, recording: &str, file: Option<&str>| {
        ipc_in(
            core,
            &ShellToCore::RevealVideo(rb_ipc::VideoFileRef {
                recording: recording.to_owned(),
                file: file.map(str::to_owned),
            }),
            dir,
        )
        .expect("revealVideo is this module's")
        .map_err(|(code, _)| code)
    };
    assert_eq!(
        reveal(&with, NEW, Some("../../secret.mp4")),
        Err(IpcErrorCode::Refused)
    );
    assert_eq!(reveal(&with, NEW, None), Err(IpcErrorCode::NotFound));
    let without = core(None);
    assert_eq!(reveal(&without, NEW, None), Err(IpcErrorCode::NotFound));
    let Some(Ok(CoreToShell::Videos(list))) = ipc_in(&without, &ShellToCore::GetVideos, dir) else {
        panic!("answered");
    };
    assert!(!list.project);
}

#[test]
fn the_tool_takes_no_arguments() {
    assert!(tool_args(&json!(null)).is_ok());
    assert!(tool_args(&json!({})).is_ok());
    assert!(tool_args(&json!({ "path": "/" })).is_err());
    assert!(tool_args(&json!([])).is_err());
}

#[test]
fn the_root_of_a_recordings_folder() {
    let root = Path::new("/work/shop");
    let folder = root.join(".rbrowser/out/video").join(NEW);
    assert_eq!(
        project_of(&folder),
        Some((root.to_path_buf(), NEW.to_owned()))
    );
    assert_eq!(project_of(Path::new("/x")), None);
}

/// RB-090-52: `convertVideo` is held to the panel's rules before anything is queued - only a
/// recorded video of the other format, and MP4 only where the platform has an H.264 encoder.
#[tokio::test]
async fn convert_refuses_what_it_may_not_make() {
    let project = tempfile::tempdir().expect("temp dir");
    let data = tempfile::tempdir().expect("temp dir");
    let root = project.path();
    recorded(data.path(), root, NEW, "mobile.webm");
    planted(root, NEW, "planted.mp4");
    let core = core(Some(root));
    let dir = Some(data.path());
    let convert = |file: &str, to: rb_ipc::VideoTarget| {
        ipc_in(
            &core,
            &ShellToCore::ConvertVideo(rb_ipc::ConvertVideoRequest {
                recording: NEW.to_owned(),
                file: file.to_owned(),
                to,
            }),
            dir,
        )
        .expect("convertVideo is this module's")
        .map_err(|(code, _)| code)
    };
    assert_eq!(
        convert("mobile.webm", rb_ipc::VideoTarget::Webm).err(),
        Some(IpcErrorCode::Refused),
        "a WebM is no MP4 to convert"
    );
    assert_eq!(
        convert("planted.mp4", rb_ipc::VideoTarget::Webm).err(),
        Some(IpcErrorCode::NotFound),
        "a file rBrowser did not record"
    );
    if crate::video::convert::mp4_encoder().is_none() {
        assert_eq!(
            convert("mobile.webm", rb_ipc::VideoTarget::Mp4).err(),
            Some(IpcErrorCode::Refused),
            "no H.264 encoder here"
        );
    }
    let list = video_list(Some(root), &Recorded::default(), false);
    assert_eq!(
        list.mp4_export,
        crate::video::convert::mp4_encoder().is_some()
    );
}
