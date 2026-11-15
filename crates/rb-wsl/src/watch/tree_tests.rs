use std::fs;
use std::sync::mpsc;

use notify::event::{CreateKind, DataChange, ModifyKind, RemoveKind, RenameMode};
use notify::{Config, Event, EventKind, PollWatcher, RecommendedWatcher};

use super::*;
use crate::watch::WatchRules;

fn filter() -> Filter {
    Filter::new(&WatchRules::default()).expect("default rules")
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temp dir");
    for folder in [
        "src/styles",
        "node_modules/react",
        ".git/objects",
        ".rbrowser",
    ] {
        fs::create_dir_all(dir.path().join(folder)).expect("a folder");
    }
    fs::write(dir.path().join("src/styles/app.css"), "a{}").expect("a file");
    dir
}

fn watcher() -> RecommendedWatcher {
    let (tx, _rx) = mpsc::channel::<notify::Result<Event>>();
    RecommendedWatcher::new(tx, Config::default()).expect("a watcher")
}

fn event(kind: EventKind, path: &Path) -> Event {
    Event::new(kind).add_path(path.to_path_buf())
}

fn modify() -> EventKind {
    EventKind::Modify(ModifyKind::Data(DataChange::Content))
}

#[test]
fn per_folder_watches_skip_ignored_folders() {
    let dir = project();
    let tree =
        TreeWatch::new(dir.path(), filter(), watcher(), TreeMode::PerFolder).expect("watched");
    let root = dir.path();
    assert!(tree.watches(root));
    assert!(tree.watches(&root.join("src")));
    assert!(tree.watches(&root.join("src/styles")));
    assert!(tree.watches(&root.join(".rbrowser")));
    assert!(!tree.watches(&root.join("node_modules")));
    assert!(!tree.watches(&root.join("node_modules/react")));
    assert!(!tree.watches(&root.join(".git")));
    assert_eq!(tree.folders(), 4);
}

#[test]
fn a_recursive_watch_holds_the_root_alone() {
    let dir = project();
    let tree =
        TreeWatch::new(dir.path(), filter(), watcher(), TreeMode::Recursive).expect("watched");
    assert_eq!(tree.folders(), 1);
}

#[test]
fn events_become_relative_changes_and_ignored_ones_vanish() {
    let dir = project();
    let root = dir.path();
    let mut tree = TreeWatch::new(root, filter(), watcher(), TreeMode::Recursive).expect("watched");
    let seen = tree
        .on_event(&event(modify(), &root.join("src/styles/app.css")))
        .expect("an event");
    assert_eq!(
        seen,
        vec![Seen::Change(Change::new(
            "src/styles/app.css",
            ChangeKind::Modify
        ))]
    );
    let ignored = tree
        .on_event(&event(modify(), &root.join("node_modules/react/index.js")))
        .expect("an event");
    assert!(ignored.is_empty());
    let outside = tree
        .on_event(&event(modify(), Path::new("/somewhere/else.css")))
        .expect("an event");
    assert!(outside.is_empty());
}

#[test]
fn event_kinds_map_to_change_kinds_and_access_is_dropped() {
    let dir = project();
    let root = dir.path();
    let file = root.join("src/styles/app.css");
    let mut tree = TreeWatch::new(root, filter(), watcher(), TreeMode::Recursive).expect("watched");
    let kind_of = |tree: &mut TreeWatch<RecommendedWatcher>, kind| match tree
        .on_event(&event(kind, &file))
        .expect("an event")
        .as_slice()
    {
        [Seen::Change(change)] => Some(change.kind),
        _ => None,
    };
    assert_eq!(
        kind_of(&mut tree, EventKind::Create(CreateKind::File)),
        Some(ChangeKind::Create)
    );
    assert_eq!(
        kind_of(
            &mut tree,
            EventKind::Modify(ModifyKind::Name(RenameMode::To))
        ),
        Some(ChangeKind::Rename)
    );
    assert_eq!(
        kind_of(&mut tree, EventKind::Remove(RemoveKind::File)),
        Some(ChangeKind::Remove)
    );
    assert_eq!(
        kind_of(
            &mut tree,
            EventKind::Access(notify::event::AccessKind::Read)
        ),
        None
    );
}

