use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rb_ipc::PathKind;

use super::*;
use crate::cdp::lock;
use crate::host::ui::PathPick;

type Seen = Arc<Mutex<Vec<Option<PathBuf>>>>;

fn pick_once() -> (Seen, PickOnce) {
    let seen: Seen = Arc::default();
    let sink = seen.clone();
    let once = PickOnce::new(Box::new(move |path| lock(&sink).push(path)));
    (seen, once)
}

fn pick(kind: PathKind) -> PathPick {
    PathPick {
        kind,
        title: "Choose".to_owned(),
        save_as: None,
    }
}

/// RB-080-73: "Save diagnostics..." asks where with the Save dialog, a file name proposed.
#[test]
fn a_pick_with_a_proposed_file_is_a_save_dialog() {
    let mut save = pick(PathKind::File);
    save.save_as = Some(PathBuf::from(
        "/home/me/Desktop/rbrowser-diagnostics-20261114-120000.zip",
    ));
    let request = DialogRequest::from(&save);
    assert_eq!(request.mode, DialogMode::Save);
    assert_eq!(
        request.default_path.as_deref(),
        Some("/home/me/Desktop/rbrowser-diagnostics-20261114-120000.zip")
    );
    assert_eq!(request.filters, [".zip"]);
    let open = DialogRequest::from(&pick(PathKind::File));
    assert_eq!(open.mode, DialogMode::OpenFile);
    assert_eq!(open.default_path, None);
}

/// CEF reads the `accept_filters` list of `run_file_dialog` without a null check: the request
/// always carries a list, empty when nothing is filtered (RB-080-19, the owner's crash).
#[test]
fn the_request_always_carries_a_filter_list() {
    let file = DialogRequest::from(&pick(PathKind::File));
    assert_eq!(file.mode, DialogMode::OpenFile);
    assert_eq!(file.title, "Choose");
    assert!(file.filters.is_empty());
    assert_eq!(
        DialogRequest::from(&pick(PathKind::Folder)).mode,
        DialogMode::OpenFolder
    );
}

#[test]
fn the_first_chosen_path_is_handed_on_once() {
    let (seen, once) = pick_once();
    assert!(once.finish(["C:\\Firefox\\firefox.exe".to_owned(), "D:\\x".to_owned()]));
    assert!(!once.finish(["D:\\again".to_owned()]));
    assert_eq!(
        *lock(&seen),
        vec![Some(PathBuf::from("C:\\Firefox\\firefox.exe"))]
    );
}

#[test]
fn cancelling_hands_on_none_once() {
    let (seen, once) = pick_once();
    assert!(once.finish(Vec::<String>::new()));
    assert!(!once.finish(Vec::<String>::new()));
    assert_eq!(*lock(&seen), vec![None]);
}

#[test]
fn an_empty_path_counts_as_cancelled() {
    let (seen, once) = pick_once();
    assert!(once.finish([String::new()]));
    assert_eq!(*lock(&seen), vec![None]);
}

/// A callback object CEF releases without calling it (the window closed under the dialog)
/// still answers the waiting page: cancelled, exactly once.
#[test]
fn dropping_an_unfinished_pick_answers_cancelled() {
    let (seen, once) = pick_once();
    drop(once);
    assert_eq!(*lock(&seen), vec![None]);
    let (seen, once) = pick_once();
    once.finish(["C:\\x".to_owned()]);
    drop(once);
    assert_eq!(*lock(&seen), vec![Some(PathBuf::from("C:\\x"))]);
}

#[test]
fn first_path_skips_empty_entries() {
    assert_eq!(
        first_path([String::new(), "C:\\x".to_owned()]),
        Some(PathBuf::from("C:\\x"))
    );
    assert_eq!(first_path(Vec::<String>::new()), None);
}
