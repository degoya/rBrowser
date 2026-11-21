use std::fs;

use super::*;
use crate::recorder::model::{RECORDING_VERSION, Recording};

const PROTECTED: &[&str] = &["flows", "snippets", "plugins", "mocks", "scripts"];

fn recording(name: &str) -> Recording {
    Recording {
        schema: None,
        version: RECORDING_VERSION,
        name: name.into(),
        start_url: Some("http://x.test/".into()),
        environment: None,
        breakpoints: Vec::new(),
        started_at_ms: 1,
        app_version: "0.1.0".into(),
        steps: Vec::new(),
    }
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(dir.path().join(".rbrowser")).expect("config dir");
    dir
}

#[test]
fn the_folder_must_lie_in_rbrowser_outside_the_protected_folders() {
    let root = project();
    for good in [DEFAULT_DIR, ".rbrowser/recordings", "./.rbrowser\\out\\rec"] {
        assert!(
            RecordingStore::open(root.path(), good, PROTECTED).is_ok(),
            "{good}"
        );
    }
    for bad in [
        ".rbrowser/flows",
        ".rbrowser/Flows/rec",
        ".rbrowser/plugins/x",
        ".rbrowser",
        "out/recordings",
        "../elsewhere",
        ".rbrowser/../../x",
        "/tmp/x",
        "",
    ] {
        assert!(
            matches!(
                RecordingStore::open(root.path(), bad, PROTECTED),
                Err(StoreError::BadDir(_))
            ),
            "{bad}"
        );
    }
}

#[test]
fn save_writes_and_never_overwrites() {
    let root = project();
    let store = RecordingStore::open(root.path(), DEFAULT_DIR, PROTECTED).expect("opens");
    let path = store.save(&recording("login")).expect("saves");
    assert_eq!(
        path,
        root.path().join(".rbrowser/out/recordings/login.json")
    );
    let before = fs::read_to_string(&path).expect("reads");
    let mut changed = recording("login");
    changed.app_version = "9.9.9".into();
    assert!(matches!(store.save(&changed), Err(StoreError::Exists(name)) if name == "login"));
    assert_eq!(fs::read_to_string(&path).expect("reads"), before);
}

#[test]
fn save_new_takes_the_next_free_name() {
    let root = project();
    let store = RecordingStore::open(root.path(), DEFAULT_DIR, PROTECTED).expect("opens");
    let mut first = recording("login");
    store.save_new(&mut first).expect("saves");
    let mut second = recording("login");
    let path = store.save_new(&mut second).expect("saves");
    assert_eq!(second.name, "login-2");
    assert!(path.ends_with("login-2.json"));
    assert_eq!(store.list().expect("lists"), ["login", "login-2"]);
    assert_eq!(store.load("login-2").expect("loads"), second);
}

#[test]
fn names_are_plain() {
    for bad in ["", "../x", "a/b", "a.b", "a b", &"x".repeat(65)] {
        assert!(check_name(bad).is_err(), "{bad}");
    }
    assert!(check_name("Login_2-b").is_ok());
    // RB-050-01 review: a Windows device name is no file name.
    for device in ["CON", "prn", "Aux", "nul", "COM1", "com9", "LPT1", "lpt9"] {
        assert!(check_name(device).is_err(), "{device}");
    }
    for fine in ["console", "com10", "lpt0", "nul-2", "conx"] {
        assert!(check_name(fine).is_ok(), "{fine}");
    }
    let root = project();
    let store = RecordingStore::open(root.path(), DEFAULT_DIR, PROTECTED).expect("opens");
    assert!(matches!(
        store.save(&recording("../../evil")),
        Err(StoreError::BadName(_))
    ));
    assert!(matches!(
        store.load("missing"),
        Err(StoreError::NotFound(_))
    ));
    assert!(store.list().expect("an absent folder lists").is_empty());
}