#[test]
fn an_overflow_is_a_rescan() {
    let dir = project();
    let mut tree =
        TreeWatch::new(dir.path(), filter(), watcher(), TreeMode::PerFolder).expect("watched");
    let overflow = Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
    assert_eq!(
        tree.on_event(&overflow).expect("an event"),
        vec![Seen::Rescan]
    );
}

#[test]
fn a_new_folder_is_watched_and_a_removed_one_forgotten() {
    let dir = project();
    let root = dir.path();
    let mut tree = TreeWatch::new(root, filter(), watcher(), TreeMode::PerFolder).expect("watched");
    let added = root.join("src/components/forms");
    fs::create_dir_all(&added).expect("a folder");
    let seen = tree
        .on_event(&event(
            EventKind::Create(CreateKind::Folder),
            &root.join("src/components"),
        ))
        .expect("an event");
    assert_eq!(
        seen,
        vec![Seen::Change(Change::new(
            "src/components",
            ChangeKind::Create
        ))]
    );
    assert!(tree.watches(&root.join("src/components")));
    assert!(tree.watches(&added));

    fs::remove_dir_all(root.join("src/components")).expect("removed");
    tree.on_event(&event(
        EventKind::Remove(RemoveKind::Folder),
        &root.join("src/components"),
    ))
    .expect("an event");
    assert!(!tree.watches(&root.join("src/components")));
    assert!(!tree.watches(&added));
    assert!(tree.watches(&root.join("src/styles")));
}

#[test]
fn a_new_ignored_folder_is_not_watched() {
    let dir = project();
    let root = dir.path();
    let mut tree = TreeWatch::new(root, filter(), watcher(), TreeMode::PerFolder).expect("watched");
    fs::create_dir_all(root.join("src/node_modules/x")).expect("a folder");
    let seen = tree
        .on_event(&event(
            EventKind::Create(CreateKind::Folder),
            &root.join("src/node_modules"),
        ))
        .expect("an event");
    assert!(seen.is_empty());
    assert!(!tree.watches(&root.join("src/node_modules")));
}

#[test]
fn a_missing_root_is_refused() {
    let dir = project();
    let error = TreeWatch::new(
        &dir.path().join("nope"),
        filter(),
        watcher(),
        TreeMode::PerFolder,
    )
    .expect_err("no folder");
    assert_eq!(error.i18n_key(), "reload.error.root");
}

#[test]
fn polling_watches_folder_by_folder_too() {
    let dir = project();
    let (tx, _rx) = mpsc::channel::<notify::Result<Event>>();
    let poll = PollWatcher::new(
        tx,
        Config::default().with_poll_interval(std::time::Duration::from_secs(60)),
    )
    .expect("a poll watcher");
    let tree = TreeWatch::new(dir.path(), filter(), poll, TreeMode::PerFolder).expect("watched");
    assert_eq!(tree.folders(), 4);
}

/// A folder link made while the watch runs (to `..` here) is no folder of the project: the
/// watch never walks the tree it points to.
#[cfg(unix)]
#[test]
fn a_new_folder_link_is_not_walked() {
    let dir = project();
    let root = dir.path();
    let mut tree = TreeWatch::new(root, filter(), watcher(), TreeMode::PerFolder).expect("watched");
    let before = tree.folders();
    let link = root.join("src/up");
    std::os::unix::fs::symlink("..", &link).expect("a link");
    tree.on_event(&event(EventKind::Create(CreateKind::Folder), &link))
        .expect("an event");
    assert!(!tree.watches(&link));
    assert!(!tree.watches(&link.join("styles")));
    assert_eq!(tree.folders(), before);
}