#[cfg(unix)]
#[test]
fn a_link_out_of_the_project_or_into_a_protected_folder_is_refused() {
    let root = project();
    let outside = tempfile::tempdir().expect("temp dir");
    std::os::unix::fs::symlink(outside.path(), root.path().join(".rbrowser/out")).expect("link");
    let store = RecordingStore::open(root.path(), DEFAULT_DIR, PROTECTED).expect("opens");
    assert!(matches!(
        store.save(&recording("a")),
        Err(StoreError::BadDir(_))
    ));
    assert!(
        fs::read_dir(outside.path())
            .expect("reads")
            .next()
            .is_none()
    );

    let root = project();
    fs::create_dir_all(root.path().join(".rbrowser/flows")).expect("flows");
    std::os::unix::fs::symlink(
        root.path().join(".rbrowser/flows"),
        root.path().join(".rbrowser/rec"),
    )
    .expect("link");
    let store = RecordingStore::open(root.path(), ".rbrowser/rec", PROTECTED).expect("opens");
    assert!(matches!(
        store.save(&recording("a")),
        Err(StoreError::BadDir(_))
    ));
}

#[test]
fn replace_writes_over_an_existing_recording_only() {
    // The Recorder panel's save (RB-090-60): an edited recording goes back into its file.
    let root = project();
    let store = RecordingStore::open(root.path(), DEFAULT_DIR, PROTECTED).expect("store");
    assert!(matches!(
        store.replace(&recording("login")),
        Err(StoreError::NotFound(_))
    ));
    store.save(&recording("login")).expect("saved");
    let mut edited = recording("login");
    edited.start_url = Some("http://x.test/edited".into());
    store.replace(&edited).expect("replaced");
    assert_eq!(store.load("login").expect("loads"), edited);
    // No temporary file stays beside it.
    assert_eq!(store.list().expect("lists"), vec!["login".to_owned()]);
    let entries = fs::read_dir(store.dir()).expect("folder").count();
    assert_eq!(entries, 1);
    assert!(matches!(
        store.replace(&recording("../x")),
        Err(StoreError::BadName(_))
    ));
}

#[test]
fn delete_removes_one_stored_recording_only() {
    // The Recorder panel's "Delete" (RB-090-91).
    let root = project();
    let store = RecordingStore::open(root.path(), DEFAULT_DIR, PROTECTED).expect("store");
    store.save(&recording("login")).expect("saved");
    store.save(&recording("old")).expect("saved");
    store.delete("old").expect("deleted");
    assert_eq!(store.list().expect("lists"), vec!["login".to_owned()]);
    assert!(matches!(store.delete("old"), Err(StoreError::NotFound(_))));
    for bad in ["../config", "a/b", "", "old.json", "CON"] {
        assert!(
            matches!(store.delete(bad), Err(StoreError::BadName(_))),
            "{bad:?}"
        );
    }
    assert!(root.path().join(".rbrowser").exists());
    // A folder of that name is no recording.
    fs::create_dir_all(store.dir().join("dir.json")).expect("folder");
    assert!(matches!(store.delete("dir"), Err(StoreError::NotFound(_))));
    assert!(store.dir().join("dir.json").is_dir());
    assert!(store.path_of("login").expect("a path").is_file());
}

/// RB-092-01 review: read-modify-write under the store's lock loses no update, however many
/// writers there are at once (an agent's step tools, the panel's save, a continued recording).
#[test]
fn edits_under_the_write_lock_lose_no_update() {
    let root = project();
    let store = RecordingStore::open(root.path(), DEFAULT_DIR, PROTECTED).expect("store");
    store.save(&recording("search")).expect("saved");
    let writers: Vec<_> = (0..4)
        .map(|writer| {
            let store = store.clone();
            std::thread::spawn(move || {
                for edit in 0..10 {
                    let _guard = write_lock();
                    let mut read = store.load("search").expect("read");
                    read.start_url = Some(format!("http://x.test/{writer}/{edit}"));
                    read.started_at_ms += 1;
                    store.replace(&read).expect("written");
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().expect("a writer");
    }
    assert_eq!(store.load("search").expect("read").started_at_ms, 1 + 40);
    let left: Vec<String> = fs::read_dir(store.dir())
        .expect("the folder")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(left.is_empty(), "{left:?}");
}
